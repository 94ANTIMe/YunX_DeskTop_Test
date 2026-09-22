//! aria2 JSON-RPC 客户端（ADR-0007 C5）：密钥持久化、底层调用、自愈触发与瞬时重试。
//! APP_HANDLE 全局句柄、端口查杀也在此（均服务 RPC 生命周期）。

use std::sync::OnceLock;

use tauri::AppHandle;

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

// 父模块的引擎诊断日志与自愈重拉（rpc_call 的 Unauthorized / 连接失败恢复路径）
use super::engine::{engine_log, respawn_engine};

pub(crate) const RPC_PORT: u16 = 16800;

pub(crate) static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// 持久化读取/写入本地 RPC 密钥，保持多次重启或热重载间密钥绝对一致
pub(crate) fn rpc_secret() -> &'static str {
    static SECRET: OnceLock<String> = OnceLock::new();
    SECRET.get_or_init(|| {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let dir = std::path::PathBuf::from(appdata).join("com.yunx.desktop");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("aria2_secret.txt");
            if let Ok(s) = std::fs::read_to_string(&path) {
                let trimmed = s.trim();
                if trimmed.len() >= 16 {
                    return trimmed.to_string();
                }
            }
            let sec = crate::api::random_alnum(24);
            let _ = std::fs::write(&path, &sec);
            return sec;
        }
        crate::api::random_alnum(24)
    })
}

pub(crate) fn rpc_url() -> String {
    format!("http://127.0.0.1:{RPC_PORT}/jsonrpc")
}

/// 定向查杀 16800 端口上遗留的旧 aria2c 孤儿进程（仅针对 aria2c.exe，防端口冲突与 Unauthorized）
#[cfg(windows)]
pub(crate) fn kill_stale_aria2_on_port(port: u16) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = std::process::Command::new("cmd")
        .args(["/c", &format!("netstat -ano -p tcp | findstr /R /C:\":{port} .*LISTENING\"")])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pid_str) = parts.last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid != 0 && pid != std::process::id() {
                        let check = std::process::Command::new("cmd")
                            .args(["/c", &format!("tasklist /FI \"PID eq {pid}\" /FO CSV /NH")])
                            .creation_flags(CREATE_NO_WINDOW)
                            .output();
                        if let Ok(chk) = check {
                            let chk_str = String::from_utf8_lossy(&chk.stdout);
                            if chk_str.to_lowercase().contains("aria2c") {
                                let _ = std::process::Command::new("taskkill")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .creation_flags(CREATE_NO_WINDOW)
                                    .output();
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn kill_stale_aria2_on_port(_port: u16) {}

/// aria2 RPC 专用 HTTP 客户端（OnceLock 复用连接池，避免每次调用重建 Client 与 TCP 连接）
pub(crate) fn rpc_http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// 底层单次 JSON-RPC 调用
pub(crate) async fn rpc_call_raw(method: &str, params: Vec<Value>) -> AppResult<Value> {
    let http = rpc_http();
    // aria2 JSON-RPC：params 数组首元素为 "token:<secret>"，其后为实际参数
    let mut all_params: Vec<Value> = vec![json!(format!("token:{}", rpc_secret()))];
    all_params.extend(params);
    let body = json!({
        "jsonrpc": "2.0",
        "id": "yunx",
        "method": method,
        "params": all_params,
    });
    let resp = http
        .post(rpc_url())
        .timeout(std::time::Duration::from_secs(10))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Api(format!("下载引擎通信失败: {e}")))?;
    // 引擎繁忙 / 重启中会返回空体或截断响应；先读文本再解析，避免 reqwest
    // "error decoding response body" 这类不可读错误，且便于上层识别为瞬时失败重试
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::Api(format!("下载引擎响应读取失败: {e}")))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Api(format!("下载引擎响应解析失败（引擎可能正在重启）: {e}")))?;
    if let Some(err) = v.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("RPC 错误");
        return Err(AppError::Api(format!("下载引擎: {msg}")));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

/// 瞬时传输失败（引擎重启 / 繁忙导致的空响应、连接抖动）：可安全重试一次
pub(crate) fn is_transient_rpc_error(msg: &str) -> bool {
    msg.contains("下载引擎通信失败")
        || msg.contains("下载引擎响应读取失败")
        || msg.contains("下载引擎响应解析失败")
}

/// 连接级失败（请求发不出去：引擎进程死亡 / 端口无人监听），是「引擎已死」的信号
pub(crate) fn is_connection_error(msg: &str) -> bool {
    msg.contains("下载引擎通信失败")
}


/// 发起 JSON-RPC 请求：Unauthorized / 连接级失败走互斥自愈重拉（引擎崩溃不再永久停摆），
/// 瞬时传输失败短退避重试一次
pub(crate) async fn rpc_call(method: &str, params: Vec<Value>) -> AppResult<Value> {
    match rpc_call_raw(method, params.clone()).await {
        Ok(v) => Ok(v),
        Err(AppError::Api(msg)) if msg.contains("Unauthorized") => {
            if let Some(app) = APP_HANDLE.get() {
                engine_log(app, "rpc_call: 遇到 Unauthorized 鉴权失败，触发下载引擎自愈机制");
                if respawn_engine(app).await {
                    engine_log(app, "rpc_call: 引擎自愈重拉成功，重试原 RPC 请求");
                    return rpc_call_raw(method, params).await;
                }
            }
            Err(AppError::Api(msg))
        }
        Err(AppError::Api(msg)) if is_transient_rpc_error(&msg) => {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            match rpc_call_raw(method, params.clone()).await {
                Ok(v) => {
                    if let Some(app) = APP_HANDLE.get() {
                        engine_log(app, "rpc_call: 瞬时通信失败已重试成功");
                    }
                    Ok(v)
                }
                // 重试仍是连接级失败：引擎进程多半已崩溃 / 被杀，自愈重拉后再试最后一次
                Err(second) => {
                    if is_connection_error(&second.to_string()) {
                        if let Some(app) = APP_HANDLE.get() {
                            if respawn_engine(app).await {
                                engine_log(app, "rpc_call: 引擎无响应已自愈重拉，重试原 RPC 请求");
                                return rpc_call_raw(method, params).await;
                            }
                        }
                    }
                    Err(second)
                }
            }
        }
        Err(e) => Err(e),
    }
}

/// 发送已完整构好的 JSON-RPC 请求体（不做任何 token 处理），返回 result 或 None。
/// 供 system.multicall 等「子调用自带 token」的调用使用——外层再注入 token 会污染参数
/// （aria2 报 "The parameter at 0 has wrong type"，批量查询整体失败）。
pub(crate) async fn rpc_post_raw(body: &Value) -> Option<Value> {
    let resp = rpc_http()
        .post(rpc_url())
        .timeout(std::time::Duration::from_secs(10))
        .json(body)
        .send()
        .await
        .ok()?;
    let text = resp.text().await.ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    if v.get("error").is_some() {
        return None;
    }
    Some(v.get("result").cloned().unwrap_or(Value::Null))
}

/// 构造 system.multicall 请求体（纯函数，测试锁定结构）：
/// params[0] 必须是「调用数组」本身，每个子调用自带 token；
/// 外层绝不能再叠 token（rpc_call 的自动注入对 multicall 是致命的）。
pub(crate) fn build_multicall_body(secret: &str, gids: &[String]) -> Value {
    let calls: Vec<Value> = gids
        .iter()
        .map(|gid| json!({ "methodName": "aria2.tellStatus", "params": [format!("token:{secret}"), gid] }))
        .collect();
    json!({
        "jsonrpc": "2.0",
        "id": "yunx-batch",
        "method": "system.multicall",
        "params": [calls],
    })
}

#[cfg(test)]
mod tests {
    use super::build_multicall_body;

    #[test]
    fn multicall_body_keeps_calls_array_at_param0_without_envelope_token() {
        let body = build_multicall_body("sek", &["g1".to_string(), "g2".to_string()]);
        assert_eq!(body["method"], "system.multicall");
        let params = body["params"].as_array().unwrap();
        // 外层只能有「调用数组」一个参数——多出的 token 字符串会让 aria2 报 wrong type
        assert_eq!(params.len(), 1);
        let calls = params[0].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["methodName"], "aria2.tellStatus");
        assert_eq!(calls[0]["params"][0], "token:sek");
        assert_eq!(calls[0]["params"][1], "g1");
    }
}

