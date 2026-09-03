use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppResult;
use crate::models::Settings;
use crate::state::AppState;

/// 读取设置（settings.json）
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    app.state::<AppState>().load_settings()
}

/// 更新设置（持久化 + 同步 aria2 限速/并发 + 开机自启）
#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> AppResult<()> {
    use tauri_plugin_autostart::ManagerExt;

    let state = app.state::<AppState>();
    let prev = state.load_settings();
    state.save_settings(&settings)?;
    crate::aria2::apply_settings(&app, &settings).await;
    // 通知前端设置已变更（导航胶囊「搜索」显隐、剪贴板开关等立即生效）
    let _ = app.emit("settings:updated", &settings);
    // 开机自启状态与操作系统对齐（仅在值变化时写，避免每次保存都触发注册表写入）
    if prev.auto_launch != settings.auto_launch {
        let res = if settings.auto_launch {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(e) = res {
            state.log(crate::logger::ERROR, "app", "autostart", "设置开机自启失败", &e.to_string());
        }
    }
    Ok(())
}

/// 测试百度网盘第三方加速通道（连通性及解析码校验）
#[tauri::command]
pub async fn test_baidu_speed_service(
    app: AppHandle,
    base_url: Option<String>,
    password: Option<String>,
) -> AppResult<crate::api::baidaccel::AccelCheckResult> {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let base = base_url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| crate::api::baidaccel::base_url_of(&settings));
    let pwd = password
        .map(|p| p.trim().to_string())
        .unwrap_or_else(|| settings.baidu_speed_password.trim().to_string());

    let res = crate::api::baidaccel::check_service(&state.http, &base, &pwd).await;
    Ok(res)
}
