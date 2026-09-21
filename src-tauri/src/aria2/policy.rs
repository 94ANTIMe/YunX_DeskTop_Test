//! aria2 策略纯函数（ADR-0007 C5）：状态映射、并发调参、路径净化、代理参数。
//! 全部为无副作用可单测的叶函数；测试随迁本文件。

use serde_json::{json, Value};

use crate::models::{DownloadTaskView, Settings};

/// aria2 任务快照（tellStatus 关键字段）
#[derive(Debug, Clone, Default)]
pub(crate) struct TaskStatus {
    pub(crate) status: String,       // active/waiting/paused/error/complete/removed
    pub(crate) total: i64,
    pub(crate) completed: i64,
    pub(crate) speed: i64,
    pub(crate) error_msg: String,
    pub(crate) files: Vec<String>,   // 实际落盘路径
}

/// aria2 状态 → 任务状态常量
pub(crate) fn map_status(s: &str) -> i32 {
    match s {
        "active" => DownloadTaskView::STATUS_DOWNLOADING,
        "waiting" => DownloadTaskView::STATUS_PENDING,
        "paused" => DownloadTaskView::STATUS_PAUSED,
        "complete" => DownloadTaskView::STATUS_COMPLETED,
        "error" => DownloadTaskView::STATUS_FAILED,
        _ => DownloadTaskView::STATUS_DOWNLOADING,
    }
}

pub(crate) fn parse_status(v: &Value) -> TaskStatus {
    let s = |key: &str| match v.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(value) if value.is_number() => value.to_string(),
        _ => String::new(),
    };
    let files = v
        .get("files")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.get("path").and_then(|p| p.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let error_code = s("errorCode");
    let error_message = s("errorMessage");
    let error_msg = if error_message.is_empty() && !error_code.is_empty() && error_code != "0" {
        format!("aria2 错误码 {error_code}")
    } else if !error_message.is_empty() && !error_code.is_empty() && error_code != "0" {
        format!("{error_message}（aria2 错误码 {error_code}）")
    } else {
        error_message
    };
    TaskStatus {
        status: s("status"),
        total: v.get("totalLength").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        completed: v.get("completedLength").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        speed: v.get("downloadSpeed").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        error_msg,
        files,
    }
}

/// 夸克直链对 Range 并发和单主机连接数更敏感，采用保守参数避免 active 但 0 速度。
pub(crate) fn transfer_tuning(platform: &str, threads: i32, connections: i32, mirror_count: usize) -> (i32, i32) {
    if platform == "quark" {
        return (threads.clamp(1, 4), connections.clamp(1, 4));
    }
    let split = if mirror_count > 1 {
        (threads.clamp(16, 64) * (mirror_count as i32).min(2)).clamp(16, 64)
    } else {
        threads.clamp(1, 64)
    };
    (split, connections.clamp(1, 16))
}

/// 夸克直链僵死守护：速度 ≤1KB/s 时由 aria2 中止任务，避免 0 速度永久占住并发槽。
/// 中止后任务转失败态，用户点「开始」会走重新取链（见 resume）。
pub(crate) fn stall_guard_options(platform: &str) -> Vec<(String, String)> {
    if platform == "quark" {
        vec![("lowest-speed-limit".into(), "1024".into())]
    } else {
        vec![]
    }
}

/// HTTP 直链任务的 aria2 addUri 选项（纯函数，便于测试锁定参数组装）
pub(crate) fn http_task_options(
    dir: &str,
    file_name: &str,
    header_list: &[String],
    split: i32,
    max_connections: i32,
    min_split: &str,
    max_tries: i32,
    extra: &[(String, String)],
) -> Value {
    let mut options = json!({
        "dir": dir,
        "out": sanitize_out_path(file_name),
        "header": header_list,
        "split": split,
        "max-connection-per-server": max_connections,
        "min-split-size": min_split,
        "continue": "true",
        "max-tries": max_tries,
        "max-file-not-found": 3,
    });
    for (k, v) in extra {
        options[k.as_str()] = json!(v);
    }
    options
}


pub(crate) fn limit_str(limit: i64) -> String {
    if limit <= 0 {
        "0".into()
    } else {
        format!("{}B", limit) // aria2 接受 "1048576B" 形式
    }
}

/// 构建 `--all-proxy` 参数（http / socks5，含可选用户密码，密码经 URL 编码）
pub(crate) fn build_proxy_arg(settings: &Settings) -> String {
    let scheme = if settings.proxy_type.eq_ignore_ascii_case("socks5") { "socks5" } else { "http" };
    let mut url = format!("{scheme}://");
    if !settings.proxy_username.is_empty() {
        url.push_str(&urlencoding::encode(&settings.proxy_username));
        url.push(':');
        if !settings.proxy_password.is_empty() {
            url.push_str(&urlencoding::encode(&settings.proxy_password));
        }
        url.push('@');
    }
    url.push_str(settings.proxy_host.trim());
    if settings.proxy_port > 0 {
        url.push(':');
        url.push_str(&settings.proxy_port.to_string());
    }
    url
}

/// 设置中代理是否已填完整（可用）
pub(crate) fn proxy_configured(settings: &Settings) -> bool {
    settings.proxy_enabled
        && !settings.proxy_host.trim().is_empty()
        && settings.proxy_port > 0
}

pub(crate) fn proxy_log_summary(settings: &Settings) -> String {
    format!(
        "type={} host={} port={} auth={}",
        settings.proxy_type,
        settings.proxy_host.trim(),
        settings.proxy_port,
        !settings.proxy_username.is_empty()
    )
}

// ---------- 任务入队 / 控制 ----------

/// Windows 保留设备名（不区分大小写，命中带扩展名形式如 nul.txt 的主干即可）
pub(crate) fn is_windows_reserved_name(comp: &str) -> bool {
    let stem = comp.split('.').next().unwrap_or(comp);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    )
}

/// 净化入队文件名（aria2 `out` 参数与删除原语共用，DB 存净化值）：
/// 剥离 `..` / 独立盘符等路径分量（防直链文件名把文件写出下载目录），
/// 保留 `relDir/文件名` 的子目录结构；处理 Windows 保留名与尾点尾空格；全空回退 download.bin。
pub fn sanitize_out_path(name: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for raw in name.split(['/', '\\']) {
        let comp = raw.trim().trim_end_matches('.').trim_end();
        // 空分量 / 当前目录 / 上级目录 / 盘符（如 "C:"）一律丢弃
        if comp.is_empty() || comp == "." || comp == ".." {
            continue;
        }
        let bytes = comp.as_bytes();
        if bytes.len() == 2 && bytes[1] == b':' {
            continue;
        }
        if is_windows_reserved_name(comp) {
            parts.push(format!("_{comp}"));
        } else {
            parts.push(comp.to_string());
        }
    }
    if parts.is_empty() {
        return "download.bin".to_string();
    }
    parts.join("/")
}
