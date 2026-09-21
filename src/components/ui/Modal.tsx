import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  children?: ReactNode;
  footer?: ReactNode;
  /** alertdialog 用于不可逆确认类弹层 */
  kind?: "dialog" | "alertdialog";
  closeOnEsc?: boolean;
  closeOnOverlay?: boolean;
  showClose?: boolean;
  /** 追加到面板的类（覆盖 max-w 等） */
  panelClassName?: string;
}

/**
 * 统一弹层原语：z-[70]、遮罩 bg-black/40 backdrop-blur + animate-fade、面板 animate-rise；
 * 初始焦点落在面板（焦点环由全局 :focus-visible 提供）、关闭时还原焦点、body 滚动锁；
 * Esc / 点遮罩关闭可配（默认开启）。
 */
export default function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  kind = "dialog",
  closeOnEsc = true,
  closeOnOverlay = true,
  showClose = true,
  panelClassName = "",
}: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreRef = useRef<HTMLElement | null>(null);

  // 焦点管理 + body 滚动锁（open 切换时执行；cleanup 负责关闭侧的还原）
  useEffect(() => {
    if (!open) return;
    restoreRef.current = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prevOverflow;
      restoreRef.current?.focus?.();
    };
  }, [open]);

  useEffect(() => {
    if (!open || !closeOnEsc) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, closeOnEsc, onClose]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/40 p-4 backdrop-blur animate-fade"
      onClick={closeOnOverlay ? onClose : undefined}
    >
      <div
        ref={panelRef}
        tabIndex={-1}
        role={kind}
        aria-modal="true"
        aria-label={title}
        className={`w-full max-w-md animate-rise rounded-card border border-ink/10 bg-carrier shadow-capsule outline-none ${panelClassName}`}
        onClick={(e) => e.stopPropagation()}
      >
        {(title || showClose) && (
          <div className="flex items-center justify-between gap-3 px-5 pt-4">
            {title ? (
              <h3 className="text-sm font-semibold text-ink">{title}</h3>
            ) : (
              <span aria-hidden />
            )}
            {showClose && (
              <button
                onClick={onClose}
                aria-label="关闭"
                className="shrink-0 cursor-pointer rounded-ctrl p-1 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
              >
                <X size={16} />
              </button>
            )}
          </div>
        )}
        {children && <div className="px-5 py-4">{children}</div>}
        {footer && <div className="flex justify-end gap-2 px-5 pb-4">{footer}</div>}
      </div>
    </div>
  );
}
