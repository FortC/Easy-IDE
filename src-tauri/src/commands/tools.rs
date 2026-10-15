//! P4 内置工具命令：版本控制状态/差异/基线（Git+SVN 统一分发）、
//! Maven 依赖树/一键排除（多模块）、增量包构建/历史、工具链刷新。
//! 凡是 spawn 子进程或大量 IO 的命令一律 async + spawn_blocking：
//! Tauri 非 async 命令在主线程执行，同步等 `Command::output()` 会冻结整个窗口。

use crate::maven;
use crate::config;
use crate::pkg;
use crate::state::AppState;
use crate::vcs;
use std::path::PathBuf;
use tauri::{AppHandle, State};

// ---------------- 版本控制（Git + SVN 统一入口） ----------------

/// 命令层 kind 参数解析："git"/"svn" 强制指定，其余自动探测
fn parse_kind(kind: Option<String>) -> Option<vcs::VcsKind> {
    kind.as_deref().and_then(vcs::VcsKind::parse)
}

/// 取工作区根目录快照（锁内取值，不跨 await 持锁）
fn vault_root(state: &State<'_, AppState>) -> Result<PathBuf, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    guard
        .as_ref()
        .map(|vc| vc.root.clone())
        .ok_or_else(|| "尚未打开工作区".to_string())
}

#[tauri::command]
pub async fn vcs_status(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: Option<String>,
) -> Result<vcs::VcsStatus, String> {
    let root = vault_root(&state)?;
    let kind = parse_kind(kind);
    tauri::async_runtime::spawn_blocking(move || vcs::status(&app, &root, kind))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

#[tauri::command]
pub async fn vcs_diff(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: Option<String>,
    path: String,
) -> Result<String, String> {
    let root = vault_root(&state)?;
    let kind = parse_kind(kind);
    tauri::async_runtime::spawn_blocking(move || vcs::diff_file(&app, &root, kind, &path))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

#[tauri::command]
pub async fn vcs_log(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<vcs::CommitInfo>, String> {
    let root = vault_root(&state)?;
    let kind = parse_kind(kind);
    let limit = limit.unwrap_or(30);
    tauri::async_runtime::spawn_blocking(move || vcs::log(&app, &root, kind, limit))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

#[tauri::command]
pub async fn vcs_tags(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: Option<String>,
) -> Result<Vec<String>, String> {
    let root = vault_root(&state)?;
    let kind = parse_kind(kind);
    tauri::async_runtime::spawn_blocking(move || vcs::tags(&app, &root, kind))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

// ---------------- Maven ----------------

/// B4 离线模块树：沿 <modules> 递归解析 pom.xml，不依赖 mvn，可离线秒开。
/// 路径守卫与 vault_root 一致（不允许越界工作区），这里只读不写，故无需备份。
#[tauri::command]
pub async fn maven_module_tree(
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<maven::pom::ModuleNode>, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || maven::pom::scan_module_tree(&root))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

/// B4 离线模块依赖明细：模块内 <dependencies>（版本可由 dependencyManagement 补全）
#[tauri::command]
pub async fn maven_module_deps(
    _app: AppHandle,
    state: State<'_, AppState>,
    module: Option<String>,
) -> Result<Vec<maven::pom::PomDep>, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let rel = module.unwrap_or_default();
        // 路径逃逸防护：module 必须是 root 之下的相对路径
        if rel.contains("..") || PathBuf::from(&rel).is_absolute() {
            return Vec::new();
        }
        maven::pom::module_dependencies(&root, &rel)
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))
}

/// 模块依赖树（module = project_model 模块相对路径，空/None = 项目根）
#[tauri::command]
pub async fn maven_dep_tree(
    app: AppHandle,
    state: State<'_, AppState>,
    module: Option<String>,
) -> Result<Vec<maven::ModuleTree>, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || maven::dependency_tree(&app, &root, module.as_deref()))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

/// 一键排除：在所选模块（默认项目根）的 pom.xml 中给直接依赖插入 <exclusion>（先备份 pom.xml.bak）
#[tauri::command]
pub async fn maven_apply_exclusion(
    _app: AppHandle,
    state: State<'_, AppState>,
    module: Option<String>,
    dep_group: String,
    dep_artifact: String,
    excl_group: String,
    excl_artifact: String,
) -> Result<String, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dir = maven::module_dir(&root, module.as_deref())?;
        let pom_path = dir.join("pom.xml");
        if !pom_path.exists() {
            let where_desc = match module.as_deref().map(str::trim) {
                Some(m) if !m.is_empty() => format!("模块 {} 目录", m),
                _ => "项目根".to_string(),
            };
            return Err(format!("{}没有 pom.xml（请确认模块选择后重试）", where_desc));
        }
        let pom = std::fs::read_to_string(&pom_path).map_err(|e| format!("读取 pom.xml 失败：{}", e))?;
        let updated =
            maven::apply_exclusion(&pom, &dep_group, &dep_artifact, &excl_group, &excl_artifact)?;
        // 备份后写入（备份失败不阻断主流程，但必须告警而不是静默吞掉）
        if let Err(e) = std::fs::write(dir.join("pom.xml.bak"), &pom) {
            eprintln!("[tools] pom.xml.bak 备份失败：{}", e);
        }
        std::fs::write(&pom_path, &updated).map_err(|e| format!("写入 pom.xml 失败：{}", e))?;
        Ok(updated)
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}

/// 工具链重新探测：清掉环境缓存（设置 → 工具链 的"重新探测"按钮；纯内存操作）
#[tauri::command]
pub fn toolchain_refresh() -> Result<(), String> {
    crate::toolchain::invalidate();
    Ok(())
}

/// B7-B 档：探测本地模型端点（设置 → AI → 本地模型 的「测试连接」按钮）。
/// 纯网络探测，base 未给时回落已保存的设置。
#[tauri::command]
pub async fn local_llm_probe(
    app: AppHandle,
    base: Option<String>,
) -> Result<crate::local_llm::LocalLlmProbe, String> {
    let settings = config::load_settings(&app);
    let url = base
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| settings.local_llm_base.clone());
    tauri::async_runtime::spawn_blocking(move || crate::local_llm::probe(&url))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}

// ---------------- 增量包 ----------------

#[derive(serde::Serialize)]
pub struct PkgBaselines {
    pub tags: Vec<String>,
    pub commits: Vec<vcs::CommitInfo>,
}

#[tauri::command]
pub async fn pkg_baselines(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PkgBaselines, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(PkgBaselines {
            // 基线选择：git 取 tag / 最近提交；svn 取最近版本号（无仓库时给空列表）
            tags: vcs::tags(&app, &root, None).unwrap_or_default(),
            commits: vcs::log(&app, &root, None, 30).unwrap_or_default(),
        })
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?
}

#[tauri::command]
pub async fn pkg_build(
    app: AppHandle,
    state: State<'_, AppState>,
    baseline: String,
    template: String,
) -> Result<pkg::PkgResult, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || pkg::build(&app, &root, &baseline, &template))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))?
}

#[tauri::command]
pub async fn pkg_history(
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<pkg::PkgHistoryItem>, String> {
    let root = vault_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || pkg::history(&root))
        .await
        .map_err(|e| format!("后台任务异常：{}", e))
}
