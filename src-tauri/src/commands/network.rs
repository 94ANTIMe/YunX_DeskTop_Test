//! 网络探测命令：代理连通性测试（真实出口 IP）。
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::aria2::{build_proxy_arg, proxy_configured};
use crate::state::AppState;

/// 代理连通性测试结果（设置页「测试代理」展示）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub ok: bool,
    /// 通过代理探测到的公网出口 IP
    pub ip: String,
    pub latency_ms: u64,
    pub error: String,
}

fn fail(state: &AppState, action: &str, detail: &str, latency_ms: u64, error: String) -> ProxyTestResult {
    state.log(crate::logger::ERROR, "proxy", action, "代理连通测试失败", detail);
    ProxyTestResult { ok: false, ip: String::new(), latency_ms, error }
}

/// 通过当前代理设置探测公网出口 IP（api.ipify.org；8s 超时）
#[tauri::command]
pub async fn test_proxy(app: AppHandle) -> ProxyTestResult {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    if !proxy_configured(&settings) {
        return ProxyTestResult {
            ok: false,
            ip: String::new(),
            latency_ms: 0,
            error: "请先开启代理并填写服务器地址与端口".into(),
        };
    }
    // 日志只记录 host:port，避免密码泄漏
    let masked = format!(
        "{}://{}:{}{}",
        if settings.proxy_type.eq_ignore_ascii_case("socks5") { "socks5" } else { "http" },
        settings.proxy_host.trim(),
        settings.proxy_port,
        if settings.proxy_username.is_empty() { "" } else { "（带认证）" }
    );
    let proxy_url = build_proxy_arg(&settings);
    let start = Instant::now();
    let proxy = match reqwest::Proxy::all(&proxy_url) {
        Ok(p) => p,
        Err(e) => return fail(&state, "test", &e.to_string(), 0, format!("代理地址无法解析: {e}")),
    };
    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(&state, "test", &e.to_string(), 0, format!("代理客户端初始化失败: {e}")),
    };
    match client.get("https://api.ipify.org?format=json").send().await {
        Ok(resp) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            match resp.text().await {
                Ok(text) => {
                    let ip = serde_json::from_str::<serde_json::Value>(&text)
                        .ok()
                        .and_then(|v| v.get("ip").and_then(|i| i.as_str()).map(String::from))
                        .unwrap_or_default();
                    if ip.is_empty() {
                        return fail(&state, "test", "响应无 IP", latency_ms, "未从响应中解析到出口 IP".into());
                    }
                    state.log(
                        crate::logger::SUCCESS,
                        "proxy",
                        "test",
                        &format!("代理连通：出口 IP {ip}"),
                        &format!("{masked} · {latency_ms}ms"),
                    );
                    ProxyTestResult { ok: true, ip, latency_ms, error: String::new() }
                }
                Err(e) => fail(&state, "test", &e.to_string(), latency_ms, format!("响应解析失败: {e}")),
            }
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            fail(&state, "test", &e.to_string(), latency_ms, format!("代理连接失败: {e}"))
        }
    }
}