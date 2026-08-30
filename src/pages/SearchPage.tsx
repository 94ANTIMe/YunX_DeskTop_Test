import { useEffect, useState } from "react";
import { Link2, Loader2, Search as SearchIcon } from "lucide-react";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import { errMsg, ipc, DEFAULT_SETTINGS, type SearchItem, type Settings } from "../lib/ipc";
import { platformLabel } from "../lib/format";
import searchHero from "../assets/art/search-hero.jpg";

interface SearchPageProps {
  /** 当前 Tab 是否为搜索页（激活时刷新服务地址配置） */
  active: boolean;
  onGoResolve: (link: string, pwd: string) => void;
}

/** 云析支持解析下载的网盘类型（搜索结果里高亮可解析项） */
const SUPPORTED_TYPES = new Set(["quark", "uc", "xunlei", "baidu", "c139", "pan123"]);

/** 搜索页：PanSou 自部署服务搜全网网盘资源 → 一键转解析下载 */
export default function SearchPage({ active, onGoResolve }: SearchPageProps) {
  const [kw, setKw] = useState("");
  const [results, setResults] = useState<SearchItem[]>([]);
  const [searching, setSearching] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const configured = !!settings?.pansouBaseUrl?.trim();

  // 激活时刷新设置（设置页改了服务地址后切回来能立即生效）
  useEffect(() => {
    if (!active) return;
    ipc
      .getSettings()
      .then((s) => setSettings({ ...DEFAULT_SETTINGS, ...s }))
      .catch(() => setSettings(null));
  }, [active]);

  async function search() {
    const keyword = kw.trim();
    if (!keyword || searching) return;
    setSearching(true);
    setError("");
    setNotice("");
    try {
      const items = await ipc.pansouSearch(keyword);
      setResults(items);
      if (items.length === 0) setNotice("未搜索到结果（换个关键词试试）");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setSearching(false);
    }
  }

  function goResolve(item: SearchItem) {
    onGoResolve(item.url, item.password);
  }

  // 按类型分组统计（结果顶部徽标）
  const typeCount = results.reduce<Record<string, number>>((acc, it) => {
    acc[it.type] = (acc[it.type] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="space-y-6">
      <PageHeader tab="search" subtitle="自部署 PanSou 聚合搜索全网网盘分享资源" />

      {/* 搜索输入区 */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <div className="flex gap-4">
          {!configured && !searching && results.length === 0 && (
            <img
              src={searchHero}
              alt=""
              draggable={false}
              className="hidden h-44 w-44 shrink-0 rounded-card object-cover md:block"
            />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex gap-3">
              <input
                value={kw}
                onChange={(e) => setKw(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && search()}
                placeholder="输入影视、音乐、软件等资源关键词…"
                className="h-11 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-4 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
              />
              <button
                onClick={search}
                disabled={!kw.trim() || searching}
                className="flex shrink-0 items-center gap-2 rounded-ctrl bg-clay px-5 py-2 text-sm font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
              >
                {searching ? <Loader2 size={15} className="animate-spin" /> : <SearchIcon size={15} />}
                {searching ? "搜索中…" : "搜索"}
              </button>
            </div>
            <p className="mt-3 text-xs text-ink-soft/70">
              {configured ? (
                <>
                  服务：<span className="font-mono">{settings?.pansouBaseUrl}</span>
                </>
              ) : (
                <>
                  未配置搜索服务，请前往「设置 → 搜索」填写自部署 PanSou 服务地址
                </>
              )}
            </p>
          </div>
        </div>
      </section>

      {/* 提示条 */}
      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-cactus/25 px-4 py-2.5 text-sm text-ink">{notice}</div>
      )}

      {/* 结果区 */}
      {searching ? (
        <div className="rounded-card bg-carrier px-6 py-16 text-center text-sm text-ink-soft">
          <Loader2 size={20} className="mx-auto mb-3 animate-spin text-clay" />
          正在聚合各网盘来源…
        </div>
      ) : results.length === 0 ? (
        <EmptyState
          image={searchHero}
          title={configured ? "开始搜索网盘资源" : "先配置 PanSou 搜索服务"}
          description={
            configured
              ? "输入关键词，聚合搜索夸克、百度、UC 等网盘的公开分享资源，点击结果直接转入解析下载。"
              : "PanSou 是可自部署的网盘聚合搜索服务，配置服务地址后即可在这里搜到全网公开分享资源。"
          }
        />
      ) : (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "120ms" }}>
          {/* 类型统计徽标 */}
          <div className="flex flex-wrap items-center gap-2 border-b border-ink/10 pb-4">
            <span className="text-xs font-medium text-ink-soft">
              共 {results.length} 条结果
            </span>
            {Object.entries(typeCount).map(([type, count]) => (
              <span
                key={type}
                className="rounded-full bg-carrier-deep px-2.5 py-0.5 font-mono text-[10px] text-ink-soft"
              >
                {platformLabel(type)} × {count}
              </span>
            ))}
          </div>

          <ul className="divide-y divide-ink/10">
            {results.map((item, i) => {
              const supported = SUPPORTED_TYPES.has(item.type);
              return (
                <li key={`${item.url}-${i}`} className="flex items-center gap-3 py-3">
                  <span className="shrink-0 rounded-full bg-clay px-2.5 py-0.5 font-mono text-[10px] font-semibold tracking-widest text-white">
                    {platformLabel(item.type)}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm text-ink" title={item.note || item.url}>
                      {item.note || item.url}
                    </p>
                    <p className="truncate font-mono text-[10px] text-ink-soft/70" title={item.url}>
                      {item.url}
                      {item.password ? ` · 提取码 ${item.password}` : ""}
                      {item.source ? ` · ${item.source}` : ""}
                    </p>
                  </div>
                  <button
                    onClick={() => goResolve(item)}
                    disabled={!supported}
                    className={`flex shrink-0 items-center gap-1.5 rounded-ctrl px-3 py-1.5 text-xs font-semibold transition-colors ${
                      supported
                        ? "bg-clay text-white hover:bg-clay-deep"
                        : "cursor-not-allowed bg-carrier-deep text-ink-soft/50"
                    }`}
                    title={supported ? "转入解析页取直链下载" : "该网盘类型暂不支持解析下载"}
                  >
                    <Link2 size={13} />
                    解析
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </div>
  );
}
