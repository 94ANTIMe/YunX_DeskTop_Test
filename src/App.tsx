import { useState } from "react";
import TopCapsule from "./components/TopCapsule";
import ResolvePage from "./pages/ResolvePage";
import DrivePage from "./pages/DrivePage";
import SearchPage from "./pages/SearchPage";
import DownloadPage from "./pages/DownloadPage";
import LogsPage from "./pages/LogsPage";
import SettingsPage from "./pages/SettingsPage";
import { useTheme } from "./hooks/useTheme";
import type { TabId } from "./lib/tabs";

/** 应用壳：顶部居中悬浮胶囊 + 内容区。
 *  六页常驻挂载（hidden 切换）：切栏目不卸载组件，解析会话 / 收集进度 / 列表状态全部保留。 */
export default function App() {
  const [tab, setTab] = useState<TabId>("resolve");
  const { mode, effective, setMode, toggle } = useTheme();
  // 搜索页 → 解析页的待解析链接（消费后清空，避免重复触发）
  const [pendingResolve, setPendingResolve] = useState<{ link: string; pwd: string } | null>(null);

  /** 搜索结果转入解析：填入链接并自动开始解析 */
  function goResolve(link: string, pwd: string) {
    setPendingResolve({ link, pwd });
    setTab("resolve");
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-ivory text-ink">
      <header className="z-10 flex shrink-0 justify-center px-6 pt-6">
        <TopCapsule
          current={tab}
          onSelect={setTab}
          theme={effective}
          onToggleTheme={toggle}
        />
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
    </div>
  );
}
