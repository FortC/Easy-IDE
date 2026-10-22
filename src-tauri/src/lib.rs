//! EasyIDE 入口：命令注册 + 自定义资源协议。
//! emdasset://vault/<percent-encoded 相对路径> —— 只允许读当前 vault 内文件。

mod code_index;
mod commands;
mod difflib;
mod config;
mod docs_hub;
mod vcs;
mod maven;
mod pkg;
mod project_model;
mod recipe;
mod toolchain;
mod tray;
mod index;
mod local_llm;
mod mcp;
mod state;

use std::path::{Component, Path, PathBuf};
use tauri::Manager;

pub fn run() {
    // PF7 冷启动分段埋点：记录 Rust 侧起点，经初始化脚本注入前端，
    // 前端可算出「窗口创建 → Vue 挂载完成」耗时，定位瓶颈在 Rust 还是 JS。
    let rust_start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let rust_start_ms = rust_start.to_string();

    tauri::Builder::default()
        // 单实例：已运行时右键/双击打开文件 → 聚焦窗口并转发路径（必须最先注册）
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            use tauri::Emitter;
            if let Some(w) = app.get_webview_window("main") {
                // 窗口可能被隐藏到托盘，先 show 再还原聚焦
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
            if let Some(file) = args.iter().skip(1).rev().find(|a| {
                let l = a.to_lowercase();
                l.ends_with(".md") || l.ends_with(".markdown") || l.ends_with(".canvas")
            }) {
                let _ = app.emit("os-open-file", file.clone());
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .register_uri_scheme_protocol("emdasset", |ctx, request| {
            serve_asset(ctx.app_handle(), &request.uri().to_string())
        })
        .setup(move |app| {
            // PF7：把 Rust 启动时刻注入前端（相对 epoch 毫秒），
            // 前端 main.ts 读取 __easyideTauriStart 计算 tauri_to_mount 段
            if let Some(w) = app.get_webview_window("main") {
                let s = rust_start_ms.clone();
                let _ = w.eval(&format!(
                    "window.__easyideTauriStart = {} - performance.timeOrigin;",
                    s
                ));
            }
            // 捕获启动参数中的文件路径（右键"用 EasyIDE 打开"）
            let st = app.state::<state::AppState>();
            if let Some(file) = std::env::args().skip(1).rev().find(|a| {
                let l = a.to_lowercase();
                l.ends_with(".md") || l.ends_with(".markdown") || l.ends_with(".canvas")
            }) {
                if let Ok(mut pf) = st.pending_file.lock() {
                    *pf = Some(file);
                }
            }
            // 系统托盘：创建失败只告警，不影响启动（此时关闭按钮走正常退出，见 tray::minimize_to_tray_enabled）
            if let Err(e) = tray::init(app.handle()) {
                eprintln!("[托盘] 创建失败，关闭窗口将直接退出：{}", e);
            }
            Ok(())
        })
        // 关闭请求（标题栏关闭按钮 / Alt+F4）→ 不退出，隐藏到托盘
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::minimize_to_tray_enabled() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault::list_vaults,
            commands::vault::open_vault,
            commands::vault::create_vault,
            commands::vault::close_vault,
            commands::vault::forget_vault,
            commands::fsops::read_text_file,
            commands::fsops::write_text_file,
            commands::fsops::path_exists,
            commands::fsops::list_dir,
            commands::fsops::create_note,
            commands::fsops::create_folder,
            commands::fsops::rename_path,
            commands::fsops::delete_path,
            commands::fsops::save_image,
            commands::fsops::write_external,
            commands::search::search_files,
            commands::search::search_content,
            commands::search::search_by_tag,
            commands::index_cmd::rebuild_index,
            commands::index_cmd::get_all_notes,
            commands::index_cmd::get_tags,
            commands::index_cmd::get_backlinks,
            commands::index_cmd::resolve_link,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_config_dirs,
            commands::settings::get_vault_root,
            commands::mcp_cmd::mcp_test,
            commands::mcp_cmd::mcp_sync,
            commands::settings::snippet_files,
            commands::settings::read_snippet_file,
            commands::settings::create_snippet_file,
            commands::settings::delete_snippet_file,
            commands::os_open::get_pending_file,
            commands::os_open::open_path_from_os,
            commands::os_open::open_as_temp,
            commands::os_open::save_temp_back,
            commands::os_open::register_context_menu,
            commands::os_open::unregister_context_menu,
            commands::os_open::open_default_apps_settings,
            commands::fsops::import_files,
            commands::fsops::read_external_binary,
            commands::fsops::list_all_files_with_mtime,
            commands::fsops::duplicate_file,
            commands::fsops::delete_files,
            commands::ai::ai_chat,
            commands::code_index::code_symbol_search,
            commands::code_index::code_file_symbols,
            commands::code_index::code_file_search,
            commands::code_index::code_text_search,
            commands::docs::docs_scan,
            commands::docs::docs_move,
            commands::docs::docs_ignore,
            commands::docs::docs_unignore,
            commands::docs::docs_zip,
            commands::docs::docs_generate_agents,
            commands::docs::docs_update_settings,
            commands::docs::docs_set_mapping,
            commands::docs::docs_ai_classify,
            commands::docs::docs_quick_op,
            commands::agent::agent_chat_stream,
            commands::agent::agent_cancel,
            commands::agent::agent_run_command,
            commands::agent::agent_kill_command,
            commands::agent::agent_list_running,
            commands::recipe_cmd::saved_actions_list,
            commands::recipe_cmd::saved_actions_save,
            commands::recipe_cmd::saved_action_run,
            commands::build_cmd::build_detect,
            commands::build_cmd::build_goal_run,
            commands::agent::agent_sessions_list,
            commands::agent::agent_session_load,
            commands::agent::agent_session_save,
            commands::agent::agent_session_delete,
            commands::agent::toolchain_detect,
            commands::tools::vcs_status,
            commands::tools::vcs_diff,
            commands::tools::vcs_log,
            commands::tools::vcs_tags,
            commands::tools::maven_dep_tree,
            commands::tools::maven_module_tree,
            commands::tools::maven_module_deps,
            commands::tools::maven_apply_exclusion,
            commands::tools::toolchain_refresh,
            commands::tools::local_llm_probe,
            commands::tools::pkg_baselines,
            commands::tools::pkg_build,
            commands::tools::pkg_history,
            commands::agent::diff_preview,
            commands::recipe_cmd::project_detect,
            commands::recipe_cmd::recipe_list,
            commands::recipe_cmd::recipe_get,
            commands::recipe_cmd::recipe_save,
            commands::recipe_cmd::recipe_delete,
            commands::recipe_cmd::recipe_check,
            commands::recipe_cmd::recipe_run,
            commands::recipe_cmd::recipe_trust,
            commands::recipe_cmd::recipe_record_usage,
            commands::terminal::terminal_create,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_kill,
            commands::terminal::terminal_alive,
        ])
        .build(tauri::generate_context!())
        .expect("EasyIDE 启动失败")
        .run(|_app, event| {
            // 退出清理钩子（ExitRequested 与 Exit 都幂等，重复调用安全）：
            // 杀掉所有已登记子进程树 + 终止终端 ConPTY 会话，杜绝僵尸进程
            match event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    commands::agent::kill_all_tracked();
                    commands::terminal::kill_session();
                }
                _ => {}
            }
        });
}

fn serve_asset(app: &tauri::AppHandle, uri: &str) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, StatusCode};
    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap()
    };

    // emdasset://vault/<encoded-rel-path>
    let rest = match uri.strip_prefix("emdasset://vault/") {
        Some(r) => r,
        None => return not_found(),
    };
    let rel = percent_encoding::percent_decode_str(rest)
        .decode_utf8_lossy()
        .to_string();

    // 相对路径安全校验（拒绝绝对路径与 .. 穿越）
    let rel_path = Path::new(&rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return not_found();
    }

    let state = app.state::<state::AppState>();
    let guard = match state.vault.lock() {
        Ok(g) => g,
        Err(_) => return not_found(),
    };
    let vc = match guard.as_ref() {
        Some(vc) => vc,
        None => return not_found(),
    };
    let full: PathBuf = vc.root.join(rel_path);
    if !full.starts_with(&vc.root) {
        return not_found();
    }

    // 大小写不敏感二次校验（Windows 路径 join 后仍需确认未越界）
    let root_lower = vc.root.to_string_lossy().to_lowercase();
    let full_lower = full.to_string_lossy().to_lowercase();
    if !full_lower.starts_with(&root_lower) {
        return not_found();
    }

    let bytes = match std::fs::read(&full) {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    let mime = mime_of(full.extension().and_then(|e| e.to_str()).unwrap_or(""));
    Response::builder()
        .header("Content-Type", mime)
        .header("Cache-Control", "max-age=300")
        .body(bytes)
        .unwrap()
}

fn mime_of(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
