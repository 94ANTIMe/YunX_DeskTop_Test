import { useEffect } from "react";
import { AlertTriangle } from "lucide-react";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmText?: string;
  cancelText?: string;
  /** 危险操作（红色确认按钮） */
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 轻量确认弹层：仅用于破坏性 / 不可逆操作（清空记录等），避免过度打断 */
export default function ConfirmDialog({
  open,
  title,
  description,
  confirmText = "确认",
  cancelText = "取消",
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  // Esc 取消
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm animate-fade"
      onClick={onCancel}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        className="w-full max-w-sm animate-rise rounded-card border border-ink/10 bg-carrier p-5 shadow-capsule"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-3">
          <div className={`shrink-0 rounded-ctrl p-2 ${danger ? "bg-danger/10" : "bg-carrier-deep"}`}>
            <AlertTriangle size={18} className={danger ? "text-danger" : "text-ink-soft"} />
          </div>
          <div className="min-w-0">
            <p className="text-sm font-semibold text-ink">{title}</p>
            <p className="mt-1.5 text-xs leading-relaxed text-ink-soft">{description}</p>
          </div>
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="rounded-ctrl border border-ink/15 px-4 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-ink/30 hover:text-ink"
          >
            {cancelText}
          </button>
          <button
            onClick={onConfirm}
            className={`rounded-ctrl px-4 py-1.5 text-xs font-semibold transition-colors ${
              danger ? "bg-danger text-on-danger hover:bg-danger/90" : "bg-clay text-on-accent hover:bg-clay-deep"
            }`}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
