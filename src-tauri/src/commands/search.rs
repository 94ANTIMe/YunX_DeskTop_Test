use tauri::{AppHandle, Manager};

use crate::api;
use crate::error::AppResult;
use crate::models::SearchItem;
use crate::state::AppState;

/// PanSou 网盘搜索（自部署服务；服务地址未配置时报错引导）
#[tauri::command]
pub async fn pansou_search(
    app: AppHandle,
    kw: String,
    cloud_types: Option<Vec<String>>,
) -> AppResult<Vec<SearchItem>> {
    let base = {
        let state = app.state::<AppState>();
        state.load_settings().pansou_base_url
    };
    if base.trim().is_empty() {
        return Err(crate::error::AppError::Api(
            "未配置 PanSou 搜索服务地址，请在设置页填写".into(),
        ));
    }
    let result = api::pansou::search(&base, &kw, cloud_types.as_deref()).await?;
    app.state::<AppState>().log(
        crate::logger::INFO,
        "pansou",
        "search",
        &format!("搜索「{kw}」：{} 条结果", result.len()),
        &format!("base={base}"),
    );
    Ok(result)
}
