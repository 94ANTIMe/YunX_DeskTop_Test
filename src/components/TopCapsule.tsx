import { Moon, Sun } from "lucide-react";
import { TABS, type TabId } from "../lib/tabs";
import { formatSpeed } from "../lib/format";

interface TopCapsuleProps {
  current: TabId;
  onSelect: (tab: TabId) => void;
  theme: "light" | "dark";
  onToggleTheme: () => void;
  /** 隐藏的 Tab（如搜索 Tab 默认隐藏，设置开启后显示） */
  hiddenTabs?: TabId[];
  /** 下载进行中提示（useDownloads store 派生）：count = 排队+下载中任务数，speed = 下载中速度总和 */
  downloadHint?: { count: number; speed: number };
}

/** 顶部居中悬浮胶囊：品牌字 + Tab 胶囊导航 + 下载角标 + 主题切换（取代原左侧 SideNav） */
export default function TopCapsule({ current, onSelect, theme, onToggleTheme, hiddenTabs, downloadHint }: TopCapsuleProps) {
  const visible = TABS.filter((t) => !hiddenTabs?.includes(t.id));
  const downloadBusy = (downloadHint?.count ?? 0) > 0;
  return (
    <nav
      aria-label="主导航"
      className="flex items-center gap-1 rounded-full border border-ink/10 bg-carrier/95 px-2 py-1.5 shadow-capsule backdrop-blur animate-drop"
    >
      {/* 品牌块 + 竖分隔线 */}
      <div className="flex items-center gap-2 pl-3 pr-4">
        <div className="leading-none">
          <span className="font-display text-lg font-semibold tracking-tight text-ink">云析</span>
          <p className="mt-0.5 font-mono text-[8px] tracking-[0.22em] text-ink-soft">
            YUNX · DESKTOP
          </p>
        </div>
        <span className="h-8 w-px bg-ink/10" />
      </div>

      {/* 导航 pills */}
      {visible.map((tab) => {
        const Icon = tab.icon;
        const active = current === tab.id;
        const showDownload = tab.id === "download" && downloadBusy;
        return (
          <button
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            aria-current={active ? "page" : undefined}
            aria-label={showDownload ? `${tab.label}，${downloadHint!.count} 个任务进行中` : undefined}
            className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium transition-colors ${
              active
                ? "bg-clay text-on-accent"
                : "text-ink-soft hover:bg-carrier-deep hover:text-ink"
            }`}
          >
            <Icon size={16} strokeWidth={active ? 2.2 : 1.8} />
            {tab.label}
            {/* 下载角标 + 速度小字（非聚焦 span，语义由按钮 aria-label 承载） */}
            {showDownload && (
              <>
                <span
                  className={`rounded-full px-1.5 font-mono text-[10px] font-semibold leading-4 ${
                    active ? "bg-on-accent/20 text-on-accent" : "bg-clay text-on-accent"
                  }`}
                >
                  {downloadHint!.count}
                </span>
                {downloadHint!.speed > 0 && (
                  <span className={`font-mono text-[10px] ${active ? "text-on-accent/80" : "text-clay-deep"}`}>
                    {formatSpeed(downloadHint!.speed)}
                  </span>
                )}
              </>
            )}
          </button>
        );
      })}

      {/* 主题切换 */}
      <button
        onClick={onToggleTheme}
        title={theme === "dark" ? "切换浅色" : "切换深色"}
        className="ml-1 rounded-full p-2 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
      >
        {theme === "dark" ? <Sun size={17} strokeWidth={1.8} /> : <Moon size={17} strokeWidth={1.8} />}
      </button>
    </nav>
  );
}
