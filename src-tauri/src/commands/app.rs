use serde::Serialize;
use tauri::AppHandle;

/// 应用基础信息
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub app_name: &'static str,
    pub version: &'static str,
    pub platform: &'static str,
}

/// 返回应用基础信息（版本号来自 Cargo.toml）
#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        app_name: "云析",
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
    }
}

/// 设置/关闭开机自启（OS 注册表 Run 键 / LaunchAgent）
#[tauri::command]
pub fn set_auto_launch(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let res = if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() };
    res.map_err(|e| e.to_string())
}
