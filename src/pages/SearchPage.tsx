import { useEffect, useState } from "react";
import { Bell, Link2, Loader2, Plus, RefreshCw, Rss, Search as SearchIcon, Trash2, X } from "lucide-react";
import PageHeader from "../components/PageHeader";
import EmptyState from "../components/EmptyState";
import { errMsg, ipc, DEFAULT_SETTINGS, type SearchItem, type Settings, type Subscription } from "../lib/ipc";
import { platformLabel } from "../lib/format";
import searchHero from "../assets/art/search-hero.jpg";

interface SearchPageProps {
  /** 当前 Tab 是否为搜索页（激活时刷新服务地址配置） */
  active: boolean;
  onGoResolve: (link: string, pwd: string) => void;
}

/** 云析支持解析下载的网盘类型（搜索结果里高亮可解析项） */
const SUPPORTED_TYPES = new Set(["quark", "uc", "xunlei", "baidu", "c139", "pan123"]);

/** 订阅平台偏好选项（值 = 平台 key，空 = 自动：夸克 + UC） */
const SUB_PLATFORM_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "自动（夸克 + UC）" },
  { value: "quark", label: "夸克" },
  { value: "uc", label: "UC" },
  { value: "baidu", label: "百度网盘" },
  { value: "xunlei", label: "迅雷" },
  { value: "pan123", label: "123 云盘" },
  { value: "c139", label: "移动云" },
];

/** 订阅表单（新建对话框状态） */
interface SubForm {
  keyword: string;
  platform: string;
  regex: string;
}

/** 搜索页：PanSou 自部署服务搜全网网盘资源 → 一键转解析下载；支持订阅关键词定时追更 */
export default function SearchPage({ active, onGoResolve }: SearchPageProps) {
  const [kw, setKw] = useState("");
  const [results, setResults] = useState<SearchItem[]>([]);
  const [searching, setSearching] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  // 订阅
  const [subs, setSubs] = useState<Subscription[]>([]);
  const [subsOpen, setSubsOpen] = useState(false);
  const [subForm, setSubForm] = useState<SubForm | null>(null);
  const [subBusy, setSubBusy] = useState<number | null>(null);
  const [subSubmitting, setSubSubmitting] = useState(false);

  const configured = !!settings?.pansouBaseUrl?.trim();

  // 激活时刷新设置与订阅列表（设置页改了服务地址后切回来能立即生效）
  useEffect(() => {
    if (!active) return;
    ipc
      .getSettings()
      .then((s) => setSettings({ ...DEFAULT_SETTINGS, ...s }))
      .catch(() => setSettings(null));
    ipc
      .listSubscriptions()
      .then((rows) => setSubs(rows))
      .catch(() => setSubs([]));
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

  function flashNotice(text: string) {
    setNotice(text);
    window.setTimeout(() => setNotice(""), 4000);
  }

  // ---------- 订阅操作 ----------

  function subscribeFromResult() {
    if (!settings?.pansouBaseUrl?.trim()) {
      setError("请先在「设置 → 搜索」配置 PanSou 服务地址");
      return;
    }
    setSubForm({ keyword: kw.trim(), platform: "", regex: "" });
  }

  async function submitSubForm() {
    if (!subForm || subSubmitting) return;
    const keyword = subForm.keyword.trim();
    if (!keyword) return;
    setSubSubmitting(true);
    setError("");
    try {
      await ipc.addSubscription(keyword, subForm.platform ? [subForm.platform] : undefined, subForm.regex.trim() || undefined);
      setSubForm(null);
      const rows = await ipc.listSubscriptions();
      setSubs(rows);
      setSubsOpen(true);
      flashNotice(`已订阅「${keyword}」，将按设置间隔定时检查更新`);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setSubSubmitting(false);
    }
  }

  async function toggleSub(sub: Subscription) {
    try {
      await ipc.updateSubscription(
        sub.id,
        !sub.enabled,
        sub.keyword,
        JSON.parse(sub.cloudTypesJson || "[]"),
        sub.episodeRegex || undefined,
      );
      const rows = await ipc.listSubscriptions();
      setSubs(rows);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function removeSub(id: number) {
    try {
      await ipc.removeSubscription(id);
      setSubs((rows) => rows.filter((r) => r.id !== id));
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function runSub(sub: Subscription) {
    if (subBusy !== null) return;
    setSubBusy(sub.id);
    setError("");
    try {
      const summary = await ipc.runSubscriptionNow(sub.id);
      flashNotice(`「${sub.keyword}」${summary}`);
      const rows = await ipc.listSubscriptions();
      setSubs(rows);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setSubBusy(null);
    }
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
                className="flex shrink-0 items-center gap-2 rounded-ctrl bg-clay px-5 py-2 text-sm font-semibold text-on-accent transition-colors hover:bg-clay-deep disabled:opacity-50"
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

      {/* 我的订阅（定时追更：搜索 → 过滤新集 → 自动下载） */}
      {(subs.length > 0 || configured) && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "90ms" }}>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setSubsOpen((v) => !v)}
              className="flex min-w-0 flex-1 items-center gap-2 text-left"
            >
              <Rss size={15} className="shrink-0 text-clay" strokeWidth={1.8} />
              <span className="text-sm font-semibold text-ink">我的订阅</span>
              {subs.length > 0 && (
                <span className="rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                  {subs.length}
                </span>
              )}
              <span className="truncate text-xs text-ink-soft/70">
                定时搜索关键词、过滤新集并自动下载（间隔见设置页）
              </span>
            </button>
            <button
              onClick={subscribeFromResult}
              disabled={!configured}
              className="flex shrink-0 items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
              title={configured ? "新建订阅" : "先配置 PanSou 服务地址"}
            >
              <Plus size={13} />
              新建
            </button>
          </div>

          {subsOpen && (
            <ul className="mt-3 divide-y divide-ink/10 border-t border-ink/10 pt-1">
              {subs.length === 0 ? (
                <li className="py-4 text-center text-xs text-ink-soft/70">
                  暂无订阅。搜索到想要的资源后点「订阅」，新集更新会自动下载到本地。
                </li>
              ) : (
                subs.map((sub) => (
                  <li key={sub.id} className="flex items-center gap-3 py-3">
                    <button
                      role="switch"
                      aria-checked={sub.enabled}
                      onClick={() => toggleSub(sub)}
                      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
                        sub.enabled ? "bg-clay" : "bg-ink/15"
                      }`}
                      title={sub.enabled ? "点击暂停订阅" : "点击启用订阅"}
                    >
                      <span
                        className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all ${
                          sub.enabled ? "left-[18px]" : "left-0.5"
                        }`}
                      />
                    </button>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm text-ink">{sub.keyword}</p>
                      <p className="truncate text-[11px] text-ink-soft/70" title={sub.lastResult}>
                        {sub.lastRunAt === 0
                          ? "尚未检查（新建订阅将在下个周期自动检查）"
                          : sub.lastResult || "已检查"}
                      </p>
                    </div>
                    <button
                      onClick={() => runSub(sub)}
                      disabled={subBusy !== null}
                      className="flex shrink-0 items-center gap-1.5 rounded-ctrl border border-ink/15 px-2.5 py-1.5 text-xs text-ink transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
                      title="立即检查更新"
                    >
                      {subBusy === sub.id ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <RefreshCw size={12} />
                      )}
                      检查
                    </button>
                    <button
                      onClick={() => removeSub(sub.id)}
                      className="shrink-0 rounded-ctrl border border-ink/15 px-2 py-1.5 text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
                      title="删除订阅"
                    >
                      <Trash2 size={12} />
                    </button>
                  </li>
                ))
              )}
            </ul>
          )}
        </section>
      )}

      {/* 提示条 */}
      {error && (
        <div className="rounded-ctrl bg-danger/10 px-4 py-2.5 text-sm text-danger">{error}</div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-success/10 px-4 py-2.5 text-sm text-ink">{notice}</div>
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
                  <span className="shrink-0 rounded-full bg-clay px-2.5 py-0.5 font-mono text-[10px] font-semibold tracking-widest text-on-accent">
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
                    onClick={() => setSubForm({ keyword: kw.trim() || item.note, platform: supported ? item.type : "", regex: "" })}
                    className="flex shrink-0 items-center gap-1.5 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
                    title="订阅该关键词，更新自动下载"
                  >
                    <Bell size={13} />
                    订阅
                  </button>
                  <button
                    onClick={() => goResolve(item)}
                    disabled={!supported}
                    className={`flex shrink-0 items-center gap-1.5 rounded-ctrl px-3 py-1.5 text-xs font-semibold transition-colors ${
                      supported
                        ? "bg-clay text-on-accent hover:bg-clay-deep"
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

      {/* 新建订阅对话框 */}
      {subForm && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
          onClick={() => !subSubmitting && setSubForm(null)}
        >
          <div
            className="w-full max-w-md rounded-card bg-carrier p-6 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between">
              <h3 className="flex items-center gap-2 text-sm font-semibold text-ink">
                <Rss size={15} className="text-clay" />
                新建订阅
              </h3>
              <button
                onClick={() => setSubForm(null)}
                className="rounded-ctrl p-1 text-ink-soft transition-colors hover:text-ink"
              >
                <X size={15} />
              </button>
            </div>
            <div className="mt-4 space-y-3">
              <div>
                <label className="text-xs text-ink-soft">搜索关键词</label>
                <input
                  value={subForm.keyword}
                  onChange={(e) => setSubForm({ ...subForm, keyword: e.currentTarget.value })}
                  onKeyDown={(e) => e.key === "Enter" && submitSubForm()}
                  autoFocus
                  placeholder="如：三体 4K"
                  className="mt-1 h-10 w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-3 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                />
              </div>
              <div>
                <label className="text-xs text-ink-soft">优先平台</label>
                <select
                  value={subForm.platform}
                  onChange={(e) => setSubForm({ ...subForm, platform: e.currentTarget.value })}
                  className="mt-1 h-10 w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-3 text-sm text-ink focus:border-clay focus:outline-none"
                >
                  {SUB_PLATFORM_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="text-xs text-ink-soft">文件名过滤正则（可选）</label>
                <input
                  value={subForm.regex}
                  onChange={(e) => setSubForm({ ...subForm, regex: e.currentTarget.value })}
                  onKeyDown={(e) => e.key === "Enter" && submitSubForm()}
                  placeholder="如：4K|1080（只下载匹配的文件，留空不过滤）"
                  className="mt-1 h-10 w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                />
              </div>
              <p className="text-[11px] text-ink-soft/70">
                订阅后按设置页的检查间隔定时搜索：识别到新集数（S01E02 / 第12集 / EP03 等）即自动解析下载，已下载的集数不会重复。
              </p>
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button
                onClick={() => setSubForm(null)}
                disabled={subSubmitting}
                className="rounded-ctrl border border-ink/15 px-4 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay disabled:opacity-50"
              >
                取消
              </button>
              <button
                onClick={submitSubForm}
                disabled={!subForm.keyword.trim() || subSubmitting}
                className="flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-semibold text-on-accent transition-colors hover:bg-clay-deep disabled:opacity-50"
              >
                {subSubmitting && <Loader2 size={12} className="animate-spin" />}
                订阅
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
