import { useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ArrowRight,
  CheckCircle2,
  ChevronDown,
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
import { errMsg, ipc, onDownloadsUpdated, type DownloadDetail, type DownloadTask } from "../lib/ipc";
import { formatBytes, formatDate, formatRemain, formatSpeed, platformLabel } from "../lib/format";
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

/** 速度采样点数（约 40 秒窗口，1s 一采） */
const SPEED_POINTS = 40;

/** 环形进度 */
function ProgressRing({ pct, failed }: { pct: number; failed: boolean }) {
  const size = 92;
  const r = (size - 10) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="shrink-0">
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        strokeWidth="7"
        className="stroke-ink/10"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        strokeWidth="7"
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - Math.min(100, pct) / 100)}
        transform={`rotate(-90 ${size / 2} ${size / 2})`}
        className={failed ? "stroke-clay-deep" : pct >= 100 ? "stroke-cactus" : "stroke-clay"}
        style={{ transition: "stroke-dashoffset 0.6s ease" }}
      />
      <text
        x="50%"
        y="50%"
        textAnchor="middle"
        dominantBaseline="central"
        className="fill-ink font-mono text-sm font-semibold"
      >
        {pct}%
      </text>
    </svg>
  );
}

/** 速度 sparkline（最近 ~40 个采样点） */
function SpeedSpark({ points }: { points: number[] }) {
  const w = 320;
  const h = 56;
  if (points.length < 2) {
    return (
      <p className="flex h-14 items-center justify-center text-[11px] text-ink-soft/60">
        速度采样中…（每秒记录一次）
      </p>
    );
  }
  const max = Math.max(...points, 1);
  const step = w / (points.length - 1);
  const d = points
    .map(
      (p, i) =>
        `${i === 0 ? "M" : "L"}${(i * step).toFixed(1)},${(h - 4 - (p / max) * (h - 10)).toFixed(1)}`
    )
    .join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" className="h-14 w-full">
      <path d={d} fill="none" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" className="stroke-clay" />
    </svg>
  );
}

/** 指标格 */
function Metric({ label, value, mono = true }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-ctrl bg-carrier-deep px-3.5 py-2.5">
      <p className="text-[10px] tracking-wide text-ink-soft/70">{label}</p>
      <p
        className={`mt-1 truncate text-sm text-ink ${mono ? "font-mono" : ""}`}
        title={value}
      >
        {value || "—"}
      </p>
    </div>
  );
}

/** 下载页：aria2 任务实时列表（事件驱动 + 全量兜底）+ 任务 Dashboard 详情 */
export default function DownloadPage({ onNavigate }: DownloadPageProps) {
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [error, setError] = useState("");
  const [busyId, setBusyId] = useState<number | null>(null);
  // Dashboard：展开的任务 id + 详情快照（连接数/上传速度/耗时/URL 等扩展字段）
  const [openId, setOpenId] = useState<number | null>(null);
  const [detail, setDetail] = useState<DownloadDetail | null>(null);
  // 速度采样历史（id → 最近 N 个速度点；事件驱动写入）
  const speedHistory = useRef<Map<number, number[]>>(new Map());

  // 初始全量 + 事件实时更新（合并：全量为底，事件覆盖同 id）
  useEffect(() => {
    let mounted = true;
    ipc
      .listDownloadTasks()
      .then((list) => mounted && setTasks(list))
      .catch(() => {});
    const un = onDownloadsUpdated((active) => {
      // 速度采样（仅进行中任务）
      const now = Date.now();
      for (const t of active) {
        if (t.status === 1) {
          const arr = speedHistory.current.get(t.id) ?? [];
          arr.push(t.speed);
          if (arr.length > SPEED_POINTS) arr.shift();
          speedHistory.current.set(t.id, arr);
        }
      }
      setTasks((prev) => {
        const map = new Map(prev.map((t) => [t.id, t]));
        for (const t of active) map.set(t.id, t);
        // 事件里消失的任务保留原记录（暂停/完成的兜底显示）
        return [...map.values()].sort((a, b) => b.id - a.id);
      });
      void now;
    });
    return () => {
      mounted = false;
      un.then((f) => f());
    };
  }, []);

  // Dashboard 打开期间每 2s 刷新详情扩展字段
  useEffect(() => {
    if (openId == null) return;
    let alive = true;
    const load = () => {
      ipc
        .getDownloadDetail(openId)
        .then((d) => alive && setDetail(d))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 2000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [openId]);

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

  /** 展开/收起任务 Dashboard */
  function toggleDetail(id: number) {
    if (openId === id) {
      setOpenId(null);
      setDetail(null);
    } else {
      setOpenId(id);
      setDetail(null);
      ipc
        .getDownloadDetail(id)
        .then(setDetail)
        .catch(() => {});
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
      setOpenId(null);
      setDetail(null);
      speedHistory.current.clear();
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
            const open = openId === task.id;
            return (
              <section key={task.id} className="animate-rise rounded-card bg-carrier p-5">
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
                    {/* Dashboard 开关 */}
                    <button
                      onClick={() => toggleDetail(task.id)}
                      className={`rounded-ctrl border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                        open
                          ? "border-clay bg-clay/10 text-clay-deep"
                          : "border-ink/15 text-ink-soft hover:border-clay hover:text-clay-deep"
                      }`}
                      title="展开任务详情 Dashboard"
                    >
                      详情
                      <ChevronDown
                        size={12}
                        className={`ml-1 inline transition-transform ${open ? "rotate-180" : ""}`}
                      />
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

                {/* 任务 Dashboard（点「详情」展开） */}
                {open && (
                  <TaskDashboard
                    task={task}
                    detail={detail && detail.id === task.id ? detail : null}
                    history={speedHistory.current.get(task.id) ?? []}
                    onClose={() => {
                      setOpenId(null);
                      setDetail(null);
                    }}
                  />
                )}
              </section>
            );
          })
        )}
      </div>
    </div>
  );
}

/** 任务详情 Dashboard：环形进度 + 指标网格 + 速度曲线 */
function TaskDashboard({
  task,
  detail,
  history,
  onClose,
}: {
  task: DownloadTask;
  detail: DownloadDetail | null;
  history: number[];
  onClose: () => void;
}) {
  const pct =
    task.totalSize > 0
      ? Math.min(100, Math.round((task.downloadedSize / task.totalSize) * 100))
      : 0;
  const failed = task.status === 4;
  const done = task.status === 3;

  // 实时字段取 task（事件流），扩展字段取 detail（2s 轮询）
  const connections = detail?.connections ?? 0;
  const uploadSpeed = detail?.uploadSpeed ?? 0;
  const totalTime = detail?.totalTime ?? 0;
  const url = detail?.url ?? task.url ?? "";
  const createTime = detail?.createTime || task.createTime;

  return (
    <div className="mt-4 animate-rise rounded-card border border-ink/10 bg-carrier-deep/60 p-4">
      {/* 头部：环形进度 + 文件信息 + 大号速度 */}
      <div className="flex items-center gap-5">
        <ProgressRing pct={pct} failed={failed} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="rounded-full bg-carrier px-2 py-0.5 font-mono text-[10px] text-ink-soft">
              {platformLabel(task.platform)}
            </span>
            <span
              className={`text-xs font-medium ${
                done ? "text-cactus" : failed ? "text-clay-deep" : "text-ink-soft"
              }`}
            >
              {STATUS_TEXT[task.status] ?? "未知"}
            </span>
          </div>
          <p className="mt-1.5 break-all text-sm font-medium text-ink">{task.fileName}</p>
          <p className="mt-1 font-mono text-xs text-ink-soft">
            {formatBytes(task.downloadedSize)} / {formatBytes(task.totalSize)}
          </p>
        </div>
        <div className="shrink-0 text-right">
          <p className="font-mono text-xl font-semibold text-clay-deep">
            {task.status === 1 ? formatSpeed(task.speed) || "—" : done ? "已完成" : "—"}
          </p>
          {task.status === 1 && task.speed > 0 && task.totalSize > 0 && (
            <p className="mt-1 text-[11px] text-ink-soft">
              剩余 {formatRemain(task.totalSize, task.downloadedSize, task.speed)}
            </p>
          )}
        </div>
      </div>

      {/* 指标网格 */}
      <div className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Metric label="分片连接数" value={String(connections)} />
        <Metric label="上传速度" value={uploadSpeed > 0 ? formatSpeed(uploadSpeed) : "0 B/s"} />
        <Metric label="已耗时" value={totalTime > 0 ? `${totalTime} s` : "—"} />
        <Metric label="平均速度" value={avgSpeedText(task, totalTime)} />
      </div>

      {/* 速度曲线 */}
      <div className="mt-4">
        <div className="flex items-center justify-between">
          <p className="text-[10px] tracking-wide text-ink-soft/70">速度曲线（最近 ~40 秒）</p>
          <p className="font-mono text-[10px] text-ink-soft/70">
            峰值 {formatSpeed(Math.max(...(history.length ? history : [0]), 0)) || "—"}
          </p>
        </div>
        <div className="mt-1 rounded-ctrl bg-carrier px-3 py-2">
          <SpeedSpark points={history} />
        </div>
      </div>

      {/* 路径与链接 */}
      <dl className="mt-4 space-y-2 text-xs">
        {task.savePath && (
          <div className="flex items-start gap-2">
            <dt className="w-16 shrink-0 pt-0.5 text-ink-soft/70">保存至</dt>
            <dd className="min-w-0 flex-1">
              <span className="break-all font-mono text-[11px] text-ink-soft" title={task.savePath}>
                {task.savePath}
              </span>
              {done && (
                <button
                  onClick={() => revealItemInDir(task.savePath).catch(() => {})}
                  className="ml-2 inline-flex items-center gap-1 rounded-ctrl border border-ink/15 px-2 py-0.5 text-[10px] font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
                >
                  <FolderOpen size={11} />
                  打开
                </button>
              )}
            </dd>
          </div>
        )}
        {url && (
          <div className="flex items-start gap-2">
            <dt className="w-16 shrink-0 pt-0.5 text-ink-soft/70">下载源</dt>
            <dd className="min-w-0 flex-1 break-all font-mono text-[11px] text-ink-soft" title={url}>
              {url.length > 96 ? `${url.slice(0, 96)}…` : url}
            </dd>
          </div>
        )}
        {createTime > 0 && (
          <div className="flex items-start gap-2">
            <dt className="w-16 shrink-0 pt-0.5 text-ink-soft/70">创建于</dt>
            <dd className="min-w-0 flex-1 font-mono text-[11px] text-ink-soft">
              {formatDate(createTime)}
            </dd>
          </div>
        )}
        {failed && task.errorMsg && (
          <div className="flex items-start gap-2">
            <dt className="w-16 shrink-0 pt-0.5 text-clay-deep/80">错误</dt>
            <dd className="min-w-0 flex-1 break-all rounded-ctrl bg-clay/10 px-2.5 py-1.5 font-mono text-[11px] text-clay-deep">
              {task.errorMsg}
            </dd>
          </div>
        )}
      </dl>

      <div className="mt-4 flex justify-end">
        <button
          onClick={onClose}
          className="rounded-ctrl border border-ink/15 px-3 py-1 text-[11px] font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
        >
          收起详情
        </button>
      </div>
    </div>
  );
}

/** 平均速度文本（下载量 / 已耗时） */
function avgSpeedText(task: DownloadTask, totalTime: number): string {
  if (totalTime > 0 && task.downloadedSize > 0) {
    return formatSpeed(Math.round(task.downloadedSize / totalTime));
  }
  return "—";
}
