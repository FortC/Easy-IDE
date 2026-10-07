//! 文档中心数据模型与项目级持久化状态。

use std::collections::BTreeMap;

/// 扫描出的一个 md 文档
#[derive(serde::Serialize, Clone, Debug)]
pub struct DocEntry {
    pub path: String,
    pub name: String,
    pub mtime: u64,
    pub size: u64,
    /// 位于 AI 工具目录（.claude/.cursor 等）
    pub from_ai_dir: bool,
    /// 修改日期 yyyy-MM-dd（按日期分组用）
    pub date_key: String,
    /// 分类链计算出的生效类别（None = 未分类）
    pub category: Option<String>,
}

/// AI 工具目录默认名单（移植自插件，31 个）
pub const DEFAULT_AI_TOOL_DIRS: &str = ".codex\n.workbuddy\n.claude\n.claude-code\n.cursor\n.aider\n.continue\n.copilot\n.github-copilot\n.windsurf\n.codeium\n.tabnine\n.cody\n.gemini\n.qwen\n.kiro\n.trae\n.augment\n.augment-guidelines\n.cline\n.roo\n.goose\n.openai\n.anthropic\n.zcode\n.openhands\n.devin\n.avante\n.spec-story\nspecstory\nmemory-bank";

/// 内置类别词表（目录库）
pub const DEFAULT_CATEGORIES: &[&str] = &[
    "项目说明", "设计文档", "开发文档", "接口文档", "教程指南",
    "会议记录", "计划任务", "笔记备忘", "翻译内容", "其他",
];

/// 知识库角色顺序：需求 → 示例 → 成果 → 错误
pub const KNOWLEDGE_ROLES: [&str; 4] = ["req", "exa", "res", "err"];
pub const KNOWLEDGE_ROLE_NAMES: [&str; 4] = ["需求库", "示例库", "成果库", "错误库"];

/// AI 工具文档组的固定类别名（不会与用户类别冲突：手动归类不提供此项）
pub const AI_DOCS_CATEGORY: &str = "AI 工具文档";
/// 项目级持久化状态（.easyide/md-assistant.json）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DocsState {
    pub version: u32,
    /// 手动文件夹映射：路径前缀 → 类别（最长前缀优先）
    #[serde(default)]
    pub dir_mapping: BTreeMap<String, String>,
    /// AI 分类缓存：path → {类别, 分类时的 mtime}（文件改过即失效）
    #[serde(default)]
    pub ai_category: BTreeMap<String, AiCategoryRecord>,
    /// 忽略列表：path → 忽略时间
    #[serde(default)]
    pub ignored: BTreeMap<String, String>,
    /// 知识库目录的项目级配置：role → 目录名（空串 = 停用）
    #[serde(default)]
    pub knowledge_dirs: BTreeMap<String, String>,
    /// AI 工具目录名单（换行分隔；默认 31 个）
    #[serde(default)]
    pub ai_tool_dirs: String,
    /// 是否默认展示 AI 工具文档组
    #[serde(default)]
    pub show_ai_tool_docs: bool,
    /// 默认分组方式：dir | smart | flat | date
    #[serde(default)]
    pub default_group: String,
    /// AGENTS.md 生成配置
    #[serde(default)]
    pub agents_target: String,
    #[serde(default = "default_true")]
    pub rule_read_req: bool,
    #[serde(default = "default_true")]
    pub rule_prefer_exa: bool,
    #[serde(default = "default_true")]
    pub rule_avoid_err: bool,
    #[serde(default = "default_true")]
    pub rule_archive_res: bool,
    #[serde(default)]
    pub rule_extra: String,
    /// 目录库（类别词表，可编辑维护）
    #[serde(default = "default_categories")]
    pub categories: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AiCategoryRecord {
    pub cat: String,
    pub mtime: u64,
}

fn default_true() -> bool {
    true
}

fn default_categories() -> Vec<String> {
    DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect()
}

impl Default for DocsState {
    fn default() -> Self {
        DocsState {
            version: 1,
            dir_mapping: BTreeMap::new(),
            ai_category: BTreeMap::new(),
            ignored: BTreeMap::new(),
            knowledge_dirs: KNOWLEDGE_ROLES
                .iter()
                .zip(KNOWLEDGE_ROLE_NAMES.iter())
                .map(|(r, n)| (r.to_string(), n.to_string()))
                .collect(),
            ai_tool_dirs: DEFAULT_AI_TOOL_DIRS.to_string(),
            show_ai_tool_docs: false,
            default_group: "smart".to_string(),
            agents_target: "AGENTS.md".to_string(),
            rule_read_req: true,
            rule_prefer_exa: true,
            rule_avoid_err: true,
            rule_archive_res: true,
            rule_extra: String::new(),
            categories: default_categories(),
        }
    }
}

/// 统一类别词表：目录库 + 本项目生效的知识库目录名（去重、保序）
pub fn all_categories(state: &DocsState) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in &state.categories {
        if !out.contains(c) {
            out.push(c.clone());
        }
    }
    for role in KNOWLEDGE_ROLES {
        if let Some(dir) = effective_knowledge_dir(state, role) {
            if !dir.is_empty() && !out.contains(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

/// 生效知识库目录：role → 目录名（知识库为项目维度，直接取项目配置）
pub fn effective_knowledge_dir(state: &DocsState, role: &str) -> Option<String> {
    state.knowledge_dirs.get(role).cloned()
}

/// 手动映射匹配（最长前缀优先；命中 等于前缀 或 前缀/ 开头）
pub fn manual_category(path: &str, mapping: &BTreeMap<String, String>) -> Option<String> {
    let mut best: Option<&str> = None;
    for key in mapping.keys() {
        let mut k = key.trim();
        while let Some(stripped) = k.strip_prefix("./") {
            k = stripped;
        }
        while let Some(stripped) = k.strip_suffix('/') {
            k = stripped;
        }
        if k.is_empty() {
            continue;
        }
        if path == k || path.starts_with(&format!("{}/", k)) {
            if best.map(|b| k.len() > b.len()).unwrap_or(true) {
                best = Some(k);
            }
        }
    }
    best.and_then(|k| mapping.get(k).cloned())
}
