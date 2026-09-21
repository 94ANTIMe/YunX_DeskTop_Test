/**
 * 下载任务状态语义的单一来源（对齐 Rust models.rs `DownloadTaskView::STATUS_*`）。
 * 状态文案、常量、图标选型一律从这里取，禁止再写魔法数字。
 */

export const STATUS_PENDING = 0;
export const STATUS_DOWNLOADING = 1;
export const STATUS_PAUSED = 2;
export const STATUS_COMPLETED = 3;
export const STATUS_FAILED = 4;

export const STATUS_TEXT: Record<number, string> = {
  [STATUS_PENDING]: "排队中",
  [STATUS_DOWNLOADING]: "下载中",
  [STATUS_PAUSED]: "已暂停",
  [STATUS_COMPLETED]: "已完成",
  [STATUS_FAILED]: "失败",
};

export function statusText(status: number): string {
  return STATUS_TEXT[status] ?? "未知";
}
