use serde::Serialize;

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
