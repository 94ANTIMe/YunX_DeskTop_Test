//! 139 网盘（和彩云）API（移植 Android C139Api + C139Constants）。
//! 分享接口：share-kd-njs.yun.139.com，请求/响应经 AES-CBC（固定密钥 + IV 前置）加密；
//! mcloud-sign 按明文 body 计算；匿名列目录（hcy-cool-flag=1），下载需登录态 authorization。
use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use reqwest::Client;
use serde_json::{json, Value};

use super::{b64_decode, b64_encode, md5_hex, random_alnum};
use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

pub const SHARE_AES_KEY: &str = "PVGDwmcvfs1uV3d1";
const SHARE_BASE: &str = "https://share-kd-njs.yun.139.com";
const SHARE_LIST_URL: &str = "https://share-kd-njs.yun.139.com/yun-share/richlifeApp/devapp/IOutLink/getOutLinkInfoV6";
const SHARE_LINK_URL: &str = "https://share-kd-njs.yun.139.com/yun-share/richlifeApp/devapp/IOutLink/dlFromOutLinkV3";
const SHARE_GENERAL_URL: &str = "https://share-kd-njs.yun.139.com/yun-share/richlifeApp/devapp/IOutLink/getOutLinkGeneral";
pub const SHARE_MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36";
const SHARE_X_DEVICEINFO: &str = "||3|12.27.0|||||chrome 150.0.0.0|360X444|zh-cn|||";
const SHARE_X_HUAWEI_CHANNELSRC: &str = "10245500";
const SHARE_X_MM_SOURCE: &str = "0002";

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

fn str_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn i64_or(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

// ---------- mcloud-sign（按明文 body 计算） ----------

/// calSign：encodeURIComponent → 单字符升序 → base64 → md5(b64)+md5(ts:rand) → md5 → 大写
fn cal_sign(body_json: &str, ts: &str, rand: &str) -> String {
    let encoded = urlencoding::encode(body_json).to_string();
    let mut sorted: Vec<char> = encoded.chars().collect();
    sorted.sort();
    let sorted_str: String = sorted.into_iter().collect();
    let b64 = b64_encode(sorted_str.as_bytes());
    let res = format!("{}{}", md5_hex(&b64), md5_hex(&format!("{ts}:{rand}")));
    md5_hex(&res).to_uppercase()
}

/// 生成 mcloud-sign 头：`<ts>,<rand>,<sign>`；ts 格式 YYYY-MM-DD HH:MM:SS
fn sign_header(body_json: &str) -> String {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand = random_alnum(16);
    format!("{ts},{rand},{}", cal_sign(body_json, &ts, &rand))
}

// ---------- AES-CBC 加解密（固定密钥 + IV 前置；解密后可能 gzip） ----------

fn encrypt_body(plaintext: &str) -> String {
    let iv: [u8; 16] = rand::random();
    let key: &[u8; 16] = SHARE_AES_KEY.as_bytes().try_into().expect("AES key length");
    let ct = Aes128CbcEnc::new(key.into(), (&iv).into())
        .encrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(plaintext.as_bytes());
    let mut raw = iv.to_vec();
    raw.extend_from_slice(&ct);
    b64_encode(&raw)
}

fn decrypt_body(b64: &str) -> Option<String> {
    use std::io::Read;
    let raw = b64_decode(b64)?;
    if raw.len() < 16 {
        return None;
    }
    let (iv, ct) = raw.split_at(16);
    let iv: [u8; 16] = iv.try_into().ok()?;
    let key: &[u8; 16] = SHARE_AES_KEY.as_bytes().try_into().expect("AES key length");
    let pt = Aes128CbcDec::new(key.into(), (&iv).into())
        .decrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(ct)
        .ok()?;
    // 解密后若 gzip（首 2 字节 0x1f 0x8b）先解压
    if pt.len() > 2 && pt[0] == 0x1f && pt[1] == 0x8b {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(&pt[..]).read_to_end(&mut out).ok()?;
        return String::from_utf8(out).ok();
    }
    String::from_utf8(pt).ok()
}

// ---------- 登录态工具 ----------

/// 从 authorization（"Basic base64(pc:账号:authToken)"）解码账号
pub fn account_from_authorization(authorization: &str) -> Option<String> {
    let b64 = authorization.trim().strip_prefix("Basic").unwrap_or(authorization).trim();
    let decoded = b64_decode(b64)?;
    let s = String::from_utf8(decoded).ok()?;
    s.split(':').nth(1).filter(|x| !x.is_empty()).map(String::from)
}

/// 从 Cookie 提取 authorization（形如 "Basic cGM6..."）
pub fn extract_authorization(cookie: &str) -> Option<String> {
    cookie.split(';').map(|s| s.trim()).find_map(|kv| {
        let v = kv.strip_prefix("authorization=")?;
        if v.is_empty() { None } else { Some(v.to_string()) }
    })
}

/// 登录态判定：authorization 存在（路径 B）或 Os_SSo_Sid + RMKEY（路径 A）
pub fn is_valid_cookie(cookie: &str) -> bool {
    if extract_authorization(cookie).is_some() {
        return true;
    }
    let has = |key: &str| {
        cookie.split(';').any(|kv| {
            let kv = kv.trim();
            kv.starts_with(key) && kv.len() > key.len()
        })
    };
    has("Os_SSo_Sid=") && has("RMKEY=")
}

/// 从 Cookie 提取完整账号（ORCHES-I-ACCOUNT-ENCRYPT base64 → authorization → Login_UserNumber）
pub fn extract_account_full(cookie: &str) -> Option<String> {
    for kv in cookie.split(';') {
        let kv = kv.trim();
        if let Some(v) = kv.strip_prefix("ORCHES-I-ACCOUNT-ENCRYPT=") {
            if !v.is_empty() {
                if let Some(bytes) = b64_decode(v) {
                    if let Ok(s) = String::from_utf8(bytes) {
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
        }
    }
    if let Some(auth) = extract_authorization(cookie) {
        if let Some(acc) = account_from_authorization(&auth) {
            return Some(acc);
        }
    }
    for kv in cookie.split(';') {
        let kv = kv.trim();
        if let Some(v) = kv.strip_prefix("Login_UserNumber=") {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 从 Cookie 提取脱敏昵称（ORCHES-I-ACCOUNT-SIMPLIFY → ENCRYPT → Login_UserNumber）
pub fn extract_nickname(cookie: &str) -> Option<String> {
    for key in ["ORCHES-I-ACCOUNT-SIMPLIFY=", "Login_UserNumber="] {
        for kv in cookie.split(';') {
            let kv = kv.trim();
            if let Some(v) = kv.strip_prefix(key) {
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    extract_account_full(cookie)
}

// ---------- 分享接口 ----------

/// 匿名 POST（列表端点专用：不带鉴权，body 加密，hcy-cool-flag=1）
async fn share_post_anonymous(client: &Client, url: &str, plain_body: &str) -> AppResult<Value> {
    let encrypted = encrypt_body(plain_body);
    let resp = client
        .post(url)
        .header("hcy-cool-flag", "1")
        .header("x-deviceinfo", SHARE_X_DEVICEINFO)
        .header("x-huawei-channelsrc", SHARE_X_HUAWEI_CHANNELSRC)
        .header("x-mm-source", SHARE_X_MM_SOURCE)
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("User-Agent", SHARE_MOBILE_UA)
        .header("Origin", "https://yun.139.com")
        .header("Referer", "https://yun.139.com/")
        .body(encrypted)
        .send()
        .await?;
    let body = resp.text().await?;
    // 响应应为加密 base64；网关透传明文时兜底
    Ok(serde_json::from_str(&body).or_else(|_| {
        decrypt_body(&body)
            .and_then(|p| serde_json::from_str(&p).ok())
            .ok_or_else(|| AppError::Api("响应解析失败".into()))
    })?)
}

/// 分享 POST（带鉴权 + mcloud-sign；body 加密）
async fn share_post_encrypted(
    client: &Client,
    url: &str,
    plain_body: &str,
    authorization: Option<&str>,
) -> AppResult<Value> {
    let encrypted = encrypt_body(plain_body);
    let mut req = client
        .post(url)
        .header("hcy-cool-flag", "1")
        .header("x-deviceinfo", SHARE_X_DEVICEINFO)
        .header("x-huawei-channelsrc", SHARE_X_HUAWEI_CHANNELSRC)
        .header("x-mm-source", SHARE_X_MM_SOURCE)
        .header("mcloud-sign", sign_header(plain_body))
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("User-Agent", SHARE_MOBILE_UA)
        .header("Origin", "https://yun.139.com")
        .header("Referer", "https://yun.139.com/")
        .body(encrypted);
    if let Some(auth) = authorization.filter(|a| !a.is_empty()) {
        req = req.header("Authorization", auth);
    }
    let resp = req.send().await?;
    let body = resp.text().await?;
    Ok(serde_json::from_str(&body).or_else(|_| {
        decrypt_body(&body)
            .and_then(|p| serde_json::from_str(&p).ok())
            .ok_or_else(|| AppError::Api("响应解析失败".into()))
    })?)
}

/// 分享标题（getOutLinkGeneral 匿名）
pub async fn get_out_link_title(client: &Client, link_id: &str) -> AppResult<String> {
    let plain = json!({ "getOutLinkGeneralReq": { "linkID": link_id, "isPasswd": 1, "account": "" } });
    let v = share_post_anonymous(client, SHARE_GENERAL_URL, &plain.to_string()).await?;
    let title = v
        .pointer("/data/getOutLinkGeneralResp/outLinkGeneral/0/lkName")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    Ok(title.to_string())
}

/// 分享明文提取码（139 官方会明文回吐，用于自动填充）
pub async fn get_out_link_password(client: &Client, link_id: &str) -> AppResult<String> {
    let plain = json!({ "getOutLinkGeneralReq": { "linkID": link_id, "isPasswd": 1, "account": "" } });
    let v = share_post_anonymous(client, SHARE_GENERAL_URL, &plain.to_string()).await?;
    let pwd = v
        .pointer("/data/getOutLinkGeneralResp/outLinkGeneral/0/passwd")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    Ok(pwd.to_string())
}

/// 分享列目录（匿名，caLst 子文件夹 + coLst 文件/嵌套文件夹合并）
pub async fn get_share_files(
    client: &Client,
    link_id: &str,
    pca_id: &str,
    passwd: &str,
    begin: i64,
    end: i64,
) -> AppResult<Vec<ShareFile>> {
    let plain = json!({
        "getOutLinkInfoReq": {
            "account": "",
            "linkID": link_id,
            "passwd": passwd,
            "caSrt": 1,
            "coSrt": 1,
            "srtDr": 0,
            "bNum": begin,
            "pCaID": pca_id,
            "eNum": end,
        }
    });
    let v = share_post_anonymous(client, SHARE_LIST_URL, &plain.to_string()).await?;
    let code = str_or(&v, "resultCode");
    if !code.is_empty() && code != "0" {
        let desc = str_or(&v, "desc");
        return Err(AppError::Api(if desc.is_empty() {
            format!("获取文件列表失败（{code}）")
        } else {
            desc
        }));
    }
    if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(true) {
        let desc = str_or(&v, "desc");
        return Err(AppError::Api(if desc.is_empty() { "获取文件列表失败".into() } else { desc }));
    }
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    let mut files = Vec::new();
    // 1) 子文件夹 caLst
    if let Some(ca) = data.get("caLst").and_then(|l| l.as_array()) {
        for item in ca {
            files.push(ShareFile {
                fid: str_or(item, "caID"),
                fname: str_or(item, "caName"),
                fsize: 0,
                isdir: true,
                pdir_fid: pca_id.to_string(),
                fid_token: String::new(),
                modify_time: str_or(item, "udTime"),
            });
        }
    }
    // 2) 文件列表 coLst（含 coType==2 的文件夹）
    if let Some(co) = data.get("coLst").and_then(|l| l.as_array()) {
        for item in co {
            let isdir = item
                .get("isdir")
                .and_then(|x| x.as_bool())
                .unwrap_or_else(|| i64_or(item, "coType") == 2);
            files.push(ShareFile {
                fid: str_or(item, "coID"),
                fname: str_or(item, "coName"),
                fsize: i64_or(item, "coSize"),
                isdir,
                pdir_fid: pca_id.to_string(),
                fid_token: String::new(),
                modify_time: str_or(item, "udTime"),
            });
        }
    }
    Ok(files.into_iter().filter(|f| !f.fid.is_empty() || !f.fname.is_empty()).collect())
}

/// 分享下载直链（dlFromOutLinkV3 → data.redrUrl OBS 直链，900s 有效）
pub async fn get_share_download_link(
    client: &Client,
    co_id: &str,
    link_id: &str,
    account: &str,
    authorization: Option<&str>,
) -> AppResult<(String, String, i64)> {
    let plain = json!({
        "dlFromOutLinkReqV3": {
            "account": account,
            "linkID": link_id,
            "coIDLst": { "item": [co_id] },
            "commonAccountInfo": { "account": account, "accountType": 1 },
        }
    });
    let v = share_post_encrypted(client, SHARE_LINK_URL, &plain.to_string(), authorization).await?;
    let code = str_or(&v, "resultCode");
    if !code.is_empty() && code != "0" {
        let desc = str_or(&v, "desc");
        return Err(AppError::Api(if desc.is_empty() {
            format!("获取下载链接失败（{code}）")
        } else {
            desc
        }));
    }
    if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(true) {
        let desc = str_or(&v, "desc");
        return Err(AppError::Api(if desc.is_empty() { "获取下载链接失败".into() } else { desc }));
    }
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    let url = str_or(&data, "redrUrl");
    if url.is_empty() {
        return Err(AppError::Api("未返回下载链接".into()));
    }
    let filename = {
        let n = str_or(&data, "fileName");
        if n.is_empty() {
            let n2 = str_or(&data, "coName");
            if n2.is_empty() { co_id.to_string() } else { n2 }
        } else {
            n
        }
    };
    let size = i64_or(&data, "coSize");
    Ok((url, filename, size))
}

// 保留 base 常量引用（后续转存/管理功能用）
#[allow(dead_code)]
const _SHARE_BASE: &str = SHARE_BASE;
