//! 交互式终端（Windows ConPTY，经 portable-pty）：
//! 一个工作区一个会话；输出（含 ANSI）base64 编码经 `terminal-out` 推送，EOF/退出经 `terminal-exit`。

use crate::config;
use crate::state::AppState;
use crate::toolchain;
use base64::Engine;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

static TERM_SEQ: AtomicU32 = AtomicU32::new(1);
static TERMINALS: Mutex<Option<HashMap<u32, TermHandle>>> = Mutex::new(None);

struct TermHandle {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

fn with_terminals<T>(f: impl FnOnce(&mut HashMap<u32, TermHandle>) -> T) -> T {
    let mut guard = TERMINALS.lock().unwrap();
    f(guard.get_or_insert_with(HashMap::new))
}

#[derive(serde::Serialize, Clone)]
pub struct TermOut {
    pub id: u32,
    /// base64 编码的原始输出字节（保 ANSI 与 UTF-8 完整性）
    pub data: String,
}

#[derive(serde::Serialize, Clone)]
pub struct TermExit {
    pub id: u32,
    pub code: Option<i32>,
}

/// 创建终端会话（cwd=工作区根，注入工具链环境），返回会话 id
#[tauri::command]
pub fn terminal_create(
    app: AppHandle,
    state: State<'_, AppState>,
    cols: u16,
    rows: u16,
) -> Result<u32, String> {
    // 已有会话：直接复用（单会话模型）
    if let Some(id) = with_terminals(|m| m.keys().next().copied()) {
        return Ok(id);
    }
    let (root, env) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        let vc = guard.as_ref().ok_or("尚未打开工作区")?;
        let settings = config::load_settings(&app);
        (vc.root.clone(), toolchain::build_env(&settings))
    };
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("创建 PTY 失败：{}", e))?;
    let mut cmd = CommandBuilder::new("cmd.exe");
    cmd.cwd(&root);
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| format!("启动 shell 失败：{}", e))?;
    // slave 必须释放，否则子进程退出后 reader 收不到 EOF
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| format!("克隆读端失败：{}", e))?;
    let writer = pair.master.take_writer().map_err(|e| format!("获取写端失败：{}", e))?;

    let id = TERM_SEQ.fetch_add(1, Ordering::Relaxed);
    with_terminals(|m| {
        m.insert(id, TermHandle { writer, master: pair.master, child });
    });

    // 输出泵线程
    let app2 = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                    let _ = app2.emit("terminal-out", TermOut { id, data });
                }
                Err(_) => break,
            }
        }
        // EOF：取退出码并广播
        let code = with_terminals(|m| {
            m.get_mut(&id)
                .and_then(|h| h.child.wait().ok().map(|s| s.exit_code() as i32))
        });
        with_terminals(|m| {
            m.remove(&id);
        });
        let _ = app2.emit("terminal-exit", TermExit { id, code });
    });
    Ok(id)
}

/// 写入终端（键盘输入）
#[tauri::command]
pub fn terminal_write(id: u32, data: String) -> Result<(), String> {
    let written = with_terminals(|m| match m.get_mut(&id) {
        Some(h) => h
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("写入终端失败：{}", e)),
        None => Err("终端会话已结束".to_string()),
    });
    written?;
    with_terminals(|m| {
        if let Some(h) = m.get_mut(&id) {
            let _ = h.writer.flush();
        }
    });
    Ok(())
}

/// 终端尺寸变化
#[tauri::command]
pub fn terminal_resize(id: u32, cols: u16, rows: u16) -> Result<(), String> {
    with_terminals(|m| {
        m.get_mut(&id)
            .ok_or_else(|| "终端会话已结束".to_string())?
            .master
            .resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("调整尺寸失败：{}", e))
    })
}

/// 终止会话
#[tauri::command]
pub fn terminal_kill(id: u32) -> Result<(), String> {
    with_terminals(|m| {
        if let Some(h) = m.get_mut(&id) {
            let _ = h.child.kill();
        }
        m.remove(&id);
    });
    Ok(())
}

/// 当前是否有活动会话（工作区切换后前端重建）
#[tauri::command]
pub fn terminal_alive() -> bool {
    with_terminals(|m| !m.is_empty())
}
