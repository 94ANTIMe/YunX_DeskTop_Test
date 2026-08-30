import { useEffect, useState } from "react";
import { open as openDialogDir } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Bell, ClipboardPaste, Download, ExternalLink, FolderOpen, Loader2, Minimize2, Power, RefreshCw, Search } from "lucide-react";
import PageHeader from "../components/PageHeader";
import { errMsg, ipc, DEFAULT_SETTINGS, type AppInfo, type Settings as SettingsT } from "../lib/ipc";
import { useUpdate } from "../hooks/useUpdate";
import { formatBytes } from "../lib/format";
import type { ThemeMode } from "../hooks/useTheme";
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
    name: "PanSou",
    role: "网盘聚合搜索 API 服务（自部署对接）",
    url: "https://github.com/fish2018/pansou",
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

const THEME_OPTIONS: { mode: ThemeMode; label: string; value: number }[] = [
  { mode: "system", label: "跟随系统", value: 0 },
  { mode: "light", label: "浅色", value: 1 },
  { mode: "dark", label: "深色", value: 2 },
];

/** 限速选项（字节/秒） */
const SPEED_OPTIONS: { value: number; label: string }[] = [
  { value: 0, label: "不限速" },
  { value: 1_048_576, label: "1 MB/s" },
  { value: 5_242_880, label: "5 MB/s" },
  { value: 10_485_760, label: "10 MB/s" },
  { value: 52_428_800, label: "50 MB/s" },
];

interface SettingsPageProps {
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
  /** 跳转主 Tab（如「前往搜索」） */
  onNavigate?: (tab: TabId) => void;
}

/** 设置页：外观 / 下载（settings.json 持久化）/ 搜索服务 / 关于 */
export default function SettingsPage({ themeMode, onThemeModeChange, onNavigate }: SettingsPageProps) {
  const [settings, setSettings] = useState<SettingsT | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [saving, setSaving] = useState(false);
  const updater = useUpdate();

  // 初始加载（失败回退默认值，避免区块整体不渲染）
  useEffect(() => {
    ipc
      .getSettings()
      .then((s) => setSettings({ ...DEFAULT_SETTINGS, ...s }))
      .catch(() => setSettings({ ...DEFAULT_SETTINGS }));
    ipc.getAppInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  // 保存（防抖由按钮触发；数字输入即时保存过于频繁，改为失焦/按钮统一保存）
  async function persist(next: SettingsT) {
    setSettings(next);
    setSaving(true);
    setError("");
    try {
      await ipc.updateSettings(next);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setSaving(false);
    }
  }

  function setTheme(mode: ThemeMode, value: number) {
    onThemeModeChange(mode);
    if (settings) persist({ ...settings, darkMode: value });
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
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-cactus/25 px-4 py-2.5 text-sm text-ink">{notice}</div>
      )}

      {/* 外观 */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <h3 className="text-sm font-semibold text-ink">外观</h3>
        <div className="mt-4 flex gap-1.5">
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.mode}
              onClick={() => setTheme(opt.mode, opt.value)}
              className={`rounded-ctrl px-4 py-1.5 text-sm font-medium transition-colors ${
                themeMode === opt.mode
                  ? "bg-clay text-white"
                  : "bg-carrier-deep text-ink-soft hover:text-ink"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
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
                        ? "bg-clay text-white"
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
              <span className="text-sm text-cactus">已是最新版本</span>
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
                      ? `${((updater.progress.received / updater.progress.total) * 100).toFixed(0)}%`
                      : "…"}
                  </span>
                </div>
              )}
              <div className="flex items-center gap-2">
                <button
                  onClick={async () => {
                    const path = await updater.download();
                    if (path) updater.install(path);
                  }}
                  disabled={updater.downloading || updater.installing}
                  className="flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-medium text-white transition-colors enabled:hover:bg-clay-deep disabled:opacity-50"
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
                <p className="mt-2 text-xs text-clay-deep">{updater.error}</p>
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

      {/* 开源致谢 */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "210ms" }}>
        <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">OPEN SOURCE</p>
        <h3 className="mt-1.5 text-sm font-semibold text-ink">开源致谢</h3>
        <p className="mt-1 text-xs text-ink-soft/80">
          本项目依赖 / 参考了以下开源项目，谨此致谢；各项目版权归其作者所有。
        </p>
        <ul className="mt-4 space-y-2">
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
      </section>
    </div>
  );
}
