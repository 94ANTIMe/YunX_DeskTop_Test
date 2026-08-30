//! 应用日志（app_log 表持久化；收集/取链/下载/登录全链路埋点）。
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;

/// 日志级别
pub const INFO: &str = "info";
pub const SUCCESS: &str = "success";
pub const ERROR: &str = "error";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRow {
    pub id: i64,
    pub time: i64,
    pub level: String,
    pub platform: String,
    pub action: String,
    pub message: String,
    pub detail: String,
}

/// 写入一条日志（best-effort：失败由调用方忽略）
pub fn add(
    conn: &Connection,
    level: &str,
    platform: &str,
    action: &str,
    message: &str,
    detail: &str,
) -> AppResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO app_log (time, level, platform, action, message, detail) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![now, level, platform, action, message, detail],
    )?;
    // 容量控制：保留最近 2000 条
    conn.execute(
        "DELETE FROM app_log WHERE id < (SELECT MIN(id) FROM (SELECT id FROM app_log ORDER BY id DESC LIMIT 2000))",
        [],
    )?;
    Ok(())
}

/// 查询日志（level 筛选：None = 全部）
pub fn list(conn: &Connection, level: Option<&str>, limit: i64) -> AppResult<Vec<LogRow>> {
    let rows: Vec<LogRow> = match level {
        Some(lv) if !lv.is_empty() => {
            let mut stmt = conn.prepare(
                "SELECT id, time, level, platform, action, message, detail \
                 FROM app_log WHERE level = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![lv, limit], map_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
        _ => {
            let mut stmt = conn.prepare(
                "SELECT id, time, level, platform, action, message, detail \
                 FROM app_log ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], map_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
    };
    Ok(rows)
}

/// 清空日志
pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM app_log", [])?;
    Ok(())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogRow> {
    Ok(LogRow {
        id: row.get(0)?,
        time: row.get(1)?,
        level: row.get(2)?,
        platform: row.get(3)?,
        action: row.get(4)?,
        message: row.get(5)?,
        detail: row.get(6)?,
    })
}
