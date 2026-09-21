use serde::{Deserialize, Serialize};
/// 应用设置（app_data_dir/settings.json 持久化；键语义对齐 Android SettingsRepository）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// 自定义下载保存目录（空 = 系统下载文件夹）
    pub download_dir: String,
    /// 最大同时下载任务数，默认 1
    pub max_concurrent_downloads: i32,
    /// 分片并发数（aria2 split），默认 32，上限 512
    pub download_threads: i32,
    /// 全局限速（字节/秒；0 = 不限速）
    pub download_speed_limit: i64,
    /// 失败重试次数，默认 3
    pub download_retry_count: i32,
    /// 分片最小体积（aria2 min-split-size，单位 MB），默认 4，范围 1-64
    pub download_min_split_mb: i32,
    /// 单服务器最大连接数（aria2 max-connection-per-server），默认 16，上限 16
    pub download_conn_per_server: i32,
    /// PanSou 自部署搜索服务地址（如 http://192.168.1.100:8888）；空 = 未配置
    pub pansou_base_url: String,
    /// 百度网盘加速通道（百度分享高速下载）：开关
    /// 兼容旧版字段名 shanlianEnabled → baiduSpeedEnabled
    #[serde(alias = "shanlianEnabled")]
    pub baidu_speed_enabled: bool,
    /// 加速通道服务地址；空 = 默认服务（兼容旧字段名 shanlianBaseUrl）
    #[serde(alias = "shanlianBaseUrl")]
    pub baidu_speed_base_url: String,
    /// 加速通道解析码（失效时自动更新；兼容旧字段名 shanlianPassword）
    #[serde(alias = "shanlianPassword")]
    pub baidu_speed_password: String,
    /// 深色模式：0 跟随系统 / 1 浅色 / 2 深色
    pub dark_mode: i32,
    /// 配色主题 ID（前端 src/lib/themes.ts 注册表；未知 ID 前端回退 warm-editorial）
    #[serde(default = "default_color_theme")]
    pub color_theme: String,
    /// 启动时自动检查在线更新（GitHub Releases）
    pub auto_check_update: bool,
    /// 剪贴板监听：复制分享链接自动提示解析（默认关）
    pub clipboard_monitor: bool,
    /// 最小化到系统托盘（常驻后台，默认开）
    pub minimize_to_tray: bool,
    /// 下载完成/失败系统通知（默认开）
    pub download_notify: bool,
    /// 开机自启（默认关）
    pub auto_launch: bool,
    /// 导航胶囊显示「搜索」Tab（默认关）
    pub show_search_tab: bool,
    /// 代理开关（默认关；aria2 全局限速走 --all-proxy）
    pub proxy_enabled: bool,
    /// 代理协议："http" | "socks5"
    pub proxy_type: String,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_username: String,
    /// 代理密码（落盘经 DPAPI 加密）
    pub proxy_password: String,
    /// 平台当前选中账号（platform → 账号 key；缺省回退平台 key 的旧行）
    pub active_account_keys: std::collections::BTreeMap<String, String>,
    /// 首启引导已完成
    pub onboarded: bool,
    /// 订阅追剧自动下载总开关（默认开；PanSou 未配置时实际不生效）
    pub subscription_enabled: bool,
    /// 订阅检查间隔（分钟，默认 360）
    pub subscription_interval_minutes: i64,
    /// BT Tracker 列表自动更新（XIU2/TrackersListCollection 每日列表，默认开）
    pub bt_tracker_auto_update: bool,
    /// 下载完成后动作："none" | "shutdown"（60 秒后关机）| "sleep"（睡眠）
    pub after_download_action: String,
}

/// 配色主题默认值（与前端 DEFAULT_COLOR_THEME 对齐）
pub fn default_color_theme() -> String {
    "warm-editorial".into()
}

/// update_settings 返回：设置必定已保存；engine_sync_* 仅提示下载引擎同步是否失败（非阻塞）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsResult {
    pub engine_sync_failed: bool,
    pub engine_sync_error: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            max_concurrent_downloads: 1,
            download_threads: 32,
            download_speed_limit: 0,
            download_retry_count: 3,
            download_min_split_mb: 4,
            download_conn_per_server: 16,
            pansou_base_url: String::new(),
            baidu_speed_enabled: false,
            baidu_speed_base_url: String::new(),
            baidu_speed_password: String::new(),
            dark_mode: 0,
            color_theme: default_color_theme(),
            auto_check_update: true,
            clipboard_monitor: false,
            minimize_to_tray: true,
            download_notify: true,
            auto_launch: false,
            show_search_tab: false,
            proxy_enabled: false,
            proxy_type: "http".into(),
            proxy_host: String::new(),
            proxy_port: 0,
            proxy_username: String::new(),
            proxy_password: String::new(),
            active_account_keys: std::collections::BTreeMap::new(),
            onboarded: false,
            subscription_enabled: true,
            subscription_interval_minutes: 360,
            bt_tracker_auto_update: true,
            after_download_action: "none".into(),
        }
    }
}
