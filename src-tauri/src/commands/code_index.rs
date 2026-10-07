//! 代码索引命令：符号检索 / 文件检索 / 全文搜索 / 大纲。

use crate::code_index::engine::{parallel_text_search, CodeIndexEngine};
use crate::code_index::model::{CodeHit, Symbol, SymbolHit, TextHit};
use crate::state::AppState;
use tauri::State;

fn with_engine<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&CodeIndexEngine) -> Result<T, String>,
) -> Result<T, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => f(&vc.code_engine),
        None => Err("尚未打开工作区".into()),
    }
}

/// 符号检索：kinds 可选过滤（class/interface/enum/record/method/function/field）
#[tauri::command]
pub fn code_symbol_search(
    state: State<'_, AppState>,
    query: String,
    kinds: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<SymbolHit>, String> {
    with_engine(&state, |engine| {
        Ok(engine.search_symbols(&query, kinds.as_deref(), limit.unwrap_or(50)))
    })
}

/// 单文件符号（大纲面板）
#[tauri::command]
pub fn code_file_symbols(state: State<'_, AppState>, path: String) -> Result<Vec<Symbol>, String> {
    with_engine(&state, |engine| Ok(engine.file_symbols(&path)))
}

/// 工作区文件名检索（Ctrl+P）
#[tauri::command]
pub fn code_file_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<CodeHit>, String> {
    with_engine(&state, |engine| {
        Ok(engine.search_files(&query, limit.unwrap_or(50)))
    })
}

/// 全文搜索（Ctrl+Shift+F）：并行扫描全部文本文件
#[tauri::command]
pub fn code_text_search(
    state: State<'_, AppState>,
    query: String,
    case_sensitive: Option<bool>,
    max_results: Option<usize>,
) -> Result<Vec<TextHit>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(vec![]);
    }
    // 候选集在锁内取，搜索在锁外跑（不阻塞其它命令）
    let (root, candidates) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(vc) => (vc.root.clone(), vc.code_engine.search_candidates()),
            None => return Err("尚未打开工作区".into()),
        }
    };
    Ok(parallel_text_search(
        &root,
        &candidates,
        &query,
        case_sensitive.unwrap_or(false),
        max_results.unwrap_or(500),
    ))
}
