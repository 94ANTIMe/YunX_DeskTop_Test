use rusqlite::params;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::models::ResolveHistoryRow;
use crate::state::AppState;

/// 解析成功时记录一条历史（内部调用，非 command）
pub fn record_resolve(app: &AppHandle, link: &str, platform: &str, title: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let state = app.state::<AppState>();
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = conn.execute(
        "INSERT INTO resolve_history (link, title, platform, create_time) VALUES (?1, ?2, ?3, ?4)",
        params![link, title, platform, now],
    );
    // 只保留最近 50 条，避免无限增长
    let _ = conn.execute(
        "DELETE FROM resolve_history WHERE id NOT IN \
         (SELECT id FROM resolve_history ORDER BY create_time DESC LIMIT 50)",
        [],
    );
}

/// 列出解析历史
#[tauri::command]
pub fn list_resolve_history(app: AppHandle) -> AppResult<Vec<ResolveHistoryRow>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, link, title, platform, create_time \
         FROM resolve_history ORDER BY create_time DESC LIMIT 50",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ResolveHistoryRow {
                id: row.get(0)?,
                link: row.get(1)?,
                title: row.get(2)?,
                platform: row.get(3)?,
                create_time: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 删除单条解析历史
#[tauri::command]
pub fn delete_resolve_history(app: AppHandle, id: i64) -> AppResult<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM resolve_history WHERE id = ?1", params![id])?;
    Ok(())
}

/// 清空全部解析历史
#[tauri::command]
pub fn clear_resolve_history(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM resolve_history", [])?;
    Ok(())
}
