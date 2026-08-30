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

/// PanSou 连通性检测结果（首启引导 / 设置页）
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PansouPingResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: String,
}

/// 测试 PanSou 服务连通性（发起一次真实搜索请求）
#[tauri::command]
pub async fn pansou_ping(app: AppHandle, base_url: String) -> PansouPingResult {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return PansouPingResult { ok: false, latency_ms: 0, error: "服务地址为空".into() };
    }
    let start = std::time::Instant::now();
    match crate::api::pansou::search(&base, "yunx", None).await {
        Ok(items) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            app.state::<AppState>().log(
                crate::logger::SUCCESS,
                "pansou",
                "ping",
                &format!("PanSou 连通：{} 条结果", items.len()),
                &format!("{base} · {latency_ms}ms"),
            );
            PansouPingResult { ok: true, latency_ms, error: String::new() }
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            app.state::<AppState>().log(
                crate::logger::ERROR,
                "pansou",
                "ping",
                "PanSou 连通检测失败",
                &e.to_string(),
            );
            PansouPingResult { ok: false, latency_ms, error: e.to_string() }
        }
    }
}
