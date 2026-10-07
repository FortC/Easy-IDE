//! 全局状态：当前打开的 vault 上下文。

use crate::code_index::engine::CodeIndexEngine;
use crate::docs_hub::model::DocsState;
use crate::index::engine::IndexEngine;
use crate::index::watcher::WatcherHandle;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

pub struct VaultContext {
    pub root: PathBuf,
    pub engine: IndexEngine,
    pub cache_path: PathBuf,
    /// 代码索引（符号/文件清单），与笔记引擎平行
    pub code_engine: CodeIndexEngine,
    pub code_cache_path: PathBuf,
    /// 文档中心项目级状态（.easyide/md-assistant.json 的内存镜像）
    pub docs: DocsState,
    pub watcher: Option<WatcherHandle>,
}

impl VaultContext {
    /// 相对路径 → 绝对路径（拒绝 `..`、绝对路径注入，保证不出 vault）
    pub fn safe_join(&self, rel: &str) -> anyhow::Result<PathBuf> {
        let rel_path = Path::new(rel);
        if rel_path.is_absolute() {
            anyhow::bail!("非法路径：{:?}", rel);
        }
        for c in rel_path.components() {
            if matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
                anyhow::bail!("非法路径：{:?}", rel);
            }
        }
        Ok(self.root.join(rel_path))
    }
}

#[derive(Default)]
pub struct AppState {
    pub vault: Mutex<Option<VaultContext>>,
    /// 系统启动参数里带的文件路径（右键"用 EasyIDE 打开"传入） */
    pub pending_file: Mutex<Option<String>>,
}
