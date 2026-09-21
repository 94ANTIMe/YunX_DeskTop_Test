export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  /** 可达性名称（无可见 label 时必填） */
  "aria-label"?: string;
  className?: string;
}

/** 原生 select 封装：统一设置页既有的边框/底色/聚焦风格（保持原生下拉的键盘与读屏行为） */
export default function Select({ value, options, onChange, disabled = false, "aria-label": ariaLabel, className = "" }: SelectProps) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.currentTarget.value)}
      disabled={disabled}
      aria-label={ariaLabel}
      className={`h-9 rounded-ctrl border border-ink/10 bg-carrier-deep px-2 text-xs text-ink transition-colors focus:border-clay focus:outline-none disabled:cursor-not-allowed disabled:opacity-40 ${className}`}
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}
