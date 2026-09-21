use serde::{Deserialize, Serialize};
/// 账号摘要（前端网盘页展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub platform: String,
    pub nickname: Option<String>,
    pub logged_in: bool,
}

/// 平台账号行（多账号切换列表）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    pub platform: String,
    pub key: String,
    pub nickname: String,
    pub updated_at: i64,
    /// 是否为当前选中账号
    pub active: bool,
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

/// 解析历史记录（对齐 resolve_history 表）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveHistoryRow {
    pub id: i64,
    pub link: String,
    pub title: String,
    pub platform: String,
    pub create_time: i64,
}

/// 订阅条目（subscription 表行，前端展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionRow {
    pub id: i64,
    pub keyword: String,
    /// 优先搜索的网盘类型（JSON 数组字符串，如 ["quark","uc"]）
    pub cloud_types_json: String,
    /// 自定义集数过滤正则（空 = 不过滤）
    pub episode_regex: String,
    pub enabled: bool,
    /// 上次检查时间戳（毫秒；0 = 从未运行）
    pub last_run_at: i64,
    /// 上次执行结果摘要（供 UI 展示）
    pub last_result: String,
    pub create_time: i64,
}

/// 订阅新增/编辑入参（前端 upsert）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionUpsert {
    pub keyword: String,
    /// 优先搜索的网盘类型（如 ["quark","uc"]）
    #[serde(default)]
    pub cloud_types: Vec<String>,
    /// 自定义集数过滤正则（空 = 不过滤）
    #[serde(default)]
    pub episode_regex: String,
}
