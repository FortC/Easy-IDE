//! 项目文档中心（移植自 idea-md-assistant 插件）：
//! 扫描项目内 Markdown → 五级分类链 → 分组展示/归档移动/ZIP/AGENTS.md 生成。
//! 知识库为项目维度：全部状态持久化在 `<项目根>/.easyide/md-assistant.json`。

pub mod hub;
pub mod model;
pub mod rules;
