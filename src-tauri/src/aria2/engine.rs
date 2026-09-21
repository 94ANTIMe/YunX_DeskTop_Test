//! aria2 引擎生命周期（ADR-0007 C5）：sidecar 拉起、健康自愈、失联重挂、
//! BT tracker 拉取与热更新、启动恢复。RPC 细节见 super::rpc，任务状态机见 mod.rs。

use std::sync::OnceLock;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use crate::models::{DownloadTaskView, Settings};
use crate::state::AppState;

use super::rpc::{
    kill_stale_aria2_on_port, rpc_call, rpc_call_raw, rpc_secret, RPC_PORT, APP_HANDLE,
};
use super::policy::{build_proxy_arg, limit_str, proxy_configured, proxy_log_summary};
use super::{poll_loop, torrent_options, refreshed_target};
use super::{add_to_aria2, update_status};

/// 全球活跃度最高的高速公共 BitTorrent Tracker 列表（定期自愈注入）
pub const DEFAULT_BT_TRACKERS: &str = "\
udp://tracker.opentrackr.org:1337/announce,\
udp://open.demonii.com:1337/announce,\
udp://open.stealth.si:80/announce,\
udp://tracker.torrent.eu.org:451/announce,\
udp://explodie.org:6969/announce,\
udp://tracker.tiny-vps.com:6969/announce,\
udp://p4p.arenabg.com:1337/announce,\
udp://tracker.moeking.me:6969/announce,\
https://tracker.tamersunion.org:443/announce,\
http://tracker.dler.org:6969/announce,\
udp://tracker.altrosky.nl:6969/announce";

/// BT Tracker 在线列表源（借鉴 Motrix；依次回退：GitHub 原始 → jsDelivr → ghproxy 镜像）
const TRACKER_SOURCES: &[&str] = &[
    "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/all.txt",
    "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/all.txt",
    "https://ghproxy.net/https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/all.txt",
];
/// 动态 tracker 列表上限（控制 aria2 启动参数长度）
const MAX_TRACKERS: usize = 300;

/// 当前生效的 tracker 列表（拉取成功后热更新；入队与引擎启动参数共用）
static TRACKERS: OnceLock<std::sync::RwLock<String>> = OnceLock::new();

pub(crate) fn trackers_cell() -> &'static std::sync::RwLock<String> {
    TRACKERS.get_or_init(|| std::sync::RwLock::new(DEFAULT_BT_TRACKERS.to_string()))
}

/// 当前 tracker 列表（逗号分隔）
pub fn current_bt_trackers() -> String {
    trackers_cell()
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|_| DEFAULT_BT_TRACKERS.to_string())
}

/// 解析 tracker 文本：去空行 / 注释 / 非法行，去重并截断上限
pub(crate) fn parse_trackers(text: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || !t.contains("://") {
            continue;
        }
        if seen.insert(t.to_string()) {
            out.push(t);
            if out.len() >= MAX_TRACKERS {
                break;
            }
        }
    }
    out.join(",")
}

/// 拉取最新 tracker 列表：在线源依次回退 → 本地缓存（data_dir/bt_trackers.txt）→ 内置列表。
/// 成功后更新全局缓存并落盘供离线兜底。
pub(crate) async fn fetch_bt_trackers(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));
    if proxy_configured(&settings) {
        if let Ok(p) = reqwest::Proxy::all(build_proxy_arg(&settings)) {
            builder = builder.proxy(p);
        }
    }
    let http = builder.build().unwrap_or_else(|_| reqwest::Client::new());
    for src in TRACKER_SOURCES {
        match http.get(*src).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) => {
                    let list = parse_trackers(&text);
                    if list.len() > 50 {
                        if let Ok(mut cell) = trackers_cell().write() {
                            *cell = list.clone();
                        }
                        let _ = std::fs::write(state.data_dir.join("bt_trackers.txt"), &list);
                        engine_log(
                            app,
                            &format!(
                                "fetch_bt_trackers: 已更新 {} 个 tracker（来源 {src}）",
                                list.split(',').count()
                            ),
                        );
                        return list;
                    }
                    engine_log(app, &format!("fetch_bt_trackers: 源返回无效内容 {src}"));
                }
                Err(e) => engine_log(app, &format!("fetch_bt_trackers: 源读取失败 {src} {e}")),
            },
            Err(e) => engine_log(app, &format!("fetch_bt_trackers: 源不可用 {src} {e}")),
        }
    }
    if let Ok(cached) = std::fs::read_to_string(state.data_dir.join("bt_trackers.txt")) {
        let cached = cached.trim().to_string();
        if cached.len() > 50 {
            engine_log(app, "fetch_bt_trackers: 在线拉取失败，使用本地缓存");
            return cached;
        }
    }
    engine_log(app, "fetch_bt_trackers: 在线与缓存均不可用，使用内置列表");
    DEFAULT_BT_TRACKERS.to_string()
}

/// 拉取最新列表并热更新到运行中的引擎（启动首拉 + poll_loop 周期调用）
pub(crate) async fn refresh_trackers(app: &AppHandle) {
    let list = fetch_bt_trackers(app).await;
    if rpc_call("aria2.changeGlobalOption", vec![json!({ "bt-tracker": list })])
        .await
        .is_err()
    {
        engine_log(app, "refresh_trackers: changeGlobalOption 失败（引擎未就绪？）");
    }
}


/// 引擎自愈互斥：并发 rpc_call 同时触发重拉时只执行一次，其余排队后复检
pub(crate) static HEAL_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
/// 重拉冷却：spawn 失败（依赖缺失等）时避免 poll_loop 每秒风暴式重拉
pub(crate) static LAST_RESPAWN: OnceLock<std::sync::Mutex<Option<std::time::Instant>>> = OnceLock::new();
/// 重挂冷却：poll 检测到 gid 失联后避免连续重挂风暴
pub(crate) static LAST_REMOUNT: OnceLock<std::sync::Mutex<Option<std::time::Instant>>> = OnceLock::new();

pub(crate) fn heal_lock() -> &'static tokio::sync::Mutex<()> {
    HEAL_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// 查杀端口遗留进程并重拉引擎（互斥 + 5 秒冷却）。
/// 拿到锁后先复检引擎健康——并发场景下可能已有其他调用方完成自愈。
pub(crate) async fn respawn_engine(app: &AppHandle) -> bool {
    let _guard = heal_lock().lock().await;
    if rpc_call_raw("aria2.getVersion", vec![]).await.is_ok() {
        return true;
    }
    {
        let cell = LAST_RESPAWN.get_or_init(|| std::sync::Mutex::new(None));
        let Ok(mut last) = cell.lock() else {
            return false;
        };
        if let Some(t) = *last {
            if t.elapsed() < std::time::Duration::from_secs(5) {
                return false;
            }
        }
        *last = Some(std::time::Instant::now());
    }
    engine_log(app, "respawn_engine: 引擎无响应，查杀遗留进程后重拉");
    // netstat/tasklist/taskkill 是同步子进程，放 blocking 线程执行，避免冻结 tokio worker
    let _ = tokio::task::spawn_blocking(|| kill_stale_aria2_on_port(RPC_PORT)).await;
    spawn_sidecar(app).await
}

/// 引擎健康但活跃任务 gid 全部失联（引擎进程被换过）→ 复用启动恢复逻辑重挂。
/// 不在 respawn_engine 里做：自愈发生在 rpc_call 内部，原地重挂会与原请求的重试 addUri
/// 撞车造成同一任务双入队；poll 触发点没有在途 addUri，且复用 live gid 复检天然幂等。
pub(crate) async fn remount_if_detached(app: &AppHandle, unknown_active: usize, active_total: usize) {
    if active_total == 0 || unknown_active == 0 {
        return;
    }
    if unknown_active < active_total {
        return; // 个别失联可能是任务刚结束，全部失联才判定引擎换代
    }
    if rpc_call_raw("aria2.getVersion", vec![]).await.is_err() {
        return; // 引擎不健康：交给 rpc_call 自愈，下一轮 poll 再触发
    }
    {
        let cell = LAST_REMOUNT.get_or_init(|| std::sync::Mutex::new(None));
        let Ok(mut last) = cell.lock() else {
            return;
        };
        if let Some(t) = *last {
            if t.elapsed() < std::time::Duration::from_secs(10) {
                return;
            }
        }
        *last = Some(std::time::Instant::now());
    }
    engine_log(app, "remount_if_detached: 活跃任务与引擎失联，自动重挂");
    let state = app.state::<AppState>();
    state.log(crate::logger::INFO, "aria2", "engine", "检测到任务与引擎失联，正在恢复下载任务", "");
    resume_pending_tasks(app).await;
}


// ---------- 启动（sidecar） ----------

/// 引擎诊断日志（data_dir/engine.log；启动链路排查）
pub(crate) fn engine_log(app: &AppHandle, msg: &str) {
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

/// 拉起 aria2 sidecar 并等待 RPC 就绪
pub(crate) async fn spawn_sidecar(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let download_dir = resolve_download_dir(app, &settings);
    engine_log(app, &format!("spawn_sidecar: 下载目录 {}", download_dir.display()));

    // BT Tracker 用当前列表（不在此处在线拉取，避免阻塞引擎启动；
    // start() 会在引擎就绪后后台首拉并通过 changeGlobalOption 热更新）
    let bt_trackers = current_bt_trackers();

    let shell = app.shell();
    match shell.sidecar("aria2c") {
        Ok(cmd) => {
            engine_log(app, "spawn_sidecar: sidecar 已解析，准备 spawn");
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
                "--enable-dht=true".to_string(),
                "--enable-dht6=true".to_string(),
                "--enable-peer-exchange=true".to_string(),
                "--bt-enable-lpd=true".to_string(),
                "--bt-max-peers=60".to_string(),
                "--follow-torrent=mem".to_string(),
                "--seed-time=0".to_string(),
                format!("--bt-tracker={}", bt_trackers),
                format!("--stop-with-process={}", std::process::id()),
            ];
            if proxy_configured(&settings) {
                let proxy = build_proxy_arg(&settings);
                args.push(format!("--all-proxy={proxy}"));
                engine_log(app, &format!("spawn_sidecar: 已注入代理 {}", proxy_log_summary(&settings)));
            }
            let cmd = cmd.args(args);
            match cmd.spawn() {
                Ok((mut rx, _child)) => {
                    engine_log(app, "spawn_sidecar: aria2 进程已 spawn");
                    let app_log = app.clone();
                    tauri::async_runtime::spawn(async move {
                        while let Some(evt) = rx.recv().await {
                            if let tauri_plugin_shell::process::CommandEvent::Stderr(line) = evt {
                                engine_log(&app_log, &format!("aria2: {}", String::from_utf8_lossy(&line)));
                            }
                        }
                    });
                }
                Err(e) => engine_log(app, &format!("spawn_sidecar: aria2 spawn 失败 {e:?}")),
            }
        }
        Err(e) => engine_log(app, &format!("spawn_sidecar: sidecar 解析失败 {e:?}")),
    }

    let mut ready = false;
    for _ in 0..30 {
        if rpc_call_raw("aria2.getVersion", vec![]).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    ready
}


/// 启动 aria2 进程并进入轮询循环（setup 时调用一次）
pub async fn start(app: AppHandle) {
    let _ = APP_HANDLE.set(app.clone());
    engine_log(&app, "start: 引擎启动流程开始");

    // 1. 先探测已有端口实例是否正常就绪
    let mut healthy = false;
    if rpc_call_raw("aria2.getVersion", vec![]).await.is_ok() {
        engine_log(&app, "start: 现有 aria2 引擎已就绪且鉴权通过，直接复用");
        healthy = true;
    }

    // 2. 若不可用或端口冲突，定向查杀 16800 上的旧 aria2c 进程并重新拉起
    if !healthy {
        let _ = tokio::task::spawn_blocking(|| kill_stale_aria2_on_port(RPC_PORT)).await;
        let ready = spawn_sidecar(&app).await;
        engine_log(&app, &format!("start: 新引擎启动 就绪={ready}"));
    }

    // BT Tracker 自动更新：引擎复用旧实例时启动参数未带新列表，这里首拉一次热更新
    if app.state::<AppState>().load_settings().bt_tracker_auto_update {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move { refresh_trackers(&app2).await });
    }

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

// ---------- 启动恢复 + 轮询 ----------

/// 引擎当前任务表中的存活 gid（active + waiting；waiting 含 paused）。
/// 复用存活引擎启动时，旧 gid 可能仍有效——先查表防止重复 addUri 造成同文件二次下载。
pub(crate) async fn live_engine_gids() -> std::collections::HashSet<String> {
    let mut gids = std::collections::HashSet::new();
    let mut collect = |v: Value| {
        if let Some(arr) = v.as_array() {
            for item in arr {
                if let Some(g) = item.get("gid").and_then(|g| g.as_str()) {
                    gids.insert(g.to_string());
                }
            }
        }
    };
    if let Ok(v) = rpc_call("aria2.tellActive", vec![]).await {
        collect(v);
    }
    if let Ok(v) = rpc_call("aria2.tellWaiting", vec![json!(0), json!(1000)]).await {
        collect(v);
    }
    gids
}

/// 恢复单个 BT 种子任务：从落盘种子副本重新 addTorrent（url 列是 "torrent:文件名" 伪地址，无法 addUri）
pub(crate) async fn resume_torrent_task(app: &AppHandle, id: i64) {
    let state = app.state::<AppState>();
    let torrent_path = state.data_dir.join("torrents").join(format!("{id}.torrent"));
    let bytes = match tokio::fs::read(&torrent_path).await {
        Ok(b) => b,
        Err(_) => {
            let msg = "种子文件已丢失，请重新添加该 BT 任务";
            eprintln!("[yunx] 恢复任务 {id} 失败: {msg}");
            let _ = update_status(app, id, DownloadTaskView::STATUS_FAILED, msg).await;
            return;
        }
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let Ok(options) = torrent_options(app) else {
        return;
    };
    match rpc_call("aria2.addTorrent", vec![json!(b64), json!([]), options]).await {
        Ok(v) => {
            let gid = v.as_str().unwrap_or("").to_string();
            if !gid.is_empty() {
                let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                let _ = conn.execute(
                    "UPDATE download_task SET gid = ?1 WHERE id = ?2",
                    rusqlite::params![gid, id],
                );
            }
        }
        Err(e) => {
            let _ = update_status(app, id, DownloadTaskView::STATUS_FAILED, &e.to_string()).await;
        }
    }
}

/// 恢复未完成任务：
/// - BT 种子文件任务从落盘副本重加；副本丢失则明确置败（而非喂非法 URL 静默失败）
/// - 引擎仍认识旧 gid（复用存活实例）时保持原绑定，不重复入队
/// - 其余（gid 失效 / 空）清 gid 重新 addUri，paused 状态的以暂停态入队
pub(crate) async fn resume_pending_tasks(app: &AppHandle) {
    let state = app.state::<AppState>();
    let rows: Vec<(i64, String, String, String, String, String, String, String, String, bool)> = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = match conn.prepare(
            "SELECT id, gid, url, file_name, request_headers_json, platform, cleanup_id, mirrors_json, fetch_ctx_json, status \
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
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, i32>(9)? == DownloadTaskView::STATUS_PAUSED,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();
        rows
    };
    let live = live_engine_gids().await;
    for (id, gid, url, file_name, headers_json, platform, cleanup_id, mirrors_json, fetch_ctx, was_paused) in rows {
        // BT 种子文件任务：走 addTorrent 恢复
        if url.starts_with("torrent:") {
            resume_torrent_task(app, id).await;
            continue;
        }
        // 引擎仍在处理该任务（复用存活实例 / 热重启）：保持原 gid 绑定即可
        if !gid.is_empty() && live.contains(&gid) {
            continue;
        }
        let headers: Vec<(String, String)> =
            serde_json::from_str(&headers_json).unwrap_or_default();
        // 夸克任务重挂前重新取链（成功回写 DB），避免旧直链已失效
        let (url, headers) =
            refreshed_target(app, id, &platform, &fetch_ctx, &file_name, url, headers).await;
        let mirrors: Vec<String> = serde_json::from_str(&mirrors_json).unwrap_or_default();
        // 清掉旧 gid（新 aria2 实例不认识）
        {
            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
            let _ = conn.execute("UPDATE download_task SET gid = '' WHERE id = ?1", rusqlite::params![id]);
        }
        if let Err(e) = add_to_aria2(app, id, &url, &file_name, &headers, &platform, &cleanup_id, was_paused, mirrors).await {
            eprintln!("[yunx] 恢复任务 {id} 失败: {e}");
            let _ = update_status(app, id, DownloadTaskView::STATUS_FAILED, &e.to_string()).await;
        }
    }
}

