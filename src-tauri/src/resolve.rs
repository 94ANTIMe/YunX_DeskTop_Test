//! 解析编排（对应 Android *ResolveRepository）：
//! createSession → listFiles → 取链（必要时转存临时目录）→ 清理。
//! 夸克/UC：token + 分享列表 + 取链（夸克经转存，延迟清理；UC 直取）。
//! 百度：verify → xpan list → transfer → locatedownload → 即时清理。
//! 139：匿名列目录 + 登录态取链（AES 加密分享接口）。
//! 123：匿名列目录 + 登录态签名取链（download-v2 解码 + redirect 跟随）。
//! 迅雷：getShare → restore 临时目录 → 文件详情取链 → 即时清理。
use uuid::Uuid;

use crate::api::{baidaccel, baidu, c139, pan123, quark, uc, xunlei};
use crate::db::accounts::{self, Account};
use crate::error::{AppError, AppResult};
use crate::logger;
use crate::models::{
    CollectedFile, DownloadLink, Platform, ResolveSessionInfo, ShareFile, ShareFilePage,
};
use crate::state::AppState;

/// 解析会话（内存态；键 = UUID）
#[derive(Debug, Clone)]
pub struct ResolveSession {
    pub platform: Platform,
    pub share_id: String,
    pub pwd: String,
    pub title: String,
    // 夸克 / UC
    pub stoken: String,
    // 百度
    pub sekey: String,
    pub baidu_share_id: String,
    pub baidu_uk: String,
    // 迅雷
    pub pass_code_token: String,
    // 139
    pub account: String,
    pub authorization: String,
    // 分页游标（123 / 迅雷）
    pub last_dir: String,
    pub next_cursor: String,
    pub next_page_token: String,
    // 百度高速通道（百度分享加速路由）
    pub accel: bool,
    pub accel_randsk: String,
    pub accel_uk: String,
    pub accel_shareid: String,
}

impl ResolveSession {
    fn new(platform: Platform, share_id: String, pwd: String) -> Self {
        Self {
            platform,
            share_id,
            pwd,
            title: String::new(),
            stoken: String::new(),
            sekey: String::new(),
            baidu_share_id: String::new(),
            baidu_uk: String::new(),
            pass_code_token: String::new(),
            account: String::new(),
            authorization: String::new(),
            last_dir: String::new(),
            next_cursor: String::new(),
            next_page_token: String::new(),
            accel: false,
            accel_randsk: String::new(),
            accel_uk: String::new(),
            accel_shareid: String::new(),
        }
    }
}

/// 会话表容量上限（超出淘汰最旧）
const MAX_SESSIONS: usize = 32;

pub(crate) fn load_account_cookie(state: &AppState, platform: Platform, need_login_msg: &str) -> AppResult<String> {
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let active = state.active_account_key(&platform);
    match accounts::load(&conn, platform, &active)? {
        Some(acc) if !acc.cookie().is_empty() => Ok(acc.cookie().to_string()),
        _ => Err(AppError::Api(need_login_msg.to_string())),
    }
}

fn insert_session(state: &AppState, session: ResolveSession) -> String {
    let key = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    // 容量控制：超过上限移除最早插入的会话
    if sessions.len() >= MAX_SESSIONS {
        if let Some(oldest) = sessions.keys().next().cloned() {
            sessions.remove(&oldest);
        }
    }
    sessions.insert(key.clone(), session);
    key
}

fn get_session(state: &AppState, key: &str) -> AppResult<ResolveSession> {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sessions
        .get(key)
        .cloned()
        .ok_or_else(|| AppError::Api("解析会话已过期，请重新解析".into()))
}

fn update_session(state: &AppState, key: &str, session: ResolveSession) {
    let mut sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sessions.insert(key.to_string(), session);
}

/// 持久化更新平台 Cookie（__puus 刷新后）
fn persist_cookie(state: &AppState, platform: Platform, cookie: &str, nickname: &str) {
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    // 写入当前选中账号行（不要新建行，否则每次 __puus 刷新都产生一条新账号）
    let key = state.active_account_key(&platform);
    let _ = accounts::save_with_key(
        &conn,
        &Account::Cookie { platform, cookie: cookie.to_string(), nickname: nickname.to_string() },
        now,
        &key,
    );
}

/// 夸克转存取链路线（他人分享）：
/// 临时目录 → 唯一子目录 tr_*（去重键每次不同，根治二次转存 404 code:21001）
/// → 转存 → 轮询 → 取链；返回 (url, size, 子目录 fid)（下载完成后删整个子目录）。
/// 自己的分享会因服务端拒绝转存抛错，由调用方走直取路线。
async fn quark_transfer_route(
    state: &AppState,
    session: &ResolveSession,
    cookie: &str,
    file: &ShareFile,
) -> AppResult<(String, i64, String)> {
    let base_dir = quark::ensure_temp_dir(&state.http, cookie).await?;
    let sub_dir = quark::create_transfer_subdir(&state.http, &base_dir, cookie).await?;
    let task_id = quark::save_share_file(
        &state.http, &session.share_id, &session.stoken, &file.pdir_fid, &file.fid, &file.fid_token, &sub_dir, cookie,
    )
    .await?;
    let new_fid = quark::poll_task(&state.http, &task_id, cookie).await?;
    let (url, _, size) = quark::get_download_link(&state.http, &new_fid, cookie).await?;
    Ok((url, size, sub_dir))
}

// ---------- 百度高速通道（百度分享加速路由） ----------

/// 是否命中百度验证码风控（errno 105 等），用于给出明确提示并避免反复硬撞
fn is_captcha_blocked(e: &AppError) -> bool {
    match e {
        AppError::Api(m) => {
            let m = m.to_lowercase();
            m.contains("105") || m.contains("验证码") || m.contains("captcha") || m.contains("needverify")
        }
        _ => false,
    }
}

/// 验证码风控错误 → 明确指引；其余原样返回（供解析码刷新等链路使用）
#[allow(dead_code)]
pub(crate) fn captcha_hint(e: AppError, hint: &str) -> AppError {
    if is_captcha_blocked(&e) {
        AppError::Api(hint.to_string())
    } else {
        e
    }
}

/// 百度官方接口错误 → 用户可读的明确提示（避免笼统的 errno 直出）
/// - errno=2 参数错误：多为分享已失效/链接无效
/// - 登录态相关：提示重新登录
/// - 其余保留原样
fn baidu_err_hint(e: AppError) -> AppError {
    match &e {
        AppError::Api(m) => {
            let low = m.to_lowercase();
            if low.contains("errno=2") || low.contains("参数错误") {
                AppError::Api("分享可能已失效或链接无效，请确认链接后重试".into())
            } else if low.contains("errno=4") || low.contains("errno=10") {
                AppError::Api("百度网盘登录状态异常，请重新登录后重试".into())
            } else if is_captcha_blocked(&e) {
                AppError::Api("百度网盘当前触发验证码风控（需要人机验证），请稍后几分钟再试".into())
            } else {
                e
            }
        }
        _ => e,
    }
}

/// 百度加速列目录：若密码不可用或接口超时，秒级快速失败并回退官方链路
async fn accel_list(state: &AppState, surl: &str, pwd: &str, dir: &str) -> AppResult<baidaccel::AccelListData> {
    let settings = state.load_settings();
    let base = baidaccel::base_url_of(&settings);
    let password = settings.baidu_speed_password.trim().to_string();
    if password.is_empty() {
        return Err(AppError::Api("加速通道解析码未配置，回退官方链路".into()));
    }
    match baidaccel::get_file_list(&state.http, &base, surl, pwd, dir, &password).await {
        Ok(v) => Ok(v),
        Err(e) => {
            state.log(logger::INFO, "baidu", "accel", "加速通道不可用，快速回退官方链路", &e.to_string());
            Err(e)
        }
    }
}

/// 百度加速取直链（单次调用）
async fn accel_fetch_link(
    state: &AppState,
    base: &str,
    session: &ResolveSession,
    file: &ShareFile,
    password: &str,
) -> AppResult<(String, String)> {
    baidaccel::get_download_links(
        &state.http, base, &session.share_id, &session.pwd, &file.pdir_fid, &file.fid,
        &session.accel_randsk, &session.accel_uk, &session.accel_shareid, password,
    )
    .await
}

/// 百度官方链路建会话（PCS 式验证 → 首页列表；加速通道不可用时的回退路线）
async fn baidu_official_session(
    state: &AppState,
    parsed: &crate::models::ParsedShare,
    session: &mut ResolveSession,
) -> AppResult<(Vec<ShareFile>, String)> {
    let cookie = load_account_cookie(state, Platform::Baidu, "请先登录百度网盘")?;
    let share = baidu::verify_share_pcs(&state.http, &parsed.share_id, &parsed.pwd, &cookie).await.map_err(|e| {
        state.log(logger::ERROR, "baidu", "verify", "回退官方链路：验证分享失败", &e.to_string());
        baidu_err_hint(e)
    })?;
    session.sekey = share.randsk.clone();
    session.baidu_share_id = share.share_id.clone();
    session.baidu_uk = share.uk.clone();
    let list = baidu::list_share(&state.http, &parsed.share_id, &session.sekey, "/", &cookie, 1)
        .await
        .map_err(|e| {
            state.log(logger::ERROR, "baidu", "list", "回退官方链路：列出分享失败", &e.to_string());
            baidu_err_hint(e)
        })?;
    Ok((list.files, String::new()))
}

/// 百度官方链路取链（转存 + locate），加速通道取链/直链不可用时回退
async fn baidu_official_link(state: &AppState, session_key: &str, file: &ShareFile) -> AppResult<DownloadLink> {
    let mut session = get_session(state, session_key)?;
    // 若会话尚未持有官方 sekey，现场补建
    if session.sekey.is_empty() {
        let parsed = crate::models::ParsedShare {
            platform: "baidu".into(),
            share_id: session.share_id.clone(),
            pwd: session.pwd.clone(),
        };
        baidu_official_session(state, &parsed, &mut session).await?;
    }
    let cookie = load_account_cookie(state, Platform::Baidu, "请先登录百度网盘")?;
    let temp_dir = baidu::ensure_temp_dir(&state.http, &cookie).await?;
    let (_, new_path) = baidu::transfer(
        &state.http,
        &session.share_id,
        &session.baidu_share_id,
        &session.baidu_uk,
        &session.sekey,
        &file.fid,
        &temp_dir,
        &cookie,
    )
    .await
    .map_err(|e| {
        state.log(logger::ERROR, "baidu", "transfer", "回退官方链路：转存失败", &e.to_string());
        baidu_err_hint(e)
    })?;
    let urls = crate::baidupcs::locate_urls(&state.http, &cookie, &new_path, &state.data_dir)
        .await
        .map_err(|e| {
            state.log(logger::ERROR, "baidu", "link", "回退官方链路：取链失败", &format!("path={new_path} {e}"));
            e
        })?;
    let main_url = urls.first().cloned().unwrap_or_default();
    let mirrors = if urls.len() > 1 { urls[1..].to_vec() } else { Vec::new() };
    Ok(DownloadLink {
        url: main_url,
        filename: file.fname.clone(),
        size: file.fsize,
        headers: vec![("User-Agent".into(), crate::baidupcs::UA.into())],
        platform: "baidu".into(),
        cleanup_id: new_path,
        mirrors,
    })
}

/// 直链可达性探测：发一次 Range 小请求，HTTP 2xx/206 视为可用
/// 返回 Ok(true) 可用；Err(msg) 记录不可达原因
async fn link_reachable(client: &reqwest::Client, url: &str, ua: &str) -> Result<bool, String> {
    let resp = match client
        .get(url)
        .header("User-Agent", ua)
        .header("Range", "bytes=0-0")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Err(format!("请求失败: {e}")),
    };
    let status = resp.status();
    if status.is_success() {
        Ok(true)
    } else {
        Err(format!("HTTP {}", status.as_u16()))
    }
}

// ---------- 解析入口（建会话 + 首页列表） ----------

pub async fn resolve_share(state: &AppState, text: &str) -> AppResult<ResolveSessionInfo> {
    let parsed = match crate::parser::parse(text) {
        Ok(p) => p,
        Err(e) => {
            state.log(crate::logger::ERROR, "", "resolve", "解析失败：未识别到分享链接", &text.chars().take(200).collect::<String>());
            return Err(e);
        }
    };
    let platform = Platform::from_key(&parsed.platform)
        .ok_or_else(|| AppError::Api("未知平台".into()))?;
    state.log(
        crate::logger::INFO,
        platform.key(),
        "resolve",
        "开始解析分享链接",
        &format!("share_id={} pwd={}", parsed.share_id, if parsed.pwd.is_empty() { "-" } else { &parsed.pwd }),
    );
    let mut session = ResolveSession::new(platform, parsed.share_id.clone(), parsed.pwd.clone());
    let (files, title) = match build_session(state, platform, &parsed, &mut session).await {
        Ok(v) => v,
        Err(e) => {
            state.log(crate::logger::ERROR, platform.key(), "resolve", &format!("解析失败：{e}"), "");
            return Err(e);
        }
    };
    session.title = title.clone();
    let has_more = files.len() >= page_size(platform);
    let key = insert_session(state, session);
    state.log(
        crate::logger::SUCCESS,
        platform.key(),
        "resolve",
        &format!("解析成功：{}", if title.is_empty() { "（无标题）" } else { &title }),
        &format!("首页 {} 个条目（会话 {}）", files.len(), &key[..8]),
    );
    Ok(ResolveSessionInfo {
        session_key: key,
        platform: parsed.platform,
        title,
        files,
        has_more,
    })
}

/// 各平台建会话取首页（原 resolve_share 主体）
async fn build_session(
    state: &AppState,
    platform: Platform,
    parsed: &crate::models::ParsedShare,
    session: &mut ResolveSession,
) -> AppResult<(Vec<ShareFile>, String)> {
    let (files, title) = match platform {
        Platform::Quark => {
            let cookie = load_account_cookie(state, platform, "请先登录夸克网盘")?;
            let (stoken, title) = quark::get_share_token(&state.http, &parsed.share_id, &parsed.pwd, &cookie).await?;
            session.stoken = stoken;
            let (files, _) = quark::get_share_files(&state.http, &parsed.share_id, &session.stoken, "0", &cookie, 1, 100).await?;
            (files, title)
        }
        Platform::Uc => {
            let cookie = load_account_cookie(state, platform, "请先登录 UC 网盘")?;
            let (stoken, title) = uc::get_share_token(&state.http, &parsed.share_id, &parsed.pwd, &cookie).await?;
            session.stoken = stoken;
            let files = uc::get_transfer_share_files(&state.http, &parsed.share_id, &session.stoken, "0", &cookie, 1, 100).await?;
            (files, title)
        }
        Platform::Baidu => {
            // 自动静默优先探测高速通道，不可用时毫秒级无感回退官方多镜像并发链路
            match accel_list(state, &parsed.share_id, &parsed.pwd, "/").await {
                Ok(data) => {
                    session.accel = true;
                    session.accel_randsk = data.randsk.clone();
                    session.accel_uk = data.uk.clone();
                    session.accel_shareid = data.shareid.clone();
                    state.log(logger::INFO, "baidu", "accel", "百度高速通道已启用", "");
                    return Ok((data.files, data.uname));
                }
                Err(e) => {
                    state.log(
                        logger::INFO,
                        "baidu",
                        "accel",
                        "加速通道未就绪，自动切入官方多源并发链路",
                        &e.to_string(),
                    );
                }
            }
            baidu_official_session(state, parsed, session).await?
        }
        Platform::C139 => {
            // 139 官方会明文回吐提取码，自动填充
            if session.pwd.is_empty() {
                if let Ok(pwd) = c139::get_out_link_password(&state.http, &parsed.share_id).await {
                    session.pwd = pwd;
                }
            }
            let title = c139::get_out_link_title(&state.http, &parsed.share_id).await?;
            let files = c139::get_share_files(&state.http, &parsed.share_id, "root", &session.pwd, 1, 200).await?;
            // 登录态预取（下载需要）
            if let Ok(cookie) = load_account_cookie(state, platform, "") {
                if let Some(auth) = c139::extract_authorization(&cookie) {
                    session.authorization = auth;
                    session.account = c139::extract_account_full(&cookie).unwrap_or_default();
                }
            }
            (files, title)
        }
        Platform::Pan123 => {
            let (files, next) = pan123::get_share_files(&state.http, &parsed.share_id, &session.pwd, "0", "", 1).await?;
            session.next_cursor = next.clone().unwrap_or_default();
            session.last_dir = "0".to_string();
            (files, String::new())
        }
        Platform::Xunlei => {
            state.load_xunlei_runtime()?;
            // 克隆运行时（MutexGuard 不跨 await），完成后写回
            let mut rt = {
                let guard = state.xunlei.lock().map_err(|_| AppError::Lock)?;
                if guard.access_token.is_empty() {
                    return Err(AppError::Api("请先登录迅雷网盘".into()));
                }
                guard.clone()
            };
            let (title, files, pass_code_token, next) =
                xunlei::get_share(&state.http, &mut rt, &parsed.share_id, &parsed.pwd, "").await?;
            *state.xunlei.lock().map_err(|_| AppError::Lock)? = rt;
            state.persist_xunlei_runtime("")?;
            session.pass_code_token = pass_code_token;
            session.next_page_token = next;
            (files, title)
        }
        Platform::Direct => {
            let url = parsed.share_id.clone();
            let mut fname = url
                .split('?')
                .next()
                .unwrap_or(&url)
                .split('/')
                .last()
                .unwrap_or("download.bin")
                .to_string();
            if let Ok(decoded) = urlencoding::decode(&fname) {
                fname = decoded.into_owned();
            }
            if fname.is_empty() {
                fname = "download.bin".to_string();
            }
            let mut fsize = 0i64;
            if let Ok(resp) = state.http.head(&url).timeout(std::time::Duration::from_secs(3)).send().await {
                if let Some(len) = resp.headers().get(reqwest::header::CONTENT_LENGTH) {
                    if let Ok(s) = len.to_str() {
                        fsize = s.parse::<i64>().unwrap_or(0);
                    }
                }
            }
            let file = ShareFile {
                fid: "direct".into(),
                fname: fname.clone(),
                fsize,
                isdir: false,
                pdir_fid: "".into(),
                fid_token: url.clone(),
                modify_time: "".into(),
            };
            (vec![file], fname)
        }
        Platform::Magnet => {
            let magnet = parsed.share_id.clone();
            let fname = if let Some(dn_idx) = magnet.find("dn=") {
                let after = &magnet[dn_idx + 3..];
                let end = after.find('&').unwrap_or(after.len());
                urlencoding::decode(&after[..end])
                    .map(|s| s.into_owned())
                    .unwrap_or_else(|_| "Magnet_BT_Task".to_string())
            } else {
                "Magnet_BT_Task".to_string()
            };
            let file = ShareFile {
                fid: "magnet".into(),
                fname: fname.clone(),
                fsize: 0,
                isdir: false,
                pdir_fid: "".into(),
                fid_token: magnet.clone(),
                modify_time: "".into(),
            };
            (vec![file], fname)
        }
    };
    session.title = title.clone();
    Ok((files, title))
}

fn page_size(platform: Platform) -> usize {
    match platform {
        Platform::C139 => 200,
        _ => 100,
    }
}

// ---------- 文件列表（目录导航 / 翻页） ----------

pub async fn list_share_files(
    state: &AppState,
    session_key: &str,
    dir_id: &str,
    page: i64,
) -> AppResult<ShareFilePage> {
    let mut session = get_session(state, session_key)?;
    let platform = session.platform;
    let dir_changed = session.last_dir != dir_id;
    let (files, has_more) = match platform {
        Platform::Quark => {
            let cookie = load_account_cookie(state, platform, "请先登录夸克网盘")?;
            let (files, _) = quark::get_share_files(&state.http, &session.share_id, &session.stoken, dir_id, &cookie, page, 100).await?;
            let has_more = files.len() >= 100;
            (files, has_more)
        }
        Platform::Uc => {
            let cookie = load_account_cookie(state, platform, "请先登录 UC 网盘")?;
            let files = uc::get_transfer_share_files(&state.http, &session.share_id, &session.stoken, dir_id, &cookie, page, 100).await?;
            let has_more = files.len() >= 100;
            (files, has_more)
        }
        Platform::Baidu if session.accel => {
            // 加速路由：dir_id 即分享内绝对路径；刷新后回写 randsk/uk/shareid
            let data = accel_list(state, &session.share_id, &session.pwd, dir_id).await?;
            session.accel_randsk = data.randsk.clone();
            session.accel_uk = data.uk.clone();
            session.accel_shareid = data.shareid.clone();
            (data.files, false)
        }
        Platform::Baidu => {
            let cookie = load_account_cookie(state, platform, "请先登录百度网盘")?;
            let list = baidu::list_share(&state.http, &session.share_id, &session.sekey, dir_id, &cookie, page).await?;
            (list.files, list.has_more)
        }
        Platform::C139 => {
            let begin = (page - 1).max(0) * 200 + 1;
            let end = page * 200;
            let files = c139::get_share_files(&state.http, &session.share_id, dir_id, &session.pwd, begin, end).await?;
            let has_more = files.len() >= 200;
            (files, has_more)
        }
        Platform::Pan123 => {
            // 目录切换 → 重置游标；翻页 → 用会话游标
            let next = if dir_changed || page <= 1 { String::new() } else { session.next_cursor.clone() };
            let (files, next_cursor) = pan123::get_share_files(&state.http, &session.share_id, &session.pwd, dir_id, &next, 1).await?;
            session.next_cursor = next_cursor.clone().unwrap_or_default();
            let has_more = next_cursor.is_some();
            (files, has_more)
        }
        Platform::Xunlei => {
            let token = if dir_changed || page <= 1 { String::new() } else { session.next_page_token.clone() };
            let mut rt = {
                let guard = state.xunlei.lock().map_err(|_| AppError::Lock)?;
                guard.clone()
            };
            let (files, next) = xunlei::get_share_detail(&state.http, &mut rt, &session.share_id, dir_id, &session.pass_code_token, &token).await?;
            *state.xunlei.lock().map_err(|_| AppError::Lock)? = rt;
            session.next_page_token = next;
            let has_more = files.len() >= 100;
            (files, has_more)
        }
        Platform::Direct | Platform::Magnet => (Vec::new(), false),
    };
    session.last_dir = dir_id.to_string();
    update_session(state, session_key, session);
    Ok(ShareFilePage { files, has_more })
}

// ---------- 递归收集目录下全部文件（文件夹下载用） ----------

/// 递归收集目录下全部文件；顺带记录相对目录（还原文件夹结构保存）
pub async fn collect_folder_files(
    state: &AppState,
    session_key: &str,
    dir_id: &str,
) -> AppResult<Vec<CollectedFile>> {
    let mut all = Vec::new();
    let mut page = 1i64;
    loop {
        let page_result = list_share_files(state, session_key, dir_id, page).await?;
        let has_more = page_result.has_more;
        for f in page_result.files {
            if f.isdir {
                let mut sub = Box::pin(collect_folder_files(state, session_key, &f.fid)).await?;
                // 子目录内文件相对路径前插当前目录名
                for cf in sub.iter_mut() {
                    cf.rel_dir = if cf.rel_dir.is_empty() {
                        f.fname.clone()
                    } else {
                        format!("{}/{}", f.fname, cf.rel_dir)
                    };
                }
                all.extend(sub);
            } else {
                all.push(CollectedFile { file: f, rel_dir: String::new() });
            }
        }
        if !has_more || page > 20 {
            break;
        }
        page += 1;
    }
    Ok(all)
}

// ---------- 取下载直链 ----------

pub async fn get_download_link(
    state: &AppState,
    session_key: &str,
    file: &ShareFile,
) -> AppResult<DownloadLink> {
    let session = get_session(state, session_key)?;
    let platform = session.platform;
    let share_id = session.share_id.clone();
    match platform {
        Platform::Quark => {
            let mut cookie = load_account_cookie(state, platform, "请先登录夸克网盘")?;
            // 取链前刷新 __puus（修复 AlistGo/alist#830 下载 412）
            if let Ok(refreshed) = quark::refresh_session(&state.http, &cookie).await {
                if refreshed != cookie {
                    persist_cookie(state, platform, &refreshed, "");
                    cookie = refreshed;
                }
            }
            // ① 转存路线（他人分享）：唯一子目录 tr_*（去重键每次不同，根治二次转存 404）
            //    → 转存 → 轮询 → 取链 → cleanup = 子目录 fid（下载完成后删整个子目录）
            // ② 直取路线（自己的分享，服务端拒绝转存自己的分享）：直接用分享 fid 取链
            let (url, size, cleanup_id) = match quark_transfer_route(state, &session, &cookie, file).await {
                Ok(v) => v,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("禁止转存自己的分享") {
                        state.log(crate::logger::INFO, "quark", "link", "自己的分享，跳过转存直接取链", &file.fname);
                        let (url, _, size) = quark::get_download_link(&state.http, &file.fid, &cookie).await?;
                        (url, size, String::new())
                    } else {
                        return Err(e);
                    }
                }
            };
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: if file.fsize > 0 { file.fsize } else { size },
                headers: vec![
                    ("Cookie".into(), cookie),
                    ("User-Agent".into(), quark::UA.into()),
                    ("Referer".into(), quark::DOWNLOAD_REFERER.into()),
                ],
                platform: platform.key().to_string(),
                cleanup_id,
                mirrors: Vec::new(),
            })
        }
        Platform::Uc => {
            let mut cookie = load_account_cookie(state, platform, "请先登录 UC 网盘")?;
            if let Ok(refreshed) = uc::refresh_session(&state.http, &cookie).await {
                if refreshed != cookie {
                    persist_cookie(state, platform, &refreshed, "");
                    cookie = refreshed;
                }
            }
            let (url, _, size) = uc::get_share_download_link(
                &state.http, &file.fid, &file.fid_token, &session.stoken, &share_id, &cookie,
            )
            .await?;
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: if file.fsize > 0 { file.fsize } else { size },
                headers: vec![
                    ("Cookie".into(), cookie),
                    ("User-Agent".into(), uc::UA.into()),
                    ("Referer".into(), uc::DOWNLOAD_REFERER.into()),
                ],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Baidu if session.accel => {
            // 加速取链：列表已携带 dlink 时直接用（省每日额度），否则调 get_download_links；
            // 解析码失效（20016）自动刷新一次后重试；直链不可达时自动回退官方链路
            let (url, ua) = if file.fid_token.starts_with("http") {
                (file.fid_token.clone(), baidaccel::DOWNLOAD_UA.to_string())
            } else {
                let settings = state.load_settings();
                let base = baidaccel::base_url_of(&settings);
                let password = settings.baidu_speed_password.trim().to_string();
                let mut accel_ok = None;
                if !password.is_empty() {
                    if let Ok(link) = accel_fetch_link(state, &base, &session, file, &password).await {
                        if let Ok(true) = link_reachable(&state.http, &link.0, &link.1).await {
                            accel_ok = Some(link);
                        }
                    }
                }
                if let Some(link) = accel_ok {
                    link
                } else {
                    state.log(logger::INFO, "baidu", "accel", "加速通道不可用，立即快速切入官方直连", &file.fname);
                    return baidu_official_link(state, session_key, file).await;
                }
            };
            state.log(logger::SUCCESS, "baidu", "accel", "取链成功", &file.fname);
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: file.fsize,
                headers: vec![("User-Agent".into(), ua)],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Baidu => {
            let cookie = load_account_cookie(state, platform, "请先登录百度网盘")?;
            let temp_dir = baidu::ensure_temp_dir(&state.http, &cookie).await?;
            state.log(logger::INFO, "baidu", "transfer", "开始转存", &format!("{} → {}", file.fname, temp_dir));
            let (new_fs_id, new_path) = baidu::transfer(
                &state.http,
                &session.share_id,
                &session.baidu_share_id,
                &session.baidu_uk,
                &session.sekey,
                &file.fid,
                &temp_dir,
                &cookie,
            )
            .await
            .map_err(|e| {
                state.log(logger::ERROR, "baidu", "transfer", &format!("转存失败：{}", file.fname), &e.to_string());
                e
            })?;
            state.log(logger::SUCCESS, "baidu", "transfer", "转存成功", &format!("fs_id={new_fs_id} path={new_path}"));
            // 取链改用 BaiduPCS-Go locate_urls（多地域源站镜像提取）
            let urls = crate::baidupcs::locate_urls(&state.http, &cookie, &new_path, &state.data_dir)
                .await
                .map_err(|e| {
                    state.log(logger::ERROR, "baidu", "link", "取链失败", &format!("path={new_path} {e}"));
                    e
                })?;
            let main_url = urls.first().cloned().unwrap_or_default();
            let mirrors = if urls.len() > 1 { urls[1..].to_vec() } else { Vec::new() };
            Ok(DownloadLink {
                url: main_url,
                filename: file.fname.clone(),
                size: file.fsize,
                headers: vec![("User-Agent".into(), crate::baidupcs::UA.into())],
                platform: platform.key().to_string(),
                cleanup_id: new_path,
                mirrors,
            })
        }
        Platform::C139 => {
            let (url, _, size) = c139::get_share_download_link(
                &state.http, &file.fid, &share_id, &session.account, Some(&session.authorization),
            )
            .await?;
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: if file.fsize > 0 { file.fsize } else { size },
                headers: vec![("User-Agent".into(), c139::SHARE_MOBILE_UA.to_string())],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Pan123 => {
            let token = {
                let conn = state.db.lock().map_err(|_| AppError::Lock)?;
                let active = state.active_account_key(&platform);
                match accounts::load(&conn, platform, &active)? {
                    Some(Account::Pan123 { access_token, .. }) if !access_token.is_empty() => access_token,
                    _ => return Err(AppError::Api("请先登录 123 云盘".into())),
                }
            };
            let (url, _, size) = pan123::get_share_download_link(&state.http, &share_id, file, &token).await?;
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: if file.fsize > 0 { file.fsize } else { size },
                headers: vec![
                    ("User-Agent".into(), pan123::DART_UA.into()),
                    ("Referer".into(), pan123::DOWNLOAD_REFERER.into()),
                ],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Xunlei => {
            state.load_xunlei_runtime()?;
            let mut rt = {
                let guard = state.xunlei.lock().map_err(|_| AppError::Lock)?;
                if guard.access_token.is_empty() {
                    return Err(AppError::Api("请先登录迅雷网盘".into()));
                }
                guard.clone()
            };
            let temp_dir = xunlei::ensure_temp_dir(&state.http, &mut rt).await?;
            let new_id = xunlei::restore(
                &state.http, &mut rt, &share_id, &session.pass_code_token, &temp_dir, &[file.fid.clone()],
            )
            .await?;
            let (url, _, size) = xunlei::get_file_detail(&state.http, &mut rt, &new_id).await?;
            // 取链后即时清理（直链自带签名，删除不影响下载）
            let _ = xunlei::batch_delete(&state.http, &mut rt, &[new_id]).await;
            *state.xunlei.lock().map_err(|_| AppError::Lock)? = rt;
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: if file.fsize > 0 { file.fsize } else { size },
                headers: vec![("User-Agent".into(), xunlei::WEB_UA.into())],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Direct => {
            let url = if !file.fid_token.is_empty() { file.fid_token.clone() } else { session.share_id.clone() };
            Ok(DownloadLink {
                url,
                filename: file.fname.clone(),
                size: file.fsize,
                headers: vec![
                    ("User-Agent".into(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36".into()),
                ],
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
        Platform::Magnet => {
            let magnet = if !file.fid_token.is_empty() { file.fid_token.clone() } else { session.share_id.clone() };
            Ok(DownloadLink {
                url: magnet,
                filename: file.fname.clone(),
                size: file.fsize,
                headers: Vec::new(),
                platform: platform.key().to_string(),
                cleanup_id: String::new(),
                mirrors: Vec::new(),
            })
        }
    }
}

/// 夸克延迟清理（下载完成后调用；由 aria2 完成回调触发）
pub async fn cleanup_quark(state: &AppState, cleanup_id: &str) {
    if cleanup_id.is_empty() {
        return;
    }
    if let Ok(cookie) = load_account_cookie(state, Platform::Quark, "") {
        let _ = quark::delete_file(&state.http, cleanup_id, &cookie).await;
    }
}

/// 百度网盘延迟清理（下载完成或任务删除后调用；由 aria2 引擎触发）
pub async fn cleanup_baidu(state: &AppState, cleanup_path: &str) {
    if cleanup_path.is_empty() {
        return;
    }
    if let Ok(cookie) = load_account_cookie(state, Platform::Baidu, "") {
        let _ = crate::baidupcs::remove(&cookie, cleanup_path, &state.data_dir).await;
    }
}

