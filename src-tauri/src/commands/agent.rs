//! AI Agent 命令层：
//! - 流式对话（OpenAI 兼容 / Anthropic 双协议 SSE，事件推送增量，可取消）
//! - 命令执行（工具链环境注入、输出流事件、进程树中止、危险命令硬拦截）
//! - 会话持久化（.easyide/sessions/*.json）
//! Agent 循环（工具调用→观察→继续）由前端编排，Rust 是无状态执行器 + 确认门后端。

use crate::config;
use crate::state::AppState;
use crate::toolchain;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AgentEvent {
    pub id: u64,
    pub text: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AgentErrorEvent {
    pub id: u64,
    pub error: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ConsoleLine {
    pub id: u32,
    pub line: String,
    pub stderr: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ConsoleExit {
    pub id: u32,
    pub code: Option<i32>,
}

static STREAM_SEQ: AtomicU64 = AtomicU64::new(1);
static CMD_SEQ: AtomicU64 = AtomicU64::new(1);

/// 进行中的流式请求取消标记
static CANCELS: Mutex<Option<HashMap<u64, Arc<AtomicBool>>>> = Mutex::new(None);
/// 进行中的命令：cmd_id -> 系统进程 pid（中止用 taskkill 进程树，不持进程句柄，避免与 wait 抢锁）
static LIVE_PIDS: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);

fn insert_cancel(id: u64, flag: Arc<AtomicBool>) {
    CANCELS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, flag);
}

fn remove_cancel(id: u64) {
    if let Some(map) = CANCELS.lock().unwrap().as_mut() {
        map.remove(&id);
    }
}

fn cancel_stream(id: u64) {
    if let Some(map) = CANCELS.lock().unwrap().as_ref() {
        if let Some(flag) = map.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

fn insert_pid(cmd_id: u32, pid: u32) {
    LIVE_PIDS.lock().unwrap().get_or_insert_with(HashMap::new).insert(cmd_id, pid);
}

fn remove_pid(cmd_id: u32) {
    if let Some(map) = LIVE_PIDS.lock().unwrap().as_mut() {
        map.remove(&cmd_id);
    }
}

fn pid_of(cmd_id: u32) -> Option<u32> {
    LIVE_PIDS.lock().unwrap().as_ref()?.get(&cmd_id).copied()
}

/// 危险命令硬拦截（确认门之外的第二道防线；前端已确认的命令也过这里）
fn is_dangerous(command: &str) -> Option<&'static str> {
    let lower = command.to_lowercase();
    let checks: [(&str, &str); 12] = [
        ("rm -rf", "递归强制删除"),
        ("rd /s", "递归删除目录"),
        ("del /f /s /q", "强制删除"),
        ("format ", "格式化磁盘"),
        ("mkfs", "格式化文件系统"),
        ("reg delete", "删除注册表项"),
        ("reg add", "写注册表"),
        ("shutdown", "关机/重启"),
        ("taskkill /f /im", "强制结束进程"),
        ("diskpart", "磁盘分区"),
        ("cipher /w", "擦除磁盘数据"),
        ("powershell -enc", "编码执行脚本"),
    ];
    checks
        .iter()
        .find(|(pat, _)| lower.contains(pat))
        .map(|(_, why)| *why)
}

// ---------------- 流式对话 ----------------

/// 发起流式对话：立即返回流 id；增量经 `agent-delta`、完成经 `agent-done`、失败经 `agent-error` 推送
#[tauri::command]
pub fn agent_chat_stream(
    app: AppHandle,
    messages: Vec<AgentMessage>,
    system: String,
) -> Result<u64, String> {
    let settings = config::load_settings(&app);
    if settings.ai_api_key.trim().is_empty() {
        return Err("未配置 AI API Key，请到 设置 → AI 中填写".into());
    }
    let id = STREAM_SEQ.fetch_add(1, Ordering::Relaxed);
    let cancel = Arc::new(AtomicBool::new(false));
    insert_cancel(id, cancel.clone());

    let history: Vec<Value> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    std::thread::spawn(move || {
        let result = stream_chat_inner(&app, &settings, &history, &system, &cancel, id);
        remove_cancel(id);
        match result {
            Ok(full) => {
                let _ = app.emit("agent-done", AgentEvent { id, text: full });
            }
            Err(e) => {
                let _ = app.emit("agent-error", AgentErrorEvent { id, error: e });
            }
        }
    });
    Ok(id)
}

/// 取消进行中的流式请求
#[tauri::command]
pub fn agent_cancel(id: u64) {
    cancel_stream(id);
}

fn stream_chat_inner(
    app: &AppHandle,
    settings: &config::AppSettings,
    history: &[Value],
    system: &str,
    cancel: &AtomicBool,
    stream_id: u64,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP 客户端创建失败：{}", e))?;

    let base = settings.ai_base_url.trim().trim_end_matches('/').to_string();
    let anthropic = settings.ai_provider == "anthropic";

    let (url, headers, body) = if anthropic {
        let url = if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        };
        let mut body = json!({
            "model": settings.ai_model,
            "max_tokens": 8192,
            "stream": true,
            "messages": history,
            "system": system,
        });
        // 与插件一致：部分模型不接受 max_tokens 以外的限制，这里不额外传
        let _ = &mut body;
        (
            url,
            vec![
                ("x-api-key", settings.ai_api_key.clone()),
                ("anthropic-version", "2023-06-01".to_string()),
            ],
            body,
        )
    } else {
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };
        let mut messages = vec![json!({ "role": "system", "content": system })];
        messages.extend_from_slice(history);
        let body = json!({
            "model": settings.ai_model,
            "messages": messages,
            "stream": true,
        });
        (
            url,
            vec![("Authorization", format!("Bearer {}", settings.ai_api_key))],
            body,
        )
    };

    let mut req = client.post(&url).json(&body);
    for (k, v) in &headers {
        req = req.header(*k, v);
    }
    let resp = req.send().map_err(|e| format!("请求失败：{}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .or_else(|| v.get("message").and_then(|m| m.as_str()))
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| text.chars().take(300).collect());
        return Err(format!("AI 服务返回 {}：{}", status.as_u16(), msg));
    }

    // SSE 逐行读取，逐段推送增量
    let reader = BufReader::new(resp);
    let mut full = String::new();
    let mut buf = String::new();
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消".into());
        }
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let data = match line.strip_prefix("data:") {
            Some(d) => d.trim().to_string(),
            None => continue, // 注释行 / event: 行 / 空行
        };
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let v: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue, // 心跳或非 JSON 行
        };
        let delta = if anthropic {
            // content_block_delta: {"delta":{"type":"text_delta","text":"..."}}
            if v.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
                v.get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            }
        } else {
            v.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        };
        if delta.is_empty() {
            continue;
        }
        full.push_str(&delta);
        buf.push_str(&delta);
        if buf.chars().count() >= 24 {
            let _ = app.emit("agent-delta", AgentEvent { id: stream_id, text: std::mem::take(&mut buf) });
        }
    }
    if !buf.is_empty() {
        let _ = app.emit("agent-delta", AgentEvent { id: stream_id, text: buf });
    }
    if full.is_empty() {
        return Err("AI 返回内容为空（流中断或被安全策略拦截）".into());
    }
    Ok(full)
}

// ---------------- 命令执行 ----------------

/// 执行命令（cwd=项目根，注入工具链环境）：
/// 立即返回 cmd_id；输出经 `console-output`、结束经 `console-exit` 推送。
#[tauri::command]
pub fn agent_run_command(app: AppHandle, state: State<'_, AppState>, command: String) -> Result<u32, String> {
    if let Some(why) = is_dangerous(&command) {
        return Err(format!("已拦截危险命令（{}）：{}", why, command));
    }
    let (root, env) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        let vc = guard.as_ref().ok_or("尚未打开工作区")?;
        let settings = config::load_settings(&app);
        (vc.root.clone(), toolchain::build_env(&settings))
    };
    let id = CMD_SEQ.fetch_add(1, Ordering::Relaxed) as u32;
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", &command]).current_dir(&root);
        for (k, v) in &env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit(
                    "console-output",
                    ConsoleLine { id, line: format!("启动失败：{}", e), stderr: true },
                );
                let _ = app.emit("console-exit", ConsoleExit { id, code: None });
                return;
            }
        };
        let pid = child.id();
        insert_pid(id, pid);
        // 两个读线程：stdout / stderr
        if let Some(out) = child.stdout.take() {
            let app2 = app.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(|l| l.ok()) {
                    let _ = app2.emit("console-output", ConsoleLine { id, line, stderr: false });
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let app2 = app.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(|l| l.ok()) {
                    let _ = app2.emit("console-output", ConsoleLine { id, line, stderr: true });
                }
            });
        }
        let status = child.wait();
        remove_pid(id);
        let _ = app.emit(
            "console-exit",
            ConsoleExit { id, code: status.ok().and_then(|s| s.code()) },
        );
    });
    Ok(id)
}

/// 中止命令（Windows：taskkill 进程树，避免 mvn→java 残留）
#[tauri::command]
pub fn agent_kill_command(id: u32) -> Result<(), String> {
    if let Some(pid) = pid_of(id) {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    Ok(())
}

// ---------------- 会话持久化 ----------------

fn sessions_dir(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".easyide").join("sessions")
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub updated: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SessionDoc {
    pub id: String,
    pub title: String,
    pub updated: String,
    pub messages: Vec<AgentMessage>,
}

#[tauri::command]
pub fn agent_sessions_list(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let dir = sessions_dir(&vc.root);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "json").unwrap_or(false) {
                if let Some(doc) = std::fs::read_to_string(&p)
                    .ok()
                    .and_then(|t| serde_json::from_str::<SessionDoc>(&t).ok())
                {
                    out.push(SessionMeta { id: doc.id, title: doc.title, updated: doc.updated });
                }
            }
        }
    }
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out.truncate(50);
    Ok(out)
}

#[tauri::command]
pub fn agent_session_load(state: State<'_, AppState>, id: String) -> Result<SessionDoc, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let path = sessions_dir(&vc.root).join(format!("{}.json", sanitize_id(&id)));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取会话失败：{}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("解析会话失败：{}", e))
}

#[tauri::command]
pub fn agent_session_save(
    state: State<'_, AppState>,
    session: SessionDoc,
) -> Result<(), String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let dir = sessions_dir(&vc.root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建会话目录失败：{}", e))?;
    let path = dir.join(format!("{}.json", sanitize_id(&session.id)));
    let json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存会话失败：{}", e))
}

#[tauri::command]
pub fn agent_session_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let path = sessions_dir(&vc.root).join(format!("{}.json", sanitize_id(&id)));
    let _ = std::fs::remove_file(&path);
    Ok(())
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

// ---------------- 工具链探测 ----------------

#[tauri::command]
pub fn toolchain_detect(kind: String, prefer: Option<String>) -> Result<toolchain::ToolInfo, String> {
    Ok(toolchain::detect(&kind, prefer.as_deref().unwrap_or("")))
}
