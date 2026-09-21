interface ToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  size?: "md" | "sm";
  disabled?: boolean;
  /** 可达性名称（switch 无可见文字标签时必填） */
  "aria-label"?: string;
}

const TRACK = {
  md: "h-6 w-11",
  sm: "h-5 w-9",
} as const;

const KNOB = {
  md: { box: "h-5 w-5", on: "left-[22px]", off: "left-0.5" },
  sm: { box: "h-4 w-4", on: "left-[18px]", off: "left-0.5" },
} as const;

/** 开关原语：role="switch" + aria-checked；旋钮色走 on-accent token（明暗主题下均为白色） */
export default function Toggle({ checked, onChange, size = "md", disabled = false, "aria-label": ariaLabel }: ToggleProps) {
  const knob = KNOB[size];
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative shrink-0 cursor-pointer rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${TRACK[size]} ${
        checked ? "bg-clay" : "bg-ink/15"
      }`}
    >
      <span
        className={`absolute top-0.5 rounded-full bg-on-accent shadow transition-all ${knob.box} ${
          checked ? knob.on : knob.off
        }`}
      />
    </button>
  );
}
