interface SkeletonProps {
  className?: string;
  /** 圆角档位：ctrl 控件 / card 卡片 / full 全圆 */
  rounded?: "ctrl" | "card" | "full";
}

/** 骨架屏原语：载入占位；prefers-reduced-motion 下停用脉冲动画（仅保留色块） */
export default function Skeleton({ className = "", rounded = "ctrl" }: SkeletonProps) {
  const radius = rounded === "card" ? "rounded-card" : rounded === "full" ? "rounded-full" : "rounded-ctrl";
  return <div aria-hidden className={`animate-pulse bg-carrier-deep motion-reduce:animate-none ${radius} ${className}`} />;
}
