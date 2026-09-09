import { useCallback, useRef } from "react";
import { ipc, type Settings } from "../lib/ipc";
import { normalizeColorTheme } from "../lib/themes";
import { THEME_MODE_VALUE, themeModeFromValue, type ThemeMode } from "./useTheme";

/** 外观变更（明暗模式 / 配色主题）：即时预览 + 统一进入共享设置保存队列 */
export interface AppearancePatch {
  mode?: ThemeMode;
  colorTheme?: string;
}

interface AppearanceSaverOptions {
  /** 预览 / 回滚时同步明暗模式（来自 useTheme.setMode） */
  setMode: (mode: ThemeMode) => void;
  /** 预览 / 回滚时同步配色主题（来自 useTheme.setColorTheme） */
  setColorTheme: (id: string) => void;
}

/**
 * 外观保存队列（设置页 / 顶部胶囊共享）：
 * - 先即时预览（useTheme 同步 localStorage 缓存与 CSS 变量），再串行写入 settings.json；
 * - 每次 patch 只覆盖对应字段，其余字段以后端最近已知值为准（顶部切换不丢配色）；
 * - 保存失败：用后端回读值同步恢复明暗 / 配色预览与缓存（未知主题回退默认）。
 */
export function useAppearanceSaver({ setMode, setColorTheme }: AppearanceSaverOptions) {
  // 最近一次已知设置（启动读取 / settings:updated 事件 / 本队列写入后刷新）
  const knownRef = useRef<Settings | null>(null);
  const queueRef = useRef<Promise<void>>(Promise.resolve());

  /** 启动读取与其他来源保存后刷新基准设置 */
  const markSettings = useCallback((s: Settings) => {
    knownRef.current = s;
  }, []);

  const applyAppearance = useCallback((patch: AppearancePatch) => {
    if (patch.mode) setMode(patch.mode);
    if (patch.colorTheme) setColorTheme(normalizeColorTheme(patch.colorTheme));
    queueRef.current = queueRef.current.then(async () => {
      try {
        const base = knownRef.current ?? (await ipc.getSettings());
        const merged: Settings = {
          ...base,
          darkMode: patch.mode ? THEME_MODE_VALUE[patch.mode] : base.darkMode,
          colorTheme: patch.colorTheme ? normalizeColorTheme(patch.colorTheme) : base.colorTheme,
        };
        knownRef.current = merged;
        await ipc.updateSettings(merged);
      } catch {
        try {
          const restored = await ipc.getSettings();
          knownRef.current = restored;
          setMode(themeModeFromValue(restored.darkMode));
          setColorTheme(restored.colorTheme);
        } catch {
          // 后端不可达：保留本地预览状态，等待下次保存重试
        }
      }
    });
  }, [setMode, setColorTheme]);

  return { applyAppearance, markSettings };
}
