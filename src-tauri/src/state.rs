use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

use reqwest::Client;
use rusqlite::Connection;

use crate::api::xunlei::XunleiRuntime;
use crate::error::AppResult;
use crate::models::Settings;
use crate::resolve::ResolveSessions;

/// 全局应用状态
pub struct AppState {
    /// SQLite 连接（命令串行访问）
    pub db: Mutex<Connection>,
    /// 共享 HTTP 客户端（平台 API）
    pub http: Client,
    /// 解析会话表（sessionKey → 会话）
    pub sessions: Mutex<ResolveSessions>,
    /// 迅雷运行时（token / captcha / 设备指纹）
    pub xunlei: Mutex<XunleiRuntime>,
    /// 应用数据目录（settings.json / xunlei_fp.json）
    pub data_dir: PathBuf,
    /// 设置内存缓存（剪贴板 1.5s / 轮询 1s / 取链等热点路径免磁盘 IO + DPAPI 解密；save_settings 同步刷新）
    pub settings_cache: RwLock<Option<Settings>>,
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
            sessions: Mutex::new(ResolveSessions::default()),
            xunlei: Mutex::new(XunleiRuntime { fp: xunlei_fp, ..Default::default() }),
            data_dir: data_dir.to_path_buf(),
            settings_cache: RwLock::new(None),
        })
    }

    /// 读取设置（优先内存缓存；未命中才读 settings.json + DPAPI 解密）
    pub fn load_settings(&self) -> Settings {
        if let Ok(cache) = self.settings_cache.read() {
            if let Some(cached) = cache.clone() {
                return cached;
            }
        }
        let mut settings: Settings = std::fs::read_to_string(self.data_dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if !settings.proxy_password.is_empty() {
            settings.proxy_password =
                crate::crypto::decrypt(&settings.proxy_password).ok().flatten().unwrap_or_default();
        }
        if let Ok(mut cache) = self.settings_cache.write() {
            *cache = Some(settings.clone());
        }
        settings
    }

    /// IPC 首次读取使用严格模式：已有文件损坏时返回错误，让前端可以展示恢复页。
    pub fn load_settings_checked(&self) -> AppResult<Settings> {
        let path = self.data_dir.join("settings.json");
        if !path.exists() {
            return Ok(self.load_settings());
        }
        let text = std::fs::read_to_string(path)?;
        let mut settings: Settings = serde_json::from_str(&text)?;
        if !settings.proxy_password.is_empty() {
            settings.proxy_password = crate::crypto::decrypt(&settings.proxy_password)?.unwrap_or_default();
        }
        if let Ok(mut cache) = self.settings_cache.write() {
            *cache = Some(settings.clone());
        }
        Ok(settings)
    }

    /// 保存设置（代理密码 DPAPI 加密后落盘）
    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        let mut to_write = settings.clone();
        if !to_write.proxy_password.is_empty() {
            to_write.proxy_password = crate::crypto::encrypt(&to_write.proxy_password)?;
        }
        let text = serde_json::to_string_pretty(&to_write)?;
        let destination = self.data_dir.join("settings.json");
        let temporary = self.data_dir.join("settings.json.tmp");
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        atomic_replace(&temporary, &destination)?;
        // 同步刷新内存缓存（保持解密态，与 load_settings 语义一致）
        if let Ok(mut cache) = self.settings_cache.write() {
            *cache = Some(settings.clone());
        }
        Ok(())
    }

    /// 平台当前账号 key（无选中回退平台 key——兼容 v0.2 单行库）
    pub fn active_account_key(&self, platform: &crate::models::Platform) -> String {
        let settings = self.load_settings();
        settings
            .active_account_keys
            .get(platform.key())
            .cloned()
            .unwrap_or_else(|| platform.key().to_string())
    }

    /// 写入平台当前账号 key
    pub fn set_active_account(&self, platform: &crate::models::Platform, key: &str) {
        let mut settings = self.load_settings();
        settings.active_account_keys.insert(platform.key().to_string(), key.to_string());
        let _ = self.save_settings(&settings);
    }

    /// 从 DB 加载迅雷 token 到运行时（调用任何 pan 接口前确保）
    pub fn load_xunlei_runtime(&self) -> AppResult<()> {
        let active = self.active_account_key(&crate::models::Platform::Xunlei);
        let acc = {
            let conn = self.db.lock().map_err(|_| crate::error::AppError::Lock)?;
            crate::db::accounts::load(&conn, crate::models::Platform::Xunlei, &active)?
        };
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

    /// 把迅雷运行时 token 持久化回 DB（写入当前选中账号行）
    pub fn persist_xunlei_runtime(&self, nickname: &str) -> AppResult<()> {
        // 快照运行时后立即释放 xunlei 锁：DB 写不再嵌套在 xunlei 锁内（消除与 load_xunlei_runtime 的 ABBA 死锁隐患）
        let snapshot = {
            let rt = self.xunlei.lock().map_err(|_| crate::error::AppError::Lock)?;
            if rt.access_token.is_empty() {
                return Ok(());
            }
            (rt.access_token.clone(), rt.refresh_token.clone(), rt.fp.device_id.clone(), rt.captcha_token.clone())
        };
        let conn = self.db.lock().map_err(|_| crate::error::AppError::Lock)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let key = self.active_account_key(&crate::models::Platform::Xunlei);
        crate::db::accounts::save_with_key(
            &conn,
            &crate::db::accounts::Account::Xunlei {
                access_token: snapshot.0,
                refresh_token: snapshot.1,
                device_id: snapshot.2,
                captcha_token: snapshot.3,
                nickname: nickname.to_string(),
            },
            now,
            &key,
        )
    }
}

#[cfg(windows)]
fn atomic_replace(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { MoveFileExW(PCWSTR(source.as_ptr()), PCWSTR(destination.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
}

#[cfg(not(windows))]
fn atomic_replace(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(source, destination)
}
