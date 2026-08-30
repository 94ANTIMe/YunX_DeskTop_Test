import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, LogIn, LogOut } from "lucide-react";
import PageHeader from "../components/PageHeader";
import LoginDialog from "../components/LoginDialog";
import { errMsg, ipc, onLoginSuccess, type AccountSummary } from "../lib/ipc";
import driveHero from "../assets/art/drive-hero.jpg";

interface DrivePlatform {
  id: string;
  name: string;
  latin: string;
  note?: string;
  /** 平台个人主页（默认浏览器打开） */
  home: string;
}

/** 6 平台卡片（对齐 Android 网盘页；登录态真实读取） */
const PLATFORMS: DrivePlatform[] = [
  { id: "quark", name: "夸克网盘", latin: "QUARK", home: "https://pan.quark.cn/" },
  { id: "uc", name: "UC 网盘", latin: "UC", home: "https://drive.uc.cn/" },
  { id: "xunlei", name: "迅雷网盘", latin: "XUNLEI", home: "https://pan.xunlei.com/" },
  { id: "baidu", name: "百度网盘", latin: "BAIDU", note: "不建议使用，可能导致账号风控", home: "https://pan.baidu.com/" },
  { id: "c139", name: "139 网盘", latin: "139", home: "https://yun.139.com/" },
  { id: "pan123", name: "123 云盘", latin: "123PAN", home: "https://www.123pan.com/" },
];

/** 网盘页：账号登录/登出（登录方式：夸克/UC/百度/139 WebView；迅雷密码+短信；123 账密 JWT） */
export default function DrivePage() {
  const [accounts, setAccounts] = useState<AccountSummary[]>([]);
  const [loginPlatform, setLoginPlatform] = useState<string | null>(null);
  const [error, setError] = useState("");

  async function refresh() {
    try {
      setAccounts(await ipc.listAccounts());
    } catch (e) {
      setError(errMsg(e));
    }
  }

  useEffect(() => {
    refresh();
    let unlisten: (() => void) | null = null;
    onLoginSuccess(() => {
      setLoginPlatform(null);
      refresh();
    }).then((f) => {
      unlisten = f;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  async function logout(platform: string) {
    setError("");
    try {
      await ipc.logout(platform);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  const summary = (id: string) => accounts.find((a) => a.platform === id);

  return (
    <div className="space-y-6">
      <PageHeader tab="drive" subtitle="登录网盘账号后即可解析对应平台分享并高速下载" />

      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}

      {/* hero 插画带 */}
      <section className="flex animate-rise items-center gap-8 rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
        <div>
          <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">SIX DRIVES</p>
          <h3 className="mt-1.5 font-display text-2xl font-semibold text-ink">网盘账号中心</h3>
        </div>
        <img
          src={driveHero}
          alt=""
          draggable={false}
          className="hidden h-36 w-56 shrink-0 rounded-card object-cover sm:block"
        />
      </section>

      <div className="grid grid-cols-2 gap-4 xl:grid-cols-3">
        {PLATFORMS.map((p, i) => {
          const acc = summary(p.id);
          return (
            <section
              key={p.id}
              className="animate-rise rounded-card bg-carrier p-5"
              style={{ animationDelay: `${120 + i * 50}ms` }}
            >
              <div className="flex items-start justify-between">
                <div>
                  <p className="font-mono text-[10px] tracking-[0.25em] text-ink-soft">{p.latin}</p>
                  <h3 className="mt-1.5 font-display text-xl font-semibold text-ink">{p.name}</h3>
                </div>
                {/* 登录态：实心点 */}
                <span
                  className={`mt-1 h-2 w-2 rounded-full ${
                    acc?.loggedIn ? "bg-cactus" : "border border-ink-soft/40"
                  }`}
                />
              </div>
              {p.note && <p className="mt-2 text-[11px] text-clay-deep">{p.note}</p>}
              <div className="mt-4 flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-xs text-ink-soft">
                  {acc?.loggedIn ? (acc.nickname || "已登录") : "未登录"}
                </span>
                <button
                  onClick={() => openUrl(p.home).catch(() => {})}
                  className="flex shrink-0 items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
                  title="使用电脑默认浏览器打开个人信息页"
                >
                  <ExternalLink size={12} />
                  个人信息页
                </button>
                {acc?.loggedIn ? (
                  <button
                    onClick={() => logout(p.id)}
                    className="flex shrink-0 items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
                  >
                    <LogOut size={13} />
                    登出
                  </button>
                ) : (
                  <button
                    onClick={() => setLoginPlatform(p.id)}
                    className="flex shrink-0 items-center gap-1.5 rounded-ctrl bg-clay px-3.5 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-clay-deep"
                  >
                    <LogIn size={13} />
                    登录
                  </button>
                )}
              </div>
            </section>
          );
        })}
      </div>

      {/* 登录对话框 */}
      {loginPlatform && (
        <LoginDialog
          platform={loginPlatform}
          onClose={() => setLoginPlatform(null)}
          onSuccess={() => setLoginPlatform(null)}
        />
      )}
    </div>
  );
}
