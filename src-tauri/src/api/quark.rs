//! 夸克网盘 API（移植 Android QuarkApi + QuarkConstants）。
//! 链路：getShareToken → getShareFiles → 转存临时目录 → pollTask → getDownloadLink → deleteFile。
//! __puus 会话刷新（AlistGo/alist#830）：refreshSession 剥离 __puus 后请求 /config 触发重下发。
use reqwest::Client;
use serde_json::{json, Value};

use super::{merge_puus, refresh_puus_session, set_cookies};
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
const TRANSFER_POLL_ATTEMPTS: u32 = 60;

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
    refresh_puus_session(client, CONFIG_URL, UA, DOWNLOAD_REFERER, cookie).await
}

fn merge_download_cookie(cookie: &str, set_cookies: &[String]) -> String {
    merge_puus(cookie, set_cookies)
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

/// 夸克取链上下文：转存路线存新转存文件 fid，直取/个人文件路线存原 fid。
/// 恢复 / 失败重试时按它重新取链（直链与 __puus 都有时效）。
pub fn quark_fetch_ctx(fid: &str) -> String {
    serde_json::json!({ "fid": fid }).to_string()
}

/// 轮询响应非 200 的失败判定：连续 3 次非 200 视为接口报错（登录态失效 / 任务失败 / 限流），
/// 组装带错误码与消息的失败信息；未达阈值返回 None（继续轮询容忍瞬时抖动）。
fn poll_non_ok_failure(non_ok: u32, code: i64, message: &str) -> Option<String> {
    if non_ok < 3 {
        return None;
    }
    Some(if message.is_empty() {
        format!("转存任务轮询失败（code {code}）")
    } else {
        format!("转存任务轮询失败（code {code}）：{message}")
    })
}

/// 轮询异步任务直到完成，返回转存后的新 fid（60 次 × 1s）
pub async fn poll_task(client: &Client, task_id: &str, cookie: &str) -> AppResult<String> {
    let url = format!("{TASK_URL}&task_id={}&retry_index=0", urlencoding::encode(task_id));
    let mut non_ok = 0u32;
    for _ in 0..TRANSFER_POLL_ATTEMPTS {
        let resp = client
            .get(&url)
            .header("Cookie", cookie)
            .header("User-Agent", UA)
            .send()
            .await?;
        let v: Value = resp.json().await?;
        if v.get("status").and_then(|s| s.as_i64()).unwrap_or(0) == 200 {
            non_ok = 0;
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
        } else {
            non_ok += 1;
            let code = v.get("status").and_then(|s| s.as_i64()).unwrap_or(0);
            let message = v.get("message").and_then(|m| m.as_str()).unwrap_or("");
            if let Some(err) = poll_non_ok_failure(non_ok, code, message) {
                return Err(AppError::Api(err));
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
) -> AppResult<(String, String, i64, String)> {
    let body = json!({ "fids": [fid] });
    let resp = client
        .post(DOWNLOAD_URL)
        .header("Cookie", cookie)
        .header("User-Agent", UA)
        .json(&body)
        .send()
        .await?;
    let download_cookie = merge_download_cookie(cookie, &set_cookies(&resp));
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
    Ok((url, filename, i64_or(item, "size"), download_cookie))
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

#[cfg(test)]
mod tests {
    use super::{merge_download_cookie, poll_non_ok_failure, TRANSFER_POLL_ATTEMPTS};

    #[test]
    fn transfer_polling_allows_slow_quark_tasks() {
        assert_eq!(TRANSFER_POLL_ATTEMPTS, 60);
    }

    #[test]
    fn poll_non_ok_aborts_with_code_and_message_after_three_failures() {
        // 未达阈值：容忍瞬时抖动继续轮询
        assert_eq!(poll_non_ok_failure(1, 401, "登录态失效"), None);
        assert_eq!(poll_non_ok_failure(2, 401, "登录态失效"), None);
        // 连续 3 次：带错误码与消息失败
        assert_eq!(
            poll_non_ok_failure(3, 401, "登录态失效").as_deref(),
            Some("转存任务轮询失败（code 401）：登录态失效")
        );
        // 空消息：只带错误码
        assert_eq!(
            poll_non_ok_failure(4, 500, "").as_deref(),
            Some("转存任务轮询失败（code 500）")
        );
    }

    #[test]
    fn download_cookie_accepts_puus_refreshed_by_download_endpoint() {
        let cookie = merge_download_cookie(
            "__pus=old-pus; __puus=old-puus; other=x",
            &["__puus=new-puus; Path=/; HttpOnly".into()],
        );
        assert!(cookie.contains("__puus=new-puus"));
        assert!(cookie.contains("__pus=old-pus"));
        assert!(cookie.contains("other=x"));
    }
}
