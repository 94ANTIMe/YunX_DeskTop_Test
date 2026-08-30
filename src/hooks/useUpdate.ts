import { useEffect, useState } from "react";
import { errMsg, ipc, onUpdateProgress, type UpdateInfo } from "../lib/ipc";

/** 在线更新操作封装：检查 / 下载（带进度）/ 安装。横幅与设置页共用。 */
export function useUpdate() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<{ received: number; total: number } | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");

  // 订阅下载进度事件
  useEffect(() => {
    const un = onUpdateProgress((p) => setProgress(p));
    return () => {
      un.then((f) => f());
    };
  }, []);

  /** 在线检查是否可更新 */
  async function check() {
    setChecking(true);
    setError("");
    try {
      const result = await ipc.checkUpdate();
      setInfo(result);
      return result;
    } catch (e) {
      setError(errMsg(e));
      return null;
    } finally {
      setChecking(false);
    }
  }

  /** 下载最新安装包（后端上报进度事件） */
  async function download() {
    setDownloading(true);
    setError("");
    setProgress({ received: 0, total: 0 });
    try {
      const path = await ipc.downloadUpdate();
      setProgress((p) => (p ? { ...p, received: p.total } : p));
      return path;
    } catch (e) {
      setError(errMsg(e));
      return null;
    } finally {
      setDownloading(false);
    }
  }

  /** 静默安装已下载的安装包（装完应用自动退出重启） */
  async function install(path: string) {
    setInstalling(true);
    setError("");
    try {
      await ipc.installUpdate(path);
    } catch (e) {
      setError(errMsg(e));
      setInstalling(false);
    }
  }

  return {
    info,
    checked: info !== null,
    checking,
    download,
    downloading,
    progress,
    install,
    installing,
    error,
    check,
    // 供板块本地重置错误
    clearError: () => setError(""),
  };
}