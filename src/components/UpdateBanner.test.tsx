import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import UpdateBanner from "./UpdateBanner";
import { friendlyUpdateError } from "../hooks/useUpdate";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(() => Promise.resolve()) }));

const base = {
  latestVersion: "0.6.0",
  currentVersion: "0.5.1",
  notes: "新版本说明",
  downloading: false,
  installing: false,
  progress: null,
  error: "",
  onUpdate: vi.fn(),
  onDismiss: vi.fn(),
  onCancel: vi.fn(),
};

describe("UpdateBanner", () => {
  it("展示当前版本 → 新版本迁移与立即更新入口", () => {
    render(<UpdateBanner {...base} />);
    // 版本文案跨两个文本节点，按元素 textContent 匹配最内层 span
    expect(
      screen.getByText((_, element) => element?.tagName === "SPAN" && element.textContent === "v0.5.1 → v0.6.0"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /立即更新/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "稍后再说" })).toBeInTheDocument();
  });

  it("下载中展示字节进度与百分比，稍后 = 取消下载", () => {
    render(
      <UpdateBanner
        {...base}
        downloading
        progress={{ received: 5 * 1024 * 1024, total: 20 * 1024 * 1024 }}
      />,
    );
    expect(screen.getByText(/\/ 20\.0 MB · 25%/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消下载" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /立即更新/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "取消下载" }));
    expect(base.onCancel).toHaveBeenCalledTimes(1);
    expect(base.onDismiss).not.toHaveBeenCalled();
  });

  it("下载失败：显示可读错误、提供发布页兜底与重试更新", () => {
    render(
      <UpdateBanner
        {...base}
        error="网络连接失败，请检查网络或代理后重试，或前往发布页手动下载"
        rawError="error sending request"
        browserUrl="https://github.com/94ANTIMe/YunX_DeskTop_Test/releases/latest"
      />,
    );
    expect(screen.getByText(/网络连接失败/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /发布页/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /重试更新/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /重试更新/ }));
    expect(base.onUpdate).toHaveBeenCalledTimes(1);
  });

  it("friendlyUpdateError 错误映射（网络 / 超时 / 签名 / 兜底透传）", () => {
    expect(friendlyUpdateError("error sending request for url (https://github.com/)")).toMatch(/网络连接失败/);
    expect(friendlyUpdateError("operation timed out")).toMatch(/超时/);
    expect(friendlyUpdateError("Signature verification failed")).toMatch(/签名校验失败/);
    expect(friendlyUpdateError("其它未知错误")).toBe("其它未知错误");
  });
});
