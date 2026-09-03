import { useEffect, useState } from "react";
import {
  Loader2,
  Search,
  Sparkles,
  X,
  Zap,
} from "lucide-react";
import { errMsg, ipc, type SearchItem } from "../lib/ipc";
import { platformLabel } from "../lib/format";

/** 自动清洗文件名称，提取核心搜索关键词 */
export function cleanSearchKeyword(filename: string): string {
  return filename
    // 去除文件格式后缀
    .replace(/\.(zip|rar|7z|tar|gz|mp4|mkv|avi|wmv|iso|exe|apk|pdf|epub|flac|mp3|docx|pptx)$/i, "")
    // 去除分卷标号如 .part01, .z01, .001
    .replace(/\.(part\d+|z\d+|00\d+)$/i, "")
    // 去除常见画质/发布标签
    .replace(/\[(4k|1080p|720p|hdr|remux|bdrip|web-dl|x264|x265|hevc)[^\]]*\]/gi, "")
    .replace(/\((4k|1080p|720p|hdr|remux|bdrip|web-dl|x264|x265|hevc)[^\)]*\)/gi, "")
    // 去除结尾重复下载序号如 (1), (2)
    .replace(/\(\d+\)$/, "")
    // 下划线与点替换为空格
    .replace(/[._]/g, " ")
    .trim();
}

interface CrossDriveSearchModalProps {
  open: boolean;
  filename: string;
  onClose: () => void;
  onResolveShare: (url: string, pwd?: string) => void;
}

export default function CrossDriveSearchModal({
  open,
  filename,
  onClose,
  onResolveShare,
}: CrossDriveSearchModalProps) {
  const [keyword, setKeyword] = useState("");
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<SearchItem[]>([]);
  const [filterPlatform, setFilterPlatform] = useState<string>("all");
  const [error, setError] = useState("");

  useEffect(() => {
    if (open && filename) {
      const cleaned = cleanSearchKeyword(filename);
      setKeyword(cleaned || filename);
      doSearch(cleaned || filename);
    } else {
      setResults([]);
      setError("");
    }
  }, [open, filename]);

  async function doSearch(q: string) {
    const trimmed = q.trim();
    if (!trimmed) return;
    setLoading(true);
    setError("");
    try {
      const list = await ipc.pansouSearch(trimmed);
      setResults(list);
      if (list.length === 0) {
        setError("未在其他网盘中检索到同名的有效资源");
      }
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }

  if (!open) return null;

  // 过滤当前展示平台（优先推荐夸克/UC/123/迅雷等高速网盘）
  const displayResults = results.filter((item) => {
    if (filterPlatform === "all") return true;
    return item.type === filterPlatform;
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm animate-fade">
      <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-card border border-ink/10 bg-carrier shadow-2xl">
        {/* 标题栏 */}
        <div className="flex items-center justify-between border-b border-ink/10 px-5 py-4">
          <div className="flex items-center gap-2">
            <Sparkles size={18} className="text-clay" />
            <h3 className="text-base font-semibold text-ink">跨网盘秒搜同款资源</h3>
            <span className="rounded-full bg-clay/10 px-2.5 py-0.5 text-[11px] font-medium text-clay-deep">
              绕过限速·免VIP满速下载
            </span>
          </div>
          <button
            onClick={onClose}
            className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-ink/5 hover:text-ink"
          >
            <X size={18} />
          </button>
        </div>

        {/* 搜索输入与关键词 */}
        <div className="border-b border-ink/10 bg-carrier-deep/40 px-5 py-3.5">
          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-soft" />
              <input
                type="text"
                value={keyword}
                onChange={(e) => setKeyword(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") doSearch(keyword);
                }}
                placeholder="搜索同名资源关键词…"
                className="h-9 w-full rounded-ctrl border border-ink/10 bg-carrier pl-8 pr-3 text-xs text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
              />
            </div>
            <button
              onClick={() => doSearch(keyword)}
              disabled={loading || !keyword.trim()}
              className="flex h-9 items-center gap-1.5 rounded-ctrl bg-clay px-4 text-xs font-semibold text-white hover:bg-clay-deep disabled:opacity-50"
            >
              {loading ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />}
              <span>搜索</span>
            </button>
          </div>

          {/* 快捷网盘筛选器 */}
          <div className="mt-2.5 flex items-center gap-2">
            <span className="text-[11px] text-ink-soft">平台筛选：</span>
            {[
              { id: "all", label: "全部" },
              { id: "quark", label: "夸克 (满速)" },
              { id: "uc", label: "UC (满速)" },
              { id: "pan123", label: "123 (满速)" },
              { id: "xunlei", label: "迅雷" },
            ].map((p) => (
              <button
                key={p.id}
                onClick={() => setFilterPlatform(p.id)}
                className={`rounded-ctrl px-2.5 py-0.5 text-[11px] font-medium transition-colors ${
                  filterPlatform === p.id
                    ? "bg-clay text-white"
                    : "bg-carrier border border-ink/10 text-ink-soft hover:text-ink"
                }`}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>

        {/* 结果列表 */}
        <div className="flex-1 overflow-y-auto p-5 space-y-2.5">
          {loading && (
            <div className="flex flex-col items-center justify-center py-12 text-ink-soft">
              <Loader2 size={24} className="animate-spin text-clay" />
              <p className="mt-3 text-xs">正在全网并发聚合搜索不限速同款资源…</p>
            </div>
          )}

          {!loading && error && (
            <div className="rounded-ctrl bg-clay/10 p-4 text-center text-xs text-clay-deep">
              {error}
            </div>
          )}

          {!loading && displayResults.length > 0 && (
            displayResults.map((item, idx) => (
              <div
                key={idx}
                className="flex items-center justify-between gap-4 rounded-ctrl border border-ink/10 bg-carrier-deep/60 p-3 hover:border-clay/40 transition-colors"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="shrink-0 rounded bg-ink/10 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-ink">
                      {platformLabel(item.type)}
                    </span>
                    <p className="truncate text-xs font-medium text-ink" title={item.url}>
                      {item.url}
                    </p>
                  </div>
                  {item.password && (
                    <p className="mt-1 font-mono text-[11px] text-ink-soft">
                      提取码：<span className="font-semibold text-clay">{item.password}</span>
                    </p>
                  )}
                </div>
                <button
                  onClick={() => {
                    onClose();
                    onResolveShare(item.url, item.password);
                  }}
                  className="flex shrink-0 items-center gap-1.5 rounded-ctrl bg-clay/10 px-3 py-1.5 text-xs font-semibold text-clay-deep hover:bg-clay hover:text-white transition-colors"
                >
                  <Zap size={13} />
                  <span>立即解析</span>
                </button>
              </div>
            ))
          )}
        </div>

        {/* 底部提示 */}
        <div className="flex items-center justify-between border-t border-ink/10 bg-carrier-deep/30 px-5 py-2.5 text-[11px] text-ink-soft/80">
          <span>提示：夸克、UC 与 123 云盘对普通免费账号不限速，转存即可享最高数十 MB/s 下载。</span>
          <button
            onClick={onClose}
            className="rounded-ctrl px-3 py-1 text-xs text-ink-soft hover:text-ink"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
