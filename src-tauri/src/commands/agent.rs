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
    // 锁中毒不 panic：接管内部值继续用（下同）
    CANCELS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(id, flag);
}

fn remove_cancel(id: u64) {
    let mut guard = CANCELS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(map) = guard.as_mut() {
        map.remove(&id);
    }
}

fn cancel_stream(id: u64) {
    let guard = CANCELS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(map) = guard.as_ref() {
        if let Some(flag) = map.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

fn insert_pid(cmd_id: u32, pid: u32) {
    LIVE_PIDS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(cmd_id, pid);
}

fn remove_pid(cmd_id: u32) {
    let mut guard = LIVE_PIDS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(map) = guard.as_mut() {
        map.remove(&cmd_id);
    }
}

fn pid_of(cmd_id: u32) -> Option<u32> {
    let guard = LIVE_PIDS.lock().unwrap_or_else(|p| p.into_inner());
    guard.as_ref()?.get(&cmd_id).copied()
}

/// 终止单个进程树（Windows：taskkill /PID x /T /F，避免 mvn→java 等孙进程残留）。
/// 进程已退出时 taskkill 会失败：静默 + 日志，绝不向调用方报错打断 UI。
/// 走 `toolchain::hidden` 隐藏窗口 —— 否则每点一次「停止」就弹一个黑窗。
fn kill_process_tree(pid: u32) {
    match crate::toolchain::hidden_new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
    {
        Ok(out) if out.status.success() => {}
        Ok(out) => eprintln!(
            "[清理] taskkill PID {} 未成功（进程可能已退出）：{}",
            pid,
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(e) => eprintln!("[清理] taskkill PID {} 执行失败：{}", pid, e),
    }
}

/// 退出清理入口：取出并清空 PID 登记表，逐个杀掉进程树。
/// 幂等：重复调用安全（第二次为空操作）；taskkill 在锁外执行，不阻塞登记/移除。
pub fn kill_all_tracked() {
    kill_all_tracked_with(kill_process_tree);
}

/// 可注入杀进程动作的实现，便于单元测试（真实入口见 [`kill_all_tracked`]）
fn kill_all_tracked_with(mut kill: impl FnMut(u32)) {
    let pids: Vec<u32> = {
        let mut guard = LIVE_PIDS.lock().unwrap_or_else(|p| p.into_inner());
        match guard.as_mut() {
            Some(map) => map.drain().map(|(_, pid)| pid).collect(),
            None => Vec::new(),
        }
    };
    for pid in pids {
        kill(pid);
    }
}

/// 危险命令硬拦截（确认门之外的第二道防线；前端已确认的命令与配方快速通道都过这里）。
/// 按「命令行首词 + 参数组合」匹配，不做事宽泛子串匹配（`mvn -Dformat=...`、`gradle formatCheck`
/// 等正常命令不再被 `format ` 之类子串误伤）；`&`/`&&`/`||`/`|`/`;` 连接的多段命令逐段检查，
/// `cmd /C`、`bash -c`、`powershell -Command` 的内层脚本递归解析。
pub(crate) fn is_dangerous(command: &str) -> Option<&'static str> {
    for seg in split_segments(command) {
        if let Some(why) = segment_dangerous(seg) {
            return Some(why);
        }
    }
    None
}

/// 按 shell 连接符切分命令段（只在 ASCII 分隔符处切，UTF-8 多字节不受影响）
fn split_segments(command: &str) -> Vec<&str> {
    let mut segs = Vec::new();
    let bytes = command.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'&' || c == b'|' || c == b';' {
            segs.push(&command[start..i]);
            // && 与 || 是单个连接符
            if (c == b'&' || c == b'|') && i + 1 < bytes.len() && bytes[i + 1] == c {
                i += 2;
            } else {
                i += 1;
            }
            start = i;
            continue;
        }
        i += 1;
    }
    segs.push(&command[start..]);
    segs
}

/// 单段命令判定：首词（去目录/扩展名）→ 危险程序 + 参数组合规则
fn segment_dangerous(segment: &str) -> Option<&'static str> {
    let seg = segment.trim().trim_start_matches('@').trim();
    if seg.is_empty() {
        return None;
    }
    let (first, rest) = split_first_token(seg);
    let prog = program_name(first);
    let args: Vec<&str> = rest.split_whitespace().collect();
    match prog.as_str() {
        // rm：递归 + 强制组合（-rf / -fr / -r -f / --recursive --force）
        "rm" => rm_recursive_force(&args).then_some("递归强制删除"),
        // rd / rmdir：带 /s 递归删除
        "rd" | "rmdir" => args
            .iter()
            .any(|a| a.eq_ignore_ascii_case("/s") || *a == "-r" || *a == "--recursive")
            .then_some("递归删除目录"),
        // del / erase：/f /s /q 三件套
        "del" | "erase" => {
            let has = |flag: &str| args.iter().any(|a| a.eq_ignore_ascii_case(flag));
            (has("/f") && has("/s") && has("/q")).then_some("强制删除")
        }
        "format" => Some("格式化磁盘"),
        s if s.starts_with("mkfs") => Some("格式化文件系统"),
        // reg：delete / add 子命令（query/save 等只读子命令放行）
        "reg" => match args.first().map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("delete") => Some("删除注册表项"),
            Some("add") => Some("写注册表"),
            _ => None,
        },
        "shutdown" => Some("关机/重启"),
        "taskkill" => args.iter().any(|a| a.eq_ignore_ascii_case("/f")).then_some("强制结束进程"),
        "diskpart" => Some("磁盘分区"),
        // cipher：/w（含 /w:<目录>）擦除磁盘数据
        "cipher" => args
            .iter()
            .any(|a| {
                let l = a.to_ascii_lowercase();
                l == "/w" || l.starts_with("/w:")
            })
            .then_some("擦除磁盘数据"),
        // powershell / pwsh：编码执行脚本；-Command 内层递归
        "powershell" | "pwsh" => {
            if args.iter().any(|a| {
                let l = a.to_ascii_lowercase();
                l == "-e" || l == "-enc" || l == "-ec" || l.starts_with("-encodedcommand")
            }) {
                Some("编码执行脚本")
            } else {
                recurse_after_flag(rest, &["-command", "-c"])
            }
        }
        // cmd /C、bash -c 等：取内层脚本递归解析
        "cmd" => recurse_after_flag(rest, &["/c", "/k"]),
        "bash" | "sh" | "zsh" => recurse_after_flag(rest, &["-c"]),
        _ => None,
    }
}

/// 取执行旗标之后的内层脚本递归判定（如 `cmd /C del /f /s /q x` 的内层）
fn recurse_after_flag(rest: &str, flags: &[&str]) -> Option<&'static str> {
    let lower = rest.to_ascii_lowercase();
    for flag in flags {
        if let Some(idx) = find_flag(&lower, flag) {
            let inner = rest[idx + flag.len()..].trim().trim_matches('"').trim();
            if !inner.is_empty() {
                return is_dangerous(inner);
            }
        }
    }
    None
}

/// 在小写串中找独立词边界出现的旗标位置
fn find_flag(hay_lower: &str, flag: &str) -> Option<usize> {
    let bytes = hay_lower.as_bytes();
    let mut from = 0;
    while let Some(rel) = hay_lower[from..].find(flag) {
        let idx = from + rel;
        let after = idx + flag.len();
        let before_ok = idx == 0 || bytes[idx - 1].is_ascii_whitespace();
        let after_ok = after >= bytes.len() || bytes[after].is_ascii_whitespace();
        if before_ok && after_ok {
            return Some(idx);
        }
        from = after;
    }
    None
}

/// rm 参数组合：递归 + 强制才算"递归强制删除"（单 -r 或单 -f 不拦）
fn rm_recursive_force(args: &[&str]) -> bool {
    let mut recursive = false;
    let mut force = false;
    for a in args {
        let arg = a.trim();
        if arg == "--" {
            break; // 之后是路径参数
        }
        if arg == "--recursive" {
            recursive = true;
        } else if arg == "--force" {
            force = true;
        } else if arg.len() > 1 && arg.starts_with('-') && !arg.starts_with("--") {
            // 短选项簇：-rf / -fr / -Rfv 等
            for c in arg[1..].chars() {
                match c {
                    'r' | 'R' => recursive = true,
                    'f' | 'F' => force = true,
                    _ => {}
                }
            }
        }
        if recursive && force {
            return true;
        }
    }
    recursive && force
}

/// 命令行首 token（支持引号包住的带空格路径），返回 (首词, 其余)
fn split_first_token(seg: &str) -> (&str, &str) {
    let s = seg.trim_start();
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"').unwrap_or(rest.len());
        let first = &rest[..end];
        let remaining = rest.get(end + 1..).unwrap_or("").trim_start();
        (first, remaining)
    } else {
        match s.find(char::is_whitespace) {
            Some(idx) => (&s[..idx], s[idx..].trim_start()),
            None => (s, ""),
        }
    }
}

/// 程序名归一：去目录、去可执行扩展名、小写
fn program_name(token: &str) -> String {
    let base = token.rsplit(['\\', '/']).next().unwrap_or(token);
    let lower = base.to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com", ".ps1"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    lower
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

    // B7-B 档：本地模型端点（Ollama / llama.cpp 等 OpenAI 兼容服务）。
    // 做法是**在进入分派前把 base/model/key 整体换成本地值**，而不是新写一条分支 ——
    // OpenAI 兼容端点与云端走的正是同一套请求体与 SSE 解析，复制一份只会带来
    // 「改了一处忘了另一处」的长期维护负担。
    let (base, model, api_key, anthropic) = if settings.local_llm_enabled {
        let lb = if settings.local_llm_base.trim().is_empty() {
            "http://localhost:11434/v1".to_string()
        } else {
            settings.local_llm_base.trim().trim_end_matches('/').to_string()
        };
        // 模型留空时给个 Ollama 常见值；用户没下模型会返回明确错误，比静默失败好
        let lm = if settings.local_llm_model.trim().is_empty() {
            "qwen2.5-coder:7b".to_string()
        } else {
            settings.local_llm_model.trim().to_string()
        };
        // 本地服务一般不校验 key，但 Ollama 兼容层仍期望 Authorization 头存在
        (lb, lm, "local".to_string(), false)
    } else {
        (
            settings.ai_base_url.trim().trim_end_matches('/').to_string(),
            settings.ai_model.clone(),
            settings.ai_api_key.clone(),
            settings.ai_provider == "anthropic",
        )
    };

    let (url, headers, body) = if anthropic {
        let url = if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        };
        let mut body = json!({
            "model": model,
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
                ("x-api-key", api_key),
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
            "model": model,
            "messages": messages,
            "stream": true,
        });
        (
            url,
            vec![("Authorization", format!("Bearer {}", api_key))],
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
pub async fn agent_run_command(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
) -> Result<SpawnResult, String> {
    if let Some(why) = is_dangerous(&command) {
        return Err(format!("已拦截危险命令（{}）：{}", why, command));
    }
    let root = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .map(|vc| vc.root.clone())
            .ok_or_else(|| "尚未打开工作区".to_string())?
    };
    // 工具链环境构建可能 spawn java -version / mvn -v 等子进程，放后台线程避免阻塞 UI
    let bg_app = app.clone();
    let (root, env) = tauri::async_runtime::spawn_blocking(move || {
        let settings = config::load_settings(&bg_app);
        (root, toolchain::build_env(&settings))
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?;
    let (id, pid) = spawn_streaming_detached(app, command, root, env);
    Ok(SpawnResult { id, pid })
}

/// 流式执行命令（AI 执行与配方快速通道共用）：返回 cmd_id，
/// 进程启动结果：`cmd_id` 立即返回，进程在后台跑（B1）。
///
/// 关键点：以前 `agent_run_command` 只回传 `cmd_id`，前端只能靠 `console-exit`
/// 事件反推状态，于是 AI 主循环不得不 `await` 到进程退出 —— 启动 Spring Boot
/// 就永远等不到，`streaming` 卡在 true，用户只能点「停止」，而「停止」又会杀进程。
/// 现在把 `pid` 一并回传，前端可立刻显示「已启动 PID xxxx」。
#[derive(serde::Serialize, Clone, Debug)]
pub struct SpawnResult {
    pub id: u32,
    pub pid: u32,
}

/// 输出经 `console-output`、结束经 `console-exit` 推送，进程树可通过 agent_kill_command 终止。
/// **立即返回** cmd_id（不等待进程结束）——这是 B1 的核心：长驻进程不得阻塞调用方。
pub(crate) fn spawn_streaming(
    app: AppHandle,
    command: String,
    cwd: std::path::PathBuf,
    env: Vec<(String, String)>,
) -> u32 {
    let (id, _pid) = spawn_streaming_detached(app, command, cwd, env);
    id
}

/// 与 [`spawn_streaming`] 同上，但额外回传 pid（长驻进程需要在 UI 上显示）。
pub(crate) fn spawn_streaming_detached(
    app: AppHandle,
    command: String,
    cwd: std::path::PathBuf,
    env: Vec<(String, String)>,
) -> (u32, u32) {
    let mut cmd = crate::toolchain::hidden_new("cmd");
    cmd.args(["/C", &command]);
    spawn_detached_with(app, cmd, cwd, env)
}

/// exe 与参数以独立 argv 传递的 spawn（文件树右键构建命令走这条）。
///
/// ⚠️ Windows 引号陷阱（实测结论）：`std::process::Command` 会把参数里的 `"` 转义成 `\"`，
/// cmd 的 /C 解析器不认这种转义；且 /C 命令首字符是引号时会剥掉首尾引号，多段带引号
/// 路径必碎。规避办法：先塞一个无引号首词 `call`（cmd 不再剥引号），exe 与各参数作为
/// 独立 argv（Rust 只在含空格的参数外加边界引号，cmd 原样保留），带空格路径由此保真。
/// `call` 对 .exe / 裸名 / 批处理均兼容（等价控制台手敲）。
pub(crate) fn spawn_streaming_detached_argv(
    app: AppHandle,
    exe: String,
    args: Vec<String>,
    cwd: std::path::PathBuf,
    env: Vec<(String, String)>,
) -> (u32, u32) {
    let mut cmd = crate::toolchain::hidden_new("cmd");
    cmd.arg("/C").arg("call").arg(&exe);
    cmd.args(&args);
    spawn_detached_with(app, cmd, cwd, env)
}

/// 流式转发 + 退出事件的公共主体：cmd 已配好程序与参数，这里负责
/// CREATE_NO_WINDOW 下的 spawn、pid 回传、stdout/stderr 逐行推送与进程登记。
fn spawn_detached_with(
    app: AppHandle,
    mut cmd: std::process::Command,
    cwd: std::path::PathBuf,
    env: Vec<(String, String)>,
) -> (u32, u32) {
    let id = CMD_SEQ.fetch_add(1, Ordering::Relaxed) as u32;
    // pid 要在 spawn 之后才有值；用 oneshot 把真实 pid 回传给调用方（立即返回，不等退出）
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    std::thread::spawn(move || {
        cmd.current_dir(&cwd);
        for (k, v) in &env {
            cmd.env(k, v);
        }
        let spawned = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit(
                    "console-output",
                    ConsoleLine { id, line: format!("启动失败：{}", e), stderr: true },
                );
                let _ = app.emit("console-exit", ConsoleExit { id, code: None });
                let _ = tx.send(0);
                return;
            }
        };
        let pid = child.id();
        insert_pid(id, pid);
        // spawn 已完成，pid 立刻可用（不等进程退出）
        let _ = tx.send(pid);
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
    // rx.recv 只等到「spawn 完成」这一刻（毫秒级），拿不到 pid 时回落 0
    let pid = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or(0);
    (id, pid)
}

/// 列出当前仍在运行的命令（B1：UI 恢复用 —— 重开面板后也知道项目还活着）。
#[tauri::command]
pub fn agent_list_running() -> Vec<RunningProc> {
    let guard = LIVE_PIDS.lock().unwrap_or_else(|p| p.into_inner());
    let map = match guard.as_ref() {
        Some(m) => m,
        None => return Vec::new(),
    };
    let mut out: Vec<RunningProc> = map
        .iter()
        .map(|(id, pid)| RunningProc {
            id: *id,
            pid: *pid,
            alive: process_alive(*pid),
        })
        .collect();
    // 按 cmd_id 排序，保证多次调用顺序稳定
    out.sort_by_key(|p| p.id);
    out
}

/// 运行中进程信息
#[derive(serde::Serialize, Clone, Debug)]
pub struct RunningProc {
    pub id: u32,
    pub pid: u32,
    /// PID 是否仍存活（登记可能已滞后于进程退出，故实探一次）
    pub alive: bool,
}

/// 探测进程是否存活：Windows 用 OpenProcess，非 Windows 退化为 kill(pid, 0) 语义不可得 → 保守认为存活。
fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        // PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
        const QUERY_LIMITED: u32 = 0x1000;
        extern "system" {
            fn OpenProcess(dw: u32, b: i32, pid: u32) -> isize;
            fn CloseHandle(h: isize) -> i32;
        }
        unsafe {
            let h = OpenProcess(QUERY_LIMITED, 0, pid);
            if h == 0 {
                return false;
            }
            CloseHandle(h);
            true
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        true
    }
}

/// 中止命令（Windows：taskkill 进程树，避免 mvn→java 残留）
#[tauri::command]
pub async fn agent_kill_command(id: u32) -> Result<(), String> {
    let pid = pid_of(id);
    // taskkill 同步等待子进程结束，放后台线程避免阻塞 UI；已退出 PID 的失败在 kill_process_tree 内静默
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?;
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

/// 工具链探测（spawn java -version / mvn -v 等子进程，放后台线程避免阻塞 UI）
#[tauri::command]
pub async fn toolchain_detect(
    kind: String,
    prefer: Option<String>,
) -> Result<toolchain::ToolInfo, String> {
    let prefer = prefer.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || toolchain::detect(&kind, &prefer))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

// ---------------- 文件修改提案 ----------------

/// 生成旧内容(磁盘) → 新内容(AI 提案) 的 unified diff 预览
#[tauri::command]
pub fn diff_preview(
    state: State<'_, AppState>,
    path: String,
    new_content: String,
) -> Result<String, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let full = vc
        .safe_join(&path)
        .map_err(|e| format!("非法路径：{}", e))?;
    let old = std::fs::read_to_string(&full).unwrap_or_default();
    Ok(crate::difflib::unified_diff(&old, &new_content, &path))
}

#[cfg(test)]
mod tests {
    use super::{insert_pid, is_dangerous, kill_all_tracked_with, pid_of};

    #[test]
    fn dangerous_commands_blocked() {
        // 12 条原规则的防护语义全部保留
        assert!(is_dangerous("rm -rf /").is_some());
        assert!(is_dangerous("rm -rf build").is_some());
        assert!(is_dangerous("rm -r -f node_modules").is_some());
        assert!(is_dangerous("rm -fr .").is_some());
        assert!(is_dangerous("rd /s /q folder").is_some());
        assert!(is_dangerous("del /f /s /q C:\\temp").is_some());
        assert!(is_dangerous("format C:").is_some());
        assert!(is_dangerous("mkfs.ext4 /dev/sda1").is_some());
        assert!(is_dangerous("reg delete HKLM\\Software\\Foo /f").is_some());
        assert!(is_dangerous("reg add HKLM\\Software\\Foo").is_some());
        assert!(is_dangerous("shutdown /s /t 0").is_some());
        assert!(is_dangerous("taskkill /f /im java.exe").is_some());
        assert!(is_dangerous("diskpart /s script.txt").is_some());
        assert!(is_dangerous("cipher /w:C:\\secure").is_some());
        assert!(is_dangerous("powershell -enc SQBFAFgA").is_some());
        assert!(is_dangerous("powershell -EncodedCommand SQBFAFgA").is_some());
        // 多段命令逐段检查、内层脚本递归
        assert!(is_dangerous("echo ok && rm -rf /").is_some());
        assert!(is_dangerous("cmd /c del /f /s /q C:\\temp").is_some());
        assert!(is_dangerous("bash -c \"rm -rf /\"").is_some());
    }

    #[test]
    fn normal_commands_allowed() {
        assert!(is_dangerous("mvn -B dependency:tree -Dformat=xml").is_none());
        assert!(is_dangerous("gradle formatCheck").is_none());
        assert!(is_dangerous("mvn clean install").is_none());
        assert!(is_dangerous("npm run build").is_none());
        assert!(is_dangerous("git status").is_none());
        assert!(is_dangerous("git log --format=%h%x09%s").is_none());
        assert!(is_dangerous("git diff --no-index -- NUL a -> b.md").is_none());
        assert!(is_dangerous("echo shutdown 演示").is_none());
        assert!(is_dangerous("del file.txt").is_none());
        assert!(is_dangerous("rm node_modules.lock").is_none());
        assert!(is_dangerous("cipher").is_none());
        assert!(is_dangerous("reg query HKLM\\Software").is_none());
        assert!(is_dangerous("cmd /c mvn -v").is_none());
        assert!(is_dangerous("mvnw.cmd -B package").is_none());
    }

    /// kill_all_tracked 幂等性：清空登记表并逐个杀进程，重复调用是空操作
    /// （单用例独占 LIVE_PIDS 静态，避免并发测试互相清空）
    #[test]
    fn kill_all_tracked_is_idempotent() {
        insert_pid(900_001, 111);
        insert_pid(900_002, 222);
        let mut killed = Vec::new();
        kill_all_tracked_with(|pid| killed.push(pid));
        killed.sort_unstable();
        assert_eq!(killed, vec![111, 222]);
        // 登记表已清空：等待线程事后的 remove_pid 也不会再命中
        assert!(pid_of(900_001).is_none());
        assert!(pid_of(900_002).is_none());
        // 幂等：再次调用（如 ExitRequested 后又收到 Exit）不杀任何进程、不报错
        let mut again = Vec::new();
        kill_all_tracked_with(|pid| again.push(pid));
        assert!(again.is_empty());
    }
}
