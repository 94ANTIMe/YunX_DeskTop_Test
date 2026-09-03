use regex::Regex;
use std::sync::OnceLock;

use crate::error::{AppError, AppResult};
use crate::models::{ParsedShare, Platform};

/// 分享链接/文案解析（移植 Android ShareLinkParser）
pub fn parse(text: &str) -> AppResult<ParsedShare> {
    struct Rules {
        url: Regex,
        quark: Regex,
        uc: Regex,
        xunlei: Regex,
        baidu: Regex,
        c139: Regex,
        pan123_main: Regex,
        pan123_sub: Regex,
        pan123_srr: Regex,
        pwd_in_url: Regex,
        pwd_in_text: Regex,
    }
    static RULES: OnceLock<Rules> = OnceLock::new();
    let r = RULES.get_or_init(|| Rules {
        url: Regex::new(r"https?://[^\s]+").unwrap(),
        quark: Regex::new(r"(?i)pan\.quark\.cn/s/([A-Za-z0-9]+)").unwrap(),
        uc: Regex::new(r"(?i)drive\.uc\.cn/s/([A-Za-z0-9]+)").unwrap(),
        xunlei: Regex::new(r"(?i)pan\.xunlei\.com/s/([A-Za-z0-9_-]+)").unwrap(),
        baidu: Regex::new(r"(?i)pan\.baidu\.com/s/(1[A-Za-z0-9_-]+)").unwrap(),
        c139: Regex::new(r"(?i)yun\.139\.com/shareweb/.*?/w/i/([A-Za-z0-9_-]+)").unwrap(),
        pan123_main: Regex::new(r"(?i)123(?:865|pan)\.(?:com|cn)/s/([A-Za-z0-9]+-[A-Za-z0-9]+)").unwrap(),
        pan123_sub: Regex::new(r"(?i)share\.123pan\.cn/123pan/([A-Za-z0-9-]+)").unwrap(),
        pan123_srr: Regex::new(r"api/srr\?sk=([A-Za-z0-9-]+)").unwrap(),
        pwd_in_url: Regex::new(r"[?&]pwd=([A-Za-z0-9]+)").unwrap(),
        pwd_in_text: Regex::new(r"(?:提取码|访问码|密码)[：:]\s*([A-Za-z0-9]{4,8})").unwrap(),
    });

    let trimmed = text.trim();
    // 1. 优先检测磁力链接（Magnet）
    if let Some(pos) = trimmed.to_lowercase().find("magnet:?xt=urn:btih:") {
        let after = &trimmed[pos..];
        let end = after
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>' || c == '`')
            .unwrap_or(after.len());
        let magnet_uri = after[..end]
            .trim_end_matches(['。', '，', ',', '；', ';', ')', ']', '}', '"', '\'', '`', '>', '）'])
            .to_string();
        return Ok(ParsedShare {
            platform: Platform::Magnet.key().to_string(),
            share_id: magnet_uri,
            pwd: String::new(),
        });
    }

    // 2. 检测网络 URL（网盘分享或通用 HTTP 直链）
    let url = r
        .url
        .find(trimmed)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['。', '，', ',', '；', ';', ')', ']', '}', '"', '\'', '`', '>', '）'])
                .to_string()
        })
        .ok_or_else(|| AppError::Unsupported("未识别到可下载链接，请粘贴网盘分享、Magnet 磁力或网络直链".into()))?;

    let pwd = r
        .pwd_in_url
        .captures(&url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            r.pwd_in_text
                .captures(trimmed)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
        .unwrap_or_default();

    let try_match = |re: &Regex| -> Option<String> {
        re.captures(&url).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
    };

    let (platform, share_id) = if let Some(sid) = try_match(&r.quark) {
        (Platform::Quark, sid)
    } else if let Some(sid) = try_match(&r.uc) {
        (Platform::Uc, sid)
    } else if let Some(sid) = try_match(&r.xunlei) {
        (Platform::Xunlei, sid)
    } else if let Some(sid) = try_match(&r.baidu) {
        // 百度 surl 不含开头的 "1"（verify/list 接口用 1 后面的部分）
        (Platform::Baidu, sid.trim_start_matches('1').to_string())
    } else if let Some(sid) = try_match(&r.c139) {
        (Platform::C139, sid)
    } else if let Some(sid) = try_match(&r.pan123_main) {
        (Platform::Pan123, sid)
    } else if let Some(sid) = try_match(&r.pan123_sub) {
        (Platform::Pan123, sid)
    } else if let Some(sid) = try_match(&r.pan123_srr) {
        (Platform::Pan123, sid)
    } else {
        // 普通 HTTP/HTTPS 直链下载（通用下载）
        (Platform::Direct, url.clone())
    };

    Ok(ParsedShare {
        platform: platform.key().to_string(),
        share_id,
        pwd,
    })
}
