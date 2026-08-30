//! aria2 下载引擎（sidecar 外部二进制 + JSON-RPC 管理）。
//! 启动：aria2c --enable-rpc --rpc-listen-port=16800 --rpc-secret --stop-with-process=<自身PID>
//! 任务：addUri 携带 header（Cookie/UA/Referer）+ out 文件名 + split 并发；
//! 进度：1s 轮询 tellStatus → 事件推送前端 + DB 节流持久化；完成后触发夸克延迟清理。
use std::sync::OnceLock;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_shell::ShellExt;

use crate::error::{AppError, AppResult};
use crate::models::{DownloadDetail, DownloadTaskView, Settings};
use crate::state::AppState;

const RPC_PORT: u16 = 16800;

fn rpc_secret() -> &'static str {
    static SECRET: OnceLock<String> = OnceLock::new();
    SECRET.get_or_init(|| crate::api::random_alnum(24))
}

fn rpc_url() -> String {
    format!("http://127.0.0.1:{RPC_PORT}/jsonrpc")
}

/// aria2 任务快照（tellStatus 关键字段）
#[derive(Debug, Clone, Default)]
struct TaskStatus {
    status: String,       // active/waiting/paused/error/complete/removed
    total: i64,
    completed: i64,
    speed: i64,
    error_msg: String,
    files: Vec<String>,   // 实际落盘路径
}

async fn rpc_call(method: &str, params: Vec<Value>) -> AppResult<Value> {
    let http = reqwest::Client::new();
    // aria2 JSON-RPC：params 数组首元素为 "token:<secret>"，其后为实际参数
    let mut all_params: Vec<Value> = vec![json!(format!("token:{}", rpc_secret()))];
    all_params.extend(params);
    let body = json!({
        "jsonrpc": "2.0",
        "id": "yunx",
        "method": method,
        "params": all_params,
    });
    let resp = http
        .post(rpc_url())
        .timeout(std::time::Duration::from_secs(10))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Api(format!("下载引擎通信失败: {e}")))?;
    let v: Value = resp.json().await?;
    if let Some(err) = v.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("RPC 错误");
        return Err(AppError::Api(format!("下载引擎: {msg}")));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

/// aria2 状态 → 任务状态常量
fn map_status(s: &str) -> i32 {
    match s {
        "active" => DownloadTaskView::STATUS_DOWNLOADING,
        "waiting" => DownloadTaskView::STATUS_PENDING,
        "paused" => DownloadTaskView::STATUS_PAUSED,
        "complete" => DownloadTaskView::STATUS_COMPLETED,
        "error" => DownloadTaskView::STATUS_FAILED,
        _ => DownloadTaskView::STATUS_DOWNLOADING,
    }
}

fn parse_status(v: &Value) -> TaskStatus {
    let s = |key: &str| v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let files = v
        .get("files")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.get("path").and_then(|p| p.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    TaskStatus {
        status: s("status"),
        total: v.get("totalLength").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        completed: v.get("completedLength").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        speed: v.get("downloadSpeed").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0),
        error_msg: s("errorMessage"),
        files,
    }
}

// ---------- 启动（sidecar） ----------

/// 引擎诊断日志（data_dir/engine.log；启动链路排查）
fn engine_log(app: &AppHandle, msg: &str) {
    let state = app.state::<AppState>();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.data_dir.join("engine.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%m-%d %H:%M:%S"), msg);
    }
}

/// 启动 aria2 进程并进入轮询循环（setup 时调用一次）
pub async fn start(app: AppHandle) {
    engine_log(&app, "start: 引擎启动流程开始");
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let download_dir = resolve_download_dir(&app, &settings);
    engine_log(&app, &format!("start: 下载目录 {}", download_dir.display()));

    // 启动 sidecar 进程
    let shell = app.shell();
    match shell.sidecar("aria2c") {
        Ok(cmd) => {
            engine_log(&app, "start: sidecar 已解析，准备 spawn");
            let mut args = vec![
                "--enable-rpc".to_string(),
                format!("--rpc-listen-port={RPC_PORT}"),
                format!("--rpc-secret={}", rpc_secret()),
                format!("--dir={}", download_dir.display()),
                "--continue=true".to_string(),
                format!("--max-concurrent-downloads={}", settings.max_concurrent_downloads.max(1)),
                format!("--split={}", settings.download_threads.clamp(1, 64)),
                format!("--max-connection-per-server={}", settings.download_conn_per_server.clamp(1, 16)),
                format!("--min-split-size={}M", settings.download_min_split_mb.clamp(1, 64)),
                "--file-allocation=none".to_string(),
                "--allow-overwrite=true".to_string(),
                "--auto-file-renaming=true".to_string(),
                format!("--max-tries={}", settings.download_retry_count.clamp(0, 10)),
                "--retry-wait=3".to_string(),
                format!("--max-overall-download-limit={}", limit_str(settings.download_speed_limit)),
                "--console-log-level=warn".to_string(),
                format!("--stop-with-process={}", std::process::id()),
            ];
            // 代理注入（设置开启且填写完整时；aria2 全局代理影响所有连接）
            if proxy_configured(&settings) {
                let proxy = build_proxy_arg(&settings);
                args.push(format!("--all-proxy={proxy}"));
                engine_log(&app, &format!("start: 已注入代理 --all-proxy={proxy}"));
            }
            let cmd = cmd.args(args);
            match cmd.spawn() {
                Ok((mut rx, _child)) => {
                    engine_log(&app, "start: aria2 进程已 spawn");
                    // 捕获 aria2 输出（stderr 诊断 → engine.log）
                    let app_log = app.clone();
                    tauri::async_runtime::spawn(async move {
                        while let Some(evt) = rx.recv().await {
                            if let tauri_plugin_shell::process::CommandEvent::Stderr(line) = evt {
                                engine_log(&app_log, &format!("aria2: {}", String::from_utf8_lossy(&line)));
                            }
                        }
                    });
                }
                Err(e) => engine_log(&app, &format!("start: aria2 spawn 失败 {e:?}")),
            }
        }
        Err(e) => engine_log(&app, &format!("start: sidecar 解析失败 {e:?}")),
    }

    // 等待 RPC 就绪
    let mut ready = false;
    let mut attempts = 0;
    for i in 0..30 {
        attempts = i + 1;
        if rpc_call("aria2.getVersion", vec![]).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    engine_log(&app, &format!("start: RPC 就绪={ready}（尝试 {attempts} 轮）"));

    // 恢复未完成任务（aria2 重启后 gid 失效，重新入队续传）
    resume_pending_tasks(&app).await;

    // 轮询循环：1s 拉取任务状态 → 事件 + DB 节流持久化
    poll_loop(app).await;
}

/// 解析下载目录（自定义目录 → 系统下载文件夹）
pub fn resolve_download_dir(app: &AppHandle, settings: &Settings) -> std::path::PathBuf {
    if !settings.download_dir.is_empty() {
        let p = std::path::PathBuf::from(&settings.download_dir);
        if p.is_dir() {
            return p;
        }
    }
    app.path()
        .download_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
}

fn limit_str(limit: i64) -> String {
    if limit <= 0 {
        "0".into()
    } else {
        format!("{}B", limit) // aria2 接受 "1048576B" 形式
    }
}

/// 构建 `--all-proxy` 参数（http / socks5，含可选用户密码，密码经 URL 编码）
pub(crate) fn build_proxy_arg(settings: &Settings) -> String {
    let scheme = if settings.proxy_type.eq_ignore_ascii_case("socks5") { "socks5" } else { "http" };
    let mut url = format!("{scheme}://");
    if !settings.proxy_username.is_empty() {
        url.push_str(&urlencoding::encode(&settings.proxy_username));
        url.push(':');
        if !settings.proxy_password.is_empty() {
            url.push_str(&urlencoding::encode(&settings.proxy_password));
        }
        url.push('@');
    }
    url.push_str(settings.proxy_host.trim());
    if settings.proxy_port > 0 {
        url.push(':');
        url.push_str(&settings.proxy_port.to_string());
    }
    url
}

/// 设置中代理是否已填完整（可用）
pub(crate) fn proxy_configured(settings: &Settings) -> bool {
    settings.proxy_enabled
        && !settings.proxy_host.trim().is_empty()
        && settings.proxy_port > 0
}

// ---------- 任务入队 / 控制 ----------

/// 入队下载（插入 DB → addUri → 回写 gid）
pub async fn enqueue(
    app: &AppHandle,
    url: &str,
    file_name: &str,
    headers: &[(String, String)],
    platform: &str,
    cleanup_id: &str,
    start_paused: bool,
) -> AppResult<i64> {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let dir = resolve_download_dir(app, &settings);

    let headers_json = serde_json::to_string(&headers)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let id = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "INSERT INTO download_task (url, file_name, request_headers_json, platform, cleanup_id, create_time, status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0) RETURNING id",
            rusqlite::params![url, file_name, headers_json, platform, cleanup_id, now],
            |r| r.get::<_, i64>(0),
        )?
    };

    // aria2 addUri：header 列表 + out 文件名 + split 并发
    let mut header_list: Vec<String> = headers.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    if !header_list.iter().any(|h| h.to_lowercase().starts_with("user-agent")) {
        header_list.push(format!("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));
    }
    let mut options = json!({
        "dir": dir.display().to_string(),
        "out": file_name,
        "header": header_list,
        "split": settings.download_threads.clamp(1, 64),
        "max-connection-per-server": settings.download_conn_per_server.clamp(1, 16),
        "min-split-size": format!("{}M", settings.download_min_split_mb.clamp(1, 64)),
        "continue": "true",
        "max-tries": settings.download_retry_count.clamp(0, 10),
    });
    if start_paused {
        options["paused"] = json!("true");
    }
    let gid = rpc_call("aria2.addUri", vec![json!([url]), options])
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
        &format!("任务 #{id} gid={gid} split={} {}", settings.download_threads, if cleanup_id.is_empty() { String::new() } else { format!("cleanup={cleanup_id}") }),
    );
    Ok(id)
}

/// 暂停（remove + paused=true 语义：aria2 pause 保留进度）
pub async fn pause(app: &AppHandle, id: i64) -> AppResult<()> {
    let gid = gid_of(app, id)?;
    if !gid.is_empty() {
        rpc_call("aria2.pause", vec![json!(gid)]).await?;
    }
    update_status(app, id, DownloadTaskView::STATUS_PAUSED, "").await
}

/// 恢复
pub async fn resume(app: &AppHandle, id: i64) -> AppResult<()> {
    let gid = gid_of(app, id)?;
    if !gid.is_empty() {
        let _ = rpc_call("aria2.unpause", vec![json!(gid)]).await;
    }
    update_status(app, id, DownloadTaskView::STATUS_DOWNLOADING, "").await
}

/// 删除任务（aria2 remove + DB 删除 + 夸克转存清理）
pub async fn remove(app: &AppHandle, id: i64, delete_local: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (gid, file_name, cleanup_id, platform) = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT gid, file_name, cleanup_id, platform FROM download_task WHERE id = ?1",
            rusqlite::params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
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
        let dir = resolve_download_dir(app, &state.load_settings());
        for name in [file_name.clone(), format!("{file_name}.aria2")] {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("DELETE FROM download_task WHERE id = ?1", rusqlite::params![id])?;
    }
    // 删除任务同样触发夸克转存清理（用户放弃下载）
    if platform == "quark" && !cleanup_id.is_empty() {
        crate::resolve::cleanup_quark(&state, &cleanup_id).await;
    }
    Ok(())
}

/// 清空全部下载任务记录（aria2 全部 forceRemove + DB 清空）
pub async fn clear_all(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    // 收集全部 gid 并强制移除（含进行中/等待/暂停）
    let gids: Vec<String> = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        let mut stmt = conn.prepare("SELECT gid FROM download_task WHERE gid != ''")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.filter_map(Result::ok).collect()
    };
    for gid in gids {
        let _ = rpc_call("aria2.forceRemove", vec![json!(gid)]).await;
        let _ = rpc_call("aria2.removeDownloadResult", vec![json!(gid)]).await;
    }
    {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.execute("DELETE FROM download_task", [])?;
    }
    // 清空后向前端推送空列表
    let views: Vec<DownloadTaskView> = Vec::new();
    let _ = app.emit("downloads:updated", &views);
    Ok(())
}

/// 暂停全部进行中/等待任务（aria2.pauseAll + DB 置暂停态 + 事件）
pub async fn pause_all(app: &AppHandle) -> AppResult<()> {
    let _ = rpc_call("aria2.pauseAll", vec![]).await;
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
    let _ = rpc_call("aria2.unpauseAll", vec![]).await;
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
    fn optional_row(self) -> AppResult<Option<(String, String, String, String)>>;
}

impl OptionalRow for rusqlite::Result<(String, String, String, String)> {
    fn optional_row(self) -> AppResult<Option<(String, String, String, String)>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

// ---------- 启动恢复 + 轮询 ----------

/// 恢复未完成任务（重新 addUri，paused 状态的以暂停态入队）
async fn resume_pending_tasks(app: &AppHandle) {
    let state = app.state::<AppState>();
    let rows: Vec<(i64, String, String, String, String, String, bool)> = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare(
            "SELECT id, url, file_name, request_headers_json, platform, cleanup_id, status \
             FROM download_task WHERE status IN (0, 1, 2)",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i32>(6)? == DownloadTaskView::STATUS_PAUSED,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();
        rows
    };
    for (id, url, file_name, headers_json, platform, cleanup_id, was_paused) in rows {
        let headers: Vec<(String, String)> =
            serde_json::from_str(&headers_json).unwrap_or_default();
        // 清掉旧 gid（新 aria2 实例不认识）
        {
            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
            let _ = conn.execute("UPDATE download_task SET gid = '' WHERE id = ?1", rusqlite::params![id]);
        }
        if let Err(e) = enqueue(app, &url, &file_name, &headers, &platform, &cleanup_id, was_paused).await {
            eprintln!("[yunx] 恢复任务 {id} 失败: {e}");
            let _ = update_status(app, id, DownloadTaskView::STATUS_FAILED, &e.to_string()).await;
        }
    }
}

/// 轮询循环：1s 拉取所有进行中任务状态 → 事件推送 + DB 节流写
async fn poll_loop(app: AppHandle) {
    let mut last_persist = std::time::Instant::now() - std::time::Duration::from_secs(10);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let state = app.state::<AppState>();

        // 读取进行中任务（含刚完成的，短暂保留）
        let rows: Vec<(i64, String, String, String, i32, String)> = {
            let conn = match state.db.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let mut stmt = match conn.prepare(
                "SELECT id, gid, file_name, platform, status, cleanup_id \
                 FROM download_task WHERE status IN (0, 1, 2) OR (status = 3 AND create_time > 0 AND save_path != '') LIMIT 200",
            ) {
                Ok(s) => s,
                Err(_) => continue,
            };
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i32>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
        };
        // 完成态的任务由状态转换时已写入 save_path，这里只需查进行中的
        let rows: Vec<_> = rows.into_iter().filter(|r| r.4 != DownloadTaskView::STATUS_COMPLETED).collect();

        let mut views: Vec<DownloadTaskView> = Vec::new();
        let mut persist_due = last_persist.elapsed() >= std::time::Duration::from_secs(2);

        for (id, gid, file_name, platform, db_status, cleanup_id) in rows {
            if gid.is_empty() {
                continue;
            }
            let status = match rpc_call("aria2.tellStatus", vec![json!(gid)]).await {
                Ok(v) => parse_status(&v),
                Err(_) => {
                    // aria2 不认识该 gid（进程重启）：保持 DB 状态
                    views.push(DownloadTaskView {
                        id, gid, url: String::new(), file_name, platform,
                        total_size: 0, downloaded_size: 0, speed: 0,
                        status: db_status, error_msg: String::new(), save_path: String::new(),
                        create_time: 0,
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
                        "UPDATE download_task SET status = 3, total_size = ?1, downloaded_size = ?1, save_path = ?2 WHERE id = ?3",
                        rusqlite::params![status.total, save_path, id],
                    );
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
                if platform == "quark" && !cleanup_id.is_empty() {
                    let state_ref = app.state::<AppState>();
                    crate::resolve::cleanup_quark(&state_ref, &cleanup_id).await;
                }
            } else if new_status == DownloadTaskView::STATUS_FAILED && db_status != DownloadTaskView::STATUS_FAILED {
                {
                    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = conn.execute(
                        "UPDATE download_task SET status = 4, error_msg = ?1, total_size = ?2, downloaded_size = ?3 WHERE id = ?4",
                        rusqlite::params![status.error_msg, status.total, status.completed, id],
                    );
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
                // 节流持久化进度
                let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                let _ = conn.execute(
                    "UPDATE download_task SET status = ?1, total_size = ?2, downloaded_size = ?3, save_path = ?4 WHERE id = ?5",
                    rusqlite::params![new_status, status.total, status.completed, save_path, id],
                );
            }

            views.push(DownloadTaskView {
                id,
                gid,
                url: String::new(),
                file_name,
                platform,
                total_size: status.total,
                downloaded_size: status.completed,
                speed: status.speed,
                status: new_status,
                error_msg: status.error_msg,
                save_path,
                create_time: 0,
            });
        }

        if persist_due {
            last_persist = std::time::Instant::now();
        }
        let _ = app.emit("downloads:updated", &views);
        // 托盘 tooltip 汇总进行中任务数与总速度
        let active_count = views.iter().filter(|v| v.status == DownloadTaskView::STATUS_DOWNLOADING).count();
        let total_speed: i64 = views.iter().map(|v| v.speed).sum();
        crate::tray::update_speed(&app, active_count, total_speed);
    }
}

/// 全量任务列表（含已完成/失败；页面切换时拉取）
pub fn list_tasks(app: &AppHandle) -> AppResult<Vec<DownloadTaskView>> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time \
         FROM download_task ORDER BY create_time DESC LIMIT 500",
    )?;
    let rows = stmt
        .query_map([], |r| {
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
    use rusqlite::OptionalExtension;
    let state = app.state::<AppState>();
    let row: Option<(String, String, String, String, i64, i64, i32, String, String, i64)> = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time \
             FROM download_task WHERE id = ?1",
            rusqlite::params![id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                    r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?,
                ))
            },
        )
        .optional()?
    };

    let (
        gid, url, file_name, platform, total_size, downloaded_size, status, error_msg, save_path, create_time,
    ) = row.unwrap_or_default();

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

/// 设置变更后同步 aria2（限速 / 并发 / 代理）。
/// 注意：aria2 对 all-proxy 的运行中修改支持有限，代理变更彻底生效仍需重启引擎
///（重启后由启动参数 --all-proxy 注入）；此处 best-effort 尝试即时更新。
pub async fn apply_settings(app: &AppHandle, settings: &Settings) {
    let mut options = json!({
        "max-overall-download-limit": limit_str(settings.download_speed_limit),
        "max-concurrent-downloads": settings.max_concurrent_downloads.max(1),
    });
    if proxy_configured(settings) {
        options["all-proxy"] = json!(build_proxy_arg(settings));
    }
    if rpc_call("aria2.changeGlobalOption", vec![options]).await.is_err() {
        engine_log(app, "apply_settings: changeGlobalOption 失败（代理/限速可能需要重启引擎后生效）");
    }
}
