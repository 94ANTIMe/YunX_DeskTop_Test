pub mod accounts;
pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

/// 在指定目录打开（或创建）yunx.db 并建表
pub fn init(data_dir: &Path) -> AppResult<Connection> {
    std::fs::create_dir_all(data_dir)?;
    let conn = Connection::open(data_dir.join("yunx.db"))?;
    // WAL + NORMAL：读写不再互斥、fsync 频率大幅下降；busy_timeout 缓解多路径争抢连接时的立即失败
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");
    let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
    conn.execute_batch(schema::SCHEMA)?;
    // v0.1 骨架库 → v0.2 增加 gid 列（列已存在时忽略）
    let _ = conn.execute(
        "ALTER TABLE download_task ADD COLUMN gid TEXT NOT NULL DEFAULT ''",
        [],
    );
    // v0.4.6 增加 finish_time 列（完成/失败时间戳，事件窗口与统计用）
    let _ = conn.execute(
        "ALTER TABLE download_task ADD COLUMN finish_time INTEGER NOT NULL DEFAULT 0",
        [],
    );
    // v0.5.1 持久化多镜像，失败重试和重启恢复仍沿用原下载源。
    let _ = conn.execute(
        "ALTER TABLE download_task ADD COLUMN mirrors_json TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    // v0.2 迅雷指纹持久化（文件方式，见 xunlei 模块）无表变更
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::init;

    #[test]
    fn legacy_download_table_gets_mirrors_column_without_data_loss() {
        let dir = std::env::temp_dir().join(format!("yunx-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        {
            let legacy = rusqlite::Connection::open(dir.join("yunx.db")).unwrap();
            legacy.execute_batch(
                "CREATE TABLE download_task (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    total_size INTEGER NOT NULL DEFAULT 0,
                    downloaded_size INTEGER NOT NULL DEFAULT 0,
                    status INTEGER NOT NULL DEFAULT 0,
                    error_msg TEXT NOT NULL DEFAULT '',
                    save_path TEXT NOT NULL DEFAULT '',
                    request_headers_json TEXT NOT NULL DEFAULT '{}',
                    chunk_count INTEGER NOT NULL DEFAULT 0,
                    planned_total_size INTEGER NOT NULL DEFAULT 0,
                    cleanup_id TEXT NOT NULL DEFAULT '',
                    platform TEXT NOT NULL DEFAULT '',
                    avg_speed INTEGER NOT NULL DEFAULT 0,
                    create_time INTEGER NOT NULL
                );
                INSERT INTO download_task (url, file_name, create_time) VALUES ('u', 'keep.bin', 1);",
            ).unwrap();
        }
        {
            let migrated = init(&dir).unwrap();
            let row: (String, String) = migrated.query_row(
                "SELECT file_name, mirrors_json FROM download_task WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).unwrap();
            assert_eq!(row, ("keep.bin".into(), "[]".into()));
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
