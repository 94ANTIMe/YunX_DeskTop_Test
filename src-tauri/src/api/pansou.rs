//! PanSou 网盘搜索 API（对接自部署 fish2018/pansou 服务）。
//! GET /api/search?kw=...&res=merge → merged_by_type 按网盘类型分组的结果；
//! 每个条目的 url + password 可直接交给云析的分享解析流程取直链下载。
use serde_json::Value;

use super::http_client;
use crate::error::{AppError, AppResult};
use crate::models::SearchItem;

/// 搜索网盘资源。
/// @param base 自部署 PanSou 服务根地址（如 http://192.168.1.100:8888）
/// @param kw 搜索关键词
/// @param cloud_types 限定返回的网盘类型（如 ["quark","baidu"]）；None = 全部
pub async fn search(base: &str, kw: &str, cloud_types: Option<&[String]>) -> AppResult<Vec<SearchItem>> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(AppError::Api("未配置 PanSou 搜索服务地址".into()));
    }
    let kw = kw.trim();
    if kw.is_empty() {
        return Err(AppError::Api("请输入搜索关键词".into()));
    }

    // 查询参数：kw + res=merge（按类型合并）+ 可选 cloud_types
    let mut url = format!("{base}/api/search?kw={}&res=merge", url_encode(kw));
    if let Some(types) = cloud_types {
        let joined = types
            .iter()
            .map(|t| url_encode(t.trim()))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        if !joined.is_empty() {
            url.push_str(&format!("&cloud_types={joined}"));
        }
    }

    let resp = http_client()
        .get(&url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| AppError::Api(format!("搜索服务通信失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Api(format!("搜索服务返回 {}", resp.status())));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| AppError::Api(format!("搜索服务响应解析失败: {e}")))?;
    Ok(parse_merged(&body))
}

/// 解析 merged_by_type 响应（兼容根级或 data 包裹两种布局）
fn parse_merged(root: &Value) -> Vec<SearchItem> {
    let empty = serde_json::json!({});
    let merged = root
        .get("merged_by_type")
        .or_else(|| root.get("data").and_then(|d| d.get("merged_by_type")))
        .unwrap_or(&empty);
    let Some(groups) = merged.as_object() else {
        return Vec::new();
    };

    let mut items: Vec<SearchItem> = Vec::new();
    for (ptype, arr) in groups {
        let Some(arr) = arr.as_array() else { continue };
        for entry in arr {
            let s = |key: &str| {
                entry
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let url = s("url");
            if url.is_empty() {
                continue;
            }
            // note 兜底 title（不同版本字段名）
            let note = if entry.get("note").and_then(|v| v.as_str()).is_some() {
                s("note")
            } else {
                s("title")
            };
            items.push(SearchItem {
                r#type: ptype.clone(),
                url,
                password: s("password"),
                note,
                source: s("source"),
            });
        }
    }
    // 同一份资源可能来自多个来源，按 (type, url, note) 去重
    let mut seen = std::collections::HashSet::new();
    items.retain(|it| seen.insert((it.r#type.clone(), it.url.clone(), it.note.clone())));
    items
}

/// URL 查询参数编码（application/x-www-form-urlencoded 风格）
fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_merged_handles_root_and_data_layout() {
        let root = serde_json::json!({
            "code": 200,
            "merged_by_type": {
                "quark": [
                    {"url": "https://pan.quark.cn/s/a", "password": "1a2b", "note": "资源A", "source": "tg:xx"}
                ]
            }
        });
        let items = parse_merged(&root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].r#type, "quark");
        assert_eq!(items[0].password, "1a2b");

        let wrapped = serde_json::json!({
            "code": 200,
            "data": {
                "merged_by_type": {
                    "baidu": [
                        {"url": "https://pan.baidu.com/s/b", "note": "资源B", "source": "tg:yy"},
                        {"url": "https://pan.baidu.com/s/b", "note": "资源B", "source": "plugin:zz"}
                    ]
                }
            }
        });
        let items = parse_merged(&wrapped);
        assert_eq!(items.len(), 1); // 去重后只剩一条
    }

    #[test]
    fn encode_special_chars() {
        assert_eq!(url_encode("a b/中"), "a%20b%2F%E4%B8%AD");
    }
}
