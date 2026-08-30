import { Moon, Sun } from "lucide-react";
import { TABS, type TabId } from "../lib/tabs";

interface TopCapsuleProps {
  current: TabId;
  onSelect: (tab: TabId) => void;
  theme: "light" | "dark";
  onToggleTheme: () => void;
  /** 隐藏的 Tab（如搜索 Tab 默认隐藏，设置开启后显示） */
  hiddenTabs?: TabId[];
}

/** 顶部居中悬浮胶囊：品牌字 + Tab 胶囊导航 + 主题切换（取代原左侧 SideNav） */
export default function TopCapsule({ current, onSelect, theme, onToggleTheme, hiddenTabs }: TopCapsuleProps) {
  const visible = TABS.filter((t) => !hiddenTabs?.includes(t.id));
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
        return (
          <button
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            aria-current={active ? "page" : undefined}
            className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium transition-colors ${
              active
                ? "bg-clay text-white"
                : "text-ink-soft hover:bg-carrier-deep hover:text-ink"
            }`}
          >
            <Icon size={16} strokeWidth={active ? 2.2 : 1.8} />
            {tab.label}
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
