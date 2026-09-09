//! 订阅追剧自动化（借鉴 quark-auto-save / CloudSaver 的订阅转存思路）。
//! 订阅关键词 → 定时 PanSou 聚合搜索 → 过滤新剧集 → 复用解析 / 取链 / 入队
//! 既有链路自动下载到本地；subscription_item 表记录已下载集数防重复。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use regex::Regex;
use rusqlite::params;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::error::{AppError, AppResult};
use crate::models::{CollectedFile, SearchItem, SubscriptionRow, SubscriptionUpsert};
use crate::state::AppState;

/// 云析支持解析下载的平台（与前端 SUPPORTED_TYPES 对齐）
const SUPPORTED: &[&str] = &["quark", "uc", "xunlei", "baidu", "c139", "pan123"];
/// 单次执行最多成功解析的分享数（换源尝试的上限）
const MAX_CANDIDATES: usize = 3;
/// 单次执行最多发起解析尝试的分享数（含失效分享，防止死循环大量请求）
const MAX_RESOLVE_ATTEMPTS: usize = 6;
/// 单次执行最多新增下载文件数（防风暴）
const MAX_NEW_PER_RUN: usize = 20;
/// 执行互斥（定时器与手动「立即执行」撞车保护）
static RUNNING: AtomicBool = AtomicBool::new(false);

struct RunningGuard;
impl Drop for RunningGuard {
    fn drop(&mut self) { RUNNING.store(false, Ordering::SeqCst); }
}

struct SessionGuard<'a> {
    state: &'a AppState,
    key: String,
}
impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut sessions) = self.state.sessions.lock() {
            sessions.remove(&self.key);
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn is_due(last_run_at: i64, interval_minutes: i64, now: i64) -> bool {
    last_run_at <= 0 || now.saturating_sub(last_run_at) >= interval_minutes.max(15) * 60_000
}

// ---------- 剧集集数提取 ----------

fn episode_regexes() -> &'static Vec<Regex> {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        vec![
            // EP03 / ep 3
            Regex::new(r"(?i)\bep\s*(\d{1,3})\b").unwrap(),
            // 第12集 / 第3话 / 第5期 / 第7回
            Regex::new(r"第\s*(\d{1,4})\s*[集话話期回]").unwrap(),
            // 【04】/(12)（括号包裹的 1-3 位纯数字；1080p 等带后缀不匹配）
            Regex::new(r"[\[【(](\d{1,3})[\]】)]").unwrap(),
            // 结尾数字：Show - 05.mkv / Show_06.mp4
            Regex::new(r"(?i)[\-_ ](\d{2,3})\s*\.(?:mkv|mp4|avi|ts|flv|rmvb|wmv|mov|iso|m2ts)$").unwrap(),
        ]
    })
}

/// 从文件名提取集数标识（"ep12" 形式，数字归一化去前导零）；
/// 无法识别集数的返回 None（调用方回退用文件名做去重键）。
pub fn episode_key(name: &str) -> Option<String> {
    static SEASON_EPISODE: OnceLock<Regex> = OnceLock::new();
    let season_episode = SEASON_EPISODE.get_or_init(|| Regex::new(r"(?i)s\s*(\d{1,2})\s*e\s*(\d{1,3})").unwrap());
    if let Some(caps) = season_episode.captures(name) {
        let season = caps.get(1)?.as_str().parse::<u32>().ok()?;
        let episode = caps.get(2)?.as_str().parse::<u32>().ok()?;
        return Some(format!("s{season}e{episode}"));
    }
    for re in episode_regexes() {
        if let Some(caps) = re.captures(name) {
            if let Some(m) = caps.get(1) {
                if let Ok(n) = m.as_str().parse::<u32>() {
                    return Some(format!("ep{n}"));
                }
            }
        }
    }
    None
}

// ---------- DB 读写 ----------

const COLS: &str = "id, keyword, cloud_types_json, episode_regex, enabled, last_run_at, last_result, create_time";

fn row_to_sub(r: &rusqlite::Row) -> rusqlite::Result<SubscriptionRow> {
    Ok(SubscriptionRow {
        id: r.get(0)?,
        keyword: r.get(1)?,
        cloud_types_json: r.get(2)?,
        episode_regex: r.get(3)?,
        enabled: r.get::<_, i32>(4)? != 0,
        last_run_at: r.get(5)?,
        last_result: r.get(6)?,
        create_time: r.get(7)?,
    })
}

fn load_all(conn: &rusqlite::Connection) -> Vec<SubscriptionRow> {
    let mut stmt = match conn.prepare(&format!("SELECT {COLS} FROM subscription ORDER BY create_time DESC")) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], row_to_sub)
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
}

pub fn list(state: &AppState) -> AppResult<Vec<SubscriptionRow>> {
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    Ok(load_all(&conn))
}

pub fn add(state: &AppState, upsert: &SubscriptionUpsert) -> AppResult<i64> {
    let keyword = upsert.keyword.trim();
    if keyword.is_empty() {
        return Err(AppError::Api("订阅关键词不能为空".into()));
    }
    let types_json = serde_json::to_string(&upsert.cloud_types).unwrap_or_else(|_| "[]".into());
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "INSERT INTO subscription (keyword, cloud_types_json, episode_regex, enabled, create_time) \
         VALUES (?1, ?2, ?3, 1, ?4)",
        params![keyword, types_json, upsert.episode_regex.trim(), now_ms()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(state: &AppState, id: i64, enabled: bool, upsert: &SubscriptionUpsert) -> AppResult<()> {
    let keyword = upsert.keyword.trim();
    if keyword.is_empty() {
        return Err(AppError::Api("订阅关键词不能为空".into()));
    }
    let types_json = serde_json::to_string(&upsert.cloud_types).unwrap_or_else(|_| "[]".into());
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let n = conn.execute(
        "UPDATE subscription SET keyword = ?1, cloud_types_json = ?2, episode_regex = ?3, enabled = ?4 WHERE id = ?5",
        params![keyword, types_json, upsert.episode_regex.trim(), enabled as i32, id],
    )?;
    if n == 0 {
        return Err(AppError::Api("订阅不存在".into()));
    }
    Ok(())
}

pub fn remove(state: &AppState, id: i64) -> AppResult<()> {
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM subscription WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM subscription_item WHERE subscription_id = ?1", params![id])?;
    Ok(())
}

fn load_one(conn: &rusqlite::Connection, id: i64) -> AppResult<SubscriptionRow> {
    conn.query_row(
        &format!("SELECT {COLS} FROM subscription WHERE id = ?1"),
        params![id],
        row_to_sub,
    )
    .map_err(|_| AppError::Api("订阅不存在".into()))
}

fn mark_run(conn: &rusqlite::Connection, id: i64, result: &str) {
    let _ = conn.execute(
        "UPDATE subscription SET last_run_at = ?1, last_result = ?2 WHERE id = ?3",
        params![now_ms(), result, id],
    );
}

/// 是否已下载过（按集数键或文件名任一命中即视为重复）
fn has_item(conn: &rusqlite::Connection, sub_id: i64, key: &str, file_name: &str) -> bool {
    // 旧版本把第一季 S01E02 记录成 ep2；仅对第一季回查旧键，第二季起不能误命中。
    let legacy_key = key
        .strip_prefix("s1e")
        .map(|episode| format!("ep{episode}"))
        .unwrap_or_default();
    conn.query_row(
        "SELECT COUNT(*) FROM subscription_item \
         WHERE subscription_id = ?1 AND (episode_key = ?2 OR file_name = ?3 OR (?4 != '' AND episode_key = ?4))",
        params![sub_id, key, file_name, legacy_key],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

fn record_item(conn: &rusqlite::Connection, sub_id: i64, key: &str, file_name: &str, task_id: i64) {
    let _ = conn.execute(
        "INSERT OR IGNORE INTO subscription_item \
         (subscription_id, episode_key, file_name, download_task_id, create_time) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sub_id, key, file_name, task_id, now_ms()],
    );
}

// ---------- 执行管线 ----------

/// 候选排序：用户偏好的网盘类型在前，其余支持的平台殿后
fn order_candidates(items: Vec<SearchItem>, preferred: &[String]) -> Vec<SearchItem> {
    let mut ordered: Vec<SearchItem> = Vec::new();
    for t in preferred {
        ordered.extend(items.iter().filter(|i| &i.r#type == t).cloned());
    }
    for it in items {
        if SUPPORTED.contains(&it.r#type.as_str()) && !ordered.iter().any(|o| o.url == it.url) {
            ordered.push(it);
        }
    }
    ordered
}

/// 执行一次订阅检查（搜索 → 换源解析 → 过滤新集 → 自动入队下载）。
/// 返回结果摘要（同时写入 last_result 与日志、发系统通知）。
async fn run_subscription_inner(app: &AppHandle, sub: &SubscriptionRow) -> String {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let base = settings.pansou_base_url.trim().to_string();
    if base.is_empty() {
        return "未配置 PanSou 搜索服务，无法执行订阅".into();
    }

    let preferred: Vec<String> = serde_json::from_str(&sub.cloud_types_json).unwrap_or_default();
    let items = match crate::api::pansou::search(&base, &sub.keyword, None).await {
        Ok(v) => v,
        Err(e) => {
            state.log(
                crate::logger::ERROR,
                "subscription",
                "search",
                &format!("订阅搜索失败：{}", sub.keyword),
                &e.to_string(),
            );
            return format!("搜索失败：{e}");
        }
    };
    if items.is_empty() {
        state.log(
            crate::logger::INFO,
            "subscription",
            "search",
            &format!("订阅搜索无结果：{}", sub.keyword),
            "",
        );
        return "搜索无结果".into();
    }

    let user_filter = if sub.episode_regex.trim().is_empty() {
        None
    } else {
        match Regex::new(sub.episode_regex.trim()) {
            Ok(re) => Some(re),
            Err(e) => {
                state.log(
                    crate::logger::ERROR,
                    "subscription",
                    "filter",
                    "自定义集数过滤正则无效",
                    &e.to_string(),
                );
                return format!("过滤正则无效：{e}");
            }
        }
    };

    let candidates = order_candidates(items, &preferred);
    let mut new_count = 0usize;
    let mut resolved = 0usize;
    let mut attempts = 0usize;
    let mut last_err = String::new();
    let mut enqueued: Vec<String> = Vec::new();

    'outer: for item in candidates {
        if resolved >= MAX_CANDIDATES || attempts >= MAX_RESOLVE_ATTEMPTS || new_count >= MAX_NEW_PER_RUN {
            break;
        }
        attempts += 1;
        let text = if item.password.is_empty() {
            item.url.clone()
        } else {
            format!("{} 提取码：{}", item.url, item.password)
        };
        let info = match crate::resolve::resolve_share(&state, &text, Some(&item.password)).await {
            Ok(v) => v,
            Err(e) => {
                last_err = e.to_string();
                state.log(
                    crate::logger::INFO,
                    "subscription",
                    "resolve",
                    &format!("候选分享解析失败，尝试下一个：{}", item.note),
                    &e.to_string(),
                );
                continue;
            }
        };
        resolved += 1;
        let _session_guard = SessionGuard { state: &state, key: info.session_key.clone() };

        // 根级含目录 → 递归收集（还原 rel_dir 结构）；纯文件分享直接用首页列表
        let files: Vec<CollectedFile> = if info.files.iter().any(|f| f.isdir) {
            match crate::resolve::collect_folder_files(&state, &info.session_key, "0").await {
                Ok(collected) => collected,
                Err(e) => {
                    last_err = e.to_string();
                    state.log(
                        crate::logger::ERROR,
                        "subscription",
                        "collect",
                        &format!("目录收集失败：{}", info.title),
                        &e.to_string(),
                    );
                    continue;
                }
            }
        } else {
            info.files.clone().into_iter().map(|file| CollectedFile { file, rel_dir: String::new() }).collect()
        };

        for collected in &files {
            let f = &collected.file;
            if f.isdir || new_count >= MAX_NEW_PER_RUN {
                continue;
            }
            if let Some(re) = &user_filter {
                if !re.is_match(&f.fname) {
                    continue;
                }
            }
            let key = episode_key(&f.fname).unwrap_or_else(|| f.fname.clone());
            let exists = {
                let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                has_item(&conn, sub.id, &key, &f.fname)
            };
            if exists {
                continue;
            }
            match crate::resolve::get_download_link(&state, &info.session_key, f).await {
                Ok(link) => {
                    let out_name = if f.fname == link.filename || link.filename.is_empty() {
                        f.fname.clone()
                    } else {
                        link.filename
                    };
                    let out_name = if collected.rel_dir.is_empty() {
                        out_name
                    } else {
                        format!("{}/{}", collected.rel_dir, out_name)
                    };
                    match crate::aria2::enqueue(
                        app,
                        &link.url,
                        &out_name,
                        &link.headers,
                        &link.platform,
                        &link.cleanup_id,
                        false,
                        link.mirrors,
                    )
                    .await
                    {
                        Ok(task_id) => {
                            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                            record_item(&conn, sub.id, &key, &f.fname, task_id);
                            drop(conn);
                            new_count += 1;
                            enqueued.push(f.fname.clone());
                        }
                        Err(e) => {
                            last_err = e.to_string();
                        }
                    }
                }
                Err(e) => {
                    last_err = e.to_string();
                    state.log(
                        crate::logger::ERROR,
                        "subscription",
                        "link",
                        &format!("取链失败：{}", f.fname),
                        &e.to_string(),
                    );
                }
            }
        }

        if new_count >= MAX_NEW_PER_RUN {
            break 'outer;
        }
    }

    let summary = if new_count > 0 {
        format!("新增 {new_count} 个文件：{}", enqueued.join("、"))
    } else if resolved == 0 {
        format!("所有候选分享均解析失败：{last_err}")
    } else {
        "检查完成，无新集".to_string()
    };
    if new_count > 0 && settings.download_notify {
        let _ = app
            .notification()
            .builder()
            .title("订阅更新")
            .body(format!("「{}」新增 {} 个文件开始下载", sub.keyword, new_count))
            .show();
    }
    summary
}

/// 任何结果都统一更新时间与摘要，确保失败/无结果同样受调度间隔约束。
pub async fn run_subscription(app: &AppHandle, sub: &SubscriptionRow) -> String {
    let summary = run_subscription_inner(app, sub).await;
    let state = app.state::<AppState>();
    if let Ok(conn) = state.db.lock() {
        mark_run(&conn, sub.id, &summary);
    }
    state.log(
        crate::logger::INFO,
        "subscription",
        "run",
        &format!("订阅「{}」{}", sub.keyword, summary),
        "",
    );
    summary
}

/// 手动立即执行（命令入口；与定时器互斥）
pub async fn run_now(app: AppHandle, id: i64) -> AppResult<String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Api("已有订阅检查在执行中，请稍后再试".into()));
    }
    let _running_guard = RunningGuard;
    let state = app.state::<AppState>();
    let sub = {
        let conn = state.db.lock().map_err(|_| AppError::Lock)?;
        load_one(&conn, id)
    }?;
    let summary = run_subscription(&app, &sub).await;
    Ok(summary)
}

// ---------- 定时调度 ----------

/// 后台调度循环（setup 时 spawn）：每 15 分钟醒来一次，把到期的订阅逐个执行。
/// 到期条件：last_run_at 距今超过全局检查间隔（从未运行的订阅首轮即执行）。
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move { scheduler(app).await });
}

async fn scheduler(app: AppHandle) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
        if RUNNING.swap(true, Ordering::SeqCst) {
            continue;
        }
        let _running_guard = RunningGuard;
        let state = app.state::<AppState>();
        let settings = state.load_settings();
        if !settings.subscription_enabled || settings.pansou_base_url.trim().is_empty() {
            continue;
        }
        let interval = settings.subscription_interval_minutes.max(15) as i64;
        let due: Vec<SubscriptionRow> = {
            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
            load_all(&conn)
                .into_iter()
                .filter(|s| s.enabled && is_due(s.last_run_at, interval, now_ms()))
                .collect()
        };
        for sub in due {
            run_subscription(&app, &sub).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{episode_key, has_item, is_due, mark_run};

    #[test]
    fn episode_keys_include_season() {
        assert_eq!(episode_key("Show.S01E02.mkv").as_deref(), Some("s1e2"));
        assert_eq!(episode_key("Show.S02E02.mkv").as_deref(), Some("s2e2"));
        assert_eq!(episode_key("Show EP03.mp4").as_deref(), Some("ep3"));
        assert_eq!(episode_key("动画 第12集.mp4").as_deref(), Some("ep12"));
    }

    #[test]
    fn old_episode_key_only_blocks_first_season() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE subscription_item (subscription_id INTEGER, episode_key TEXT, file_name TEXT);").unwrap();
        conn.execute("INSERT INTO subscription_item VALUES (1, 'ep2', 'legacy-file.mkv')", []).unwrap();
        assert!(has_item(&conn, 1, "s1e2", "new-s1.mkv"));
        assert!(!has_item(&conn, 1, "s2e2", "new-s2.mkv"));
    }

    #[test]
    fn due_check_respects_interval() {
        assert!(is_due(0, 60, 1));
        assert!(!is_due(1_000, 60, 1_000 + 3_599_999));
        assert!(is_due(1_000, 60, 1_000 + 3_600_000));
    }

    #[test]
    fn every_summary_updates_last_run() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE subscription (id INTEGER PRIMARY KEY, last_run_at INTEGER, last_result TEXT); INSERT INTO subscription VALUES (1, 0, '');").unwrap();
        for summary in ["搜索无结果", "搜索失败：offline", "过滤正则无效"] {
            mark_run(&conn, 1, summary);
            let row: (i64, String) = conn.query_row("SELECT last_run_at, last_result FROM subscription WHERE id = 1", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
            assert!(row.0 > 0);
            assert_eq!(row.1, summary);
        }
    }
}
