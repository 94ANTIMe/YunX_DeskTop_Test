use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::logger;
use crate::models::{DownloadLink, ParsedShare, ResolveSessionInfo, ShareFile, ShareFilePage};
use crate::state::AppState;

/// 解析分享链接（平台识别 + 提取码）
#[tauri::command]
pub fn parse_share_link(text: String) -> AppResult<ParsedShare> {
    crate::parser::parse(&text)
}

/// 建立解析会话并返回首页文件列表
#[tauri::command]
pub async fn resolve_share(app: AppHandle, text: String) -> AppResult<ResolveSessionInfo> {
    let state = app.state::<AppState>();
    crate::resolve::resolve_share(&state, &text).await
}

/// 列出分享目录文件（目录导航 / 翻页）
#[tauri::command]
pub async fn list_share_files(
    app: AppHandle,
    session_key: String,
    dir_id: String,
    page: Option<i64>,
) -> AppResult<ShareFilePage> {
    let state = app.state::<AppState>();
    let result = crate::resolve::list_share_files(&state, &session_key, &dir_id, page.unwrap_or(1)).await;
    match &result {
        Ok(page_result) => {
            state.log(
                logger::INFO,
                "",
                "list",
                &format!("读取目录 {} 个条目", page_result.files.len()),
                &format!("dir={dir_id} page={} more={}", page.unwrap_or(1), page_result.has_more),
            );
        }
        Err(e) => {
            state.log(logger::ERROR, "", "list", &format!("读取目录失败：{e}"), &format!("dir={dir_id}"));
        }
    }
    result
}

/// 递归收集目录下全部文件（文件夹下载用）
#[tauri::command]
pub async fn collect_folder_files(
    app: AppHandle,
    session_key: String,
    dir_id: String,
) -> AppResult<Vec<ShareFile>> {
    let state = app.state::<AppState>();
    state.log(logger::INFO, "", "collect", "开始收集文件夹文件", &format!("dir={dir_id}"));
    let result = crate::resolve::collect_folder_files(&state, &session_key, &dir_id).await;
    match &result {
        Ok(files) => {
            state.log(
                logger::SUCCESS,
                "",
                "collect",
                &format!("收集完成：共 {} 个文件", files.len()),
                &files.iter().map(|f| f.fname.as_str()).take(20).collect::<Vec<_>>().join("、"),
            );
        }
        Err(e) => {
            state.log(logger::ERROR, "", "collect", &format!("收集失败：{e}"), &format!("dir={dir_id}"));
        }
    }
    result
}

/// 获取文件下载直链（必要时转存临时目录）
#[tauri::command]
pub async fn get_download_link(
    app: AppHandle,
    session_key: String,
    file: ShareFile,
) -> AppResult<DownloadLink> {
    let state = app.state::<AppState>();
    let result = crate::resolve::get_download_link(&state, &session_key, &file).await;
    match &result {
        Ok(link) => {
            state.log(
                logger::SUCCESS,
                &link.platform,
                "link",
                &format!("取链成功：{}", link.filename),
                &format!("size={} url={}", link.size, &link.url[..link.url.len().min(120)]),
            );
        }
        Err(e) => {
            state.log(logger::ERROR, "", "link", &format!("取链失败：{}", file.fname), &e.to_string());
        }
    }
    result
}

/// 校验会话是否有效（前端恢复状态用）
#[tauri::command]
pub fn validate_session(app: AppHandle, session_key: String) -> AppResult<bool> {
    let state = app.state::<AppState>();
    let sessions = state.sessions.lock().map_err(|_| AppError::Lock)?;
    Ok(sessions.contains_key(&session_key))
}

// ---------- 日志 ----------

/// 查询日志（level: "success" / "error" / "info" / 空 = 全部）
#[tauri::command]
pub fn list_logs(app: AppHandle, level: Option<String>, limit: Option<i64>) -> AppResult<Vec<logger::LogRow>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    logger::list(&conn, level.as_deref(), limit.unwrap_or(500))
}

/// 清空日志
#[tauri::command]
pub fn clear_logs(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    logger::clear(&conn)
}
