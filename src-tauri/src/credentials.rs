//! 平台凭据读取（原 resolve.rs 中的账号会话读取，ADR-0007 下沉到独立模块）。
//! api 层适配文件只依赖本模块取 Cookie/Token，不再反向依赖 resolve 编排层，
//! 消除 api ↔ resolve 循环依赖。

use crate::db::accounts;
use crate::error::{AppError, AppResult};
use crate::models::Platform;
use crate::state::AppState;

/// 读取指定平台的当前账号凭据（Cookie / 访问令牌）。
/// 账号行不存在或凭据为空时抛 `need_login_msg`；注意它不做内容校验，
/// 「刚完成登录但异步校验尚未落库」的窗口期会走到该错误（提示文案需覆盖此场景）。
pub(crate) fn load_account_cookie(
    state: &AppState,
    platform: Platform,
    need_login_msg: &str,
) -> AppResult<String> {
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let active = state.active_account_key(&platform);
    match accounts::load(&conn, platform, &active)? {
        Some(acc) if !acc.cookie().is_empty() => Ok(acc.cookie().to_string()),
        _ => Err(AppError::Api(need_login_msg.to_string())),
    }
}
