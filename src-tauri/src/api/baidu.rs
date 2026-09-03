//! 百度网盘 API（移植 Android BaiduApi + BaiduConstants）。
//! 链路：share/verify → xpan/share list → share/transfer 转存 → locatedownload 取链 → filemanager delete 清理。
use reqwest::Client;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

pub const UA_WEB: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
pub const UA_NETDISK: &str = "netdisk;12.24.6;piano;android-android;16;JSbridge4.4.0;jointBridge;1.1.0";
pub const ACCOUNT_INFO_URL: &str = "https://pan.baidu.com/api/gettemplatevariable";
const APP_ID: &str = "250528";
pub const TEMP_DIR_NAME: &str = "YunX临时转存";

fn str_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// 取 id 字段（fs_id 等可能是字符串或数字，统一转字符串）
fn id_or(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn i64_or(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn check_errno(v: &Value, fallback: &str) -> AppResult<()> {
    let errno = v.get("errno").and_then(|e| e.as_i64()).unwrap_or(-1);
    if errno != 0 {
        let msg = v
            .get("err_msg")
            .and_then(|m| m.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| v.get("show_msg").and_then(|m| m.as_str()).filter(|s| !s.is_empty()))
            .unwrap_or(fallback);
        return Err(AppError::Api(format!("{msg}（errno={errno}）")));
    }
    Ok(())
}

/// 昵称（gettemplatevariable username）
pub async fn fetch_nickname(client: &Client, cookie: &str) -> AppResult<String> {
    let url = format!(
        "{ACCOUNT_INFO_URL}?clienttype=0&app_id={APP_ID}&web=1&fields={}",
        urlencoding::encode(r#"["username"]"#)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA_WEB)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "获取账号信息失败")?;
    let nick = v.pointer("/result/username").and_then(|x| x.as_str()).unwrap_or("");
    if nick.is_empty() {
        return Err(AppError::Api("Cookie 无效或已过期".into()));
    }
    Ok(nick.to_string())
}

/// 登录态判定（BDUSS 存在）
pub fn is_valid_cookie(cookie: &str) -> bool {
    cookie.contains("BDUSS=")
}

/// bdstoken（transfer/create 用）
async fn get_bdstoken(client: &Client, cookie: &str) -> AppResult<String> {
    let url = format!(
        "{ACCOUNT_INFO_URL}?clienttype=0&app_id={APP_ID}&web=1&fields={}",
        urlencoding::encode(r#"["bdstoken"]"#)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA_WEB)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "获取 bdstoken 失败")?;
    let token = v.pointer("/result/bdstoken").and_then(|x| x.as_str()).unwrap_or("");
    if token.is_empty() {
        return Err(AppError::Api("获取 bdstoken 失败，请重新登录".into()));
    }
    Ok(token.to_string())
}

/// 分享会话（PCS 式验证产物；share_id/uk 供转存用）
pub struct ShareSession {
    pub randsk: String,
    pub share_id: String,
    pub uk: String,
}

/// 验证提取码 → 分享会话（PCS 客户端式）。
/// 流程：GET 分享页（netdisk UA）提取 shareid/share_uk → POST verify（clienttype=1）→ randsk。
/// 该式可绕过网页式 verify 的 105 验证码风控（实测 2026-09），且转存仅在此会话下可用。
pub async fn verify_share_pcs(
    client: &Client,
    surl: &str,
    pwd: &str,
    cookie: &str,
) -> AppResult<ShareSession> {
    let surl = surl.trim_start_matches('1');
    // 1. 分享页（netdisk UA；未验证状态会重定向到 init 页，跟随即可）
    let page = client
        .get(format!("https://pan.baidu.com/s/1{surl}"))
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Referer", "https://pan.baidu.com/disk/home")
        .send()
        .await?
        .text()
        .await?;
    let num_of = |key: &str| -> String {
        page.split(&format!("\"{key}"))
            .nth(1)
            .and_then(|rest| rest.split(':').nth(1))
            .map(|rest| {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '"').collect();
                digits.trim_matches('"').to_string()
            })
            .unwrap_or_default()
    };
    let share_id = num_of("shareid");
    let share_uk = num_of("share_uk");
    if share_id.is_empty() || share_uk.is_empty() {
        return Err(AppError::Api("分享页解析失败（分享可能已失效）".into()));
    }
    // 2. bdstoken + verify（clienttype=1 式）
    let bdstoken = get_bdstoken(client, cookie).await?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let resp = client
        .post(format!(
            "https://pan.baidu.com/share/verify?shareid={share_id}&time={ts}&clienttype=1&uk={share_uk}"
        ))
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Referer", format!("https://pan.baidu.com/s/1{surl}"))
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(format!("pwd={}&vcode=null&vcode_str=null&bdstoken={bdstoken}", urlencoding::encode(pwd)))
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "验证提取码失败")?;
    let randsk = str_or(&v, "randsk");
    if randsk.is_empty() {
        return Err(AppError::Api("未返回分享密钥".into()));
    }
    Ok(ShareSession { randsk, share_id, uk: share_uk })
}

/// 百度分享列表结果
pub struct BaiduShareList {
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 列出分享文件（旧版 /share/list 接口 + BDCLND cookie）。
/// 仅 PCS 式验证会话可用（xpan 接口对 clienttype=1 会话返回 errno=140，实测 2026-09）。
/// 子目录导航用 dir 参数（取目录条目的 path 字段值）。
pub async fn list_share(
    client: &Client,
    surl: &str,
    sekey: &str,
    dir: &str,
    cookie: &str,
    page: i64,
) -> AppResult<BaiduShareList> {
    let surl = surl.trim_start_matches('1');
    let is_root = dir.is_empty() || dir == "/";
    let mut url = format!(
        "https://pan.baidu.com/share/list?bdstoken={}&web=5&app_id={APP_ID}&shorturl={surl}&channel=chunlei&page={page}&num=100",
        get_bdstoken(client, cookie).await?
    );
    if is_root {
        url.push_str("&root=1");
    } else {
        url.push_str(&format!("&root=0&dir={}", urlencoding::encode(dir)));
    }
    let auth_cookie = if sekey.is_empty() || cookie.contains("BDCLND=") {
        cookie.to_string()
    } else {
        format!("{cookie}; BDCLND={sekey}")
    };
    let resp = client
        .get(&url)
        .header("Cookie", auth_cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Referer", format!("https://pan.baidu.com/s/1{surl}"))
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "获取分享文件列表失败")?;
    let list = v.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let mut files = Vec::new();
    for item in &list {
        let isdir = item.get("isdir").and_then(|x| x.as_i64()) == Some(1)
            || item.get("isdir").and_then(|x| x.as_str()) == Some("1");
        let path = str_or(item, "path");
        files.push(ShareFile {
            // 目录用 path 作 fid（导航传参），文件用 fs_id（转存传参）
            fid: if isdir { path.clone() } else { id_or(item, "fs_id") },
            fname: str_or(item, "server_filename"),
            fsize: i64_or(item, "size"),
            isdir,
            pdir_fid: path.clone(),
            fid_token: path,
            modify_time: str_or(item, "server_mtime"),
        });
    }
    let has_more = files.len() >= 100;
    Ok(BaiduShareList { files, has_more })
}

/// 列出个人网盘目录（检查临时转存目录），返回子项 path
async fn list_dir(client: &Client, dir: &str, cookie: &str) -> AppResult<Vec<String>> {
    let bdstoken = get_bdstoken(client, cookie).await?;
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&bdstoken={bdstoken}&web=1&app_id={APP_ID}&clienttype=0&channel=chunlei",
        urlencoding::encode(dir)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Referer", "https://yun.baidu.com/disk/main")
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "列出网盘目录失败")?;
    let list = v.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let paths: Vec<String> = list.iter().map(|item| str_or(item, "path")).collect();
    Ok(paths)
}

/// 创建目录（api/create a=commit）
async fn create_dir(client: &Client, path: &str, cookie: &str) -> AppResult<()> {
    let bdstoken = get_bdstoken(client, cookie).await?;
    let body = format!(
        "path={}&isdir=1&size&block_list=%5B%5D&method=post&dataType=json",
        urlencoding::encode(path)
    );
    let resp = client
        .post(format!(
            "https://pan.baidu.com/api/create?a=commit&channel=chunlei&web=1&app_id={APP_ID}&clienttype=0&bdstoken={bdstoken}"
        ))
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Referer", "https://yun.baidu.com/disk/main")
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    if v.get("errno").and_then(|e| e.as_i64()).unwrap_or(-1) != 0 {
        return Err(AppError::Api("创建临时转存目录失败".into()));
    }
    Ok(())
}

/// 确保临时转存目录存在（返回目录绝对路径，如 /YunX临时转存）
pub async fn ensure_temp_dir(client: &Client, cookie: &str) -> AppResult<String> {
    let path = format!("/{TEMP_DIR_NAME}");
    let children = list_dir(client, "/", cookie).await?;
    if children.iter().any(|p| p == &path) {
        return Ok(path);
    }
    create_dir(client, &path, cookie).await?;
    Ok(path)
}

/// 转存分享文件 → (新 fs_id, 新路径)
/// 仅在 PCS 式验证会话下可用（网页式会话会报 errno=2 参数错误，实测 2026-09）。
/// sekey 经 BDCLND cookie 传递（不裸拼 query，避免 randsk 含 +/= 时被截断）。
pub async fn transfer(
    client: &Client,
    surl: &str,
    share_id: &str,
    uk: &str,
    sekey: &str,
    fs_id: &str,
    to_dir: &str,
    cookie: &str,
) -> AppResult<(String, String)> {
    let surl = surl.trim_start_matches('1');
    let bdstoken = get_bdstoken(client, cookie).await?;
    let url = format!(
        "https://pan.baidu.com/share/transfer?shareid={share_id}&from={uk}&channel=chunlei&ondup=newcopy&web=1&app_id={APP_ID}&bdstoken={bdstoken}&clienttype=0"
    );
    // fsidlist 为裸数字数组（与实测可用实现一致；字符串数组在部分会话下报 errno=2）
    let body = format!("fsidlist=%5B{fs_id}%5D&path={}", urlencoding::encode(to_dir));
    let auth_cookie = if cookie.contains("BDCLND=") {
        cookie.to_string()
    } else {
        format!("{cookie}; BDCLND={sekey}")
    };
    let resp = client
        .post(url)
        .header("Cookie", auth_cookie)
        .header("User-Agent", UA_WEB)
        .header("Origin", "https://pan.baidu.com")
        .header("Referer", format!("https://pan.baidu.com/s/1{surl}"))
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "转存失败")?;
    let first = v.pointer("/extra/list/0").cloned().unwrap_or(Value::Null);
    let fs_id_new = {
        let s = id_or(&first, "to_fs_id");
        if s.is_empty() { id_or(&first, "from_fs_id") } else { s }
    };
    if fs_id_new.is_empty() {
        return Err(AppError::Api("转存失败：未返回新文件".into()));
    }
    let path_new = {
        let p = str_or(&first, "to");
        if p.is_empty() { format!("{to_dir}/") } else { p }
    };
    Ok((fs_id_new, path_new))
}

// 转存清理（删除临时文件）由 BaiduPCS-Go sidecar 的 rm 完成（见 baidupcs::remove）：
// 网页版 filemanager 删除接口在账号风控态返回 errno=132，sidecar 的 PCS 通道不受影响（实测 2026-09）。
