//! 快速通道执行：指纹命中即直接 spawn（零 AI、零网络），输出走同一套 console 事件。
//! 命令执行仍过危险命令拦截——配方不是绕过安全门的后门。

use super::model::RecipeRunResponse;
use super::store;
use crate::config;
use std::collections::HashMap;
use std::path::Path;
use tauri::AppHandle;

/// 执行配方（校验平台 + 指纹 → spawn）。失败原因写入 response.reason。
/// root 由命令层取好传入，本函数可在后台线程整段执行（指纹哈希 + 工具链环境构建都不轻）。
pub fn run(
    app: AppHandle,
    root: &Path,
    action: &str,
    variant: &str,
) -> Result<RecipeRunResponse, String> {
    let settings = config::load_settings(&app);
    let env = crate::toolchain::build_env(&settings);
    let id = store::recipe_id(action, variant);
    let Some(recipe) = store::load(root, &id) else {
        return Ok(RecipeRunResponse {
            ok: false,
            reason: "missing".into(),
            ..Default::default()
        });
    };
    if !store::platform_matches(&recipe) {
        return Ok(RecipeRunResponse {
            ok: false,
            reason: "platform".into(),
            command: recipe.command.clone(),
            ..Default::default()
        });
    }
    let (valid, changed) = store::check_fingerprint(root, &recipe);
    if !valid {
        return Ok(RecipeRunResponse {
            ok: false,
            reason: "stale".into(),
            changed,
            command: recipe.command.clone(),
            ..Default::default()
        });
    }
    if recipe.command.trim().is_empty() {
        return Ok(RecipeRunResponse {
            ok: false,
            reason: "配方命令为空".into(),
            ..Default::default()
        });
    }
    if let Some(why) = crate::commands::agent::is_dangerous(&recipe.command) {
        return Ok(RecipeRunResponse {
            ok: false,
            reason: format!("危险命令被拦截（{}）", why),
            command: recipe.command.clone(),
            ..Default::default()
        });
    }

    // 工具链环境 + 配方自定义 env 覆盖
    let mut merged: HashMap<String, String> = env.into_iter().collect();
    for (k, v) in &recipe.env {
        merged.insert(k.clone(), v.clone());
    }
    let cwd = if recipe.cwd.trim().is_empty() {
        root.to_path_buf()
    } else {
        root.join(&recipe.cwd)
    };

    let cmd_id = crate::commands::agent::spawn_streaming(
        app,
        recipe.command.clone(),
        cwd,
        merged.into_iter().collect(),
    );
    store::touch_usage(root, &id);
    Ok(RecipeRunResponse {
        ok: true,
        cmd_id: Some(cmd_id),
        command: recipe.command.clone(),
        ..Default::default()
    })
}
