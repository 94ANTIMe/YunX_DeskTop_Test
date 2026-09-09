use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppResult;
use crate::models::{default_color_theme, Settings, UpdateSettingsResult};
use crate::state::AppState;

/// 读取设置（settings.json）
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> AppResult<Settings> {
    app.state::<AppState>().load_settings_checked()
}

/// 判断本次保存是否只改了外观字段（明暗模式 / 配色主题）。
/// 仅外观变化时跳过 aria2 引擎同步，避免主题保存被下载引擎状态干扰。
fn appearance_only(prev: &Settings, next: &Settings) -> bool {
    let mut a = match serde_json::to_value(prev) {
        Ok(Value::Object(map)) => map,
        _ => return false,
    };
    let mut b = match serde_json::to_value(next) {
        Ok(Value::Object(map)) => map,
        _ => return false,
    };
    for key in ["darkMode", "colorTheme"] {
        a.remove(key);
        b.remove(key);
    }
    a == b
}

/// 更新设置：持久化 + 同步 aria2 限速/并发 + 开机自启；仅外观变更时跳过引擎同步。
/// 设置落盘成功即返回 Ok；引擎同步失败不作为错误抛出（settings 已保存，引擎重启后生效），
/// 通过返回值告知前端展示非阻塞提示，避免触发前端的报错回滚流程。
#[tauri::command]
pub async fn update_settings(app: AppHandle, mut settings: Settings) -> AppResult<UpdateSettingsResult> {
    use tauri_plugin_autostart::ManagerExt;

    let state = app.state::<AppState>();
    let prev = state.load_settings();
    // 主题 ID 兜底：空值回写默认主题（未知 ID 由前端回退，不在后端硬编码主题清单）
    if settings.color_theme.trim().is_empty() {
        settings.color_theme = default_color_theme();
    }
    let skip_engine = appearance_only(&prev, &settings);
    state.save_settings(&settings)?;
    let engine_error = if skip_engine {
        None
    } else {
        crate::aria2::apply_settings(&app, &settings).await.err()
    };
    if let Some(e) = &engine_error {
        state.log(
            crate::logger::ERROR,
            "app",
            "apply_settings",
            "设置已保存，但下载引擎同步失败（重启引擎后生效）",
            &e.to_string(),
        );
    }
    // 通知前端设置已变更（导航胶囊「搜索」显隐、剪贴板开关等立即生效）
    let _ = app.emit("settings:updated", &settings);
    // 开机自启状态与操作系统对齐（仅在值变化时写，避免每次保存都触发注册表写入）
    if prev.auto_launch != settings.auto_launch {
        let res = if settings.auto_launch {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(e) = res {
            state.log(crate::logger::ERROR, "app", "autostart", "设置开机自启失败", &e.to_string());
        }
    }
    Ok(UpdateSettingsResult {
        engine_sync_failed: engine_error.is_some(),
        engine_sync_error: engine_error.map(|e| e.to_string()),
    })
}

/// 测试百度网盘第三方加速通道（连通性及解析码校验）
#[tauri::command]
pub async fn test_baidu_speed_service(
    app: AppHandle,
    base_url: Option<String>,
    password: Option<String>,
) -> AppResult<crate::api::baidaccel::AccelCheckResult> {
    let state = app.state::<AppState>();
    let settings = state.load_settings();
    let base = base_url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| crate::api::baidaccel::base_url_of(&settings));
    let pwd = password
        .map(|p| p.trim().to_string())
        .unwrap_or_else(|| settings.baidu_speed_password.trim().to_string());

    let res = crate::api::baidaccel::check_service(&state.http, &base, &pwd).await;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::appearance_only;
    use crate::models::{Settings, UpdateSettingsResult};

    #[test]
    fn appearance_only_changes_skip_engine_sync() {
        let prev = Settings::default();
        let mut next = prev.clone();
        // 只改明暗模式与配色 → 外观变更
        next.dark_mode = 2;
        next.color_theme = "cyber-neon".into();
        assert!(appearance_only(&prev, &next));

        // 改了下载相关字段 → 需要引擎同步
        next.download_speed_limit = 1024;
        assert!(!appearance_only(&prev, &next));

        // 改代理 → 需要引擎同步
        let mut proxy = prev.clone();
        proxy.proxy_enabled = true;
        assert!(!appearance_only(&prev, &proxy));

        // 完全相同（无变化）→ 视为外观保存，跳过引擎同步
        assert!(appearance_only(&prev, &prev));

        // 旧数据兼容：缺 color_theme 字段的 JSON 反序列化回退默认主题
        let legacy: Settings = serde_json::from_str(r#"{"darkMode":1}"#).unwrap();
        assert_eq!(legacy.color_theme, "warm-editorial");
        assert_eq!(legacy.dark_mode, 1);
    }

    #[test]
    fn update_settings_result_camel_case_serialization() {
        let ok = UpdateSettingsResult { engine_sync_failed: false, engine_sync_error: None };
        let v = serde_json::to_value(&ok).unwrap();
        assert!(v.get("engineSyncFailed").is_some());
        assert!(v.get("engine_sync_failed").is_none());

        let bad = UpdateSettingsResult {
            engine_sync_failed: true,
            engine_sync_error: Some("下载引擎通信失败: connection refused".into()),
        };
        let v = serde_json::to_value(&bad).unwrap();
        assert_eq!(v["engineSyncFailed"], true);
        assert_eq!(v["engineSyncError"], "下载引擎通信失败: connection refused");
    }
}
