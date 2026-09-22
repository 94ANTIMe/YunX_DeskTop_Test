//! aria2 下载引擎（sidecar 外部二进制 + JSON-RPC 管理）。
//! 启动：aria2c --enable-rpc --rpc-listen-port=16800 --rpc-secret --stop-with-process=<自身PID>
//! 任务：addUri 携带 header（Cookie/UA/Referer）+ out 文件名 + split 并发；
//! 进度：1s 轮询 tellStatus → 事件推送前端 + DB 节流持久化；完成后触发夸克延迟清理。

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::error::{AppError, AppResult};
use crate::models::{DownloadDetail, DownloadTaskView, Settings};
use crate::state::AppState;

mod engine;
mod policy;
mod rpc;

pub(crate) use engine::*;
pub(crate) use policy::*;
pub(crate) use rpc::*;


async fn mark_enqueue_failed(app: &AppHandle, id: i64, message: &str) {
    let state = app.state::<AppState>();
    let finish_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    if let Ok(conn) = state.db.lock() {
        let _ = mark_enqueue_failed_in_db(&conn, id, message, finish_time);
    }
    push_list(app).await;
}

fn mark_enqueue_failed_in_db(conn: &rusqlite::Connection, id: i64, message: &str, finish_time: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE download_task SET status = ?1, error_msg = ?2, finish_time = ?3, gid = '' WHERE id = ?4",
        rusqlite::params![DownloadTaskView::STATUS_FAILED, message, finish_time, id],
    )
}

fn clear_targets(conn: &rusqlite::Connection) -> AppResult<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare("SELECT gid, platform, cleanup_id FROM download_task")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

async fn cleanup_transfer(state: &AppState, platform: &str, cleanup_id: &str) {
    if cleanup_id.is_empty() {
        return;
    }
    match platform {
        "quark" => crate::resolve::cleanup_quark(state, cleanup_id).await,
        "baidu" => crate::resolve::cleanup_baidu(state, cleanup_id).await,
        _ => {}
    }
}

/// 将下载任务提交给 aria2 引擎（addUri → 回写 gid，不插入 DB）
async fn add_to_aria2(
    app: &AppHandle,
    id: i64,
    url: &str,
    file_name: &str,
    headers: &[(String, String)],
    platform: &str,
    cleanup_id: &str,
    start_paused: bool,
    mirrors: Vec<String>,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let dir = resolve_download_dir(app, &settings);

    let mut uris = vec![url.to_string()];
    for m in mirrors {
        if !uris.contains(&m) {
            uris.push(m);
        }
    }
    let mirror_count = uris.len();

    // aria2 addUri：header 列表 + out 文件名 + split 并发
    let mut header_list: Vec<String> = headers.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    if !header_list.iter().any(|h| h.to_lowercase().starts_with("user-agent")) {
        header_list.push(format!("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));
    }

    // 针对百度及多镜像任务优化并发连接；夸克使用保守并发，避免 active 但 0 速度。
    let (split, max_connections) = transfer_tuning(
        platform,
        settings.download_threads,
        settings.download_conn_per_server,
        mirror_count,
    );
    let min_split = format!("{}M", settings.download_min_split_mb.clamp(1, 64));

    let is_magnet = url.starts_with("magnet:?");
    let mut options = if is_magnet {
        json!({
            "dir": dir.display().to_string(),
            "continue": "true",
            "bt-tracker": current_bt_trackers(),
            "seed-time": "0",
        })
    } else {
        let extra = stall_guard_options(platform);
        http_task_options(
            &dir.display().to_string(),
            file_name,
            &header_list,
            split,
            max_connections,
            &min_split,
            settings.download_retry_count.clamp(0, 10),
            &extra,
        )
    };
    if start_paused {
        options["paused"] = json!("true");
    }
    let gid = rpc_call("aria2.addUri", vec![json!(uris), options])
        .await?
        .as_str()
        .unwrap_or("")
        .to_string();
    if gid.is_empty() {
        state.log(crate::logger::ERROR, platform, "download", &format!("入队失败：{file_name}"), "aria2 未返回 gid");
        return Err(AppError::Api("下载引擎入队失败".into()));
    }
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("UPDATE download_task SET gid = ?1 WHERE id = ?2", rusqlite::params![gid, id])?;
    }
    state.log(
        crate::logger::SUCCESS,
        platform,
        "download",
        &format!("已加入下载：{file_name}"),
        &format!("任务 #{id} gid={gid} split={split} 镜像源={mirror_count} {}", if cleanup_id.is_empty() { String::new() } else { format!("cleanup={cleanup_id}") }),
    );
    Ok(gid)
}

/// 入队下载（插入 DB → add_to_aria2）
pub async fn enqueue(
    app: &AppHandle,
    url: &str,
    file_name: &str,
    headers: &[(String, String)],
    platform: &str,
    cleanup_id: &str,
    start_paused: bool,
    mirrors: Vec<String>,
    fetch_ctx: &str,
) -> AppResult<i64> {
    let state = app.state::<AppState>();
    // 入库前净化：删除原语（delete_local 拼路径）与重启恢复共用 DB 值，必须与 aria2 out 一致
    let file_name = sanitize_out_path(file_name);
    let headers_json = serde_json::to_string(&headers)?;
    let mirrors_json = serde_json::to_string(&mirrors)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let id = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "INSERT INTO download_task (url, file_name, request_headers_json, platform, cleanup_id, mirrors_json, fetch_ctx_json, create_time, status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0) RETURNING id",
            rusqlite::params![url, file_name, headers_json, platform, cleanup_id, mirrors_json, fetch_ctx, now],
            |r| r.get::<_, i64>(0),
        )?
    };

    if let Err(error) = add_to_aria2(app, id, url, &file_name, headers, platform, cleanup_id, start_paused, mirrors).await {
        mark_enqueue_failed(app, id, &error.to_string()).await;
        cleanup_transfer(&state, platform, cleanup_id).await;
        return Err(error);
    }
    Ok(id)
}

/// BT 任务（magnet / .torrent）共用的 aria2 选项
fn torrent_options(app: &AppHandle) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let dir = resolve_download_dir(app, &settings);
    Ok(json!({
        "dir": dir.display().to_string(),
        "continue": "true",
        "bt-tracker": current_bt_trackers(),
        "seed-time": "0",
    }))
}

/// 提交本地 BT 种子文件（.torrent）入队下载
pub async fn enqueue_torrent(
    app: &AppHandle,
    torrent_bytes: &[u8],
    file_name: &str,
) -> AppResult<i64> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(torrent_bytes);
    let state = app.state::<AppState>();
    let options = torrent_options(app)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let id = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "INSERT INTO download_task (url, file_name, request_headers_json, platform, cleanup_id, create_time, status) \
             VALUES (?1, ?2, '[]', 'magnet', '', ?3, 1) RETURNING id",
            rusqlite::params![format!("torrent:{file_name}"), file_name, now],
            |r| r.get::<_, i64>(0),
        )?
    };

    // 种子字节落盘：重启后 "torrent:文件名" 伪 URL 无法断点恢复，靠这份副本重新 addTorrent
    let torrents_dir = state.data_dir.join("torrents");
    let _ = std::fs::create_dir_all(&torrents_dir);
    let _ = std::fs::write(torrents_dir.join(format!("{id}.torrent")), torrent_bytes);

    let gid = match rpc_call("aria2.addTorrent", vec![json!(b64), json!([]), options]).await {
        Ok(v) => v.as_str().unwrap_or("").to_string(),
        Err(e) => {
            state.log(crate::logger::ERROR, "magnet", "download", &format!("种子解析入队失败：{file_name}"), &e.to_string());
            mark_enqueue_failed(app, id, &e.to_string()).await;
            return Err(e);
        }
    };
    if gid.is_empty() {
        let error = AppError::Api("下载引擎未返回种子任务 gid".into());
        mark_enqueue_failed(app, id, &error.to_string()).await;
        return Err(error);
    }

    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("UPDATE download_task SET gid = ?1 WHERE id = ?2", rusqlite::params![gid, id])?;
    }
    state.log(
        crate::logger::SUCCESS,
        "magnet",
        "download",
        &format!("BT 种子已加入下载：{file_name}"),
        &format!("任务 #{id} gid={gid}"),
    );
    Ok(id)
}

/// 暂停（remove + paused=true 语义：aria2 pause 保留进度）
pub async fn pause(app: &AppHandle, id: i64) -> AppResult<()> {
    let gid = gid_of(app, id)?;
    if !gid.is_empty() {
        rpc_call("aria2.pause", vec![json!(gid)]).await?;
    }
    update_status(app, id, DownloadTaskView::STATUS_PAUSED, "").await?;
    push_list(app).await;
    Ok(())
}

/// 恢复 / 重挂前的直链与请求头：夸克任务按 fetch_ctx 重新取链（直链与 __puus 都有时效），
/// 成功则回写 DB 供后续恢复使用；失败沿用 DB 旧值（旧直链可能仍可用），错误只记日志。
async fn refreshed_target(
    app: &AppHandle,
    id: i64,
    platform: &str,
    fetch_ctx: &str,
    file_name: &str,
    fallback_url: String,
    fallback_headers: Vec<(String, String)>,
) -> (String, Vec<(String, String)>) {
    if platform != "quark" || fetch_ctx.is_empty() {
        return (fallback_url, fallback_headers);
    }
    let state = app.state::<AppState>();
    match crate::resolve::refresh_quark_download_link(&state, fetch_ctx).await {
        Ok((fresh_url, fresh_headers)) => {
            if let Ok(fresh_json) = serde_json::to_string(&fresh_headers) {
                if let Ok(conn) = state.db.lock() {
                    let _ = conn.execute(
                        "UPDATE download_task SET url = ?1, request_headers_json = ?2 WHERE id = ?3",
                        rusqlite::params![fresh_url, fresh_json, id],
                    );
                }
            }
            state.log(crate::logger::INFO, "quark", "download", "恢复前已重新取链（直链已刷新）", file_name);
            (fresh_url, fresh_headers)
        }
        Err(e) => {
            state.log(crate::logger::ERROR, "quark", "download", "重新取链失败，沿用原直链恢复", &e.to_string());
            (fallback_url, fallback_headers)
        }
    }
}

/// 恢复
pub async fn resume(app: &AppHandle, id: i64) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (gid, url, file_name, headers_json, platform, cleanup_id, mirrors_json, status, fetch_ctx) = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT gid, url, file_name, request_headers_json, platform, cleanup_id, mirrors_json, status, fetch_ctx_json FROM download_task WHERE id = ?1",
            rusqlite::params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, i32>(7)?,
                    r.get::<_, String>(8)?,
                ))
            },
        )?
    };

    let mut tell_gid = gid.clone();
    let mut unpaused = false;
    if !gid.is_empty() && status == DownloadTaskView::STATUS_PAUSED {
        if rpc_call("aria2.unpause", vec![json!(gid)]).await.is_ok() {
            unpaused = true;
        }
    }

    if !unpaused {
        let mut url = url.clone();
        let mut headers: Vec<(String, String)> = serde_json::from_str(&headers_json).unwrap_or_default();
        let refreshed = refreshed_target(app, id, &platform, &fetch_ctx, &file_name, url.clone(), headers).await;
        url = refreshed.0;
        headers = refreshed.1;
        // 重新入队 aria2（用于从失败态恢复或 unpause 失败时重入队，支持断点续传）
        let mirrors: Vec<String> = serde_json::from_str(&mirrors_json).unwrap_or_default();
        let new_gid = add_to_aria2(app, id, &url, &file_name, &headers, &platform, &cleanup_id, false, mirrors).await?;
        tell_gid = new_gid;
    }

    // 状态求实：unpause 后任务可能回到 aria2 等队列（waiting），
    // 重新入队的任务也未必立刻 active——按引擎真实状态落库，查不到再回退排队态。
    let real_status = tell_status_mapped(&tell_gid).await;
    let new_status = real_status.unwrap_or(DownloadTaskView::STATUS_PENDING);
    update_status(app, id, new_status, "").await?;
    push_list(app).await;
    Ok(())
}

/// 查单个 gid 的 aria2 状态并映射为任务状态；查不到 / 引擎不可达返回 None
async fn tell_status_mapped(gid: &str) -> Option<i32> {
    if gid.is_empty() {
        return None;
    }
    let v = rpc_call("aria2.tellStatus", vec![json!(gid), json!(["status"])]).await.ok()?;
    let status = v.get("status")?.as_str()?;
    Some(map_status(status))
}

/// 删除任务（aria2 remove + DB 删除 + 转存清理）
pub async fn remove(app: &AppHandle, id: i64, delete_local: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (gid, url, file_name, cleanup_id, platform, save_path) = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT gid, url, file_name, cleanup_id, platform, save_path FROM download_task WHERE id = ?1",
            rusqlite::params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional_row()?
        .unwrap_or_default()
    };
    if !gid.is_empty() {
        let _ = rpc_call("aria2.forceRemove", vec![json!(gid)]).await;
    }
    if delete_local && !file_name.is_empty() {
        let path = if save_path.is_empty() {
            resolve_download_dir(app, &state.load_settings()).join(sanitize_out_path(&file_name))
        } else {
            std::path::PathBuf::from(&save_path)
        };
        let _ = std::fs::remove_file(&path);
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            let _ = std::fs::remove_file(path.with_file_name(format!("{name}.aria2")));
        }
    }
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("DELETE FROM download_task WHERE id = ?1", rusqlite::params![id])?;
    }
    // BT 任务顺带清理落盘的种子副本
    if url.starts_with("torrent:") {
        let _ = std::fs::remove_file(state.data_dir.join("torrents").join(format!("{id}.torrent")));
    }
    // 删除任务同样触发夸克 / 百度转存清理（用户放弃下载）
    cleanup_transfer(&state, &platform, &cleanup_id).await;
    push_list(app).await;
    Ok(())
}

/// 清空全部下载任务记录（aria2 全部 forceRemove + DB 清空）
pub async fn clear_all(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    // 收集全部 gid 并强制移除（含进行中/等待/暂停）
    let tasks: Vec<(String, String, String)> = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        clear_targets(&conn)?
    };
    // 任务间并发移除（单任务内仍先 forceRemove 后清结果），aria2 卡顿时不再线性放大清空耗时
    futures_util::future::join_all(tasks.iter().filter(|(gid, _, _)| !gid.is_empty()).map(|(gid, _, _)| async move {
        let _ = rpc_call("aria2.forceRemove", vec![json!(gid)]).await;
        let _ = rpc_call("aria2.removeDownloadResult", vec![json!(gid)]).await;
    }))
    .await;
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("DELETE FROM download_task", [])?;
    }
    // 全部清空时顺带移除 BT 种子副本目录
    let _ = std::fs::remove_dir_all(state.data_dir.join("torrents"));
    for (_, platform, cleanup_id) in &tasks {
        cleanup_transfer(&state, platform, cleanup_id).await;
    }
    // 清空后向前端推送空列表
    let views: Vec<DownloadTaskView> = Vec::new();
    let _ = app.emit("downloads:updated", &views);
    Ok(())
}

/// 暂停全部进行中/等待任务（aria2.pauseAll + DB 置暂停态 + 事件）
pub async fn pause_all(app: &AppHandle) -> AppResult<()> {
    rpc_call("aria2.pauseAll", vec![]).await?;
    let state = app.state::<AppState>();
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute(
            "UPDATE download_task SET status = ?1 WHERE status IN (0, 1)",
            rusqlite::params![DownloadTaskView::STATUS_PAUSED],
        )?;
    }
    push_list(app).await;
    Ok(())
}

/// 继续全部暂停任务（aria2.unpauseAll + DB 置下载态 + 事件）
pub async fn resume_all(app: &AppHandle) -> AppResult<()> {
    rpc_call("aria2.unpauseAll", vec![]).await?;
    let state = app.state::<AppState>();
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute(
            "UPDATE download_task SET status = ?1 WHERE status = ?2",
            rusqlite::params![DownloadTaskView::STATUS_DOWNLOADING, DownloadTaskView::STATUS_PAUSED],
        )?;
    }
    push_list(app).await;
    Ok(())
}

/// 拉取全量任务并向前端推送（pause_all / resume_all 用）
async fn push_list(app: &AppHandle) {
    if let Ok(views) = list_tasks(app) {
        let _ = app.emit("downloads:updated", &views);
    }
}

// ---------- 工具 ----------

fn gid_of(app: &AppHandle, id: i64) -> AppResult<String> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    Ok(conn
        .query_row(
            "SELECT gid FROM download_task WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_default())
}

async fn update_status(app: &AppHandle, id: i64, status: i32, error_msg: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE download_task SET status = ?1, error_msg = ?2 WHERE id = ?3",
        rusqlite::params![status, error_msg, id],
    )?;
    Ok(())
}

trait OptionalRow {
    fn optional_row(self) -> AppResult<Option<(String, String, String, String, String, String)>>;
}

impl OptionalRow for rusqlite::Result<(String, String, String, String, String, String)> {
    fn optional_row(self) -> AppResult<Option<(String, String, String, String, String, String)>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// 轮询行：进行中任务 + 24h 内终态任务（终态随事件流保留，前端合并不再卡旧状态）
struct Row {
    id: i64,
    gid: String,
    file_name: String,
    platform: String,
    db_status: i32,
    cleanup_id: String,
    url: String,
    create_time: i64,
    total_size: i64,
    downloaded_size: i64,
    error_msg: String,
    save_path: String,
    active: bool,
}

/// 统计聚合：完成/失败时按（本地日, 平台）累加（独立表，清空任务记录不影响统计）
fn bump_stat(conn: &rusqlite::Connection, platform: &str, ok: bool, bytes: i64) {
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let files = if ok { 1 } else { 0 };
    let failed = if ok { 0 } else { 1 };
    let _ = conn.execute(
        "INSERT INTO download_stat (day, platform, files, bytes, failed) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(day, platform) DO UPDATE SET files = files + ?3, bytes = bytes + ?4, failed = failed + ?5",
        rusqlite::params![day, platform, files, if ok { bytes } else { 0 }, failed],
    );
}

/// 全部任务已结束且设置了完成后动作时，返回该动作（"shutdown" / "sleep"）
fn pending_after_complete(state: &AppState) -> Option<String> {
    let action = state.load_settings().after_download_action;
    if action != "shutdown" && action != "sleep" {
        return None;
    }
    let conn = state.db.lock().ok()?;
    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM download_task WHERE status IN (0, 1, 2)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if active == 0 {
        Some(action)
    } else {
        None
    }
}

/// 执行完成后动作：发系统预告通知后落盘执行（仅 Windows 桌面场景）
#[cfg(windows)]
async fn run_after_complete_action(app: &AppHandle, action: &str, last_file: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    fn build(program: &str, args: &[&str], flags: u32) -> std::process::Command {
        let mut c = std::process::Command::new(program);
        c.args(args);
        c.creation_flags(flags);
        c
    }

    let (title, body, mut cmd) = match action {
        // shutdown 自带 60 秒宽限，命令行执行 shutdown /a 可取消
        "shutdown" => (
            "即将关机",
            format!("全部任务已完成（{last_file}）。60 秒后关机，命令行执行 shutdown /a 可取消。"),
            build("shutdown", &["/s", "/t", "60"], CREATE_NO_WINDOW),
        ),
        "sleep" => (
            "即将睡眠",
            format!("全部任务已完成（{last_file}）。10 秒后系统进入睡眠。"),
            build(
                "rundll32.exe",
                &["powrprof.dll,SetSuspendState", "0,1,0"],
                CREATE_NO_WINDOW,
            ),
        ),
        _ => return,
    };
    engine_log(app, &format!("after_download: 触发 {action} 动作"));
    if app.state::<AppState>().load_settings().download_notify {
        let _ = app.notification().builder().title(title).body(body).show();
    }
    // 睡眠动作给一个短宽限期，避免终态通知尚未弹出即挂起
    if action == "sleep" {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
    let _ = cmd.spawn();
}

/// 非 Windows 平台不执行完成后动作（当前仅发布 Windows 桌面版）
#[cfg(not(windows))]
async fn run_after_complete_action(_app: &AppHandle, _action: &str, _last_file: &str) {}

/// 批量查询任务状态：一次 system.multicall 打包全部 tellStatus（N 个任务 1 次 HTTP 往返）
/// 单个 gid 查询失败（引擎不认识/已移除）对应位置返回 None
async fn tell_status_batch(gids: &[String]) -> Vec<Option<TaskStatus>> {
    if gids.is_empty() {
        return Vec::new();
    }
    // multicall 的子调用各自带 token，必须绕过 rpc_call 的自动外层 token 直发
    // （外层多出的 token 字符串会让 aria2 报 wrong type 并整体失败——
    //  这就是「界面状态永远冻结在排队中」的根因，详见 build_multicall_body 注释）
    let body = build_multicall_body(&rpc_secret(), gids);
    match rpc_post_raw(&body).await {
        Some(Value::Array(results)) if results.len() == gids.len() => results
            .into_iter()
            .map(|entry| match entry {
                Value::Array(inner) => inner.into_iter().next().map(|v| parse_status(&v)),
                _ => None,
            })
            .collect(),
        _ => vec![None; gids.len()],
    }
}

/// 事件载荷指纹：任一任务的关键字段变化才推送 downloads:updated（空闲时前端零 IPC）
fn views_fingerprint(views: &[DownloadTaskView]) -> String {
    use std::fmt::Write as _;
    let mut fp = String::with_capacity(views.len() * 64);
    for v in views {
        let _ = write!(
            fp,
            "{}/{}/{}/{}/{}/{}/{}/{};",
            v.id, v.status, v.total_size, v.downloaded_size, v.speed, v.error_msg, v.save_path, v.create_time
        );
    }
    fp
}

/// 终态任务保留 7 天后自动清理（查询窗口为 24h；7 天缓冲供翻旧账）。
/// download_task 无限增长会让 poll_loop 每秒的窗口查询线性变慢，这里兜底删除。
fn cleanup_expired_tasks(state: &AppState) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let cutoff = now_ms.saturating_sub(7 * 24 * 3600 * 1000);
    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "DELETE FROM download_task WHERE status IN (3, 4) AND finish_time > 0 AND finish_time < ?1",
            rusqlite::params![cutoff],
        );
    }
}

/// 轮询循环：批量拉取任务状态 → 变更检测后事件推送 + DB 节流写
/// 有进行中任务时 1s 一轮，空闲降为 3s；内容无变化不 emit、不动托盘
async fn poll_loop(app: AppHandle) {
    let mut last_persist = std::time::Instant::now() - std::time::Duration::from_secs(10);
    let mut last_tracker_refresh = std::time::Instant::now();
    let mut last_cleanup = std::time::Instant::now() - std::time::Duration::from_secs(3600);
    let mut last_emit_fp = String::new();
    let mut last_tray: (usize, i64) = (0, 0);
    let mut last_tray_at = std::time::Instant::now() - std::time::Duration::from_secs(10);
    let mut interval_secs = 1u64;
    let mut remount_strikes = 0u32;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
        let state = app.state::<AppState>();

        // BT Tracker 每 6 小时在线刷新并热更新到引擎（异步执行，不阻塞轮询）
        if last_tracker_refresh.elapsed() >= std::time::Duration::from_secs(6 * 3600) {
            last_tracker_refresh = std::time::Instant::now();
            if state.load_settings().bt_tracker_auto_update {
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move { refresh_trackers(&app2).await });
            }
        }

        // 每小时清理超期终态任务（持锁窗口仅一条 DELETE）
        if last_cleanup.elapsed() >= std::time::Duration::from_secs(3600) {
            last_cleanup = std::time::Instant::now();
            cleanup_expired_tasks(&state);
        }

        // 读取进行中任务 + 24h 内终态任务（进行中优先占用 LIMIT 名额）
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let finish_window = now_ms - 24 * 3600 * 1000;
        let rows: Vec<Row> = {
            let conn = match state.db.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let mut stmt = match conn.prepare(
                "SELECT id, gid, file_name, platform, status, cleanup_id, url, create_time, total_size, downloaded_size, error_msg, save_path \
                 FROM download_task \
                 WHERE status IN (0, 1, 2) OR (status IN (3, 4) AND finish_time > ?1) \
                 ORDER BY CASE WHEN status IN (0, 1, 2) THEN 0 ELSE 1 END, id DESC LIMIT 200",
            ) {
                Ok(s) => s,
                Err(_) => continue,
            };
            stmt.query_map(rusqlite::params![finish_window], |r| {
                let status: i32 = r.get(4)?;
                Ok(Row {
                    id: r.get(0)?,
                    gid: r.get(1)?,
                    file_name: r.get(2)?,
                    platform: r.get(3)?,
                    db_status: status,
                    cleanup_id: r.get(5)?,
                    url: r.get(6)?,
                    create_time: r.get(7)?,
                    total_size: r.get(8)?,
                    downloaded_size: r.get(9)?,
                    error_msg: r.get(10)?,
                    save_path: r.get(11)?,
                    active: status == DownloadTaskView::STATUS_PENDING
                        || status == DownloadTaskView::STATUS_DOWNLOADING
                        || status == DownloadTaskView::STATUS_PAUSED,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
        };
        let had_active = rows.iter().any(|r| r.active);

        // 活跃任务 gid 一次批量取回状态（替代逐任务串行 RPC）
        let active_gids: Vec<String> = rows
            .iter()
            .filter(|r| r.active && !r.gid.is_empty())
            .map(|r| r.gid.clone())
            .collect();
        let batch = tell_status_batch(&active_gids).await;
        // 引擎换代（崩溃自愈 / 进程被杀）会让所有活跃 gid 失联：后台触发重挂，
        // 期间界面短暂保持 DB 状态，重挂完成后按新 gid 继续轮询。
        // 大文件满速下载时引擎 RPC 偶发超时会让整批查询失败——单次失败不计，
        // 连续 3 轮全部失联才判定换代（否则高速下载期会每 10 秒误报一次重挂）。
        let unknown_active = batch.iter().filter(|x| x.is_none()).count();
        let active_total = active_gids.len();
        if active_total > 0 && unknown_active == active_total {
            remount_strikes += 1;
        } else {
            remount_strikes = 0;
        }
        if remount_strikes >= 3 {
            remount_strikes = 0;
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                remount_if_detached(&app2).await;
            });
        }
        let mut status_iter = batch.into_iter();

        let mut views: Vec<DownloadTaskView> = Vec::new();
        let mut persist_due = last_persist.elapsed() >= std::time::Duration::from_secs(2);
        // 节流进度落盘先收集、循环后单事务批量写（50 任务从 50 个写事务降到 1 个）
        let mut progress_updates: Vec<(i64, i32, i64, i64, String)> = Vec::new();

        for row in rows {
            let Row {
                id,
                gid,
                file_name,
                platform,
                db_status,
                cleanup_id,
                url,
                create_time,
                total_size: db_total,
                downloaded_size: db_done,
                error_msg: db_err,
                save_path: db_save,
                active,
            } = row;
            // 24h 内终态：直接从 DB 构建视图，不再询问 aria2
            if !active {
                views.push(DownloadTaskView {
                    id, gid, url, file_name, platform,
                    total_size: db_total, downloaded_size: db_done, speed: 0,
                    status: db_status, error_msg: db_err, save_path: db_save,
                    create_time,
                });
                continue;
            }
            if gid.is_empty() {
                continue;
            }
            let status = match status_iter.next().flatten() {
                Some(s) => s,
                None => {
                    // aria2 不认识该 gid（进程重启）或引擎不可达：保持 DB 状态
                    views.push(DownloadTaskView {
                        id, gid, url, file_name, platform,
                        total_size: db_total, downloaded_size: db_done, speed: 0,
                        status: db_status, error_msg: db_err, save_path: db_save,
                        create_time,
                    });
                    continue;
                }
            };
            let new_status = map_status(&status.status);
            let save_path = status.files.first().cloned().unwrap_or_default();

            // 完成转换：写终态 + 触发夸克延迟清理
            if new_status == DownloadTaskView::STATUS_COMPLETED && db_status != DownloadTaskView::STATUS_COMPLETED {
                {
                    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = conn.execute(
                        "UPDATE download_task SET status = 3, total_size = ?1, downloaded_size = ?1, save_path = ?2, finish_time = ?3 WHERE id = ?4",
                        rusqlite::params![status.total, save_path, now_ms, id],
                    );
                    bump_stat(&conn, &platform, true, status.total);
                }
                persist_due = true;
                state.log(
                    crate::logger::SUCCESS,
                    &platform,
                    "download",
                    &format!("下载完成：{file_name}"),
                    &format!("任务 #{id} size={} save={}", status.total, save_path),
                );
                // 下载完成系统通知（开关控制）
                if state.load_settings().download_notify {
                    let _ = app
                        .notification()
                        .builder()
                        .title("下载完成")
                        .body(file_name.clone())
                        .show();
                }
                // 转存清理与完成后动作移出轮询循环异步执行：baidupcs 清理（秒级）与 10s
                // 睡眠不再阻塞进度轮询，其余任务的进度/完成事件不再被拖住
                if !cleanup_id.is_empty() {
                    let app2 = app.clone();
                    let plat = platform.clone();
                    let cid = cleanup_id.clone();
                    tauri::async_runtime::spawn(async move {
                        let state_ref = app2.state::<AppState>();
                        cleanup_transfer(&state_ref, &plat, &cid).await;
                    });
                }
                // 下载完成后动作（关机 / 睡眠；仅当无其他进行中任务时触发）
                if let Some(action) = pending_after_complete(&state) {
                    let app2 = app.clone();
                    let last_file = file_name.clone();
                    tauri::async_runtime::spawn(async move {
                        run_after_complete_action(&app2, &action, &last_file).await;
                    });
                }
            } else if new_status == DownloadTaskView::STATUS_FAILED && db_status != DownloadTaskView::STATUS_FAILED {
                {
                    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = conn.execute(
                        "UPDATE download_task SET status = 4, error_msg = ?1, total_size = ?2, downloaded_size = ?3, finish_time = ?4 WHERE id = ?5",
                        rusqlite::params![status.error_msg, status.total, status.completed, now_ms, id],
                    );
                    bump_stat(&conn, &platform, false, 0);
                }
                persist_due = true;
                state.log(
                    crate::logger::ERROR,
                    &platform,
                    "download",
                    &format!("下载失败：{file_name}"),
                    &format!("任务 #{id} {}", status.error_msg),
                );
                // 下载失败系统通知（开关控制）
                if state.load_settings().download_notify {
                    let _ = app
                        .notification()
                        .builder()
                        .title("下载失败")
                        .body(format!("{file_name} · {}", status.error_msg))
                        .show();
                }
            } else if persist_due {
                // 节流持久化进度：先收集，循环结束后统一单事务写入
                progress_updates.push((id, new_status, status.total, status.completed, save_path.clone()));
            }

            views.push(DownloadTaskView {
                id,
                gid,
                url,
                file_name,
                platform,
                total_size: status.total,
                downloaded_size: status.completed,
                speed: status.speed,
                status: new_status,
                error_msg: status.error_msg,
                save_path,
                create_time,
            });
        }

        // 进度批量落盘：单事务替代逐行 autocommit UPDATE
        if persist_due && !progress_updates.is_empty() {
            if let Ok(mut conn) = state.db.lock() {
                if let Ok(tx) = conn.transaction() {
                    for (id, st, total, done, sp) in &progress_updates {
                        let _ = tx.execute(
                            "UPDATE download_task SET status = ?1, total_size = ?2, downloaded_size = ?3, save_path = ?4 WHERE id = ?5",
                            rusqlite::params![st, total, done, sp, id],
                        );
                    }
                    let _ = tx.commit();
                }
            }
        }
        if persist_due {
            last_persist = std::time::Instant::now();
        }
        // 变更检测：内容一致则跳过事件推送（空闲时前端零 IPC、零重渲染）
        let fp = views_fingerprint(&views);
        if fp != last_emit_fp {
            last_emit_fp = fp;
            let _ = app.emit("downloads:updated", &views);
            // 托盘 tooltip 汇总进行中任务数与总速度：内容变化 + 至少 5s 一次
            //（tooltip 仅悬停可见，速度每秒都变，逐秒刷新是纯浪费的系统调用）
            let active_count = views.iter().filter(|v| v.status == DownloadTaskView::STATUS_DOWNLOADING).count();
            let total_speed: i64 = views.iter().map(|v| v.speed).sum();
            if (active_count, total_speed) != last_tray && last_tray_at.elapsed() >= std::time::Duration::from_secs(5) {
                last_tray = (active_count, total_speed);
                last_tray_at = std::time::Instant::now();
                crate::tray::update_speed(&app, active_count, total_speed);
            }
        }
        // 自适应频率：有进行中任务保持 1s，空闲降为 3s
        interval_secs = if had_active { 1 } else { 3 };
    }
}

/// 任务列表（进行中 + 24h 内终态）：与 downloads:updated 事件窗口同一查询与排序，
/// 避免初始加载与事件流口径错位（前端 mergeTasks 只增改，超出事件窗口的行会变僵尸）。
pub fn list_tasks(app: &AppHandle) -> AppResult<Vec<DownloadTaskView>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let finish_window = now_ms - 24 * 3600 * 1000;
    let mut stmt = conn.prepare(
        "SELECT id, gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time \
         FROM download_task \
         WHERE status IN (0, 1, 2) OR (status IN (3, 4) AND finish_time > ?1) \
         ORDER BY CASE WHEN status IN (0, 1, 2) THEN 0 ELSE 1 END, id DESC LIMIT 200",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![finish_window], |r| {
            Ok(DownloadTaskView {
                id: r.get(0)?,
                gid: r.get(1)?,
                url: r.get(2)?,
                file_name: r.get(3)?,
                platform: r.get(4)?,
                total_size: r.get(5)?,
                downloaded_size: r.get(6)?,
                speed: 0,
                status: r.get(7)?,
                error_msg: r.get(8)?,
                save_path: r.get(9)?,
                create_time: r.get(10)?,
            })
        })?
        .filter_map(Result::ok)
        .collect();
    Ok(rows)
}

/// 拉取单个任务完整详情（含 aria2 tellStatus 扩展字段，供 Dashboard 面板）
pub async fn detail(app: &AppHandle, id: i64) -> AppResult<DownloadDetail> {
    let state = app.state::<AppState>();
    let row = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        load_detail_row(&conn, id)?
    };

    let (
        gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time,
    ) = row;

    let mut connections = 0i32;
    let mut upload_speed = 0i64;
    let mut total_time = 0i64;
    let mut speed = 0i64;
    if !gid.is_empty() {
        if let Ok(v) = rpc_call("aria2.tellStatus", vec![json!(gid)]).await {
            speed = parse_status(&v).speed;
            total_time = v.get("totalTime").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0);
            connections = v.get("connections").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0);
            upload_speed = v.get("uploadSpeed").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0);
        }
    }

    Ok(DownloadDetail {
        id,
        gid,
        url,
        file_name,
        platform,
        total_size,
        downloaded_size,
        speed,
        status,
        error_msg,
        save_path,
        create_time,
        connections,
        upload_speed,
        total_time,
    })
}

type DetailDbRow = (String, String, String, String, i64, i64, i32, String, String, i64);

fn load_detail_row(conn: &rusqlite::Connection, id: i64) -> AppResult<DetailDbRow> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time \
         FROM download_task WHERE id = ?1",
        rusqlite::params![id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?)),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("下载任务 #{id} 不存在")))
}

/// 设置变更后同步 aria2（限速 / 并发 / 代理）。
/// 注意：aria2 对 all-proxy 的运行中修改支持有限，代理变更彻底生效仍需重启引擎
///（重启后由启动参数 --all-proxy 注入）；此处 best-effort 尝试即时更新。
pub async fn apply_settings(app: &AppHandle, settings: &Settings) -> AppResult<()> {
    let mut options = json!({
        "max-overall-download-limit": limit_str(settings.download_speed_limit),
        "max-concurrent-downloads": settings.max_concurrent_downloads.max(1),
    });
    options["all-proxy"] = json!(if proxy_configured(settings) { build_proxy_arg(settings) } else { String::new() });
    if let Err(error) = rpc_call("aria2.changeGlobalOption", vec![options]).await {
        engine_log(app, "apply_settings: changeGlobalOption 失败（代理/限速可能需要重启引擎后生效）");
        // 不再包含「设置已保存」上下文：该错误作为 engineSyncError 由前端以非阻塞提示展示
        return Err(AppError::Api(format!("限速 / 并发 / 代理同步失败，重启引擎后生效：{error}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clear_targets, http_task_options, load_detail_row, mark_enqueue_failed_in_db, parse_status, proxy_log_summary, sanitize_out_path, stall_guard_options, transfer_tuning};
    use crate::error::AppError;
    use crate::models::Settings;

    #[test]
    fn proxy_log_never_contains_credentials() {
        let mut settings = Settings::default();
        settings.proxy_type = "http".into();
        settings.proxy_host = "127.0.0.1".into();
        settings.proxy_port = 7890;
        settings.proxy_username = "secret-user".into();
        settings.proxy_password = "secret-password".into();
        let line = proxy_log_summary(&settings);
        assert!(line.contains("host=127.0.0.1"));
        assert!(line.contains("auth=true"));
        assert!(!line.contains("secret-user"));
        assert!(!line.contains("secret-password"));
    }

    #[test]
    fn quark_transfer_uses_conservative_single_server_connections() {
        let tuning = transfer_tuning("quark", 32, 16, 1);
        assert_eq!(tuning, (4, 4));
    }

    #[test]
    fn only_quark_tasks_carry_lowest_speed_limit_guard() {
        let quark = stall_guard_options("quark");
        assert_eq!(quark, vec![("lowest-speed-limit".to_string(), "1024".to_string())]);
        assert!(stall_guard_options("baidu").is_empty());
        assert!(stall_guard_options("uc").is_empty());
    }

    #[test]
    fn http_task_options_embed_extra_options_and_sanitize_out() {
        let extra = stall_guard_options("quark");
        let options = http_task_options(
            "D:/downloads",
            "../../evil.exe",
            &["Cookie: __puus=live".to_string()],
            4,
            4,
            "4M",
            3,
            &extra,
        );
        assert_eq!(options["out"], "evil.exe");
        assert_eq!(options["split"], 4);
        assert_eq!(options["max-connection-per-server"], 4);
        assert_eq!(options["lowest-speed-limit"], "1024");
        assert_eq!(options["continue"], "true");
        let headers = options["header"].as_array().unwrap();
        assert_eq!(headers.len(), 1);
        assert!(headers[0].as_str().unwrap().starts_with("Cookie: "));
    }

    #[test]
    fn numeric_aria2_error_code_is_exposed_in_failure_message() {
        let status = parse_status(&serde_json::json!({
            "status": "error",
            "errorCode": 13,
            "errorMessage": "",
        }));
        assert_eq!(status.error_msg, "aria2 错误码 13");
    }

    #[test]
    fn enqueue_failure_and_clear_cleanup_targets_are_persisted() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE download_task (
                id INTEGER PRIMARY KEY, gid TEXT, url TEXT, file_name TEXT, platform TEXT,
                total_size INTEGER DEFAULT 0, downloaded_size INTEGER DEFAULT 0,
                status INTEGER, error_msg TEXT, save_path TEXT DEFAULT '', create_time INTEGER,
                finish_time INTEGER, cleanup_id TEXT
            );
            INSERT INTO download_task VALUES (1, '', 'u1', 'a.bin', 'quark', 0, 0, 0, '', '', 1, 0, 'cleanup-a');
            INSERT INTO download_task VALUES (2, 'gid-b', 'u2', 'b.bin', 'baidu', 0, 0, 1, '', '', 2, 0, 'cleanup-b');",
        ).unwrap();
        mark_enqueue_failed_in_db(&conn, 1, "rpc failed", 99).unwrap();
        let failed: (i32, String, i64, String) = conn.query_row(
            "SELECT status, error_msg, finish_time, gid FROM download_task WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(failed, (crate::models::DownloadTaskView::STATUS_FAILED, "rpc failed".into(), 99, "".into()));
        let targets = clear_targets(&conn).unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|(_, platform, cleanup)| platform == "quark" && cleanup == "cleanup-a"));
        assert!(targets.iter().any(|(_, platform, cleanup)| platform == "baidu" && cleanup == "cleanup-b"));
    }

    #[test]
    fn missing_download_detail_is_not_found() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE download_task (
                id INTEGER PRIMARY KEY, gid TEXT, url TEXT, file_name TEXT, platform TEXT,
                total_size INTEGER, downloaded_size INTEGER, status INTEGER, error_msg TEXT,
                save_path TEXT, create_time INTEGER
            );",
        ).unwrap();
        assert!(matches!(load_detail_row(&conn, 404), Err(AppError::NotFound(_))));
    }

    #[test]
    fn sanitize_out_path_blocks_traversal_and_drive_components() {
        // 路径穿越分量整体剥离，不改变剩余结构
        assert_eq!(sanitize_out_path("../../evil.exe"), "evil.exe");
        assert_eq!(sanitize_out_path("a/../../b/c.bin"), "a/b/c.bin");
        assert_eq!(sanitize_out_path("..\\..\\x.zip"), "x.zip");
        // 盘符与绝对路径前缀剥掉
        assert_eq!(sanitize_out_path("C:\\Users\\web\\file.zip"), "Users/web/file.zip");
        assert_eq!(sanitize_out_path("/etc/passwd"), "etc/passwd");
        // 反斜杠 / URL 解码后的分隔符一律按分量切
        assert_eq!(sanitize_out_path("a%2F..%2F..%2Fb"), "a%2F..%2F..%2Fb");
    }

    #[test]
    fn sanitize_out_path_handles_windows_reserved_and_edge_cases() {
        // Windows 保留名加下划线前缀（含带扩展名形式）
        assert_eq!(sanitize_out_path("CON"), "_CON");
        assert_eq!(sanitize_out_path("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_out_path("com1.dat"), "_com1.dat");
        // 尾点尾空格剔除（Windows 会吞掉它们造成歧义）
        assert_eq!(sanitize_out_path("file."), "file");
        assert_eq!(sanitize_out_path("name "), "name");
        // relDir/文件名 子目录结构保留
        assert_eq!(sanitize_out_path("剧集/S01/E01.mkv"), "剧集/S01/E01.mkv");
        // 全空 / 纯穿越回退默认名
        assert_eq!(sanitize_out_path(""), "download.bin");
        assert_eq!(sanitize_out_path("../.."), "download.bin");
        assert_eq!(sanitize_out_path("C:\\"), "download.bin");
    }
}
