//! 系统托盘：最小化常驻后台的入口。
//! 右键菜单：显示主界面 / 暂停全部 / 继续全部 / 退出；图标 tooltip 实时汇总下载速度。
//! 最小化→托盘隐藏由 lib.rs 的轮询监测（is_minimized）按设置控制，这里负责建托盘与更新 tooltip。
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder};
use tauri::{AppHandle, Manager};

/// 托盘唯一 id（供 update_speed / 菜单事件定位）
const TRAY_ID: &str = "yunx-tray";

/// 创建系统托盘（setup 时调用一次）
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
    let pause_all = MenuItemBuilder::with_id("pause_all", "暂停全部").build(app)?;
    let resume_all = MenuItemBuilder::with_id("resume_all", "继续全部").build(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出"))?;

    let menu = MenuBuilder::new(app)
        .items(&[&show, &pause_all, &resume_all, &quit])
        .build()?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("云析 · 常驻后台中")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => restore_main(app),
            "pause_all" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::aria2::pause_all(&app).await;
                });
            }
            "resume_all" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::aria2::resume_all(&app).await;
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 左键点击恢复主界面
            if let tauri::tray::TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                restore_main(tray.app_handle());
            }
        });
    // 托盘图标复用主窗口图标（icon.ico）
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// 恢复/聚焦主窗口（主窗口 label 为 "main"，见 tauri.conf.json windows[0]）
fn restore_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 更新托盘 tooltip 为当前下载速度汇总
pub fn update_speed(app: &AppHandle, active_count: usize, speed: i64) {
    let speed_txt = if speed <= 0 {
        "空闲".to_string()
    } else {
        format!("{:.1} MB/s", speed as f64 / 1_048_576.0)
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("云析 · {active_count} 个任务 · {speed_txt}")));
    }
}
