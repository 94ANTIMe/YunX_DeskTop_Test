import { useEffect, useState } from "react";
import TopCapsule from "./components/TopCapsule";
import ClipboardPrompt from "./components/ClipboardPrompt";
import UpdateBanner from "./components/UpdateBanner";
import ResolvePage from "./pages/ResolvePage";
import DrivePage from "./pages/DrivePage";
import SearchPage from "./pages/SearchPage";
import DownloadPage from "./pages/DownloadPage";
import LogsPage from "./pages/LogsPage";
import SettingsPage from "./pages/SettingsPage";
import { useTheme } from "./hooks/useTheme";
import { useUpdate } from "./hooks/useUpdate";
import { ipc, onClipboardShare, type ClipboardShareEvent } from "./lib/ipc";
import type { TabId } from "./lib/tabs";

/** 应用壳：顶部居中悬浮胶囊 + 内容区。
 *  六页常驻挂载（hidden 切换）：切栏目不卸载组件，解析会话 / 收集进度 / 列表状态全部保留。 */
export default function App() {
  const [tab, setTab] = useState<TabId>("resolve");
  const { mode, effective, setMode, toggle } = useTheme();
  // 设置（只读关键开关：剪贴板监听、搜索 Tab 显隐）
  const [showSearchTab, setShowSearchTab] = useState(false);
  const [clipboardOn, setClipboardOn] = useState(false);
  // 搜索页 → 解析页的待解析链接（消费后清空，避免重复触发）
  const [pendingResolve, setPendingResolve] = useState<{ link: string; pwd: string } | null>(null);
  // 剪贴板命中的分享链接提示（null = 不显示）
  const [clipShare, setClipShare] = useState<ClipboardShareEvent | null>(null);
  const updater = useUpdate();
  // 本次会话是否已忽略更新横幅（「稍后」后不再提醒）
  const [updateDismissed, setUpdateDismissed] = useState(false);

  // 启动时读取设置：剪贴板开关、搜索 Tab 显隐、自动检查更新
  useEffect(() => {
    let alive = true;
    ipc
      .getSettings()
      .then((s) => {
        if (!alive) return;
        setClipboardOn(s.clipboardMonitor);
        setShowSearchTab(s.showSearchTab);
        if (s.autoCheckUpdate) updater.check();
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
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

  // 搜索 Tab 被关闭时，若当前停在搜索页则跳回解析页
  useEffect(() => {
    if (!showSearchTab && tab === "search") setTab("resolve");
  }, [showSearchTab, tab]);

  /** 下载并安装最新版本（装完应用自动退出重启） */
  async function applyUpdate() {
    const path = await updater.download();
    if (path) updater.install(path);
  }

  /** 搜索结果 / 剪贴板转入解析：填入链接并自动开始解析 */
  function goResolve(link: string, pwd: string) {
    setPendingResolve({ link, pwd });
    setClipShare(null);
    setTab("resolve");
  }

  const showUpdateBanner = !updateDismissed && !!updater.info?.hasUpdate;

  return (
    <div className="flex h-full flex-col overflow-hidden bg-ivory text-ink">
      <header className="z-10 flex shrink-0 justify-center px-6 pt-6">
        <div className="flex w-full max-w-5xl flex-col items-center gap-2">
          {showUpdateBanner && (
            <UpdateBanner
              latestVersion={updater.info?.latestVersion ?? ""}
              notes={updater.info?.notes ?? ""}
              downloading={updater.downloading}
              installing={updater.installing}
              progress={updater.progress}
              error={updater.error}
              onUpdate={applyUpdate}
              onDismiss={() => setUpdateDismissed(true)}
            />
          )}
          <TopCapsule
            current={tab}
            onSelect={setTab}
            theme={effective}
            onToggleTheme={toggle}
            hiddenTabs={showSearchTab ? [] : ["search"]}
          />
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-10 pb-10 pt-8">
          {/* 常驻挂载：切 Tab 只切可见性，状态不丢 */}
          <div className={tab === "resolve" ? "block" : "hidden"}>
            <ResolvePage
              onNavigate={setTab}
              pending={pendingResolve}
              onPendingConsumed={() => setPendingResolve(null)}
            />
          </div>
          <div className={tab === "drive" ? "block" : "hidden"}>
            <DrivePage />
          </div>
          <div className={tab === "search" ? "block" : "hidden"}>
            <SearchPage active={tab === "search"} onGoResolve={goResolve} />
          </div>
          <div className={tab === "download" ? "block" : "hidden"}>
            <DownloadPage onNavigate={setTab} />
          </div>
          <div className={tab === "logs" ? "block" : "hidden"}>
            <LogsPage />
          </div>
          <div className={tab === "settings" ? "block" : "hidden"}>
            <SettingsPage themeMode={mode} onThemeModeChange={setMode} onNavigate={setTab} />
          </div>
        </div>
      </main>
      {/* 剪贴板分享链接提示 */}
      {clipboardOn && clipShare && (
        <ClipboardPrompt share={clipShare} onResolve={goResolve} onDismiss={() => setClipShare(null)} />
      )}
    </div>
  );
}
