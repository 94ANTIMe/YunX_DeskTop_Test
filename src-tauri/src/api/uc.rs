//! UC 网盘 API（移植 Android UCApi + UCConstants）。
//! 链路：getShareToken → 分享列表 → 直接取分享下载直链（官方抓包：无需转存）。
use reqwest::Client;
use serde_json::{json, Value};

use super::{merge_puus, without_puus};
use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const DOWNLOAD_REFERER: &str = "https://drive.uc.cn/";
pub const ACCOUNT_INFO_URL: &str = "https://drive.uc.cn/account/info";
const SHARE_TOKEN_URL: &str = "https://pc-api.uc.cn/1/clouddrive/share/sharepage/token?pr=UCBrowser&fr=pc";
const TRANSFER_SHARE_DETAIL_URL: &str = "https://pc-api.uc.cn/1/clouddrive/transfer_share/detail?entry=ft&fr=pc&pr=UCBrowser";
const DOWNLOAD_URL: &str = "https://pc-api.uc.cn/1/clouddrive/file/download?entry=ft&fr=pc&pr=UCBrowser";
const CONFIG_URL: &str = "https://pc-api.uc.cn/1/clouddrive/config?pr=UCBrowser&fr=pc";

fn set_cookies(resp: &reqwest::Response) -> Vec<String> {
    resp.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(String::from))
        .collect()
}

fn check_status<'a>(v: &'a Value, fallback: &str) -> AppResult<&'a Value> {
    let status = v.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
    if status != 200 {
        let msg = v.get("message").and_then(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or(fallback);
        return Err(AppError::Api(msg.to_string()));
    }
    v.get("data").ok_or_else(|| AppError::Api("响应缺少 data".into()))
}

fn str_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn i64_or(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn bool_or(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}

fn parse_file(item: &Value) -> Option<ShareFile> {
    Some(ShareFile {
        fid: str_or(item, "fid"),
        fname: str_or(item, "file_name"),
        fsize: i64_or(item, "size"),
        isdir: bool_or(item, "dir"),
        pdir_fid: str_or(item, "pdir_fid"),
        fid_token: str_or(item, "share_fid_token"),
        modify_time: str_or(item, "updated_at"),
    })
    .filter(|f| !f.fid.is_empty() || !f.fname.is_empty())
}

/// 账号昵称（登录验证用）
pub async fn fetch_nickname(client: &Client, cookie: &str) -> AppResult<String> {
    let resp = client
        .get(ACCOUNT_INFO_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        let nick = v.pointer("/data/nickname").and_then(|x| x.as_str()).unwrap_or("");
        if !nick.is_empty() {
            return Ok(nick.to_string());
        }
    }
    Err(AppError::Api("Cookie 无效或已过期".into()))
}

/// 登录态判定（与夸克同源 __pus/__puus）
pub fn is_valid_cookie(cookie: &str) -> bool {
    cookie.contains("__pus=") && cookie.contains("__puus=")
}

/// 刷新会话 Cookie（同夸克 refreshPuus）
pub async fn refresh_session(client: &Client, cookie: &str) -> AppResult<String> {
    let resp = client
        .get(CONFIG_URL)
        .header("Cookie", without_puus(cookie))
        .header("User-Agent", UA)
        .header("Referer", DOWNLOAD_REFERER)
        .send()
        .await?;
    let merged = merge_puus(cookie, &set_cookies(&resp));
    Ok(merged)
}

/// 分享 Token（stoken + 标题）
pub async fn get_share_token(client: &Client, share_id: &str, pwd: &str, cookie: &str) -> AppResult<(String, String)> {
    let body = json!({
        "pwd_id": share_id,
        "passcode": pwd,
        "share_for_transfer": true,
    });
    let resp = client
        .post(SHARE_TOKEN_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "获取分享 Token 失败")?;
    Ok((str_or(data, "stoken"), str_or(data, "title")))
}

/// 转存分享详情列表（GET + stoken；返回的 share_fid_token 与 stoken 绑定，取链用）
pub async fn get_transfer_share_files(
    client: &Client,
    share_id: &str,
    stoken: &str,
    pdir_fid: &str,
    cookie: &str,
    page: i64,
    size: i64,
) -> AppResult<Vec<ShareFile>> {
    let url = format!(
        "{TRANSFER_SHARE_DETAIL_URL}&pwd_id={share_id}&pdir_fid={pdir_fid}&fetch_file_list=1&passcode=&_page={page}&_size={size}&_fetch_total=1&_fetch_task=1&_fetch_share=1&_sort=&stoken={}",
        urlencoding::encode(stoken)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .header("Origin", "https://fast.uc.cn")
        .header("Referer", "https://fast.uc.cn/")
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "获取分享文件列表失败")?;
    let list = data
        .get("list")
        .and_then(|l| l.as_array())
        .cloned()
        .or_else(|| {
            data.pointer("/detail_info/list")
                .and_then(|l| l.as_array())
                .cloned()
        })
        .unwrap_or_default();
    Ok(list.iter().filter_map(parse_file).filter(|f| !f.fid.is_empty()).collect())
}

/// 分享下载直链（官方抓包：无需转存，fids + pwd_id + stoken + fids_token 直取）
pub async fn get_share_download_link(
    client: &Client,
    fid: &str,
    fid_token: &str,
    stoken: &str,
    pwd_id: &str,
    cookie: &str,
) -> AppResult<(String, String, i64)> {
    let body = json!({
        "fids": [fid],
        "pwd_id": pwd_id,
        "stoken": stoken,
        "fids_token": [fid_token],
    });
    let resp = client
        .post(DOWNLOAD_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let status = v.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
    if status != 200 {
        let msg = v.get("message").and_then(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or("获取下载链接失败");
        return Err(AppError::Api(msg.to_string()));
    }
    let item = v
        .pointer("/data/0")
        .ok_or_else(|| AppError::Api("未返回下载链接".into()))?;
    let url = str_or(item, "download_url");
    if url.is_empty() {
        return Err(AppError::Api("未返回下载链接".into()));
    }
    let filename = {
        let n = str_or(item, "file_name");
        if n.is_empty() { str_or(item, "filename") } else { n }
    };
    Ok((url, filename, i64_or(item, "size")))
}
