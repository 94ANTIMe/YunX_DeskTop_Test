/**
 * 全局 Toast 单例（模块级状态 + useSyncExternalStore 订阅，同 useUpdate 模式）。
 * 队列上限 4（挤掉最旧）、自动消失、同类（kind+message 相同）去重只刷新计时。
 * 渲染端见 components/ToastHost.tsx；下载完成/失败的自动 toast 由 useDownloads store 触发。
 */

export type ToastKind = "success" | "error" | "info";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const MAX_VISIBLE = 4;
const AUTO_DISMISS_MS = 4000;

let items: ToastItem[] = [];
let seq = 1;
const listeners = new Set<() => void>();
const timers = new Map<number, number>();

function emit() {
  for (const l of listeners) l();
}

function scheduleDismiss(id: number) {
  const timer = window.setTimeout(() => dismissToast(id), AUTO_DISMISS_MS);
  timers.set(id, timer);
}

/** 关闭单条 toast（自动消失 / 手动点关闭共用） */
export function dismissToast(id: number) {
  const timer = timers.get(id);
  if (timer != null) {
    window.clearTimeout(timer);
    timers.delete(id);
  }
  if (!items.some((t) => t.id === id)) return;
  items = items.filter((t) => t.id !== id);
  emit();
}

function push(kind: ToastKind, message: string) {
  if (!message) return -1;
  // 同类去重：已有相同 kind+message 的条目 → 复用并重置自动消失计时
  const existing = items.find((t) => t.kind === kind && t.message === message);
  if (existing) {
    const timer = timers.get(existing.id);
    if (timer != null) window.clearTimeout(timer);
    scheduleDismiss(existing.id);
    emit();
    return existing.id;
  }
  const id = seq++;
  items = [...items, { id, kind, message }].slice(-MAX_VISIBLE);
  // 被挤掉的可能是任意最旧条目，统一清理孤儿计时器
  const alive = new Set(items.map((t) => t.id));
  for (const [tid, timer] of timers) {
    if (!alive.has(tid)) {
      window.clearTimeout(timer);
      timers.delete(tid);
    }
  }
  scheduleDismiss(id);
  emit();
  return id;
}

export const toast = {
  success: (message: string) => push("success", message),
  error: (message: string) => push("error", message),
  info: (message: string) => push("info", message),
};

export function subscribeToasts(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getToasts(): ToastItem[] {
  return items;
}
