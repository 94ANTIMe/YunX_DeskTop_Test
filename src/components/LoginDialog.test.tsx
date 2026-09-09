import { fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LoginDialog from "./LoginDialog";

const { webLoginStart, webLoginCancel } = vi.hoisted(() => ({
  webLoginStart: vi.fn(() => Promise.resolve()),
  webLoginCancel: vi.fn(() => Promise.resolve()),
}));

vi.mock("../lib/ipc", () => ({
  errMsg: (error: unknown) => String(error),
  ipc: { webLoginStart, webLoginCancel },
  onLoginSuccess: () => Promise.resolve(() => {}),
}));

describe("LoginDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("遮罩和 Esc 都走取消登录链路", async () => {
    const onClose = vi.fn();
    const first = render(<LoginDialog platform="quark" onClose={onClose} onSuccess={vi.fn()} />);
    fireEvent.click(first.container.firstElementChild!);
    await waitFor(() => expect(webLoginCancel).toHaveBeenCalledWith("quark"));
    expect(onClose).toHaveBeenCalledOnce();
    first.unmount();

    const second = render(<LoginDialog platform="quark" onClose={onClose} onSuccess={vi.fn()} />);
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(webLoginCancel).toHaveBeenCalledTimes(2));
    second.unmount();
  });
});
