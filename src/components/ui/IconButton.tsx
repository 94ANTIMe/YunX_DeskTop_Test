import type { ButtonHTMLAttributes } from "react";

export type IconButtonVariant = "ghost" | "outline";
export type IconButtonSize = "sm" | "md";

const VARIANT: Record<IconButtonVariant, string> = {
  ghost: "text-ink-soft enabled:hover:bg-carrier-deep enabled:hover:text-ink disabled:opacity-40",
  outline: "border border-ink/15 text-ink-soft enabled:hover:border-clay enabled:hover:text-clay-deep disabled:opacity-40",
};

const SIZE: Record<IconButtonSize, string> = {
  sm: "p-1",
  md: "p-1.5",
};

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  /** 必填：纯图标按钮的可达性名称 */
  "aria-label": string;
}

/** 图标按钮原语：无文字内容，aria-label 必填（交互四件套 · 可达性） */
export default function IconButton({
  variant = "ghost",
  size = "md",
  className = "",
  type = "button",
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex cursor-pointer items-center justify-center rounded-ctrl transition-colors ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...rest}
    />
  );
}
