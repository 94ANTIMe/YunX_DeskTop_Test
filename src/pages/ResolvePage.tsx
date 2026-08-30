import { useEffect, useRef, useState } from "react";
import {
  ArrowDownToLine,
  ArrowLeft,
  Bookmark,
  BookmarkPlus,
  ChevronRight,
  FileText,
  FolderOpen,
  History,
  Link2,
  Loader2,
  Trash2,
} from "lucide-react";
import PageHeader from "../components/PageHeader";
import { errMsg, ipc, type Bookmark as BookmarkRow, type ResolveHistory, type ResolveSessionInfo, type ShareFile } from "../lib/ipc";
import { formatBytes, platformLabel } from "../lib/format";
import type { TabId } from "../lib/tabs";
import resolveHero from "../assets/art/resolve-hero.jpg";

interface ResolvePageProps {
  onNavigate: (tab: TabId) => void;
  /** 搜索页转入的待解析链接（消费后由 onPendingConsumed 清空） */
  pending?: { link: string; pwd: string } | null;
  onPendingConsumed?: () => void;
}

interface DirStackEntry {
  fid: string;
  name: string;
}

/** 解析页：粘贴链接 → 建会话 → 文件树导航 → 取链入队下载 + 收藏 */
export default function ResolvePage({ onNavigate, pending, onPendingConsumed }: ResolvePageProps) {
  const [input, setInput] = useState("");
  const [pwd, setPwd] = useState("");
  const [session, setSession] = useState<ResolveSessionInfo | null>(null);
  const [dirStack, setDirStack] = useState<DirStackEntry[]>([]);
  const [files, setFiles] = useState<ShareFile[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [page, setPage] = useState(1);
  const [resolving, setResolving] = useState(false);
  const [loadingDir, setLoadingDir] = useState(false);
  const [downloadingFid, setDownloadingFid] = useState<string | null>(null);
  const [folderBusy, setFolderBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  /** 文件夹收集进度（收集 + 逐个取链入队） */
  const [folderProgress, setFolderProgress] = useState<{ name: string; done: number; total: number } | null>(null);
  const [showBookmarks, setShowBookmarks] = useState(false);
  const [bookmarks, setBookmarks] = useState<BookmarkRow[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [history, setHistory] = useState<ResolveHistory[]>([]);
  const noticeTimer = useRef<number | undefined>(undefined);

  const showNotice = (msg: string) => {
    setNotice(msg);
    window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(""), 4000);
  };

  // 解析（text 缺省取输入框内容；搜索页转入时传「链接 + 提取码」组合文本）
  async function resolve(text?: string) {
    const t = (text ?? input).trim();
    if (!t || resolving) return;
    setResolving(true);
    setError("");
    try {
      const info = await ipc.resolveShare(t);
      setSession(info);
      setFiles(info.files);
      setHasMore(info.hasMore);
      setDirStack([{ fid: "0", name: info.title || "根目录" }]);
      setPage(1);
      if (info.title) showNotice(`已解析：${info.title}`);
      ipc.listResolveHistory().then(setHistory).catch(() => {});
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setResolving(false);
    }
  }

  // 搜索页转入：填入链接 + 提取码并立即解析
  useEffect(() => {
    if (!pending) return;
    const link = pending.link.trim();
    if (!link) return;
    setInput(link);
    setPwd(pending.pwd);
    onPendingConsumed?.();
    const text = pending.pwd ? `${link} 提取码：${pending.pwd}` : link;
    resolve(text);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending]);

  // 目录导航
  async function openDir(entry: ShareFile) {
    if (!session || loadingDir) return;
    setLoadingDir(true);
    setError("");
    try {
      const result = await ipc.listShareFiles(session.sessionKey, entry.fid, 1);
      setFiles(result.files);
      setHasMore(result.hasMore);
      setDirStack((s) => [...s, { fid: entry.fid, name: entry.fname }]);
      setPage(1);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoadingDir(false);
    }
  }

  // 返回上级
  async function backTo(index: number) {
    if (!session || dirStack.length <= index + 1 || loadingDir) return;
    const target = dirStack[index];
    setLoadingDir(true);
    try {
      const result = await ipc.listShareFiles(session.sessionKey, target.fid, 1);
      setFiles(result.files);
      setHasMore(result.hasMore);
      setDirStack((s) => s.slice(0, index + 1));
      setPage(1);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoadingDir(false);
    }
  }

  // 加载更多
  async function loadMore() {
    if (!session || loadingDir || !hasMore) return;
    setLoadingDir(true);
    try {
      const next = page + 1;
      const result = await ipc.listShareFiles(
        session.sessionKey,
        dirStack[dirStack.length - 1].fid,
        next,
      );
      setFiles((f) => [...f, ...result.files]);
      setHasMore(result.hasMore);
      setPage(next);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoadingDir(false);
    }
  }

  // 单文件下载：取链 → 入队
  async function downloadFile(file: ShareFile) {
    if (!session || downloadingFid) return;
    setDownloadingFid(file.fid);
    setError("");
    try {
      const link = await ipc.getDownloadLink(session.sessionKey, file);
      await ipc.enqueueDownload(
        link.url,
        link.filename || file.fname,
        link.headers,
        link.platform,
        link.cleanupId || undefined,
      );
      showNotice(`已加入下载：${link.filename || file.fname}`);
      onNavigate("download");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setDownloadingFid(null);
    }
  }

  // 文件夹下载：递归收集 → 逐个取链入队（带实时进度；切换栏目不中断）
  async function downloadFolder(entry: ShareFile) {
    if (!session || folderBusy) return;
    setFolderBusy(entry.fid);
    setFolderProgress(null);
    setError("");
    try {
      const collected = await ipc.collectFolderFiles(session.sessionKey, entry.fid);
      if (collected.length === 0) {
        showNotice("该文件夹为空");
        return;
      }
      setFolderProgress({ name: entry.fname, done: 0, total: collected.length });
      let done = 0;
      let failed = 0;
      for (const file of collected) {
        try {
          const link = await ipc.getDownloadLink(session.sessionKey, file);
          await ipc.enqueueDownload(
            link.url,
            link.filename || file.fname,
            link.headers,
            link.platform,
            link.cleanupId || undefined,
          );
          done++;
        } catch {
          failed++;
        }
        setFolderProgress((p) => (p ? { ...p, done: done + failed } : p));
      }
      showNotice(`已入队 ${done} 个文件${failed > 0 ? `，${failed} 个失败（详见日志）` : ""}`);
      if (done > 0) onNavigate("download");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setFolderBusy(null);
      setFolderProgress(null);
    }
  }

  // 收藏当前链接
  async function bookmarkCurrent() {
    const link = input.trim();
    if (!link) return;
    try {
      await ipc.addBookmark(link, session?.title ?? "", pwd);
      showNotice("已收藏该链接");
    } catch (e) {
      setError(errMsg(e));
    }
  }

  // 收藏列表
  async function openBookmarks() {
    try {
      setBookmarks(await ipc.listBookmarks());
      setShowBookmarks(true);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function removeBookmark(id: number) {
    try {
      await ipc.removeBookmark(id);
      setBookmarks((b) => b.filter((x) => x.id !== id));
    } catch (e) {
      setError(errMsg(e));
    }
  }

  // 解析历史
  async function openHistory() {
    try {
      setHistory(await ipc.listResolveHistory());
      setShowHistory(true);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function removeHistory(id: number) {
    try {
      await ipc.deleteResolveHistory(id);
      setHistory((h) => h.filter((x) => x.id !== id));
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function clearHistory() {
    try {
      await ipc.clearResolveHistory();
      setHistory([]);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  // 输入变化自动匹配提取码（文本中「提取码: xxxx」）
  useEffect(() => {
    if (!input) return;
    const m = input.match(/(?:提取码|访问码|密码)[：:]\s*([A-Za-z0-9]{4,8})/);
    if (m && !pwd) setPwd(m[1]);
  }, [input]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="space-y-6">
      <PageHeader tab="resolve" subtitle="粘贴网盘分享链接，自动识别平台与提取码">
        <div className="flex items-center gap-2">
          <button
            onClick={bookmarkCurrent}
            disabled={!input.trim()}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-40"
          >
            <BookmarkPlus size={14} />
            收藏此链接
          </button>
          <button
            onClick={openBookmarks}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
          >
            <Bookmark size={14} />
            收藏夹
          </button>
          <button
            onClick={openHistory}
            className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep"
          >
            <History size={14} />
            解析记录
          </button>
        </div>
      </PageHeader>

      {/* 输入区 */}
      <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <div className="flex gap-4">
          {/* hero 插画（未解析时展示） */}
          {!session && (
            <img
              src={resolveHero}
              alt=""
              draggable={false}
              className="hidden h-44 w-44 shrink-0 rounded-card object-cover md:block"
            />
          )}
          <div className="min-w-0 flex-1">
            <textarea
              value={input}
              onChange={(e) => setInput(e.currentTarget.value)}
              placeholder="粘贴分享链接或整段分享文案，如 https://pan.quark.cn/s/xxxxxxxx"
              className="h-24 w-full resize-none rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-3 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
            />
            <div className="mt-3 flex items-center gap-3">
              <input
                value={pwd}
                onChange={(e) => setPwd(e.currentTarget.value)}
                placeholder="提取码（可自动识别）"
                className="w-36 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 py-1.5 text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
              />
              <button
                onClick={() => resolve()}
                disabled={!input.trim() || resolving}
                className="flex items-center gap-2 rounded-ctrl bg-clay px-5 py-2 text-sm font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
              >
                {resolving ? <Loader2 size={15} className="animate-spin" /> : <Link2 size={15} />}
                {resolving ? "解析中…" : "解析"}
              </button>
              <span className="text-xs text-ink-soft/70">支持：夸克 / UC / 迅雷 / 百度 / 139 / 123</span>
            </div>
          </div>
        </div>
      </section>

      {/* 提示条 */}
      {folderProgress && (
        <div className="rounded-ctrl bg-carrier px-4 py-2.5">
          <div className="flex items-center justify-between text-sm">
            <span className="min-w-0 truncate text-ink">
              正在处理「{folderProgress.name}」…
            </span>
            <span className="ml-3 shrink-0 font-mono text-xs text-clay-deep">
              {folderProgress.done}/{folderProgress.total}
            </span>
          </div>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-carrier-deep">
            <div
              className="h-full rounded-full bg-clay transition-all duration-300"
              style={{
                width: `${folderProgress.total > 0 ? Math.round((folderProgress.done / folderProgress.total) * 100) : 0}%`,
              }}
            />
          </div>
          <p className="mt-1.5 text-[11px] text-ink-soft/70">
            可切换到「日志」页查看每一步详情；切换栏目不会中断收集
          </p>
        </div>
      )}
      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-cactus/25 px-4 py-2.5 text-sm text-ink">{notice}</div>
      )}

      {/* 解析结果：面包屑 + 文件列表 */}
      {session && (
        <section className="animate-rise rounded-card bg-carrier p-6" style={{ animationDelay: "120ms" }}>
          {/* 面包屑 */}
          <div className="flex flex-wrap items-center gap-1 border-b border-ink/10 pb-4">
            <span className="rounded-full bg-clay px-2.5 py-0.5 font-mono text-[10px] font-semibold tracking-widest text-white">
              {platformLabel(session.platform)}
            </span>
            {dirStack.map((entry, i) => (
              <span key={`${entry.fid}-${i}`} className="flex items-center gap-1">
                {i > 0 && <ChevronRight size={13} className="text-ink-soft/50" />}
                <button
                  onClick={() => backTo(i)}
                  className={`rounded px-1.5 py-0.5 text-xs transition-colors ${
                    i === dirStack.length - 1
                      ? "font-semibold text-ink"
                      : "text-ink-soft hover:bg-carrier-deep hover:text-ink"
                  }`}
                >
                  {entry.name}
                </button>
              </span>
            ))}
            {dirStack.length > 1 && (
              <button
                onClick={() => backTo(dirStack.length - 2)}
                className="ml-2 flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-ink-soft hover:bg-carrier-deep hover:text-ink"
              >
                <ArrowLeft size={12} />
                返回上级
              </button>
            )}
            {loadingDir && <Loader2 size={14} className="animate-spin text-clay" />}
          </div>

          {/* 文件列表 */}
          {files.length === 0 ? (
            <p className="py-10 text-center text-sm text-ink-soft">
              {loadingDir ? "加载中…" : "此目录为空"}
            </p>
          ) : (
            <ul className="divide-y divide-ink/10">
              {files.map((file) => (
                <li key={file.fid} className="flex items-center gap-3 py-3">
                  {file.isdir ? (
                    <FolderOpen size={18} className="shrink-0 text-clay" />
                  ) : (
                    <FileText size={18} className="shrink-0 text-ink-soft" />
                  )}
                  <button
                    onClick={() => file.isdir && openDir(file)}
                    className="min-w-0 flex-1 truncate text-left text-sm text-ink hover:text-clay-deep"
                    title={file.fname}
                  >
                    {file.fname}
                  </button>
                  <span className="shrink-0 font-mono text-xs text-ink-soft">
                    {file.isdir ? "文件夹" : formatBytes(file.fsize)}
                  </span>
                  <button
                    onClick={() => (file.isdir ? downloadFolder(file) : downloadFile(file))}
                    disabled={downloadingFid !== null || folderBusy !== null}
                    className="flex shrink-0 items-center gap-1.5 rounded-ctrl bg-clay px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
                  >
                    {file.isdir ? (
                      folderBusy === file.fid ? <Loader2 size={13} className="animate-spin" /> : <ArrowDownToLine size={13} />
                    ) : (
                      downloadingFid === file.fid ? <Loader2 size={13} className="animate-spin" /> : <ArrowDownToLine size={13} />
                    )}
                    {file.isdir
                      ? folderBusy === file.fid
                        ? "收集中…"
                        : "下载全部"
                      : downloadingFid === file.fid
                        ? "取链中…"
                        : "下载"}
                  </button>
                </li>
              ))}
            </ul>
          )}

          {/* 加载更多 */}
          {hasMore && files.length > 0 && (
            <button
              onClick={loadMore}
              disabled={loadingDir}
              className="mt-4 w-full rounded-ctrl border border-ink/10 py-2 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
            >
              {loadingDir ? "加载中…" : "加载更多"}
            </button>
          )}
        </section>
      )}

      {/* 收藏夹浮层 */}
      {showBookmarks && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-ink/20 p-8 backdrop-blur-sm"
          onClick={() => setShowBookmarks(false)}
        >
          <div
            className="max-h-[70vh] w-full max-w-lg animate-rise overflow-hidden rounded-card bg-carrier shadow-capsule"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-ink/10 px-6 py-4">
              <h3 className="font-display text-lg font-semibold text-ink">收藏的分享链接</h3>
              <button
                onClick={() => setShowBookmarks(false)}
                className="rounded-ctrl px-2 py-1 text-xs text-ink-soft hover:bg-carrier-deep hover:text-ink"
              >
                关闭
              </button>
            </div>
            <div className="max-h-[52vh] overflow-y-auto px-6 py-2">
              {bookmarks.length === 0 ? (
                <p className="py-10 text-center text-sm text-ink-soft">暂无收藏</p>
              ) : (
                <ul className="divide-y divide-ink/10">
                  {bookmarks.map((b) => (
                    <li key={b.id} className="flex items-center gap-3 py-3">
                      <span className="shrink-0 rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                        {platformLabel(b.platform) || "未知"}
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm text-ink">{b.title || b.link}</p>
                        <p className="truncate font-mono text-[10px] text-ink-soft/70">{b.link}</p>
                      </div>
                      <button
                        onClick={() => {
                          setShowBookmarks(false);
                          setInput(b.link);
                          setPwd(b.pwd);
                        }}
                        className="shrink-0 rounded-ctrl bg-clay px-3 py-1 text-xs font-semibold text-white hover:bg-clay-deep"
                      >
                        解析
                      </button>
                      <button
                        onClick={() => removeBookmark(b.id)}
                        className="shrink-0 rounded-ctrl p-1.5 text-ink-soft hover:bg-clay/10 hover:text-clay-deep"
                        title="删除收藏"
                      >
                        <Trash2 size={14} />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 解析记录浮层 */}
      {showHistory && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-ink/20 p-8 backdrop-blur-sm"
          onClick={() => setShowHistory(false)}
        >
          <div
            className="max-h-[70vh] w-full max-w-lg animate-rise overflow-hidden rounded-card bg-carrier shadow-capsule"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-ink/10 px-6 py-4">
              <h3 className="font-display text-lg font-semibold text-ink">解析记录</h3>
              <div className="flex items-center gap-2">
                {history.length > 0 && (
                  <button
                    onClick={clearHistory}
                    className="rounded-ctrl px-2.5 py-1 text-xs text-ink-soft hover:bg-clay/10 hover:text-clay-deep"
                  >
                    清空记录
                  </button>
                )}
                <button
                  onClick={() => setShowHistory(false)}
                  className="rounded-ctrl px-2 py-1 text-xs text-ink-soft hover:bg-carrier-deep hover:text-ink"
                >
                  关闭
                </button>
              </div>
            </div>
            <div className="max-h-[52vh] overflow-y-auto px-6 py-2">
              {history.length === 0 ? (
                <p className="py-10 text-center text-sm text-ink-soft">暂无解析记录</p>
              ) : (
                <ul className="divide-y divide-ink/10">
                  {history.map((h) => (
                    <li key={h.id} className="flex items-center gap-3 py-3">
                      <span className="shrink-0 rounded-full bg-carrier-deep px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                        {platformLabel(h.platform) || "未知"}
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm text-ink">{h.title || "（无标题）"}</p>
                        <p className="truncate font-mono text-[10px] text-ink-soft/70">{h.link}</p>
                      </div>
                      <span className="shrink-0 font-mono text-[10px] text-ink-soft/60">
                        {new Date(h.createTime).toLocaleDateString()}
                      </span>
                      <button
                        onClick={() => {
                          setShowHistory(false);
                          setInput(h.link);
                          setPwd("");
                        }}
                        className="shrink-0 rounded-ctrl bg-clay px-3 py-1 text-xs font-semibold text-white hover:bg-clay-deep"
                      >
                        再解析
                      </button>
                      <button
                        onClick={() => removeHistory(h.id)}
                        className="shrink-0 rounded-ctrl p-1.5 text-ink-soft hover:bg-clay/10 hover:text-clay-deep"
                        title="删除记录"
                      >
                        <Trash2 size={14} />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
