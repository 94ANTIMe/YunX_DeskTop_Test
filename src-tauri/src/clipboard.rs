//! 剪贴板监听：轮询读取系统剪贴板，命中夸克/UC/百度等网盘分享链接时向前端发射
//! `clipboard:share-detected` 事件，由前端弹窗提示「去解析」。
//! 由设置 `clipboard_monitor` 控制开关；同一链接按「平台:share_id」指纹去重。
use tauri::{AppHandle, Emitter, Manager};

/// 指纹去重 + 轮询主循环（setup 时调用一次）
pub async fn spawn(app: AppHandle) {
    let mut last_fingerprint = String::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let state = app.state::<crate::state::AppState>();
        // 开关关闭 → 静默跳过（不读剪贴板，避免无谓开销）
        if !state.load_settings().clipboard_monitor {
            continue;
        }
        let text: Option<String> = tokio::task::spawn_blocking(|| {
            let mut cb = arboard::Clipboard::new().ok()?;
            cb.get_text().ok()
        })
        .await
        .unwrap_or(None);
        let Some(text) = text else { continue };
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let Ok(parsed) = crate::parser::parse(&text) else { continue };
        if parsed.share_id.is_empty() {
            continue;
        }
        let fingerprint = format!("{}:{}", parsed.platform, parsed.share_id);
        if fingerprint == last_fingerprint {
            continue;
        }
        last_fingerprint = fingerprint;
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let _ = app.emit(
            "clipboard:share-detected",
            serde_json::json!({ "text": text, "parsed": parsed, "at": at }),
        );
    }
}