use serde::{Deserialize, Serialize};

/// 网盘平台标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Quark,
    Uc,
    Xunlei,
    Baidu,
    C139,
    Pan123,
}

impl Platform {
    /// 小写标识（DB / IPC 传输用）
    pub fn key(&self) -> &'static str {
        match self {
            Platform::Quark => "quark",
            Platform::Uc => "uc",
            Platform::Xunlei => "xunlei",
            Platform::Baidu => "baidu",
            Platform::C139 => "c139",
            Platform::Pan123 => "pan123",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "quark" => Platform::Quark,
            "uc" => Platform::Uc,
            "xunlei" => Platform::Xunlei,
            "baidu" => Platform::Baidu,
            "c139" => Platform::C139,
            "pan123" => Platform::Pan123,
            _ => return None,
        })
    }
}

/// 分享链接解析结果（对齐 Android ShareLinkParser.ParsedShare）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedShare {
    pub platform: String,
    pub share_id: String,
    pub pwd: String,
}

/// 分享内文件条目（对齐 Android ShareFile）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFile {
    pub fid: String,
    pub fname: String,
    pub fsize: i64,
    pub isdir: bool,
    pub pdir_fid: String,
    /// 平台专属令牌（夸克/UC share_fid_token；123 为 "S3KeyFlag|Etag|StorageNode"）
    pub fid_token: String,
    pub modify_time: String,
}

/// 解析会话建立结果（首页）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSessionInfo {
    pub session_key: String,
    pub platform: String,
    pub title: String,
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 文件列表页
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFilePage {
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 下载直链结果（含下载所需请求头）
#[derive(Debug, Clone, Serialize)]
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
}

/// 账号摘要（前端网盘页展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub platform: String,
    pub nickname: Option<String>,
    pub logged_in: bool,
}

/// 应用设置（app_data_dir/settings.json 持久化；键语义对齐 Android SettingsRepository）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// 自定义下载保存目录（空 = 系统下载文件夹）
    pub download_dir: String,
    /// 最大同时下载任务数，默认 1
    pub max_concurrent_downloads: i32,
    /// 分片并发数（aria2 split），默认 32，上限 512
    pub download_threads: i32,
    /// 全局限速（字节/秒；0 = 不限速）
    pub download_speed_limit: i64,
    /// 失败重试次数，默认 3
    pub download_retry_count: i32,
    /// 分片最小体积（aria2 min-split-size，单位 MB），默认 4，范围 1-64
    pub download_min_split_mb: i32,
    /// 单服务器最大连接数（aria2 max-connection-per-server），默认 16，上限 16
    pub download_conn_per_server: i32,
    /// PanSou 自部署搜索服务地址（如 http://192.168.1.100:8888）；空 = 未配置
    pub pansou_base_url: String,
    /// 深色模式：0 跟随系统 / 1 浅色 / 2 深色
    pub dark_mode: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            max_concurrent_downloads: 1,
            download_threads: 32,
            download_speed_limit: 0,
            download_retry_count: 3,
            download_min_split_mb: 4,
            download_conn_per_server: 16,
            pansou_base_url: String::new(),
            dark_mode: 0,
        }
    }
}

/// PanSou 搜索结果条目（merged_by_type 分组内的一项）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    /// 网盘类型：baidu/quark/aliyun/...
    pub r#type: String,
    /// 分享链接
    pub url: String,
    /// 提取码（可能为空）
    pub password: String,
    /// 标题/备注
    pub note: String,
    /// 来源（tg:频道 / plugin:插件）
    pub source: String,
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

/// 收藏的网盘链接（对齐 bookmark 表）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkRow {
    pub id: i64,
    pub link: String,
    pub title: String,
    pub platform: String,
    pub pwd: String,
    pub category: String,
    pub create_time: i64,
}
