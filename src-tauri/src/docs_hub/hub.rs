//! 文档中心核心：扫描 + 分类链 + 归档移动 + ZIP 打包 + AGENTS.md 生成。

use super::model::{
    all_categories, manual_category, AiCategoryRecord, DocEntry, DocsState, AI_DOCS_CATEGORY,
    KNOWLEDGE_ROLES,
};
use super::rules;
use crate::code_index::engine::is_excluded_dir;
use chrono::{Local, TimeZone};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 文档扫描的额外排除目录（在代码索引排除名单之上）
fn is_docs_excluded(name: &str) -> bool {
    is_excluded_dir(name)
        || matches!(name, ".idea" | ".svn" | ".hg" | ".pytest_cache" | ".terraform" | "tmp" | "temp" | "logs" | "coverage")
}

/// 解析 AI 工具目录名单（换行分隔）
pub fn ai_tool_dirs(state: &DocsState) -> HashSet<String> {
    state
        .ai_tool_dirs
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn state_path(root: &Path) -> PathBuf {
    root.join(".easyide").join("md-assistant.json")
}

pub fn load_state(root: &Path) -> DocsState {
    std::fs::read_to_string(state_path(root))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_state(root: &Path, state: &DocsState) {
    let path = state_path(root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rel_path(root: &Path, full: &Path) -> String {
    full.strip_prefix(root)
        .unwrap_or(full)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 五级分类链：手动映射 → 知识库目录 → AI 工具目录 → AI 分类缓存(mtime 校验) → 规则打分 → 未分类
fn classify_entry(root: &Path, state: &DocsState, rel: &str, mtime: u64) -> Option<String> {
    if let Some(c) = manual_category(rel, &state.dir_mapping) {
        return Some(c);
    }
    for role in KNOWLEDGE_ROLES {
        if let Some(dir) = state.knowledge_dirs.get(role) {
            if !dir.is_empty() && (rel == dir.as_str() || rel.starts_with(&format!("{}/", dir))) {
                return Some(dir.clone());
            }
        }
    }
    // AI 工具目录子树（任一祖先目录命中名单）
    let ai_dirs = ai_tool_dirs(state);
    if rel
        .split('/')
        .any(|seg| ai_dirs.contains(seg))
    {
        return Some(AI_DOCS_CATEGORY.to_string());
    }
    if let Some(rec) = state.ai_category.get(rel) {
        if rec.mtime == mtime {
            return Some(rec.cat.clone());
        }
    }
    // 规则兜底：读内容提标题/摘要（只读前 8KB）
    let full = root.join(rel);
    if let Ok(text) = read_head(&full, 8192) {
        let title = rules::md_title(&text);
        let excerpt = rules::md_excerpt(&text, 300);
        if let Some(c) = rules::classify(rel, title.as_deref(), &excerpt) {
            return Some(c);
        }
    }
    None
}

/// 只读文件前 max_bytes 字节（有损解码，分类摘要够用）
fn read_head(full: &Path, max_bytes: u64) -> Result<String, std::io::Error> {
    use std::io::Read;
    let mut f = std::fs::File::open(full)?;
    let meta = f.metadata()?;
    let take = meta.len().min(max_bytes) as usize;
    let mut buf = vec![0u8; take];
    f.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// 扫描项目内全部 md（排除忽略项），附带生效类别
pub fn scan(root: &Path, state: &DocsState) -> Vec<DocEntry> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_docs_excluded(&e.file_name().to_string_lossy()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".md") && !name.ends_with(".markdown") {
            continue;
        }
        let rel = rel_path(root, entry.path());
        if state.ignored.contains_key(&rel) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let mtime = mtime_of(&meta);
        let date_key = Local
            .timestamp_millis_opt(mtime as i64)
            .single()
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        out.push(DocEntry {
            path: rel.clone(),
            name: entry
                .file_name()
                .to_string_lossy()
                .to_string(),
            mtime,
            size: meta.len(),
            from_ai_dir: classify_entry(root, state, &rel, mtime)
                .as_deref()
                == Some(AI_DOCS_CATEGORY),
            date_key,
            category: classify_entry(root, state, &rel, mtime),
        });
        if out.len() >= 5000 {
            break; // 与插件一致的上限保护
        }
    }
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    out
}

/// 记录 AI 分类（带 mtime 失效戳）
pub fn store_ai_category(state: &mut DocsState, rel: &str, cat: &str, mtime: u64) {
    state.ai_category.insert(
        rel.to_string(),
        AiCategoryRecord {
            cat: cat.to_string(),
            mtime,
        },
    );
}

/// 归档移动：把文档物理移动到 <root>/<category>/，同名冲突加时间戳后缀。
/// 返回 (旧相对路径, 新相对路径) 列表。
pub fn move_docs(
    root: &Path,
    paths: &[String],
    category: &str,
) -> Result<Vec<(String, String)>, String> {
    let dir = root.join(category);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败：{}", e))?;
    let mut moved = Vec::new();
    for rel in paths {
        let full = root.join(rel);
        if !full.exists() {
            continue;
        }
        let file_name = full
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "doc.md".to_string());
        let mut target = dir.join(&file_name);
        if target.exists() && target != full {
            let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
            let (stem, ext) = split_ext(&file_name);
            target = dir.join(format!("{}-{}.{}", stem, stamp, ext));
        }
        if target == full {
            continue; // 已在目标位置
        }
        std::fs::rename(&full, &target).map_err(|e| format!("移动失败：{}", e))?;
        moved.push((rel.clone(), rel_path(root, &target)));
    }
    Ok(moved)
}

fn split_ext(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i + 1..].to_string()),
        _ => (name.to_string(), "md".to_string()),
    }
}

/// 打包 ZIP：entries 为 (相对路径, 包内名称)。输出到项目根 `docs-YYYYMMDD-HHMMSS.zip`。
pub fn zip_docs(root: &Path, entries: &[(String, String)]) -> Result<String, String> {
    if entries.is_empty() {
        return Err("没有可打包的文档".into());
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let out = root.join(format!("docs-{}.zip", stamp));
    let file = std::fs::File::create(&out).map_err(|e| format!("创建 ZIP 失败：{}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut used: HashSet<String> = HashSet::new();
    for (rel, display) in entries {
        let full = root.join(rel);
        let Ok(bytes) = std::fs::read(&full) else { continue };
        // 清洗非法字符 + 去重
        let mut name: String = display
            .chars()
            .map(|c| match c {
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect();
        if !name.to_lowercase().ends_with(".md") {
            name.push_str(".md");
        }
        let mut final_name = name.clone();
        let mut n = 2;
        while !used.insert(final_name.clone()) {
            let (stem, ext) = split_ext(&name);
            final_name = format!("{}-{}.{}", stem, n, ext);
            n += 1;
        }
        zip.start_file(final_name, opts)
            .map_err(|e| format!("写入 ZIP 失败：{}", e))?;
        zip.write_all(&bytes).map_err(|e| format!("写入 ZIP 失败：{}", e))?;
    }
    zip.finish().map_err(|e| format!("完成 ZIP 失败：{}", e))?;
    Ok(out.to_string_lossy().to_string())
}

// ---------------- AGENTS.md 生成（标记块合并，移植插件算法） ----------------

pub const BEGIN_MARKER: &str = "<!-- MD-ASSISTANT:AI-RULES:BEGIN -->";
pub const END_MARKER: &str = "<!-- MD-ASSISTANT:AI-RULES:END -->";
const MAX_ERROR_ITEMS: usize = 30;

/// 生成/更新项目 AI 规则文件（默认 AGENTS.md），返回其相对路径。
/// 生成结果同时是内置 AI Agent 的系统提示词素材（P3 消费）。
pub fn generate_agents(root: &Path, state: &DocsState) -> Result<String, String> {
    let lib_req = state.knowledge_dirs.get("req").cloned().unwrap_or_default();
    let lib_exa = state.knowledge_dirs.get("exa").cloned().unwrap_or_default();
    let lib_res = state.knowledge_dirs.get("res").cloned().unwrap_or_default();
    let lib_err = state.knowledge_dirs.get("err").cloned().unwrap_or_default();

    // 确保知识库目录存在
    for lib in [&lib_req, &lib_exa, &lib_res, &lib_err] {
        if !lib.is_empty() {
            let _ = std::fs::create_dir_all(root.join(lib));
        }
    }

    let mut error_items: Vec<String> = Vec::new();
    if state.rule_avoid_err && !lib_err.is_empty() {
        collect_error_items(&root.join(&lib_err), &mut error_items);
    }

    let body = build_agents_body(state, &lib_req, &lib_exa, &lib_res, &lib_err, &error_items);
    let target_rel = if state.agents_target.trim().is_empty() {
        "AGENTS.md".to_string()
    } else {
        state.agents_target.trim().replace('\\', "/")
    };
    let target = root.join(&target_rel);
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let existing = std::fs::read_to_string(&target).unwrap_or_default();
    let block = format!("{}\n{}\n{}\n", BEGIN_MARKER, body, END_MARKER);
    let merged = merge_markers(&existing, &block);
    std::fs::write(&target, merged).map_err(|e| format!("写入失败：{}", e))?;
    Ok(target_rel)
}

fn merge_markers(existing: &str, block: &str) -> String {
    if existing.trim().is_empty() {
        return block.to_string();
    }
    let b = existing.find(BEGIN_MARKER);
    let e = existing.find(END_MARKER);
    if let (Some(b), Some(e)) = (b, e) {
        if e > b {
            return format!("{}{}{}", &existing[..b], block, &existing[e + END_MARKER.len()..]);
        }
    }
    if existing.ends_with('\n') {
        format!("{}\n{}", existing, block)
    } else {
        format!("{}\n\n{}", existing, block)
    }
}

fn collect_error_items(dir: &Path, items: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if items.len() >= MAX_ERROR_ITEMS {
            return;
        }
        let p = e.path();
        if p.is_dir() {
            collect_error_items(&p, items);
        } else {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let lower = name.to_lowercase();
            if !lower.ends_with(".md") && !lower.ends_with(".markdown") {
                continue;
            }
            let Ok(text) = read_head(&p, 8192) else { continue };
            let title = rules::md_title(&text)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| split_ext(&name).0);
            let excerpt = rules::md_excerpt(&text, 80);
            items.push(if excerpt.is_empty() {
                format!("- {}", title)
            } else {
                format!("- {}：{}", title, excerpt)
            });
        }
    }
}

fn build_agents_body(
    state: &DocsState,
    lib_req: &str,
    lib_exa: &str,
    lib_res: &str,
    lib_err: &str,
    error_items: &[String],
) -> String {
    let mut sb = String::new();
    sb.push_str("# AI 协作规则（由 EasyIDE 文档中心维护，标记块之外的内容可自由编辑）\n\n");
    if state.rule_read_req && !lib_req.is_empty() {
        sb.push_str(&format!(
            "## 需求\n- 开始工作前，先阅读 `{}/` 目录下的最新需求文档。\n\n",
            lib_req
        ));
    }
    if state.rule_prefer_exa && !lib_exa.is_empty() {
        sb.push_str(&format!(
            "## 优先参考\n- 生成代码时，优先在 `{}/` 目录中查找参考实现，遵循其中的风格与约定。\n\n",
            lib_exa
        ));
    }
    if state.rule_avoid_err && !lib_err.is_empty() {
        sb.push_str(&format!("## 避免（来自 `{}/` 错误库）\n- 请勿重复以下记录过的错误：\n", lib_err));
        if error_items.is_empty() {
            sb.push_str("  - （暂无记录；向错误库目录添加文档即可自动汇总）\n");
        } else {
            for item in error_items {
                sb.push_str(item);
                sb.push('\n');
            }
        }
        sb.push('\n');
    }
    if state.rule_archive_res && !lib_res.is_empty() {
        sb.push_str(&format!(
            "## 归档\n- 任务完成后，将产出说明（做了什么、关键决策、核心代码片段）写入 `{}/` 目录。\n\n",
            lib_res
        ));
    }
    if !state.rule_extra.trim().is_empty() {
        sb.push_str("## 附加规则\n");
        sb.push_str(state.rule_extra.trim());
        sb.push_str("\n\n");
    }
    sb.trim().to_string()
}

/// 统一词表（供命令层暴露给前端下拉框/AI 分类）
pub fn category_vocab(state: &DocsState) -> Vec<String> {
    all_categories(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_markers_roundtrip() {
        let block = format!("{}{}{}\n", BEGIN_MARKER, "BODY-1", END_MARKER);
        // 空文件
        assert_eq!(merge_markers("", &block), block);
        // 无标记 → 追加
        let m = merge_markers("# 手写内容", &block);
        assert!(m.starts_with("# 手写内容\n\n"));
        // 已有标记 → 原位替换，手写内容保留
        let old = format!("# 头\n{}OLD{}", BEGIN_MARKER, END_MARKER);
        let m = merge_markers(&old, &block);
        assert!(m.starts_with("# 头\n"));
        assert!(m.contains("BODY-1"));
        assert!(!m.contains("OLD"));
    }

    #[test]
    fn classify_chain_priority() {
        let root = std::env::temp_dir().join(format!("easyide-docs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("design")).unwrap();
        std::fs::write(root.join("design").join("a.md"), "# 架构说明\n").unwrap();
        std::fs::write(root.join("README.md"), "# 项目\n").unwrap();
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        std::fs::write(root.join(".claude").join("rules.md"), "# 规则\n").unwrap();

        let mut state = DocsState::default();
        state.dir_mapping.insert("design".to_string(), "设计文档".to_string());
        let entries = scan(&root, &state);
        let find = |p: &str| entries.iter().find(|e| e.path == p).cloned();

        // 手动映射优先
        assert_eq!(find("design/a.md").unwrap().category.as_deref(), Some("设计文档"));
        // 规则兜底（README → 项目说明）
        assert_eq!(find("README.md").unwrap().category.as_deref(), Some("项目说明"));
        // AI 目录 → 固定组
        let c = find(".claude/rules.md").unwrap();
        assert_eq!(c.category.as_deref(), Some(AI_DOCS_CATEGORY));
        assert!(c.from_ai_dir);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_and_zip() {
        let root = std::env::temp_dir().join(format!("easyide-docs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes").join("会议.md"), "# 会议\n").unwrap();

        let moved = move_docs(&root, &["notes/会议.md".to_string()], "会议记录").unwrap();
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].1, "会议记录/会议.md");
        assert!(root.join("会议记录").join("会议.md").exists());

        let zip = zip_docs(&root, &[("会议记录/会议.md".to_string(), "会议记录/会议".to_string())]).unwrap();
        assert!(zip.ends_with(".zip"));
        assert!(std::fs::metadata(&zip).unwrap().len() > 0);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn agents_generation() {
        let root = std::env::temp_dir().join(format!("easyide-docs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut state = DocsState::default();
        state.knowledge_dirs.insert("err".into(), "错误库".into());
        std::fs::create_dir_all(root.join("错误库")).unwrap();
        std::fs::write(root.join("错误库").join("空指针.md"), "# 空指针异常\n调用前未判空导致\n").unwrap();

        let rel = generate_agents(&root, &state).unwrap();
        assert_eq!(rel, "AGENTS.md");
        let text = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(text.contains(BEGIN_MARKER));
        assert!(text.contains("空指针异常"));
        assert!(text.contains("错误库"));

        // 二次生成：原位替换不重复
        std::thread::sleep(std::time::Duration::from_millis(10));
        generate_agents(&root, &state).unwrap();
        let text2 = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert_eq!(text2.matches(BEGIN_MARKER).count(), 1);

        std::fs::remove_dir_all(&root).ok();
    }
}
