//! 系统托盘：图标 + 右键菜单（显示主窗口 / 退出），双击左键唤起主窗口。
//! 托盘创建失败只告警不阻断启动；此时关闭窗口走正常退出（见 [`minimize_to_tray_enabled`]），
//! 避免窗口被藏起来却没有任何唤起入口。

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// 托盘是否创建成功。关闭按钮的行为依赖它：托盘不可用时不得把窗口藏起来。
static TRAY_READY: AtomicBool = AtomicBool::new(false);

/// 关闭窗口是否应隐藏到托盘（托盘创建失败时返回 false，走正常关闭退出）
pub fn minimize_to_tray_enabled() -> bool {
    TRAY_READY.load(Ordering::SeqCst)
}

/// 创建托盘图标与右键菜单。失败返回 Err，由调用方降级处理（只告警、不阻断启动）。
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show-window", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit-app", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("easyide-tray")
        .tooltip("EasyIDE")
        .menu(&menu)
        // 菜单只在右键弹出；左键双击留给"显示主窗口"
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show-window" => show_main_window(app),
            // 真正退出：触发 RunEvent::ExitRequested/Exit，退出清理钩子随之执行
            "quit-app" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 双击托盘图标 = 显示主窗口
            if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = event {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    } else {
        eprintln!("[托盘] 未找到应用图标，托盘图标可能显示异常");
    }
    // 托盘由 App 资源表持引用，此处返回值可直接丢弃
    builder.build(app)?;
    TRAY_READY.store(true, Ordering::SeqCst);
    Ok(())
}

/// 显示并聚焦主窗口（托盘菜单与双击图标共用）
fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
