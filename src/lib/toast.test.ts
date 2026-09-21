import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dismissToast, getToasts, subscribeToasts, toast } from "./toast";

describe("toast 单例", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    getToasts().slice().forEach((t) => dismissToast(t.id));
  });

  it("入队后可在订阅中读到，自动消失后清空", () => {
    const seen: number[] = [];
    const unsub = subscribeToasts(() => seen.push(getToasts().length));
    toast.success("已保存");
    expect(getToasts()).toHaveLength(1);
    expect(getToasts()[0].message).toBe("已保存");
    vi.advanceTimersByTime(4100);
    expect(getToasts()).toHaveLength(0);
    expect(seen.length).toBeGreaterThanOrEqual(2);
    unsub();
  });

  it("同类（kind+message 相同）去重只保留一条并重置计时", () => {
    vi.useFakeTimers();
    toast.error("下载失败：a.exe");
    vi.advanceTimersByTime(3000);
    toast.error("下载失败：a.exe");
    expect(getToasts()).toHaveLength(1);
    // 计时已重置：再过 3s（距最后一次入队）仍应存活
    vi.advanceTimersByTime(3000);
    expect(getToasts()).toHaveLength(1);
    vi.advanceTimersByTime(1200);
    expect(getToasts()).toHaveLength(0);
  });

  it("队列上限 4：超出时挤掉最旧", () => {
    for (const m of ["一", "二", "三", "四", "五", "六"]) toast.info(m);
    const messages = getToasts().map((t) => t.message);
    expect(messages).toEqual(["三", "四", "五", "六"]);
  });

  it("手动 dismiss 立即移除；空消息不入队", () => {
    const id = toast.success("手动关闭");
    expect(getToasts()).toHaveLength(1);
    dismissToast(id);
    expect(getToasts()).toHaveLength(0);
    expect(toast.info("")).toBe(-1);
  });
});
