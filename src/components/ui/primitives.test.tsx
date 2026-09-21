import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import Button from "./Button";
import IconButton from "./IconButton";
import Toggle from "./Toggle";
import Select from "./Select";
import SliderRow from "./SliderRow";
import Skeleton from "./Skeleton";

describe("ui/Button", () => {
  it("默认 type=button 防表单误提交；variant/size 类生效", () => {
    render(<Button variant="primary" size="lg">保存</Button>);
    const btn = screen.getByRole("button", { name: "保存" });
    expect(btn).toHaveAttribute("type", "button");
    expect(btn.className).toContain("bg-clay");
    expect(btn.className).toContain("px-5");
  });

  it("disabled 档位：primary 50 / outline 40，且 hover 类挂 enabled:", () => {
    render(
      <>
        <Button variant="primary" disabled>实底</Button>
        <Button variant="outline" disabled>描边</Button>
      </>,
    );
    const solid = screen.getByRole("button", { name: "实底" });
    const line = screen.getByRole("button", { name: "描边" });
    expect(solid.className).toContain("disabled:opacity-50");
    expect(line.className).toContain("disabled:opacity-40");
    expect(solid.className).toContain("enabled:hover:bg-clay-deep");
  });
});

describe("ui/IconButton", () => {
  it("aria-label 必传可用作可达性名称", () => {
    const onClick = vi.fn();
    render(
      <IconButton aria-label="刷新" onClick={onClick}>
        <span>↻</span>
      </IconButton>,
    );
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    expect(onClick).toHaveBeenCalledOnce();
  });
});

describe("ui/Toggle", () => {
  it("switch 语义与点击翻转", () => {
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} aria-label="自动更新" />);
    const el = screen.getByRole("switch", { name: "自动更新" });
    expect(el).toHaveAttribute("aria-checked", "false");
    fireEvent.click(el);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("sm 尺寸应用紧凑轨道", () => {
    render(<Toggle checked onChange={() => {}} size="sm" aria-label="小开关" />);
    expect(screen.getByRole("switch", { name: "小开关" }).className).toContain("h-5 w-9");
  });
});

describe("ui/Select", () => {
  it("渲染选项并回调选中值", () => {
    const onChange = vi.fn();
    render(
      <Select
        value="http"
        aria-label="代理类型"
        options={[
          { value: "http", label: "HTTP" },
          { value: "socks5", label: "SOCKS5" },
        ]}
        onChange={onChange}
      />,
    );
    fireEvent.change(screen.getByRole("combobox", { name: "代理类型" }), {
      target: { value: "socks5" },
    });
    expect(onChange).toHaveBeenCalledWith("socks5");
  });
});

describe("ui/SliderRow", () => {
  it("拖动即时 onInput，松手 / 失焦才 onCommit（值变化才提交）", () => {
    const onInput = vi.fn();
    const onCommit = vi.fn();
    render(
      <SliderRow label="并发" min={1} max={5} value={2} onInput={onInput} onCommit={onCommit} />,
    );
    const slider = screen.getByRole("slider", { name: "并发" });
    fireEvent.change(slider, { target: { value: "4" } });
    expect(onInput).toHaveBeenCalledWith(4);
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.mouseUp(slider, { target: { value: "4" } });
    expect(onCommit).toHaveBeenCalledWith(4);
    // 松手 / 失焦时值未变化（回到受控值）→ 不重复提交
    onCommit.mockClear();
    fireEvent.blur(slider, { target: { value: "2" } });
    expect(onCommit).not.toHaveBeenCalled();
  });
});

describe("ui/Skeleton", () => {
  it("对读屏隐藏且带 reduced-motion 降级", () => {
    const { container } = render(<Skeleton className="h-4 w-40" />);
    const el = container.firstChild as HTMLElement;
    expect(el.getAttribute("aria-hidden")).toBe("true");
    expect(el.className).toContain("motion-reduce:animate-none");
  });
});
