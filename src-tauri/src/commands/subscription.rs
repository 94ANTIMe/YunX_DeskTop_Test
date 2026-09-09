//! 订阅追剧命令：CRUD + 手动立即执行。

use tauri::{AppHandle, Manager};

use crate::error::AppResult;
use crate::models::{SubscriptionRow, SubscriptionUpsert};
use crate::state::AppState;

/// 列出全部订阅
#[tauri::command]
pub async fn list_subscriptions(app: AppHandle) -> AppResult<Vec<SubscriptionRow>> {
    let state = app.state::<AppState>();
    crate::subscription::list(&state)
}

/// 新增订阅（返回 id）
#[tauri::command]
pub async fn add_subscription(
    app: AppHandle,
    keyword: String,
    cloud_types: Option<Vec<String>>,
    episode_regex: Option<String>,
) -> AppResult<i64> {
    let state = app.state::<AppState>();
    let keyword_trimmed = keyword.trim().to_string();
    let upsert = SubscriptionUpsert {
        keyword,
        cloud_types: cloud_types.unwrap_or_else(|| vec!["quark".into(), "uc".into()]),
        episode_regex: episode_regex.unwrap_or_default(),
    };
    let id = crate::subscription::add(&state, &upsert)?;
    state.log(
        crate::logger::SUCCESS,
        "subscription",
        "add",
        &format!("新增订阅：{keyword_trimmed}"),
        &format!("id={id}"),
    );
    Ok(id)
}

/// 编辑订阅（关键词 / 平台偏好 / 过滤正则 / 启停）
#[tauri::command]
pub async fn update_subscription(
    app: AppHandle,
    id: i64,
    enabled: bool,
    keyword: String,
    cloud_types: Option<Vec<String>>,
    episode_regex: Option<String>,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let upsert = SubscriptionUpsert {
        keyword,
        cloud_types: cloud_types.unwrap_or_else(|| vec!["quark".into(), "uc".into()]),
        episode_regex: episode_regex.unwrap_or_default(),
    };
    crate::subscription::update(&state, id, enabled, &upsert)
}

/// 删除订阅（连同已下载集数记录）
#[tauri::command]
pub async fn remove_subscription(app: AppHandle, id: i64) -> AppResult<()> {
    let state = app.state::<AppState>();
    crate::subscription::remove(&state, id)
}

/// 手动立即执行一次订阅检查（阻塞至完成，返回结果摘要）
#[tauri::command]
pub async fn run_subscription_now(app: AppHandle, id: i64) -> AppResult<String> {
    crate::subscription::run_now(app, id).await
}
