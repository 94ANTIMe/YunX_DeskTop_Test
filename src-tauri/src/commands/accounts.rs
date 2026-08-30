use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::login;
use crate::models::{AccountSummary, Platform};
use crate::state::AppState;

/// 列出 6 平台账号状态
#[tauri::command]
pub fn list_accounts(app: AppHandle) -> AppResult<Vec<AccountSummary>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    crate::db::accounts::list_summaries(&conn)
}

/// 登出平台账号（同时清空该平台登录窗口的 WebView2 Cookie，避免自动回登）
#[tauri::command]
pub fn logout(app: AppHandle, platform: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    crate::db::accounts::delete(&conn, platform)?;
    // 清空登录专用 WebView2 数据目录（防旧账号会话残留自动回登）
    crate::login::clear_login_profile(&app, platform);
    // 迅雷登出同时清运行时
    if platform == Platform::Xunlei {
        if let Ok(mut rt) = state.xunlei.lock() {
            rt.access_token.clear();
            rt.refresh_token.clear();
            rt.user_id.clear();
            rt.captcha_token.clear();
        }
    }
    Ok(())
}

/// 打开 WebView 登录窗口（夸克 / UC / 百度 / 139）
/// ⚠️ 必须为 async 命令：sync 命令在主线程直接 build() 会因 WebView2 异步初始化
/// 无法泵消息而死锁（窗口空白且无法关闭——已实测复现）；async 命令在异步线程发起，
/// 由 Tauri 调度到空闲主线程完成创建。
#[tauri::command]
pub async fn web_login_start(app: AppHandle, platform: String) -> AppResult<()> {
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    login::web_login_start(&app, platform)
}

/// 取消网页登录
#[tauri::command]
pub async fn web_login_cancel(app: AppHandle, platform: String) -> AppResult<()> {
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    login::web_login_cancel(&app, platform)
}

/// 迅雷账号密码登录（可能触发短信验证）
#[tauri::command]
pub async fn xunlei_login(app: AppHandle, username: String, password: String) -> AppResult<crate::api::xunlei::LoginStep> {
    login::xunlei_login(&app, &username, &password).await
}

/// 迅雷短信验证码登录
#[tauri::command]
pub async fn xunlei_sms_login(
    app: AppHandle,
    username: String,
    sms_code: String,
    credit_key: String,
    sms_token: String,
) -> AppResult<crate::api::xunlei::LoginStep> {
    login::xunlei_sms_login(&app, &username, &sms_code, &credit_key, &sms_token).await
}

/// 123 云盘账号密码登录
#[tauri::command]
pub async fn pan123_login(app: AppHandle, account: String, password: String) -> AppResult<String> {
    login::pan123_login(&app, &account, &password).await
}
