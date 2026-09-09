import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";
import { DEFAULT_SETTINGS, type Settings as SettingsT, type UpdateSettingsResult } from "../lib/ipc";
import { COLOR_THEMES } from "../lib/themes";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn<() => Promise<SettingsT>>(),
  updateSettings: vi.fn<(_s: SettingsT) => Promise<UpdateSettingsResult>>(() =>
    Promise.resolve({ engineSyncFailed: false, engineSyncError: null }),
  ),
  getAppInfo: vi.fn<() => Promise<{ version: string }>>(() => Promise.resolve({ version: "0.5.1" })),
  onSettingsUpdated: vi.fn<() => Promise<() => void>>(() => Promise.resolve(() => {})),
}));

vi.mock("../lib/ipc", async (loadOriginal) => {
  const original = await loadOriginal<typeof import("../lib/ipc")>();
  return {
    ...original,
    ipc: { ...original.ipc, ...mocks },
    onSettingsUpdated: mocks.onSettingsUpdated,
  };
});

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn(() => Promise.resolve(null)) }));

function renderPage(overrides?: { colorTheme?: string }) {
  const onAppearanceChange = vi.fn();
  render(
    <SettingsPage
      themeMode="system"
      colorTheme={overrides?.colorTheme ?? "warm-editorial"}
      onAppearanceChange={onAppearanceChange}
    />,
  );
  return { onAppearanceChange };
}

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.updateSettings.mockResolvedValue({ engineSyncFailed: false, engineSyncError: null });
  });

  it("首次读取失败时显示错误并可重试", async () => {
    mocks.getSettings.mockRejectedValueOnce(new Error("settings.json 已损坏")).mockResolvedValueOnce(DEFAULT_SETTINGS);
    renderPage();
    expect(await screen.findByText("设置读取失败")).toBeInTheDocument();
    expect(screen.getByText(/settings.json 已损坏/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => expect(screen.getByText("剪贴板监听")).toBeInTheDocument());
  });

  it("连续设置变更合并到最新快照并保持单飞", async () => {
    mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
    let releaseFirst!: () => void;
    mocks.updateSettings.mockImplementationOnce(
      () =>
        new Promise<UpdateSettingsResult>((resolve) => {
          releaseFirst = () => resolve({ engineSyncFailed: false, engineSyncError: null });
        }),
    );
    renderPage();
    const switches = await screen.findAllByRole("switch");
    fireEvent.click(switches[0]);
    fireEvent.click(switches[3]);
    expect(mocks.updateSettings).toHaveBeenCalledTimes(1);
    releaseFirst();
    await waitFor(() => expect(mocks.updateSettings).toHaveBeenCalledTimes(2));
    const latest = mocks.updateSettings.mock.calls[1][0];
    expect(latest.clipboardMonitor).toBe(true);
    expect(latest.autoLaunch).toBe(true);
  });

  it("下载引擎同步失败时保存成功：仅非阻塞提示，不报错、不回滚", async () => {
    // 首次读取返回默认值；保存后的回读对账返回已保存值（模拟真实后端行为）
    mocks.getSettings
      .mockResolvedValueOnce(DEFAULT_SETTINGS)
      .mockResolvedValue({ ...DEFAULT_SETTINGS, clipboardMonitor: true });
    mocks.updateSettings.mockResolvedValue({
      engineSyncFailed: true,
      engineSyncError: "限速 / 并发 / 代理同步失败，重启引擎后生效：下载引擎通信失败: connection refused",
    });
    renderPage();
    const switches = await screen.findAllByRole("switch");
    fireEvent.click(switches[0]);
    expect(await screen.findByText(/设置已保存；下载引擎暂未同步/)).toBeInTheDocument();
    expect(screen.getByText(/重启引擎后生效：下载引擎通信失败/)).toBeInTheDocument();
    // 不走错误 + 回读回滚路径
    expect(screen.queryByText(/已重新读取后端设置/)).not.toBeInTheDocument();
    await waitFor(() => expect(switches[0]).toHaveAttribute("aria-checked", "true"));
  });

  describe("外观：配色主题", () => {
    it("渲染七张主题预览卡（radio 语义），选中态来自 colorTheme 属性", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      renderPage({ colorTheme: "dream-candy" });
      const group = await screen.findByRole("radiogroup", { name: "配色主题" });
      const radios = within(group).queryAllByRole("radio");
      expect(radios).toHaveLength(COLOR_THEMES.length);
      const checked = within(group).getByRole("radio", { checked: true });
      expect(checked).toHaveAttribute("aria-label", "梦幻糖果");
      // 漫游 tabindex：仅选中卡可 Tab 进入
      expect(checked).toHaveAttribute("tabindex", "0");
      expect(within(group).getAllByRole("radio", { hidden: true }).filter((el) => el.getAttribute("tabindex") === "-1").length).toBe(COLOR_THEMES.length - 1);
    });

    it("未知主题 ID 回退：无选中卡也不报错", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      renderPage({ colorTheme: "not-exist-theme" });
      const group = await screen.findByRole("radiogroup", { name: "配色主题" });
      expect(within(group).queryAllByRole("radio", { checked: true })).toHaveLength(0);
      expect(screen.getAllByRole("radio").length).toBe(COLOR_THEMES.length);
    });

    it("点击主题卡即时预览（onAppearanceChange），不直接写设置", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      const { onAppearanceChange } = renderPage();
      const card = await screen.findByRole("radio", { name: "赛博霓虹" });
      fireEvent.click(card);
      expect(onAppearanceChange).toHaveBeenCalledWith({ colorTheme: "cyber-neon" });
      expect(mocks.updateSettings).not.toHaveBeenCalled();
    });

    it("明暗模式按钮走外观变更（保留 darkMode 数值语义由 App 映射）", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      const { onAppearanceChange } = renderPage();
      fireEvent.click(await screen.findByRole("button", { name: "深色" }));
      expect(onAppearanceChange).toHaveBeenCalledWith({ mode: "dark" });
    });
  });

  describe("开源致谢：原位折叠", () => {
    it("默认收起：列表链接退出可访问性树（aria-hidden + inert），标题带 aria-expanded / aria-controls", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      renderPage();
      const toggle = await screen.findByRole("button", { name: /开源致谢/ });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(toggle).toHaveAttribute("aria-controls", "ack-panel");
      // 面板收起：不可访问（隐藏），依赖 / 参考数文案可见
      const panel = document.getElementById("ack-panel");
      expect(panel).toHaveAttribute("data-open", "false");
      expect(panel).toHaveAttribute("aria-hidden", "true");
      expect(panel).toHaveAttribute("inert");
      expect(screen.getByText(/依赖 \/ 参考 6 个开源项目/)).toBeInTheDocument();
    });

    it("点击标题展开：aria-expanded 变 true，项目与外链全部可见可访问", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      renderPage();
      const toggle = await screen.findByRole("button", { name: /开源致谢/ });
      fireEvent.click(toggle);
      expect(toggle).toHaveAttribute("aria-expanded", "true");
      // 原有项目全部保留（byRole 查询验证已退出隐藏状态；^ 锚定项目名避免命中描述文本）
      for (const name of [/^aria2/, /^BaiduPCS-Go/, /^PanSou/, /^TrackersListCollection/, /^TurboDL/, /^YunX/]) {
        expect(screen.getByRole("button", { name })).toBeInTheDocument();
      }
      const panel = document.getElementById("ack-panel");
      expect(panel).toHaveAttribute("data-open", "true");
      expect(panel).toHaveAttribute("aria-hidden", "false");
    });

    it("展开后可再次收起，收起后链接重新进入隐藏状态", async () => {
      mocks.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
      renderPage();
      const toggle = await screen.findByRole("button", { name: /开源致谢/ });
      fireEvent.click(toggle);
      expect(screen.getByRole("button", { name: /^aria2/ })).toBeInTheDocument();
      fireEvent.click(toggle);
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      await waitFor(() => {
        expect(document.getElementById("ack-panel")).toHaveAttribute("aria-hidden", "true");
      });
      expect(screen.queryByRole("button", { name: /^aria2/ })).not.toBeInTheDocument();
    });
  });
});
