mod api;
mod aria2;
mod clipboard;
mod commands;
mod crypto;
mod db;
mod error;
mod logger;
mod login;
mod models;
mod parser;
mod resolve;
mod state;
mod tray;
mod update;

#[cfg(test)]
mod live_tests;

use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // 应用数据目录（%APPDATA%\com.yunx.desktop）初始化 SQLite + 设置 + 迅雷指纹
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState::new(&data_dir)?);

            // 系统托盘（常驻后台入口）
            let _ = tray::build(app.handle());

            // 启动 aria2 下载引擎（sidecar + RPC 轮询循环）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                aria2::start(handle).await;
            });

            // 剪贴板监听（分享链接自动提示解析，设置开关控制）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                clipboard::spawn(handle).await;
            });

            // 最小化 → 托盘监测（tauri 2 无 Minimized 事件，轮询 is_minimized 实现）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut was_min = false;
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let Some(window) = handle.get_webview_window("main") else { break };
                    let Ok(min) = window.is_minimized() else { continue };
                    if min && !was_min {
                        if handle.state::<AppState>().load_settings().minimize_to_tray {
                            let _ = window.hide();
                        }
                        was_min = true;
                    } else if !min {
                        was_min = false;
                    }
                }
            });

            // 开机自启状态与设置对齐（幂等）
            let settings = app.state::<AppState>().load_settings();
            if settings.auto_launch {
                use tauri_plugin_autostart::ManagerExt;
                let _ = app.autolaunch().enable();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::app::set_auto_launch,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::accounts::list_accounts,
            commands::accounts::list_account_rows,
            commands::accounts::switch_account,
            commands::accounts::logout,
            commands::accounts::web_login_start,
            commands::accounts::web_login_cancel,
            commands::accounts::xunlei_login,
            commands::accounts::xunlei_sms_login,
            commands::accounts::pan123_login,
            commands::network::test_proxy,
            commands::resolve::parse_share_link,
            commands::resolve::resolve_share,
            commands::resolve::list_share_files,
            commands::resolve::collect_folder_files,
            commands::resolve::get_download_link,
            commands::resolve::validate_session,
            commands::resolve::list_logs,
            commands::resolve::clear_logs,
            commands::search::pansou_search,
            commands::search::pansou_ping,
            commands::download::enqueue_download,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::pause_all_downloads,
            commands::download::resume_all_downloads,
            commands::download::remove_download_task,
            commands::download::list_download_tasks,
            commands::download::clear_download_tasks,
            commands::download::download_detail,
            commands::bookmark::list_bookmarks,
            commands::bookmark::add_bookmark,
            commands::bookmark::remove_bookmark,
            commands::history::list_resolve_history,
            commands::history::delete_resolve_history,
            commands::history::clear_resolve_history,
            commands::update::check_update,
            commands::update::download_update,
            commands::update::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
