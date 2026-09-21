import { memo, useCallback, useState } from "react";
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
import ConfirmDialog from "../components/ui/ConfirmDialog";
import { errMsg, ipc, type DownloadTask } from "../lib/ipc";
import { formatBytes, formatRemain, formatSpeed, platformLabel } from "../lib/format";
import { clearLocalTasks, forgetTask, getSpeedHistory, useDownloadsState } from "../hooks/useDownloads";
import {
  statusText,
  STATUS_COMPLETED,
  STATUS_DOWNLOADING,
  STATUS_FAILED,
  STATUS_PAUSED,
  STATUS_PENDING,
} from "../lib/download-status";
import Skeleton from "../components/ui/Skeleton";
import type { TabId } from "../lib/tabs";
import emptyArt from "../assets/art/empty-downloads.jpg";

interface DownloadPageProps {
  /** 当前 Tab 是否为下载页（非激活时事件只暂存不渲染，隐藏页零重渲染） */
  active: boolean;
  onNavigate: (tab: TabId) => void;
  onGoResolve?: (url: string, pwd?: string) => void;
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
  const done = task.status === STATUS_COMPLETED;
  const failed = task.status === STATUS_FAILED;
  return (
    <section
      onClick={() => onOpen(task.id)}
      className="animate-rise cursor-pointer rounded-card bg-carrier p-5 transition-shadow hover:shadow-capsule"
      title="点击查看任务详情 Dashboard"
    >
      <div className="flex items-center gap-3">
        {/* 状态图标 */}
        {task.status === STATUS_DOWNLOADING ? (
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
              {statusText(task.status)}
            </span>
            {task.totalSize > 0 && (
              <span className="font-mono">
                {formatBytes(task.downloadedSize)} / {formatBytes(task.totalSize)}
              </span>
            )}
            {task.status === STATUS_DOWNLOADING && task.speed > 0 && (
              <span className="font-mono text-clay-deep">{formatSpeed(task.speed)}</span>
            )}
            {task.status === STATUS_DOWNLOADING && task.totalSize > 0 && task.speed > 0 && (
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
          {(task.status === STATUS_DOWNLOADING || task.status === STATUS_PENDING) && (
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
          {(task.status === STATUS_PAUSED || task.status === STATUS_FAILED) && (
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

      {/* 进度条（scaleX 合成层动画，避免每秒 width 变化触发布局重排） */}
      {!done && (
        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-carrier-deep">
          <div
            className={`h-full w-full origin-left rounded-full transition-transform duration-300 ${
              failed ? "bg-clay-deep" : "bg-clay"
            }`}
            style={{ transform: `scaleX(${pct / 100})` }}
          />
        </div>
      )}
      {done && (
        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-success/40">
          <div className="h-full w-full rounded-full bg-success" />
        </div>
      )}
      {task.status === STATUS_PENDING && pct === 0 && (
        <p className="mt-2 text-[11px] text-ink-soft/70">等待空闲下载位…</p>
      )}
    </section>
  );
});

/** 下载页：聚合摘要条 + 任务列表 + 右侧详情抽屉（store 单例驱动，事件合并/采样在 useDownloads） */
function DownloadPage({ active, onNavigate, onGoResolve }: DownloadPageProps) {
  // 隐藏时快照恒定（零重渲染），激活即恢复最新任务列表
  const { tasks, loaded } = useDownloadsState(active);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<number | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [searchModalFilename, setSearchModalFilename] = useState<string | null>(null);

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
      clearLocalTasks();
      setOpenId(null);
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  // 传给 memo 行组件 / 子组件的稳定回调（避免引用变化击穿 memo）
  const openTaskById = useCallback((id: number) => setOpenId(id), []);
  const pauseTask = useCallback((id: number) => void act(id, () => ipc.pauseDownload(id)), [act]);
  const resumeTask = useCallback((id: number) => void act(id, () => ipc.resumeDownload(id)), [act]);
  const removeTask = useCallback(
    (id: number) => setConfirmCancelId(id),
    [],
  );
  const doRemoveTask = useCallback(
    (id: number) =>
      void act(id, async () => {
        await ipc.removeDownloadTask(id, false);
        forgetTask(id);
        setConfirmCancelId(null);
      }),
    [act],
  );
  const searchSameFile = useCallback((filename: string) => setSearchModalFilename(filename), []);
  const [confirmCancelId, setConfirmCancelId] = useState<number | null>(null);
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
        {!loaded ? (
          [0, 1, 2].map((i) => (
            <div key={i} className="rounded-card bg-carrier p-5">
              <div className="flex items-center gap-3">
                <Skeleton className="size-5 rounded-full" />
                <div className="min-w-0 flex-1 space-y-2">
                  <Skeleton className="h-3.5 w-1/3" />
                  <Skeleton className="h-2.5 w-1/4" />
                </div>
              </div>
              <Skeleton className="mt-4 h-1.5 w-full" rounded="full" />
            </div>
          ))
        ) : tasks.length === 0 ? (
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
        history={openId != null ? getSpeedHistory(openId) : []}
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

      {/* 取消单个任务确认 */}
      <ConfirmDialog
        open={confirmCancelId != null}
        danger
        title="取消这个下载任务？"
        description="将中断下载并移除任务记录；已下载的部分文件保留在本地。"
        confirmText="取消任务"
        onConfirm={() => {
          if (confirmCancelId != null) void doRemoveTask(confirmCancelId);
        }}
        onCancel={() => setConfirmCancelId(null)}
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
