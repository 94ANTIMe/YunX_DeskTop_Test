mod api;
mod aria2;
mod commands;
mod db;
mod error;
mod logger;
mod login;
mod models;
mod parser;
mod resolve;
mod state;

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
        .setup(|app| {
            // 应用数据目录（%APPDATA%\com.yunx.desktop）初始化 SQLite + 设置 + 迅雷指纹
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState::new(&data_dir)?);

            // 启动 aria2 下载引擎（sidecar + RPC 轮询循环）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                aria2::start(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::accounts::list_accounts,
            commands::accounts::logout,
            commands::accounts::web_login_start,
            commands::accounts::web_login_cancel,
            commands::accounts::xunlei_login,
            commands::accounts::xunlei_sms_login,
            commands::accounts::pan123_login,
            commands::resolve::parse_share_link,
            commands::resolve::resolve_share,
            commands::resolve::list_share_files,
            commands::resolve::collect_folder_files,
            commands::resolve::get_download_link,
            commands::resolve::validate_session,
            commands::resolve::list_logs,
            commands::resolve::clear_logs,
            commands::search::pansou_search,
            commands::download::enqueue_download,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::remove_download_task,
            commands::download::list_download_tasks,
            commands::download::clear_download_tasks,
            commands::bookmark::list_bookmarks,
            commands::bookmark::add_bookmark,
            commands::bookmark::remove_bookmark,
            commands::history::list_resolve_history,
            commands::history::delete_resolve_history,
            commands::history::clear_resolve_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
