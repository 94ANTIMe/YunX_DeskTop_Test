/** 字节数人性化（1024 进制） */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log2(bytes) / 10), units.length - 1);
  const value = bytes / 1024 ** i;
  return `${i === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[i]}`;
}

/** 速度（字节/秒） */
export function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "";
  return `${formatBytes(bytesPerSec)}/s`;
}

/** 时间戳 → 本地日期 */
export function formatDate(ms: number): string {
  if (!ms) return "";
  return new Date(ms).toLocaleString("zh-CN", { hour12: false });
}

/** 平台标识 → 中文名 */
export const PLATFORM_LABELS: Record<string, string> = {
  quark: "夸克",
  uc: "UC",
  xunlei: "迅雷",
  baidu: "百度",
  c139: "139",
  pan123: "123",
};

export function platformLabel(platform: string): string {
  return PLATFORM_LABELS[platform] ?? platform;
}

/** 秒 → 剩余时间 */
export function formatRemain(total: number, downloaded: number, speed: number): string {
  if (speed <= 0 || total <= downloaded) return "";
  const seconds = Math.ceil((total - downloaded) / speed);
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分 ${seconds % 60} 秒`;
  return `${Math.floor(minutes / 60)} 时 ${minutes % 60} 分`;
}
