//! 登录模块：WebView2 窗口承载平台登录页 + Cookie 轮询抓取（含 HttpOnly）。
//! 夸克 / UC / 百度 / 139：网页登录 → cookies_for_url 轮询 → 平台 API 验证 → 存库关窗。
//! 迅雷：密码登录（可能触发短信）→ 短信登录 → signin/token 换 access_token。
//! 123：账号密码 → JWT。
//!
//! 重要：每个平台的登录窗口使用独立的 WebView2 数据目录（data_dir/webview-login/<平台>），
//! 每次打开登录窗前会清空该目录。这样每次登录都从干净 Cookie 出发，避免「退出登录后旧账号
//! 会话残留 → 重新登录又自动回登」的问题（Quark/UC/百度/139 共用此机制）。
use tauri::{AppHandle, Emitter, Manager};

use crate::api::{baidu, c139, pan123, quark, uc, xunlei};
use crate::db::accounts::{self, Account};
use crate::error::{AppError, AppResult};
use crate::models::Platform;
use crate::state::AppState;

/// 各平台登录配置
struct WebLoginConfig {
    label: String,
    login_url: &'static str,
    cookie_urls: &'static [&'static str],
    title: &'static str,
}

fn web_login_config(platform: Platform) -> WebLoginConfig {
    match platform {
        Platform::Quark => WebLoginConfig {
            label: "login-quark".into(),
            login_url: "https://pan.quark.cn/?fr=pc&platform=pc",
            cookie_urls: &["https://pan.quark.cn"],
            title: "登录夸克网盘",
        },
        Platform::Uc => WebLoginConfig {
            label: "login-uc".into(),
            login_url: "https://drive.uc.cn/",
            cookie_urls: &["https://drive.uc.cn", "https://fast.uc.cn"],
            title: "登录 UC 网盘",
        },
        Platform::Baidu => WebLoginConfig {
            label: "login-baidu".into(),
            login_url: "https://pan.baidu.com/",
            cookie_urls: &["https://pan.baidu.com", "https://d.pcs.baidu.com"],
            title: "登录百度网盘",
        },
        Platform::C139 => WebLoginConfig {
            label: "login-c139".into(),
            login_url: "https://yun.139.com/m/#/login",
            cookie_urls: &["https://mail.10086.cn", "https://yun.139.com"],
            title: "登录 139 网盘",
        },
        _ => WebLoginConfig {
            label: String::new(),
            login_url: "",
            cookie_urls: &[],
            title: "",
        },
    }
}

/// 139 登录态保留字段（对齐 Android C139Constants.KEEP_KEYS）
const C139_KEEP_KEYS: &[&str] = &[
    "Os_SSo_Sid", "RMKEY", "UserData", "Login_UserNumber",
    "_139_index_isLoginType", "UUIDToken", "JSESSIONID",
    "areaCode8011", "provCode8011",
    "authorization", "auth_token", "token", "ud_id",
    "ORCHES-I-ACCOUNT-SIMPLIFY", "ORCHES-I-ACCOUNT-ENCRYPT", "nation_code",
    "platform", "cutover_status", "a_k", "skey", "WT_FPC",
];

/// 读取窗口在某 URL 域上的 Cookie（"k=v; k=v" 形式；含 HttpOnly）
fn cookies_for_url(window: &tauri::WebviewWindow, url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let cookies = window.cookies_for_url(parsed).ok()?;
    let joined = cookies
        .iter()
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; ");
    Some(joined).filter(|s| !s.is_empty())
}

/// 汇总平台登录 Cookie（139 需跨域过滤保留字段）
fn collect_cookie(window: &tauri::WebviewWindow, platform: Platform, urls: &[&str]) -> String {
    match platform {
        Platform::C139 => {
            let mut out: Vec<String> = Vec::new();
            for url in urls {
                if let Some(cookie) = cookies_for_url(window, url) {
                    for kv in cookie.split(";") {
                        let kv = kv.trim();
                        if let Some(name) = kv.split('=').next() {
                            if C139_KEEP_KEYS.contains(&name) && !out.iter().any(|x| x.starts_with(&format!("{name}="))) {
                                out.push(kv.to_string());
                            }
                        }
                    }
                }
            }
            out.join("; ")
        }
        _ => {
            let mut out: Vec<String> = Vec::new();
            for url in urls {
                if let Some(cookie) = cookies_for_url(window, url) {
                    for kv in cookie.split(";") {
                        let kv = kv.trim();
                        if !kv.is_empty()
                            && !out.iter().any(|x| x.starts_with(&format!("{}/", kv.split('=').next().unwrap_or(""))) || *x == kv)
                        {
                            out.push(kv.to_string());
                        }
                    }
                }
            }
            out.join("; ")
        }
    }
}

/// 验证 Cookie 并保存账号（返回昵称；失败 None）
/// 注意：先做异步验证，后短锁写库（MutexGuard 不得跨 await）
async fn validate_and_save(state: &AppState, platform: Platform, cookie: &str) -> Option<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    match platform {
        Platform::Quark => {
            if !quark::is_valid_cookie(cookie) {
                return None;
            }
            let nickname = quark::fetch_nickname(&state.http, cookie).await.ok()?;
            let conn = state.db.lock().ok()?;
            let key = accounts::save(
                &conn,
                &Account::Cookie { platform, cookie: cookie.into(), nickname: nickname.clone() },
                now,
            )
            .ok()?;
            drop(conn);
            state.set_active_account(&platform, &key);
            Some(nickname)
        }
        Platform::Uc => {
            if !uc::is_valid_cookie(cookie) {
                return None;
            }
            let nickname = uc::fetch_nickname(&state.http, cookie).await.ok()?;
            let conn = state.db.lock().ok()?;
            let key = accounts::save(
                &conn,
                &Account::Cookie { platform, cookie: cookie.into(), nickname: nickname.clone() },
                now,
            )
            .ok()?;
            drop(conn);
            state.set_active_account(&platform, &key);
            Some(nickname)
        }
        Platform::Baidu => {
            if !baidu::is_valid_cookie(cookie) {
                return None;
            }
            let nickname = baidu::fetch_nickname(&state.http, cookie).await.ok()?;
            let conn = state.db.lock().ok()?;
            let key = accounts::save(
                &conn,
                &Account::Cookie { platform, cookie: cookie.into(), nickname: nickname.clone() },
                now,
            )
            .ok()?;
            drop(conn);
            state.set_active_account(&platform, &key);
            Some(nickname)
        }
        Platform::C139 => {
            if !c139::is_valid_cookie(cookie) {
                return None;
            }
            let nickname = c139::extract_nickname(cookie).unwrap_or_else(|| "139 用户".into());
            let authorization = c139::extract_authorization(cookie).unwrap_or_default();
            let conn = state.db.lock().ok()?;
            let key = accounts::save(
                &conn,
                &Account::C139 { cookie: cookie.into(), authorization, nickname: nickname.clone() },
                now,
            )
            .ok()?;
            drop(conn);
            state.set_active_account(&platform, &key);
            Some(nickname)
        }
        _ => None,
    }
}

/// 新账号登录保存：以独立新行入库（多账号），并写为当前选中账号；返回 key
fn set_xunlei_new_account(state: &AppState, nickname: &str) -> Option<String> {
    let rt = state.xunlei.lock().ok()?;
    if rt.access_token.is_empty() {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let conn = state.db.lock().ok()?;
    let key = accounts::save(
        &conn,
        &Account::Xunlei {
            access_token: rt.access_token.clone(),
            refresh_token: rt.refresh_token.clone(),
            device_id: rt.fp.device_id.clone(),
            captcha_token: rt.captcha_token.clone(),
            nickname: nickname.to_string(),
        },
        now,
    )
    .ok()?;
    drop(conn);
    state.set_active_account(&Platform::Xunlei, &key);
    Some(key)
}

/// 打开 WebView 登录窗口并启动 Cookie 轮询
pub fn web_login_start(app: &AppHandle, platform: Platform) -> AppResult<()> {
    let config = web_login_config(platform);
    if config.label.is_empty() {
        return Err(AppError::Api("该平台不支持网页登录".into()));
    }
    // 已有窗口则聚焦
    if let Some(existing) = app.get_webview_window(&config.label) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let data_dir = app.state::<AppState>().data_dir.clone();
    let login_log = move |msg: &str| {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data_dir.join("login.log"))
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "{} {}", chrono::Local::now().format("%m-%d %H:%M:%S"), msg)
            });
    };
    let url = reqwest::Url::parse(config.login_url)
        .map_err(|_| AppError::Api("登录地址无效".into()))?;
    // 每次登录用干净数据目录（清空旧 Cookie，杜绝旧账号自动回登）
    clear_login_profile(app, platform);
    login_log(&format!("打开登录窗口 {} → {url}", config.label));
    let nav_logger = login_log.clone();
    let profile_dir = login_profile_dir(app, platform);
    let window = tauri::WebviewWindowBuilder::new(app, &config.label, tauri::WebviewUrl::External(url))
        .title(config.title)
        .inner_size(980.0, 700.0)
        .center()
        // 独立 WebView2 数据目录：登录 Cookie 与主窗口隔离，且每会话干净
        .data_directory(profile_dir)
        // 桌面 Chrome UA（部分平台对非浏览器 UA 返回空白页）
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .on_navigation(move |nav| {
            nav_logger(&format!("导航: {}", nav.as_str()));
            true
        })
        .build()
        .map_err(|e| {
            login_log(&format!("窗口创建失败: {e:?}"));
            AppError::Api(format!("打开登录窗口失败: {e}"))
        })?;

    // Cookie 轮询任务（2s 一次；Cookie 变化即验证，成功则存库关窗）
    let app = app.clone();
    let label = config.label.clone();
    let cookie_urls: Vec<&'static str> = config.cookie_urls.to_vec();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut last_cookie = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if std::time::Instant::now() > deadline {
                break;
            }
            // 窗口已关闭（用户放弃）→ 静默退出
            let Some(window) = app.get_webview_window(&label) else { break };
            let cookie = collect_cookie(&window, platform, &cookie_urls);
            if cookie.is_empty() || cookie == last_cookie {
                continue;
            }
            last_cookie = cookie.clone();
            if let Some(nickname) = validate_and_save(&state, platform, &cookie).await {
                let _ = window.close();
                let _ = app.emit("login:success", serde_json::json!({ "platform": platform.key(), "nickname": nickname }));
                break;
            }
        }
    });
    let _ = window;
    Ok(())
}

/// 取消网页登录（关闭窗口）
pub fn web_login_cancel(app: &AppHandle, platform: Platform) -> AppResult<()> {
    let config = web_login_config(platform);
    if let Some(window) = app.get_webview_window(&config.label) {
        let _ = window.close();
    }
    Ok(())
}

/// 登录专用 WebView2 数据目录（每个平台独立，隔离登录 Cookie 以免跨会话串扰）
fn login_profile_dir(app: &AppHandle, platform: Platform) -> std::path::PathBuf {
    app.state::<AppState>().data_dir.join("webview-login").join(platform.key())
}

/// 清空平台登录数据目录（登出时调用，确保下次登录从干净 Cookie 出发）
pub fn clear_login_profile(app: &AppHandle, platform: Platform) {
    let dir = login_profile_dir(app, platform);
    // best-effort：失败静默忽略，不阻塞登出主流程
    let _ = std::fs::remove_dir_all(&dir);
    state_log(app, &format!("clear_login_profile: 已清空登录数据目录 {}", dir.display()));
}

fn state_log(app: &AppHandle, msg: &str) {
    let state = app.state::<AppState>();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.data_dir.join("login.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%m-%d %H:%M:%S"), msg);
    }
}

// ---------- 迅雷表单登录 ----------

/// 迅雷密码登录（可能触发短信验证步骤）
pub async fn xunlei_login(
    app: &AppHandle,
    username: &str,
    password: &str,
) -> AppResult<xunlei::LoginStep> {
    let state = app.state::<AppState>();
    // 克隆运行时（MutexGuard 不跨 await）；结束后写回
    let mut rt = {
        let guard = state.xunlei.lock().map_err(|_| AppError::Lock)?;
        guard.clone()
    };
    let step = xunlei::login_with_password(&state.http, &rt, username, password).await?;
    // 密码直接成功（受信设备）→ captcha init → exchange token
    if !step.session_id.is_empty() {
        xunlei::init_captcha(&state.http, &mut rt, username, "POST:/auth/signin/token").await?;
        let exchanged = xunlei::exchange_token(&state.http, &mut rt, &step.session_id).await;
        *state.xunlei.lock().map_err(|_| AppError::Lock)? = rt;
        if let Err(e) = exchanged {
            return Err(e);
        }
        let nickname = if step.nickname.is_empty() { "迅雷用户".to_string() } else { step.nickname.clone() };
        if set_xunlei_new_account(&state, &nickname).is_none() {
            return Err(AppError::Api("账号保存失败".into()));
        }
        let _ = app.emit("login:success", serde_json::json!({ "platform": "xunlei", "nickname": nickname }));
        return Ok(xunlei::LoginStep {
            need_sms: false,
            credit_key: String::new(),
            sms_token: String::new(),
            session_id: String::new(),
            nickname,
            review_url: String::new(),
            message: "登录成功".into(),
        });
    }
    // 触发短信验证：发送验证码（creditkey/token 优先取 reviewurl 参数，兜底 sendsms 响应）
    let mut step = step;
    let mut credit_key = String::new();
    let mut sms_token = String::new();
    if !step.review_url.is_empty() {
        let params = xunlei::parse_review_url(&step.review_url);
        credit_key = params.get("creditkey").cloned().unwrap_or_default();
        sms_token = params.get("token").cloned().unwrap_or_default();
    }
    if credit_key.is_empty() || sms_token.is_empty() {
        let (ck, tk) = xunlei::send_sms(&state.http, &rt, username).await?;
        if !ck.is_empty() {
            credit_key = ck;
        }
        if !tk.is_empty() {
            sms_token = tk;
        }
    }
    step.credit_key = credit_key;
    step.sms_token = sms_token;
    Ok(step)
}

/// 迅雷短信验证码登录
pub async fn xunlei_sms_login(
    app: &AppHandle,
    username: &str,
    sms_code: &str,
    credit_key: &str,
    sms_token: &str,
) -> AppResult<xunlei::LoginStep> {
    let state = app.state::<AppState>();
    let mut rt = {
        let guard = state.xunlei.lock().map_err(|_| AppError::Lock)?;
        guard.clone()
    };
    let step = xunlei::sms_login(&state.http, &rt, username, sms_code, credit_key, sms_token).await?;
    if step.session_id.is_empty() {
        return Ok(step);
    }
    xunlei::init_captcha(&state.http, &mut rt, username, "POST:/auth/signin/token").await?;
    let _ = xunlei::exchange_token(&state.http, &mut rt, &step.session_id).await?;
    *state.xunlei.lock().map_err(|_| AppError::Lock)? = rt;
    let nickname = if step.nickname.is_empty() { "迅雷用户".to_string() } else { step.nickname.clone() };
    if set_xunlei_new_account(&state, &nickname).is_none() {
        return Err(AppError::Api("账号保存失败".into()));
    }
    let _ = app.emit("login:success", serde_json::json!({ "platform": "xunlei", "nickname": nickname }));
    Ok(xunlei::LoginStep {
        need_sms: false,
        credit_key: String::new(),
        sms_token: String::new(),
        session_id: String::new(),
        nickname,
        review_url: String::new(),
        message: "登录成功".into(),
    })
}

// ---------- 123 表单登录 ----------

pub async fn pan123_login(app: &AppHandle, account: &str, password: &str) -> AppResult<String> {
    let state = app.state::<AppState>();
    let token = pan123::login(&state.http, account, password).await?;
    let nickname = pan123::fetch_nickname(&state.http, &token).await?;
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let key = accounts::save(
        &conn,
        &Account::Pan123 { access_token: token, account: account.to_string(), nickname: nickname.clone() },
        now,
    )?;
    drop(conn);
    state.set_active_account(&Platform::Pan123, &key);
    let _ = app.emit("login:success", serde_json::json!({ "platform": "pan123", "nickname": nickname }));
    Ok(nickname)
}
