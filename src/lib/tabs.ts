import {
  ArrowDownToLine,
  Cloud,
  Link2,
  ScrollText,
  Search,
  Settings2,
  type LucideIcon,
} from "lucide-react";

/** 主 Tab 标识（对应 Android 版 MainTab 枚举 + 桌面版新增搜索/日志页） */
export type TabId = "resolve" | "drive" | "search" | "download" | "logs" | "settings";

export interface TabDef {
  id: TabId;
  /** 中文标题 */
  label: string;
  /** 拉丁章节名（杂志 kicker） */
  latin: string;
  /** 章节序号 */
  index: string;
  icon: LucideIcon;
}

export const TABS: TabDef[] = [
  { id: "resolve", label: "解析", latin: "RESOLVE", index: "01", icon: Link2 },
  { id: "drive", label: "网盘", latin: "DRIVES", index: "02", icon: Cloud },
  { id: "search", label: "搜索", latin: "SEARCH", index: "03", icon: Search },
  { id: "download", label: "下载", latin: "DOWNLOADS", index: "04", icon: ArrowDownToLine },
  { id: "logs", label: "日志", latin: "LOGS", index: "05", icon: ScrollText },
  { id: "settings", label: "设置", latin: "SETTINGS", index: "06", icon: Settings2 },
];
