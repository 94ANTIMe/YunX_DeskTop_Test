use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::models::{StatsDaily, StatsOverview, StatsPlatform, StatsTotals};
use crate::state::AppState;

/// 下载统计总览：近 N 天逐日聚合 + 全量累计 + 平台分布。
/// 数据来自 download_stat 聚合表（完成/失败时累加），清空任务记录不影响统计。
#[tauri::command]
pub async fn get_download_stats(app: AppHandle, days: Option<u32>) -> AppResult<StatsOverview> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;

    let totals = conn.query_row(
        "SELECT COALESCE(SUM(files), 0), COALESCE(SUM(bytes), 0), COALESCE(SUM(failed), 0) FROM download_stat",
        [],
        |r| {
            Ok(StatsTotals {
                files: r.get(0)?,
                bytes: r.get(1)?,
                failed: r.get(2)?,
            })
        },
    )?;

    let days = days.unwrap_or(30).clamp(1, 365) as i64;
    let since = (chrono::Local::now() - chrono::Duration::days(days - 1))
        .format("%Y-%m-%d")
        .to_string();

    let mut stmt = conn.prepare(
        "SELECT day, SUM(files), SUM(bytes), SUM(failed) \
         FROM download_stat WHERE day >= ?1 GROUP BY day ORDER BY day",
    )?;
    let daily = stmt
        .query_map(rusqlite::params![since], |r| {
            Ok(StatsDaily {
                day: r.get(0)?,
                files: r.get(1)?,
                bytes: r.get(2)?,
                failed: r.get(3)?,
            })
        })?
        .filter_map(Result::ok)
        .collect();

    let mut stmt = conn.prepare(
        "SELECT platform, SUM(files), SUM(bytes), SUM(failed) \
         FROM download_stat GROUP BY platform ORDER BY bytes DESC",
    )?;
    let platforms = stmt
        .query_map([], |r| {
            Ok(StatsPlatform {
                platform: r.get(0)?,
                files: r.get(1)?,
                bytes: r.get(2)?,
                failed: r.get(3)?,
            })
        })?
        .filter_map(Result::ok)
        .collect();

    Ok(StatsOverview { totals, daily, platforms })
}
