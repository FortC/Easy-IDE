//! Vault 管理命令：打开/创建/关闭/移除知识库。

use crate::code_index::engine::CodeIndexEngine;
use crate::config;
use crate::index::engine::IndexEngine;
use crate::index::model::NoteIndex;
use crate::index::watcher;
use crate::state::{AppState, VaultContext};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn list_vaults(app: AppHandle) -> Vec<config::VaultEntry> {
    config::load_vaults(&app)
}

/// 打开已有 vault：增量加载索引、启动文件监听
#[tauri::command]
pub fn open_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<OpenVaultResult, String> {
    let root = PathBuf::from(&path);
    let meta = std::fs::metadata(&root).map_err(|e| format!("无法访问文件夹：{}", e))?;
    if !meta.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    open_vault_inner(app, state, root, path)
}

/// 创建新 vault（空文件夹）并打开
#[tauri::command]
pub fn create_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<OpenVaultResult, String> {
    let root = PathBuf::from(&path);
    if root.exists() {
        return Err("目录已存在".into());
    }
    std::fs::create_dir_all(&root).map_err(|e| format!("创建失败：{}", e))?;
    open_vault_inner(app, state, root, path)
}

pub(crate) fn open_vault_inner(
    app: AppHandle,
    state: State<'_, AppState>,
    root: PathBuf,
    path: String,
) -> Result<OpenVaultResult, String> {
    let cache_path = IndexEngine::cache_path(&config::config_dir(&app), &root);

    // 关闭旧 vault 的监听
    {
        let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(old) = guard.as_mut() {
            if let Some(w) = old.watcher.take() {
                w.stop();
            }
        }
        *guard = None;
    }

    // 索引：只同步加载缓存（快），磁盘增量校验放后台线程，避免大项目首开阻塞界面
    let engine = IndexEngine::load_cache(&cache_path).unwrap_or_default();
    let code_cache_path = CodeIndexEngine::cache_path(&config::config_dir(&app), &root);
    let code_engine = CodeIndexEngine::load_cache(&code_cache_path).unwrap_or_default();

    // 文档中心项目级状态（.easyide/md-assistant.json；不存在则用默认值）
    let docs = crate::docs_hub::hub::load_state(&root);

    let watch = watcher::spawn(root.clone(), app.clone())
        .map_err(|e| format!("文件监听启动失败：{}", e))?;

    let notes = engine.all_notes();
    {
        let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
        *guard = Some(VaultContext {
            root: root.clone(),
            engine,
            cache_path,
            code_engine,
            code_cache_path,
            docs,
            watcher: Some(watch),
        });
    }

    // 记录 vault 列表与最近打开
    config::touch_vault(&app, &path);
    let mut settings = config::load_settings(&app);
    settings.last_vault = Some(path.clone());
    config::save_settings(&app, &settings);

    // 窗口标题带项目名
    if let Some(w) = app.get_webview_window("main") {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let _ = w.set_title(&format!("{} — EasyIDE", name));
    }

    // 后台：磁盘增量校验两个索引并落缓存（完成事件 workspace-indexed）
    {
        let bg_app = app.clone();
        let bg_root = root.clone();
        std::thread::spawn(move || {
            let state = bg_app.state::<AppState>();
            let mut guard = match state.vault.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(vc) = guard.as_mut() else { return };
            if vc.root != bg_root {
                return; // 已切换工作区，丢弃
            }
            vc.engine.refresh_against_disk(&bg_root);
            vc.engine.save_cache(&vc.cache_path);
            vc.code_engine.refresh_against_disk(&bg_root);
            vc.code_engine.save_cache(&vc.code_cache_path);
            drop(guard);
            let _ = bg_app.emit("workspace-indexed", ());
        });
    }

    Ok(OpenVaultResult { root: path, notes })
}

#[tauri::command]
pub fn close_vault(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(vc) = guard.as_mut() {
            if let Some(w) = vc.watcher.take() {
                w.stop();
            }
        }
        *guard = None;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title("EasyIDE");
    }
    Ok(())
}

#[tauri::command]
pub fn forget_vault(app: AppHandle, path: String) -> Result<(), String> {
    let mut vaults = config::load_vaults(&app);
    vaults.retain(|v| v.path != path);
    config::save_vaults(&app, &vaults);
    let mut settings = config::load_settings(&app);
    if settings.last_vault.as_deref() == Some(path.as_str()) {
        settings.last_vault = vaults.first().map(|v| v.path.clone());
        config::save_settings(&app, &settings);
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct OpenVaultResult {
    pub root: String,
    pub notes: Vec<NoteIndex>,
}
