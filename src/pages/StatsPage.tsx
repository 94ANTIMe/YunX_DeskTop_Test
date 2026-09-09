import { useCallback, useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import { errMsg, ipc, type StatsOverview } from "../lib/ipc";
import { formatBytes, platformLabel } from "../lib/format";

/** 本地时区 day key（与后端 download_stat.day 格式一致） */
function dayKey(offsetFromToday: number): string {
  const d = new Date(Date.now() - offsetFromToday * 86400000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 每日流量柱状图（CSS 柱体，无图表库；hover 显示日期明细） */
function DailyChart({ days }: { days: { day: string; bytes: number; files: number; failed: number }[] }) {
  const max = Math.max(...days.map((d) => d.bytes), 1);
  return (
    <div>
      <div className="flex items-center justify-between">
        <p className="text-[10px] tracking-wide text-ink-soft/70">
          每日下载流量 · 峰值 {formatBytes(max)}
        </p>
        <p className="font-mono text-[10px] text-ink-soft/70">近 {days.length} 天</p>
      </div>
      <div className="relative mt-3 h-44">
        {/* 横向参考线 */}
        {[25, 50, 75].map((y) => (
          <div
            key={y}
            className="absolute inset-x-0 border-t border-dashed border-ink/5"
            style={{ top: `${y}%` }}
          />
        ))}
        <div className="flex h-full items-end gap-[3px]">
          {days.map((d) => {
            const h = d.bytes > 0 ? Math.max(4, Math.round((d.bytes / max) * 100)) : 0;
            const date = new Date(`${d.day}T00:00:00`);
            const label = `${date.getMonth() + 1}/${date.getDate()}`;
            return (
              <div key={d.day} className="group relative flex h-full min-w-0 flex-1 items-end">
                {/* 柱体（0 流量画基线短桩） */}
                <div
                  className={`w-full rounded-t-sm transition-colors ${
                    d.bytes > 0 ? "bg-clay group-hover:bg-clay-deep" : "bg-carrier-deep"
                  }`}
                  style={{ height: d.bytes > 0 ? `${h}%` : "3px" }}
                />
                {/* hover 明细 */}
                {d.bytes > 0 && (
                  <div className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-1.5 hidden -translate-x-1/2 whitespace-nowrap rounded-ctrl bg-ink px-2.5 py-1.5 text-[10px] leading-relaxed text-ivory shadow-capsule group-hover:block">
                    <p className="font-mono font-semibold">{label}</p>
                    <p className="font-mono">{formatBytes(d.bytes)} · {d.files} 个文件</p>
                    {d.failed > 0 && <p className="text-clay">失败 {d.failed}</p>}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
      {/* X 轴起止标签 */}
      <div className="mt-1.5 flex justify-between font-mono text-[10px] text-ink-soft/60">
        <span>{days[0] ? dayLabel(days[0].day) : ""}</span>
        <span>{days.length ? dayLabel(days[days.length - 1].day) : ""}</span>
      </div>
    </div>
  );
}

/** "2026-09-06" → "9/6" */
function dayLabel(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  return `${m}/${d}${y !== new Date().getFullYear() ? `/${String(y).slice(2)}` : ""}`;
}

/** 平台分布横向条形 */
function PlatformBars({ platforms }: { platforms: StatsOverview["platforms"] }) {
  const max = Math.max(...platforms.map((p) => p.bytes), 1);
  const total = platforms.reduce((s, p) => s + p.bytes, 0);
  return (
    <div className="space-y-2.5">
      {platforms.map((p) => (
        <div key={p.platform} className="flex items-center gap-3">
          <span className="w-16 shrink-0 text-right text-[11px] text-ink-soft">
            {platformLabel(p.platform)}
          </span>
          <div className="h-2.5 min-w-0 flex-1 overflow-hidden rounded-full bg-carrier-deep">
            <div
              className="h-full rounded-full bg-clay transition-all duration-500"
              style={{ width: `${Math.max(2, (p.bytes / max) * 100)}%` }}
            />
          </div>
          <span className="w-20 shrink-0 text-right font-mono text-[11px] text-ink">
            {formatBytes(p.bytes)}
          </span>
          <span className="w-12 shrink-0 text-right font-mono text-[10px] text-ink-soft/70">
            {total > 0 ? `${Math.round((p.bytes / total) * 100)}%` : "—"}
          </span>
        </div>
      ))}
      <p className="pt-1 text-right text-[10px] text-ink-soft/60">
        共 {platforms.reduce((s, p) => s + p.files, 0)} 个文件 · {platforms.reduce((s, p) => s + p.failed, 0)} 次失败
      </p>
    </div>
  );
}

/** KPI 卡片 */
function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="animate-rise rounded-card bg-carrier p-5">
      <p className="text-[10px] tracking-wide text-ink-soft/70">{label}</p>
      <p className="mt-2 font-mono text-xl font-semibold text-ink">{value}</p>
      {sub && <p className="mt-1 text-[10px] text-ink-soft/70">{sub}</p>}
    </div>
  );
}

/** 统计页：下载流量报表（数据来自独立聚合表，清空任务记录不影响） */
export default function StatsPage({ active }: { active: boolean }) {
  const [stats, setStats] = useState<StatsOverview | null>(null);
  const [range, setRange] = useState<14 | 30>(14);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setStats(await ipc.getDownloadStats(30));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // 进入页面时拉取（常驻挂载，仅 active 时刷新，统计非实时）
  useEffect(() => {
    if (active) void load();
  }, [active, load]);

  const daily = stats?.daily ?? [];
  const byDay = new Map(daily.map((d) => [d.day, d]));
  const window30 = Array.from({ length: 30 }, (_, i) => {
    const key = dayKey(29 - i);
    const d = byDay.get(key);
    return { day: key, bytes: d?.bytes ?? 0, files: d?.files ?? 0, failed: d?.failed ?? 0 };
  });
  const window14 = window30.slice(-14);
  const shown = range === 14 ? window14 : window30;
  const rangeBytes = shown.reduce((s, d) => s + d.bytes, 0);
  const hasData = (stats?.totals.files ?? 0) > 0 || (stats?.totals.failed ?? 0) > 0;

  return (
    <div className="space-y-6">
      <PageHeader tab="stats" subtitle="下载流量与平台分布 · 本地聚合">
        <button
          onClick={() => void load()}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-40"
          title="重新统计"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          刷新
        </button>
      </PageHeader>

      {error && (
        <div className="rounded-ctrl bg-danger/10 px-4 py-2.5 text-sm text-danger">{error}</div>
      )}

      {!hasData ? (
        <div className="rounded-card bg-carrier">
          <EmptyState
            title="暂无下载数据"
            description="完成首次下载后，这里会按日聚合流量、文件数与平台分布。"
          />
        </div>
      ) : (
        <>
          {/* KPI 行 */}
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <Kpi label="累计完成" value={`${stats!.totals.files} 个`} sub={`失败 ${stats!.totals.failed} 次`} />
            <Kpi label="累计流量" value={formatBytes(stats!.totals.bytes)} />
            <Kpi label={`近 ${range} 天流量`} value={formatBytes(rangeBytes)} />
            <Kpi
              label="平台数"
              value={String(stats!.platforms.length)}
              sub={stats!.platforms[0] ? `主力：${platformLabel(stats!.platforms[0].platform)}` : undefined}
            />
          </div>

          {/* 每日流量 */}
          <div className="animate-rise rounded-card bg-carrier p-5">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="font-display text-lg font-semibold text-ink">每日流量</h3>
              {/* 时间窗切换 */}
              <div className="flex rounded-full border border-ink/10 p-0.5">
                {([14, 30] as const).map((r) => (
                  <button
                    key={r}
                    onClick={() => setRange(r)}
                    className={`rounded-full px-3 py-1 text-[11px] font-medium transition-colors ${
                      range === r ? "bg-clay text-on-accent" : "text-ink-soft hover:text-ink"
                    }`}
                  >
                    {r} 天
                  </button>
                ))}
              </div>
            </div>
            <DailyChart days={shown} />
          </div>

          {/* 平台分布 */}
          {stats!.platforms.length > 0 && (
            <div className="animate-rise rounded-card bg-carrier p-5">
              <h3 className="mb-4 font-display text-lg font-semibold text-ink">平台分布</h3>
              <PlatformBars platforms={stats!.platforms} />
            </div>
          )}
        </>
      )}
    </div>
  );
}
