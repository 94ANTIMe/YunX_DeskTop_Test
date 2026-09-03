//! 百度网盘高速下载通道：百度分享经加速服务列目录 + 取高速直链（实测 5-8 MB/s）。
//! 服务端唯一门槛是解析码（3-6 个汉字，1-4 天更换）；本模块负责：
//! ① 列目录 / 取下载直链 API 调用；
//! ② 解析码自动刷新：下载更新包 → 解压提取 → 解析「最新密码：X」→ 持久化到 settings.json。
use std::io::Write;
use std::path::Path;

use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::logger;
use crate::models::{Settings, ShareFile};
use crate::state::AppState;

/// 默认服务地址
pub const DEFAULT_BASE_URL: &str = "https://mf.dp.wpurl.cc";
/// 直链下载 UA（服务端取链返回的 ua 字段值）
pub const DOWNLOAD_UA: &str = "netdisk;P2SP;3.0.20.138";

/// 官方工具分享（内含密码记录的更新包；密码随包更新）
const PWD_SHARE_SURL: &str = "CJUG1MXWHopfLBpaEUIoKA";
const PWD_SHARE_PWD: &str = "kwdm";

/// 解析码错误标记（is_password_error 判别用；服务端 code=20016）
const PWD_ERR_MARK: &str = "解析码错误或已失效";

/// 解析码刷新单飞锁（并发解析只触发一次刷新）
static REFRESH_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

/// 服务地址（设置留空回退默认）
pub fn base_url_of(settings: &Settings) -> String {
    let url = settings.baidu_speed_base_url.trim().trim_end_matches('/').to_string();
    if url.is_empty() { DEFAULT_BASE_URL.to_string() } else { url }
}

/// 是否为「解析码错误/过期」错误（触发自动刷新）
pub fn is_password_error(e: &AppError) -> bool {
    matches!(e, AppError::Api(m) if m.contains(PWD_ERR_MARK))
}

// ---------- API ----------

/// get_file_list 结果（randsk/uk/shareid 供后续 get_download_links 用）
pub struct AccelListData {
    pub files: Vec<ShareFile>,
    pub randsk: String,
    pub uk: String,
    pub shareid: String,
    /// 分享者昵称（作标题展示）
    pub uname: String,
}

async fn post_api(client: &Client, base: &str, path: &str, body: Value) -> AppResult<Value> {
    let resp = client
        .post(format!("{base}{path}"))
        .timeout(std::time::Duration::from_secs(90))
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code == 200 {
        return Ok(v.get("data").cloned().unwrap_or(Value::Null));
    }
    let msg = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("未知错误")
        .to_string();
    if code == 20016 {
        Err(AppError::Api(format!("{PWD_ERR_MARK}（{msg}）")))
    } else {
        Err(AppError::Api(format!("加速处理失败：{msg}")))
    }
}

/// 列出分享目录（dir 为分享内绝对路径；目录条目的 fid 即路径）
pub async fn get_file_list(
    client: &Client,
    base: &str,
    surl: &str,
    pwd: &str,
    dir: &str,
    parse_password: &str,
) -> AppResult<AccelListData> {
    let mut url = format!("https://pan.baidu.com/s/{surl}");
    if !pwd.is_empty() {
        url.push_str(&format!("?pwd={pwd}"));
    }
    let body = json!({
        "url": url,
        "surl": surl,
        "pwd": pwd,
        "dir": if dir.is_empty() { "/" } else { dir },
        "parse_password": parse_password,
    });
    let data = post_api(client, base, "/api/v1/user/parse/get_file_list", body).await?;
    let list = data
        .get("list")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    let s = |key: &str| data.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut files = Vec::new();
    for item in &list {
        let isdir = item.get("is_dir").and_then(|x| x.as_bool()).unwrap_or(false);
        let path = item.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let dlink = item.get("dlink").and_then(|x| x.as_str()).unwrap_or("").to_string();
        files.push(ShareFile {
            // 与百度官方路由同约定：目录 fid = 路径（导航传参），文件 fid = fs_id
            fid: if isdir {
                path.clone()
            } else {
                let id = item.get("fs_id");
                id.and_then(|x| x.as_i64())
                    .map(|v| v.to_string())
                    .or_else(|| id.and_then(|x| x.as_str()).map(String::from))
                    .unwrap_or_default()
            },
            fname: item.get("server_filename").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            fsize: item.get("size").and_then(|x| x.as_i64()).unwrap_or(0),
            isdir,
            // 文件完整路径（get_download_links 的 dir 参数由其父目录推导）
            pdir_fid: path,
            // 小文件列表直接携带 dlink（免一次取链调用，节省每日额度）
            fid_token: if !isdir && dlink.starts_with("http") { dlink } else { String::new() },
            modify_time: item
                .get("server_mtime")
                .and_then(|x| x.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        });
    }
    Ok(AccelListData {
        files,
        randsk: s("randsk"),
        uk: s("uk"),
        shareid: s("shareid"),
        uname: s("uname"),
    })
}

/// 取下载直链 → (url, ua)；file_path 为文件在分享内的完整路径（推导所属 dir）
pub async fn get_download_links(
    client: &Client,
    base: &str,
    surl: &str,
    pwd: &str,
    file_path: &str,
    fs_id: &str,
    randsk: &str,
    uk: &str,
    shareid: &str,
    parse_password: &str,
) -> AppResult<(String, String)> {
    let dir = parent_dir_of(file_path);
    let body = json!({
        "randsk": randsk,
        "uk": uk,
        "shareid": shareid,
        "fs_id": [fs_id],
        "surl": surl,
        "dir": dir,
        "pwd": pwd,
        "token": "guest",
        "parse_password": parse_password,
        "vcode_str": "",
        "vcode_input": "",
    });
    let data = post_api(client, base, "/api/v1/user/parse/get_download_links", body).await?;
    let first = data
        .get(0)
        .cloned()
        .ok_or_else(|| AppError::Api("加速服务未返回下载链接".into()))?;
    let ua = first
        .get("ua")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(DOWNLOAD_UA)
        .to_string();
    let url = first
        .get("urls")
        .and_then(|u| u.as_array())
        .and_then(|arr| arr.iter().find_map(|u| u.as_str()))
        .unwrap_or("")
        .to_string();
    if url.is_empty() {
        return Err(AppError::Api("加速服务未返回有效直链".into()));
    }
    Ok((url, ua))
}

/// 文件完整路径 → 所属目录（"/a/b/c.rar" → "/a/b"；根级 → "/"）
fn parent_dir_of(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
    }
}

// ---------- 解析码自动刷新 ----------

/// 刷新解析码：下载官方更新包 → 解压提取 → 持久化。
/// 需要用户已登录百度网盘（下载约 200-240 KiB/s，约 1-2 分钟）。
pub async fn refresh_password(state: &AppState) -> AppResult<String> {
    let _guard = REFRESH_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    state.log(logger::INFO, "baidu", "accel", "解析码失效，开始自动更新", "");

    let cookie = crate::resolve::load_account_cookie(
        state,
        crate::models::Platform::Baidu,
        "自动更新解析码需先登录百度网盘",
    )?;

    // 1. 验证官方工具分享 + 定位更新包（根目录 + 一级子目录）
    let share = crate::api::baidu::verify_share_pcs(&state.http, PWD_SHARE_SURL, PWD_SHARE_PWD, &cookie).await.map_err(|e| {
        crate::resolve::captcha_hint(e, "解析码自动获取失败（触发百度风控），可在 设置 → 百度网盘加速 中手动填写解析码")
    })?;
    let root = crate::api::baidu::list_share(&state.http, PWD_SHARE_SURL, &share.randsk, "/", &cookie, 1).await?;
    let mut candidate: Option<String> = None; // fs_id
    if let Some(f) = pick_pwd_rar(&root.files) {
        candidate = Some(f.fid.clone());
    } else {
        for d in root.files.iter().filter(|f| f.isdir).take(5) {
            if let Ok(sub) = crate::api::baidu::list_share(&state.http, PWD_SHARE_SURL, &share.randsk, &d.fid, &cookie, 1).await {
                if let Some(f) = pick_pwd_rar(&sub.files) {
                    candidate = Some(f.fid.clone());
                    break;
                }
            }
        }
    }
    let fs_id = candidate.ok_or_else(|| AppError::Api("官方分享中未找到更新包".into()))?;

    // 2. 转存到临时目录 → locate 取链
    let temp_dir = crate::api::baidu::ensure_temp_dir(&state.http, &cookie).await?;
    let (_, new_path) = crate::api::baidu::transfer(
        &state.http,
        PWD_SHARE_SURL,
        &share.share_id,
        &share.uk,
        &share.randsk,
        &fs_id,
        &temp_dir,
        &cookie,
    )
    .await?;
    let url = match crate::baidupcs::locate(&state.http, &cookie, &new_path, &state.data_dir).await {
        Ok(u) => u,
        Err(e) => {
            let _ = crate::baidupcs::remove(&cookie, &new_path, &state.data_dir).await;
            return Err(e);
        }
    };

    // 3. 下载更新包到缓存目录（%APPDATA%/com.yunx.desktop/cache）
    let cache_dir = state.data_dir.join("cache");
    std::fs::create_dir_all(&cache_dir)?;
    let rar_path = cache_dir.join("accel_tool.bin");
    state.log(logger::INFO, "baidu", "accel", "正在获取更新数据（需 1-2 分钟）", "");
    if let Err(e) = download_file(&state.http, &url, &rar_path).await {
        let _ = crate::baidupcs::remove(&cookie, &new_path, &state.data_dir).await;
        return Err(e);
    }
    // 4. 即时清理转存（locate 直链自带签名，删除不影响下载）
    let _ = crate::baidupcs::remove(&cookie, &new_path, &state.data_dir).await;

    // 5. 提取并解析密码
    let password = extract_password(&rar_path)?;
    let mut settings = state.load_settings();
    settings.baidu_speed_password = password.clone();
    state.save_settings(&settings)?;
    state.log(logger::SUCCESS, "baidu", "accel", "解析码已自动更新", "");
    Ok(password)
}

/// 在分享列表中挑密码记录文件：优先文件名含「加速」的，否则最小体积（更新数据 16MB）
fn pick_pwd_rar(files: &[ShareFile]) -> Option<&ShareFile> {
    let mut best: Option<(&ShareFile, bool)> = None; // (文件, 是否「加速」命名)
    for f in files.iter().filter(|f| !f.isdir) {
        if !f.fname.to_lowercase().ends_with(".rar") {
            continue;
        }
        let named = f.fname.contains("加速");
        match &best {
            None => best = Some((f, named)),
            Some((bf, old_named)) => {
                if (named && !*old_named) || (named == *old_named && f.fsize < bf.fsize) {
                    best = Some((f, named));
                }
            }
        }
    }
    best.map(|(f, _)| f)
}

/// 流式下载到本地（Range 请求走 bdd0 节点，规避完整 GET 风控）
async fn download_file(client: &Client, url: &str, dest: &Path) -> AppResult<()> {
    let resp = client
        .get(url)
        .header("User-Agent", crate::baidupcs::UA)
        .header("Range", "bytes=0-")
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Api(format!("数据处理失败：HTTP {status}")));
    }
    let mut file = std::fs::File::create(dest)?;
    let mut stream = resp.bytes_stream();
    let mut size: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes)?;
        size += bytes.len() as u64;
    }
    if size < 1024 * 512 {
        return Err(AppError::Api("更新数据处理不完整".into()));
    }
    Ok(())
}

/// 从更新包中提取密码记录并解析（支持 RAR4/RAR5；txt 为 UTF-8）
fn extract_password(rar: &Path) -> AppResult<String> {
    let archive = unrar::Archive::new(rar)
        .open_for_processing()
        .map_err(|e| AppError::Api(format!("更新数据打开失败：{e}")))?;
    let mut cur = archive;
    loop {
        let header = match cur
            .read_header()
            .map_err(|e| AppError::Api(format!("更新数据读取失败：{e}")))?
        {
            Some(h) => h,
            None => break,
        };
        let name = header.entry().filename.to_string_lossy().to_string();
        if name.to_lowercase().ends_with(".txt") {
            let (data, next) = header
                .read()
                .map_err(|e| AppError::Api(format!("密码文件读取失败：{e}")))?;
            let text = String::from_utf8_lossy(&data);
            if let Some(pwd) = parse_password_txt(&text) {
                return Ok(pwd);
            }
            cur = next;
        } else {
            cur = header
                .skip()
                .map_err(|e| AppError::Api(format!("更新数据处理失败：{e}")))?;
        }
    }
    Err(AppError::Api("更新数据中未找到密码记录".into()))
}

/// 解析密码 txt：首选「最新密码：X」；回退第一条「解析密码：X」（历史记录新→旧排列）
pub fn parse_password_txt(text: &str) -> Option<String> {
    let mut latest: Option<String> = None;
    let mut history: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        let after = |prefix: &str| -> Option<&str> {
            line.strip_prefix(prefix)
                .map(|rest| rest.strip_prefix('：').or_else(|| rest.strip_prefix(':')).unwrap_or(rest))
        };
        if let Some(v) = after("最新密码") {
            let v = v.trim();
            if !v.is_empty() {
                latest = Some(v.to_string());
            }
        } else if let Some(pos) = line.find("解析密码") {
            let rest = &line[pos + "解析密码".len()..];
            if let Some(v) = rest.strip_prefix('：').or_else(|| rest.strip_prefix(':')) {
                let v = v.trim();
                if !v.is_empty() && history.is_none() {
                    history = Some(v.to_string());
                }
            }
        }
    }
    latest.or(history)
}