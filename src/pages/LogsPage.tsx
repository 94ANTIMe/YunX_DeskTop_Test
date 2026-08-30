import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle2, ChevronDown, Info, Loader2, RefreshCw, Trash2, XCircle } from "lucide-react";
import PageHeader from "../components/PageHeader";
import { errMsg, ipc, type LogRow } from "../lib/ipc";
import { formatDate, platformLabel } from "../lib/format";
import logsHero from "../assets/art/logs-hero.jpg";

type LevelFilter = "" | "success" | "error" | "info";

const LEVEL_META: Record<string, { label: string; icon: typeof Info; cls: string; dot: string }> = {
  success: { label: "成功", icon: CheckCircle2, cls: "text-cactus", dot: "bg-cactus" },
  error: { label: "失败", icon: XCircle, cls: "text-clay-deep", dot: "bg-clay-deep" },
  info: { label: "记录", icon: Info, cls: "text-ink-soft", dot: "bg-ink-soft/50" },
};

const FILTERS: { value: LevelFilter; label: string }[] = [
  { value: "", label: "全部" },
  { value: "success", label: "成功" },
  { value: "error", label: "失败" },
  { value: "info", label: "记录" },
];

/** 日志页：解析 / 收集 / 取链 / 下载 / 登录全链路日志（3s 轮询） */
export default function LogsPage() {
  const [logs, setLogs] = useState<LogRow[]>([]);
  const [filter, setFilter] = useState<LevelFilter>("");
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [auto, setAuto] = useState(true);
  const filterRef = useRef(filter);
  filterRef.current = filter;

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = await ipc.listLogs(filterRef.current || undefined, 500);
      setLogs(list);
      setError("");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // 初始 + 3s 自动轮询
  useEffect(() => {
    refresh();
    if (!auto) return;
    const timer = window.setInterval(refresh, 3000);
    return () => window.clearInterval(timer);
  }, [auto, refresh]);

  // 切筛选立即刷新
  useEffect(() => {
    refresh();
  }, [filter, refresh]);

  async function clearAll() {
    try {
      await ipc.clearLogs();
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  const errorCount = logs.filter((l) => l.level === "error").length;
  const successCount = logs.filter((l) => l.level === "success").length;

  return (
    <div className="space-y-6">
      <PageHeader tab="logs" subtitle="收集 / 取链 / 下载 / 登录全链路记录">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs text-cactus">{successCount} 成功</span>
          <span className="font-mono text-xs text-clay-deep">{errorCount} 失败</span>
          <button
            onClick={() => setAuto((a) => !a)}
            className={`rounded-ctrl border px-3 py-1.5 text-xs font-medium transition-colors ${
              auto ? "border-clay text-clay-deep" : "border-ink/15 text-ink-soft hover:border-clay hover:text-clay-deep"
            }`}
          >
            {auto ? "自动刷新中" : "已暂停"}
          </button>
          <button
            onClick={refresh}
            disabled={loading}
            className="rounded-ctrl border border-ink/15 p-1.5 text-ink-soft transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-40"
            title="刷新"
          >
            {loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
          </button>
          <button
            onClick={clearAll}
            className="rounded-ctrl border border-ink/15 p-1.5 text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
            title="清空日志"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </PageHeader>

      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}

      {/* hero 插画带 */}
      <section className="flex animate-rise items-center gap-8 rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <div>
          <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">ACTIVITY LOG</p>
          <h3 className="mt-1.5 font-display text-2xl font-semibold text-ink">操作日志</h3>
          <p className="mt-2 max-w-md text-sm text-ink-soft">
            解析、取链、下载、登录的每一步都会在这里留下记录；点按条目可展开细节。
          </p>
        </div>
        <img
          src={logsHero}
          alt=""
          draggable={false}
          className="hidden h-36 w-56 shrink-0 rounded-card object-cover sm:block"
        />
      </section>

      {/* 筛选 */}
      <section className="animate-rise rounded-card bg-carrier p-4" style={{ animationDelay: "120ms" }}>
        <div className="flex gap-1.5">
          {FILTERS.map((f) => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={`rounded-ctrl px-4 py-1.5 text-sm font-medium transition-colors ${
                filter === f.value
                  ? "bg-clay text-white"
                  : "bg-carrier-deep text-ink-soft hover:text-ink"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </section>

      {/* 日志列表 */}
      <section className="animate-rise rounded-card bg-carrier p-2" style={{ animationDelay: "120ms" }}>
        {logs.length === 0 ? (
          <p className="py-12 text-center text-sm text-ink-soft">暂无日志</p>
        ) : (
          <ul className="divide-y divide-ink/10">
            {logs.map((log) => {
              const meta = LEVEL_META[log.level] ?? LEVEL_META.info;
              const Icon = meta.icon;
              const expanded = expandedId === log.id;
              return (
                <li key={log.id} className="px-4 py-3">
                  <button
                    className="flex w-full items-center gap-3 text-left"
                    onClick={() => setExpandedId(expanded ? null : log.id)}
                  >
                    <Icon size={16} className={`shrink-0 ${meta.cls}`} />
                    <span className="shrink-0 font-mono text-[11px] text-ink-soft">
                      {formatDate(log.time)}
                    </span>
                    {log.platform && (
                      <span className="shrink-0 rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                        {platformLabel(log.platform)}
                      </span>
                    )}
                    <span className="min-w-0 flex-1 truncate text-sm text-ink">{log.message}</span>
                    {log.detail && (
                      <ChevronDown
                        size={14}
                        className={`shrink-0 text-ink-soft/60 transition-transform ${expanded ? "rotate-180" : ""}`}
                      />
                    )}
                  </button>
                  {expanded && log.detail && (
                    <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded-ctrl bg-carrier-deep px-3 py-2 font-mono text-[11px] leading-relaxed text-ink-soft">
                      {log.detail}
                    </pre>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
