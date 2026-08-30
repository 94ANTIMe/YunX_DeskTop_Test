use tauri::{AppHandle, Manager};

use crate::error::AppResult;
use crate::models::Settings;
use crate::state::AppState;

/// 读取设置（settings.json）
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    app.state::<AppState>().load_settings()
}

/// 更新设置（持久化 + 同步 aria2 限速/并发）
#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> AppResult<()> {
    let state = app.state::<AppState>();
    state.save_settings(&settings)?;
    crate::aria2::apply_settings(&app, &settings).await;
    Ok(())
}
