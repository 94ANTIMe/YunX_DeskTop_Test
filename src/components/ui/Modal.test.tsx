import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Modal from "./Modal";
import ConfirmDialog from "./ConfirmDialog";

afterEach(() => {
  document.body.style.overflow = "";
});

describe("ui/Modal", () => {
  it("open 时渲染 dialog 语义并锁定 body 滚动，关闭后还原", () => {
    const { rerender } = render(
      <Modal open onClose={() => {}} title="标题">
        <p>内容</p>
      </Modal>,
    );
    expect(screen.getByRole("dialog", { name: "标题" })).toHaveAttribute("aria-modal", "true");
    expect(document.body.style.overflow).toBe("hidden");
    rerender(
      <Modal open={false} onClose={() => {}} title="标题">
        <p>内容</p>
      </Modal>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(document.body.style.overflow).toBe("");
  });

  it("Esc 关闭可配：默认开；closeOnEsc=false 不响应", () => {
    const onClose = vi.fn();
    const { rerender } = render(<Modal open onClose={onClose} title="A" />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
    rerender(<Modal open onClose={onClose} title="A" closeOnEsc={false} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("点遮罩关闭、点面板不关闭", () => {
    const onClose = vi.fn();
    render(<Modal open onClose={onClose} title="B"><button>内按钮</button></Modal>);
    fireEvent.click(screen.getByRole("dialog", { name: "B" }));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("dialog", { name: "B" }).parentElement!);
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe("ui/ConfirmDialog", () => {
  it("danger 时用 alertdialog 语义；确认 / 取消回调正确", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        open
        danger
        title="清空记录？"
        description="不可恢复"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "确认" }));
    expect(onConfirm).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
