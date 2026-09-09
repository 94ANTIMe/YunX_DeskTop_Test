import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppearanceSaver } from "./useAppearance";
import { DEFAULT_SETTINGS } from "../lib/ipc";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock("../lib/ipc", async (loadOriginal) => {
  const original = await loadOriginal<typeof import("../lib/ipc")>();
  return { ...original, ipc: { ...original.ipc, ...mocks } };
});

describe("useAppearanceSaver（共享外观保存队列）", () => {
  function setup() {
    const setMode = vi.fn();
    const setColorTheme = vi.fn();
    return { setMode, setColorTheme, ...renderHook(() => useAppearanceSaver({ setMode, setColorTheme })) };
  }

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.updateSettings.mockResolvedValue(undefined);
  });

  it("顶部明暗切换不丢配色：patch 只覆盖对应字段", async () => {
    const { result, setMode, setColorTheme } = setup();
    result.current.markSettings({ ...DEFAULT_SETTINGS, darkMode: 0, colorTheme: "dream-candy" });
    act(() => result.current.applyAppearance({ mode: "dark" }));
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(1));
    const saved = mocks.updateSettings.mock.calls[0][0];
    expect(saved.darkMode).toBe(2);
    expect(saved.colorTheme).toBe("dream-candy");
    expect(setMode).toHaveBeenCalledWith("dark");
    expect(setColorTheme).not.toHaveBeenCalled();
  });

  it("快速连续切换串行保存，后续保存基于最新已知值", async () => {
    const { result } = setup();
    result.current.markSettings({ ...DEFAULT_SETTINGS, darkMode: 0, colorTheme: "warm-editorial" });
    let releaseFirst!: () => void;
    mocks.updateSettings.mockImplementationOnce(() => new Promise<void>((r) => { releaseFirst = r; }));
    act(() => {
      result.current.applyAppearance({ colorTheme: "cyber-neon" });
      result.current.applyAppearance({ mode: "light" });
    });
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(1));
    releaseFirst();
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(2));
    const first = mocks.updateSettings.mock.calls[0][0];
    const second = mocks.updateSettings.mock.calls[1][0];
    expect(first.colorTheme).toBe("cyber-neon");
    expect(first.darkMode).toBe(0);
    // 第二次保存保留第一次的配色（顶部切换不丢配色）
    expect(second.colorTheme).toBe("cyber-neon");
    expect(second.darkMode).toBe(1);
  });

  it("保存失败用后端回读值回滚（恢复明暗与配色预览）", async () => {
    const { result, setMode, setColorTheme } = setup();
    result.current.markSettings({ ...DEFAULT_SETTINGS, darkMode: 0, colorTheme: "warm-editorial" });
    mocks.updateSettings.mockRejectedValueOnce(new Error("磁盘写入失败"));
    mocks.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, darkMode: 1, colorTheme: "acid-warning" });
    act(() => result.current.applyAppearance({ mode: "dark", colorTheme: "yellow-purple" }));
    await waitFor(() => expect(mocks.getSettings).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      expect(setMode).toHaveBeenLastCalledWith("light");
      expect(setColorTheme).toHaveBeenLastCalledWith("acid-warning");
    });
  });

  it("未知主题 ID 规范化后再预览与保存", async () => {
    const { result, setColorTheme } = setup();
    result.current.markSettings({ ...DEFAULT_SETTINGS });
    act(() => result.current.applyAppearance({ colorTheme: "not-exist" }));
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(1));
    expect(setColorTheme).toHaveBeenCalledWith("warm-editorial");
    expect(mocks.updateSettings.mock.calls[0][0].colorTheme).toBe("warm-editorial");
  });

  it("基准设置缺失时先向后端读取", async () => {
    const { result } = setup();
    mocks.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, colorTheme: "electric-cyan" });
    act(() => result.current.applyAppearance({ mode: "dark" }));
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(1));
    expect(mocks.getSettings).toHaveBeenCalledTimes(1);
    const saved = mocks.updateSettings.mock.calls[0][0];
    expect(saved.darkMode).toBe(2);
    expect(saved.colorTheme).toBe("electric-cyan");
  });
});
