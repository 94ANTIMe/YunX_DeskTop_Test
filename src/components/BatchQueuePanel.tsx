import { useEffect, useRef, useState } from "react";
import {
  CheckCircle2,
  ChevronDown,
  Download,
  Loader2,
  Plus,
  X,
  XCircle,
} from "lucide-react";
import { errMsg, ipc, type ShareFile } from "../lib/ipc";
import { collectFolder, fetchAndEnqueue, looksLikeLink } from "../lib/download";
import { formatBytes, platformLabel } from "../lib/format";

interface BatchQueuePanelProps {
  open: boolean;
  onClose: () => void;
  /** 入队完成后引导去下载页 */
  onGoDownload: () => void;
}

/** 队列条目状态机：pending → parsing → ready → downloading → done/partial/failed */
type ItemStatus = "pending" | "parsing" | "ready" | "failed" | "downloading" | "done" | "partial";

interface BatchFileRow {
  file: ShareFile;
  selected: boolean;
}

interface BatchItem {
  id: number;
  /** 原始行文本（resolveShare 直接吃原文，提取码自动识别） */
  raw: string;
  status: ItemStatus;
  error: string;
  title: string;
  platform: string;
  sessionKey: string;
  files: BatchFileRow[];
  /** 下载进度（total 随文件夹展开增长） */
  progress: { done: number; failed: number; total: number } | null;
  expanded: boolean;
}

/** 状态徽标文案与配色 */
const STATUS_META: Record<ItemStatus, { text: string; cls: string }> = {
  pending: { text: "等待解析", cls: "text-ink-soft" },
  parsing: { text: "解析中", cls: "text-clay" },
  ready: { text: "就绪", cls: "text-ink-soft" },
  failed: { text: "失败", cls: "text-clay-deep" },
  downloading: { text: "入队中", cls: "text-clay" },
  done: { text: "已全部入队", cls: "text-success" },
  partial: { text: "部分失败", cls: "text-clay-deep" },
};

/** 批量链接队列：粘贴多行链接 → 串行解析 → 默认全选 → 取链入队（右侧抽屉） */
export default function BatchQueuePanel({ open, onClose, onGoDownload }: BatchQueuePanelProps) {
  const [shown, setShown] = useState(open);
  const [leaving, setLeaving] = useState(false);
  const shownRef = useRef(open);
  shownRef.current = shown;

  const [text, setText] = useState("");
  const [items, setItems] = useState<BatchItem[]>([]);
  const [parsing, setParsing] = useState(false);
  const [downloadingAll, setDownloadingAll] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const nextId = useRef(1);
  // 进度补丁节流缓冲（见 flushProgress / scheduleProgress）
  const progressBuffer = useRef<Map<number, Partial<BatchItem>>>(new Map());
  const progressTimer = useRef<number | null>(null);

  // 进出动画（与 TaskDetailDrawer 同路径：右侧滑入滑出）
  useEffect(() => {
    if (open) {
      setShown(true);
      setLeaving(false);
    } else if (shownRef.current) {
      setLeaving(true);
    }
  }, [open]);

  useEffect(() => {
    if (!leaving) return;
    const timer = window.setTimeout(() => {
      setShown(false);
      setLeaving(false);
    }, 220);
    return () => window.clearTimeout(timer);
  }, [leaving]);

  // Esc 关闭
  useEffect(() => {
    if (!shown) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !leaving) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, leaving, onClose]);

  if (!shown) return null;

  function patchItem(id: number, patch: Partial<BatchItem>) {
    setItems((prev) => prev.map((it) => (it.id === id ? { ...it, ...patch } : it)));
  }

  /** 刷新缓冲的进度补丁（500ms 批量合帧） */
  function flushProgress() {
    if (progressTimer.current != null) {
      window.clearTimeout(progressTimer.current);
      progressTimer.current = null;
    }
    const buffered = progressBuffer.current;
    if (buffered.size === 0) return;
    progressBuffer.current = new Map();
    setItems((prev) =>
      prev.map((it) => {
        const patch = buffered.get(it.id);
        return patch ? { ...it, ...patch } : it;
      }),
    );
  }

  /** 进度补丁走 500ms 节流：文件夹逐文件进度不再触发逐次全列表拷贝 + 重渲染 */
  function scheduleProgress(id: number, patch: Partial<BatchItem>) {
    const buffered = progressBuffer.current;
    const existing = buffered.get(id);
    buffered.set(id, existing ? { ...existing, ...patch } : patch);
    if (progressTimer.current == null) {
      progressTimer.current = window.setTimeout(flushProgress, 500);
    }
  }

  /** 粘贴文本 → 候选行入队并串行解析 */
  async function addLines() {
    const lines = text
      .split(/\n+/)
      .map((l) => l.trim())
      .filter((l) => l && looksLikeLink(l));
    // 与现有队列去重
    const known = new Set(items.map((it) => it.raw));
    const fresh = [...new Set(lines)].filter((l) => !known.has(l));
    if (fresh.length === 0) {
      setError("没有识别到新的链接行（每行一条）");
      return;
    }
    setError("");
    const newItems: BatchItem[] = fresh.map((raw) => ({
      id: nextId.current++,
      raw,
      status: "pending",
      error: "",
      title: "",
      platform: "",
      sessionKey: "",
      files: [],
      progress: null,
      expanded: false,
    }));
    setItems((prev) => [...prev, ...newItems]);
    setText("");
    setParsing(true);
    for (const item of newItems) {
      await parseItem(item);
    }
    setParsing(false);
  }

  /** 串行解析单条：resolveShare 建会话（磁力/直链同链路）+ 补齐分页文件 */
  async function parseItem(item: BatchItem) {
    patchItem(item.id, { status: "parsing" });
    try {
      const info = await ipc.resolveShare(item.raw);
      const files: ShareFile[] = [...info.files];
      let page = 2;
      let hasMore = info.hasMore;
      while (hasMore && page <= 20) {
        const res = await ipc.listShareFiles(info.sessionKey, "0", page);
        files.push(...res.files);
        hasMore = res.hasMore;
        page++;
      }
      patchItem(item.id, {
        status: "ready",
        title: info.title || item.raw,
        platform: info.platform,
        sessionKey: info.sessionKey,
        files: files.map((f) => ({ file: f, selected: true })),
      });
    } catch (e) {
      patchItem(item.id, { status: "failed", error: errMsg(e) });
    }
  }

  /** 把一条链接的所选文件取链入队（文件夹递归收集还原目录结构） */
  async function downloadItem(item: BatchItem) {
    if (busyId || item.status === "downloading") return;
    const selected = item.files.filter((f) => f.selected);
    if (!item.sessionKey || selected.length === 0) return;
    setBusyId(item.id);
    setError("");
    patchItem(item.id, { status: "downloading", progress: { done: 0, failed: 0, total: selected.length } });
    let done = 0;
    let failed = 0;
    let total = selected.length;
    for (const f of selected) {
      try {
        if (f.file.isdir) {
          const collected = await collectFolder(item.sessionKey, f.file.fid);
          total += Math.max(0, collected.length - 1);
          for (const cf of collected) {
            try {
              await fetchAndEnqueue(item.sessionKey, cf, cf.relDir || undefined);
              done++;
            } catch {
              failed++;
            }
            scheduleProgress(item.id, { progress: { done, failed, total } });
          }
        } else {
          await fetchAndEnqueue(item.sessionKey, f.file);
          done++;
        }
      } catch {
        failed++;
      }
      scheduleProgress(item.id, { progress: { done, failed, total } });
    }
    // 丢弃未刷新的进度补丁再写终态（防止晚到的进度补丁覆盖终态的 progress: null）
    progressBuffer.current.delete(item.id);
    patchItem(item.id, {
      status: failed === 0 ? "done" : done > 0 ? "partial" : "failed",
      error: failed > 0 ? `${failed} 个文件取链失败（详见日志）` : "",
      progress: null,
    });
    setBusyId(null);
  }

  /** 一键下载全部：串行处理所有就绪条目 */
  async function downloadAll() {
    if (busyId || downloadingAll) return;
    setDownloadingAll(true);
    for (const it of items) {
      if (it.status === "ready" && it.files.some((f) => f.selected)) {
        await downloadItem(it);
      }
    }
    setDownloadingAll(false);
  }

  const readyItems = items.filter((it) => it.status === "ready" && it.files.some((f) => f.selected));
  // progress 在条目完成后清空不留存，用状态简化展示：done/partial 计为已入队条目
  const doneCount = items.filter((it) => it.status === "done" || it.status === "partial").length;

  return (
    <div className="fixed inset-0 z-[60]">
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm animate-fade" onClick={onClose} />
      <aside
        role="dialog"
        aria-modal="true"
        className={`absolute inset-y-0 right-0 flex w-[540px] max-w-[94vw] flex-col border-l border-ink/10 bg-carrier shadow-2xl ${
          leaving ? "animate-drawer-out" : "animate-drawer-in"
        }`}
      >
        {/* 头部 */}
        <header className="flex shrink-0 items-center justify-between border-b border-ink/10 px-5 py-4">
          <div>
            <p className="font-mono text-[11px] tracking-[0.3em] text-ink-soft">BATCH · QUEUE</p>
            <p className="mt-0.5 text-sm font-semibold text-ink">批量链接队列</p>
          </div>
          <button
            onClick={onClose}
            className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
            title="关闭 (Esc)"
          >
            <X size={16} />
          </button>
        </header>

        {/* 粘贴区 */}
        <div className="shrink-0 space-y-2 border-b border-ink/10 p-5">
          <textarea
            value={text}
            onChange={(e) => setText(e.currentTarget.value)}
            placeholder={"每行粘贴一条链接（自动识别平台与提取码）：\nhttps://pan.quark.cn/s/xxxx\nmagnet:?xt=urn:btih:…"}
            className="h-20 w-full resize-none rounded-ctrl border border-ink/10 bg-carrier-deep px-3 py-2.5 font-mono text-xs text-ink placeholder:text-ink-soft/50 focus:border-clay focus:outline-none"
          />
          <div className="flex items-center justify-between">
            <p className="text-[11px] text-ink-soft/70">
              支持六大网盘分享 / 磁力 / 直链混合粘贴；解析与入队均串行执行
            </p>
            <button
              onClick={() => void addLines()}
              disabled={!text.trim() || parsing}
              className="flex shrink-0 items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-semibold text-on-accent transition-colors hover:bg-clay-deep disabled:opacity-50"
            >
              {parsing ? <Loader2 size={13} className="animate-spin" /> : <Plus size={13} />}
              {parsing ? "解析中…" : "解析并添加"}
            </button>
          </div>
          {error && <p className="text-xs text-clay-deep">{error}</p>}
        </div>

        {/* 队列汇总条 */}
        {items.length > 0 && (
          <div className="flex shrink-0 items-center gap-3 border-b border-ink/10 bg-carrier-deep/40 px-5 py-2.5">
            <p className="text-xs text-ink-soft">
              共 {items.length} 条 · 就绪 {readyItems.length} · 已入队 {doneCount} 条
            </p>
            <button
              onClick={() => void downloadAll()}
              disabled={downloadingAll || busyId != null || readyItems.length === 0}
              className="ml-auto flex items-center gap-1.5 rounded-ctrl bg-clay px-4 py-1.5 text-xs font-semibold text-on-accent transition-colors hover:bg-clay-deep disabled:opacity-50"
              title="按默认全选，把所有就绪链接的文件依次取链入队"
            >
              {downloadingAll ? <Loader2 size={13} className="animate-spin" /> : <Download size={13} />}
              一键下载全部
            </button>
          </div>
        )}

        {/* 队列列表 */}
        <div className="min-h-0 flex-1 overflow-y-auto p-5 pt-4">
          {items.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center text-center">
              <Download size={28} className="text-ink-soft/40" />
              <p className="mt-3 text-sm text-ink-soft">队列为空</p>
              <p className="mt-1 text-xs text-ink-soft/70">
                在上方粘贴多条分享链接，解析后默认全选文件，可逐条取消勾选
              </p>
            </div>
          ) : (
            <div className="space-y-2.5">
              {items.map((it) => {
                const meta = STATUS_META[it.status];
                const selectedCount = it.files.filter((f) => f.selected).length;
                const totalSize = it.files
                  .filter((f) => f.selected)
                  .reduce((s, f) => s + (f.file.isdir ? 0 : f.file.fsize), 0);
                return (
                  <div key={it.id} className="rounded-card border border-ink/10 bg-carrier-deep/40">
                    {/* 条目主行 */}
                    <button
                      onClick={() => patchItem(it.id, { expanded: !it.expanded })}
                      className="flex w-full items-center gap-2.5 px-3.5 py-3 text-left"
                    >
                      {it.status === "parsing" || it.status === "downloading" ? (
                        <Loader2 size={14} className="shrink-0 animate-spin text-clay" />
                      ) : it.status === "done" ? (
                        <CheckCircle2 size={14} className="shrink-0 text-success" />
                      ) : it.status === "failed" || it.status === "partial" ? (
                        <XCircle size={14} className="shrink-0 text-clay-deep" />
                      ) : (
                        <span className="size-2 shrink-0 rounded-full bg-ink-soft/40" />
                      )}
                      <span className="shrink-0 rounded-full bg-carrier px-2 py-0.5 font-mono text-[10px] text-ink-soft">
                        {it.platform ? platformLabel(it.platform) : "识别中"}
                      </span>
                      <span className="min-w-0 flex-1 truncate text-xs font-medium text-ink" title={it.raw}>
                        {it.title || it.raw}
                      </span>
                      <span className={`shrink-0 text-[10px] font-medium ${meta.cls}`}>{meta.text}</span>
                      <ChevronDown
                        size={12}
                        className={`shrink-0 text-ink-soft transition-transform ${it.expanded ? "rotate-180" : ""}`}
                      />
                    </button>

                    {/* 错误 / 进度 */}
                    {it.status === "failed" && it.error && (
                      <p className="break-all px-3.5 pb-2.5 text-[11px] text-clay-deep">{it.error}</p>
                    )}
                    {it.status === "downloading" && it.progress && (
                      <div className="px-3.5 pb-3">
                        <div className="flex items-center justify-between font-mono text-[10px] text-ink-soft">
                          <span>
                            取链入队 {it.progress.done + it.progress.failed}/{it.progress.total}
                            {it.progress.failed > 0 ? ` · 失败 ${it.progress.failed}` : ""}
                          </span>
                        </div>
                        <div className="mt-1 h-1 overflow-hidden rounded-full bg-carrier">
                          <div
                            className="h-full w-full origin-left rounded-full bg-clay transition-transform duration-300"
                            style={{
                              transform: `scaleX(${it.progress.total > 0 ? Math.min(1, (it.progress.done + it.progress.failed) / it.progress.total) : 0})`,
                            }}
                          />
                        </div>
                      </div>
                    )}
                    {it.status === "partial" && it.error && (
                      <p className="px-3.5 pb-2.5 text-[11px] text-clay-deep">{it.error}</p>
                    )}

                    {/* 展开文件清单 */}
                    {it.expanded && it.files.length > 0 && (
                      <div className="border-t border-ink/10 px-3.5 py-2.5">
                        <div className="mb-2 flex items-center justify-between text-[10px] text-ink-soft/70">
                          <span>
                            已选 {selectedCount}/{it.files.length} 个 · 约 {formatBytes(totalSize)}
                          </span>
                          <span className="flex gap-2">
                            <button
                              className="transition-colors hover:text-clay"
                              onClick={(e) => {
                                e.stopPropagation();
                                patchItem(it.id, { files: it.files.map((f) => ({ ...f, selected: true })) });
                              }}
                            >
                              全选
                            </button>
                            <button
                              className="transition-colors hover:text-clay"
                              onClick={(e) => {
                                e.stopPropagation();
                                patchItem(it.id, { files: it.files.map((f) => ({ ...f, selected: false })) });
                              }}
                            >
                              全不选
                            </button>
                          </span>
                        </div>
                        <ul className="max-h-44 space-y-0.5 overflow-y-auto">
                          {it.files.map((f, idx) => (
                            <li key={`${f.file.fid}-${idx}`}>
                              <label className="flex cursor-pointer items-center gap-2 rounded-ctrl px-1.5 py-1 hover:bg-carrier">
                                <input
                                  type="checkbox"
                                  checked={f.selected}
                                  onChange={(e) =>
                                    patchItem(it.id, {
                                      files: it.files.map((x, i) =>
                                        i === idx ? { ...x, selected: e.currentTarget.checked } : x
                                      ),
                                    })
                                  }
                                  className="accent-clay"
                                />
                                {f.file.isdir ? (
                                  <span className="shrink-0 rounded-full bg-carrier px-1.5 py-0.5 text-[9px] text-ink-soft">
                                    目录
                                  </span>
                                ) : (
                                  <span className="w-14 shrink-0 text-right font-mono text-[10px] text-ink-soft/70">
                                    {formatBytes(f.file.fsize)}
                                  </span>
                                )}
                                <span className="min-w-0 flex-1 truncate text-[11px] text-ink" title={f.file.fname}>
                                  {f.file.fname}
                                </span>
                              </label>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}

                    {/* 条目操作 */}
                    {it.status === "ready" && (
                      <div className="flex items-center justify-end gap-2 border-t border-ink/10 px-3.5 py-2.5">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            void downloadItem(it);
                          }}
                          disabled={busyId != null || downloadingAll || selectedCount === 0}
                          className="flex items-center gap-1.5 rounded-ctrl border border-clay px-3 py-1 text-[11px] font-medium text-clay-deep transition-colors hover:bg-clay/10 disabled:opacity-40"
                        >
                          <Download size={12} />
                          下载所选（{selectedCount}）
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 底部 */}
        <footer className="flex shrink-0 items-center justify-between border-t border-ink/10 px-5 py-3">
          <p className="text-[10px] text-ink-soft/60">关闭面板仅收起，队列与状态保留（不入队的链接不受影响）</p>
          <button
            onClick={() => {
              onClose();
              onGoDownload();
            }}
            className="rounded-ctrl border border-ink/15 px-3 py-1 text-[11px] font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
          >
            前往下载页
          </button>
        </footer>
      </aside>
    </div>
  );
}
