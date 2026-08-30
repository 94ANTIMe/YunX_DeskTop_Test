use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::update::{self, UpdateInfo};

/// 单飞锁：同一时刻只允许一个更新动作（检查/下载/安装）进行。
fn try_acquire(state: &State<'_, AppState>) -> AppResult<()> {
    let mut lock = state.updating.lock().map_err(|_| AppError::Lock)?;
    if *lock {
        return Err(AppError::Api("已有更新操作进行中，请稍候".into()));
    }
    *lock = true;
    Ok(())
}

fn release(state: &State<'_, AppState>) {
    if let Ok(mut lock) = state.updating.lock() {
        *lock = false;
    }
}

/// 在线检查更新（GitHub Releases）
#[tauri::command]
pub async fn check_update(app: AppHandle) -> AppResult<UpdateInfo> {
    Ok(update::check(&app).await?)
}

/// 下载最新安装包（带进度事件 `update:progress`）
#[tauri::command]
pub async fn download_update(app: AppHandle) -> AppResult<String> {
    let state = app.state::<AppState>();
    try_acquire(&state)?;
    let result = async {
        let info = update::check(&app).await?;
        if !info.has_update || info.download_url.is_empty() {
            return Err(AppError::Api("没有可下载的更新".into()));
        }
        update::download(&app, &info.download_url, &info.gitcode_download_url).await
    }
    .await;
    release(&state);
    result
}

/// 静默安装已下载的安装包（装完自动退出并重启新版本）
#[tauri::command]
pub async fn install_update(app: AppHandle, path: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    try_acquire(&state)?;
    // 安装会退出进程，不再释放锁
    update::install(&app, &path)
}