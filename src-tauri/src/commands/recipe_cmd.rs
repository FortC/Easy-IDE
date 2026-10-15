//! 配方系统命令层（V2-P0）：
//! - project_detect：项目模型探测（类型/模块/任务槽/指纹文件）
//! - recipe_*：配方 CRUD + 状态检查 + 快速通道执行 + 使用记录
//! 配方文件（.easyide/recipes/）由本模块独占写入，write_text_file 拒绝写该目录（防注入）。

use crate::project_model::{self, ProjectModel};
use crate::recipe::execute;
use crate::recipe::model::{Recipe, RecipeRunResponse, RecipeStatus};
use crate::recipe::store;
use crate::state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, State};

/// 取工作区根目录快照（锁内取值，不跨 await 持锁）
pub(crate) fn vault_root(state: &State<'_, AppState>) -> Result<PathBuf, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    guard
        .as_ref()
        .map(|vc| vc.root.clone())
        .ok_or_else(|| "尚未打开工作区".to_string())
}

// ---------------- 项目模型 ----------------

/// 项目模型探测（读取解析大量构建文件，放后台线程避免阻塞 UI）
#[tauri::command]
pub async fn project_detect(state: State<'_, AppState>) -> Result<ProjectModel, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || project_model::detect(&root))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

// ---------------- 配方 CRUD ----------------

/// 前端提交的配方输入（指纹/平台/版本由后端补齐）
#[derive(serde::Deserialize, Default)]
#[serde(default)]
pub struct RecipeInput {
    pub action: String,
    pub variant: String,
    pub command: String,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub ready_pattern: String,
    pub port: u16,
    /// ai | user
    pub source: String,
    pub model: String,
    pub conversation_id: String,
    pub prompt: String,
    pub trusted: bool,
}

#[tauri::command]
pub async fn recipe_save(state: State<'_, AppState>, input: RecipeInput) -> Result<Recipe, String> {
    if input.action.trim().is_empty() {
        return Err("配方缺少 action".into());
    }
    if input.command.trim().is_empty() {
        return Err("配方命令为空".into());
    }
    // 危险命令不允许沉淀为配方：执行时的硬拦截兜底之外，保存时即拒绝（含 AI 来源配方）
    if let Some(why) = crate::commands::agent::is_dangerous(&input.command) {
        return Err(format!("危险命令不允许保存为配方（{}）：{}", why, input.command));
    }
    let root = vault_root(&state)?;
    // 保存要做项目探测 + 构建文件指纹哈希，放后台线程避免阻塞 UI
    tauri::async_runtime::spawn_blocking(move || {
        let id = store::recipe_id(&input.action, &input.variant);

        let mut recipe =
            store::load(&root, &id).unwrap_or_else(|| store::new_recipe(&input.action, &input.variant));
        recipe.command = input.command.trim().to_string();
        recipe.cwd = input.cwd.trim().trim_start_matches('/').to_string();
        recipe.env = input.env;
        recipe.expect.ready_pattern = input.ready_pattern.trim().to_string();
        recipe.expect.port = input.port;
        recipe.expect.success_exit_codes = vec![0];

        // 强指纹：探测到的构建文件全量 hash（保存时点的项目状态 = 批准基准）
        let model = project_model::detect(&root);
        recipe.preconditions.platform = store::platform();
        recipe.preconditions.fingerprint = store::compute_fingerprint(&root, &model.fingerprint_files);

        recipe.meta.version = recipe.meta.version.saturating_add(1);
        if recipe.meta.created_at.is_empty() {
            recipe.meta.created_at = store::now();
            recipe.meta.created_by = if input.source == "user" { "user".into() } else { "ai".into() };
        }
        recipe.meta.status = "active".into();
        recipe.meta.source = if input.source == "user" { "user".into() } else { "ai".into() };
        recipe.meta.model = input.model;
        recipe.meta.conversation_id = input.conversation_id;
        recipe.meta.prompt = input.prompt;
        recipe.meta.trusted = input.trusted;

        store::save(&root, &recipe).map_err(|e| format!("保存配方失败：{}", e))?;
        Ok(recipe)
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}

#[tauri::command]
pub fn recipe_list(state: State<'_, AppState>) -> Result<Vec<Recipe>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(store::list(&vc.root))
}

#[tauri::command]
pub fn recipe_get(
    state: State<'_, AppState>,
    action: String,
    variant: Option<String>,
) -> Result<Option<Recipe>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(store::load(&vc.root, &store::recipe_id(&action, variant.as_deref().unwrap_or(""))))
}

#[tauri::command]
pub fn recipe_delete(
    state: State<'_, AppState>,
    action: String,
    variant: Option<String>,
) -> Result<(), String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    store::delete(&vc.root, &store::recipe_id(&action, variant.as_deref().unwrap_or("")));
    Ok(())
}

/// 全部配方的状态摘要（RunBar 徽标：valid=直通 / valid=false=需确认重推理 / 无=需推理）
/// 指纹校验要逐个哈希构建文件，放后台线程避免阻塞 UI
#[tauri::command]
pub async fn recipe_check(state: State<'_, AppState>) -> Result<Vec<RecipeStatus>, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(store::list(&root)
            .into_iter()
            .map(|r| {
                let (valid, changed) = store::check_fingerprint(&root, &r);
                let valid = valid && store::platform_matches(&r);
                RecipeStatus {
                    id: r.id(),
                    action: r.action.clone(),
                    variant: r.variant.clone(),
                    command: r.command.clone(),
                    cwd: r.cwd.clone(),
                    valid,
                    changed,
                    usage_count: r.meta.usage_count,
                    avg_duration_ms: r.meta.avg_duration_ms,
                    trusted: r.meta.trusted,
                    status: r.meta.status.clone(),
                }
            })
            .collect())
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}

// ---------------- 快速通道 ----------------

#[tauri::command]
pub async fn recipe_run(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    variant: Option<String>,
) -> Result<RecipeRunResponse, String> {
    let root = vault_root(&state)?;
    let variant = variant.unwrap_or_default();
    // 指纹校验要哈希构建文件、工具链环境构建可能 spawn 子进程，放后台线程避免阻塞 UI
    tauri::async_runtime::spawn_blocking(move || execute::run(app, &root, &action, &variant))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

/// 信任标记：指纹命中即免确认直通（不升版本）
#[tauri::command]
pub fn recipe_trust(
    state: State<'_, AppState>,
    action: String,
    variant: Option<String>,
    trusted: bool,
) -> Result<(), String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    let id = store::recipe_id(&action, variant.as_deref().unwrap_or(""));
    if let Some(mut r) = store::load(&vc.root, &id) {
        r.meta.trusted = trusted;
        store::save(&vc.root, &r).map_err(|e| format!("保存配方失败：{}", e))?;
    }
    Ok(())
}

/// 快速通道执行结束后的使用记录（前端在 console-exit 时调用）
#[tauri::command]
pub fn recipe_record_usage(
    state: State<'_, AppState>,
    action: String,
    variant: Option<String>,
    code: Option<i32>,
    duration_ms: u64,
) -> Result<(), String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    store::record_result(
        &vc.root,
        &store::recipe_id(&action, variant.as_deref().unwrap_or("")),
        code,
        duration_ms,
    );
    Ok(())
}

// ---------------- 保存的动作按钮（B5.2）----------------
// 存 .easyide/actions.json。写入同样只经本模块，禁止 write_text_file 直写
// （与 recipes/ 同一防注入口径）。

/// 读取全部保存的按钮；文件不存在/损坏返回空列表
#[tauri::command]
pub fn saved_actions_list(state: State<'_, AppState>) -> Vec<crate::recipe::saved::SavedAction> {
    let Ok(root) = vault_root(&state) else {
        return Vec::new();
    };
    crate::recipe::saved::load(&root)
        .map(|f| f.actions)
        .unwrap_or_default()
}

/// 覆盖写入全部保存的按钮（前端编辑后整体提交，避免逐条竞态）
#[tauri::command]
pub fn saved_actions_save(
    state: State<'_, AppState>,
    actions: Vec<crate::recipe::saved::SavedAction>,
) -> Result<(), String> {
    let root = vault_root(&state)?;
    let file = crate::recipe::saved::SavedActionsFile {
        version: 1,
        actions,
    };
    crate::recipe::saved::save(&root, &file).map_err(|e| format!("保存动作按钮失败：{}", e))
}

/// 直接执行一条保存的命令（B5.2：点按钮不再询问 AI）。
///
/// ⚠️ 安全：保存的按钮仍过危险命令硬拦截 —— 「用户自己保存过」不等于「可以绕过安全门」，
/// 否则一次误保存就能把 `rm -rf` 变成一个随手点的按钮。
#[tauri::command]
pub async fn saved_action_run(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
    background: Option<bool>,
) -> Result<crate::commands::agent::SpawnResult, String> {
    if let Some(why) = crate::commands::agent::is_dangerous(&command) {
        return Err(format!("已拦截危险命令（{}）：{}", why, command));
    }
    let root = vault_root(&state)?;
    let bg_app = app.clone();
    let env = tauri::async_runtime::spawn_blocking(move || {
        let settings = crate::config::load_settings(&bg_app);
        crate::toolchain::build_env(&settings)
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?;
    let (id, pid) = crate::commands::agent::spawn_streaming_detached(app, command, root, env);
    let _ = background; // background 只影响前端状态展示，执行路径一致
    Ok(crate::commands::agent::SpawnResult { id, pid })
}
