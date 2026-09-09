import { useMemo } from "react";
import { Pause, Play, Trash2 } from "lucide-react";
import type { DownloadTask } from "../lib/ipc";
import { formatSpeed } from "../lib/format";

interface DownloadSummaryProps {
  tasks: DownloadTask[];
  onPauseAll: () => void;
  onResumeAll: () => void;
  onClearAll: () => void;
}

/** 聚合摘要条：总速度 + 状态计数 + 全局操作（原 PageHeader 角落汇总升级为页级 Dashboard） */
export default function DownloadSummary({ tasks, onPauseAll, onResumeAll, onClearAll }: DownloadSummaryProps) {
  // 统计 memo 化：tasks 引用不变时跳过 200 行 × 5 次遍历重算
  const { active, pending, done, failed, totalSpeed } = useMemo(() => {
    const active = tasks.filter((t) => t.status === 1);
    const pending = tasks.filter((t) => t.status === 0);
    const done = tasks.filter((t) => t.status === 3);
    const failed = tasks.filter((t) => t.status === 4);
    const totalSpeed = active.reduce((s, t) => s + t.speed, 0);
    return { active, pending, done, failed, totalSpeed };
  }, [tasks]);

  const chips = [
    { label: "进行中", value: active.length, dot: "bg-clay" },
    { label: "排队", value: pending.length, dot: "bg-ink-soft/60" },
    { label: "已完成", value: done.length, dot: "bg-success" },
    { label: "失败", value: failed.length, dot: "bg-clay-deep" },
  ];

  return (
    <div className="animate-rise rounded-card bg-carrier p-5">
      <div className="flex flex-wrap items-center gap-x-8 gap-y-4">
        {/* 总速度 */}
        <div className="shrink-0">
          <p className="font-mono text-2xl font-semibold text-ink">
            {formatSpeed(totalSpeed) || "—"}
          </p>
          <p className="mt-0.5 text-[11px] text-ink-soft">
            总速度 · 共 {tasks.length} 个任务
          </p>
        </div>

        {/* 状态计数 */}
        <div className="flex flex-wrap items-center gap-2">
          {chips.map((c) => (
            <span
              key={c.label}
              className="flex items-center gap-1.5 rounded-full bg-carrier-deep px-3 py-1 text-[11px] text-ink-soft"
            >
              <span className={`size-1.5 rounded-full ${c.dot}`} />
              {c.label}
              <span className="font-mono font-semibold text-ink">{c.value}</span>
            </span>
          ))}
        </div>

        {/* 全局操作 */}
        <div className="ml-auto flex items-center gap-2">
          <button
            onClick={onPauseAll}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
            title="暂停全部进行中 / 排队任务"
          >
            <Pause size={13} />
            全部暂停
          </button>
          <button
            onClick={onResumeAll}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
            title="继续全部暂停任务"
          >
            <Play size={13} />
            全部继续
          </button>
          <button
            onClick={onClearAll}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay-deep hover:bg-clay/10 hover:text-clay-deep"
            title="取消并移除全部下载任务记录"
          >
            <Trash2 size={13} />
            全部取消
          </button>
        </div>
      </div>
    </div>
  );
}
