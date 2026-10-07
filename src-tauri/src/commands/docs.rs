//! 文档中心命令：扫描 / 分组数据 / 归档 / 忽略 / ZIP / AGENTS.md / AI 分类 / AI 快捷操作。

use crate::config;
use crate::docs_hub::{hub, model};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, State};

/// 一次扫描返回给前端的完整数据（文档 + 词表 + 配置）
#[derive(serde::Serialize)]
pub struct DocsScanResult {
    pub entries: Vec<model::DocEntry>,
    /// 统一词表（目录库 + 生效知识库目录）
    pub categories: Vec<String>,
    pub state: DocsStateView,
}

/// 暴露给前端的状态视图（含忽略计数等摘要）
#[derive(serde::Serialize)]
pub struct DocsStateView {
    pub knowledge_dirs: Vec<(String, String)>,
    pub ai_tool_dirs: String,
    pub show_ai_tool_docs: bool,
    pub default_group: String,
    pub agents_target: String,
    pub rule_extra: String,
    pub ignored_count: usize,
}

fn view_of(state: &model::DocsState) -> DocsStateView {
    DocsStateView {
        knowledge_dirs: model::KNOWLEDGE_ROLES
            .iter()
            .map(|r| (r.to_string(), state.knowledge_dirs.get(*r).cloned().unwrap_or_default()))
            .collect(),
        ai_tool_dirs: state.ai_tool_dirs.clone(),
        show_ai_tool_docs: state.show_ai_tool_docs,
        default_group: state.default_group.clone(),
        agents_target: state.agents_target.clone(),
        rule_extra: state.rule_extra.clone(),
        ignored_count: state.ignored.len(),
    }
}

/// with_vault 的可变版本（文档操作要写 .easyide 状态）
fn with_vault_mut<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&std::path::PathBuf, &mut model::DocsState) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_mut() {
        Some(vc) => {
            let ret = f(&vc.root, &mut vc.docs)?;
            hub::save_state(&vc.root, &vc.docs);
            Ok(ret)
        }
        None => Err("尚未打开工作区".into()),
    }
}

/// 扫描项目 md 文档（分类链已计算）
#[tauri::command]
pub fn docs_scan(state: State<'_, AppState>) -> Result<DocsScanResult, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(DocsScanResult {
        entries: hub::scan(&vc.root, &vc.docs),
        categories: hub::category_vocab(&vc.docs),
        state: view_of(&vc.docs),
    })
}

/// 归档移动：物理移动到类别同名目录（编辑器标签同步交给前端）
#[tauri::command]
pub fn docs_move(
    state: State<'_, AppState>,
    paths: Vec<String>,
    category: String,
) -> Result<Vec<(String, String)>, String> {
    with_vault_mut(&state, |root, _| hub::move_docs(root, &paths, &category))
}

/// 忽略文档
#[tauri::command]
pub fn docs_ignore(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    with_vault_mut(&state, |_root, docs| {
        for p in paths {
            docs.ignored.insert(p, now.clone());
        }
        Ok(())
    })
}

/// 取消忽略（path 为空 = 全部恢复）
#[tauri::command]
pub fn docs_unignore(state: State<'_, AppState>, path: Option<String>) -> Result<(), String> {
    with_vault_mut(&state, |_root, docs| {
        match path {
            Some(p) => {
                docs.ignored.remove(&p);
            }
            None => docs.ignored.clear(),
        }
        Ok(())
    })
}

/// ZIP 打包：entries 为 (相对路径, 包内显示名)。返回 zip 绝对路径。
#[tauri::command]
pub fn docs_zip(
    state: State<'_, AppState>,
    entries: Vec<(String, String)>,
) -> Result<String, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    hub::zip_docs(&vc.root, &entries)
}

/// 生成/更新 AGENTS.md（标记块合并），返回相对路径
#[tauri::command]
pub fn docs_generate_agents(state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    hub::generate_agents(&vc.root, &vc.docs)
}

/// 更新文档中心设置（知识库目录 / AI 目录名单 / 显示开关 / 默认分组 / agents 配置）
#[tauri::command]
pub fn docs_update_settings(
    state: State<'_, AppState>,
    settings: DocsSettingsPatch,
) -> Result<(), String> {
    with_vault_mut(&state, |_root, docs| {
        for (role, dir) in settings.knowledge_dirs {
            docs.knowledge_dirs.insert(role, dir);
        }
        if let Some(v) = settings.ai_tool_dirs {
            docs.ai_tool_dirs = v;
        }
        if let Some(v) = settings.show_ai_tool_docs {
            docs.show_ai_tool_docs = v;
        }
        if let Some(v) = settings.default_group {
            docs.default_group = v;
        }
        if let Some(v) = settings.agents_target {
            docs.agents_target = v;
        }
        if let Some(v) = settings.rule_extra {
            docs.rule_extra = v;
        }
        if let Some(v) = settings.categories {
            docs.categories = v;
        }
        Ok(())
    })
}

#[derive(serde::Deserialize)]
pub struct DocsSettingsPatch {
    #[serde(default)]
    knowledge_dirs: Vec<(String, String)>,
    #[serde(default)]
    ai_tool_dirs: Option<String>,
    #[serde(default)]
    show_ai_tool_docs: Option<bool>,
    #[serde(default)]
    default_group: Option<String>,
    #[serde(default)]
    agents_target: Option<String>,
    #[serde(default)]
    rule_extra: Option<String>,
    #[serde(default)]
    categories: Option<Vec<String>>,
}

/// 设置手动文件夹映射（整体替换）
#[tauri::command]
pub fn docs_set_mapping(
    state: State<'_, AppState>,
    mapping: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    with_vault_mut(&state, |_root, docs| {
        docs.dir_mapping = mapping;
        Ok(())
    })
}

/// AI 批量分类（15 个/批，严格 JSON 输出；进度经 docs-classify-progress 事件推送）
#[tauri::command]
pub async fn docs_ai_classify(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<usize, String> {
    let settings = config::load_settings(&app);
    if settings.ai_api_key.trim().is_empty() {
        return Err("未配置 AI API Key，请到 设置 → AI 中填写".into());
    }
    // 锁内取数据，锁外跑 AI
    let (root, mut docs, allowed) = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        let vc = guard.as_ref().ok_or("尚未打开工作区")?;
        (vc.root.clone(), vc.docs.clone(), hub::category_vocab(&vc.docs))
    };
    let total = paths.len();
    let mut done = 0usize;
    let mut classified = 0usize;
    for batch in paths.chunks(15) {
        let prompt = build_classify_prompt(&root, batch, &allowed);
        let system = "你是文档分类助手，只输出 JSON，不输出解释。";
        let batch_settings = settings.clone();
        let resp = tauri::async_runtime::spawn_blocking(move || {
            super::ai::chat_blocking(&batch_settings, &prompt, Some(system))
        })
        .await
        .map_err(|e| format!("AI 线程异常：{}", e))??;
        // 解析 {"results":[{"i":n,"c":"类别"}]}
        let json = strip_fences(&resp);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                for r in results {
                    let i = r.get("i").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let c = r.get("c").and_then(|x| x.as_str()).unwrap_or("");
                    if i >= 1 && i <= batch.len() && !c.is_empty() {
                        let cat = if allowed.iter().any(|a| a == c) {
                            c.to_string()
                        } else {
                            "其他".to_string()
                        };
                        let rel = &batch[i - 1];
                        let mtime = std::fs::metadata(root.join(rel))
                            .ok()
                            .map(|m| {
                                m.modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0)
                            })
                            .unwrap_or(0);
                        hub::store_ai_category(&mut docs, rel, &cat, mtime);
                        classified += 1;
                    }
                }
            }
        }
        done += batch.len();
        let _ = app.emit("docs-classify-progress", serde_json::json!({ "done": done, "total": total }));
    }
    // 写回状态
    {
        let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
        if let Some(vc) = guard.as_mut() {
            vc.docs = docs;
            hub::save_state(&vc.root, &vc.docs);
        }
    }
    Ok(classified)
}

fn build_classify_prompt(root: &std::path::Path, batch: &[String], allowed: &[String]) -> String {
    let mut sb = String::new();
    sb.push_str("请为下列 Markdown 文档分类。类别必须从以下集合中选择：");
    sb.push_str(&allowed.join("、"));
    sb.push('\n');
    sb.push_str("集合末尾为项目知识库类别；文档内容明显属于某个知识库（需求/示例/错误复盘/成果记录）时优先选择对应知识库类别。");
    sb.push('\n');
    sb.push_str("输出严格的 JSON，格式：{\"results\":[{\"i\":<序号>,\"c\":\"<类别>\"}]}，必须覆盖全部文档，不要输出任何其他内容。\n");
    sb.push_str("文档列表：\n");
    for (i, rel) in batch.iter().enumerate() {
        let content = std::fs::read_to_string(root.join(rel)).unwrap_or_default();
        let title = crate::docs_hub::rules::md_title(&content)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md").to_string());
        let excerpt = crate::docs_hub::rules::md_excerpt(&content, 300);
        sb.push_str(&format!("{}. 《{}》 路径: {} 摘要: {}\n", i + 1, title, rel, excerpt));
    }
    sb
}

/// 剥离 ``` 代码围栏（AI 可能包一层 fence）
fn strip_fences(s: &str) -> &str {
    let t = s.trim();
    if t.starts_with("```") {
        if let (Some(first_nl), Some(last_fence)) = (t.find('\n'), t.rfind("```")) {
            if last_fence > first_nl {
                return t[first_nl + 1..last_fence].trim();
            }
        }
    }
    t
}

/// 单文档 AI 快捷操作（总结/翻译/润色/大纲/待办），返回模型回复文本
#[tauri::command]
pub async fn docs_quick_op(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    op: String,
) -> Result<String, String> {
    let prompts: &[(&str, &str)] = &[
        ("summarize", "请全面总结这篇文档的核心内容，按要点分条输出。"),
        ("translate", "请将这篇文档完整翻译成英文，保持原有的 Markdown 结构与格式。"),
        ("polish", "请润色优化这篇文档的措辞与排版，保持原意不变，输出完整优化后的 Markdown。"),
        ("outline", "请为这篇文档生成一个层级大纲，每个标题附一句话说明。"),
        ("todos", "请提取这篇文档中所有待办事项或任务项，输出为任务清单。"),
    ];
    let prompt = prompts
        .iter()
        .find(|(k, _)| *k == op)
        .map(|(_, p)| p.to_string())
        .ok_or_else(|| format!("未知操作：{}", op))?;
    let settings = config::load_settings(&app);
    if settings.ai_api_key.trim().is_empty() {
        return Err("未配置 AI API Key，请到 设置 → AI 中填写".into());
    }
    let content = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        let vc = guard.as_ref().ok_or("尚未打开工作区")?;
        std::fs::read_to_string(vc.root.join(&path)).map_err(|e| format!("读取失败：{}", e))?
    };
    // 与插件一致：单文档上限 2 万字符
    let capped: String = if content.chars().count() > 20_000 {
        content.chars().take(20_000).collect()
    } else {
        content
    };
    let full = format!("{}\n\n---\n\n{}", prompt, capped);
    tauri::async_runtime::spawn_blocking(move || super::ai::chat_blocking(&settings, &full, None))
        .await
        .map_err(|e| format!("AI 线程异常：{}", e))?
}
