use std::io::Write;

use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// 默认更新源仓库（GitHub Releases API）。
const DEFAULT_REPO: &str = "94ANTIMe/YunX_DeskTop_Test";
/// GitCode 镜像仓库（下载源之二，主仓库 release 的镜像）。
const GC_REPO: &str = "ANTIMelody/YunX_DeskTop_Test";
/// 测速探测下载的字节数（仅用于预估各源吞吐，越小探测越快，这里用 256KB）。
const PROBE_BYTES: u64 = 256 * 1024;

/// 在线更新检查结果（序列化传给前端）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 是否有可用更新
    pub has_update: bool,
    /// 当前版本号
    pub current_version: &'static str,
    /// 远端最新版本号（无更新时为当前版本）
    pub latest_version: String,
    /// Release 名称
    pub name: String,
    /// Release 说明
    pub notes: String,
    /// 匹配的 NSIS 安装包直链（GitHub）
    pub download_url: String,
    /// 匹配的 NSIS 安装包直链（GitCode 镜像）
    pub gitcode_download_url: String,
    /// Release 页面（手动下载兜底）
    pub browser_download_url: String,
}

impl UpdateInfo {
    fn no_update() -> Self {
        Self {
            has_update: false,
            current_version: env!("CARGO_PKG_VERSION"),
            latest_version: env!("CARGO_PKG_VERSION").to_string(),
            name: String::new(),
            notes: String::new(),
            download_url: String::new(),
            gitcode_download_url: String::new(),
            browser_download_url: String::new(),
        }
    }
}

/// 数字分段版本比较：a < b 返回 true（容忍 tag 前导 'v' / 非纯数字段）。
fn version_less(a: &str, b: &str) -> bool {
    let seg = |s: &str| -> Vec<i32> {
        s.trim_start_matches('v')
            .split(['.', '-', '+'])
            .filter_map(|x| x.parse::<i32>().ok())
            .collect()
    };
    let av = seg(a);
    let bv = seg(b);
    let n = av.len().max(bv.len());
    for i in 0..n {
        let x = av.get(i).copied().unwrap_or(0);
        let y = bv.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Less => return true,
            std::cmp::Ordering::Greater => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    false
}

/// GitHub Release JSON 的字段切片（只取所需）
#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// 是否 NSIS 安装包（`...-setup.exe`；忽略 MSI）
fn is_nsis_setup(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with("-setup.exe")
}

/// 检查是否有新版本（从 GitHub 最新 Release 匹配 NSIS 安装包）。
pub async fn check(app: &AppHandle) -> AppResult<UpdateInfo> {
    let state = app.state::<AppState>();
    let url = format!("https://api.github.com/repos/{DEFAULT_REPO}/releases/latest");
    let resp = match state
        .http
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "yunx-desktop")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            state.log("warn", "update", "check", "更新检查网络失败", &e.to_string());
            return Ok(UpdateInfo::no_update());
        }
    };
    if !resp.status().is_success() {
        state.log(
            "warn",
            "update",
            "check",
            "更新检查未命中",
            &format!("status={}", resp.status()),
        );
        return Ok(UpdateInfo::no_update());
    }
    let rel: GhRelease = match resp.json().await {
        Ok(r) => r,
        Err(e) => {
            state.log("warn", "update", "check", "更新检查响应解析失败", &e.to_string());
            return Ok(UpdateInfo::no_update());
        }
    };
    let latest = rel.tag_name.trim_start_matches('v').to_string();
    let Some(asset) = rel.assets.iter().find(|a| is_nsis_setup(&a.name)) else {
        return Ok(UpdateInfo::no_update());
    };
    if !version_less(env!("CARGO_PKG_VERSION"), &latest) {
        return Ok(UpdateInfo::no_update());
    }
    state.log(
        "info",
        "update",
        "check",
        &format!("发现新版本 v{latest}"),
        &asset.name,
    );
    Ok(UpdateInfo {
        has_update: true,
        current_version: env!("CARGO_PKG_VERSION"),
        latest_version: latest,
        name: rel.name,
        notes: rel.body.trim().to_string(),
        download_url: asset.browser_download_url.clone(),
        gitcode_download_url: format!(
            "https://gitcode.com/{GC_REPO}/releases/download/{}/{}",
            rel.tag_name, asset.name
        ),
        browser_download_url: format!(
            "https://github.com/{DEFAULT_REPO}/releases/tag/{}",
            rel.tag_name
        ),
    })
}

/// 对某源发起一个小的 Range 请求，返回实测吞吐（MB/s）；失败/超时返回 None。
async fn probe(client: &Client, url: &str) -> Option<f64> {
    let resp = client
        .get(url)
        .header("Range", format!("bytes=0-{}", PROBE_BYTES - 1))
        .header("User-Agent", "yunx-desktop")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let start = std::time::Instant::now();
    let mut n: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        n += chunk.ok()?.len() as u64;
        if n >= PROBE_BYTES {
            break;
        }
    }
    if n == 0 {
        return None;
    }
    let secs = start.elapsed().as_secs_f64().max(1e-3);
    Some(n as f64 / 1_048_576.0 / secs)
}

/// 并发探测 GitHub / GitCode 两个源，返回较快者的 URL（无可用的源则报错）。
async fn pick_fastest_source(
    app: &AppHandle,
    github_url: &str,
    gitcode_url: &str,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    // 探测客户端：设总超时避免被很慢的源卡住
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(AppError::from)?;
    let (g, c) = tokio::join!(probe(&client, github_url), probe(&client, gitcode_url));
    let chosen = match (g, c) {
        (Some(gs), Some(cs)) => {
            if cs > gs {
                gitcode_url
            } else {
                github_url
            }
        }
        (Some(_), None) => github_url,
        (None, Some(_)) => gitcode_url,
        (None, None) => return Err(AppError::Api("两个下载源均不可达".into())),
    };
    state.log(
        "info",
        "update",
        "source",
        "已选择更快下载源",
        &format!("GitHub={:?}MB/s GitCode={:?}MB/s -> {}", g, c, chosen),
    );
    Ok(chosen.to_string())
}

/// 流式下载安装包到 `data_dir/update/YunX_latest-setup.exe`，期间上报 `update:progress`。
/// 下载前先并发探测并选择较快的源（GitHub / GitCode）。
pub async fn download(app: &AppHandle, github_url: &str, gitcode_url: &str) -> AppResult<String> {
    let state = app.state::<AppState>();
    let chosen = pick_fastest_source(app, github_url, gitcode_url).await?;
    // 下载用独立客户端：仅连接超时，不设总体超时（大文件需长时间）
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(AppError::from)?;
    let resp = client
        .get(&chosen)
        .header("User-Agent", "yunx-desktop")
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::Api(format!("下载安装包失败: {}", resp.status())));
    }

    let total = resp.content_length().unwrap_or(0);
    let dir = state.data_dir.join("update");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("YunX_latest-setup.exe");
    let mut file = std::fs::File::create(&path)?;

    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes)?;
        received += bytes.len() as u64;
        // 每 200ms 或完成时上报一次进度，避免刷屏
        if last_emit.elapsed().as_millis() >= 200 || received == total {
            let _ = app.emit(
                "update:progress",
                serde_json::json!({ "received": received, "total": total }),
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush()?;
    let path_str = path.to_string_lossy().into_owned();
    state.log("info", "update", "download", "安装包已下载", &path_str);
    Ok(path_str)
}

/// 静默运行安装包安装，随后退出本进程释放文件锁，由安装器覆盖并重启新版本。
pub fn install(app: &AppHandle, path: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let path = std::path::Path::new(path);
    if !path.exists() {
        return Err(AppError::Api("安装包不存在，请先下载".into()));
    }

    let mut cmd = std::process::Command::new(path);
    cmd.arg("/S"); // NSIS 静默安装
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
    let mut child = cmd.spawn()?;

    state.log("info", "update", "install", "已启动静默安装，应用即将退出", path.to_str().unwrap_or(""));
    // 短暂等待，确保安装器进程已拉起
    std::thread::sleep(std::time::Duration::from_millis(800));
    let _ = child.kill(); // 若拉起的只是启动器，忽略错误；安装器常驻继续工作
    let _ = app.exit(0); // 释放本进程占用，让安装器完成覆盖更新
    Ok(())
}