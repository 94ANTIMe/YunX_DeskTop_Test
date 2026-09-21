import { useSyncExternalStore } from "react";
import { ipc, onDownloadsUpdated, type DownloadTask } from "../lib/ipc";
import { toast } from "../lib/toast";
import {
  STATUS_COMPLETED,
  STATUS_DOWNLOADING,
  STATUS_FAILED,
  STATUS_PAUSED,
  STATUS_PENDING,
} from "../lib/download-status";

/**
 * 下载任务全局单例 store（useUpdate 同款模块级模式）：事件订阅、合并去重、速度采样、
 * 终态迁移检测各收编一份，多订阅者（下载页 / 顶部胶囊 / 完成失败 toast）共享零增量 IPC
 * ——Rust poll_loop 本就变更检测后才广播。初始全量拉取在首个订阅者出现时触发一次。
 */

interface DownloadsState {
  tasks: DownloadTask[];
  /** 初始全量是否已返回（供骨架屏区分「加载中」与「确为空」） */
  loaded: boolean;
  /** 进行中任务数（status 0|1|2，角标口径：排队也算占着队列） */
  activeCount: number;
  /** 下载中速度总和（status===1，与 DownloadSummary 同口径） */
  totalSpeed: number;
}

const EMPTY: DownloadTask[] = [];

let state: DownloadsState = { tasks: EMPTY, loaded: false, activeCount: 0, totalSpeed: 0 };

const listeners = new Set<() => void>();

function derive(tasks: DownloadTask[]): DownloadsState {
  let activeCount = 0;
  let totalSpeed = 0;
  for (const t of tasks) {
    if (t.status === STATUS_PENDING || t.status === STATUS_DOWNLOADING || t.status === STATUS_PAUSED) activeCount++;
    if (t.status === STATUS_DOWNLOADING) totalSpeed += t.speed;
  }
  return { tasks, loaded: state.loaded, activeCount, totalSpeed };
}

function setState(patch: Partial<DownloadsState>) {
  state = { ...state, ...patch };
  for (const l of listeners) l();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  start();
  return () => {
    listeners.delete(listener);
  };
}

// ---------- 事件合并 / 排序（原 DownloadPage 逻辑上收） ----------

/** 关键字段浅比较（id 相同的任务）：全部一致视为无变化 */
function taskEquals(a: DownloadTask, b: DownloadTask): boolean {
  return (
    a.status === b.status &&
    a.downloadedSize === b.downloadedSize &&
    a.totalSize === b.totalSize &&
    a.speed === b.speed &&
    a.errorMsg === b.errorMsg &&
    a.savePath === b.savePath
  );
}

/** 与后端一致的展示排序：进行中（0/1/2）优先，其后按 id 倒序 */
function compareTasks(a: DownloadTask, b: DownloadTask): number {
  const active = (s: number) => (s === 0 || s === 1 || s === 2 ? 0 : 1);
  return active(a.status) - active(b.status) || b.id - a.id;
}

/** 事件任务合并：无变化复用旧对象引用（memo 行组件与抽屉才能跳过重渲染）；整体无变化返回 null（调用方跳过 setState）。
 *  终态任务若从事件快照中消失（滑出 24h 窗口 / 已在别处删除）同步移除，避免留下进度冻结的僵尸行。 */
function mergeTasks(prev: DownloadTask[], updated: DownloadTask[]): DownloadTask[] | null {
  let changed = false;
  const byId = new Map(prev.map((t) => [t.id, t]));
  const seen = new Set<number>();
  for (const t of updated) {
    seen.add(t.id);
    const old = byId.get(t.id);
    if (!old || !taskEquals(old, t)) {
      byId.set(t.id, t);
      changed = true;
    }
  }
  for (const [id, t] of byId) {
    if (!seen.has(id) && (t.status === 3 || t.status === 4)) {
      byId.delete(id);
      changed = true;
    }
  }
  if (!changed) return null;
  return [...byId.values()].sort(compareTasks);
}

// ---------- 速度采样（原 DownloadPage 逻辑上收：页面隐藏时也持续记录） ----------

/** 速度采样点数（约 40 秒窗口，1s 一采） */
const SPEED_POINTS = 40;
const speedHistory = new Map<number, number[]>();
const NO_SPEED: number[] = [];

function sampleSpeeds(updated: DownloadTask[]) {
  for (const t of updated) {
    if (t.status === 1) {
      const arr = speedHistory.get(t.id) ?? [];
      arr.push(t.speed);
      if (arr.length > SPEED_POINTS) arr.shift();
      speedHistory.set(t.id, arr);
    }
  }
}

/** 任务详情抽屉用的速度历史（引用稳定，抽屉按需读取） */
export function getSpeedHistory(id: number): number[] {
  return speedHistory.get(id) ?? NO_SPEED;
}

// ---------- 终态迁移检测：完成 / 失败自动 toast ----------

function detectTransitions(prevTasks: DownloadTask[], updated: DownloadTask[]) {
    if (!state.loaded) return; // 首次全量不提示（存量终态不算新鲜事）
    const prevStatus = new Map(prevTasks.map((t) => [t.id, t.status]));
    for (const t of updated) {
      const prev = prevStatus.get(t.id);
      if (prev === undefined || prev === t.status) continue;
    if (t.status === STATUS_COMPLETED) {
      toast.success(`下载完成：${t.fileName}`);
    } else if (t.status === STATUS_FAILED) {
      toast.error(t.errorMsg ? `下载失败：${t.fileName}（${t.errorMsg}）` : `下载失败：${t.fileName}`);
    }
  }
}

// ---------- 启动（首个订阅者触发一次，应用生命周期内常驻） ----------

let started = false;

function start() {
  if (started) return;
  started = true;
  ipc
    .listDownloadTasks()
    .then((list) => setState({ ...derive(list), loaded: true }))
    .catch(() => setState({ loaded: true }));
  onDownloadsUpdated((updated) => {
    sampleSpeeds(updated);
    detectTransitions(state.tasks, updated);
    const merged = mergeTasks(state.tasks, updated);
    if (merged) setState(derive(merged));
  });
}

// ---------- 本地变更动作（页面操作后同步 store，不等下一轮事件） ----------

/** 任务已从后端移除：同步摘除（remove_task 后调用） */
export function forgetTask(id: number) {
  setState(derive(state.tasks.filter((t) => t.id !== id)));
}

/** 全部记录已清空：同步清空（clear_tasks 后调用；速度历史一并丢弃） */
export function clearLocalTasks() {
  speedHistory.clear();
  setState({ ...derive(EMPTY), loaded: true });
}

// ---------- Hooks ----------

function getSnapshot() {
  return state;
}

/** 读取当前状态快照（非 React 场景 / 测试用） */
export function getDownloadsState(): DownloadsState {
  return state;
}

/** 订阅状态变化（非 React 场景 / 测试用） */
export function subscribeDownloadState(listener: () => void): () => void {
  return subscribe(listener);
}

/** 全局订阅：顶栏指示器等始终需要最新数据的场景 */
export function useDownloads(): DownloadsState {
  return useSyncExternalStore(subscribe, getSnapshot);
}

const INERT: DownloadsState = { tasks: EMPTY, loaded: false, activeCount: 0, totalSpeed: 0 };

/**
 * 隐藏页友好订阅：active=false 时快照恒为 INERT（引用不变 → 隐藏页零重渲染），
 * 激活瞬间切回真实快照，无需再维护「隐藏期暂存事件、激活时 flush」的缓冲逻辑。
 */
export function useDownloadsState(active: boolean): DownloadsState {
  return useSyncExternalStore(subscribe, active ? getSnapshot : () => INERT);
}
