use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::login;
use crate::models::{AccountRow, AccountSummary, Platform};
use crate::state::AppState;

/// 当前各平台选中账号 key 映射（settings.active_account_keys）
fn active_keys_map(app: &AppHandle) -> std::collections::BTreeMap<String, String> {
    app.state::<AppState>().load_settings().active_account_keys
}

/// 列出 6 平台账号状态
#[tauri::command]
pub fn list_accounts(app: AppHandle) -> AppResult<Vec<AccountSummary>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    crate::db::accounts::list_summaries(&conn, &active_keys_map(&app))
}

/// 平台账号列表（多账号切换下拉；active = 当前选中）
#[tauri::command]
pub fn list_account_rows(app: AppHandle, platform: String) -> AppResult<Vec<AccountRow>> {
    let state = app.state::<AppState>();
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let active = state.active_account_key(&platform);
    crate::db::accounts::list_rows(&conn, platform, &active)
}

/// 切换平台当前选中账号（解析/下载走新账号）
#[tauri::command]
pub fn switch_account(app: AppHandle, platform: String, key: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let exists = crate::db::accounts::list_rows(&conn, platform, "")?
        .iter()
        .any(|r| r.key == key);
    if !exists {
        return Err(AppError::Api("该账号不存在或已登出".into()));
    }
    state.set_active_account(&platform, &key);
    Ok(())
}

/// 登出平台账号（默认登出当前选中账号；key 指定时可登出任意行）
/// 同时清空该平台登录窗口的 WebView2 Cookie，避免自动回登
#[tauri::command]
pub fn logout(app: AppHandle, platform: String, key: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    let target = if key.is_empty() { state.active_account_key(&platform) } else { key };

    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    crate::db::accounts::delete(&conn, platform, &target)?;
    drop(conn);

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
    // 登出的是当前选中账号时移除选中记录（回退最近一条行的兜底逻辑）
    if target == state.active_account_key(&platform) {
        let mut settings = state.load_settings();
        settings.active_account_keys.remove(platform.key());
        let _ = state.save_settings(&settings);
    }
    Ok(())
}

// ---------- 登录命令（薄包装：参数 String 转换 + 调用 login 模块） ----------

/// 打开 WebView 登录窗口（夸克 / UC / 百度 / 139）
/// ⚠️ 必须为 async 命令：sync 命令在主线程直接 build() 会因 WebView2 异步初始化
/// 无法泵消息而死锁（窗口空白且无法关闭——已实测复现）；async 命令在异步线程发起，
/// 由 Tauri 调度到空闲主线程完成创建。
#[tauri::command]
pub async fn web_login_start(app: AppHandle, platform: String) -> AppResult<()> {
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    login::web_login_start(&app, platform)
}

/// 取消网页登录（关闭窗口）
#[tauri::command]
pub async fn web_login_cancel(app: AppHandle, platform: String) -> AppResult<()> {
    let platform = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    login::web_login_cancel(&app, platform)
}

/// 迅雷账号密码登录（可能触发短信验证步骤）
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

/// 123 账号密码登录
#[tauri::command]
pub async fn pan123_login(app: AppHandle, account: String, password: String) -> AppResult<String> {
    login::pan123_login(&app, &account, &password).await
}

/// 列出个人网盘某目录下的文件与文件夹
#[tauri::command]
pub async fn list_personal_files(
    app: AppHandle,
    platform: String,
    dir_id: Option<String>,
) -> AppResult<Vec<crate::models::ShareFile>> {
    let state = app.state::<AppState>();
    let plat = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    crate::api::pan_files::list_personal_files(&state, plat, dir_id.as_deref().unwrap_or("")).await
}

/// 获取个人网盘文件下载直链并入队
#[tauri::command]
pub async fn get_personal_download_link(
    app: AppHandle,
    platform: String,
    file: crate::models::ShareFile,
) -> AppResult<crate::models::DownloadLink> {
    let state = app.state::<AppState>();
    let plat = Platform::from_key(&platform).ok_or_else(|| AppError::Api("未知平台".into()))?;
    crate::api::pan_files::get_personal_download_link(&state, plat, &file).await
}