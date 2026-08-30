use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;
use crate::models::{AccountSummary, Platform};

/// 内存账号模型（统一 6 平台的表结构差异）
#[derive(Debug, Clone)]
pub enum Account {
    /// 夸克 / UC / 百度：Cookie 会话
    Cookie { platform: Platform, cookie: String, nickname: String },
    /// 139：Cookie + Authorization
    C139 { cookie: String, authorization: String, nickname: String },
    /// 迅雷：token 会话
    Xunlei {
        access_token: String,
        refresh_token: String,
        device_id: String,
        captcha_token: String,
        nickname: String,
    },
    /// 123：JWT
    Pan123 { access_token: String, account: String, nickname: String },
}

impl Account {
    pub fn cookie(&self) -> &str {
        match self {
            Account::Cookie { cookie, .. } | Account::C139 { cookie, .. } => cookie,
            _ => "",
        }
    }
}

/// 读取指定平台账号
pub fn load(conn: &Connection, platform: Platform) -> AppResult<Option<Account>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let acc = match platform {
        Platform::Quark | Platform::Uc | Platform::Baidu => {
            let table = match platform {
                Platform::Quark => "quark_account",
                Platform::Uc => "uc_account",
                _ => "baidu_account",
            };
            let row: Option<(String, String)> = conn
                .query_row(
                    &format!("SELECT cookie, nickname FROM {table} WHERE id = ?1"),
                    params![platform.key()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            row.map(|(cookie, nickname)| Account::Cookie { platform, cookie, nickname })
        }
        Platform::C139 => {
            let row: Option<(String, String, String)> = conn
                .query_row(
                    "SELECT cookie, nickname, authorization FROM c139_account WHERE id = 'c139'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            row.map(|(cookie, nickname, authorization)| Account::C139 { cookie, nickname, authorization })
        }
        Platform::Xunlei => {
            let row: Option<(String, String, String, String, String)> = conn
                .query_row(
                    "SELECT access_token, refresh_token, device_id, captcha_token, nickname \
                     FROM xunlei_account WHERE id = 'xunlei'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;
            row.map(|(access_token, refresh_token, device_id, captcha_token, nickname)| {
                Account::Xunlei { access_token, refresh_token, device_id, captcha_token, nickname }
            })
        }
        Platform::Pan123 => {
            let row: Option<(String, String, String)> = conn
                .query_row(
                    "SELECT access_token, account, nickname FROM pan123_account WHERE id = 'pan123'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            row.map(|(access_token, account, nickname)| Account::Pan123 { access_token, account, nickname })
        }
    };
    // 记录更新时间（登录态刷新）
    if let Some(a) = &acc {
        let _ = save(conn, a, now);
    }
    Ok(acc)
}

/// 保存/更新账号（upsert，返回 nickname）
pub fn save(conn: &Connection, acc: &Account, now: i64) -> AppResult<()> {
    match acc {
        Account::Cookie { platform, cookie, nickname } => {
            let table = match platform {
                Platform::Quark => "quark_account",
                Platform::Uc => "uc_account",
                _ => "baidu_account",
            };
            conn.execute(
                &format!(
                    "INSERT INTO {table} (id, cookie, nickname, updated_at) VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT(id) DO UPDATE SET cookie = ?2, nickname = ?3, updated_at = ?4"
                ),
                params![platform.key(), cookie, nickname, now],
            )?;
        }
        Account::C139 { cookie, authorization, nickname } => {
            conn.execute(
                "INSERT INTO c139_account (id, cookie, nickname, authorization, updated_at) \
                 VALUES ('c139', ?1, ?2, ?3, ?4) \
                 ON CONFLICT(id) DO UPDATE SET cookie = ?1, nickname = ?2, authorization = ?3, updated_at = ?4",
                params![cookie, nickname, authorization, now],
            )?;
        }
        Account::Xunlei { access_token, refresh_token, device_id, captcha_token, nickname } => {
            conn.execute(
                "INSERT INTO xunlei_account (id, access_token, refresh_token, device_id, captcha_token, nickname, updated_at) \
                 VALUES ('xunlei', ?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET access_token = ?1, refresh_token = ?2, device_id = ?3, \
                 captcha_token = ?4, nickname = ?5, updated_at = ?6",
                params![access_token, refresh_token, device_id, captcha_token, nickname, now],
            )?;
        }
        Account::Pan123 { access_token, account, nickname } => {
            conn.execute(
                "INSERT INTO pan123_account (id, access_token, account, nickname, updated_at) \
                 VALUES ('pan123', ?1, ?2, ?3, ?4) \
                 ON CONFLICT(id) DO UPDATE SET access_token = ?1, account = ?2, nickname = ?3, updated_at = ?4",
                params![access_token, account, nickname, now],
            )?;
        }
    }
    Ok(())
}

/// 登出（删除账号行）
pub fn delete(conn: &Connection, platform: Platform) -> AppResult<()> {
    let table = match platform {
        Platform::Quark => "quark_account",
        Platform::Uc => "uc_account",
        Platform::Xunlei => "xunlei_account",
        Platform::Baidu => "baidu_account",
        Platform::C139 => "c139_account",
        Platform::Pan123 => "pan123_account",
    };
    conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![platform.key()])?;
    Ok(())
}

/// 平台账号摘要列表（网盘页展示）
pub fn list_summaries(conn: &Connection) -> AppResult<Vec<AccountSummary>> {
    let platforms = [
        Platform::Quark,
        Platform::Uc,
        Platform::Xunlei,
        Platform::Baidu,
        Platform::C139,
        Platform::Pan123,
    ];
    let mut out = Vec::with_capacity(platforms.len());
    for p in platforms {
        let acc = load(conn, p)?;
        out.push(AccountSummary {
            platform: p.key().to_string(),
            nickname: acc.as_ref().map(|a| a_nickname(a)).filter(|n| !n.is_empty()),
            logged_in: acc.is_some(),
        });
    }
    Ok(out)
}

fn a_nickname(a: &Account) -> String {
    match a {
        Account::Cookie { nickname, .. }
        | Account::C139 { nickname, .. }
        | Account::Xunlei { nickname, .. }
        | Account::Pan123 { nickname, .. } => nickname.clone(),
    }
}
