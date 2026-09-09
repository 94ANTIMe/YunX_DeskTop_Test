import { useCallback, useEffect, useState } from "react";
import {
  applyColorThemeVars,
  getColorTheme,
  normalizeColorTheme,
  type EffectiveMode,
} from "../lib/themes";

/** 主题模式：跟随系统 / 浅色 / 深色 */
export type ThemeMode = "system" | "light" | "dark";

/** 明暗模式在 settings.darkMode 中的数值含义（保持历史约定） */
export const THEME_MODE_VALUE: Record<ThemeMode, number> = {
  system: 0,
  light: 1,
  dark: 2,
};

/** settings.darkMode 数值 → ThemeMode（未知值回退跟随系统） */
export function themeModeFromValue(value: number): ThemeMode {
  return value === 1 ? "light" : value === 2 ? "dark" : "system";
}

const MODE_KEY = "yunx-theme";
/** 配色主题启动预览缓存（settings.json 才是持久化来源） */
const COLOR_KEY = "yunx-color-theme";

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolveStoredMode(): ThemeMode {
  const stored = localStorage.getItem(MODE_KEY);
  return stored === "light" || stored === "dark" ? stored : "system";
}

function resolveStoredColorTheme(): string {
  return normalizeColorTheme(localStorage.getItem(COLOR_KEY));
}

/**
 * 主题切换：明暗模式（跟随系统 / 浅色 / 深色）× 配色主题（七套，独立选择）。
 * - 明暗：localStorage 缓存 + html.dark class + 跟随系统实时联动；
 * - 配色：localStorage 启动预览缓存 + 语义 token 注入 documentElement；
 * - 启动后由 App 用后端 settings（settings.json）校准（hydrate）。
 */
export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(resolveStoredMode);
  const [colorTheme, setColorThemeState] = useState<string>(resolveStoredColorTheme);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // 跟随系统：监听系统主题变化（系统明暗变化仅在「跟随系统」时生效）
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const effective: EffectiveMode =
    mode === "system" ? (systemDark ? "dark" : "light") : mode;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", effective === "dark");
    if (mode === "system") localStorage.removeItem(MODE_KEY);
    else localStorage.setItem(MODE_KEY, mode);
  }, [mode, effective]);

  // 配色变更（或明暗解析结果变化）时注入语义 token：CSS 变量立即生效，无需刷新
  useEffect(() => {
    applyColorThemeVars(getColorTheme(colorTheme), effective);
    localStorage.setItem(COLOR_KEY, colorTheme);
  }, [colorTheme, effective]);

  const setColorTheme = useCallback((id: string) => {
    setColorThemeState(normalizeColorTheme(id));
  }, []);

  /**
   * 启动校准：用后端设置回读值覆盖本地缓存（仅启动时调用一次）。
   * settings.json 是持久化来源；后端字段缺失 / 未知主题时回退默认主题。
   */
  const hydrate = useCallback((backendMode: ThemeMode, backendColorTheme: string) => {
    setMode(backendMode);
    setColorThemeState(normalizeColorTheme(backendColorTheme));
  }, []);

  /** 侧栏快捷切换：在当前生效主题基础上翻转（配色保持不变，由调用方负责持久化） */
  const toggle = () => setMode(effective === "dark" ? "light" : "dark");

  return { mode, effective, colorTheme, setMode, setColorTheme, toggle, hydrate };
}
