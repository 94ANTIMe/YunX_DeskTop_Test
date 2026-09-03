import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------- 类型（与 Rust models.rs 对齐） ----------

export interface AppInfo {
  appName: string;
  version: string;
  platform: string;
}

export interface Settings {
  downloadDir: string;
  maxConcurrentDownloads: number;
  downloadThreads: number;
  downloadSpeedLimit: number;
  downloadRetryCount: number;
  downloadMinSplitMb: number;
  downloadConnPerServer: number;
  pansouBaseUrl: string;
  /** 百度网盘加速通道（百度分享高速下载）：开关 */
  baiduSpeedEnabled: boolean;
  /** 加速通道服务地址；空 = 默认服务 */
  baiduSpeedBaseUrl: string;
  /** 加速通道解析码（失效时自动更新） */
  baiduSpeedPassword: string;
  darkMode: number;
  autoCheckUpdate: boolean;
  clipboardMonitor: boolean;
  minimizeToTray: boolean;
  downloadNotify: boolean;
  autoLaunch: boolean;
  showSearchTab: boolean;
  /** 代理开关（aria2 全局限速走 --all-proxy） */
  proxyEnabled: boolean;
  proxyType: string;
  proxyHost: string;
  proxyPort: number;
  proxyUsername: string;
  proxyPassword: string;
  /** 平台当前选中账号（platform → 账号 key） */
  activeAccountKeys: Record<string, string>;
  /** 首启引导已完成 */
  onboarded: boolean;
}

/** 设置默认值（与 Rust Settings::default 对齐；IPC 不可用时兜底） */
export const DEFAULT_SETTINGS: Settings = {
  downloadDir: "",
  maxConcurrentDownloads: 1,
  downloadThreads: 32,
  downloadSpeedLimit: 0,
  downloadRetryCount: 3,
  downloadMinSplitMb: 4,
  downloadConnPerServer: 16,
  pansouBaseUrl: "",
  baiduSpeedEnabled: false,
  baiduSpeedBaseUrl: "",
  baiduSpeedPassword: "",
  darkMode: 0,
  autoCheckUpdate: true,
  clipboardMonitor: false,
  minimizeToTray: true,
  downloadNotify: true,
  autoLaunch: false,
  showSearchTab: false,
  proxyEnabled: false,
  proxyType: "http",
  proxyHost: "",
  proxyPort: 0,
  proxyUsername: "",
  proxyPassword: "",
  activeAccountKeys: {},
  onboarded: false,
};

/** PanSou 搜索结果条目（与 Rust SearchItem 对齐） */
export interface SearchItem {
  type: string;
  url: string;
  password: string;
  note: string;
  source: string;
}

export interface AccountSummary {
  platform: string;
  nickname: string | null;
  loggedIn: boolean;
}

/** 平台账号行（多账号切换列表；active = 当前选中） */
export interface AccountRow {
  platform: string;
  key: string;
  nickname: string;
  updatedAt: number;
  active: boolean;
}

/** 代理连通性测试结果（设置页「测试代理」展示） */
export interface ProxyTestResult {
  ok: boolean;
  ip: string;
  latencyMs: number;
  error: string;
}

/** PanSou 连通性检测结果（首启引导 / 设置页） */
export interface PansouPingResult {
  ok: boolean;
  latencyMs: number;
  error: string;
}

export interface ParsedShare {
  platform: string;
  shareId: string;
  pwd: string;
}

export interface ShareFile {
  fid: string;
  fname: string;
  fsize: number;
  isdir: boolean;
  pdirFid: string;
  fidToken: string;
  modifyTime: string;
}

export interface ResolveSessionInfo {
  sessionKey: string;
  platform: string;
  title: string;
  files: ShareFile[];
  hasMore: boolean;
}

export interface ShareFilePage {
  files: ShareFile[];
  hasMore: boolean;
}

/** 文件夹收集结果（携带相对目录，还原文件夹结构保存） */
export interface CollectedFile {
  fid: string;
  fname: string;
  fsize: number;
  isdir: boolean;
  pdirFid: string;
  fidToken: string;
  modifyTime: string;
  /** 相对目录路径（含子目录名；根级文件为空） */
  relDir: string;
}

export interface DownloadLink {
  url: string;
  filename: string;
  size: number;
  headers: [string, string][];
  platform: string;
  cleanupId: string;
}

export interface DownloadTask {
  id: number;
  gid: string;
  url: string;
  fileName: string;
  platform: string;
  totalSize: number;
  downloadedSize: number;
  speed: number;
  status: number;
  errorMsg: string;
  savePath: string;
  createTime: number;
}

/** 下载任务 Dashboard 详情（对齐 Rust DownloadDetail） */
export interface DownloadDetail {
  id: number;
  gid: string;
  url: string;
  fileName: string;
  platform: string;
  totalSize: number;
  downloadedSize: number;
  speed: number;
  status: number;
  errorMsg: string;
  savePath: string;
  createTime: number;
  connections: number;
  uploadSpeed: number;
  totalTime: number;
}

export interface Bookmark {
  id: number;
  link: string;
  title: string;
  platform: string;
  pwd: string;
  category: string;
  createTime: number;
}

export interface ResolveHistory {
  id: number;
  link: string;
  title: string;
  platform: string;
  createTime: number;
}

export interface XunleiLoginStep {
  needSms: boolean;
  creditKey: string;
  smsToken: string;
  sessionId: string;
  nickname: string;
  reviewUrl: string;
  message: string;
}

export interface LogRow {
  id: number;
  time: number;
  level: string;
  platform: string;
  action: string;
  message: string;
  detail: string;
}

export interface LoginSuccessEvent {
  platform: string;
  nickname: string;
}

/** 剪贴板命中分享链接事件（对齐 Rust clipboard 模块发射） */
export interface ClipboardShareEvent {
  text: string;
  parsed: { platform: string; shareId: string; pwd: string };
  at: number;
}

/** 在线更新检查结果（对齐 Rust UpdateInfo） */
export interface UpdateInfo {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  name: string;
  notes: string;
  downloadUrl: string;
  browserDownloadUrl: string;
}

/** 安装包下载进度（`update:progress` 事件） */
export interface UpdateProgress {
  received: number;
  total: number;
}

// ---------- 错误规范 ----------

export interface AppError {
  code: string;
  message: string;
}

export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  const obj = e as AppError | null;
  if (obj && typeof obj === "object" && "message" in obj) return String(obj.message);
  return "未知错误";
}

// ---------- 命令封装 ----------

export const ipc = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<void>("update_settings", { settings }),

  listAccounts: () => invoke<AccountSummary[]>("list_accounts"),
  /** 平台账号列表（多账号切换下拉） */
  listAccountRows: (platform: string) => invoke<AccountRow[]>("list_account_rows", { platform }),
  /** 切换平台当前选中账号 */
  switchAccount: (platform: string, key: string) =>
    invoke<void>("switch_account", { platform, key }),
  logout: (platform: string, key?: string) => invoke<void>("logout", { platform, key }),
  /** 代理连通性测试（真实出口 IP 探测） */
  testProxy: () => invoke<ProxyTestResult>("test_proxy"),
  /** PanSou 服务连通性检测 */
  pansouPing: (baseUrl: string) => invoke<PansouPingResult>("pansou_ping", { baseUrl }),
  webLoginStart: (platform: string) => invoke<void>("web_login_start", { platform }),
  webLoginCancel: (platform: string) => invoke<void>("web_login_cancel", { platform }),
  xunleiLogin: (username: string, password: string) =>
    invoke<XunleiLoginStep>("xunlei_login", { username, password }),
  xunleiSmsLogin: (username: string, smsCode: string, creditKey: string, smsToken: string) =>
    invoke<XunleiLoginStep>("xunlei_sms_login", { username, smsCode, creditKey, smsToken }),
  pan123Login: (account: string, password: string) =>
    invoke<string>("pan123_login", { account, password }),

  resolveShare: (text: string) => invoke<ResolveSessionInfo>("resolve_share", { text }),

  pansouSearch: (kw: string, cloudTypes?: string[]) =>
    invoke<SearchItem[]>("pansou_search", { kw, cloudTypes }),
  listShareFiles: (sessionKey: string, dirId: string, page?: number) =>
    invoke<ShareFilePage>("list_share_files", { sessionKey, dirId, page }),
  collectFolderFiles: (sessionKey: string, dirId: string) =>
    invoke<CollectedFile[]>("collect_folder_files", { sessionKey, dirId }),
  getDownloadLink: (sessionKey: string, file: ShareFile) =>
    invoke<DownloadLink>("get_download_link", { sessionKey, file }),

  enqueueDownload: (url: string, fileName: string, headers: [string, string][], platform: string, cleanupId?: string) =>
    invoke<number>("enqueue_download", { url, fileName, headers, platform, cleanupId }),
  pauseDownload: (id: number) => invoke<void>("pause_download", { id }),
  resumeDownload: (id: number) => invoke<void>("resume_download", { id }),
  pauseAllDownloads: () => invoke<void>("pause_all_downloads"),
  resumeAllDownloads: () => invoke<void>("resume_all_downloads"),
  removeDownloadTask: (id: number, deleteLocal: boolean) =>
    invoke<void>("remove_download_task", { id, deleteLocal }),
  listDownloadTasks: () => invoke<DownloadTask[]>("list_download_tasks"),
  clearDownloadTasks: () => invoke<void>("clear_download_tasks"),
  getDownloadDetail: (id: number) => invoke<DownloadDetail>("download_detail", { id }),

  listBookmarks: () => invoke<Bookmark[]>("list_bookmarks"),
  addBookmark: (link: string, title: string, pwd: string) =>
    invoke<number>("add_bookmark", { link, title, pwd }),
  removeBookmark: (id: number) => invoke<void>("remove_bookmark", { id }),

  listResolveHistory: () => invoke<ResolveHistory[]>("list_resolve_history"),
  deleteResolveHistory: (id: number) => invoke<void>("delete_resolve_history", { id }),
  clearResolveHistory: () => invoke<void>("clear_resolve_history"),

  listLogs: (level?: string, limit?: number) =>
    invoke<LogRow[]>("list_logs", { level, limit }),
  clearLogs: () => invoke<void>("clear_logs"),

  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  downloadUpdate: () => invoke<string>("download_update"),
  installUpdate: (path: string) => invoke<void>("install_update", { path }),
};

// ---------- 事件订阅 ----------

export function onDownloadsUpdated(handler: (tasks: DownloadTask[]) => void): Promise<UnlistenFn> {
  return listen<DownloadTask[]>("downloads:updated", (e) => handler(e.payload));
}

export function onLoginSuccess(handler: (e: LoginSuccessEvent) => void): Promise<UnlistenFn> {
  return listen<LoginSuccessEvent>("login:success", (e) => handler(e.payload));
}

export function onUpdateProgress(handler: (p: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>("update:progress", (e) => handler(e.payload));
}

export function onClipboardShare(
  handler: (e: ClipboardShareEvent) => void
): Promise<UnlistenFn> {
  return listen<ClipboardShareEvent>("clipboard:share-detected", (e) => handler(e.payload));
}

/** 设置保存后通知（导航胶囊「搜索」显隐等即时生效） */
export function onSettingsUpdated(handler: (s: Settings) => void): Promise<UnlistenFn> {
  return listen<Settings>("settings:updated", (e) => handler(e.payload));
}
