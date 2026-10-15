//! 保存的动作按钮存取（B5.2）：`.easyide/actions.json`
//!
//! 与配方（recipe）的分工：
//!  - 配方：按动作 key 固定、带指纹校验，面向可复现的构建/测试命令
//!  - 保存动作：用户随手保存的任意命令或提示词，面向日常操作（启动、解读…）
//!
//! 磁盘文件是唯一真相源；前端只是编辑视图。文件损坏时回落空列表，不阻断面板。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SavedActionsFile {
    /// 显式版本，便于将来字段演进
    #[serde(default = "schema_version")]
    pub version: u32,
    #[serde(default)]
    pub actions: Vec<SavedAction>,
}

fn schema_version() -> u32 {
    1
}

/// 单个保存的按钮
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedAction {
    /// 稳定唯一 id（重启后不变）
    pub id: String,
    /// 展示名
    pub label: String,
    /// "command" = 直接执行命令；"prompt" = 交给 AI
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// 长驻进程：点击不等待退出，需要显式停止
    #[serde(default)]
    pub background: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_pattern: Option<String>,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_key: Option<String>,
    #[serde(default)]
    pub order: i32,
}

fn default_kind() -> String {
    "command".to_string()
}

fn default_icon() -> String {
    "zap".to_string()
}

pub fn actions_path(root: &std::path::Path) -> PathBuf {
    root.join(".easyide").join("actions.json")
}

/// 读取；文件不存在或损坏均返回 None（调用方回落空列表）
pub fn load(root: &std::path::Path) -> Option<SavedActionsFile> {
    let p = actions_path(root);
    let text = std::fs::read_to_string(p).ok()?;
    // 损坏文件不能让面板打不开：解析失败即当作没有保存过
    serde_json::from_str::<SavedActionsFile>(&text).ok()
}

/// 原子写入：先写临时文件再改名，避免中途崩溃留下半截 JSON
pub fn save(root: &std::path::Path, f: &SavedActionsFile) -> anyhow::Result<()> {
    let path = actions_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(f)?)?;
    // Windows 上 rename 覆盖已存在文件会失败，先删再改
    let _ = std::fs::remove_file(&path);
    std::fs::rename(&tmp, &path)?;
    Ok(())
}