use std::path::PathBuf;
use std::sync::Mutex;

use reqwest::Client;
use rusqlite::Connection;

use crate::api::xunlei::XunleiRuntime;
use crate::error::AppResult;
use crate::models::Settings;
use crate::resolve::ResolveSession;

/// 全局应用状态
pub struct AppState {
    /// SQLite 连接（命令串行访问）
    pub db: Mutex<Connection>,
    /// 共享 HTTP 客户端（平台 API）
    pub http: Client,
    /// 解析会话表（sessionKey → 会话）
    pub sessions: Mutex<std::collections::HashMap<String, ResolveSession>>,
    /// 迅雷运行时（token / captcha / 设备指纹）
    pub xunlei: Mutex<XunleiRuntime>,
    /// 应用数据目录（settings.json / xunlei_fp.json）
    pub data_dir: PathBuf,
}

impl AppState {
    /// 写一条应用日志（best-effort：内部失败静默忽略，绝不影响主流程）
    pub fn log(&self, level: &str, platform: &str, action: &str, message: &str, detail: &str) {
        if let Ok(conn) = self.db.lock() {
            let _ = crate::logger::add(&conn, level, platform, action, message, detail);
        }
    }

    /// 初始化（建库建表 + 加载设置 + 迅雷指纹）
    pub fn new(data_dir: &std::path::Path) -> AppResult<Self> {
        let db = crate::db::init(data_dir)?;
        let xunlei_fp = crate::api::xunlei::Fingerprint::load_or_init(data_dir);
        Ok(Self {
            db: Mutex::new(db),
            http: crate::api::http_client(),
            sessions: Mutex::new(std::collections::HashMap::new()),
            xunlei: Mutex::new(XunleiRuntime { fp: xunlei_fp, ..Default::default() }),
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// 读取设置（settings.json；不存在返回默认值）
    pub fn load_settings(&self) -> Settings {
        std::fs::read_to_string(self.data_dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 保存设置
    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        let text = serde_json::to_string_pretty(settings)?;
        std::fs::write(self.data_dir.join("settings.json"), text)?;
        Ok(())
    }

    /// 从 DB 加载迅雷 token 到运行时（调用任何 pan 接口前确保）
    pub fn load_xunlei_runtime(&self) -> AppResult<()> {
        let conn = self.db.lock().map_err(|_| crate::error::AppError::Lock)?;
        let acc = crate::db::accounts::load(&conn, crate::models::Platform::Xunlei)?;
        let mut rt = self.xunlei.lock().map_err(|_| crate::error::AppError::Lock)?;
        if let Some(crate::db::accounts::Account::Xunlei {
            access_token,
            refresh_token,
            device_id,
            captcha_token,
            ..
        }) = acc
        {
            if rt.access_token.is_empty() {
                rt.access_token = access_token;
                rt.refresh_token = refresh_token;
                rt.captcha_token = captcha_token;
                if !device_id.is_empty() {
                    rt.fp.device_id = device_id;
                }
                if let Some(payload) = crate::api::jwt_payload(&rt.access_token) {
                    let sub = payload.get("sub").and_then(|x| x.as_str()).unwrap_or("");
                    if !sub.is_empty() {
                        rt.user_id = sub.to_string();
                    }
                }
            }
        }
        Ok(())
    }

    /// 把迅雷运行时 token 持久化回 DB
    pub fn persist_xunlei_runtime(&self, nickname: &str) -> AppResult<()> {
        let rt = self.xunlei.lock().map_err(|_| crate::error::AppError::Lock)?;
        if rt.access_token.is_empty() {
            return Ok(());
        }
        let conn = self.db.lock().map_err(|_| crate::error::AppError::Lock)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        crate::db::accounts::save(
            &conn,
            &crate::db::accounts::Account::Xunlei {
                access_token: rt.access_token.clone(),
                refresh_token: rt.refresh_token.clone(),
                device_id: rt.fp.device_id.clone(),
                captcha_token: rt.captcha_token.clone(),
                nickname: nickname.to_string(),
            },
            now,
        )
    }
}
