import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "outline" | "danger" | "soft";
export type ButtonSize = "sm" | "md" | "lg";

/** 实底禁用统一 50、描边/幽灵统一 40；hover 挂 enabled: 防禁用残留 */
const VARIANT: Record<ButtonVariant, string> = {
  primary: "bg-clay text-on-accent enabled:hover:bg-clay-deep disabled:opacity-50",
  outline: "border border-ink/15 text-ink enabled:hover:border-clay enabled:hover:text-clay-deep disabled:opacity-40",
  danger: "border border-danger/40 text-danger enabled:hover:bg-danger/10 disabled:opacity-40",
  soft: "bg-carrier-deep text-ink-soft enabled:hover:text-ink disabled:opacity-40",
};

const SIZE: Record<ButtonSize, string> = {
  sm: "px-2.5 py-1 text-xs",
  md: "px-4 py-2 text-sm",
  lg: "px-5 py-2.5 text-sm font-semibold",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

/** 标准按钮原语：默认 button 类型（避免表单误提交），全局 :focus-visible 焦点环由 tokens.css 提供 */
export default function Button({
  variant = "outline",
  size = "md",
  type = "button",
  className = "",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex cursor-pointer items-center justify-center gap-1.5 rounded-ctrl font-medium transition-colors ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...rest}
    />
  );
}
