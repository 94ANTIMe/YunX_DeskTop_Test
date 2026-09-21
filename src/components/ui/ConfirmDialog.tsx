import { AlertTriangle } from "lucide-react";
import Modal from "./Modal";
import Button from "./Button";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmText?: string;
  cancelText?: string;
  /** 危险操作（红色确认按钮 + alertdialog 语义） */
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 确认弹层原语（基于 ui/Modal）：仅用于破坏性 / 不可逆操作，避免过度打断 */
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
  return (
    <Modal
      open={open}
      onClose={onCancel}
      kind={danger ? "alertdialog" : "dialog"}
      showClose={false}
      panelClassName="max-w-sm"
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
        <Button variant="outline" size="sm" onClick={onCancel}>
          {cancelText}
        </Button>
        {danger ? (
          <button
            onClick={onConfirm}
            className="cursor-pointer rounded-ctrl bg-danger px-4 py-1.5 text-xs font-semibold text-on-danger transition-colors enabled:hover:bg-danger/90 disabled:opacity-50"
          >
            {confirmText}
          </button>
        ) : (
          <Button variant="primary" size="sm" onClick={onConfirm} className="px-4 py-1.5 text-xs font-semibold">
            {confirmText}
          </Button>
        )}
      </div>
    </Modal>
  );
}
