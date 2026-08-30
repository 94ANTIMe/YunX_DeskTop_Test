import { useEffect, useState } from "react";

/** 主题模式：跟随系统 / 浅色 / 深色 */
export type ThemeMode = "system" | "light" | "dark";

const STORAGE_KEY = "yunx-theme";

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolveStoredMode(): ThemeMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" || stored === "dark" ? stored : "system";
}

/** 主题切换：localStorage 持久化 + html.dark class + 跟随系统实时联动 */
export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(resolveStoredMode);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // 跟随系统：监听系统主题变化
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const effective: "light" | "dark" =
    mode === "system" ? (systemDark ? "dark" : "light") : mode;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", effective === "dark");
    if (mode === "system") localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, mode);
  }, [mode, effective]);

  /** 侧栏快捷切换：在当前生效主题基础上翻转 */
  const toggle = () => setMode(effective === "dark" ? "light" : "dark");

  return { mode, effective, setMode, toggle };
}
