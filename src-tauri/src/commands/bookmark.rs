use rusqlite::params;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::models::BookmarkRow;
use crate::state::AppState;

/// 列出收藏链接
#[tauri::command]
pub fn list_bookmarks(app: AppHandle) -> AppResult<Vec<BookmarkRow>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, link, title, platform, pwd, category, create_time \
         FROM bookmark ORDER BY create_time DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BookmarkRow {
                id: row.get(0)?,
                link: row.get(1)?,
                title: row.get(2)?,
                platform: row.get(3)?,
                pwd: row.get(4)?,
                category: row.get(5)?,
                create_time: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 收藏链接（自动识别平台；重复链接更新）
#[tauri::command]
pub fn add_bookmark(app: AppHandle, link: String, title: String, pwd: String) -> AppResult<i64> {
    let state = app.state::<AppState>();
    let platform = crate::parser::parse(&link)
        .map(|p| p.platform)
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    // 同链接已收藏 → 更新
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM bookmark WHERE link = ?1",
            params![link],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })?;
    if let Some(id) = existing {
        conn.execute(
            "UPDATE bookmark SET title = ?1, platform = ?2, pwd = ?3 WHERE id = ?4",
            params![title, platform, pwd, id],
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO bookmark (link, title, platform, pwd, category, create_time) VALUES (?1, ?2, ?3, ?4, '未分类', ?5)",
        params![link, title, platform, pwd, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 删除收藏
#[tauri::command]
pub fn remove_bookmark(app: AppHandle, id: i64) -> AppResult<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM bookmark WHERE id = ?1", params![id])?;
    Ok(())
}
