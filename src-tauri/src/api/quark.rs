//! 夸克网盘 API（移植 Android QuarkApi + QuarkConstants）。
//! 链路：getShareToken → getShareFiles → 转存临时目录 → pollTask → getDownloadLink → deleteFile。
//! __puus 会话刷新（AlistGo/alist#830）：refreshSession 剥离 __puus 后请求 /config 触发重下发。
use reqwest::Client;
use serde_json::{json, Value};

use super::{merge_puus, without_puus};
use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) quark-cloud-drive/2.5.20 Chrome/100.0.4896.160 Electron/18.3.5.12-a038f7b798 Safari/537.36 Channel/pckk_other_ch";
pub const DOWNLOAD_REFERER: &str = "https://pan.quark.cn/";
pub const TEMP_DIR_NAME: &str = "YunX临时转存";
pub const ACCOUNT_INFO_URL: &str = "https://pan.quark.cn/account/info";
const SHARE_TOKEN_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/share/sharepage/token?pr=ucpro&fr=pc";
const SHARE_DETAIL_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/share/sharepage/detail?pr=ucpro&fr=pc";
const DOWNLOAD_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/file/download?pr=ucpro&fr=pc&sys=win32&ve=3.23.2";
const FILE_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/file?pr=ucpro&fr=pc";
const SAVE_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/share/sharepage/save?pr=ucpro&fr=pc";
const TASK_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/task?pr=ucpro&fr=pc";
const DELETE_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/file/delete?pr=ucpro&fr=pc&uc_param_str=";
const CONFIG_URL: &str = "https://drive-pc.quark.cn/1/clouddrive/config?pr=ucpro&fr=pc";

fn set_cookies(resp: &reqwest::Response) -> Vec<String> {
    resp.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(String::from))
        .collect()
}

/// parseData：status != 200 → 透传 message
fn check_status<'a>(v: &'a Value, fallback: &str) -> AppResult<&'a Value> {
    let status = v.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
    if status != 200 {
        let msg = v
            .get("message")
            .and_then(|m| m.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback);
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
        let nick = v
            .pointer("/data/nickname")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if !nick.is_empty() {
            return Ok(nick.to_string());
        }
    }
    Err(AppError::Api("Cookie 无效或已过期".into()))
}

/// 登录态判定（__pus + __puus 同时存在）
pub fn is_valid_cookie(cookie: &str) -> bool {
    cookie.contains("__pus=") && cookie.contains("__puus=")
}

/// 刷新会话 Cookie（剥离 __puus → /config → Set-Cookie 重下发合并）
pub async fn refresh_session(client: &Client, cookie: &str) -> AppResult<String> {
    let resp = client
        .get(CONFIG_URL)
        .header("Cookie", without_puus(cookie))
        .header("User-Agent", UA)
        .header("Referer", DOWNLOAD_REFERER)
        .send()
        .await?;
    let merged = merge_puus(cookie, &set_cookies(&resp));
    if merged != cookie {
        Ok(merged)
    } else {
        Ok(cookie.to_string())
    }
}

/// 分享 Token（stoken + 标题）
pub async fn get_share_token(client: &Client, share_id: &str, pwd: &str, cookie: &str) -> AppResult<(String, String)> {
    let body = json!({
        "pwd_id": share_id,
        "passcode": pwd,
        "support_visit_limit_private_share": true,
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

/// 分享文件列表（_page/_size 分页；返回 (列表, 总数)）
pub async fn get_share_files(
    client: &Client,
    share_id: &str,
    stoken: &str,
    pdir_fid: &str,
    cookie: &str,
    page: i64,
    size: i64,
) -> AppResult<(Vec<ShareFile>, i64)> {
    let url = format!(
        "{SHARE_DETAIL_URL}&pwd_id={share_id}&stoken={}&pdir_fid={pdir_fid}&ver=2&force=0&_page={page}&_size={size}&_fetch_banner=0&_fetch_share=0&fetch_relate_conversation=0&_fetch_total=1&_sort=file_type:asc,file_name:asc",
        urlencoding::encode(stoken)
    );
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .header("Origin", "https://pan.quark.cn")
        .header("Referer", "https://pan.quark.cn/")
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "获取分享文件列表失败")?;
    let list = data.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let total = data
        .get("_total")
        .and_then(|t| t.as_i64())
        .unwrap_or(page.saturating_sub(1) * size + list.len() as i64);
    let files = list
        .iter()
        .filter_map(|item| {
            Some(ShareFile {
                fid: str_or(item, "fid"),
                fname: str_or(item, "file_name"),
                fsize: i64_or(item, "size"),
                isdir: bool_or(item, "dir"),
                pdir_fid: str_or(item, "pdir_fid"),
                fid_token: str_or(item, "share_fid_token"),
                modify_time: str_or(item, "updated_at"),
            })
        })
        .filter(|f| !f.fid.is_empty() || !f.fname.is_empty())
        .collect();
    Ok((files, total))
}

/// 个人网盘根目录列表（查临时转存目录 fid）
async fn get_root_files(client: &Client, cookie: &str) -> AppResult<Vec<ShareFile>> {
    let url = format!("{FILE_URL}&pdir_fid=0&page=1&size=100");
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "获取网盘文件列表失败")?;
    let list = data.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|item| {
            Some(ShareFile {
                fid: str_or(item, "fid"),
                fname: str_or(item, "file_name"),
                fsize: i64_or(item, "size"),
                isdir: bool_or(item, "dir"),
                pdir_fid: str_or(item, "pdir_fid"),
                fid_token: str_or(item, "fid_token"),
                modify_time: str_or(item, "modify_time"),
            })
        })
        .filter(|f| !f.fid.is_empty())
        .collect())
}

/// 创建目录（指定父目录），返回新目录 fid
pub async fn create_folder(client: &Client, name: &str, parent_fid: &str, cookie: &str) -> AppResult<String> {
    let body = json!({ "pdir_fid": parent_fid, "file_name": name, "dir_path": "", "dir_init_lock": false });
    let resp = client
        .post(FILE_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "创建临时转存目录失败")?;
    let fid = str_or(data, "fid");
    if fid.is_empty() {
        return Err(AppError::Api("创建临时转存目录失败".into()));
    }
    Ok(fid)
}

/// 确保临时转存目录存在，返回其 fid
pub async fn ensure_temp_dir(client: &Client, cookie: &str) -> AppResult<String> {
    let root = get_root_files(client, cookie).await?;
    if let Some(dir) = root.iter().find(|f| f.isdir && f.fname == TEMP_DIR_NAME) {
        return Ok(dir.fid.clone());
    }
    create_folder(client, TEMP_DIR_NAME, "0", cookie).await
}

/// 在临时目录下创建唯一子目录（tr_<时间戳>_<随机>）：
/// 使 sharepage/save 去重键（to_pdir_fid）每次不同 → 永远生成新 fid，
/// 从根上避免「二次转存返回已删除 fid → download 404 code:21001」。
pub async fn create_transfer_subdir(client: &Client, base_dir: &str, cookie: &str) -> AppResult<String> {
    let name = format!(
        "tr_{}_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        crate::api::random_alnum(6)
    );
    create_folder(client, &name, base_dir, cookie).await
}

/// 转存分享文件到指定目录，返回 task_id
pub async fn save_share_file(
    client: &Client,
    share_id: &str,
    stoken: &str,
    pdir_fid: &str,
    fid: &str,
    fid_token: &str,
    to_pdir_fid: &str,
    cookie: &str,
) -> AppResult<String> {
    let body = json!({
        "pwd_id": share_id,
        "stoken": stoken,
        "pdir_fid": pdir_fid,
        "to_pdir_fid": to_pdir_fid,
        "fid_list": [fid],
        "fid_token_list": [fid_token],
        "scene": "link",
    });
    let resp = client
        .post(SAVE_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let data = check_status(&v, "转存失败")?;
    let task_id = str_or(data, "task_id");
    if task_id.is_empty() {
        return Err(AppError::Api("转存失败：未返回任务".into()));
    }
    Ok(task_id)
}

/// 轮询异步任务直到完成，返回转存后的新 fid（10 次 × 1s）
pub async fn poll_task(client: &Client, task_id: &str, cookie: &str) -> AppResult<String> {
    let url = format!("{TASK_URL}&task_id={}&retry_index=0", urlencoding::encode(task_id));
    for _ in 0..10 {
        let resp = client
            .get(&url)
            .header("Cookie", cookie)
            .header("User-Agent", UA)
            .send()
            .await?;
        let v: Value = resp.json().await?;
        if v.get("status").and_then(|s| s.as_i64()).unwrap_or(0) == 200 {
            let data = v.get("data");
            if let Some(data) = data {
                let finished = i64_or(data, "finished_at") > 0
                    || i64_or(data, "status") == 2
                    || i64_or(data, "task_status") == 2;
                if finished {
                    let fid = data
                        .pointer("/save_as/save_as_top_fids/0")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    if !fid.is_empty() {
                        return Ok(fid.to_string());
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err(AppError::Api("转存任务超时".into()))
}

/// 获取下载直链（个人网盘文件）
pub async fn get_download_link(
    client: &Client,
    fid: &str,
    cookie: &str,
) -> AppResult<(String, String, i64)> {
    let body = json!({ "fids": [fid] });
    let resp = client
        .post(DOWNLOAD_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let status = v.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if status != 200 && code != 0 {
        let msg = v.get("message").and_then(|m| m.as_str()).filter(|s| !s.is_empty());
        return Err(AppError::Api(msg.unwrap_or("获取下载链接失败").to_string()));
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

/// 删除文件（清理临时转存；异步任务无需轮询）
pub async fn delete_file(client: &Client, fid: &str, cookie: &str) -> AppResult<()> {
    let body = json!({ "action_type": 2, "filelist": [fid], "exclude_fids": [] });
    let resp = client
        .post(DELETE_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    // 删除失败不阻断主流程（忽略状态校验错误）
    let _ = check_status(&v, "清理临时文件失败");
    Ok(())
}
