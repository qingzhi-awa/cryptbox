// 托盘模块：Windows 系统托盘（关闭窗口隐藏到托盘，菜单「打开/退出」）。
// 非 Windows 平台暂无托盘（关闭窗口直接退出，与旧版行为一致）。
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

/// 显示并激活主窗口。
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        // 通知前端窗口已重新显示，让其检查并同步锁定状态。
        let _ = window.emit("window-shown", ());
    }
}

/// Windows 下启动系统托盘 + 关闭窗口隐藏到托盘。
#[cfg(target_os = "windows")]
pub fn setup_tray(app: &AppHandle) {
    // 关闭窗口时隐藏到托盘（阻止默认退出），并锁定（清除主密钥）。
    if let Some(window) = app.get_webview_window("main") {
        let w = window.clone();
        let app_handle = app.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = w.hide();
                if let Some(state) = app_handle.try_state::<crate::app::AppState>() {
                    *state.key.lock().unwrap() = None;
                }
            }
        });
    }

    // 托盘菜单：打开 / 退出
    let open_item = MenuItem::with_id(app, "open", "打开 密匣", true, None::<&str>).unwrap();
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).unwrap();
    let menu = Menu::with_items(app, &[&open_item, &quit_item]).unwrap();

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("CryPtBox 密匣 - 后台常驻")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let _ = builder.build(app);
}

/// 非 Windows 平台暂无系统托盘。
#[cfg(not(target_os = "windows"))]
pub fn setup_tray(_app: &AppHandle) {}
