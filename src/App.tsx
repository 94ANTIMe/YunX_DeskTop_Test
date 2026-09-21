import { memo, useCallback, useEffect, useState } from "react";
import TopCapsule from "./components/TopCapsule";
import ToastHost from "./components/ToastHost";
import ClipboardPrompt from "./components/ClipboardPrompt";
import UpdateBanner from "./components/UpdateBanner";
import OnboardingPage from "./pages/OnboardingPage";
import ResolvePage from "./pages/ResolvePage";
import DrivePage from "./pages/DrivePage";
import SearchPage from "./pages/SearchPage";
import DownloadPage from "./pages/DownloadPage";
import StatsPage from "./pages/StatsPage";
import LogsPage from "./pages/LogsPage";
import SettingsPage from "./pages/SettingsPage";
import { themeModeFromValue, useTheme } from "./hooks/useTheme";
import { useAppearanceSaver } from "./hooks/useAppearance";
import { useUpdate } from "./hooks/useUpdate";
import { useDownloads } from "./hooks/useDownloads";
import { ipc, onClipboardShare, onSettingsUpdated, type ClipboardShareEvent, type Settings } from "./lib/ipc";
import { TABS, type TabId } from "./lib/tabs";
import { looksLikeLink } from "./lib/download";

// 常驻挂载的页面全部 memo 化：App 层状态变化（切 Tab / 更新横幅进度 / 剪贴板提示）
// 不再触发七个隐藏页整树 reconcile；回调 props 均已 useCallback 稳定
const MemoResolvePage = memo(ResolvePage);
const MemoDrivePage = memo(DrivePage);
const MemoSearchPage = memo(SearchPage);
const MemoDownloadPage = memo(DownloadPage);
const MemoStatsPage = memo(StatsPage);
const MemoLogsPage = memo(LogsPage);
const MemoSettingsPage = memo(SettingsPage);

/** 应用壳：顶部居中悬浮胶囊 + 内容区。
 *  六页常驻挂载（hidden 切换）：切栏目不卸载组件，解析会话 / 收集进度 / 列表状态全部保留。 */
export default function App() {
  const [tab, setTab] = useState<TabId>("resolve");
  const { mode, effective, colorTheme, setMode, setColorTheme, hydrate } = useTheme();
  // 首次启动引导（onboarded=false 时全屏展示；完成后进入主界面）
  const [onboarded, setOnboarded] = useState<boolean | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsError, setSettingsError] = useState("");
  // 设置（只读关键开关：剪贴板监听、搜索 Tab 显隐）
  const [showSearchTab, setShowSearchTab] = useState(false);
  const [clipboardOn, setClipboardOn] = useState(false);
  // 搜索页 → 解析页的待解析链接（消费后清空，避免重复触发）
  const [pendingResolve, setPendingResolve] = useState<{ link: string; pwd: string } | null>(null);
  // 剪贴板命中的分享链接提示（null = 不显示）
  const [clipShare, setClipShare] = useState<ClipboardShareEvent | null>(null);
  const updater = useUpdate();
  // 下载任务全局 store：顶栏角标 + 完成失败自动 toast 的数据源（单例，多订阅者零增量 IPC）
  const downloads = useDownloads();
  // 本次会话是否已忽略更新横幅（「稍后」后不再提醒）
  const [updateDismissed, setUpdateDismissed] = useState(false);
  // 外观保存队列（设置页 / 顶部胶囊共享）：串行写入 + 失败回滚
  const { applyAppearance, markSettings } = useAppearanceSaver({ setMode, setColorTheme });

  // 启动时读取设置：剪贴板开关、搜索 Tab 显隐、自动检查更新；并校准主题（settings.json 为持久化来源）
  const loadInitialSettings = useCallback(() => {
    let alive = true;
    setSettingsError("");
    ipc
      .getSettings()
      .then((s) => {
        if (!alive) return;
        setSettings(s);
        setOnboarded(Boolean(s.onboarded));
        setClipboardOn(s.clipboardMonitor);
        setShowSearchTab(s.showSearchTab);
        markSettings(s);
        hydrate(themeModeFromValue(s.darkMode), s.colorTheme);
        if (s.autoCheckUpdate) updater.check();
      })
      .catch((cause) => {
        if (alive) setSettingsError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => { alive = false; };
  }, [updater, hydrate, markSettings]);

  useEffect(() => {
    return loadInitialSettings();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 订阅剪贴板命中事件（仅开关开启时展示提示）
  useEffect(() => {
    const un = onClipboardShare((share) => {
      setClipShare((prev) =>
        prev &&
        prev.parsed.platform === share.parsed.platform &&
        prev.parsed.shareId === share.parsed.shareId
          ? prev
          : share
      );
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // 设置保存后即时生效（搜索胶囊显隐、剪贴板监听开关；同时刷新外观保存的基准设置）
  useEffect(() => {
    const un = onSettingsUpdated((s) => {
      setSettings(s);
      setOnboarded(Boolean(s.onboarded));
      setShowSearchTab(s.showSearchTab);
      setClipboardOn(s.clipboardMonitor);
      markSettings(s);
    });
    return () => {
      un.then((f) => f());
    };
  }, [markSettings]);

  // 搜索 Tab 被关闭时，若当前停在搜索页则跳回解析页
  useEffect(() => {
    if (!showSearchTab && tab === "search") setTab("resolve");
  }, [showSearchTab, tab]);

  // 全局快捷键（B3.3）：Ctrl+1~7 按可见 Tab 顺序切页
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.shiftKey || e.metaKey) return;
      if (e.key < "1" || e.key > "7") return;
      const visible = TABS.filter((t) => !(t.id === "search" && !showSearchTab));
      const target = visible[Number(e.key) - 1];
      if (target) setTab(target.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showSearchTab]);


  /** 下载并安装最新版本（装完应用自动退出重启） */
  async function applyUpdate() {
    await updater.apply();
  }

  /** 顶部胶囊明暗切换：翻转生效主题并持久化（所选配色始终保留） */
  const toggleThemePersist = useCallback(() => {
    applyAppearance({ mode: effective === "dark" ? "light" : "dark" });
  }, [applyAppearance, effective]);

  /** 更新横幅「稍后」：下载中先取消下载（可稍后重试），并隐藏横幅 */
  const dismissUpdate = useCallback(() => {
    void updater.cancel();
    setUpdateDismissed(true);
  }, [updater]);

  /** 搜索结果 / 剪贴板转入解析：填入链接并自动开始解析 */
  const goResolve = useCallback((link: string, pwd?: string) => {
    setPendingResolve({ link, pwd: pwd || "" });
    setClipShare(null);
    setTab("resolve");
  }, []);

  const consumePendingResolve = useCallback(() => setPendingResolve(null), []);
  // 全局 Ctrl+V：非输入焦点粘贴分享链接 → 直接进入解析（与剪贴板提示共用 goResolve，天然去重）
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) return;
      const text = e.clipboardData?.getData("text") ?? "";
      if (looksLikeLink(text)) goResolve(text.trim());
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [goResolve]);
  const dismissClipShare = useCallback(() => setClipShare(null), []);

  const showUpdateBanner = !updateDismissed && !!updater.info?.hasUpdate;

  // 首启引导（设置尚未读取 = null 时渲染空白等待；避免闪烁主界面）
  if (onboarded === null && settingsError) return (
    <div className="flex h-full items-center justify-center bg-ivory p-8 text-ink">
      <div className="max-w-md rounded-card bg-carrier p-6 shadow-capsule">
        <h1 className="font-display text-xl font-semibold">无法读取应用设置</h1>
        <p className="mt-2 text-sm text-clay-deep">{settingsError}</p>
        <button onClick={() => loadInitialSettings()} className="mt-5 rounded-ctrl bg-clay px-4 py-2 text-sm font-medium text-on-accent">重试</button>
      </div>
    </div>
  );
  if (onboarded === null) return <div className="h-full bg-ivory" />;
  if (onboarded === false) {
    return (
      <div className="flex h-full flex-col overflow-hidden bg-ivory text-ink">
        <main className="min-h-0 flex-1 overflow-y-auto">
          {settings && <OnboardingPage settings={settings} onDone={() => setOnboarded(true)} />}
        </main>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-ivory text-ink">
      <header className="z-10 flex shrink-0 justify-center px-6 pt-6">
        <div className="flex w-full max-w-5xl flex-col items-center gap-2">
          {showUpdateBanner && (
            <UpdateBanner
              latestVersion={updater.info?.latestVersion ?? ""}
              currentVersion={updater.info?.currentVersion}
              notes={updater.info?.notes ?? ""}
              downloading={updater.downloading}
              installing={updater.installing}
              progress={updater.progress}
              error={updater.error}
              rawError={updater.rawError}
              browserUrl={updater.info?.browserDownloadUrl || undefined}
              onUpdate={applyUpdate}
              onDismiss={dismissUpdate}
              onCancel={dismissUpdate}
            />
          )}
          <TopCapsule
            current={tab}
            onSelect={setTab}
            theme={effective}
            onToggleTheme={toggleThemePersist}
            hiddenTabs={showSearchTab ? [] : ["search"]}
            downloadHint={{ count: downloads.activeCount, speed: downloads.totalSpeed }}
          />
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-10 pb-10 pt-8">
          {/* 常驻挂载：切 Tab 只切可见性，状态不丢（页面已 memo，隐藏页零 reconcile） */}
          <div className={tab === "resolve" ? "block" : "hidden"}>
            <MemoResolvePage
              onNavigate={setTab}
              pending={pendingResolve}
              onPendingConsumed={consumePendingResolve}
            />
          </div>
          <div className={tab === "drive" ? "block" : "hidden"}>
            <MemoDrivePage onNavigate={setTab} onGoResolve={goResolve} />
          </div>
          <div className={tab === "search" ? "block" : "hidden"}>
            <MemoSearchPage active={tab === "search"} onGoResolve={goResolve} />
          </div>
          <div className={tab === "download" ? "block" : "hidden"}>
            <MemoDownloadPage active={tab === "download"} onNavigate={setTab} onGoResolve={goResolve} />
          </div>
          <div className={tab === "stats" ? "block" : "hidden"}>
            <MemoStatsPage active={tab === "stats"} />
          </div>
          <div className={tab === "logs" ? "block" : "hidden"}>
            <MemoLogsPage active={tab === "logs"} />
          </div>
          <div className={tab === "settings" ? "block" : "hidden"}>
            <MemoSettingsPage
              themeMode={mode}
              colorTheme={colorTheme}
              onAppearanceChange={applyAppearance}
              onNavigate={setTab}
            />
          </div>
        </div>
      </main>
      {/* 剪贴板分享链接提示 */}
      {clipboardOn && clipShare && (
        <ClipboardPrompt share={clipShare} onResolve={goResolve} onDismiss={dismissClipShare} />
      )}
      {/* 全局 Toast（下载完成/失败等即时反馈；aria-live 容器在组件内部） */}
      <ToastHost />
    </div>
  );
}
