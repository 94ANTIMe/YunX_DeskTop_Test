//! 平台 API 公共层：HTTP 客户端 / 编解码工具 / Cookie 工具。
pub mod baidu;
pub mod c139;
pub mod pan123;
pub mod pansou;
pub mod quark;
pub mod baidaccel;
pub mod uc;
pub mod xunlei;

use md5::{Digest, Md5};
use reqwest::Client;
use sha1::Sha1;

/// 构建共享 HTTP 客户端（对齐 Android HttpClients.apiClient 超时）
pub fn http_client() -> Client {
    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .expect("failed to build http client")
}

// ---------- 编解码工具 ----------

/// MD5 十六进制（小写）
pub fn md5_hex(input: &str) -> String {
    let d = Md5::digest(input.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-1 十六进制（小写）
pub fn sha1_hex(input: &str) -> String {
    let d = Sha1::digest(input.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// 标准 Base64 编码（含 padding，对应 Android Base64.NO_WRAP）
pub fn b64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.encode(data)
}

/// 标准 Base64 解码（容错 URL-safe 字符）
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let normalized = s.trim().replace('-', "+").replace('_', "/");
    let padded = match normalized.len() % 4 {
        2 => format!("{normalized}=="),
        3 => format!("{normalized}="),
        _ => normalized,
    };
    STANDARD.decode(padded.as_bytes()).ok()
}

/// 标准 CRC-32（IEEE 802.3）→ 8 位小写十六进制
pub fn crc32_hex(s: &str) -> String {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in s.as_bytes() {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    format!("{:x}", crc ^ 0xFFFF_FFFF)
}

/// 随机十六进制串（len 位字符）
pub fn random_hex(len: usize) -> String {
    use rand::Rng;
    const HEX: &[u8] = b"0123456789abcdef";
    let mut rng = rand::thread_rng();
    (0..len).map(|_| HEX[rng.gen_range(0..16)] as char).collect()
}

/// 随机字母数字串（len 位字符）
pub fn random_alnum(len: usize) -> String {
    use rand::Rng;
    const POOL: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len).map(|_| POOL[rng.gen_range(0..POOL.len())] as char).collect()
}

// ---------- __puus / __pus Cookie 会话刷新（夸克 / UC 共用，对应 AList） ----------

const TRACKED: [&str; 2] = ["__puus", "__pus"];

/// 从响应 Set-Cookie 合并 __puus/__pus 回原 Cookie 串
pub fn merge_puus(original: &str, set_cookies: &[String]) -> String {
    let mut cookie = original.to_string();
    for sc in set_cookies {
        let kv = sc.split(';').next().unwrap_or("").trim().to_string();
        let (name, value) = match kv.split_once('=') {
            Some((n, v)) if !n.is_empty() => (n.to_string(), v.to_string()),
            _ => continue,
        };
        if TRACKED.contains(&name.as_str()) {
            cookie = set_or_replace(&cookie, &name, &value);
        }
    }
    cookie
}

/// 剥离 __puus（触发服务端重新下发）
pub fn without_puus(cookie: &str) -> String {
    cookie
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.starts_with("__puus="))
        .collect::<Vec<_>>()
        .join("; ")
}

fn set_or_replace(cookie: &str, name: &str, value: &str) -> String {
    let mut parts: Vec<String> = cookie.split(';').map(|s| s.trim().to_string()).collect();
    let kv = format!("{name}={value}");
    match parts.iter().position(|p| p.starts_with(&format!("{name}="))) {
        Some(i) => parts[i] = kv,
        None => parts.push(kv),
    }
    parts.join("; ")
}

/// 解析 JWT payload 为 JSON（无校验）
pub fn jwt_payload(token: &str) -> Option<serde_json::Value> {
    let part = token.split('.').nth(1)?;
    let bytes = b64_decode(part)?;
    serde_json::from_slice(&bytes).ok()
}
