import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ResolvePage from "./ResolvePage";
import type { ResolveSessionInfo, ShareFilePage } from "../lib/ipc";

const { resolveShare, listShareFiles } = vi.hoisted(() => ({
  resolveShare: vi.fn<() => Promise<ResolveSessionInfo>>(() => Promise.resolve({ sessionKey: "session", platform: "quark", title: "测试", files: [], hasMore: false })),
  listShareFiles: vi.fn<() => Promise<ShareFilePage>>(() => Promise.resolve({ files: [], hasMore: false })),
}));
vi.mock("../lib/ipc", () => ({
  errMsg: (error: unknown) => String(error),
  ipc: { resolveShare, listShareFiles, listResolveHistory: () => Promise.resolve([]) },
}));

describe("ResolvePage", () => {
  it("把手填提取码作为独立 IPC 参数发送", async () => {
    render(<ResolvePage onNavigate={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText(/粘贴网盘分享链接/), { target: { value: "https://pan.quark.cn/s/abc" } });
    fireEvent.change(screen.getByPlaceholderText("提取码（可自动识别）"), { target: { value: "9xyz" } });
    fireEvent.click(screen.getByRole("button", { name: "解析" }));
    await waitFor(() => expect(resolveShare).toHaveBeenCalledWith("https://pan.quark.cn/s/abc", "9xyz"));
  });

  it("目录展开和进入复用一次请求并替换为完整面包屑", async () => {
    const folder = { fid: "a", fname: "A", fsize: 0, isdir: true, pdirFid: "0", fidToken: "", modifyTime: "" };
    const child = { fid: "b", fname: "B.txt", fsize: 1, isdir: false, pdirFid: "a", fidToken: "", modifyTime: "" };
    resolveShare.mockResolvedValueOnce({ sessionKey: "session", platform: "quark", title: "根", files: [folder], hasMore: false });
    listShareFiles.mockResolvedValueOnce({ files: [child], hasMore: false });
    render(<ResolvePage onNavigate={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText(/粘贴网盘分享链接/), { target: { value: "https://pan.quark.cn/s/abc" } });
    fireEvent.click(screen.getByRole("button", { name: "解析" }));
    await screen.findByText("已解析：根");
    fireEvent.click(screen.getByRole("button", { name: "目录树" }));
    fireEvent.click(screen.getByRole("button", { name: "展开目录" }));
    await screen.findByText("B.txt");
    fireEvent.click(screen.getByTitle("进入 A"));
    await waitFor(() => expect(listShareFiles).toHaveBeenCalledTimes(1));
    expect(screen.getAllByRole("button", { name: "根" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "A" }).length).toBeGreaterThan(0);
  });
});
