import { useRef, useState } from "react";
import { check as checkForUpdate, type Update } from "@tauri-apps/plugin-updater";
import { errMsg, type UpdateInfo } from "../lib/ipc";

/** 把更新器原始错误转成可读中文（网络 / 代理 / 签名场景给可行动的提示） */
export function friendlyUpdateError(raw: string): string {
  const s = raw.toLowerCase();
  if (s.includes("signature")) return "更新包签名校验失败，请前往发布页手动下载安装";
  if (s.includes("timeout") || s.includes("timed out")) return "连接更新服务器超时，请检查网络或代理后重试";
  if (s.includes("send request") || s.includes("sending request") || s.includes("network") || s.includes("connect") || s.includes("dns")) {
    return "网络连接失败，请检查网络或代理后重试，或前往发布页手动下载";
  }
  return raw;
}

/** 官方 Tauri Updater：检查、强制验签下载并以 passive 模式安装；下载可取消。 */
export function useUpdate() {
  const updateRef = useRef<Update | null>(null);
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<{ received: number; total: number } | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");
  const [rawError, setRawError] = useState("");

  async function check() {
    setChecking(true);
    setError("");
    setRawError("");
    try {
      await updateRef.current?.close();
      const update = await checkForUpdate();
      updateRef.current = update;
      const result: UpdateInfo = update
        ? {
            hasUpdate: true,
            currentVersion: update.currentVersion,
            latestVersion: update.version,
            name: `YunX ${update.version}`,
            notes: update.body || "",
            downloadUrl: "",
            browserDownloadUrl: "https://github.com/94ANTIMe/YunX_DeskTop_Test/releases/latest",
          }
        : {
            hasUpdate: false,
            currentVersion: "",
            latestVersion: "",
            name: "",
            notes: "",
            downloadUrl: "",
            browserDownloadUrl: "",
          };
      setInfo(result);
      return result;
    } catch (cause) {
      const raw = errMsg(cause);
      setRawError(raw);
      setError(friendlyUpdateError(raw));
      return null;
    } finally {
      setChecking(false);
    }
  }

  async function apply() {
    const update = updateRef.current;
    if (!update) return false;
    setDownloading(true);
    setInstalling(false);
    setError("");
    setRawError("");
    let received = 0;
    let total = 0;
    setProgress({ received, total });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          setProgress({ received: 0, total });
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          setProgress({ received, total });
        } else if (event.event === "Finished") {
          setProgress({ received: total, total });
          setDownloading(false);
          setInstalling(true);
        }
      });
      return true;
    } catch (cause) {
      const raw = errMsg(cause);
      setRawError(raw);
      setError(friendlyUpdateError(raw));
      setInstalling(false);
      return false;
    } finally {
      setDownloading(false);
    }
  }

  /** 取消下载（中断后可再次点「立即更新」重新下载；已进入安装阶段则不可中断） */
  async function cancel() {
    if (!downloading) return;
    try {
      await updateRef.current?.close();
    } catch {
      // 引擎侧已终止，忽略关闭错误
    }
    setDownloading(false);
    setProgress(null);
  }

  return {
    info,
    checked: info !== null,
    checking,
    downloading,
    progress,
    installing,
    error,
    rawError,
    check,
    apply,
    cancel,
    clearError: () => {
      setError("");
      setRawError("");
    },
  };
}
