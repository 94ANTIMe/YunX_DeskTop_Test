import { useSyncExternalStore } from "react";
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

interface UpdateState {
  info: UpdateInfo | null;
  checking: boolean;
  downloading: boolean;
  progress: { received: number; total: number } | null;
  installing: boolean;
  error: string;
  rawError: string;
}

/**
 * 模块级单例更新器状态：App 横幅与设置页更新卡片共享同一份 check / download / install，
 * 杜绝「横幅下载中、设置页仍可点立即更新 → 双实例并发下载」的矛盾。
 * 订阅侧用 useSyncExternalStore，两处 useUpdate() 拿到同一状态与稳定 action。
 */
let state: UpdateState = {
  info: null,
  checking: false,
  downloading: false,
  progress: null,
  installing: false,
  error: "",
  rawError: "",
};

let updateRef: Update | null = null;
const listeners = new Set<() => void>();

function setState(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  for (const l of listeners) l();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return state;
}

// Progress 事件按 chunk 推送，频率远超渲染所需：300ms 合帧，Started / Finished 即时刷新
let pendingProgress: UpdateState["progress"] = null;
let progressTimer: number | null = null;

function flushProgress() {
  if (progressTimer != null) {
    window.clearTimeout(progressTimer);
    progressTimer = null;
  }
  if (pendingProgress) {
    setState({ progress: pendingProgress });
    pendingProgress = null;
  }
}

function scheduleProgress(received: number, total: number) {
  pendingProgress = { received, total };
  if (progressTimer == null) {
    progressTimer = window.setTimeout(flushProgress, 300);
  }
}

async function check() {
  setState({ checking: true, error: "", rawError: "" });
  try {
    // 先置空引用再关闭旧对象：再次 check 失败时不会留下指向已 close 对象的死引用
    const previous = updateRef;
    updateRef = null;
    if (previous) {
      try {
        await previous.close();
      } catch {
        // 引擎侧已终止，忽略关闭错误
      }
    }
    const update = await checkForUpdate();
    updateRef = update;
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
    setState({ info: result });
    return result;
  } catch (cause) {
    const raw = errMsg(cause);
    setState({ rawError: raw, error: friendlyUpdateError(raw) });
    return null;
  } finally {
    setState({ checking: false });
  }
}

async function apply() {
  let update = updateRef;
  // 无可用 Update 对象（check 失败 / 页面重载后）但有新版信息：先重新检查再下载
  if (!update && state.info?.hasUpdate) {
    const result = await check();
    if (!result?.hasUpdate) return false;
    update = updateRef;
  }
  if (!update) return false;
  setState({ downloading: true, installing: false, error: "", rawError: "" });
  let received = 0;
  let total = 0;
  pendingProgress = { received, total };
  flushProgress();
  try {
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
        pendingProgress = { received: 0, total };
        flushProgress();
      } else if (event.event === "Progress") {
        received += event.data.chunkLength;
        scheduleProgress(received, total);
      } else if (event.event === "Finished") {
        pendingProgress = { received: total, total };
        flushProgress();
        setState({ downloading: false, installing: true });
      }
    });
    return true;
  } catch (cause) {
    const raw = errMsg(cause);
    setState({ rawError: raw, error: friendlyUpdateError(raw), installing: false });
    return false;
  } finally {
    flushProgress();
    setState({ downloading: false });
  }
}

/** 取消下载（中断后可再次点「立即更新」重新下载；已进入安装阶段则不可中断） */
async function cancel() {
  if (!state.downloading) return;
  try {
    await updateRef?.close();
  } catch {
    // 引擎侧已终止，忽略关闭错误
  }
  pendingProgress = null;
  setState({ downloading: false, progress: null });
}

function clearError() {
  setState({ error: "", rawError: "" });
}

/** 共享更新器 hook：任意处调用均返回同一全局状态（App 横幅与设置页卡片联动） */
export function useUpdate() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot);
  return {
    ...snapshot,
    checked: snapshot.info !== null,
    check,
    apply,
    cancel,
    clearError,
  };
}
