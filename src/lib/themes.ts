/**
 * 配色主题注册表（七套）——新增主题只需：
 * 1. 在 COLOR_THEMES 追加一条（id 唯一 + 中文名 + 原始双色 colorA/colorB + light/dark 两组完整 token）；
 * 2. 无需修改任何页面组件——CSS 侧通过语义 token（--app-bg 等）经 tokens.css 的 @theme 别名生效。
 *
 * 约定：
 * - colorA 用于主要操作与选中态（→ accent 家族），colorB 用于辅助强调与装饰（→ accent-decor）；
 * - colorA/colorB 保留原始色值，仅供预览色块与主题定义使用；token 内文字、悬停、焦点颜色按
 *   WCAG 对比度派生（正文 ≥4.5:1，控件边界 ≥3:1），页面与卡片一律使用低饱和派生底色；
 * - 状态色（success/warning/error）与主题强调色分离，不在本注册表中随强调色变化。
 */

/** 语义 token 集（键名与 CSS 变量 --<key> 一一对应，由 useTheme 注入 documentElement） */
export interface ThemeTokens {
  /** 页面底 */
  "app-bg": string;
  /** 一级承载面（卡片） */
  "surface-card": string;
  /** 二级承载面（输入框 / 嵌套面板） */
  "surface-input": string;
  /** 侧栏底 / 选中态底 */
  "surface-muted": string;
  /** 主文字 */
  "text-primary": string;
  /** 次级文字 */
  "text-secondary": string;
  /** 弱边框 */
  "border-soft": string;
  /** 强边框 */
  "border-strong": string;
  /** 主强调：按钮 / 选中 / 进度（颜色 A 或其对比度派生） */
  accent: string;
  /** 强调 hover / 按下 */
  "accent-strong": string;
  /** 低饱和强调底（选中底 / tint 面板） */
  "accent-soft": string;
  /** 辅助强调与装饰（颜色 B 或其对比度派生） */
  "accent-decor": string;
  /** 强调色（按钮）上的文字：亮黄 / 亮绿 / 青背景用深色字，深底强调用白字 */
  "on-accent": string;
  /** 焦点环 */
  "focus-ring": string;
  /** 成功状态（与强调色分离） */
  "status-success": string;
  /** 警告状态（与强调色分离） */
  "status-warning": string;
  /** 错误状态（与强调色分离） */
  "status-error": string;
}

/** 有效明暗模式（不含 system；由 useTheme 解析得到） */
export type EffectiveMode = "light" | "dark";

export interface ColorTheme {
  /** 稳定 ID（持久化于 settings.colorTheme / settings.json） */
  id: string;
  /** 中文名称（设置页主题卡展示） */
  name: string;
  /** 原始颜色 A：主要操作 / 选中态（预览色块保留原始值） */
  colorA: string;
  /** 原始颜色 B：辅助强调 / 装饰（预览色块保留原始值） */
  colorB: string;
  light: ThemeTokens;
  dark: ThemeTokens;
}

/** 语义 token 全集（应用顺序无关，注入 CSS 变量 --<key>） */
export const TOKEN_KEYS = Object.keys({
  "app-bg": 0,
  "surface-card": 0,
  "surface-input": 0,
  "surface-muted": 0,
  "text-primary": 0,
  "text-secondary": 0,
  "border-soft": 0,
  "border-strong": 0,
  accent: 0,
  "accent-strong": 0,
  "accent-soft": 0,
  "accent-decor": 0,
  "on-accent": 0,
  "focus-ring": 0,
  "status-success": 0,
  "status-warning": 0,
  "status-error": 0,
} satisfies Record<keyof ThemeTokens, unknown>) as (keyof ThemeTokens)[];

/** 默认主题：原有暖色编辑风 */
export const DEFAULT_COLOR_THEME = "warm-editorial";

/**
 * 七套配色主题。
 * 浅色模式：低饱和派生底色 + 原色作强调（原色过亮无法承载控件时派生加深，如电光青）；
 * 深色模式：深色主题底 + 提亮的强调色；亮色强调（黄 / 绿 / 青）一律配深色 on-accent。
 */
export const COLOR_THEMES: ColorTheme[] = [
  {
    id: "warm-editorial",
    name: "暖色编辑",
    colorA: "#d97757",
    colorB: "#bcd1ca",
    light: {
      "app-bg": "#faf9f5",
      "surface-card": "#f0eee6",
      "surface-input": "#e8e6dc",
      "surface-muted": "#e3dacc",
      "text-primary": "#141413",
      "text-secondary": "#55524a",
      "border-soft": "#dedacd",
      "border-strong": "#c8c3b1",
      accent: "#d97757",
      "accent-strong": "#c05f41",
      "accent-soft": "#f2e0d7",
      "accent-decor": "#bcd1ca",
      "on-accent": "#ffffff",
      "focus-ring": "#c05f41",
      "status-success": "#2f6b4f",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#1f1e1b",
      "surface-card": "#2a2824",
      "surface-input": "#333029",
      "surface-muted": "#26241f",
      "text-primary": "#faf9f5",
      "text-secondary": "#a8a29a",
      "border-soft": "#3a372f",
      "border-strong": "#4b463b",
      accent: "#d97757",
      "accent-strong": "#e08a6b",
      "accent-soft": "#3c2c24",
      "accent-decor": "#7fa393",
      "on-accent": "#ffffff",
      "focus-ring": "#e08a6b",
      "status-success": "#63b58c",
      "status-warning": "#fbbf24",
      "status-error": "#f87171",
    },
  },
  {
    id: "cyber-neon",
    name: "赛博霓虹",
    colorA: "#fa0387",
    colorB: "#00e0fa",
    light: {
      "app-bg": "#fdf4f8",
      "surface-card": "#f9eaf1",
      "surface-input": "#f3dfe9",
      "surface-muted": "#f5e4ed",
      "text-primary": "#221319",
      "text-secondary": "#70535f",
      "border-soft": "#ecd3de",
      "border-strong": "#d9b3c4",
      accent: "#fa0387",
      "accent-strong": "#d10271",
      "accent-soft": "#fbd9e9",
      "accent-decor": "#0b8fa4",
      "on-accent": "#221319",
      "focus-ring": "#fa0387",
      "status-success": "#0f7a55",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#190a11",
      "surface-card": "#230f19",
      "surface-input": "#2d1420",
      "surface-muted": "#28121c",
      "text-primary": "#fceef5",
      "text-secondary": "#c4a2b1",
      "border-soft": "#3a2230",
      "border-strong": "#4d2d3f",
      accent: "#ff2f9d",
      "accent-strong": "#ff5cb1",
      "accent-soft": "#43152c",
      "accent-decor": "#37d6ee",
      "on-accent": "#1c0511",
      "focus-ring": "#ff2f9d",
      "status-success": "#3ddc97",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
  {
    id: "acid-warning",
    name: "酸性警示",
    colorA: "#b4ff00",
    colorB: "#041900",
    light: {
      "app-bg": "#f9fce9",
      "surface-card": "#eff6d5",
      "surface-input": "#e3eebc",
      "surface-muted": "#e9f1c9",
      "text-primary": "#0a1a02",
      "text-secondary": "#4c5f3d",
      "border-soft": "#dbe6ae",
      "border-strong": "#c3d389",
      accent: "#b4ff00",
      "accent-strong": "#9ed600",
      "accent-soft": "#e4f7a8",
      "accent-decor": "#2e4a14",
      "on-accent": "#0a1a02",
      "focus-ring": "#5f8f00",
      "status-success": "#1a6b3c",
      "status-warning": "#a16207",
      "status-error": "#cc2f1d",
    },
    dark: {
      "app-bg": "#0c1503",
      "surface-card": "#14210a",
      "surface-input": "#1c2d0e",
      "surface-muted": "#182609",
      "text-primary": "#f2ffd9",
      "text-secondary": "#b7cc9c",
      "border-soft": "#26391c",
      "border-strong": "#354f26",
      accent: "#ccff33",
      "accent-strong": "#d9ff66",
      "accent-soft": "#26390f",
      "accent-decor": "#8fce4a",
      "on-accent": "#0c1503",
      "focus-ring": "#ccff33",
      "status-success": "#5fce8f",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
  {
    id: "classic-complement",
    name: "经典互补",
    colorA: "#0135ad",
    colorB: "#f4e11b",
    light: {
      "app-bg": "#f5f7fc",
      "surface-card": "#eaeef8",
      "surface-input": "#dfe4f3",
      "surface-muted": "#e3e8f5",
      "text-primary": "#0e1526",
      "text-secondary": "#4c5568",
      "border-soft": "#d3dae9",
      "border-strong": "#b9c3d9",
      accent: "#0135ad",
      "accent-strong": "#012a8c",
      "accent-soft": "#d4ddf5",
      "accent-decor": "#a08200",
      "on-accent": "#ffffff",
      "focus-ring": "#0135ad",
      "status-success": "#14683f",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#0b1020",
      "surface-card": "#131b31",
      "surface-input": "#1a2340",
      "surface-muted": "#161f38",
      "text-primary": "#edf1fa",
      "text-secondary": "#a6b1c8",
      "border-soft": "#232f4d",
      "border-strong": "#33406a",
      accent: "#2e63f0",
      "accent-strong": "#4d7dff",
      "accent-soft": "#1c2b55",
      "accent-decor": "#f4e11b",
      "on-accent": "#ffffff",
      "focus-ring": "#6b93ff",
      "status-success": "#4bc98c",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
  {
    id: "dream-candy",
    name: "梦幻糖果",
    colorA: "#ed35f9",
    colorB: "#76fcff",
    light: {
      "app-bg": "#fdf3fd",
      "surface-card": "#f8e8fa",
      "surface-input": "#f1dcf4",
      "surface-muted": "#f4e1f6",
      "text-primary": "#251228",
      "text-secondary": "#6f4f73",
      "border-soft": "#ecd2ee",
      "border-strong": "#d9b4dc",
      accent: "#ed35f9",
      "accent-strong": "#c91ed6",
      "accent-soft": "#f8d3fa",
      "accent-decor": "#0b8ba0",
      "on-accent": "#251228",
      "focus-ring": "#d81fe4",
      "status-success": "#0f7a55",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#1c0a1f",
      "surface-card": "#271029",
      "surface-input": "#311633",
      "surface-muted": "#2c132e",
      "text-primary": "#fbeefc",
      "text-secondary": "#cfaad2",
      "border-soft": "#3d2340",
      "border-strong": "#533057",
      accent: "#f056f6",
      "accent-strong": "#f77ef9",
      "accent-soft": "#451a47",
      "accent-decor": "#76fcff",
      "on-accent": "#1c0a1f",
      "focus-ring": "#f056f6",
      "status-success": "#3ddc97",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
  {
    id: "electric-cyan",
    name: "电光青",
    colorA: "#76fcff",
    colorB: "#02ffff",
    light: {
      "app-bg": "#f1fbfc",
      "surface-card": "#e3f4f6",
      "surface-input": "#d4edf0",
      "surface-muted": "#daeff2",
      "text-primary": "#06272c",
      "text-secondary": "#3f646b",
      "border-soft": "#cbe4e8",
      "border-strong": "#aed2d8",
      // 原始 #76fcff 过亮，浅色下无法承载文字，主强调派生为深青（原始值保留于 colorA 预览）
      accent: "#07a0b4",
      "accent-strong": "#08919f",
      "accent-soft": "#c2ecf0",
      "accent-decor": "#35cfe0",
      "on-accent": "#05262b",
      "focus-ring": "#0794a8",
      "status-success": "#0f7a55",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#04232a",
      "surface-card": "#08323b",
      "surface-input": "#0c3f4a",
      "surface-muted": "#0a3a44",
      "text-primary": "#e9fbfc",
      "text-secondary": "#9dc7cd",
      "border-soft": "#164a55",
      "border-strong": "#1f6271",
      accent: "#3fe0ee",
      "accent-strong": "#74eaf4",
      "accent-soft": "#0e4a55",
      "accent-decor": "#02ffff",
      "on-accent": "#04232a",
      "focus-ring": "#3fe0ee",
      "status-success": "#3ddc97",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
  {
    id: "yellow-purple",
    name: "明黄深紫",
    colorA: "#ffd200",
    colorB: "#33065d",
    light: {
      "app-bg": "#fbf7ec",
      "surface-card": "#f3ecd9",
      "surface-input": "#eae2c6",
      "surface-muted": "#ede5cd",
      "text-primary": "#2b1049",
      "text-secondary": "#5f4b74",
      "border-soft": "#e2d8b4",
      "border-strong": "#cdc09a",
      accent: "#ffd200",
      "accent-strong": "#e8bd00",
      "accent-soft": "#f8e88e",
      "accent-decor": "#5b2d86",
      "on-accent": "#2b1049",
      "focus-ring": "#7c3aed",
      "status-success": "#1a6b3c",
      "status-warning": "#a16207",
      "status-error": "#d92d20",
    },
    dark: {
      "app-bg": "#170b26",
      "surface-card": "#211034",
      "surface-input": "#2a1541",
      "surface-muted": "#251239",
      "text-primary": "#f5effc",
      "text-secondary": "#bca4d6",
      "border-soft": "#33224c",
      "border-strong": "#463067",
      accent: "#ffd200",
      "accent-strong": "#ffdb3d",
      "accent-soft": "#3a2454",
      "accent-decor": "#b08aff",
      "on-accent": "#1d0d33",
      "focus-ring": "#ffd200",
      "status-success": "#3ddc97",
      "status-warning": "#fbbf24",
      "status-error": "#ff7a6e",
    },
  },
];

/** 按 ID 查找主题；未知 ID（含空值）回退默认主题（旧数据兼容策略） */
export function getColorTheme(id: string | null | undefined): ColorTheme {
  return COLOR_THEMES.find((t) => t.id === id) ?? COLOR_THEMES[0];
}

/** 规范化主题 ID：未知 / 缺失一律回退 warm-editorial */
export function normalizeColorTheme(id: string | null | undefined): string {
  return COLOR_THEMES.some((t) => t.id === id) ? (id as string) : DEFAULT_COLOR_THEME;
}

/**
 * 将主题 token 注入 documentElement CSS 变量（--<key>）并写入 data-color-theme。
 * 由 useTheme 在 colorTheme × 明暗 变化时调用；tokens.css 中 @theme 别名
 * （--color-ivory → var(--app-bg) 等）随之实时生效。
 */
export function applyColorThemeVars(theme: ColorTheme, mode: EffectiveMode): void {
  const root = document.documentElement;
  root.dataset.colorTheme = theme.id;
  const tokens = mode === "dark" ? theme.dark : theme.light;
  for (const key of TOKEN_KEYS) {
    root.style.setProperty(`--${key}`, tokens[key]);
  }
}
