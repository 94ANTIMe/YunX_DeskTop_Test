import { Download, ExternalLink, X } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { formatBytes } from "../lib/format";
import type { UpdateProgress } from "../lib/ipc";

interface UpdateBannerProps {
  latestVersion: string;
  currentVersion?: string;
  notes: string;
  downloading: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  error: string;
  /** 原始错误（title 悬浮展示完整信息） */
  rawError?: string;
  /** 发布页地址（下载失败时的手动兜底入口） */
  browserUrl?: string;
  onUpdate: () => void;
  onDismiss: () => void;
  /** 下载中点「稍后」= 取消下载（安装阶段不可中断） */
  onCancel?: () => void;
}

/** 顶部更新横幅：发现新版本时提示「立即更新 / 稍后」，展示下载进度与失败兜底。 */
export default function UpdateBanner({
  latestVersion,
  currentVersion,
  notes,
  downloading,
  installing,
  progress,
  error,
  rawError,
  browserUrl,
  onUpdate,
  onDismiss,
  onCancel,
}: UpdateBannerProps) {
  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.received / progress.total) * 100))
      : 0;

  return (
    <div className="animate-rise sticky top-4 z-10 mx-auto flex w-fit max-w-3xl items-center gap-3 rounded-ctrl border border-clay/25 bg-carrier px-4 py-2.5 shadow-sm">
      <div className="min-w-0">
        <p className="text-sm font-semibold text-ink">
          发现新版本{" "}
          <span className="font-mono text-clay-deep">
            {currentVersion ? `v${currentVersion} → ` : ""}v{latestVersion}
          </span>
          {notes && <span className="ml-2 hidden text-xs font-normal text-ink-soft sm:inline">{notes.slice(0, 40)}</span>}
        </p>
        {downloading && progress && (
          <div className="mt-1 flex w-72 items-center gap-2">
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-ink/10">
              <div
                className="h-full w-full origin-left rounded-full bg-clay transition-transform"
                style={{ transform: `scaleX(${pct / 100})` }}
              />
            </div>
            <span className="shrink-0 font-mono text-[11px] text-ink-soft">
              {progress.total > 0
                ? `${formatBytes(progress.received)} / ${formatBytes(progress.total)} · ${pct}%`
                : "准备中…"}
            </span>
          </div>
        )}
        {installing && <p className="mt-1 text-xs text-ink-soft">正在安装，应用将退出并自动重启…</p>}
        {error && (
          <p className="mt-1 max-w-xs truncate text-xs text-danger" title={rawError || error}>
            {error}
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {error && browserUrl && (
          <button
            onClick={() => openUrl(browserUrl).catch(() => {})}
            className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-2.5 py-1 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
          >
            <ExternalLink size={12} />
            发布页
          </button>
        )}
        {!downloading && !installing && (
          <button
            onClick={onUpdate}
            className="flex items-center gap-1 rounded-ctrl bg-clay px-3 py-1 text-xs font-medium text-on-accent transition-colors enabled:hover:bg-clay-deep"
          >
            <Download size={13} />
            {error ? "重试更新" : "立即更新"}
          </button>
        )}
        <button
          onClick={downloading ? onCancel : onDismiss}
          disabled={installing}
          className="rounded-ctrl p-1 text-ink-soft transition-colors hover:text-ink disabled:opacity-40"
          title={downloading ? "取消下载" : "稍后再说"}
        >
          <X size={15} />
        </button>
      </div>
    </div>
  );
}
