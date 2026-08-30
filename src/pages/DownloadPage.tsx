import { useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ArrowRight,
  CheckCircle2,
  FolderOpen,
  Loader2,
  Pause,
  Play,
  RotateCcw,
  Trash2,
  XCircle,
} from "lucide-react";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import { errMsg, ipc, onDownloadsUpdated, type DownloadTask } from "../lib/ipc";
import { formatBytes, formatRemain, formatSpeed, platformLabel } from "../lib/format";
import type { TabId } from "../lib/tabs";
import emptyArt from "../assets/art/empty-downloads.jpg";

interface DownloadPageProps {
  onNavigate: (tab: TabId) => void;
}

/** 任务状态语义 */
const STATUS_TEXT: Record<number, string> = {
  0: "排队中",
  1: "下载中",
  2: "已暂停",
  3: "已完成",
  4: "失败",
};

/** 下载页：aria2 任务实时列表（事件驱动 + 全量兜底） */
export default function DownloadPage({ onNavigate }: DownloadPageProps) {
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [error, setError] = useState("");
  const [busyId, setBusyId] = useState<number | null>(null);

  // 初始全量 + 事件实时更新（合并：全量为底，事件覆盖同 id）
  useEffect(() => {
    let mounted = true;
    ipc
      .listDownloadTasks()
      .then((list) => mounted && setTasks(list))
      .catch(() => {});
    const un = onDownloadsUpdated((active) => {
      setTasks((prev) => {
        const map = new Map(prev.map((t) => [t.id, t]));
        for (const t of active) map.set(t.id, t);
        // 事件里消失的任务保留原记录（暂停/完成的兜底显示）
        return [...map.values()].sort((a, b) => b.id - a.id);
      });
    });
    return () => {
      mounted = false;
      un.then((f) => f());
    };
  }, []);

  async function act(id: number, fn: () => Promise<void>) {
    setBusyId(id);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }

  const active = tasks.filter((t) => t.status === 1);
  const totalSpeed = active.reduce((s, t) => s + t.speed, 0);

  // 一键清空全部任务记录
  async function clearAll() {
    setError("");
    try {
      await ipc.clearDownloadTasks();
      setTasks([]);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader tab="download" subtitle="aria2 分片下载 · 断点续传">
        {tasks.length > 0 && (
          <div className="flex items-center justify-end gap-4">
            <div className="text-right">
              <p className="font-mono text-sm text-ink">{formatSpeed(totalSpeed) || "—"}</p>
              <p className="mt-0.5 text-[11px] text-ink-soft">
                {active.length} 个任务进行中 · 共 {tasks.length} 个
              </p>
            </div>
            <button
              onClick={clearAll}
              className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
              title="一键删除全部下载任务记录"
            >
              <Trash2 size={13} />
              清空记录
            </button>
          </div>
        )}
      </PageHeader>

      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
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
                  className="flex items-center gap-1.5 rounded-ctrl bg-clay px-5 py-2 text-sm font-semibold text-white transition-colors hover:bg-clay-deep"
                >
                  去解析页添加
                  <ArrowRight size={15} />
                </button>
              }
            />
          </div>
        ) : (
          tasks.map((task) => {
            const pct =
              task.totalSize > 0
                ? Math.min(100, Math.round((task.downloadedSize / task.totalSize) * 100))
                : 0;
            const done = task.status === 3;
            const failed = task.status === 4;
            return (
              <section
                key={task.id}
                className="animate-rise rounded-card bg-carrier p-5"
              >
                <div className="flex items-center gap-3">
                  {/* 状态图标 */}
                  {task.status === 1 ? (
                    <Loader2 size={18} className="shrink-0 animate-spin text-clay" />
                  ) : done ? (
                    <CheckCircle2 size={18} className="shrink-0 text-cactus" />
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
                      <span className={done ? "text-cactus" : failed ? "text-clay-deep" : ""}>
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

                  {/* 操作 */}
                  <div className="flex shrink-0 items-center gap-1.5">
                    {done && task.savePath && (
                      <button
                        onClick={() => revealItemInDir(task.savePath).catch(() => {})}
                        className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
                        title={task.savePath}
                      >
                        <FolderOpen size={13} />
                        打开位置
                      </button>
                    )}
                    {(task.status === 1 || task.status === 0) && (
                      <button
                        onClick={() => act(task.id, () => ipc.pauseDownload(task.id))}
                        disabled={busyId === task.id}
                        className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink disabled:opacity-40"
                        title="暂停"
                      >
                        <Pause size={15} />
                      </button>
                    )}
                    {(task.status === 2 || task.status === 4) && (
                      <button
                        onClick={() => act(task.id, () => ipc.resumeDownload(task.id))}
                        disabled={busyId === task.id}
                        className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink disabled:opacity-40"
                        title="继续"
                      >
                        <Play size={15} />
                      </button>
                    )}
                    {failed && (
                      <button
                        onClick={() => act(task.id, async () => {
                          await ipc.removeDownloadTask(task.id, false);
                          setTasks((t) => t.filter((x) => x.id !== task.id));
                        })}
                        disabled={busyId === task.id}
                        className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink disabled:opacity-40"
                        title="移除失败任务"
                      >
                        <RotateCcw size={15} />
                      </button>
                    )}
                    <button
                      onClick={() => act(task.id, async () => {
                        await ipc.removeDownloadTask(task.id, false);
                        setTasks((t) => t.filter((x) => x.id !== task.id));
                      })}
                      disabled={busyId === task.id}
                      className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-clay/10 hover:text-clay-deep disabled:opacity-40"
                      title="删除任务"
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>

                {/* 进度条 */}
                {!done && (
                  <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-carrier-deep">
                    <div
                      className={`h-full rounded-full transition-all duration-300 ${
                        failed ? "bg-clay-deep" : "bg-clay"
                      }`}
                      style={{ width: `${task.status === 2 ? pct : pct}%` }}
                    />
                  </div>
                )}
                {done && (
                  <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-cactus/40">
                    <div className="h-full w-full rounded-full bg-cactus" />
                  </div>
                )}
                {task.status === 0 && pct === 0 && (
                  <p className="mt-2 text-[11px] text-ink-soft/70">等待空闲下载位…</p>
                )}
              </section>
            );
          })
        )}
      </div>
    </div>
  );
}
