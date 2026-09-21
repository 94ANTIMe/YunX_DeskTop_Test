import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DownloadTask } from "../lib/ipc";

// 捕获 onDownloadsUpdated 的事件回调，初始全量返回空列表
let pushEvent: (tasks: DownloadTask[]) => void = () => {};
vi.mock("../lib/ipc", () => ({
  ipc: { listDownloadTasks: vi.fn(() => Promise.resolve([])) },
  onDownloadsUpdated: vi.fn((cb: (tasks: DownloadTask[]) => void) => {
    pushEvent = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("../lib/toast", () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

import { toast } from "../lib/toast";
import { clearLocalTasks, forgetTask, getDownloadsState, subscribeDownloadState } from "./useDownloads";

function taskOf(id: number, status: number, speed = 0): DownloadTask {
  return {
    id, gid: `g${id}`, url: "", fileName: `文件${id}`, platform: "quark",
    totalSize: 100, downloadedSize: status === 3 ? 100 : 50, speed, status,
    errorMsg: status === 4 ? "aria2 错误码 1" : "", savePath: "", createTime: id,
  };
}

describe("useDownloads 单例 store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearLocalTasks();
  });

  it("派生统计：进行中计数（0/1/2）与下载中速度总和（仅 status 1）", async () => {
    const unsub = subscribeDownloadState(() => {});
    pushEvent([taskOf(1, 1, 500), taskOf(2, 0), taskOf(3, 2), taskOf(4, 3)]);
    const s = getDownloadsState();
    expect(s.loaded).toBe(true);
    expect(s.tasks).toHaveLength(4);
    expect(s.activeCount).toBe(3);
    expect(s.totalSpeed).toBe(500);
    unsub();
  });

  it("终态迁移：完成弹成功 toast，失败弹带原因的错误 toast", () => {
    const unsub = subscribeDownloadState(() => {});
    pushEvent([taskOf(1, 1), taskOf(2, 1)]);
    expect(toast.success).not.toHaveBeenCalled();
    pushEvent([taskOf(1, 3), taskOf(2, 4)]);
    expect(toast.success).toHaveBeenCalledWith("下载完成：文件1");
    expect(toast.error).toHaveBeenCalledWith("下载失败：文件2（aria2 错误码 1）");
    // 非迁移（同一状态重复推送）不再弹
    vi.mocked(toast.success).mockClear();
    pushEvent([taskOf(1, 3)]);
    expect(toast.success).not.toHaveBeenCalled();
    unsub();
  });

  it("无变化事件不产生新数组引用（memo 行组件可跳过重渲染）", () => {
    const unsub = subscribeDownloadState(() => {});
    pushEvent([taskOf(1, 1)]);
    const before = getDownloadsState().tasks;
    pushEvent([taskOf(1, 1)]);
    expect(getDownloadsState().tasks).toBe(before);
    unsub();
  });

  it("forgetTask / clearLocalTasks 本地同步", () => {
    const unsub = subscribeDownloadState(() => {});
    pushEvent([taskOf(1, 3), taskOf(2, 4)]);
    forgetTask(1);
    expect(getDownloadsState().tasks.map((t) => t.id)).toEqual([2]);
    clearLocalTasks();
    expect(getDownloadsState().tasks).toHaveLength(0);
    expect(getDownloadsState().loaded).toBe(true);
    unsub();
  });
});
