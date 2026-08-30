//! 迅雷网盘 API（移植 Android XunleiApi + XunleiConstants + XunleiDeviceFingerprint）。
//! 登录：captcha/init → v3/login（密码，可能触发 review_panel 短信）→ smslogin → v1/auth/signin/token；
//! Pan：Bearer + 设备头（无 x-signature），captcha_invalid / 401 自动刷新重试。
use std::path::Path;

use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};

use super::{jwt_payload, md5_hex, random_hex, sha1_hex};
use crate::error::{AppError, AppResult};
use crate::models::ShareFile;

const AUTH_BASE: &str = "https://xluser-ssl.xunlei.com";
const PAN_BASE: &str = "https://api-pan.xunlei.com";
pub const APP_CLIENT_ID: &str = "Xp6vsxz_7IYVw2BB";
pub const APP_CLIENT_SECRET: &str = "Xp6vsy4tN9toTVdMSpomVdXpRmES";
const APP_CLIENT_VERSION: &str = "8.31.0.9726";
const APP_PACKAGE_NAME: &str = "com.xunlei.downloadprovider";
pub const APP_UA: &str = "ANDROID-com.xunlei.downloadprovider/8.31.0.9726 netWorkType/5G appid/40 deviceName/Xiaomi_M2004j7ac deviceModel/M2004J7AC OSVersion/12 protocolVersion/301 platformVersion/10 sdkVersion/512000 Oauth2Client/0.9 (Linux 4_14_186-perf-gddfs8vbb238b) (JAVA 0)";
pub const WEB_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
pub const TEMP_DIR_NAME: &str = "YunX临时转存";

const CAPTCHA_SALTS: [&str; 10] = [
    "9uJNVj/wLmdwKrJaVj/omlQ",
    "Oz64Lp0GigmChHMf/6TNfxx7O9PyopcczMsnf",
    "Eb+L7Ce+Ej48u",
    "jKY0",
    "ASr0zCl6v8W4aidjPK5KHd1Lq3t+vBFf41dqv5+fnOd",
    "wQlozdg6r1qxh0eRmt3QgNXOvSZO6q/GXK",
    "gmirk+ciAvIgA/cxUUCema47jr/YToixTT+Q6O",
    "5IiCoM9B1/788ntB",
    "P07JH0h6qoM6TSUAK2aL9T5s2QBVeY9JWvalf",
    "+oK0AN",
];

// ---------- 设备指纹（文件持久化：xunlei_fp.json） ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Fingerprint {
    pub device_id: String,
    pub peer_id: String,
    pub device_sign: String,
}

impl Fingerprint {
    /// devicesign = div101.{deviceId}{md5(sha1(deviceId + package + appid + appkey))}
    fn build_device_sign(id: &str) -> String {
        let base = format!("{id}com.xunlei.downloadprovider4034a062aaa22f906fca4fefe9fb3a3021");
        let sha1 = sha1_hex(&base);
        format!("div101.{id}{}", md5_hex(&sha1))
    }

    pub fn load_or_init(data_dir: &Path) -> Self {
        let path = data_dir.join("xunlei_fp.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(fp) = serde_json::from_str::<Fingerprint>(&text) {
                if !fp.device_id.is_empty() && !fp.device_sign.is_empty() {
                    return fp;
                }
            }
        }
        let fp = Fingerprint {
            device_id: random_hex(32),
            peer_id: random_hex(32),
            device_sign: String::new(),
        };
        let fp = Fingerprint {
            device_sign: Self::build_device_sign(&fp.device_id),
            ..fp
        };
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&fp).unwrap_or_default());
        fp
    }
}

// ---------- 运行时会话（AccessToken / Captcha / 用户 ID） ----------

#[derive(Debug, Default, Clone)]
pub struct XunleiRuntime {
    pub fp: Fingerprint,
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub captcha_token: String,
}

/// 登录中间结果（对应 Android XunleiLoginStep）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStep {
    pub need_sms: bool,
    pub credit_key: String,
    pub sms_token: String,
    pub session_id: String,
    pub nickname: String,
    pub review_url: String,
    pub message: String,
}

fn str_or(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn i64_or(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

// ---------- captcha ----------

/// captcha_sign：client_id+version+package+device_id+ts → 10 层 md5(raw+salt) → "1.x"
fn build_captcha_sign(device_id: &str, ts_ms: &str) -> String {
    let mut h = format!("{APP_CLIENT_ID}{APP_CLIENT_VERSION}{APP_PACKAGE_NAME}{device_id}{ts_ms}");
    for salt in CAPTCHA_SALTS {
        h = md5_hex(&format!("{h}{salt}"));
    }
    format!("1.{h}")
}

/// 验证码盾初始化（action 对应目标接口；meta.user_id 空会得到降级 token）
pub async fn init_captcha(client: &Client, rt: &mut XunleiRuntime, username: &str, action: &str) -> AppResult<String> {
    let ts = chrono::Utc::now().timestamp_millis().to_string();
    let sign = build_captcha_sign(&rt.fp.device_id, &ts);
    let body = json!({
        "action": action,
        "captcha_token": rt.captcha_token,
        "client_id": APP_CLIENT_ID,
        "device_id": rt.fp.device_id,
        "meta": {
            "username": username,
            "client_version": APP_CLIENT_VERSION,
            "package_name": APP_PACKAGE_NAME,
            "timestamp": ts,
            "captcha_sign": sign,
            "user_id": rt.user_id,
        },
        "redirect_uri": "xlaccsdk01://xunlei.com/callback?state=harbor",
    });
    let resp = client
        .post(format!("{AUTH_BASE}/v1/shield/captcha/init"))
        .header("User-Agent", APP_UA)
        .header("Accept", "application/json;charset=UTF-8")
        .header("Content-Type", "application/json")
        .header("X-Client-Id", APP_CLIENT_ID)
        .header("X-Device-Id", &rt.fp.device_id)
        .header("X-Client-Version", APP_CLIENT_VERSION)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let token = str_or(&v, "captcha_token");
    if token.is_empty() {
        return Err(AppError::Api("验证码初始化失败".into()));
    }
    rt.captcha_token = token.clone();
    Ok(token)
}

// ---------- 登录 ----------

fn base_login_body(rt: &XunleiRuntime, client_version: &str, sdk_version: &str, credit_key: &str) -> Value {
    use rand::Rng;
    json!({
        "protocolVersion": "301",
        "sequenceNo": rand::thread_rng().gen_range(10_000_000i64..99_999_999).to_string(),
        "platformVersion": "10",
        "isCompressed": "0",
        "appid": "40",
        "clientVersion": client_version,
        "peerID": rt.fp.peer_id,
        "appName": "ANDROID-com.xunlei.downloadprovider",
        "sdkVersion": sdk_version,
        "devicesign": rt.fp.device_sign,
        "netWorkType": "WIFI",
        "providerName": "NONE",
        "deviceModel": "M2004J7AC",
        "deviceName": "Xiaomi_M2004j7ac",
        "OSVersion": "12",
        "creditkey": credit_key,
        "hl": "zh-CN",
    })
}

fn parse_login_response(v: &Value) -> LoginStep {
    let error_code = str_or(v, "errorCode");
    if error_code == "0" || str_or(v, "error") == "success" {
        return LoginStep {
            need_sms: false,
            credit_key: String::new(),
            sms_token: String::new(),
            session_id: str_or(v, "sessionID"),
            nickname: str_or(v, "nickName"),
            review_url: String::new(),
            message: "登录成功".into(),
        };
    }
    let error = str_or(v, "error");
    let verify_type = str_or(v, "verifyType");
    let need_sms = error == "review_panel"
        || error_code == "1007"
        || verify_type == "MEA"
        || !verify_type.is_empty();
    LoginStep {
        need_sms,
        credit_key: String::new(),
        sms_token: String::new(),
        session_id: String::new(),
        nickname: String::new(),
        review_url: str_or(v, "reviewurl"),
        message: {
            let m = str_or(v, "errorDesc");
            if m.is_empty() { str_or(v, "error_description") } else { m }
        },
    }
}

/// 账号密码登录（首次新设备必然 review_panel → 短信）
pub async fn login_with_password(client: &Client, rt: &XunleiRuntime, username: &str, password: &str) -> AppResult<LoginStep> {
    let body = base_login_body(rt, "25.0.5.25", "513006", "");
    let body = json!({
        "protocolVersion": body["protocolVersion"],
        "sequenceNo": body["sequenceNo"],
        "platformVersion": body["platformVersion"],
        "isCompressed": body["isCompressed"],
        "appid": body["appid"],
        "clientVersion": body["clientVersion"],
        "peerID": body["peerID"],
        "appName": body["appName"],
        "sdkVersion": body["sdkVersion"],
        "devicesign": body["devicesign"],
        "netWorkType": body["netWorkType"],
        "providerName": body["providerName"],
        "deviceModel": body["deviceModel"],
        "deviceName": body["deviceName"],
        "OSVersion": body["OSVersion"],
        "creditkey": body["creditkey"],
        "hl": body["hl"],
        "userName": username,
        "passWord": password,
        "verifyKey": "",
        "verifyCode": "",
        "isMd5Pwd": "0",
    });
    let resp = client
        .post(format!("{AUTH_BASE}/xluser.core.login/v3/login"))
        .header("User-Agent", "android-ok-http-client/xl-acc-sdk/version-5.1.3.513006")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    Ok(parse_login_response(&v))
}

/// 发送短信验证码 → (creditkey, token)
pub async fn send_sms(client: &Client, rt: &XunleiRuntime, mobile: &str) -> AppResult<(String, String)> {
    let body = base_login_body(rt, "8.31.0.9726", "231500", "");
    let body = json!({
        "protocolVersion": body["protocolVersion"],
        "sequenceNo": body["sequenceNo"],
        "platformVersion": body["platformVersion"],
        "isCompressed": body["isCompressed"],
        "appid": body["appid"],
        "clientVersion": body["clientVersion"],
        "peerID": body["peerID"],
        "appName": body["appName"],
        "sdkVersion": body["sdkVersion"],
        "devicesign": body["devicesign"],
        "netWorkType": body["netWorkType"],
        "providerName": body["providerName"],
        "deviceModel": body["deviceModel"],
        "deviceName": body["deviceName"],
        "OSVersion": body["OSVersion"],
        "creditkey": "",
        "hl": "zh-CN",
        "mobile": mobile,
        "register": "0",
    });
    let resp = client
        .post(format!("{AUTH_BASE}/xluser.core.login/v3/sendsms"))
        .header("User-Agent", "android-ok-http-client/xl-acc-sdk/version-5.0.12.512000")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    Ok((str_or(&v, "creditkey"), str_or(&v, "token")))
}

/// 短信验证码登录
pub async fn sms_login(
    client: &Client,
    rt: &XunleiRuntime,
    mobile: &str,
    sms_code: &str,
    credit_key: &str,
    sms_token: &str,
) -> AppResult<LoginStep> {
    let body = base_login_body(rt, "8.31.0.9726", "231500", credit_key);
    let body = json!({
        "protocolVersion": body["protocolVersion"],
        "sequenceNo": body["sequenceNo"],
        "platformVersion": body["platformVersion"],
        "isCompressed": body["isCompressed"],
        "appid": body["appid"],
        "clientVersion": body["clientVersion"],
        "peerID": body["peerID"],
        "appName": body["appName"],
        "sdkVersion": body["sdkVersion"],
        "devicesign": body["devicesign"],
        "netWorkType": body["netWorkType"],
        "providerName": body["providerName"],
        "deviceModel": body["deviceModel"],
        "deviceName": body["deviceName"],
        "OSVersion": body["OSVersion"],
        "creditkey": credit_key,
        "hl": "zh-CN",
        "mobile": mobile,
        "smsCode": sms_code,
        "token": sms_token,
        "register": "0",
    });
    let resp = client
        .post(format!("{AUTH_BASE}/xluser.core.login/v3/smslogin"))
        .header("User-Agent", "android-ok-http-client/xl-acc-sdk/version-5.0.12.512000")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    Ok(parse_login_response(&v))
}

/// 用 sessionID 换 access_token（v1/auth/signin/token，需 X-Captcha-Token）
pub async fn exchange_token(client: &Client, rt: &mut XunleiRuntime, session_id: &str) -> AppResult<(String, String)> {
    let body = json!({
        "client_id": APP_CLIENT_ID,
        "client_secret": APP_CLIENT_SECRET,
        "provider": "access_end_point_token",
        "signin_token": session_id,
    });
    let resp = client
        .post(format!("{AUTH_BASE}/v1/auth/signin/token"))
        .header("User-Agent", APP_UA)
        .header("Accept", "application/json;charset=UTF-8")
        .header("Content-Type", "application/json")
        .header("X-Client-Id", APP_CLIENT_ID)
        .header("X-Device-Id", &rt.fp.device_id)
        .header("X-Client-Version", APP_CLIENT_VERSION)
        .header("X-Captcha-Token", &rt.captcha_token)
        .json(&body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let at = {
        let s = str_or(&v, "access_token");
        if s.is_empty() { str_or(&v, "accessToken") } else { s }
    };
    let rtok = {
        let s = str_or(&v, "refresh_token");
        if s.is_empty() { str_or(&v, "refreshToken") } else { s }
    };
    if at.is_empty() {
        let msg = {
            let m = str_or(&v, "error_description");
            if m.is_empty() { "换取 access_token 失败".to_string() } else { m }
        };
        return Err(AppError::Api(msg));
    }
    if let Some(payload) = jwt_payload(&at) {
        let sub = payload.get("sub").and_then(|x| x.as_str()).unwrap_or("");
        if !sub.is_empty() {
            rt.user_id = sub.to_string();
        }
    }
    rt.access_token = at.clone();
    rt.refresh_token = rtok.clone();
    Ok((at, rtok))
}

/// 用 refresh_token 刷新 access_token
pub async fn refresh_token_flow(client: &Client, rt: &mut XunleiRuntime) -> AppResult<()> {
    if rt.refresh_token.is_empty() {
        return Err(AppError::Api("登录已过期，请重新登录迅雷".into()));
    }
    let body = format!(
        "grant_type=refresh_token&client_id={APP_CLIENT_ID}&client_secret={APP_CLIENT_SECRET}&refresh_token={}",
        urlencoding::encode(&rt.refresh_token)
    );
    let resp = client
        .post(format!("{AUTH_BASE}/v1/auth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-Device-Id", &rt.fp.device_id)
        .body(body)
        .send()
        .await?;
    let v: Value = resp.json().await?;
    let at = {
        let s = str_or(&v, "access_token");
        if s.is_empty() { str_or(&v, "accessToken") } else { s }
    };
    let rtok = {
        let s = str_or(&v, "refresh_token");
        if s.is_empty() { str_or(&v, "refreshToken") } else { s }
    };
    if at.is_empty() {
        return Err(AppError::Api("刷新登录态失败，请重新登录迅雷".into()));
    }
    if let Some(payload) = jwt_payload(&at) {
        let sub = payload.get("sub").and_then(|x| x.as_str()).unwrap_or("");
        if !sub.is_empty() {
            rt.user_id = sub.to_string();
        }
    }
    rt.access_token = at;
    if !rtok.is_empty() {
        rt.refresh_token = rtok;
    }
    Ok(())
}

/// 解析 reviewurl 查询参数（creditkey / token）
pub fn parse_review_url(review_url: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(q) = review_url.split_once('?').map(|(_, q)| q) {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                map.insert(k.to_string(), urlencoding::decode(v).unwrap_or_default().to_string());
            } else if !pair.is_empty() {
                map.insert(pair.to_string(), String::new());
            }
        }
    }
    map
}

// ---------- Pan 请求（Bearer + 设备头；captcha / token 自动刷新重试） ----------

async fn pan_execute(
    client: &Client,
    rt: &XunleiRuntime,
    method: &str,
    url: &str,
    body: Option<&Value>,
) -> AppResult<(Value, u16)> {
    let mut req = client
        .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), url)
        .header("User-Agent", WEB_UA)
        .header("Authorization", format!("Bearer {}", rt.access_token))
        .header("X-Device-Id", &rt.fp.device_id)
        .header("X-Client-Version", APP_CLIENT_VERSION)
        .header("Content-Type", "application/json")
        .header("Origin", "https://pan.xunlei.com")
        .header("Referer", "https://pan.xunlei.com/")
        .header("X-Captcha-Token", &rt.captcha_token);
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    let v = serde_json::from_str(&text)
        .map_err(|_| AppError::Api(format!("响应解析失败（HTTP {status}）")))?;
    Ok((v, status))
}

/// pan 调用（带 401 → refresh、captcha_invalid → 重新 init 的重试）
/// @param action captcha 对应 action（如 "GET:/drive/v1/share"）
async fn pan_call(
    client: &Client,
    rt: &mut XunleiRuntime,
    action: &str,
    method: &str,
    url: &str,
    body: Option<&Value>,
) -> AppResult<Value> {
    for attempt in 0..2 {
        let (v, status) = pan_execute(client, rt, method, url, body).await?;
        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("");
        if status < 400 && err.is_empty() {
            return Ok(v);
        }
        // access_token 过期：刷新 → 重新 init captcha → 重试
        if (status == 401 || err == "unauthenticated") && attempt == 0 {
            let _ = refresh_token_flow(client, rt).await;
            let _ = init_captcha(client, rt, "", action).await;
            continue;
        }
        // captcha 失效：重新 init（携带旧 token）→ 重试
        if err == "captcha_invalid" && attempt == 0 {
            init_captcha(client, rt, "", action).await?;
            continue;
        }
        let msg = {
            let m = str_or(&v, "error_description");
            if m.is_empty() {
                let m2 = str_or(&v, "message");
                if m2.is_empty() { err.to_string() } else { m2 }
            } else {
                m
            }
        };
        let msg = if msg.is_empty() { format!("请求失败（HTTP {status}）") } else { msg };
        return Err(AppError::Api(msg));
    }
    Err(AppError::Api("验证码刷新后仍失败".into()))
}

fn parse_file_array(array: &[Value]) -> Vec<ShareFile> {
    array
        .iter()
        .filter_map(|item| {
            Some(ShareFile {
                fid: str_or(item, "id"),
                fname: str_or(item, "name"),
                fsize: i64_or(item, "size"),
                isdir: str_or(item, "kind") == "drive#folder",
                pdir_fid: str_or(item, "parent_id"),
                fid_token: String::new(),
                modify_time: str_or(item, "modified_time"),
            })
        })
        .filter(|f| !f.fid.is_empty() || !f.fname.is_empty())
        .collect()
}

/// 分享解析（首页；提取码状态检查）
pub async fn get_share(
    client: &Client,
    rt: &mut XunleiRuntime,
    share_id: &str,
    pass_code: &str,
    page_token: &str,
) -> AppResult<(String, Vec<ShareFile>, String, String)> {
    let url = format!(
        "{PAN_BASE}/drive/v1/share?share_id={share_id}&pass_code={}&limit=100&page_token={}&thumbnail_size=SIZE_SMALL",
        urlencoding::encode(pass_code),
        urlencoding::encode(page_token)
    );
    let v = pan_call(client, rt, "GET:/drive/v1/share", "GET", &url, None).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    match str_or(&data, "share_status").as_str() {
        "PASS_CODE_EMPTY" => return Err(AppError::Api("请输入提取码".into())),
        "PASS_CODE_ERROR" => return Err(AppError::Api("提取码错误".into())),
        "PASS_CODE_NEED" => return Err(AppError::Api("该分享需要提取码".into())),
        _ => {}
    }
    let files = data
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| parse_file_array(a))
        .unwrap_or_default();
    Ok((
        str_or(&data, "title"),
        files,
        str_or(&data, "pass_code_token"),
        str_or(&data, "next_page_token"),
    ))
}

/// 分享子目录列表
pub async fn get_share_detail(
    client: &Client,
    rt: &mut XunleiRuntime,
    share_id: &str,
    parent_id: &str,
    pass_code_token: &str,
    page_token: &str,
) -> AppResult<(Vec<ShareFile>, String)> {
    let url = format!(
        "{PAN_BASE}/drive/v1/share/detail?share_id={share_id}&parent_id={parent_id}&pass_code_token={}&limit=100&page_token={}&thumbnail_size=SIZE_SMALL",
        urlencoding::encode(pass_code_token),
        urlencoding::encode(page_token)
    );
    let v = pan_call(client, rt, "GET:/drive/v1/share/detail", "GET", &url, None).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    let files = data
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| parse_file_array(a))
        .unwrap_or_default();
    Ok((files, str_or(&data, "next_page_token")))
}

/// 个人网盘根目录列表
async fn get_root_files(client: &Client, rt: &mut XunleiRuntime) -> AppResult<Vec<ShareFile>> {
    let filters = urlencoding::encode(r#"{"trashed":{"eq":false}}"#);
    let url = format!(
        "{PAN_BASE}/drive/v1/files?parent_id=&page_token=&limit=50&with_audit=true&filters={filters}"
    );
    let v = pan_call(client, rt, "GET:/drive/v1/files", "GET", &url, None).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    Ok(data
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| parse_file_array(a))
        .unwrap_or_default())
}

/// 创建文件夹（个人网盘）
async fn create_folder(client: &Client, rt: &mut XunleiRuntime, name: &str, parent_id: &str) -> AppResult<String> {
    let body = json!({ "kind": "drive#folder", "name": name, "parent_id": parent_id, "space": "" });
    let v = pan_call(client, rt, "POST:/drive/v1/files", "POST", &format!("{PAN_BASE}/drive/v1/files"), Some(&body)).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    let id = str_or(&data, "id");
    if id.is_empty() {
        return Err(AppError::Api("创建临时转存目录失败".into()));
    }
    Ok(id)
}

/// 确保临时转存目录存在
pub async fn ensure_temp_dir(client: &Client, rt: &mut XunleiRuntime) -> AppResult<String> {
    let root = get_root_files(client, rt).await?;
    if let Some(dir) = root.iter().find(|f| f.isdir && f.fname == TEMP_DIR_NAME) {
        return Ok(dir.fid.clone());
    }
    create_folder(client, rt, TEMP_DIR_NAME, "").await
}

/// 转存分享文件（同步返回 trace_file_ids 映射的新 id）
pub async fn restore(
    client: &Client,
    rt: &mut XunleiRuntime,
    share_id: &str,
    pass_code_token: &str,
    parent_folder_id: &str,
    file_ids: &[String],
) -> AppResult<String> {
    let body = json!({
        "share_id": share_id,
        "pass_code_token": pass_code_token,
        "parent_id": parent_folder_id,
        "ancestor_ids": [],
        "file_ids": file_ids,
        "specify_parent_id": true,
    });
    let v = pan_call(client, rt, "POST:/drive/v1/share/restore", "POST", &format!("{PAN_BASE}/drive/v1/share/restore"), Some(&body)).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    // params.trace_file_ids 是 JSON 字符串：{"分享文件id":"转存后新id"}
    let trace = data.pointer("/params/trace_file_ids").and_then(|x| x.as_str()).unwrap_or("");
    if let Ok(map) = serde_json::from_str::<Value>(trace) {
        for fid in file_ids {
            let new_id = map.get(fid.as_str()).and_then(|x| x.as_str()).unwrap_or("");
            if !new_id.is_empty() {
                return Ok(new_id.to_string());
            }
        }
    }
    let fid = str_or(&data, "file_id");
    if !fid.is_empty() {
        return Ok(fid);
    }
    Err(AppError::Api("转存失败：未返回新文件".into()))
}

/// 文件详情（下载直链 links.application/octet-stream.url）
pub async fn get_file_detail(client: &Client, rt: &mut XunleiRuntime, file_id: &str) -> AppResult<(String, String, i64)> {
    let url = format!(
        "{PAN_BASE}/drive/v1/files/{file_id}?_magic=2021&usage=PLAY&thumbnail_size=SIZE_LARGE&with=hdr10&with=subtitle_files&with=task&with=public_share_tag"
    );
    let v = pan_call(client, rt, &format!("GET:/drive/v1/files/{file_id}"), "GET", &url, None).await?;
    let data = v.get("data").cloned().unwrap_or(v.clone());
    let url_str = data
        .pointer("/links/application/octet-stream/url")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let fallback = str_or(&data, "web_content_link");
    let final_url = if url_str.is_empty() { fallback } else { url_str };
    if final_url.is_empty() {
        return Err(AppError::Api("未返回下载链接".into()));
    }
    Ok((final_url, str_or(&data, "name"), i64_or(&data, "size")))
}

/// 批量删除文件（转存清理；直链自带签名，删除不影响下载）
pub async fn batch_delete(client: &Client, rt: &mut XunleiRuntime, ids: &[String]) -> AppResult<()> {
    let body = json!({ "ids": ids, "space": "" });
    let v = pan_call(
        client,
        rt,
        "POST:/drive/v1/files:batchDelete",
        "POST",
        &format!("{PAN_BASE}/drive/v1/files:batchDelete"),
        Some(&body),
    )
    .await?;
    // 清理失败不阻断（错误也忽略）
    let _ = v;
    Ok(())
}
