import { useEffect, useState } from "react";
import { ExternalLink, Loader2, LogIn, X } from "lucide-react";
import { errMsg, ipc, onLoginSuccess, type XunleiLoginStep } from "../lib/ipc";

interface LoginDialogProps {
  platform: string;
  onClose: () => void;
  onSuccess: () => void;
}

/** 登录方式：web = WebView 抓 Cookie；xunlei = 密码+短信；pan123 = 账号密码 JWT */
const WEB_LOGIN: Record<string, boolean> = {
  quark: true,
  uc: true,
  baidu: true,
  c139: true,
};

const PLATFORM_NAMES: Record<string, string> = {
  quark: "夸克网盘",
  uc: "UC 网盘",
  baidu: "百度网盘",
  c139: "139 网盘",
  xunlei: "迅雷网盘",
  pan123: "123 云盘",
};

/** 登录对话框：WebView 平台打开独立窗口；迅雷/123 表单登录 */
export default function LoginDialog({ platform, onClose, onSuccess }: LoginDialogProps) {
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  // 迅雷表单
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [smsStep, setSmsStep] = useState<XunleiLoginStep | null>(null);
  const [smsCode, setSmsCode] = useState("");
  const [busy, setBusy] = useState(false);

  const isWeb = WEB_LOGIN[platform] ?? false;
  const isXunlei = platform === "xunlei";
  const isPan123 = platform === "pan123";

  // WebView 登录：打开窗口等待成功事件
  useEffect(() => {
    if (!isWeb) return;
    let active = true;
    (async () => {
      try {
        await ipc.webLoginStart(platform);
        setWaiting(true);
      } catch (e) {
        setError(errMsg(e));
      }
    })();
    const un = onLoginSuccess((e) => {
      if (e.platform === platform && active) {
        setNotice(`登录成功：${e.nickname}`);
        setTimeout(onSuccess, 600);
      }
    });
    return () => {
      active = false;
      un.then((f) => f());
    };
  }, [platform]); // eslint-disable-line react-hooks/exhaustive-deps

  // 取消（关窗）
  async function cancel() {
    if (isWeb) {
      await ipc.webLoginCancel(platform).catch(() => {});
    }
    onClose();
  }

  // 迅雷密码登录
  async function xunleiSubmit() {
    if (!username.trim() || !password || busy) return;
    setBusy(true);
    setError("");
    try {
      const step = await ipc.xunleiLogin(username.trim(), password);
      if (step.needSms) {
        setSmsStep(step);
        setNotice(step.message || "已发送短信验证码");
      } else {
        setNotice("登录成功");
        setTimeout(onSuccess, 500);
      }
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  }

  // 迅雷短信登录
  async function xunleiSmsSubmit() {
    if (!smsCode.trim() || !smsStep || busy) return;
    setBusy(true);
    setError("");
    try {
      const step = await ipc.xunleiSmsLogin(
        username.trim(),
        smsCode.trim(),
        smsStep.creditKey,
        smsStep.smsToken,
      );
      if (!step.needSms && !step.sessionId) {
        setNotice("登录成功");
        setTimeout(onSuccess, 500);
      } else if (step.needSms) {
        setSmsStep(step);
        setNotice(step.message || "验证码错误，请重试");
      }
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  }

  // 123 登录
  async function pan123Submit() {
    if (!username.trim() || !password || busy) return;
    setBusy(true);
    setError("");
    try {
      const nickname = await ipc.pan123Login(username.trim(), password);
      setNotice(`登录成功：${nickname}`);
      setTimeout(onSuccess, 500);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/20 p-8 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md animate-rise rounded-card bg-carrier p-6 shadow-capsule"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h3 className="font-display text-xl font-semibold text-ink">
            登录{PLATFORM_NAMES[platform] ?? platform}
          </h3>
          <button
            onClick={cancel}
            className="rounded-ctrl p-1.5 text-ink-soft transition-colors hover:bg-carrier-deep hover:text-ink"
          >
            <X size={16} />
          </button>
        </div>

        {/* WebView 登录说明 */}
        {isWeb && (
          <div className="mt-6">
            {waiting && !notice ? (
              <div className="flex items-center gap-3 rounded-ctrl bg-carrier-deep px-4 py-5">
                <Loader2 size={18} className="animate-spin text-clay" />
                <div>
                  <p className="text-sm font-medium text-ink">已打开登录窗口</p>
                  <p className="mt-1 text-xs text-ink-soft">
                    在新窗口完成登录后，将自动保存登录状态（可读取 HttpOnly Cookie）。
                  </p>
                </div>
              </div>
            ) : notice ? (
              <div className="rounded-ctrl bg-cactus/25 px-4 py-3 text-sm text-ink">{notice}</div>
            ) : (
              <p className="text-sm text-ink-soft">正在打开登录窗口…</p>
            )}
            {error && (
              <p className="mt-3 rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</p>
            )}
            <button
              onClick={cancel}
              className="mt-6 w-full rounded-ctrl border border-ink/15 py-2 text-sm font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
            >
              取消登录
            </button>
          </div>
        )}

        {/* 迅雷表单（密码 → 可能短信） */}
        {isXunlei && (
          <div className="mt-6 space-y-4">
            {!smsStep ? (
              <>
                <input
                  value={username}
                  onChange={(e) => setUsername(e.currentTarget.value)}
                  placeholder="手机号 / 账号"
                  className="w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-2.5 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                />
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.currentTarget.value)}
                  placeholder="密码"
                  className="w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-2.5 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                />
                <button
                  onClick={xunleiSubmit}
                  disabled={busy || !username.trim() || !password}
                  className="flex w-full items-center justify-center gap-2 rounded-ctrl bg-clay py-2.5 text-sm font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
                >
                  {busy ? <Loader2 size={15} className="animate-spin" /> : <LogIn size={15} />}
                  登录
                </button>
              </>
            ) : (
              <>
                <div className="rounded-ctrl bg-carrier-deep px-4 py-3 text-xs text-ink-soft">
                  <div className="flex items-center gap-1.5">
                    <ExternalLink size={13} className="text-clay" />
                    {smsStep.message || "已发送短信验证码"}
                  </div>
                  <p className="mt-1">账号 {username} 需要短信验证，请输入收到的验证码。</p>
                </div>
                <input
                  value={smsCode}
                  onChange={(e) => setSmsCode(e.currentTarget.value)}
                  placeholder="短信验证码"
                  className="w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-2.5 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
                />
                <button
                  onClick={xunleiSmsSubmit}
                  disabled={busy || !smsCode.trim()}
                  className="flex w-full items-center justify-center gap-2 rounded-ctrl bg-clay py-2.5 text-sm font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
                >
                  {busy ? <Loader2 size={15} className="animate-spin" /> : <LogIn size={15} />}
                  验证并登录
                </button>
                <button
                  onClick={() => setSmsStep(null)}
                  className="w-full rounded-ctrl border border-ink/15 py-2 text-xs text-ink-soft hover:border-clay hover:text-clay-deep"
                >
                  返回重新输入密码
                </button>
              </>
            )}
            {error && (
              <p className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</p>
            )}
            {notice && !error && (
              <p className="rounded-ctrl bg-cactus/25 px-4 py-2.5 text-sm text-ink">{notice}</p>
            )}
          </div>
        )}

        {/* 123 表单 */}
        {isPan123 && (
          <div className="mt-6 space-y-4">
            <input
              value={username}
              onChange={(e) => setUsername(e.currentTarget.value)}
              placeholder="账号 / 邮箱 / 手机号"
              className="w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-2.5 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
            />
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              placeholder="密码"
              className="w-full rounded-ctrl border border-ink/10 bg-carrier-deep px-4 py-2.5 text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
            />
            <button
              onClick={pan123Submit}
              disabled={busy || !username.trim() || !password}
              className="flex w-full items-center justify-center gap-2 rounded-ctrl bg-clay py-2.5 text-sm font-semibold text-white transition-colors hover:bg-clay-deep disabled:opacity-50"
            >
              {busy ? <Loader2 size={15} className="animate-spin" /> : <LogIn size={15} />}
              登录
            </button>
            {error && (
              <p className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</p>
            )}
            {notice && !error && (
              <p className="rounded-ctrl bg-cactus/25 px-4 py-2.5 text-sm text-ink">{notice}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
