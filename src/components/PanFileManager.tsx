import { useEffect, useState } from "react";
import {
  ArrowDownToLine,
  ArrowLeft,
  ChevronRight,
  FileText,
  FolderOpen,
  FolderTree,
  Loader2,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { errMsg, ipc, type ShareFile } from "../lib/ipc";
import { formatBytes, platformLabel } from "../lib/format";
import CrossDriveSearchModal from "./CrossDriveSearchModal";
import type { TabId } from "../lib/tabs";

interface PanFileManagerProps {
  platform: string;
  nickname: string;
  onBack: () => void;
  onNavigate: (tab: TabId) => void;
  onGoResolve?: (url: string, pwd?: string) => void;
}

interface DirCrumb {
  fid: string;
  name: string;
}

export default function PanFileManager({
  platform,
  nickname,
  onBack,
  onNavigate,
  onGoResolve,
}: PanFileManagerProps) {
  const [crumbs, setCrumbs] = useState<DirCrumb[]>([{ fid: "0", name: "根目录" }]);
  const [files, setFiles] = useState<ShareFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [downloadingFid, setDownloadingFid] = useState<string | null>(null);
  const [searchModalFilename, setSearchModalFilename] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  const currentDir = crumbs[crumbs.length - 1];

  useEffect(() => {
    loadDir(currentDir.fid);
  }, [currentDir.fid]);

  async function loadDir(fid: string) {
    setLoading(true);
    setError("");
    try {
      const list = await ipc.listPersonalFiles(platform, fid);
      setFiles(list);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }

  function enterDir(file: ShareFile) {
    setCrumbs((prev) => [...prev, { fid: file.fid, name: file.fname }]);
  }

  function jumpToCrumb(index: number) {
    if (index === crumbs.length - 1) return;
    setCrumbs((prev) => prev.slice(0, index + 1));
  }

  async function downloadPersonalFile(file: ShareFile) {
    if (downloadingFid) return;
    setDownloadingFid(file.fid);
    setError("");
    try {
      const link = await ipc.getPersonalDownloadLink(platform, file);
      await ipc.enqueueDownload(
        link.url,
        link.filename || file.fname,
        link.headers,
        link.platform,
        link.cleanupId || undefined,
        link.mirrors || undefined,
      );
      setNotice(`已加入下载队列：${link.filename || file.fname}`);
      setTimeout(() => setNotice(""), 3000);
      onNavigate("download");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setDownloadingFid(null);
    }
  }

  return (
    <div className="space-y-4 animate-fade">
      {/* 顶部工具栏与面包屑 */}
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-card bg-carrier p-4 border border-ink/10">
        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay"
          >
            <ArrowLeft size={14} />
            返回网盘
          </button>
          <div className="flex items-center gap-2">
            <span className="rounded bg-clay/10 px-2 py-0.5 font-mono text-xs font-semibold text-clay-deep">
              {platformLabel(platform)}
            </span>
            <span className="text-xs font-medium text-ink truncate max-w-[150px]">
              {nickname || "个人空间"}
            </span>
          </div>
        </div>

        <button
          onClick={() => loadDir(currentDir.fid)}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-ctrl border border-ink/10 px-3 py-1.5 text-xs text-ink-soft hover:text-ink hover:border-ink/20 disabled:opacity-50 transition-colors"
        >
          <RefreshCw size={13} className={loading ? "animate-spin text-clay" : ""} />
          <span>刷新</span>
        </button>
      </div>

      {/* 面包屑导航条 */}
      <div className="flex items-center gap-1.5 rounded-card bg-carrier-deep/50 px-4 py-2.5 text-xs text-ink border border-ink/5 overflow-x-auto">
        <FolderTree size={14} className="shrink-0 text-clay mr-1" />
        {crumbs.map((c, i) => (
          <div key={i} className="flex items-center gap-1.5 shrink-0">
            {i > 0 && <ChevronRight size={12} className="text-ink-soft/60" />}
            <button
              onClick={() => jumpToCrumb(i)}
              className={`hover:text-clay transition-colors ${
                i === crumbs.length - 1 ? "font-semibold text-clay-deep" : "text-ink-soft"
              }`}
            >
              {c.name}
            </button>
          </div>
        ))}
      </div>

      {/* 提示条 */}
      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-xs text-clay-deep">
          {error}
        </div>
      )}
      {notice && (
        <div className="rounded-ctrl bg-cactus/20 px-4 py-2.5 text-xs text-ink font-medium">
          {notice}
        </div>
      )}

      {/* 文件列表区 */}
      <div className="rounded-card bg-carrier border border-ink/10 overflow-hidden">
        {loading ? (
          <div className="flex flex-col items-center justify-center py-16 text-ink-soft">
            <Loader2 size={24} className="animate-spin text-clay" />
            <p className="mt-3 text-xs">正在读取云端目录…</p>
          </div>
        ) : files.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-ink-soft">
            <FolderOpen size={32} className="text-ink-soft/40" />
            <p className="mt-3 text-xs">该目录下暂无文件</p>
          </div>
        ) : (
          <ul className="divide-y divide-ink/10">
            {files.map((file) => (
              <li
                key={file.fid}
                className="flex items-center gap-3 px-5 py-3 hover:bg-carrier-deep/40 transition-colors"
              >
                {file.isdir ? (
                  <FolderOpen size={18} className="shrink-0 text-clay" />
                ) : (
                  <FileText size={18} className="shrink-0 text-ink-soft" />
                )}

                <div className="min-w-0 flex-1">
                  {file.isdir ? (
                    <button
                      onClick={() => enterDir(file)}
                      className="truncate text-left text-xs font-medium text-ink hover:text-clay transition-colors block max-w-full"
                      title={file.fname}
                    >
                      {file.fname}
                    </button>
                  ) : (
                    <p className="truncate text-xs font-medium text-ink" title={file.fname}>
                      {file.fname}
                    </p>
                  )}
                  {file.modifyTime && (
                    <p className="font-mono text-[10px] text-ink-soft/60 mt-0.5">
                      {file.modifyTime}
                    </p>
                  )}
                </div>

                <span className="shrink-0 font-mono text-xs text-ink-soft">
                  {file.isdir ? "文件夹" : formatBytes(file.fsize)}
                </span>

                <div className="flex shrink-0 items-center gap-2">
                  {!file.isdir && (
                    <button
                      onClick={() => setSearchModalFilename(file.fname)}
                      className="flex items-center gap-1 rounded-ctrl border border-ink/15 px-2.5 py-1 text-xs text-ink-soft hover:border-clay hover:text-clay transition-colors"
                      title="在其他网盘中搜索同款资源"
                    >
                      <Sparkles size={12} className="text-clay" />
                      <span>搜同款</span>
                    </button>
                  )}

                  {!file.isdir ? (
                    <button
                      onClick={() => downloadPersonalFile(file)}
                      disabled={downloadingFid === file.fid}
                      className="flex items-center gap-1.5 rounded-ctrl bg-clay px-3 py-1 text-xs font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
                    >
                      {downloadingFid === file.fid ? (
                        <Loader2 size={13} className="animate-spin" />
                      ) : (
                        <ArrowDownToLine size={13} />
                      )}
                      <span>{downloadingFid === file.fid ? "取链中…" : "直接下载"}</span>
                    </button>
                  ) : (
                    <button
                      onClick={() => enterDir(file)}
                      className="rounded-ctrl border border-ink/15 px-3 py-1 text-xs text-ink hover:border-clay hover:text-clay transition-colors"
                    >
                      打开
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* 跨网盘搜同款弹窗 */}
      <CrossDriveSearchModal
        open={Boolean(searchModalFilename)}
        filename={searchModalFilename || ""}
        onClose={() => setSearchModalFilename(null)}
        onResolveShare={(url, pwd) => {
          setSearchModalFilename(null);
          onGoResolve?.(url, pwd);
        }}
      />
    </div>
  );
}
