use tauri::AppHandle;

use crate::aria2;
use crate::error::AppResult;
use crate::models::DownloadTaskView;

/// 入队下载任务（直链 + 请求头 → aria2）
#[tauri::command]
pub async fn enqueue_download(
    app: AppHandle,
    url: String,
    file_name: String,
    headers: Vec<(String, String)>,
    platform: String,
    cleanup_id: Option<String>,
) -> AppResult<i64> {
    aria2::enqueue(
        &app,
        &url,
        &file_name,
        &headers,
        &platform,
        cleanup_id.as_deref().unwrap_or(""),
        false,
    )
    .await
}

/// 暂停下载
#[tauri::command]
pub async fn pause_download(app: AppHandle, id: i64) -> AppResult<()> {
    aria2::pause(&app, id).await
}

/// 恢复下载
#[tauri::command]
pub async fn resume_download(app: AppHandle, id: i64) -> AppResult<()> {
    aria2::resume(&app, id).await
}

/// 删除下载任务（delete_local 同时删本地文件）
#[tauri::command]
pub async fn remove_download_task(app: AppHandle, id: i64, delete_local: bool) -> AppResult<()> {
    aria2::remove(&app, id, delete_local).await
}

/// 全量任务列表（含已完成/失败）
#[tauri::command]
pub fn list_download_tasks(app: AppHandle) -> AppResult<Vec<DownloadTaskView>> {
    aria2::list_tasks(&app)
}

/// 一键清空全部下载任务记录（强制移除 + DB 清空）
#[tauri::command]
pub async fn clear_download_tasks(app: AppHandle) -> AppResult<()> {
    aria2::clear_all(&app).await
}
