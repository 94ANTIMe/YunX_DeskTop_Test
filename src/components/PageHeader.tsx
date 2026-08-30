import type { ReactNode } from "react";
import { TABS, type TabId } from "../lib/tabs";

interface PageHeaderProps {
  tab: TabId;
  subtitle?: string;
  /** 右侧动作槽 */
  children?: ReactNode;
}

/** 页面标题：杂志章节式（序号 · 拉丁 kicker + 中文大标题） */
export default function PageHeader({ tab, subtitle, children }: PageHeaderProps) {
  const def = TABS.find((t) => t.id === tab) ?? TABS[0];
  return (
    <header className="flex items-end justify-between gap-6 border-b border-ink/10 pb-6">
      <div>
        <p className="font-mono text-[11px] tracking-[0.3em] text-ink-soft">
          {def.index} · {def.latin}
        </p>
        <h2 className="mt-2 font-display text-4xl font-semibold tracking-tight text-ink">
          {def.label}
        </h2>
        {subtitle && <p className="mt-2 text-sm text-ink-soft">{subtitle}</p>}
      </div>
      {children && <div className="shrink-0 pb-1">{children}</div>}
    </header>
  );
}
