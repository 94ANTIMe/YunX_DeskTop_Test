import { memo, useCallback, useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ArrowRight,
  CheckCircle2,
  ChevronRight,
  Loader2,
  Pause,
  Play,
  Sparkles,
  Square,
  XCircle,
} from "lucide-react";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import CrossDriveSearchModal from "../components/CrossDriveSearchModal";
import DownloadSummary from "../components/DownloadSummary";
import TaskDetailDrawer from "../components/TaskDetailDrawer";
import ConfirmDialog from "../components/ConfirmDialog";
import { errMsg, ipc, onDownloadsUpdated, type DownloadTask } from "../lib/ipc";
import { formatBytes, formatRemain, formatSpeed, platformLabel } from "../lib/format";
import type { TabId } from "../lib/tabs";
import emptyArt from "../assets/art/empty-downloads.jpg";

interface DownloadPageProps {
  /** 当前 Tab 是否为下载页（非激活时事件只暂存不渲染，隐藏页零重渲染） */
  active: boolean;
  onNavigate: (tab: TabId) => void;
  onGoResolve?: (url: string, pwd?: string) => void;
}

/** 任务状态语义 */
const STATUS_TEXT: Record<number, string> = {
  0: "排队中",
  1: "下载中",
  2: "已暂停",
  3: "已完成",
  4: "失败",
};

/** 速度采样点数（约 40 秒窗口，1s 一采） */
const SPEED_POINTS = 40;

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

/** 事件任务合并：无变化复用旧对象引用（memo 行组件与抽屉才能跳过重渲染）；整体无变化返回 null（调用方跳过 setState） */
function mergeTasks(prev: DownloadTask[], updated: DownloadTask[]): DownloadTask[] | null {
  let changed = false;
  const byId = new Map(prev.map((t) => [t.id, t]));
  for (const t of updated) {
    const old = byId.get(t.id);
    if (!old || !taskEquals(old, t)) {
      byId.set(t.id, t);
      changed = true;
    }
  }
  if (!changed) return null;
  return [...byId.values()].sort((a, b) => b.id - a.id);
}

interface TaskCardProps {
  task: DownloadTask;
  busy: boolean;
  onOpen: (id: number) => void;
  onPause: (id: number) => void;
  onResume: (id: number) => void;
  onRemove: (id: number) => void;
  onSearch: (filename: string) => void;
}

/** 单条任务卡片（memo：任一任务进度变化只重渲染该行，不再整列表 reconcile） */
const TaskCard = memo(function TaskCard({
  task,
  busy,
  onOpen,
  onPause,
  onResume,
  onRemove,
  onSearch,
}: TaskCardProps) {
  const pct =
    task.totalSize > 0
      ? Math.min(100, Math.round((task.downloadedSize / task.totalSize) * 100))
      : 0;
  const done = task.status === 3;
  const failed = task.status === 4;
  return (
    <section
      onClick={() => onOpen(task.id)}
      className="animate-rise cursor-pointer rounded-card bg-carrier p-5 transition-shadow hover:shadow-capsule"
      title="点击查看任务详情 Dashboard"
    >
      <div className="flex items-center gap-3">
        {/* 状态图标 */}
        {task.status === 1 ? (
          <Loader2 size={18} className="shrink-0 animate-spin text-clay" />
        ) : done ? (
          <CheckCircle2 size={18} className="shrink-0 text-success" />
        ) : failed ? (
          <XCircle size={18} className="shrink-0 text-clay-deep" />
        ) : (
          <Pause size={18} className="shrink-0 text-ink-soft" />
        )}
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-ink" title={task.fileName}>
            {task.fileName}
          </p>
          <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-ink-soft">
            <span className="rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px]">
              {platformLabel(task.platform)}
            </span>
            <span className={done ? "text-success" : failed ? "text-danger" : ""}>
              {STATUS_TEXT[task.status] ?? "未知"}
            </span>
            {task.totalSize > 0 && (
              <span className="font-mono">
                {formatBytes(task.downloadedSize)} / {formatBytes(task.totalSize)}
              </span>
            )}
            {task.status === 1 && task.speed > 0 && (
              <span className="font-mono text-clay-deep">{formatSpeed(task.speed)}</span>
            )}
            {task.status === 1 && task.totalSize > 0 && task.speed > 0 && (
              <span>剩余 {formatRemain(task.totalSize, task.downloadedSize, task.speed)}</span>
            )}
            {failed && task.errorMsg && (
              <span className="max-w-md truncate text-clay-deep" title={task.errorMsg}>
                {task.errorMsg}
              </span>
            )}
          </div>
        </div>

        {/* 操作（阻断行点击冒泡） */}
        <div
          className="flex shrink-0 items-center gap-1.5"
          onClick={(e) => e.stopPropagation()}
        >
          {done && task.savePath && (
            <button
              onClick={() => revealItemInDir(task.savePath).catch(() => {})}
              className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
              title={task.savePath}
            >
              打开位置
            </button>
          )}
          {(task.status === 1 || task.status === 0) && (
            <button
              onClick={() => onPause(task.id)}
              disabled={busy}
              className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-40"
              title="暂停该任务（可继续）"
            >
              <Pause size={13} />
              暂停
            </button>
          )}
          {(task.status === 2 || task.status === 4) && (
            <button
              onClick={() => onResume(task.id)}
              disabled={busy}
              className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-success hover:text-success disabled:opacity-40"
              title="继续该任务"
            >
              <Play size={13} />
              继续
            </button>
          )}
          <button
            onClick={() => onRemove(task.id)}
            disabled={busy}
            className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:bg-clay/10 hover:text-clay-deep disabled:opacity-40"
            title="取消该任务并移除记录"
          >
            <Square size={12} />
            取消
          </button>
          {/* 跨网盘搜同款 */}
          <button
            onClick={() => onSearch(task.fileName)}
            className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay"
            title="在夸克/UC/123等不限速网盘搜同款资源"
          >
            <Sparkles size={12} className="text-clay" />
            搜同款
          </button>
          {/* 抽屉入口指示 */}
          <ChevronRight size={14} className="ml-1 shrink-0 text-ink-soft/50" />
        </div>
      </div>

      {/* 进度条 */}
      {!done && (
        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-carrier-deep">
          <div
            className={`h-full rounded-full transition-all duration-300 ${
              failed ? "bg-clay-deep" : "bg-clay"
            }`}
            style={{ width: `${pct}%` }}
          />
        </div>
      )}
      {done && (
        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-success/40">
          <div className="h-full w-full rounded-full bg-success" />
        </div>
      )}
      {task.status === 0 && pct === 0 && (
        <p className="mt-2 text-[11px] text-ink-soft/70">等待空闲下载位…</p>
      )}
    </section>
  );
});

/** 下载页：聚合摘要条 + 任务列表 + 右侧详情抽屉（事件驱动 + 全量兜底） */
function DownloadPage({ active, onNavigate, onGoResolve }: DownloadPageProps) {
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<number | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [searchModalFilename, setSearchModalFilename] = useState<string | null>(null);
  // 速度采样历史（id → 最近 N 个速度点；事件驱动写入，页面隐藏时也持续记录）
  const speedHistory = useRef<Map<number, number[]>>(new Map());
  // 非激活期间暂存最新事件快照，激活时一次性 flush
  const pendingTasks = useRef<DownloadTask[] | null>(null);
  const activeRef = useRef(active);

  // 初始全量 + 事件实时更新（合并：全量为底，事件覆盖同 id；终态由后端事件流保留 24h）
  useEffect(() => {
    let mounted = true;
    ipc
      .listDownloadTasks()
      .then((list) => mounted && setTasks(list))
      .catch(() => {});
    const un = onDownloadsUpdated((updated) => {
      // 速度采样（仅进行中任务）
      for (const t of updated) {
        if (t.status === 1) {
          const arr = speedHistory.current.get(t.id) ?? [];
          arr.push(t.speed);
          if (arr.length > SPEED_POINTS) arr.shift();
          speedHistory.current.set(t.id, arr);
        }
      }
      // 页面隐藏：只暂存快照，不触发渲染
      if (!activeRef.current) {
        pendingTasks.current = updated;
        return;
      }
      // 无变化不 setTasks（后端已带变更检测，此处兜底合并层再滤一次）
      setTasks((prev) => mergeTasks(prev, updated) ?? prev);
    });
    return () => {
      mounted = false;
      un.then((f) => f());
    };
  }, []);

  // 页面重新激活：flush 暂存的事件快照
  useEffect(() => {
    activeRef.current = active;
    if (active && pendingTasks.current) {
      const pending = pendingTasks.current;
      pendingTasks.current = null;
      setTasks((prev) => mergeTasks(prev, pending) ?? prev);
    }
  }, [active]);

  const act = useCallback(async (id: number, fn: () => Promise<void>) => {
    setBusyId(id);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }, []);

  /** 全部暂停 */
  const pauseAll = useCallback(async () => {
    setError("");
    try {
      await ipc.pauseAllDownloads();
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  /** 全部继续 */
  const resumeAll = useCallback(async () => {
    setError("");
    try {
      await ipc.resumeAllDownloads();
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  // 一键清空全部任务记录（确认后执行；统计报表数据独立留存不受影响）
  const clearAll = useCallback(async () => {
    setError("");
    try {
      await ipc.clearDownloadTasks();
      setTasks([]);
      setOpenId(null);
      speedHistory.current.clear();
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  // 传给 memo 行组件 / 子组件的稳定回调（避免引用变化击穿 memo）
  const openTaskById = useCallback((id: number) => setOpenId(id), []);
  const pauseTask = useCallback((id: number) => void act(id, () => ipc.pauseDownload(id)), [act]);
  const resumeTask = useCallback((id: number) => void act(id, () => ipc.resumeDownload(id)), [act]);
  const removeTask = useCallback(
    (id: number) =>
      void act(id, async () => {
        await ipc.removeDownloadTask(id, false);
        setTasks((list) => list.filter((x) => x.id !== id));
      }),
    [act],
  );
  const searchSameFile = useCallback((filename: string) => setSearchModalFilename(filename), []);
  const openConfirmClear = useCallback(() => setConfirmClear(true), []);

  const openTask = openId != null ? (tasks.find((t) => t.id === openId) ?? null) : null;

  return (
    <div className="space-y-6">
      <PageHeader tab="download" subtitle="aria2 分片下载 · 断点续传" />

      {error && (
        <div className="rounded-ctrl bg-danger/10 px-4 py-2.5 text-sm text-danger">{error}</div>
      )}

      {/* 聚合摘要条（原内嵌 Dashboard 的总览部分上收到页级） */}
      {tasks.length > 0 && (
        <DownloadSummary
          tasks={tasks}
          onPauseAll={pauseAll}
          onResumeAll={resumeAll}
          onClearAll={openConfirmClear}
        />
      )}

      <div className="space-y-3">
        {tasks.length === 0 ? (
          <div className="rounded-card bg-carrier">
            <EmptyState
              image={emptyArt}
              title="暂无下载任务"
              description="解析分享链接或从网盘页选择文件后，下载任务将在这里排队。"
              action={
                <button
                  onClick={() => onNavigate("resolve")}
                  className="flex items-center gap-1.5 rounded-ctrl bg-clay px-5 py-2 text-sm font-semibold text-on-accent transition-colors hover:bg-clay-deep"
                >
                  去解析页添加
                  <ArrowRight size={15} />
                </button>
              }
            />
          </div>
        ) : (
          tasks.map((task) => (
            <TaskCard
              key={task.id}
              task={task}
              busy={busyId === task.id}
              onOpen={openTaskById}
              onPause={pauseTask}
              onResume={resumeTask}
              onRemove={removeTask}
              onSearch={searchSameFile}
            />
          ))
        )}
      </div>

      {/* 任务详情抽屉（右侧滑入，单例） */}
      <TaskDetailDrawer
        task={openTask}
        history={openId != null ? (speedHistory.current.get(openId) ?? []) : []}
        onClose={() => setOpenId(null)}
      />

      {/* 清空记录确认（破坏性操作） */}
      <ConfirmDialog
        open={confirmClear}
        title="清空全部下载记录？"
        description="将取消进行中的任务并删除全部下载记录（含已完成），本地文件不会被删除。统计报表数据不受影响。"
        confirmText="全部取消"
        danger
        onConfirm={() => {
          setConfirmClear(false);
          void clearAll();
        }}
        onCancel={() => setConfirmClear(false)}
      />

      {/* 跨网盘搜同款弹窗 */}
      <CrossDriveSearchModal
        open={Boolean(searchModalFilename)}
        filename={searchModalFilename || ""}
        onClose={() => setSearchModalFilename(null)}
        onResolveShare={(url, pwd) => {
          setSearchModalFilename(null);
          onGoResolve?.(url, pwd);
        }}
      />
    </div>
  );
}

export default memo(DownloadPage);
