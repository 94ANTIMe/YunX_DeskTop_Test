import { ipc, type CollectedFile, type ShareFile } from "./ipc";

/** 单文件取链并入队，返回实际入队文件名（relDir 用于还原分享目录结构） */
export async function fetchAndEnqueue(
  sessionKey: string,
  file: ShareFile,
  relDir?: string,
): Promise<string> {
  const link = await ipc.getDownloadLink(sessionKey, file);
  const base = link.filename || file.fname;
  const outName = relDir ? `${relDir}/${base}` : base;
  await ipc.enqueueDownload(
    link.url,
    outName,
    link.headers,
    link.platform,
    link.cleanupId || undefined,
    link.mirrors || undefined,
    link.fetchCtx || undefined,
  );
  return outName;
}

/** 文件夹递归收集（扁平化文件 + 相对目录 relDir） */
export function collectFolder(sessionKey: string, dirId: string): Promise<CollectedFile[]> {
  return ipc.collectFolderFiles(sessionKey, dirId);
}

/** 行文本是否像一条链接（批量队列候选行过滤） */
export function looksLikeLink(line: string): boolean {
  return /https?:\/\/|magnet:\?xt=/i.test(line);
}
