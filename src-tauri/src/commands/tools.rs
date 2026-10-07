//! P4 内置工具命令：Git 状态/差异/基线、Maven 依赖树/一键排除、增量包构建/历史。

use crate::gitops;
use crate::maven;
use crate::pkg;
use crate::state::AppState;
use tauri::{AppHandle, State};

// ---------------- Git ----------------

#[tauri::command]
pub fn git_status(app: AppHandle, state: State<'_, AppState>) -> Result<gitops::GitStatus, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(gitops::status(&app, &vc.root))
}

#[tauri::command]
pub fn git_diff(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    gitops::diff_file(&app, &vc.root, &path)
}

#[tauri::command]
pub fn git_log(
    app: AppHandle,
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<gitops::CommitInfo>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(gitops::log(&app, &vc.root, limit.unwrap_or(30)))
}

#[tauri::command]
pub fn git_tags(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(gitops::tags(&app, &vc.root))
}

// ---------------- Maven ----------------

#[tauri::command]
pub fn maven_dep_tree(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<maven::ModuleTree>, String> {
    let root = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("尚未打开工作区")?.root.clone()
    };
    maven::dependency_tree(&app, &root)
}

/// 一键排除：在 pom.xml 中给直接依赖插入 <exclusion>（先备份 pom.xml.bak）
#[tauri::command]
pub fn maven_apply_exclusion(
    _app: AppHandle,
    state: State<'_, AppState>,
    dep_group: String,
    dep_artifact: String,
    excl_group: String,
    excl_artifact: String,
) -> Result<String, String> {
    let root = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("尚未打开工作区")?.root.clone()
    };
    let pom_path = root.join("pom.xml");
    if !pom_path.exists() {
        return Err("项目根没有 pom.xml（多模块请到子模块目录操作）".into());
    }
    let pom = std::fs::read_to_string(&pom_path).map_err(|e| format!("读取 pom.xml 失败：{}", e))?;
    let updated = maven::apply_exclusion(&pom, &dep_group, &dep_artifact, &excl_group, &excl_artifact)?;
    // 备份后写入
    let _ = std::fs::write(root.join("pom.xml.bak"), &pom);
    std::fs::write(&pom_path, &updated).map_err(|e| format!("写入 pom.xml 失败：{}", e))?;
    Ok(updated)
}

// ---------------- 增量包 ----------------

#[derive(serde::Serialize)]
pub struct PkgBaselines {
    pub tags: Vec<String>,
    pub commits: Vec<gitops::CommitInfo>,
}

#[tauri::command]
pub fn pkg_baselines(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PkgBaselines, String> {
    let root = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("尚未打开工作区")?.root.clone()
    };
    Ok(PkgBaselines {
        tags: gitops::tags(&app, &root),
        commits: gitops::log(&app, &root, 30),
    })
}

#[tauri::command]
pub fn pkg_build(
    app: AppHandle,
    state: State<'_, AppState>,
    baseline: String,
    template: String,
) -> Result<pkg::PkgResult, String> {
    let root = {
        let guard = state.vault.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("尚未打开工作区")?.root.clone()
    };
    pkg::build(&app, &root, &baseline, &template)
}

#[tauri::command]
pub fn pkg_history(
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<pkg::PkgHistoryItem>, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vc = guard.as_ref().ok_or("尚未打开工作区")?;
    Ok(pkg::history(&vc.root))
}
