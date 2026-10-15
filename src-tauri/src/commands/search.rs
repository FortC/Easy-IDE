//! 搜索命令：文件名/别名搜索、全文搜索、标签搜索。

use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;

#[derive(serde::Serialize)]
pub struct FileHit {
    pub path: String,
    pub title: String,
    pub aliases: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct ContentHit {
    pub path: String,
    pub title: String,
    pub line_no: usize, // 1 基
    pub text: String,
}

fn with_engine<T>(state: &State<'_, AppState>, f: impl FnOnce(&crate::index::engine::IndexEngine, &PathBuf) -> Result<T, String>) -> Result<T, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(vc) => f(&vc.engine, &vc.root),
        None => Err("尚未打开知识库".into()),
    }
}

/// 文件名/别名搜索（大小写不敏感，前缀优先）
#[tauri::command]
pub fn search_files(state: State<'_, AppState>, query: String) -> Result<Vec<FileHit>, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    with_engine(&state, |engine, _| {
        let mut hits: Vec<FileHit> = engine
            .notes
            .values()
            .filter(|n| {
                n.title.to_lowercase().contains(&q)
                    || n.path.to_lowercase().contains(&q)
                    || n.aliases.iter().any(|a| a.to_lowercase().contains(&q))
            })
            .map(|n| FileHit {
                path: n.path.clone(),
                title: n.title.clone(),
                aliases: n.aliases.clone(),
            })
            .collect();
        // 前缀匹配排前
        hits.sort_by_key(|h| {
            let title_lower = h.title.to_lowercase();
            if title_lower.starts_with(&q) {
                0
            } else if h.aliases.iter().any(|a| a.to_lowercase().starts_with(&q)) {
                1
            } else {
                2
            }
        });
        hits.truncate(50);
        Ok(hits)
    })
}

/// 全文搜索：逐文件扫描包含关键字的行（跳过超大文件，限制结果数）。
/// 全量读盘扫描放后台线程，避免大库搜索冻结窗口。
#[tauri::command]
pub async fn search_content(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<ContentHit>, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    // 锁内取候选快照（路径+标题），锁外逐文件扫描（锁不跨 await 持有）
    let (root, notes) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(vc) => (
                vc.root.clone(),
                vc.engine
                    .notes
                    .values()
                    .map(|n| (n.path.clone(), n.title.clone()))
                    .collect::<Vec<_>>(),
            ),
            None => return Err("尚未打开知识库".into()),
        }
    };
    tauri::async_runtime::spawn_blocking(move || scan_content(&root, &notes, &q))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

/// 逐文件扫描命中行（后台线程执行）：跳过超大文件，命中数封顶 300
fn scan_content(root: &std::path::Path, notes: &[(String, String)], q: &str) -> Vec<ContentHit> {
    let mut hits: Vec<ContentHit> = Vec::new();
    'outer: for (path, title) in notes {
        let full = root.join(path);
        let Ok(meta) = std::fs::metadata(&full) else { continue };
        if meta.len() > 2 * 1024 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else { continue };
        for (i, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(q) {
                let trimmed = line.trim();
                let text_out: String = if trimmed.chars().count() > 200 {
                    trimmed.chars().take(200).collect()
                } else {
                    trimmed.to_string()
                };
                hits.push(ContentHit {
                    path: path.clone(),
                    title: title.clone(),
                    line_no: i + 1,
                    text: text_out,
                });
                if hits.len() >= 300 {
                    break 'outer;
                }
            }
        }
    }
    hits
}

/// 标签搜索：精确标签或其子级标签下的全部笔记
#[tauri::command]
pub fn search_by_tag(state: State<'_, AppState>, tag: String) -> Result<Vec<FileHit>, String> {
    let t = tag.trim().to_lowercase();
    with_engine(&state, |engine, _| {
        Ok(engine
            .notes
            .values()
            .filter(|n| n.tags.iter().any(|g| g.to_lowercase().starts_with(&t)))
            .map(|n| FileHit {
                path: n.path.clone(),
                title: n.title.clone(),
                aliases: n.aliases.clone(),
            })
            .collect())
    })
}
