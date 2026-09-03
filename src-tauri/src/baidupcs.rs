//! BaiduPCS-Go sidecar 集成：登录 + 取直链（locate）。
//! 背景：api::baidu::locate_download 硬编码设备签名（devuid/cuid/psign），百度校验签名
//! 一旦失效即返回 403（hitcode:104），百度取链完全不可用。BaiduPCS-Go 动态计算签名，
//! 经实测 locate 直链可正常交给 aria2 下载（Range 分片 + 优先 bdd0/bjdd 节点）。
//! 因此取链步骤改用本模块，转存（transfer）仍走 api::baidu（该链路已验证可用）。
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

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
    let mut child = Command::new(sidecar()?)
        .env("BAIDUPCS_GO_CONFIG_DIR", config_dir(data_dir))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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

/// 从 locate 输出提取直链：优先 bdd0/bjdd 节点（完整 GET 与 Range 均可），否则取首条
fn pick_url(text: &str) -> Option<String> {
    let mut preferred: Option<String> = None;
    let mut fallback: Option<String> = None;
    for m in url_re().find_iter(text) {
        let mut url = m.as_str().to_string();
        // 剥离尾随 ANSI 复位码（如 \x1b[0m）
        if let Some(pos) = url.find('\u{1b}') {
            url.truncate(pos);
        }
        if url.contains("bdd0.baidupcs.com") || url.contains("bjdd") {
            preferred.get_or_insert_with(|| url.clone());
        } else if fallback.is_none() {
            fallback = Some(url);
        }
    }
    preferred.or(fallback)
}

/// 登录 + 取直链（locate）：返回可交给 aria2 下载的 URL
pub async fn locate(cookie: &str, path: &str, data_dir: &std::path::Path) -> AppResult<String> {
    let _guard = RUN_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    // 路径含空格时加引号（BaiduPCS-Go 交互 shell 按引号解析）
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
        if let Some(url) = pick_url(&out) {
            return Ok(url);
        }
        if attempt < 3 {
            // locate 偶发未返回直链，重试（每次换新 sign）
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    Err(AppError::Api("百度取链失败：未获取到有效下载链接".into()))
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
