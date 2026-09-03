//! BaiduPCS-Go sidecar 集成：登录 + 取直链（locate）。
//! 背景：api::baidu::locate_download 硬编码设备签名（devuid/cuid/psign），百度校验签名
//! 一旦失效即返回 403（hitcode:104），百度取链完全不可用。BaiduPCS-Go 动态计算签名，
//! 经实测 locate 直链可正常交给 aria2 下载（优先 bdd0/xad0 等直连源站节点）。
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use reqwest::Client;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{AppError, AppResult};

/// 百度直链下载所需 UA（BaiduPCS-Go locate 提示的 P2SP 标识；aria2 携带）
pub const UA: &str =
    "netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;";

/// 运行时 sidecar 绝对路径（lib.rs setup 时通过 shell.sidecar 解析）
static SIDECAR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// 串行化 BaiduPCS-Go 调用（共享配置目录，避免并发写冲突）
static RUN_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

/// 从 locate 输出中提取 https 直链的正则（URL 后可能跟 ANSI 复位码）
static URL_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// setup 时调用一次：记录 sidecar 路径
pub fn init_sidecar(path: PathBuf) {
    let _ = SIDECAR.set(path);
}

fn sidecar() -> AppResult<&'static PathBuf> {
    SIDECAR
        .get()
        .ok_or_else(|| AppError::Api("百度下载组件（BaiduPCS-Go）未就绪".into()))
}

fn url_re() -> &'static regex::Regex {
    URL_RE.get_or_init(|| regex::Regex::new(r"https://[^\s]+").unwrap())
}

/// 配置目录（隔离于用户默认 ~/.config/BaiduPCS-Go，防污染）
fn config_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("baidupcs")
}

/// 在 BaiduPCS-Go 交互会话中依次执行命令，返回全部 stdout
async fn run_commands(data_dir: &std::path::Path, commands: &[String]) -> AppResult<String> {
    let mut cmd = Command::new(sidecar()?);
    cmd.env("BAIDUPCS_GO_CONFIG_DIR", config_dir(data_dir))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: 静默后台运行，禁止弹出控制台黑窗

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Api(format!("百度下载组件启动失败: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        let mut input = String::new();
        for c in commands {
            input.push_str(c);
            input.push('\n');
        }
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| AppError::Api(format!("百度下载组件交互失败: {e}")))?;
    }

    let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .map_err(|_| AppError::Api("百度下载组件取链超时".into()))?
        .map_err(|e| AppError::Api(format!("百度下载组件执行失败: {e}")))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 从 locate 输出提取全部候选直链（已剥离尾随 ANSI 复位码）
fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for m in url_re().find_iter(text) {
        let mut url = m.as_str().to_string();
        if let Some(pos) = url.find('\u{1b}') {
            url.truncate(pos);
        }
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls
}

/// 探测并挑选出所有支持正常下载的直连源站与优质镜像节点（排在最前的为最优直连源站）
async fn pick_all_working_urls(client: &Client, urls: &[String]) -> Vec<String> {
    let mut working = Vec::new();
    let is_origin_node = |u: &str| {
        u.contains("bdd0.baidupcs.com")
            || u.contains("xad0.baidupcs.com")
            || u.contains("yqd0.baidupcs.com")
            || u.contains("gzdd.baidupcs.com")
            || u.contains("shdd.baidupcs.com")
            || u.contains("njdd.baidupcs.com")
            || u.contains("qd00.baidupcs.com")
            || u.contains("d.pcs.baidu.com")
    };

    // 1. 优先提取并探测所有源站节点（如保定、西安、阳泉、广州等独立数据中心）
    for url in urls.iter().filter(|u| is_origin_node(u)) {
        if let Ok(resp) = client
            .get(url)
            .header("User-Agent", UA)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() && !working.contains(url) {
                working.push(url.clone());
            }
        }
    }

    // 2. 补充探测非 403 且非 allall 节点（如电信/联通骨干镜像，丰富跨机房并发源）
    for url in urls {
        if !is_origin_node(url) && !url.contains("allall") {
            if let Ok(resp) = client
                .get(url)
                .header("User-Agent", UA)
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                let s = resp.status();
                if s.is_success() && s.as_u16() != 403 && !working.contains(url) {
                    working.push(url.clone());
                }
            }
        }
    }

    // 若探测均未响应，回退安全节点
    if working.is_empty() {
        for url in urls {
            if !url.contains("allall") && !working.contains(url) {
                working.push(url.clone());
            }
        }
    }
    if working.is_empty() {
        if let Some(first) = urls.first() {
            working.push(first.clone());
        }
    }

    // 将不同主机名的节点交错排列，确保 aria2 优先分散连接到各独立物理数据中心
    let mut interleaved = Vec::new();
    let mut remaining = working;
    while !remaining.is_empty() {
        let mut seen_hosts = std::collections::HashSet::new();
        let mut next_remaining = Vec::new();
        for u in remaining {
            let host = u.split('/').nth(2).unwrap_or("").to_string();
            if !seen_hosts.contains(&host) {
                seen_hosts.insert(host);
                interleaved.push(u);
            } else {
                next_remaining.push(u);
            }
        }
        remaining = next_remaining;
    }

    interleaved
}

/// 登录 + 取全部可用直链镜像（locate_urls）：返回多机房多节点镜像列表供 aria2 多源并发下载
pub async fn locate_urls(client: &Client, cookie: &str, path: &str, data_dir: &std::path::Path) -> AppResult<Vec<String>> {
    let _guard = RUN_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let locate_cmd = if path.chars().any(|c| c.is_whitespace()) {
        format!("locate \"{path}\"")
    } else {
        format!("locate {path}")
    };
    for attempt in 1..=3 {
        let commands = vec![
            format!("login --cookies=\"{cookie}\""),
            locate_cmd.clone(),
        ];
        let out = run_commands(data_dir, &commands).await?;
        let candidates = extract_urls(&out);
        let working = pick_all_working_urls(client, &candidates).await;
        if !working.is_empty() {
            return Ok(working);
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    Err(AppError::Api("百度取链失败：未获取到有效下载链接".into()))
}

/// 兼容老调用的单直链获取
pub async fn locate(client: &Client, cookie: &str, path: &str, data_dir: &std::path::Path) -> AppResult<String> {
    let urls = locate_urls(client, cookie, path, data_dir).await?;
    urls.first().cloned().ok_or_else(|| AppError::Api("百度取链失败：未获取到有效下载链接".into()))
}

/// 登录 + 删除文件（转存清理）。走 PCS 通道，不受网页版 filemanager
/// 的 errno=132 账号风控影响（实测 2026-09）。best-effort：失败返回 Err 由调用方忽略。
pub async fn remove(cookie: &str, path: &str, data_dir: &std::path::Path) -> AppResult<()> {
    let _guard = RUN_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let rm_cmd = if path.chars().any(|c| c.is_whitespace()) {
        format!("rm \"{path}\"")
    } else {
        format!("rm {path}")
    };
    let commands = vec![
        format!("login --cookies=\"{cookie}\""),
        rm_cmd,
    ];
    let out = run_commands(data_dir, &commands).await?;
    if out.contains("操作成功") || out.contains("已删除") {
        Ok(())
    } else {
        Err(AppError::Api("清理临时转存文件失败".into()))
    }
}
