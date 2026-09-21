import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

interface EmptyStateProps {
  /** 插画（anthropic-art 风格） */
  image?: string;
  /** 图标变体（无插画时的轻量视觉锚点） */
  icon?: LucideIcon;
  title: string;
  description: string;
  action?: ReactNode;
}

/** 空状态：插画 / 图标 + 标题 + 说明 + 动作 */
export default function EmptyState({ image, icon: Icon, title, description, action }: EmptyStateProps) {
  const hasAnchor = Boolean(image || Icon);
  return (
    <div className="flex animate-rise flex-col items-center justify-center px-6 py-16 text-center">
      {image && (
        <img
          src={image}
          alt=""
          draggable={false}
          className="h-44 w-44 rounded-card object-cover"
        />
      )}
      {!image && Icon && (
        <div className="flex size-20 items-center justify-center rounded-full bg-carrier-deep">
          <Icon size={30} strokeWidth={1.6} className="text-ink-soft/70" aria-hidden />
        </div>
      )}
      <h3 className={`${hasAnchor ? "mt-6" : ""} text-base font-semibold text-ink`}>{title}</h3>
      <p className="mt-1.5 max-w-sm text-sm text-ink-soft">{description}</p>
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
