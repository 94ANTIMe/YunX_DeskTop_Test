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

/// 验证提取码 → sekey（randsk）
pub async fn verify_share(client: &Client, surl: &str, pwd: &str, cookie: &str) -> AppResult<String> {
    let body = format!("pwd={}&vcode_str=&vcode=", urlencoding::encode(pwd));
    let resp = client
        .post(format!("https://pan.baidu.com/share/verify?surl={surl}"))
        .header("Cookie", cookie)
        .header("User-Agent", UA_WEB)
        .header("Referer", format!("https://pan.baidu.com/s/{surl}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "验证提取码失败")?;
    let sekey = str_or(&v, "randsk");
    if sekey.is_empty() {
        return Err(AppError::Api("未返回分享密钥".into()));
    }
    Ok(sekey)
}

/// 百度分享列表结果
pub struct BaiduShareList {
    pub title: String,
    pub share_id: String,
    pub uk: String,
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 列出分享文件（xpan/share list；子目录需 BDCLND=sekey）
pub async fn list_share(
    client: &Client,
    surl: &str,
    sekey: &str,
    dir: &str,
    cookie: &str,
    page: i64,
) -> AppResult<BaiduShareList> {
    let is_root = dir.is_empty() || dir == "/";
    let root = if is_root { "1" } else { "0" };
    let dir_param = if dir.is_empty() { "/" } else { dir };
    let mut url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/share?method=list&shorturl={surl}&page={page}&num=100&root={root}&dir={}",
        urlencoding::encode(dir_param)
    );
    if !sekey.is_empty() {
        url.push_str(&format!("&sekey={sekey}"));
    }
    let auth_cookie = if !sekey.is_empty() && !cookie.contains("BDCLND=") {
        format!("{cookie}; BDCLND={sekey}")
    } else {
        cookie.to_string()
    };
    let resp = client
        .get(&url)
        .header("Cookie", auth_cookie)
        .header("User-Agent", UA_WEB)
        .header("Referer", format!("https://pan.baidu.com/s/{surl}"))
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let errno = v.get("errno").and_then(|e| e.as_i64()).unwrap_or(-1);
    if errno != 0 {
        // 无 sekey 却失败 → 实为加密分享
        if sekey.is_empty() {
            return Err(AppError::Api("该分享需要提取码".into()));
        }
        check_errno(&v, "获取分享文件列表失败")?;
    }
    let list = v.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let mut files = Vec::new();
    for item in &list {
        let isdir = item.get("isdir").and_then(|x| x.as_str()) == Some("1")
            || item.get("isdir").and_then(|x| x.as_i64()) == Some(1);
        let path = str_or(item, "path");
        files.push(ShareFile {
            // 目录用 path 作 fid（导航传参），文件用 fs_id（转存传参；fs_id 可能是字符串或数字）
            fid: if isdir { path.clone() } else { id_or(item, "fs_id") },
            fname: str_or(item, "server_filename"),
            fsize: i64_or(item, "size"),
            isdir,
            pdir_fid: path.clone(),
            fid_token: path.clone(),
            modify_time: str_or(item, "server_mtime"),
        });
    }
    let has_more = files.len() >= 100;
    Ok(BaiduShareList {
        title: str_or(&v, "title"),
        share_id: str_or(&v, "share_id"),
        uk: str_or(&v, "uk"),
        files,
        has_more,
    })
}

/// 列出个人网盘目录（检查临时转存目录），返回子项 path
async fn list_dir(client: &Client, dir: &str, cookie: &str) -> AppResult<Vec<String>> {
    let url = format!(
        "https://yun.baidu.com/api/list?clienttype=0&app_id={APP_ID}&web=1&order=time&desc=1&dir={}&num=100&page=1",
        urlencoding::encode(dir)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    if v.get("errno").and_then(|e| e.as_i64()).unwrap_or(-1) != 0 {
        return Ok(Vec::new());
    }
    let list = v.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    Ok(list.iter().map(|i| str_or(i, "path")).collect())
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
pub async fn transfer(
    client: &Client,
    share_id: &str,
    uk: &str,
    sekey: &str,
    fs_id: &str,
    to_dir: &str,
    cookie: &str,
) -> AppResult<(String, String)> {
    let bdstoken = get_bdstoken(client, cookie).await?;
    let url = format!(
        "https://pan.baidu.com/share/transfer?shareid={share_id}&from={uk}&channel=chunlei&sekey={sekey}&ondup=newcopy&web=1&app_id={APP_ID}&bdstoken={bdstoken}&clienttype=0"
    );
    // fsidlist 为预编码 JSON 数组字符串，用 body 直发避免二次编码
    let body = format!("fsidlist=%5B%22{fs_id}%22%5D&path={}", urlencoding::encode(to_dir));
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
        .header("Referer", "https://pan.baidu.com/s/")
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

/// locatedownload 高速直链（选 encrypt=0 的 appall 明文通道）
pub async fn locate_download(client: &Client, path: &str, cookie: &str) -> AppResult<String> {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let url = format!(
        "https://d.pcs.baidu.com/rest/2.0/pcs/file?method=locatedownload&app_id={APP_ID}\
&clienttype=17&ver=4.0&ant=1&check_blue=1&es=1&esl=1&apn_id=1_-1&freeisp=0&queryfree=0&use=1&dtype=1&eck=1&ehps=1\
&err_ver=1.0&network_type=WIFI&channel=0&path={}&time={time}\
&rand=5ed606e9da222cde0474cdf70eda884b&devuid=0F1E9FC2E084472DA5A61C4CF4C759AF&cuid=0F1E9FC2E084472DA5A61C4CF4C759AF\
&deviceid=348642637967375013&psign=860a071f77c860e8cea06e4e54c518f3&version=2.2.111.34&version_app=12.24.6&vip=0",
        urlencoding::encode(path)
    );
    let resp = client
        .post(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("0")
        .send()
        .await?;
    let v: Value = resp.json().await?;
    check_errno(&v, "获取高速下载链接失败")?;
    let urls = v.get("urls").and_then(|u| u.as_array()).cloned().unwrap_or_default();
    let candidates: Vec<&Value> = urls
        .iter()
        .filter(|u| !u.get("url").and_then(|x| x.as_str()).unwrap_or("").is_empty())
        .collect();
    // 优先 encrypt=0 的 https（appall 明文通道）；d2-ant 加密通道排除
    let direct = candidates
        .iter()
        .find(|u| u.get("encrypt").and_then(|e| e.as_i64()).unwrap_or(1) == 0
            && u.get("url").and_then(|x| x.as_str()).map(|s| s.starts_with("https")).unwrap_or(false))
        .or_else(|| {
            candidates
                .iter()
                .find(|u| u.get("url").and_then(|x| x.as_str()).map(|s| s.starts_with("https")).unwrap_or(false))
        })
        .or_else(|| candidates.first())
        .and_then(|u| u.get("url").and_then(|x| x.as_str()))
        .ok_or_else(|| AppError::Api("未返回下载链接".into()))?;
    Ok(direct.to_string())
}

/// 删除个人网盘文件（按完整路径，转存清理）
pub async fn delete_file(client: &Client, path: &str, cookie: &str) -> AppResult<()> {
    let bdstoken = get_bdstoken(client, cookie).await?;
    let filelist = format!("[\"{path}\"]");
    let body = format!("filelist={}", urlencoding::encode(&filelist));
    let resp = client
        .post(format!(
            "https://pan.baidu.com/api/filemanager?async=2&onnest=fail&opera=delete&bdstoken={bdstoken}&newVerify=1&clienttype=0&app_id={APP_ID}&web=1"
        ))
        .header("Cookie", cookie)
        .header("User-Agent", UA_NETDISK)
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    // 清理失败不阻断主流程
    let _ = check_errno(&v, "清理临时文件失败");
    Ok(())
}
