//! 索引查询命令：重建、标签统计、反链、链接解析。

use crate::index::model::{Backlink, NoteIndex, ResolveResult, TagCount};
use crate::state::AppState;
use tauri::State;

/// 手动重建索引（丢弃缓存全量扫描；全量扫描 + 缓存落盘放后台线程，避免大库重建冻结窗口）
#[tauri::command]
pub async fn rebuild_index(state: State<'_, AppState>) -> Result<Vec<NoteIndex>, String> {
    let (root, cache_path) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        let vc = guard.as_ref().ok_or("尚未打开知识库")?;
        (vc.root.clone(), vc.cache_path.clone())
    };
    let scan_root = root.clone();
    let (engine, notes) = tauri::async_runtime::spawn_blocking(move || {
        let engine = crate::index::engine::IndexEngine::scan(&scan_root)
            .map_err(|e| format!("扫描失败：{}", e))?;
        let notes = engine.all_notes();
        engine.save_cache(&cache_path);
        Ok::<_, String>((engine, notes))
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))??;
    // 装配回内存（扫描期间已切换工作区则丢弃本次结果）
    let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_mut().ok_or("尚未打开知识库")?;
    if vc.root != root {
        return Err("工作区已切换，重建结果已丢弃，请重试".into());
    }
    vc.engine = engine;
    Ok(notes)
}

#[tauri::command]
pub fn get_all_notes(state: State<'_, AppState>) -> Result<Vec<NoteIndex>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => Ok(vc.engine.all_notes()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub fn get_tags(state: State<'_, AppState>) -> Result<Vec<TagCount>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => Ok(vc.engine.tag_counts()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub fn get_backlinks(state: State<'_, AppState>, path: String) -> Result<Vec<Backlink>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => Ok(vc.engine.backlinks(&path)),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub fn resolve_link(state: State<'_, AppState>, target: String) -> Result<ResolveResult, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => Ok(vc.engine.resolve(&target)),
        None => Ok(ResolveResult { path: None, ambiguous: false }),
    }
}
