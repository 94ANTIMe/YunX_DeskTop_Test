import { useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  CheckCircle2,
  FolderOpen,
  Loader2,
  X,
  XCircle,
} from "lucide-react";
import { ipc, type DownloadDetail, type DownloadTask } from "../lib/ipc";
import { formatBytes, formatDate, formatRemain, formatSpeed, platformLabel } from "../lib/format";

interface TaskDetailDrawerProps {
  /** 当前展开详情的任务（null = 关闭） */
  task: DownloadTask | null;
  /** 最近 ~40 个速度采样点（页面事件流维护） */
  history: number[];
  onClose: () => void;
}

/** 任务状态语义（与后端 DownloadTaskView 状态常量对齐） */
const STATUS_TEXT: Record<number, string> = {
  0: "排队中",
  1: "下载中",
  2: "已暂停",
  3: "已完成",
  4: "失败",
};

/** 环形进度 */
function ProgressRing({ pct, failed }: { pct: number; failed: boolean }) {
  const size = 92;
  const r = (size - 10) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="shrink-0">
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" strokeWidth="7" className="stroke-ink/10" />
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
    <div className="rounded-ctrl bg-carrier px-3.5 py-2.5">
      <p className="text-[10px] tracking-wide text-ink-soft/70">{label}</p>
      <p className={`mt-1 truncate text-sm text-ink ${mono ? "font-mono" : ""}`} title={value}>
        {value || "—"}
      </p>
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

/** 任务详情抽屉：右侧滑入固定面板（点击行 / Esc / 遮罩均可关闭），承载原内嵌 Dashboard 全部内容 */
export default function TaskDetailDrawer({ task, history, onClose }: TaskDetailDrawerProps) {
  const [shown, setShown] = useState<DownloadTask | null>(task);
  const [leaving, setLeaving] = useState(false);
  const [detail, setDetail] = useState<DownloadDetail | null>(null);
  const shownRef = useRef<DownloadTask | null>(shown);
  shownRef.current = shown;

  // 进出动画：任务出现 → 立即展示；任务变 null → 先播放滑出再卸载（同路径进出）
  useEffect(() => {
    if (task) {
      setShown(task);
      setLeaving(false);
    } else if (shownRef.current) {
      setLeaving(true);
    }
  }, [task]);

  useEffect(() => {
    if (!leaving) return;
    const timer = window.setTimeout(() => {
      setShown(null);
      setLeaving(false);
    }, 220);
    return () => window.clearTimeout(timer);
  }, [leaving]);

  // Esc 关闭
  useEffect(() => {
    if (!shown) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, onClose]);

  // 展开期间每 2s 轮询详情扩展字段（连接数 / 耗时 / 上传速度）
  useEffect(() => {
    if (!task) return;
    let alive = true;
    const load = () => {
      ipc
        .getDownloadDetail(task.id)
        .then((d) => alive && setDetail(d))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 2000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [task?.id]);

  if (!shown) return null;

  const t = shown;
  const pct =
    t.totalSize > 0 ? Math.min(100, Math.round((t.downloadedSize / t.totalSize) * 100)) : 0;
  const failed = t.status === 4;
  const done = t.status === 3;

  // 实时字段取任务事件流，扩展字段取 2s 轮询（切换任务时按 id 兜底防串台）
  const connections = detail && detail.id === t.id ? detail.connections : 0;
  const uploadSpeed = detail && detail.id === t.id ? detail.uploadSpeed : 0;
  const totalTime = detail && detail.id === t.id ? detail.totalTime : 0;
  const url = t.url || (detail && detail.id === t.id ? detail.url : "");
  const createTime = t.createTime || (detail && detail.id === t.id ? detail.createTime : 0);

  return (
    <div className="fixed inset-0 z-[60]">
      {/* 遮罩：点击关闭 */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm animate-fade" onClick={onClose} />

      <aside
        role="dialog"
        aria-modal="true"
        className={`absolute inset-y-0 right-0 flex w-[440px] max-w-[92vw] flex-col border-l border-ink/10 bg-carrier shadow-2xl ${
          leaving ? "animate-drawer-out" : "animate-drawer-in"
        }`}
      >
        {/* 头部 */}
        <header className="flex shrink-0 items-center justify-between border-b border-ink/10 px-5 py-4">
          <div className="flex items-center gap-2">
            <p className="font-mono text-[11px] tracking-[0.3em] text-ink-soft">TASK · DASHBOARD</p>
          </div>
          <button
            onClick={onClose}
            className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
            title="关闭详情 (Esc)"
          >
            <X size={16} />
          </button>
        </header>

        {/* 内容 */}
        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          {/* 环形进度 + 文件信息 + 大号速度 */}
          <div className="flex items-center gap-5">
            <ProgressRing pct={pct} failed={failed} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                  {platformLabel(t.platform)}
                </span>
                <span
                  className={`flex items-center gap-1 text-xs font-medium ${
                    done ? "text-success" : failed ? "text-danger" : "text-ink-soft"
                  }`}
                >
                  {t.status === 1 ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : done ? (
                    <CheckCircle2 size={12} />
                  ) : failed ? (
                    <XCircle size={12} />
                  ) : null}
                  {STATUS_TEXT[t.status] ?? "未知"}
                </span>
              </div>
              <p className="mt-1.5 break-all text-sm font-medium text-ink">{t.fileName}</p>
              <p className="mt-1 font-mono text-xs text-ink-soft">
                {formatBytes(t.downloadedSize)} / {formatBytes(t.totalSize)}
              </p>
            </div>
            <div className="shrink-0 text-right">
              <p className="font-mono text-xl font-semibold text-clay-deep">
                {t.status === 1 ? formatSpeed(t.speed) || "—" : done ? "已完成" : "—"}
              </p>
              {t.status === 1 && t.speed > 0 && t.totalSize > 0 && (
                <p className="mt-1 text-[11px] text-ink-soft">
                  剩余 {formatRemain(t.totalSize, t.downloadedSize, t.speed)}
                </p>
              )}
            </div>
          </div>

          {/* 指标网格 */}
          <div className="mt-4 grid grid-cols-2 gap-2">
            <Metric label="分片连接数" value={String(connections)} />
            <Metric label="上传速度" value={uploadSpeed > 0 ? formatSpeed(uploadSpeed) : "0 B/s"} />
            <Metric label="已耗时" value={totalTime > 0 ? `${totalTime} s` : "—"} />
            <Metric label="平均速度" value={avgSpeedText(t, totalTime)} />
          </div>

          {/* 速度曲线 */}
          <div className="mt-4">
            <div className="flex items-center justify-between">
              <p className="text-[10px] tracking-wide text-ink-soft/70">速度曲线（最近 ~40 秒）</p>
              <p className="font-mono text-[10px] text-ink-soft/70">
                峰值 {formatSpeed(Math.max(...(history.length ? history : [0]), 0)) || "—"}
              </p>
            </div>
            <div className="mt-1 rounded-ctrl bg-carrier-deep/60 px-3 py-2">
              <SpeedSpark points={history} />
            </div>
          </div>

          {/* 路径与链接 */}
          <dl className="mt-4 space-y-2 text-xs">
            {t.savePath && (
              <div className="flex items-start gap-2">
                <dt className="w-16 shrink-0 pt-0.5 text-ink-soft/70">保存至</dt>
                <dd className="min-w-0 flex-1">
                  <span className="break-all font-mono text-[11px] text-ink-soft" title={t.savePath}>
                    {t.savePath}
                  </span>
                  {done && (
                    <button
                      onClick={() => revealItemInDir(t.savePath).catch(() => {})}
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
            {failed && t.errorMsg && (
              <div className="flex items-start gap-2">
                <dt className="w-16 shrink-0 pt-0.5 text-clay-deep/80">错误</dt>
                <dd className="min-w-0 flex-1 break-all rounded-ctrl bg-clay/10 px-2.5 py-1.5 font-mono text-[11px] text-clay-deep">
                  {t.errorMsg}
                </dd>
              </div>
            )}
          </dl>
        </div>

        {/* 底部 */}
        <footer className="flex shrink-0 justify-end border-t border-ink/10 px-5 py-3">
          <button
            onClick={onClose}
            className="rounded-ctrl border border-ink/15 px-3 py-1 text-[11px] font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
          >
            收起详情
          </button>
        </footer>
      </aside>
    </div>
  );
}
