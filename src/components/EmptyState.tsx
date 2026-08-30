import type { ReactNode } from "react";

interface EmptyStateProps {
  /** 插画（anthropic-art 风格） */
  image?: string;
  title: string;
  description: string;
  action?: ReactNode;
}

/** 空状态：插画 + 标题 + 说明 + 动作 */
export default function EmptyState({ image, title, description, action }: EmptyStateProps) {
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
      <h3 className="mt-6 text-base font-semibold text-ink">{title}</h3>
      <p className="mt-1.5 max-w-sm text-sm text-ink-soft">{description}</p>
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
