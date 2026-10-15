//! 配方数据模型（schema 与《改造报告与方案V2.md》4.2.2 对齐，P0 为 MVP 子集：强指纹 + 平台约束）。

use std::collections::HashMap;

pub const RECIPE_SCHEMA: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct RecipePreconditions {
    /// 生效平台（如 windows-x86_64）；空 = 不限
    pub platform: String,
    /// 工具链约束（kind → 版本区间，如 "jdk" → ">=17"）；P0 仅存储展示，P1 匹配
    pub toolchain: HashMap<String, String>,
    /// 强指纹：相对路径 → sha256(内容) hex；任一变化即失效
    pub fingerprint: HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct RecipeExpect {
    pub success_exit_codes: Vec<i32>,
    /// 启动成功判定（正则命中输出即就绪）；空 = 只看退出码
    pub ready_pattern: String,
    pub ready_timeout_ms: u64,
    pub port: u16,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct RecipeMeta {
    pub version: u32,
    /// active | invalidated | pinned
    pub status: String,
    /// ai | user | imported
    pub source: String,
    pub model: String,
    pub conversation_id: String,
    pub prompt: String,
    pub created_at: String,
    pub created_by: String,
    pub last_used_at: String,
    pub usage_count: u32,
    pub last_exit_code: Option<i32>,
    pub avg_duration_ms: u64,
    /// 用户信任后指纹命中免确认直通
    pub trusted: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Recipe {
    pub schema: u32,
    /// run | build | test | deps | debug | clean | package | custom:<name>
    pub action: String,
    /// 多模块/多入口变体；单入口为空
    pub variant: String,
    pub shell: String,
    pub cwd: String,
    pub command: String,
    pub env: HashMap<String, String>,
    pub preconditions: RecipePreconditions,
    pub expect: RecipeExpect,
    pub meta: RecipeMeta,
}

impl Recipe {
    /// 配方唯一标识：action 或 action~variant
    pub fn id(&self) -> String {
        crate::recipe::store::recipe_id(&self.action, &self.variant)
    }
}

/// 配方状态摘要（RunBar 徽标 / 管理器列表用）
#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct RecipeStatus {
    pub id: String,
    pub action: String,
    pub variant: String,
    pub command: String,
    pub cwd: String,
    /// 指纹与平台是否全部命中
    pub valid: bool,
    /// 失效时的变化文件清单
    pub changed: Vec<String>,
    pub usage_count: u32,
    pub avg_duration_ms: u64,
    pub trusted: bool,
    pub status: String,
}

/// 快速通道执行结果
#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct RecipeRunResponse {
    pub ok: bool,
    /// missing | stale | platform | 危险命令等原因描述（成功为空串）
    pub reason: String,
    pub changed: Vec<String>,
    pub cmd_id: Option<u32>,
    pub command: String,
}
