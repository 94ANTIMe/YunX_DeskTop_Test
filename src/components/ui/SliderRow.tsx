interface SliderRowProps {
  label: string;
  /** 数值旁的附加说明（如单位语义），小字展示在 label 下 */
  codeLabel?: string;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  value: number;
  /** 拖动即时回调（每帧） */
  onInput: (value: number) => void;
  /** 松手 / 失焦提交回调（持久化时机） */
  onCommit: (value: number) => void;
  disabled?: boolean;
}

/** 设置行滑杆原语：保留既有手势——拖动即时生效、松手或失焦才提交持久化 */
export default function SliderRow({
  label,
  codeLabel,
  min,
  max,
  step = 1,
  unit = "",
  value,
  onInput,
  onCommit,
  disabled = false,
}: SliderRowProps) {
  const commit = (raw: string | number) => {
    const v = Number(raw);
    if (!Number.isNaN(v) && v !== value) onCommit(v);
  };
  return (
    <div className="flex items-center justify-between py-3">
      <div className="min-w-0">
        <dt className="text-sm text-ink-soft">{label}</dt>
        {codeLabel && <p className="mt-0.5 text-[11px] text-ink-soft/60">{codeLabel}</p>}
      </div>
      <dd className="flex items-center gap-3">
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          disabled={disabled}
          value={value}
          onChange={(e) => onInput(Number(e.currentTarget.value))}
          onMouseUp={(e) => commit(e.currentTarget.value)}
          onBlur={(e) => commit(e.currentTarget.value)}
          className="w-44 accent-clay disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={label}
        />
        <span className="w-14 text-right font-mono text-sm text-ink">
          {value}
          {unit}
        </span>
      </dd>
    </div>
  );
}
