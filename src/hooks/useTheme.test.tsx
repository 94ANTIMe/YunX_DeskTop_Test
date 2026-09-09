import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { themeModeFromValue, useTheme } from "./useTheme";

type MediaListener = (e: { matches: boolean }) => void;

/** jsdom 无 matchMedia：stub 出可手动触发的系统主题变化 */
function stubMatchMedia(initialDark: boolean) {
  const listeners = new Set<MediaListener>();
  const mq = {
    matches: initialDark,
    addEventListener: (_: string, fn: MediaListener) => listeners.add(fn),
    removeEventListener: (_: string, fn: MediaListener) => listeners.delete(fn),
  };
  vi.stubGlobal("matchMedia", vi.fn(() => mq));
  return {
    setSystemDark(dark: boolean) {
      mq.matches = dark;
      for (const fn of listeners) fn({ matches: dark });
    },
  };
}

describe("useTheme（明暗 × 配色）", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it("启动应用 localStorage 缓存（明暗 + 配色）并注入 CSS 变量与 data 属性", () => {
    localStorage.setItem("yunx-theme", "dark");
    localStorage.setItem("yunx-color-theme", "yellow-purple");
    stubMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    expect(result.current.mode).toBe("dark");
    expect(result.current.colorTheme).toBe("yellow-purple");
    expect(result.current.effective).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.dataset.colorTheme).toBe("yellow-purple");
    // 语义 token 注入（黄色主题强调色 + 深色 on-accent）
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#ffd200");
    expect(document.documentElement.style.getPropertyValue("--on-accent")).toBe("#1d0d33");
  });

  it("未知配色缓存回退 warm-editorial", () => {
    localStorage.setItem("yunx-color-theme", "nope-theme");
    stubMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    expect(result.current.colorTheme).toBe("warm-editorial");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#d97757");
  });

  it("hydrate 用后端设置校准（settings.json 为持久化来源），覆盖本地缓存", () => {
    localStorage.setItem("yunx-theme", "dark");
    localStorage.setItem("yunx-color-theme", "cyber-neon");
    stubMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    act(() => result.current.hydrate("light", "electric-cyan"));
    expect(result.current.mode).toBe("light");
    expect(result.current.colorTheme).toBe("electric-cyan");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.dataset.colorTheme).toBe("electric-cyan");
    expect(localStorage.getItem("yunx-color-theme")).toBe("electric-cyan");
  });

  it("系统明暗变化仅在跟随系统时生效，且配色保持不变", () => {
    const media = stubMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    // 跟随系统：系统切深色 → 生效深色，配色不变
    act(() => media.setSystemDark(true));
    expect(result.current.effective).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(result.current.colorTheme).toBe("warm-editorial");
    // 固定浅色：系统再变化不生效
    act(() => result.current.setMode("light"));
    act(() => media.setSystemDark(false));
    expect(result.current.effective).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("setColorTheme 更新状态、CSS 变量与 localStorage 缓存", () => {
    stubMatchMedia(false);
    const { result } = renderHook(() => useTheme());
    act(() => result.current.setColorTheme("acid-warning"));
    expect(result.current.colorTheme).toBe("acid-warning");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#b4ff00");
    expect(localStorage.getItem("yunx-color-theme")).toBe("acid-warning");
  });

  it("darkMode 数值映射（保持历史约定：0 系统 / 1 浅色 / 2 深色，未知回退系统）", () => {
    expect(themeModeFromValue(0)).toBe("system");
    expect(themeModeFromValue(1)).toBe("light");
    expect(themeModeFromValue(2)).toBe("dark");
    expect(themeModeFromValue(42)).toBe("system");
  });
});
