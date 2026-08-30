import { Download, X } from "lucide-react";
import type { UpdateProgress } from "../lib/ipc";

interface UpdateBannerProps {
  latestVersion: string;
  notes: string;
  downloading: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  error: string;
  onUpdate: () => void;
  onDismiss: () => void;
}

/** 顶部更新横幅：发现新版本时提示「立即更新 / 稍后」。 */
export default function UpdateBanner({
  latestVersion,
  notes,
  downloading,
  installing,
  progress,
  error,
  onUpdate,
  onDismiss,
}: UpdateBannerProps) {
  const busy = downloading || installing;
  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.received / progress.total) * 100))
      : 0;

  return (
    <div className="animate-rise sticky top-4 z-10 mx-auto flex w-fit max-w-3xl items-center gap-3 rounded-ctrl border border-clay/25 bg-carrier px-4 py-2.5 shadow-sm">
      <div className="min-w-0">
        <p className="text-sm font-semibold text-ink">
          发现新版本 <span className="font-mono text-clay-deep">v{latestVersion}</span>
          {notes && <span className="ml-2 hidden text-xs font-normal text-ink-soft sm:inline">{notes.slice(0, 40)}</span>}
        </p>
        {downloading && progress && (
          <div className="mt-1 flex w-64 items-center gap-2">
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-ink/10">
              <div
                className="h-full rounded-full bg-clay transition-all"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="font-mono text-[11px] text-ink-soft">{pct}%</span>
          </div>
        )}
        {installing && <p className="mt-1 text-xs text-ink-soft">正在安装，应用将退出并自动重启…</p>}
        {error && <p className="mt-1 max-w-xs truncate text-xs text-clay-deep">{error}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {!downloading && (
          <button
            onClick={onUpdate}
            disabled={busy}
            className="flex items-center gap-1 rounded-ctrl bg-clay px-3 py-1 text-xs font-medium text-white transition-colors enabled:hover:bg-clay-deep disabled:opacity-50"
          >
            <Download size={13} />
            {installing ? "安装中…" : downloading ? "下载中…" : "立即更新"}
          </button>
        )}
        <button
          onClick={onDismiss}
          disabled={busy}
          className="rounded-ctrl p-1 text-ink-soft transition-colors hover:text-ink disabled:opacity-40"
          title="稍后再说"
        >
          <X size={15} />
        </button>
      </div>
    </div>
  );
}