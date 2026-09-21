import { useEffect, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  /** 面板宽度（默认与任务详情抽屉一致） */
  widthClass?: string;
}

const LEAVE_MS = 220;

/**
 * 右侧滑出抽屉原语：收编 TaskDetailDrawer / BatchQueuePanel 同构的
 * 「出现即展示 → 关闭先播 220ms 滑出再卸载」机制（reduced-motion 下由 tokens.css 降级为淡入淡出）；
 * z-[70]、遮罩 bg-black/40 backdrop-blur、Esc 关闭、body 滚动锁。
 */
export default function Drawer({
  open,
  onClose,
  title,
  children,
  footer,
  widthClass = "w-[440px] max-w-[92vw]",
}: DrawerProps) {
  const [shown, setShown] = useState(open);
  const [leaving, setLeaving] = useState(false);
  const restoreRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      setShown(true);
      setLeaving(false);
    } else if (shown) {
      setLeaving(true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!leaving) return;
    const timer = window.setTimeout(() => {
      setShown(false);
      setLeaving(false);
    }, LEAVE_MS);
    return () => window.clearTimeout(timer);
  }, [leaving]);

  // 焦点还原 + 滚动锁
  useEffect(() => {
    if (!shown || leaving) return;
    restoreRef.current = document.activeElement as HTMLElement | null;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prevOverflow;
      restoreRef.current?.focus?.();
    };
  }, [shown, leaving]);

  useEffect(() => {
    if (!shown || leaving) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, leaving, onClose]);

  if (!shown) return null;
  return (
    <div className="fixed inset-0 z-[70]">
      <div
        className={`absolute inset-0 bg-black/40 backdrop-blur animate-fade ${leaving ? "opacity-0 transition-opacity duration-200" : ""}`}
        onClick={onClose}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === "string" ? title : undefined}
        className={`absolute inset-y-0 right-0 flex flex-col border-l border-ink/10 bg-carrier shadow-2xl outline-none ${
          leaving ? "animate-drawer-out" : "animate-drawer-in"
        } ${widthClass}`}
      >
        <header className="flex shrink-0 items-center justify-between border-b border-ink/10 px-5 py-4">
          <div className="min-w-0 text-sm font-semibold text-ink">{title}</div>
          <button
            onClick={onClose}
            aria-label="关闭"
            className="shrink-0 cursor-pointer rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
          >
            <X size={16} />
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
        {footer && <div className="shrink-0 border-t border-ink/10 px-5 py-3">{footer}</div>}
      </aside>
    </div>
  );
}
