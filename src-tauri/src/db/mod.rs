pub mod accounts;
pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

/// 在指定目录打开（或创建）yunx.db 并建表
pub fn init(data_dir: &Path) -> AppResult<Connection> {
    std::fs::create_dir_all(data_dir)?;
    let conn = Connection::open(data_dir.join("yunx.db"))?;
    conn.execute_batch(schema::SCHEMA)?;
    // v0.1 骨架库 → v0.2 增加 gid 列（列已存在时忽略）
    let _ = conn.execute(
        "ALTER TABLE download_task ADD COLUMN gid TEXT NOT NULL DEFAULT ''",
        [],
    );
    // v0.2 迅雷指纹持久化（文件方式，见 xunlei 模块）无表变更
    Ok(conn)
}
