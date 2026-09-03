import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, ChevronDown, ExternalLink, FolderOpen, LogIn, LogOut } from "lucide-react";
import PageHeader from "../components/PageHeader";
import LoginDialog from "../components/LoginDialog";
import PanFileManager from "../components/PanFileManager";
import { errMsg, ipc, onLoginSuccess, type AccountRow, type AccountSummary } from "../lib/ipc";
import type { TabId } from "../lib/tabs";
import driveHero from "../assets/art/drive-hero.jpg";

interface DrivePlatform {
  id: string;
  name: string;
  latin: string;
  note?: string;
  /** 平台个人主页（默认浏览器打开） */
  home: string;
}

interface DrivePageProps {
  onNavigate?: (tab: TabId) => void;
  onGoResolve?: (url: string, pwd?: string) => void;
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

/** 网盘页：账号登录/登出 + 个人网盘文件浏览与一键直链直取 */
export default function DrivePage({ onNavigate, onGoResolve }: DrivePageProps = {}) {
  const [accounts, setAccounts] = useState<AccountSummary[]>([]);
  // 各平台账号行缓存（拉开下拉时按需加载）
  const [rows, setRows] = useState<Record<string, AccountRow[]>>({});
  // 当前展开的账号下拉（platform | null）
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const [loginPlatform, setLoginPlatform] = useState<string | null>(null);
  const [activeFileManager, setActiveFileManager] = useState<{ platform: string; nickname: string } | null>(null);
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
      setRows({});
      refresh();
    }).then((f) => {
      unlisten = f;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  async function logout(platform: string, key?: string) {
    setError("");
    try {
      await ipc.logout(platform, key);
      setRows((r) => ({ ...r, [platform]: (r[platform] ?? []).filter((x) => x.key !== key) }));
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  /** 拉取平台账号行（下拉展开时加载） */
  async function toggleMenu(platform: string) {
    if (openMenu === platform) {
      setOpenMenu(null);
      return;
    }
    setError("");
    try {
      const list = await ipc.listAccountRows(platform);
      setRows((r) => ({ ...r, [platform]: list }));
      setOpenMenu(platform);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  /** 切换到指定账号行 */
  async function switchTo(platform: string, key: string) {
    setError("");
    try {
      await ipc.switchAccount(platform, key);
      setOpenMenu(null);
      setRows((r) => ({
        ...r,
        [platform]: (r[platform] ?? []).map((x) => ({ ...x, active: x.key === key })),
      }));
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  const summary = (id: string) => accounts.find((a) => a.platform === id);

  if (activeFileManager) {
    return (
      <PanFileManager
        platform={activeFileManager.platform}
        nickname={activeFileManager.nickname}
        onBack={() => setActiveFileManager(null)}
        onNavigate={(t) => onNavigate?.(t)}
        onGoResolve={onGoResolve}
      />
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader tab="drive" subtitle="登录网盘账号后即可解析对应平台分享并高速下载" />

      {error && (
        <div className="rounded-ctrl bg-clay/10 px-4 py-2.5 text-sm text-clay-deep">{error}</div>
      )}

      {/* hero 插画带 */}
      <section className="flex animate-rise items-center justify-between gap-8 rounded-card bg-carrier p-6" style={{ animationDelay: "60ms" }}>
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
              className={`animate-rise relative rounded-card bg-carrier p-5 ${openMenu === p.id ? "z-30" : ""}`}
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
                {acc?.loggedIn ? (
                  <div className="relative min-w-0 flex-1">
                    {/* 当前账号 + 展开下拉 */}
                    <button
                      onClick={() => toggleMenu(p.id)}
                      className="flex w-full items-center gap-1.5 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 py-1.5 text-xs text-ink transition-colors hover:border-clay"
                      title="切换账号"
                    >
                      <span className="min-w-0 flex-1 truncate text-left">
                        {acc.nickname || "已登录"}
                      </span>
                      <ChevronDown
                        size={13}
                        className={`shrink-0 text-ink-soft transition-transform ${openMenu === p.id ? "rotate-180" : ""}`}
                      />
                    </button>
                    {openMenu === p.id && (
                      <div className="absolute right-0 top-full z-20 mt-1 w-64 overflow-hidden rounded-ctrl border border-ink/10 bg-carrier shadow-lg">
                        <p className="border-b border-ink/10 px-3 py-2 text-[10px] uppercase tracking-wider text-ink-soft">
                          账号列表 · 点击切换
                        </p>
                        {(rows[p.id] ?? []).length === 0 && (
                          <p className="px-3 py-2.5 text-xs text-ink-soft/70">暂无其他账号</p>
                        )}
                        {rows[p.id]?.map((row) => (
                          <button
                            key={row.key}
                            onClick={() => switchTo(p.id, row.key)}
                            className={`flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-ivory ${
                              row.active ? "bg-ivory text-ink" : "text-ink-soft"
                            }`}
                          >
                            <span className="min-w-0 flex-1 truncate">{row.nickname || "已登录"}</span>
                            {row.active && <Check size={13} className="shrink-0 text-clay" />}
                            {!row.active && (
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  logout(p.id, row.key);
                                }}
                                className="shrink-0 rounded px-1.5 py-0.5 text-[10px] text-ink-soft/70 hover:bg-clay/10 hover:text-clay-deep"
                                title="退出该账号"
                              >
                                退
                              </button>
                            )}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                ) : (
                  <span className="min-w-0 flex-1 truncate text-xs text-ink-soft">未登录</span>
                )}
                <button
                  onClick={() => openUrl(p.home).catch(() => {})}
                  className="flex shrink-0 items-center gap-1 rounded-ctrl border border-ink/15 px-3 py-1.5 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
                  title="使用电脑默认浏览器打开个人信息页"
                >
                  <ExternalLink size={12} />
                  主页
                </button>
                {acc?.loggedIn && ["baidu", "quark", "pan123"].includes(p.id) && (
                  <button
                    onClick={() => setActiveFileManager({ platform: p.id, nickname: acc.nickname || "" })}
                    className="flex shrink-0 items-center gap-1 rounded-ctrl bg-clay/10 text-clay-deep border border-clay/30 px-3 py-1.5 text-xs font-semibold hover:bg-clay hover:text-white transition-colors"
                    title="浏览该网盘个人文件并一键直链下载"
                  >
                    <FolderOpen size={12} />
                    浏览文件
                  </button>
                )}
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
