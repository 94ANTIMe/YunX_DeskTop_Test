/// SQLite schema v1：对齐 Android 版 Room 数据结构（列名 snake_case 重设计，全新库无需迁移）。
/// 8 张表：6 平台账号 + download_task + bookmark。
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS quark_account (
    id TEXT PRIMARY KEY,
    cookie TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS uc_account (
    id TEXT PRIMARY KEY,
    cookie TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS baidu_account (
    id TEXT PRIMARY KEY,
    cookie TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS c139_account (
    id TEXT PRIMARY KEY,
    cookie TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    authorization TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS xunlei_account (
    id TEXT PRIMARY KEY,
    access_token TEXT NOT NULL DEFAULT '',
    refresh_token TEXT NOT NULL DEFAULT '',
    device_id TEXT NOT NULL DEFAULT '',
    captcha_token TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS pan123_account (
    id TEXT PRIMARY KEY,
    access_token TEXT NOT NULL DEFAULT '',
    account TEXT NOT NULL DEFAULT '',
    nickname TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS download_task (
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
    mirrors_json TEXT NOT NULL DEFAULT '[]',
    fetch_ctx_json TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    avg_speed INTEGER NOT NULL DEFAULT 0,
    create_time INTEGER NOT NULL,
    finish_time INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS download_stat (
    day TEXT NOT NULL,
    platform TEXT NOT NULL,
    files INTEGER NOT NULL DEFAULT 0,
    bytes INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, platform)
);
CREATE TABLE IF NOT EXISTS bookmark (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    link TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    pwd TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT '未分类',
    create_time INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS app_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    time INTEGER NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    platform TEXT NOT NULL DEFAULT '',
    action TEXT NOT NULL DEFAULT '',
    message TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS resolve_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    link TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    create_time INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS subscription (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword TEXT NOT NULL,
    cloud_types_json TEXT NOT NULL DEFAULT '["quark","uc"]',
    episode_regex TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at INTEGER NOT NULL DEFAULT 0,
    last_result TEXT NOT NULL DEFAULT '',
    create_time INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS subscription_item (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id INTEGER NOT NULL,
    episode_key TEXT NOT NULL DEFAULT '',
    file_name TEXT NOT NULL,
    download_task_id INTEGER NOT NULL DEFAULT 0,
    create_time INTEGER NOT NULL,
    UNIQUE(subscription_id, file_name)
);
"#;
