//! 个人网盘文件浏览器与一键直链获取
//! 支持已登录网盘账号（首批覆盖：百度网盘、夸克网盘、123云盘）的目录树漫游与直接取链下载。
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::{DownloadLink, Platform, ShareFile};
use crate::resolve::load_account_cookie;
use crate::state::AppState;

/// 列表指定网盘个人存储目录下文件与子目录
pub async fn list_personal_files(
    state: &AppState,
    platform: Platform,
    dir_id: &str,
) -> AppResult<Vec<ShareFile>> {
    match platform {
        Platform::Baidu => {
            let cookie = load_account_cookie(state, Platform::Baidu, "请先在「网盘」页登录百度账号")?;
            let dir = if dir_id.is_empty() || dir_id == "0" { "/" } else { dir_id };
            let url = format!(
                "https://pan.baidu.com/api/list?dir={}&order=time&desc=1&clienttype=0&app_id=250528&web=1&page=1&num=100",
                urlencoding::encode(dir)
            );
            let resp = state
                .http
                .get(&url)
                .header("Cookie", cookie)
                .header("User-Agent", "netdisk;11.12.3")
                .send()
                .await?;
            let v: Value = resp.json().await?;
            let errno = v.get("errno").and_then(|x| x.as_i64()).unwrap_or(-1);
            if errno != 0 {
                return Err(AppError::Api(format!("百度个人网盘目录获取失败 (errno={errno})")));
            }
            let list = v.get("list").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let files = list
                .into_iter()
                .filter_map(|item| {
                    let fs_id = item.get("fs_id").map(|x| x.to_string()).unwrap_or_default();
                    let server_filename = item
                        .get("server_filename")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let size = item.get("size").and_then(|x| x.as_i64()).unwrap_or(0);
                    let isdir = item.get("isdir").and_then(|x| x.as_i64()).unwrap_or(0) == 1;
                    let path = item.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let mtime = item.get("server_mtime").map(|x| x.to_string()).unwrap_or_default();
                    if fs_id.is_empty() && server_filename.is_empty() {
                        return None;
                    }
                    Some(ShareFile {
                        fid: if isdir { path.clone() } else { fs_id },
                        fname: server_filename,
                        fsize: size,
                        isdir,
                        pdir_fid: dir.to_string(),
                        fid_token: path,
                        modify_time: mtime,
                    })
                })
                .collect();
            Ok(files)
        }
        Platform::Quark => {
            let cookie = load_account_cookie(state, Platform::Quark, "请先在「网盘」页登录夸克账号")?;
            let pdir_fid = if dir_id.is_empty() { "0" } else { dir_id };
            let url = format!(
                "https://drive-pc.quark.cn/1/clouddrive/file?pr=ucpro&fr=pc&pdir_fid={pdir_fid}&page=1&size=100"
            );
            let resp = state
                .http
                .get(&url)
                .header("Cookie", cookie)
                .header("User-Agent", crate::api::quark::UA)
                .send()
                .await?;
            let v: Value = resp.json().await?;
            let list = v.pointer("/data/list").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let files = list
                .into_iter()
                .filter_map(|item| {
                    let fid = item.get("fid").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let fname = item.get("file_name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let fsize = item.get("size").and_then(|x| x.as_i64()).unwrap_or(0);
                    let isdir = item.get("dir").and_then(|x| x.as_bool()).unwrap_or(false);
                    let pdir_fid = item.get("pdir_fid").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let modify_time = item.get("updated_at").map(|x| x.to_string()).unwrap_or_default();
                    if fid.is_empty() {
                        return None;
                    }
                    Some(ShareFile {
                        fid,
                        fname,
                        fsize,
                        isdir,
                        pdir_fid,
                        fid_token: String::new(),
                        modify_time,
                    })
                })
                .collect();
            Ok(files)
        }
        Platform::Pan123 => {
            let token = load_account_cookie(state, Platform::Pan123, "请先在「网盘」页登录123云盘账号")?;
            let parent_id = if dir_id.is_empty() { "0" } else { dir_id };
            let url = format!(
                "https://yun.123pan.cn/b/api/file/list?parentFileId={}&page=1&limit=100",
                parent_id
            );
            let resp = state
                .http
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("platform", "web")
                .header("app-version", "3")
                .header("User-Agent", crate::api::pan123::WEB_UA)
                .send()
                .await?;
            let v: Value = resp.json().await?;
            let list = v.pointer("/data/InfoList").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let files = list
                .into_iter()
                .filter_map(|item| {
                    let fid = item.get("FileId").map(|x| x.to_string()).unwrap_or_default();
                    let fname = item.get("FileName").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let fsize = item.get("Size").and_then(|x| x.as_i64()).unwrap_or(0);
                    let ftype = item.get("Type").and_then(|x| x.as_i64()).unwrap_or(0);
                    let isdir = ftype == 1;
                    let pdir_fid = item.get("ParentFileId").map(|x| x.to_string()).unwrap_or_default();
                    let s3_flag = item.get("S3KeyFlag").and_then(|x| x.as_str()).unwrap_or("");
                    let etag = item.get("Etag").and_then(|x| x.as_str()).unwrap_or("");
                    let storage = item.get("StorageNode").and_then(|x| x.as_str()).unwrap_or("");
                    let modify_time = item.get("UpdateAt").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    if fid.is_empty() {
                        return None;
                    }
                    Some(ShareFile {
                        fid,
                        fname,
                        fsize,
                        isdir,
                        pdir_fid,
                        fid_token: format!("{s3_flag}|{etag}|{storage}"),
                        modify_time,
                    })
                })
                .collect();
            Ok(files)
        }
        _ => Err(AppError::Api("暂不支持该网盘的个人文件直接管理".into())),
    }
}

/// 获取个人网盘文件的一键直链下载地址
pub async fn get_personal_download_link(
    state: &AppState,
    platform: Platform,
    file: &ShareFile,
) -> AppResult<DownloadLink> {
    match platform {
        Platform::Baidu => {
            let cookie = load_account_cookie(state, Platform::Baidu, "请先在「网盘」页登录百度账号")?;
            let path = if !file.fid_token.is_empty() {
                &file.fid_token
            } else {
                &file.fid
            };
            let working = crate::baidupcs::locate_urls(&state.http, &cookie, path, &state.data_dir).await?;
            let url = working.first().cloned().unwrap_or_default();
            let mirrors = if working.len() > 1 { working[1..].to_vec() } else { Vec::new() };
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: file.fsize,
                headers: vec![("User-Agent".into(), crate::baidupcs::UA.into())],
                platform: "baidu".into(),
                cleanup_id: String::new(),
                mirrors,
            })
        }
        Platform::Quark => {
            let cookie = load_account_cookie(state, Platform::Quark, "请先在「网盘」页登录夸克账号")?;
            let (url, filename, size) = crate::api::quark::get_download_link(&state.http, &file.fid, &cookie).await?;
            Ok(DownloadLink {
                url,
                filename: if !filename.is_empty() { filename } else { file.fname.clone() },
                size: if size > 0 { size } else { file.fsize },
                headers: vec![
                    ("User-Agent".into(), crate::api::quark::UA.into()),
                    ("Referer".into(), crate::api::quark::DOWNLOAD_REFERER.into()),
                ],
                platform: "quark".into(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Pan123 => {
            let token = load_account_cookie(state, Platform::Pan123, "请先在「网盘」页登录123云盘账号")?;
            let body = serde_json::json!({
                "fileId": file.fid.parse::<i64>().unwrap_or(0),
            });
            let resp = state
                .http
                .post("https://www.123865.com/b/api/file/download/info")
                .header("Authorization", format!("Bearer {token}"))
                .header("platform", "web")
                .header("app-version", "3")
                .header("User-Agent", crate::api::pan123::WEB_UA)
                .json(&body)
                .send()
                .await?;
            let v: Value = resp.json().await?;
            let download_url = v.pointer("/data/DownloadUrl").and_then(|x| x.as_str()).unwrap_or("");
            if download_url.is_empty() {
                return Err(AppError::Api("123云盘获取直链失败".into()));
            }
            Ok(DownloadLink {
                url: download_url.to_string(),
                filename: file.fname.clone(),
                size: file.fsize,
                headers: vec![
                    ("User-Agent".into(), crate::api::pan123::WEB_UA.into()),
                    ("Referer".into(), crate::api::pan123::DOWNLOAD_REFERER.into()),
                ],
                platform: "pan123".into(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        _ => Err(AppError::Api("暂不支持该网盘的文件直链下载".into())),
    }
}
