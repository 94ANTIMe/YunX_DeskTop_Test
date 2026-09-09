import { useEffect, useRef, useState } from "react";
import { open as openDialogDir } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Bell, Check, ChevronDown, ClipboardPaste, Download, ExternalLink, FolderOpen, Globe, Loader2, Magnet, Minimize2, Power, RefreshCw, Rss, Search, ShieldCheck, Wifi } from "lucide-react";
import PageHeader from "../components/PageHeader";
import { errMsg, ipc, onSettingsUpdated, DEFAULT_SETTINGS, type AppInfo, type Settings as SettingsT } from "../lib/ipc";
import { useUpdate } from "../hooks/useUpdate";
import { formatBytes } from "../lib/format";
import type { ThemeMode } from "../hooks/useTheme";
import type { AppearancePatch } from "../hooks/useAppearance";
import { COLOR_THEMES, type ColorTheme } from "../lib/themes";
import type { TabId } from "../lib/tabs";
import aboutHero from "../assets/art/about-lighthouse.jpg";

const GITHUB_URL = "https://github.com/94ANTIMe/YunX_DeskTop_Test";

/** 开关行（通用设置区块复用） */
function ToggleRow({
  icon: Icon,
  title,
  desc,
  checked,
  onChange,
}: {
  icon: typeof Bell;
  title: string;
  desc: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex min-w-0 items-start gap-3">
        <Icon size={16} className="mt-0.5 shrink-0 text-clay" strokeWidth={1.8} />
        <div className="min-w-0">
          <p className="text-sm text-ink-soft">{title}</p>
          <p className="mt-0.5 text-xs text-ink-soft/70">{desc}</p>
        </div>
      </div>
      <button
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
          checked ? "bg-clay" : "bg-ink/15"
        }`}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
            checked ? "left-[22px]" : "left-0.5"
          }`}
        />
      </button>
    </div>
  );
}

/** 本项目所依赖 / 参考的开源项目（设置页「开源致谢」区块） */
const ACKNOWLEDGEMENTS = [
  {
    name: "aria2",
    role: "多线程高速下载引擎（sidecar）",
    url: "https://github.com/aria2/aria2",
  },
  {
    name: "BaiduPCS-Go",
    role: "百度网盘直链取链 / 分享转存参考（sidecar 集成）",
    url: "https://github.com/qjfoidnh/BaiduPCS-Go",
  },
  {
    name: "PanSou",
    role: "网盘聚合搜索 API 服务（自部署对接）",
    url: "https://github.com/fish2018/pansou",
  },
  {
    name: "TrackersListCollection",
    role: "BT Tracker 每日更新列表数据源（自动追更注入 aria2）",
    url: "https://github.com/XIU2/TrackersListCollection",
  },
  {
    name: "TurboDL",
    role: "多线程分片下载优化参考",
    url: "https://github.com/henrique-coder/turbodl",
  },
  {
    name: "YunX",
    role: "云析 Android 版（同源项目）",
    url: "https://github.com/CYQawa/YunX",
  },
];

const THEME_OPTIONS: { mode: ThemeMode; label: string }[] = [
  { mode: "system", label: "跟随系统" },
  { mode: "light", label: "浅色" },
  { mode: "dark", label: "深色" },
];

/** 限速选项（字节/秒） */
const SPEED_OPTIONS: { value: number; label: string }[] = [
  { value: 0, label: "不限速" },
  { value: 1_048_576, label: "1 MB/s" },
  { value: 5_242_880, label: "5 MB/s" },
  { value: 10_485_760, label: "10 MB/s" },
  { value: 52_428_800, label: "50 MB/s" },
];

/** 订阅检查间隔选项（分钟） */
const SUB_INTERVAL_OPTIONS: { value: number; label: string }[] = [
  { value: 30, label: "30 分钟" },
  { value: 60, label: "1 小时" },
  { value: 180, label: "3 小时" },
  { value: 360, label: "6 小时" },
  { value: 720, label: "12 小时" },
  { value: 1440, label: "24 小时" },
];

/** 下载完成后动作选项 */
const AFTER_ACTION_OPTIONS: { value: string; label: string }[] = [
  { value: "none", label: "无（保持运行）" },
  { value: "shutdown", label: "60 秒后关机" },
  { value: "sleep", label: "进入睡眠" },
];

interface SettingsPageProps {
  themeMode: ThemeMode;
  /** 当前配色主题 ID（由 App 持有并注入 documentElement） */
  colorTheme: string;
  /** 外观变更（明暗 / 配色）：App 即时预览并经共享队列持久化到 settings.json */
  onAppearanceChange: (patch: AppearancePatch) => void;
  /** 跳转主 Tab（如「前往搜索」） */
  onNavigate?: (tab: TabId) => void;
}

/**
 * 主题预览卡（七选一，radio 语义 + 方向键移动）。
 * 迷你界面与双色样本直接用该主题自身 token 渲染（始终浅色基准），不随当前页面主题变化。
 */
function ThemeCard({
  theme,
  selected,
  onSelect,
}: {
  theme: ColorTheme;
  selected: boolean;
  onSelect: () => void;
}) {
  const t = theme.light;
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      aria-label={theme.name}
      tabIndex={selected ? 0 : -1}
      onClick={onSelect}
      className={`relative rounded-card border p-3 text-left transition-colors ${
        selected
          ? "border-clay bg-accent-soft"
          : "border-ink/10 bg-carrier-deep hover:border-ink/25"
      }`}
    >
      {/* 迷你界面预览 */}
      <span
        aria-hidden
        className="block h-14 w-full overflow-hidden rounded-ctrl border p-1.5"
        style={{ backgroundColor: t["app-bg"], borderColor: t["border-soft"] }}
      >
        <span className="flex items-center gap-1">
          <span className="h-2 w-6 rounded-full" style={{ backgroundColor: t.accent }} />
          <span className="h-2 w-4 rounded-full" style={{ backgroundColor: t["border-strong"] }} />
        </span>
        <span className="mt-1.5 flex items-center gap-1">
          <span className="h-4 w-9 rounded-[3px]" style={{ backgroundColor: t.accent }} />
          <span className="h-4 w-9 rounded-[3px]" style={{ backgroundColor: t["accent-soft"] }} />
          <span className="ml-auto h-4 w-4 rounded-[3px]" style={{ backgroundColor: t["accent-decor"] }} />
        </span>
        <span className="mt-1.5 block h-1.5 w-2/3 rounded-full" style={{ backgroundColor: t["text-secondary"], opacity: 0.55 }} />
      </span>
      {/* 双色样本（原始色值）+ 名称 + 选中标记 */}
      <span className="mt-2.5 flex items-center gap-2">
        <span className="flex shrink-0 -space-x-1">
          <span className="h-4 w-4 rounded-full border-2 border-ivory" style={{ backgroundColor: theme.colorA }} />
          <span className="h-4 w-4 rounded-full border-2 border-ivory" style={{ backgroundColor: theme.colorB }} />
        </span>
        <span className={`min-w-0 flex-1 truncate text-xs font-medium ${selected ? "text-ink" : "text-ink-soft"}`}>
          {theme.name}
        </span>
        {selected && <Check size={14} className="shrink-0 text-clay" strokeWidth={2.4} aria-hidden />}
      </span>
    </button>
  );
}

/** 设置页：外观（明暗 × 配色）/ 下载（settings.json 持久化）/ 搜索服务 / 关于 */
export default function SettingsPage({ themeMode, colorTheme, onAppearanceChange, onNavigate }: SettingsPageProps) {
  const [settings, setSettings] = useState<SettingsT | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [saving, setSaving] = useState(false);
  const [loadError, setLoadError] = useState("");
  const settingsRef = useRef<SettingsT | null>(null);
  const saveInFlight = useRef(false);
  const savePending = useRef(false);
  const [proxyTesting, setProxyTesting] = useState(false);
  const [proxyResult, setProxyResult] = useState<{ ok: boolean; ip?: string; latencyMs?: number; error?: string } | null>(null);
  const updater = useUpdate();
  // 开源致谢折叠状态：仅保留于当前应用会话（重启后收起；切换设置页不卸载故状态保留）
  const [ackOpen, setAckOpen] = useState(false);

  // 初始加载失败时保留错误，由恢复页显式重试。
  async function loadSettings() {
    setLoadError("");
    try {
      const loaded = { ...DEFAULT_SETTINGS, ...(await ipc.getSettings()) };
      settingsRef.current = loaded;
      setSettings(loaded);
    } catch (cause) {
      setLoadError(errMsg(cause));
      setSettings(null);
    }
  }

  useEffect(() => {
    void loadSettings();
    ipc.getAppInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  async function flushSettings() {
    if (saveInFlight.current) return;
    saveInFlight.current = true;
    setSaving(true);
    while (savePending.current && settingsRef.current) {
      savePending.current = false;
      const snapshot = { ...settingsRef.current };
      try {
        const result = await ipc.updateSettings(snapshot);
        // 设置已保存；仅当下载引擎同步失败时给非阻塞提示（重启引擎后生效），不报错、不回滚
        if (result?.engineSyncFailed) {
          setNotice(`设置已保存；下载引擎暂未同步（${result.engineSyncError || "引擎未响应"}）`);
          window.setTimeout(() => setNotice(""), 6000);
        }
      } catch (cause) {
        setError(`${errMsg(cause)}；已重新读取后端设置`);
        savePending.current = false;
        try {
          const restored = { ...DEFAULT_SETTINGS, ...(await ipc.getSettings()) };
          settingsRef.current = restored;
          setSettings(restored);
        } catch (reloadCause) {
          setLoadError(errMsg(reloadCause));
          setSettings(null);
        }
      }
    }
    saveInFlight.current = false;
    setSaving(false);
    // 队列清空后与后端对账一次（吸收顶部明暗切换等本页之外的外观保存）
    try {
      const fresh = { ...DEFAULT_SETTINGS, ...(await ipc.getSettings()) };
      if (!saveInFlight.current) {
        settingsRef.current = fresh;
        setSettings((prev) => (prev ? fresh : prev));
      }
    } catch {
      // 后端不可达：保留现状
    }
  }

  // 将本次渲染发生变化的字段合并到最新 ref，连续操作不会被旧快照覆盖。
  async function persist(next: SettingsT) {
    const rendered = settings ?? next;
    const patch = Object.fromEntries(
      (Object.keys(next) as (keyof SettingsT)[])
        .filter((key) => !Object.is(next[key], rendered[key]))
        .map((key) => [key, next[key]]),
    ) as Partial<SettingsT>;
    const merged = { ...(settingsRef.current ?? rendered), ...patch } as SettingsT;
    settingsRef.current = merged;
    setSettings(merged);
    setError("");
    savePending.current = true;
    await flushSettings();
  }

  // 保存队列清空后与后端回读值对账：吸收顶部明暗切换等其他来源的外观保存，
  // 避免本页后续保存用陈旧快照覆盖别的字段（对账失败静默，等待下次保存重试）。
  useEffect(() => {
    const un = onSettingsUpdated((s) => {
      if (!saveInFlight.current) {
        settingsRef.current = { ...DEFAULT_SETTINGS, ...s };
        setSettings((prev) => (prev ? { ...prev, ...s } : prev));
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  function setTheme(mode: ThemeMode) {
    onAppearanceChange({ mode });
  }

  // 配色主题卡方向键导航（radio 语义：左右 / 上下移动选择）
  const themeGridRef = useRef<HTMLDivElement>(null);
  function onThemeGridKeyDown(e: React.KeyboardEvent) {
    if (!["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"].includes(e.key)) return;
    e.preventDefault();
    const delta = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : -1;
    const current = COLOR_THEMES.findIndex((t) => t.id === colorTheme);
    const next = COLOR_THEMES[(current + delta + COLOR_THEMES.length) % COLOR_THEMES.length];
    onAppearanceChange({ colorTheme: next.id });
    const buttons = themeGridRef.current?.querySelectorAll<HTMLButtonElement>('[role="radio"]');
    buttons?.[COLOR_THEMES.findIndex((t) => t.id === next.id)]?.focus();
  }

  async function pickDownloadDir() {
    if (!settings) return;
    try {
      const selected = await openDialogDir({ directory: true });
      const dir = typeof selected === "string" ? selected : selected?.[0];
      if (dir) {
        await persist({ ...settings, downloadDir: dir });
        setNotice("下载目录已更新（新任务生效）");
        window.setTimeout(() => setNotice(""), 3500);
      }
    } catch (e) {
      setError(errMsg(e));
    }
  }

  // 保存 PanSou 搜索服务地址（本地草稿 → 失焦/按钮统一保存）
  async function savePansouUrl(url: string) {
    if (!settings) return;
    const trimmed = url.trim().replace(/\/+$/, "");
    await persist({ ...settings, pansouBaseUrl: trimmed });
    setNotice(trimmed ? "搜索服务地址已保存" : "已清除搜索服务地址");
    window.setTimeout(() => setNotice(""), 3500);
  }

  function goSearchTab() {
    onNavigate?.("search");
  }

  const s = settings;

  return (
    <div className="space-y-6">
      <PageHeader tab="settings" subtitle="外观、下载与关于" />

      {error && (
        <div className="rounded-ctrl bg-danger/10 px-4 py-2.5 text-sm text-danger">{error}</div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-success/10 px-4 py-2.5 text-sm text-ink">{notice}</div>
      )}
      {loadError && !s && (
        <section className="rounded-card bg-carrier p-6">
          <h3 className="text-sm font-semibold text-danger">设置读取失败</h3>
          <p className="mt-2 text-sm text-ink-soft">{loadError}</p>
          <button onClick={() => void loadSettings()} className="mt-4 rounded-ctrl bg-clay px-4 py-2 text-sm font-medium text-on-accent">重试</button>
        </section>
      )}

      {/* 外观：明暗模式 × 配色主题（两者独立选择；即时预览，经共享队列持久化） */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <h3 className="text-sm font-semibold text-ink">外观</h3>

        {/* 明暗模式（保留 darkMode 原数值含义：0 跟随系统 / 1 浅色 / 2 深色） */}
        <div className="mt-4 flex gap-1.5">
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.mode}
              onClick={() => setTheme(opt.mode)}
              aria-pressed={themeMode === opt.mode}
              className={`rounded-ctrl px-4 py-1.5 text-sm font-medium transition-colors ${
                themeMode === opt.mode
                  ? "bg-clay text-on-accent"
                  : "bg-carrier-deep text-ink-soft hover:text-ink"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>

        {/* 配色主题（与明暗独立；设置持久化于 settings.colorTheme） */}
        <div
          ref={themeGridRef}
          role="radiogroup"
          aria-label="配色主题"
          onKeyDown={onThemeGridKeyDown}
          className="mt-4 grid grid-cols-2 gap-2.5 sm:grid-cols-4 lg:grid-cols-7"
        >
          {COLOR_THEMES.map((t) => (
            <ThemeCard
              key={t.id}
              theme={t}
              selected={t.id === colorTheme}
              onSelect={() => onAppearanceChange({ colorTheme: t.id })}
            />
          ))}
        </div>
        <p className="mt-3 text-xs text-ink-soft/70">
          配色与明暗相互独立；跟随系统时仅明暗随系统变化，所选配色始终保留。
        </p>
      </section>

      {/* 通用 */}
      {s && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "90ms" }}>
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-ink">通用</h3>
            {saving && <Loader2 size={14} className="animate-spin text-clay" />}
          </div>
          <div className="mt-2 divide-y divide-ink/10">
            <ToggleRow
              icon={ClipboardPaste}
              title="剪贴板监听"
              desc="复制夸克 / UC / 百度等分享链接时自动提示解析"
              checked={s.clipboardMonitor}
              onChange={(v) => persist({ ...s, clipboardMonitor: v })}
            />
            <ToggleRow
              icon={Minimize2}
              title="最小化到系统托盘"
              desc="关闭窗口不退出，下载在后台继续；托盘右键可唤起 / 暂停 / 继续"
              checked={s.minimizeToTray}
              onChange={(v) => persist({ ...s, minimizeToTray: v })}
            />
            <ToggleRow
              icon={Bell}
              title="下载完成通知"
              desc="任务完成 / 失败时弹出系统通知"
              checked={s.downloadNotify}
              onChange={(v) => persist({ ...s, downloadNotify: v })}
            />
            <ToggleRow
              icon={Power}
              title="开机自启"
              desc="开机自动启动云析并拉起 aria2 下载引擎"
              checked={s.autoLaunch}
              onChange={(v) => persist({ ...s, autoLaunch: v })}
            />
          </div>
        </section>
      )}

      {/* 下载 */}
      {s && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "120ms" }}>
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-ink">下载（aria2 引擎）</h3>
            {saving && <Loader2 size={14} className="animate-spin text-clay" />}
          </div>

          {/* 下载目录 */}
          <div className="mt-4 flex items-center justify-between gap-4">
            <div className="min-w-0">
              <p className="text-sm text-ink-soft">保存目录</p>
              <p className="mt-1 truncate font-mono text-xs text-ink" title={s.downloadDir}>
                {s.downloadDir || "系统「下载」文件夹"}
              </p>
            </div>
            <div className="flex shrink-0 gap-2">
              <button
                onClick={pickDownloadDir}
                className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
              >
                <FolderOpen size={13} />
                选择目录
              </button>
              {s.downloadDir && (
                <button
                  onClick={() => revealItemInDir(s.downloadDir).catch(() => {})}
                  className="rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs text-ink-soft hover:border-clay hover:text-clay-deep"
                  title="打开目录"
                >
                  <ExternalLink size={13} />
                </button>
              )}
            </div>
          </div>

          <dl className="mt-4 divide-y divide-ink/10">
            {/* 分片并发 */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">分片并发数（split）</dt>
              <dd className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={64}
                  step={1}
                  value={s.downloadThreads}
                  onChange={(e) => setSettings({ ...s, downloadThreads: Number(e.currentTarget.value) })}
                  onMouseUp={(e) => persist({ ...s, downloadThreads: Number((e.currentTarget as HTMLInputElement).value) })}
                  onTouchEnd={(e) => persist({ ...s, downloadThreads: Number((e.currentTarget as HTMLInputElement).value) })}
                  className="w-44 accent-[#d97757]"
                />
                <span className="w-8 text-right font-mono text-sm text-ink">{s.downloadThreads}</span>
              </dd>
            </div>
            {/* 并发任务 */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">同时下载任务数</dt>
              <dd className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={10}
                  step={1}
                  value={s.maxConcurrentDownloads}
                  onChange={(e) => setSettings({ ...s, maxConcurrentDownloads: Number(e.currentTarget.value) })}
                  onMouseUp={(e) => persist({ ...s, maxConcurrentDownloads: Number((e.currentTarget as HTMLInputElement).value) })}
                  className="w-44 accent-[#d97757]"
                />
                <span className="w-8 text-right font-mono text-sm text-ink">{s.maxConcurrentDownloads}</span>
              </dd>
            </div>
            {/* 全局限速 */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">全局限速</dt>
              <dd className="flex gap-1.5">
                {SPEED_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    onClick={() => persist({ ...s, downloadSpeedLimit: opt.value })}
                    className={`rounded-ctrl px-3 py-1 text-xs font-medium transition-colors ${
                      s.downloadSpeedLimit === opt.value
                        ? "bg-clay text-on-accent"
                        : "bg-carrier-deep text-ink-soft hover:text-ink"
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </dd>
            </div>
            {/* 失败重试 */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">失败重试次数</dt>
              <dd className="flex items-center gap-3">
                <input
                  type="range"
                  min={0}
                  max={10}
                  step={1}
                  value={s.downloadRetryCount}
                  onChange={(e) => setSettings({ ...s, downloadRetryCount: Number(e.currentTarget.value) })}
                  onMouseUp={(e) => persist({ ...s, downloadRetryCount: Number((e.currentTarget as HTMLInputElement).value) })}
                  className="w-44 accent-[#d97757]"
                />
                <span className="w-8 text-right font-mono text-sm text-ink">{s.downloadRetryCount}</span>
              </dd>
            </div>
            {/* 分片最小体积（高级） */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">
                分片最小体积
                <span className="ml-1 font-mono text-[10px] text-ink-soft/60">min-split-size</span>
              </dt>
              <dd className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={64}
                  step={1}
                  value={s.downloadMinSplitMb}
                  onChange={(e) => setSettings({ ...s, downloadMinSplitMb: Number(e.currentTarget.value) })}
                  onMouseUp={(e) => persist({ ...s, downloadMinSplitMb: Number((e.currentTarget as HTMLInputElement).value) })}
                  className="w-44 accent-[#d97757]"
                />
                <span className="w-12 text-right font-mono text-sm text-ink">{s.downloadMinSplitMb} MB</span>
              </dd>
            </div>
            {/* 单服务器连接数（高级） */}
            <div className="flex items-center justify-between py-3">
              <dt className="text-sm text-ink-soft">
                单服务器最大连接数
                <span className="ml-1 font-mono text-[10px] text-ink-soft/60">max-connection-per-server</span>
              </dt>
              <dd className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={16}
                  step={1}
                  value={s.downloadConnPerServer}
                  onChange={(e) => setSettings({ ...s, downloadConnPerServer: Number(e.currentTarget.value) })}
                  onMouseUp={(e) => persist({ ...s, downloadConnPerServer: Number((e.currentTarget as HTMLInputElement).value) })}
                  className="w-44 accent-[#d97757]"
                />
                <span className="w-8 text-right font-mono text-sm text-ink">{s.downloadConnPerServer}</span>
              </dd>
            </div>
            <div className="py-3">
              <dt className="text-xs text-ink-soft/70">
                分片体积越小越容易吃满多连接带宽（小文件无所谓）；连接数受网盘风控限制，过高可能被限速。对新任务即时生效。
              </dt>
            </div>
            {s.downloadSpeedLimit > 0 && (
              <div className="py-3">
                <dt className="text-xs text-ink-soft/70">
                  当前限速 {formatBytes(s.downloadSpeedLimit)}/s（对新增流量立即生效）
                </dt>
              </div>
            )}
          </dl>

          {/* BT Tracker 自动更新 + 完成后动作 */}
          <div className="mt-2 divide-y divide-ink/10 border-t border-ink/10 pt-1">
            <ToggleRow
              icon={Magnet}
              title="BT Tracker 列表自动更新"
              desc="每日拉取 TrackersListCollection 高速公共列表（走代理设置），提升磁力 / BT 下载速度"
              checked={s.btTrackerAutoUpdate}
              onChange={(v) => persist({ ...s, btTrackerAutoUpdate: v })}
            />
            <div className="flex items-center justify-between gap-4 py-3">
              <div className="flex min-w-0 items-start gap-3">
                <Power size={16} className="mt-0.5 shrink-0 text-clay" strokeWidth={1.8} />
                <div className="min-w-0">
                  <p className="text-sm text-ink-soft">全部下载完成后</p>
                  <p className="mt-0.5 text-xs text-ink-soft/70">关机保留 60 秒取消窗口（命令行执行 shutdown /a）</p>
                </div>
              </div>
              <select
                value={s.afterDownloadAction}
                onChange={(e) => persist({ ...s, afterDownloadAction: e.currentTarget.value })}
                className="h-9 shrink-0 rounded-ctrl border border-ink/10 bg-carrier-deep px-2 text-xs text-ink focus:border-clay focus:outline-none"
              >
                {AFTER_ACTION_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </section>
      )}

      {/* 代理（aria2 下载透明走代理） */}
      {s && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "135ms" }}>
          <div className="flex items-center justify-between">
            <h3 className="flex items-center gap-2 text-sm font-semibold text-ink">
              <Globe size={15} className="text-clay" strokeWidth={1.8} />
              代理（下载 / PanSou / 测速）
            </h3>
            {saving && <Loader2 size={14} className="animate-spin text-clay" />}
          </div>

          {/* 开关 + 类型 */}
          <div className="mt-2 divide-y divide-ink/10">
            <ToggleRow
              icon={ShieldCheck}
              title="启用全局代理"
              desc="开启后下载引擎、搜索服务与更新下载均走代理；重启引擎后完全生效"
              checked={s.proxyEnabled}
              onChange={(v) => persist({ ...s, proxyEnabled: v })}
            />
            {s.proxyEnabled && (
              <div className="space-y-3 pt-3">
                {/* 类型 + 地址 + 端口 */}
                <div className="flex items-center gap-2">
                  <select
                    value={s.proxyType}
                    onChange={(e) => persist({ ...s, proxyType: e.currentTarget.value })}
                    className="h-9 rounded-ctrl border border-ink/10 bg-carrier-deep px-2 font-mono text-xs text-ink focus:border-clay focus:outline-none"
                  >
                    <option value="http">HTTP</option>
                    <option value="socks5">SOCKS5</option>
                  </select>
                  <input
                    type="text"
                    defaultValue={s.proxyHost}
                    key={`host-${s.proxyHost}`}
                    placeholder="127.0.0.1"
                    spellCheck={false}
                    onBlur={(e) => {
                      if (e.currentTarget.value.trim() !== s.proxyHost) persist({ ...s, proxyHost: e.currentTarget.value.trim() });
                    }}
                    className="h-9 min-w-0 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                  />
                  <input
                    type="number"
                    defaultValue={s.proxyPort || ""}
                    key={`port-${s.proxyPort}`}
                    placeholder="端口"
                    min={1}
                    max={65535}
                    onBlur={(e) => {
                      const n = Math.max(1, Math.min(65535, Number(e.currentTarget.value) || 0));
                      if (n !== s.proxyPort) persist({ ...s, proxyPort: n });
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
                    }}
                    className="h-9 w-24 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                  />
                </div>
                {/* 认证（可选） */}
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    defaultValue={s.proxyUsername}
                    key={`user-${s.proxyUsername}`}
                    placeholder="用户名（可选）"
                    spellCheck={false}
                    onBlur={(e) => {
                      if (e.currentTarget.value !== s.proxyUsername) persist({ ...s, proxyUsername: e.currentTarget.value });
                    }}
                    className="h-9 min-w-0 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                  />
                  <input
                    type="password"
                    defaultValue={s.proxyPassword}
                    key={`pw-${s.proxyPassword}`}
                    placeholder="密码（可选，DPAPI 加密存储）"
                    onBlur={(e) => {
                      if (e.currentTarget.value !== s.proxyPassword) persist({ ...s, proxyPassword: e.currentTarget.value });
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
                    }}
                    className="h-9 min-w-0 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                  />
                </div>
                {/* 测试代理 */}
                <div className="flex items-center gap-2 pt-1">
                  <button
                    onClick={async () => {
                      setProxyTesting(true);
                      setProxyResult(null);
                      setError("");
                      try {
                        setProxyResult(await ipc.testProxy());
                      } catch (e) {
                        setError(errMsg(e));
                      } finally {
                        setProxyTesting(false);
                      }
                    }}
                    disabled={proxyTesting || !s.proxyHost || !s.proxyPort}
                    className="flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-medium text-on-accent transition-colors enabled:hover:bg-clay-deep disabled:opacity-50"
                  >
                    {proxyTesting ? <Loader2 size={13} className="animate-spin" /> : <Wifi size={13} />}
                    测试代理
                  </button>
                  {proxyResult && (
                    <span
                      className={
                        proxyResult.ok
                          ? `truncate text-xs text-success`
                          : "truncate text-xs text-danger"
                      }
                      title={proxyResult.error}
                    >
                      {proxyResult.ok
                        ? `出口 IP ${proxyResult.ip} · ${proxyResult.latencyMs}ms`
                        : `连接失败：${proxyResult.error}`}
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-ink-soft/70">
                  代理地址变更即时写入下载引擎；若为限制式代理建议同时开启「重新启动引擎」使 aria2 全量走代理。
                </p>
              </div>
            )}
          </div>
        </section>
      )}



      {/* 搜索（PanSou 自部署服务） */}
      {s && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "150ms" }}>
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-ink">搜索（PanSou 服务）</h3>
            {saving && <Loader2 size={14} className="animate-spin text-clay" />}
          </div>
          <div className="mt-4 flex items-center gap-2">
            <Search size={15} className="shrink-0 text-ink-soft" />
            <input
              type="text"
              defaultValue={s.pansouBaseUrl}
              key={s.pansouBaseUrl}
              placeholder="http://192.168.1.100:8888"
              spellCheck={false}
              onBlur={(e) => {
                if (e.currentTarget.value.trim().replace(/\/+$/, "") !== s.pansouBaseUrl) {
                  savePansouUrl(e.currentTarget.value);
                }
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
              }}
              className="h-9 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
            />
            <button
              onClick={() => goSearchTab()}
              className="shrink-0 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
            >
              前往搜索
            </button>
          </div>
          <p className="mt-3 text-xs text-ink-soft/70">
            PanSou 是可自部署的网盘聚合搜索服务（fish2018/pansou），填入服务根地址后即可在「搜索」页搜全网公开分享资源；留空则关闭搜索功能。
          </p>
          <div className="mt-2 divide-y divide-ink/10 border-t border-ink/10 pt-1">
            <ToggleRow
              icon={Search}
              title="在导航栏显示「搜索」页"
              desc="开启后顶部胶囊出现「搜索」栏目；关闭则隐藏入口（服务地址保留）"
              checked={s.showSearchTab}
              onChange={(v) => persist({ ...s, showSearchTab: v })}
            />
            <ToggleRow
              icon={Rss}
              title="订阅追剧自动下载"
              desc="在搜索页订阅关键词后，定时聚合搜索、识别新集并自动解析下载（借鉴 quark-auto-save）"
              checked={s.subscriptionEnabled}
              onChange={(v) => persist({ ...s, subscriptionEnabled: v })}
            />
            {s.subscriptionEnabled && (
              <div className="flex items-center justify-between gap-4 py-3">
                <div className="flex min-w-0 items-start gap-3">
                  <Bell size={16} className="mt-0.5 shrink-0 text-clay" strokeWidth={1.8} />
                  <div className="min-w-0">
                    <p className="text-sm text-ink-soft">订阅检查间隔</p>
                    <p className="mt-0.5 text-xs text-ink-soft/70">新建订阅将在下个周期开始自动检查</p>
                  </div>
                </div>
                <select
                  value={s.subscriptionIntervalMinutes}
                  onChange={(e) => persist({ ...s, subscriptionIntervalMinutes: Number(e.currentTarget.value) })}
                  className="h-9 shrink-0 rounded-ctrl border border-ink/10 bg-carrier-deep px-2 text-xs text-ink focus:border-clay focus:outline-none"
                >
                  {SUB_INTERVAL_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>
        </section>
      )}

      {/* 更新 */}
      {s && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "165ms" }}>
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-ink">更新（GitHub Releases）</h3>
            {(updater.checking || updater.downloading) && (
              <Loader2 size={14} className="animate-spin text-clay" />
            )}
          </div>

          {/* 自动检查开关 */}
          <div className="mt-4 flex items-center justify-between gap-4">
            <div>
              <p className="text-sm text-ink-soft">启动时自动检查更新</p>
              <p className="mt-0.5 text-xs text-ink-soft/70">关闭后仅可在本页手动点「检查更新」</p>
            </div>
            <button
              role="switch"
              aria-checked={s.autoCheckUpdate}
              onClick={() => persist({ ...s, autoCheckUpdate: !s.autoCheckUpdate })}
              className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
                s.autoCheckUpdate ? "bg-clay" : "bg-ink/15"
              }`}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
                  s.autoCheckUpdate ? "left-[22px]" : "left-0.5"
                }`}
              />
            </button>
          </div>

          {/* 版本与检查 */}
          <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-ink/10 pt-4">
            <span className="text-sm text-ink-soft">
              当前 <span className="font-mono text-ink">v{info?.version ?? "…"}</span>
            </span>
            {updater.info?.hasUpdate ? (
              <span className="text-sm text-clay-deep">
                → 发现新版 <span className="font-mono">v{updater.info.latestVersion}</span>
              </span>
            ) : updater.checked ? (
              <span className="text-sm text-success">已是最新版本</span>
            ) : null}
            <button
              onClick={() => updater.check()}
              disabled={updater.checking || updater.downloading}
              className="ml-auto flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
            >
              <RefreshCw size={13} className={updater.checking ? "animate-spin" : ""} />
              检查更新
            </button>
          </div>

          {/* 下载 / 更新操作 */}
          {updater.info?.hasUpdate && (
            <div className="mt-4 border-t border-ink/10 pt-4">
              {updater.downloading && updater.progress && (
                <div className="mb-3 flex items-center gap-2">
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-ink/10">
                    <div
                      className="h-full rounded-full bg-clay transition-all"
                      style={{
                        width: `${
                          updater.progress.total > 0
                            ? Math.min(100, (updater.progress.received / updater.progress.total) * 100)
                            : 0
                        }%`,
                      }}
                    />
                  </div>
                  <span className="font-mono text-[11px] text-ink-soft">
                    {updater.progress.total > 0
                      ? `${formatBytes(updater.progress.received)} / ${formatBytes(updater.progress.total)} · ${(
                          (updater.progress.received / updater.progress.total) * 100
                        ).toFixed(0)}%`
                      : "…"}
                  </span>
                </div>
              )}
              <div className="flex items-center gap-2">
                <button
                  onClick={async () => {
                    await updater.apply();
                  }}
                  disabled={updater.downloading || updater.installing}
                  className="flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-medium text-on-accent transition-colors enabled:hover:bg-clay-deep disabled:opacity-50"
                >
                  <Download size={13} />
                  {updater.installing ? "正在安装…" : updater.downloading ? "下载中…" : "立即更新"}
                </button>
                {updater.info?.browserDownloadUrl && (
                  <button
                    onClick={() => openUrl(updater.info!.browserDownloadUrl).catch(() => {})}
                    className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
                  >
                    <ExternalLink size={13} />
                    前往发布页
                  </button>
                )}
              </div>
              {updater.error && (
                <p className="mt-2 text-xs text-danger">{updater.error}</p>
              )}
              {updater.installing && (
                <p className="mt-2 text-xs text-ink-soft">将退出应用并自动重启完成更新…</p>
              )}
            </div>
          )}
          <p className="mt-3 text-xs text-ink-soft/70">
            更新包为 NSIS 安装包（x64-setup.exe），应用内下载后静默安装并覆盖旧版；安装完成后自动重启新版本。
          </p>
        </section>
      )}

      {/* 关于 */}
      <section
        className="flex animate-rise items-center gap-8 rounded-card bg-carrier p-6"
        style={{ animationDelay: "180ms" }}
      >
        <div className="min-w-0 flex-1">
          <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">ABOUT</p>
          <h3 className="mt-1.5 font-display text-2xl font-semibold text-ink">
            YunX_DeskTop_Test{" "}
            <span className="font-mono text-base font-normal text-ink-soft">94ANTIMe</span>
          </h3>
          <p className="mt-1 text-sm text-ink-soft">
            网盘分享链接解析与高速下载 · v{info?.version ?? "…"}
          </p>
          <button
            onClick={() => openUrl(GITHUB_URL).catch(() => {})}
            className="mt-4 flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
          >
            <ExternalLink size={14} />
            GitHub 仓库
          </button>
        </div>
        <img
          src={aboutHero}
          alt="云析插画"
          draggable={false}
          className="hidden h-36 w-48 shrink-0 rounded-card object-cover sm:block"
        />
      </section>

      {/* 开源致谢：原位折叠组件（默认收起，仅显示标题 / 项目数 / 箭头；
          展开状态仅保留于当前应用会话，重启后收起；切换设置页期间不卸载故状态保留） */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "210ms" }}>
        <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">OPEN SOURCE</p>
        <button
          type="button"
          id="ack-toggle"
          aria-expanded={ackOpen}
          aria-controls="ack-panel"
          onClick={() => setAckOpen((v) => !v)}
          className="mt-1.5 flex w-full items-center justify-between gap-3 rounded-ctrl text-left focus-visible:outline-none"
        >
          <span className="min-w-0">
            <span className="block text-sm font-semibold text-ink">开源致谢</span>
            <span className="mt-0.5 block text-xs text-ink-soft/80">
              依赖 / 参考 {ACKNOWLEDGEMENTS.length} 个开源项目{ackOpen ? "" : "，点击展开说明与列表"}
            </span>
          </span>
          <ChevronDown
            size={18}
            strokeWidth={1.8}
            aria-hidden
            className={`chevron-rotate shrink-0 text-ink-soft transition-transform duration-200 ${ackOpen ? "rotate-180" : ""}`}
          />
        </button>
        <div
          id="ack-panel"
          role="region"
          aria-labelledby="ack-toggle"
          className="collapse-grid"
          data-open={ackOpen}
          aria-hidden={!ackOpen}
          inert={!ackOpen}
        >
          <div className="collapse-inner">
            <p className="mt-3 border-t border-ink/10 pt-3 text-xs text-ink-soft/80">
              本项目依赖 / 参考了以下开源项目，谨此致谢；各项目版权归其作者所有。
            </p>
            <ul className="mt-3 space-y-2">
              {ACKNOWLEDGEMENTS.map((a) => (
                <li key={a.name}>
                  <button
                    onClick={() => openUrl(a.url).catch(() => {})}
                    className="flex w-full items-center gap-3 rounded-ctrl border border-ink/10 bg-carrier-deep px-3.5 py-2.5 text-left transition-colors hover:border-clay hover:bg-ivory"
                  >
                    <span className="shrink-0 font-mono text-sm font-semibold text-ink">{a.name}</span>
                    <span className="min-w-0 flex-1 truncate text-xs text-ink-soft">{a.role}</span>
                    <ExternalLink size={13} className="shrink-0 text-ink-soft" />
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </section>
    </div>
  );
}
