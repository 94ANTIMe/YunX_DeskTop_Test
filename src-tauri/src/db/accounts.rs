use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto;
use crate::error::AppResult;
use crate::models::{AccountRow, AccountSummary, Platform};

/// 内存账号模型（统一 6 平台的表结构差异；敏感字段经 DPAPI 加密后落盘）
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

// ---------- 加解密辅助（DPAPI；失败回退原文避免阻塞主流程） ----------

fn enc(s: &str) -> String {
    crypto::encrypt(s).unwrap_or_else(|_| s.to_string())
}

/// 解密：旧明文（无前缀）原样返回；带前缀解密失败同样返回原文（保持流程可用，日志可见异常）
fn dec(s: &str) -> String {
    match crypto::decrypt(s) {
        Ok(Some(v)) => v,
        _ => s.to_string(),
    }
}

/// 生成新账号 key（平台表 id 列；登录新账号时使用）
pub fn new_key() -> String {
    format!("acc-{}", uuid::Uuid::new_v4().simple())
}

// ---------- 读取 ----------

/// 读取平台当前账号。
/// `active_key = ""` 时按「最近更新」兜底（兼容 v0.2 无多账号概念的单行库）。
/// 读到的敏感字段自动解密；顺带 touch updated_at（旧明文因此就地迁移为密文）。
pub fn load(conn: &Connection, platform: Platform, active_key: &str) -> AppResult<Option<Account>> {
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
            load_cookie(conn, platform, table, active_key)?
        }
        Platform::C139 => load_c139(conn, active_key)?,
        Platform::Xunlei => load_xunlei(conn, active_key)?,
        Platform::Pan123 => load_pan123(conn, active_key)?,
    };
    // 记录更新时间（登录态刷新）；顺带把旧明文行加密迁移
    if let (Some(acc), Some(key)) = (&acc, &row_key(conn, platform, active_key)?) {
        let _ = save_with_key(conn, acc, now, key);
    }
    Ok(acc)
}

/// 查询行主键：优先 active_key；无 → 最近更新行的 id
fn row_key(conn: &Connection, platform: Platform, active_key: &str) -> AppResult<Option<String>> {
    let table = match platform {
        Platform::Quark => "quark_account",
        Platform::Uc => "uc_account",
        Platform::Baidu => "baidu_account",
        Platform::C139 => "c139_account",
        Platform::Xunlei => "xunlei_account",
        Platform::Pan123 => "pan123_account",
    };
    if !active_key.is_empty() {
        let exists: Option<String> = conn
            .query_row(
                &format!("SELECT id FROM {table} WHERE id = ?1"),
                params![active_key],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Ok(Some(active_key.to_string()));
        }
    }
    Ok(conn
        .query_row(
            &format!("SELECT id FROM {table} ORDER BY updated_at DESC LIMIT 1"),
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

fn load_cookie(conn: &Connection, platform: Platform, table: &str, active_key: &str) -> AppResult<Option<Account>> {
    let sql = if active_key.is_empty() {
        format!("SELECT id, cookie, nickname FROM {table} ORDER BY updated_at DESC LIMIT 1")
    } else {
        format!("SELECT id, cookie, nickname FROM {table} WHERE id = ?1")
    };
    let row: Option<(String, String, String)> = if active_key.is_empty() {
        conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?
    } else {
        conn.query_row(&sql, params![active_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?
    };
    Ok(row.map(|(_, cookie, nickname)| Account::Cookie {
        platform,
        cookie: dec(&cookie),
        nickname,
    }))
}

fn load_c139(conn: &Connection, active_key: &str) -> AppResult<Option<Account>> {
    let sql = if active_key.is_empty() {
        "SELECT id, cookie, nickname, authorization FROM c139_account ORDER BY updated_at DESC LIMIT 1"
    } else {
        "SELECT id, cookie, nickname, authorization FROM c139_account WHERE id = ?1"
    };
    let row: Option<(String, String, String, String)> = if active_key.is_empty() {
        conn.query_row(sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?
    } else {
        conn.query_row(sql, params![active_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?
    };
    Ok(row.map(|(_, cookie, nickname, authorization)| Account::C139 {
        cookie: dec(&cookie),
        authorization: dec(&authorization),
        nickname,
    }))
}

fn load_xunlei(conn: &Connection, active_key: &str) -> AppResult<Option<Account>> {
    let sql = if active_key.is_empty() {
        "SELECT id, access_token, refresh_token, device_id, captcha_token, nickname \
         FROM xunlei_account ORDER BY updated_at DESC LIMIT 1"
    } else {
        "SELECT id, access_token, refresh_token, device_id, captcha_token, nickname \
         FROM xunlei_account WHERE id = ?1"
    };
    let row: Option<(String, String, String, String, String, String)> = if active_key.is_empty() {
        conn.query_row(sql, [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .optional()?
    } else {
        conn.query_row(sql, params![active_key], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .optional()?
    };
    Ok(row.map(|(_, access_token, refresh_token, device_id, captcha_token, nickname)| Account::Xunlei {
        access_token: dec(&access_token),
        refresh_token: dec(&refresh_token),
        device_id,
        captcha_token: dec(&captcha_token),
        nickname,
    }))
}

fn load_pan123(conn: &Connection, active_key: &str) -> AppResult<Option<Account>> {
    let sql = if active_key.is_empty() {
        "SELECT id, access_token, account, nickname FROM pan123_account ORDER BY updated_at DESC LIMIT 1"
    } else {
        "SELECT id, access_token, account, nickname FROM pan123_account WHERE id = ?1"
    };
    let row: Option<(String, String, String, String)> = if active_key.is_empty() {
        conn.query_row(sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?
    } else {
        conn.query_row(sql, params![active_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?
    };
    Ok(row.map(|(_, access_token, account, nickname)| Account::Pan123 {
        access_token: dec(&access_token),
        account,
        nickname,
    }))
}

// ---------- 保存 ----------

/// 登录新账号：插入新行（新 key），返回 key（调用方负责写入 settings 当前账号）
pub fn save(conn: &Connection, acc: &Account, now: i64) -> AppResult<String> {
    let key = new_key();
    save_with_key(conn, acc, now, &key)?;
    Ok(key)
}

/// 按指定 key upsert（Cookie 刷新 / 运行时持久化 / 迁移用）
pub fn save_with_key(conn: &Connection, acc: &Account, now: i64, key: &str) -> AppResult<()> {
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
                params![key, enc(cookie), nickname, now],
            )?;
        }
        Account::C139 { cookie, authorization, nickname } => {
            conn.execute(
                "INSERT INTO c139_account (id, cookie, nickname, authorization, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET cookie = ?2, nickname = ?3, authorization = ?4, updated_at = ?5",
                params![key, enc(cookie), nickname, enc(authorization), now],
            )?;
        }
        Account::Xunlei { access_token, refresh_token, device_id, captcha_token, nickname } => {
            conn.execute(
                "INSERT INTO xunlei_account (id, access_token, refresh_token, device_id, captcha_token, nickname, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(id) DO UPDATE SET access_token = ?2, refresh_token = ?3, device_id = ?4, \
                 captcha_token = ?5, nickname = ?6, updated_at = ?7",
                params![key, enc(access_token), enc(refresh_token), device_id, enc(captcha_token), nickname, now],
            )?;
        }
        Account::Pan123 { access_token, account, nickname } => {
            conn.execute(
                "INSERT INTO pan123_account (id, access_token, account, nickname, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET access_token = ?2, account = ?3, nickname = ?4, updated_at = ?5",
                params![key, enc(access_token), account, nickname, now],
            )?;
        }
    }
    Ok(())
}

// ---------- 删除 / 列表 ----------

/// 删除指定账号行（登出单账号）
pub fn delete(conn: &Connection, platform: Platform, key: &str) -> AppResult<()> {
    let table = match platform {
        Platform::Quark => "quark_account",
        Platform::Uc => "uc_account",
        Platform::Xunlei => "xunlei_account",
        Platform::Baidu => "baidu_account",
        Platform::C139 => "c139_account",
        Platform::Pan123 => "pan123_account",
    };
    conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![key])?;
    Ok(())
}

/// 平台账号行列表（多账号切换 UI；active = 当前选中）
pub fn list_rows(conn: &Connection, platform: Platform, active_key: &str) -> AppResult<Vec<AccountRow>> {
    let table = match platform {
        Platform::Quark => "quark_account",
        Platform::Uc => "uc_account",
        Platform::Baidu => "baidu_account",
        Platform::C139 => "c139_account",
        Platform::Xunlei => "xunlei_account",
        Platform::Pan123 => "pan123_account",
    };
    let cols = match platform {
        Platform::Xunlei => "id, nickname, updated_at",
        _ => "id, nickname, updated_at",
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {cols} FROM {table} ORDER BY updated_at DESC LIMIT 20"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
    })?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        let active = row.0 == active_key;
        out.push(AccountRow {
            platform: platform.key().to_string(),
            key: row.0,
            nickname: row.1,
            updated_at: row.2,
            active,
        });
    }
    Ok(out)
}

/// 平台账号摘要列表（网盘页卡片）
pub fn list_summaries(
    conn: &Connection,
    active_keys: &std::collections::BTreeMap<String, String>,
) -> AppResult<Vec<AccountSummary>> {
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
        let active = active_keys.get(p.key()).cloned().unwrap_or_default();
        let acc = load(conn, p, &active)?;
        out.push(AccountSummary {
            platform: p.key().to_string(),
            nickname: acc.as_ref().map(a_nickname).filter(|n| !n.is_empty()),
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