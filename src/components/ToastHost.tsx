import { useSyncExternalStore } from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { dismissToast, getToasts, subscribeToasts, type ToastKind } from "../lib/toast";

/** 图标与语义色：状态语义走 token（text-success / text-danger），信息类用装饰色 clay */
const KIND_ICON: Record<ToastKind, { Icon: typeof Info; cls: string; label: string }> = {
  success: { Icon: CheckCircle2, cls: "text-success", label: "成功" },
  error: { Icon: XCircle, cls: "text-danger", label: "错误" },
  info: { Icon: Info, cls: "text-clay-deep", label: "提示" },
};

/**
 * 全局 Toast 容器：固定右下角，高于弹层层级（z-[80] > Modal z-[70]）。
 * 全库首个 aria-live 容器：条目增删自动读给读屏，无需聚焦。
 */
export default function ToastHost() {
  const items = useSyncExternalStore(subscribeToasts, getToasts);
  if (items.length === 0) return null;
  return (
    <div
      aria-live="polite"
      className="fixed bottom-6 right-6 z-[80] flex w-80 flex-col gap-2"
    >
      {items.map((t) => {
        const { Icon, cls, label } = KIND_ICON[t.kind];
        return (
          <div
            key={t.id}
            role="status"
            className="animate-rise flex items-start gap-2.5 rounded-card bg-carrier px-4 py-3 shadow-capsule"
          >
            <Icon size={16} className={`mt-0.5 shrink-0 ${cls}`} aria-hidden />
            <div className="min-w-0 flex-1">
              <span className="sr-only">{label}：</span>
              <p className="break-all text-sm leading-snug text-ink">{t.message}</p>
            </div>
            <button
              onClick={() => dismissToast(t.id)}
              aria-label="关闭提示"
              className="shrink-0 rounded-ctrl p-0.5 text-ink-soft transition-colors hover:text-ink"
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
