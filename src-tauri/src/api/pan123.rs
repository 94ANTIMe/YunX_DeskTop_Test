//! 123 云盘 API（移植 Android Pan123Api + Pan123Constants）。
//! 分享列表匿名；分享下载需登录 JWT + auth-key/auth-value 签名（CRC32 派生）；
//! download-v2 params Base64 解码 + redirect_url 跟随得真实 CDN 直链。
use reqwest::Client;
use serde_json::{json, Value};

use super::{b64_decode, crc32_hex, random_hex};
use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

pub const WEB_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36";
pub const DART_UA: &str = "Dart/3.12 (dart:io)";
pub const DOWNLOAD_REFERER: &str = "https://yun.123pan.cn/";
pub const ACCOUNT_INFO_URL: &str = "https://yun.123pan.cn/b/api/user/info";
const LOGIN_URL: &str = "https://user.123pan.cn/api/user/sign_in";
const SHARE_GET_URL: &str = "https://yun.123pan.cn/b/api/share/get";
const SHARE_DOWNLOAD_INFO_URL: &str = "https://www.123865.com/b/api/share/download/info";
const SIGN_TABLE: &[u8] = b"adefghlmyijnopkqrstubcvwsz";
const SIGN_OFFSET_SECONDS: i64 = 57600;

fn str_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn i64_or(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn check_ok(v: &Value, fallback: &str) -> AppResult<()> {
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code == 0 {
        return Ok(());
    }
    let msg = v.get("message").and_then(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or(fallback);
    Err(AppError::Api(format!("{msg}（code={code}）")))
}

// ---------- 签名（auth-key / auth-value） ----------

/// 生成签名头（timeSign = crc32(替换表映射后的 UTC YYYYMMDDHHmm，ts+16h)；
/// auth-value = "ts-rand-crc32(ts|rand|path|web|3|auth_key)"）
fn make_sign(path: &str, ts: i64) -> (String, String) {
    let dt = chrono::DateTime::from_timestamp(ts + SIGN_OFFSET_SECONDS, 0)
        .expect("invalid timestamp")
        .format("%Y%m%d%H%M")
        .to_string();
    let substituted: String = dt
        .bytes()
        .map(|b| SIGN_TABLE[(b - b'0') as usize] as char)
        .collect();
    let auth_key = crc32_hex(&substituted);
    let random = {
        use rand::Rng;
        rand::thread_rng().gen_range(0..10_000_000i64)
    };
    let data = format!("{ts}|{random}|{path}|web|3|{auth_key}");
    let auth_value = format!("{ts}-{random}-{}", crc32_hex(&data));
    (auth_key, auth_value)
}

fn login_uuid() -> String {
    random_hex(32)
}

// ---------- 登录 / 账号 ----------

/// 账号密码登录 → JWT
pub async fn login(client: &Client, passport: &str, password: &str) -> AppResult<String> {
    let body = json!({ "passport": passport, "password": password, "remember": false });
    let resp = client
        .post(LOGIN_URL)
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("platform", "web")
        .header("app-version", "132")
        .header("loginuuid", login_uuid())
        .header("Origin", "https://user.123pan.cn")
        .header("Referer", "https://user.123pan.cn/centerlogin?redirect_url=&source_page=website")
        .header("User-Agent", WEB_UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 200 {
        let msg = v.get("message").and_then(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or("登录失败");
        return Err(AppError::Api(format!("{msg}（code={code}）")));
    }
    let token = v.pointer("/data/token").and_then(|x| x.as_str()).unwrap_or("");
    if token.is_empty() {
        return Err(AppError::Api("登录失败：未返回 token".into()));
    }
    Ok(token.to_string())
}

/// 昵称（校验登录态）
pub async fn fetch_nickname(client: &Client, token: &str) -> AppResult<String> {
    let (ak, av) = make_sign("/b/api/user/info", chrono::Utc::now().timestamp());
    let resp = client
        .get(ACCOUNT_INFO_URL)
        .header("platform", "web")
        .header("app-version", "3")
        .header("authorization", format!("Bearer {token}"))
        .header("loginuuid", login_uuid())
        .header("auth-key", ak)
        .header("auth-value", av)
        .header("User-Agent", WEB_UA)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_ok(&v, "获取用户信息失败")?;
    let nick = v.pointer("/data/Nickname").and_then(|x| x.as_str()).unwrap_or("");
    if nick.is_empty() {
        return Err(AppError::Api("登录态无效或已过期".into()));
    }
    Ok(nick.to_string())
}

// ---------- 分享解析 ----------

/// 分享文件列表（匿名；返回 (列表, 下一页游标 or None)）
pub async fn get_share_files(
    client: &Client,
    share_key: &str,
    share_pwd: &str,
    parent_file_id: &str,
    next: &str,
    page: i64,
) -> AppResult<(Vec<ShareFile>, Option<String>)> {
    let mut url = format!(
        "{SHARE_GET_URL}?limit=100&next={next}&orderBy=file_name&orderDirection=asc&shareKey={}&ParentFileId={parent_file_id}&Page={page}",
        urlencoding::encode(share_key)
    );
    if !share_pwd.is_empty() {
        url.push_str(&format!("&SharePwd={}", urlencoding::encode(share_pwd)));
    }
    let resp = client
        .get(&url)
        .header("User-Agent", DART_UA)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_ok(&v, "获取文件列表失败")?;
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    if data.get("Expired").and_then(|e| e.as_bool()).unwrap_or(false) {
        return Err(AppError::Api("分享已失效".into()));
    }
    let files = parse_info_list(&data);
    let next_cursor = str_or(&data, "Next");
    let next_cursor = if next_cursor == "-1" { None } else if next_cursor.is_empty() { Some(String::new()) } else { Some(next_cursor) };
    Ok((files, next_cursor))
}

/// 分享下载直链（需登录 JWT；download-v2 解码 + redirect_url 跟随）
pub async fn get_share_download_link(
    client: &Client,
    share_key: &str,
    file: &ShareFile,
    token: &str,
) -> AppResult<(String, String, i64)> {
    let (s3_key_flag, etag, _) = decode_token(&file.fid_token);
    let body = json!({
        "ShareKey": share_key,
        "FileID": file.fid,
        "S3KeyFlag": s3_key_flag,
        "Size": file.fsize,
        "Etag": etag,
    });
    let (ak, av) = make_sign("/b/api/share/download/info", chrono::Utc::now().timestamp());
    let resp = client
        .post(SHARE_DOWNLOAD_INFO_URL)
        .header("platform", "android")
        .header("app-version", "39")
        .header("authorization", format!("Bearer {token}"))
        .header("loginuuid", login_uuid())
        .header("auth-key", ak)
        .header("auth-value", av)
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("User-Agent", WEB_UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_ok(&v, "获取下载链接失败")?;
    let raw = v.pointer("/data/DownloadURL").and_then(|x| x.as_str()).unwrap_or("");
    if raw.is_empty() {
        return Err(AppError::Api("未返回下载链接".into()));
    }
    let decoded = decode_download_url(raw).unwrap_or_else(|| raw.to_string());
    let real = follow_redirect_url(client, &decoded).await?;
    Ok((real, file.fname.clone(), file.fsize))
}

// ---------- 内部工具 ----------

fn parse_info_list(data: &Value) -> Vec<ShareFile> {
    let arr = data.get("InfoList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    arr.iter()
        .filter_map(|item| {
            let ftype = i64_or(item, "Type");
            Some(ShareFile {
                fid: str_or(item, "FileId"),
                fname: str_or(item, "FileName"),
                fsize: i64_or(item, "Size"),
                isdir: ftype == 1,
                pdir_fid: str_or(item, "ParentFileId"),
                fid_token: format!(
                    "{}|{}|{}",
                    str_or(item, "S3KeyFlag"),
                    str_or(item, "Etag"),
                    str_or(item, "StorageNode")
                ),
                modify_time: str_or(item, "UpdateAt"),
            })
        })
        .filter(|f| !f.fid.is_empty() || !f.fname.is_empty())
        .collect()
}

/// 解码 fidToken（"S3KeyFlag|Etag|StorageNode"）
fn decode_token(fid_token: &str) -> (String, String, String) {
    let parts: Vec<&str> = fid_token.split('|').collect();
    (
        parts.first().copied().unwrap_or("").to_string(),
        parts.get(1).copied().unwrap_or("").to_string(),
        parts.get(2).copied().unwrap_or("").to_string(),
    )
}

/// 解码 123 下载 URL（整段 base64 或 download-v2?params=<base64 URL-safe>）
fn decode_download_url(download_url: &str) -> Option<String> {
    let trimmed = download_url.trim();
    // 形态 1：整段 base64（不含协议头）
    if !trimmed.contains("://") {
        let decoded = String::from_utf8(b64_decode(trimmed)?).ok()?;
        return decoded.starts_with("http").then_some(decoded);
    }
    // 形态 2：download-v2?params=<base64>
    let idx = trimmed.find("params=")?;
    let params = trimmed[idx + "params=".len()..].split('&').next()?;
    let decoded = String::from_utf8(b64_decode(params)?).ok()?;
    Some(decoded)
}

/// 跟随 CDN 的 redirect_url（JSON 跳转页，最多 5 跳）
async fn follow_redirect_url(client: &Client, initial_url: &str) -> AppResult<String> {
    let mut url = initial_url.to_string();
    for _ in 0..5 {
        let next = probe_json_redirect(client, &url).await;
        match next {
            Some(n) => url = n,
            None => return Ok(url),
        }
    }
    Ok(url)
}

/// 探测单跳：小 JSON 响应含 data.redirect_url 时返回新地址
async fn probe_json_redirect(client: &Client, url: &str) -> Option<String> {
    let resp = client
        .get(url)
        .header("Referer", DOWNLOAD_REFERER)
        .header("User-Agent", DART_UA)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    let len = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);
    if len >= 0 && len <= 8192 {
        let body = resp.text().await.ok()?;
        if body.trim_start().starts_with('{') {
            let v: Value = serde_json::from_str(&body).ok()?;
            return v
                .pointer("/data/redirect_url")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
        }
    }
    None
}
