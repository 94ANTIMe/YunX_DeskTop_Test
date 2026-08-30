import { useMemo } from "react";
import { ArrowRight, X } from "lucide-react";
import type { ClipboardShareEvent } from "../lib/ipc";
import { platformLabel } from "../lib/format";

interface ClipboardPromptProps {
  share: ClipboardShareEvent;
  /** 去解析（回调 App.goResolve） */
  onResolve: (text: string, pwd: string) => void;
  /** 忽略本次提示 */
  onDismiss: () => void;
}

/** 剪贴板命中分享链接的右下角浮层提示 */
export default function ClipboardPrompt({ share, onResolve, onDismiss }: ClipboardPromptProps) {
  const preview = useMemo(() => {
    const t = share.text.trim();
    return t.length > 56 ? `${t.slice(0, 56)}…` : t;
  }, [share.text]);

  return (
    <div className="animate-rise fixed bottom-5 right-5 z-50 w-80 max-w-[calc(100vw-2.5rem)] overflow-hidden rounded-card border border-ink/10 bg-carrier p-4 shadow-capsule backdrop-blur">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="font-mono text-[10px] tracking-[0.22em] text-ink-soft">
            CLIPBOARD · 检测到分享链接
          </p>
          <p className="mt-1.5 flex items-center gap-2">
            <span className="rounded-full bg-clay/15 px-2 py-0.5 font-mono text-[10px] text-clay-deep">
              {platformLabel(share.parsed.platform)}
            </span>
            <span className="text-xs text-ink-soft">是否解析下载？</span>
          </p>
        </div>
        <button
          onClick={onDismiss}
          className="shrink-0 rounded-full p-1 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
          title="忽略"
        >
          <X size={15} />
        </button>
      </div>
      <p className="mt-3 break-all rounded-ctrl bg-carrier-deep px-3 py-2 font-mono text-[11px] text-ink-soft">
        {preview}
      </p>
      <div className="mt-3 flex justify-end">
        <button
          onClick={() => onResolve(share.text, share.parsed.pwd)}
          className="flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-clay-deep"
        >
          去解析
          <ArrowRight size={14} />
        </button>
      </div>
    </div>
  );
}