use tauri::AppHandle;

use crate::aria2;
use crate::error::AppResult;
use crate::models::{DownloadDetail, DownloadTaskView};

/// 入队下载任务（直链 + 多源镜像 + 请求头 → aria2）
#[tauri::command]
pub async fn enqueue_download(
    app: AppHandle,
    url: String,
    file_name: String,
    headers: Vec<(String, String)>,
    platform: String,
    cleanup_id: Option<String>,
    mirrors: Option<Vec<String>>,
) -> AppResult<i64> {
    aria2::enqueue(
        &app,
        &url,
        &file_name,
        &headers,
        &platform,
        cleanup_id.as_deref().unwrap_or(""),
        false,
        mirrors.unwrap_or_default(),
    )
    .await
}

/// 入队 BT 种子数据
#[tauri::command]
pub async fn enqueue_torrent(
    app: AppHandle,
    torrent_data: Vec<u8>,
    file_name: String,
) -> AppResult<i64> {
    aria2::enqueue_torrent(&app, &torrent_data, &file_name).await
}

/// 入队本地 BT 种子文件路径
#[tauri::command]
pub async fn enqueue_torrent_file(
    app: AppHandle,
    file_path: String,
) -> AppResult<i64> {
    let path = std::path::Path::new(&file_path);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("download.torrent").to_string();
    let bytes = tokio::fs::read(path).await.map_err(|e| crate::error::AppError::Api(format!("读取种子文件失败: {e}")))?;
    aria2::enqueue_torrent(&app, &bytes, &name).await
}

/// 暂停全部（托盘菜单 / 前端）
#[tauri::command]
pub async fn pause_all_downloads(app: AppHandle) -> AppResult<()> {
    aria2::pause_all(&app).await
}

/// 继续全部（托盘菜单 / 前端）
#[tauri::command]
pub async fn resume_all_downloads(app: AppHandle) -> AppResult<()> {
    aria2::resume_all(&app).await
}

/// 单个任务 Dashboard 详情
#[tauri::command]
pub async fn download_detail(app: AppHandle, id: i64) -> AppResult<DownloadDetail> {
    aria2::detail(&app, id).await
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
