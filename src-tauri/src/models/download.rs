use serde::{Deserialize, Serialize};
/// 下载直链结果（含下载所需请求头与多镜像源）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLink {
    pub url: String,
    pub filename: String,
    pub size: i64,
    /// 下载直链必须携带的请求头（Cookie/UA/Referer）
    pub headers: Vec<(String, String)>,
    /// 平台标识（下载任务归属）
    pub platform: String,
    /// 取链后需延迟清理的转存文件 id（夸克：下载完成后清理）
    pub cleanup_id: String,
    /// 多源站镜像下载链接（aria2 多源并发加速）
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// 重新取链上下文（JSON，仅夸克）：恢复/失败重试时按它重新取直链；空 = 不支持重取
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fetch_ctx: String,
}

/// 下载任务（DB 行 + aria2 实时状态合并，事件推送前端）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskView {
    pub id: i64,
    pub gid: String,
    pub url: String,
    pub file_name: String,
    pub platform: String,
    pub total_size: i64,
    pub downloaded_size: i64,
    pub speed: i64,
    pub status: i32,
    pub error_msg: String,
    pub save_path: String,
    pub create_time: i64,
}

impl DownloadTaskView {
    /// 任务状态常量（对齐 Android DownloadTaskEntity）
    pub const STATUS_PENDING: i32 = 0;
    pub const STATUS_DOWNLOADING: i32 = 1;
    pub const STATUS_PAUSED: i32 = 2;
    pub const STATUS_COMPLETED: i32 = 3;
    pub const STATUS_FAILED: i32 = 4;
}

/// 下载统计总览（统计报表页；独立 download_stat 表，清空任务记录不影响）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    pub totals: StatsTotals,
    pub daily: Vec<StatsDaily>,
    pub platforms: Vec<StatsPlatform>,
}

/// 累计汇总
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsTotals {
    pub files: i64,
    pub bytes: i64,
    pub failed: i64,
}

/// 单日聚合（day = "YYYY-MM-DD"，本地时区）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDaily {
    pub day: String,
    pub files: i64,
    pub bytes: i64,
    pub failed: i64,
}

/// 平台维度聚合
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsPlatform {
    pub platform: String,
    pub files: i64,
    pub bytes: i64,
    pub failed: i64,
}

/// 下载任务 Dashboard 详情（任务卡片点开后的扩展数据）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDetail {
    pub id: i64,
    pub gid: String,
    pub url: String,
    pub file_name: String,
    pub platform: String,
    pub total_size: i64,
    pub downloaded_size: i64,
    pub speed: i64,
    pub status: i32,
    pub error_msg: String,
    pub save_path: String,
    pub create_time: i64,
    /// 当前分片连接数
    pub connections: i32,
    /// 上传速度（字节/秒）
    pub upload_speed: i64,
    /// 已耗时（秒）
    pub total_time: i64,
}
