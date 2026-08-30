import { useState } from "react";
import {
  ArrowRight,
  Check,
  Cloud,
  Link2,
  Loader2,
  Search,
  ServerCog,
  SkipForward,
  Sparkles,
} from "lucide-react";
import { errMsg, ipc, type Settings } from "../lib/ipc";
import aboutHero from "../assets/art/about-lighthouse.jpg";

/** 公共演示站（PanSou 官方 demo，可能不稳定；仅供快速体验） */
const PUBLIC_PANSOU = "https://pansou.5201314.xyz";

interface OnboardingPageProps {
  settings: Settings;
  onDone: () => void;
}

/** 首启引导：欢迎 → 搜索服务配置（PanSou）→ 完成。
 *  引导过程只写 pansouBaseUrl / showSearchTab / onboarded，其余设置不改动。 */
export default function OnboardingPage({ settings, onDone }: OnboardingPageProps) {
  const [step, setStep] = useState<0 | 1 | 2>(0);
  const [baseUrl, setBaseUrl] = useState(settings.pansouBaseUrl);
  const [showSearch, setShowSearch] = useState(settings.showSearchTab);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; ms: number; error: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  /** 保存 PanSou 配置（可选跳过后仍保存当前值） */
  async function saveAndFinish() {
    setSaving(true);
    setError("");
    try {
      const trimmed = baseUrl.trim().replace(/\/+$/, "");
      await ipc.updateSettings({
        ...settings,
        pansouBaseUrl: trimmed,
        showSearchTab: showSearch,
        onboarded: true,
      });
      onDone();
    } catch (e) {
      setError(errMsg(e));
      setSaving(false);
    }
  }

  /** 测试 PanSou 连通性 */
  async function test() {
    const trimmed = baseUrl.trim().replace(/\/+$/, "");
    if (!trimmed) return;
    setTesting(true);
    setError("");
    setTestResult(null);
    try {
      const r = await ipc.pansouPing(trimmed);
      setTestResult({ ok: r.ok, ms: r.latencyMs, error: r.error });
    } catch (e) {
      setTestResult({ ok: false, ms: 0, error: errMsg(e) });
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="flex min-h-full flex-col items-center justify-center px-6 py-12">
      <div className="w-full max-w-lg">
        {/* 步骤指示 */}
        <div className="mb-8 flex items-center justify-center gap-2">
          {[0, 1, 2].map((i) => (
            <span
              key={i}
              className={`h-1.5 rounded-full transition-all ${
                i === step ? "w-8 bg-clay" : i < step ? "w-3 bg-clay/60" : "w-3 bg-ink/10"
              }`}
            />
          ))}
        </div>

        {/* 步骤 0：欢迎 */}
        {step === 0 && (
          <section className="animate-rise rounded-card bg-carrier p-8 text-center">
            <img
              src={aboutHero}
              alt="云析"
              draggable={false}
              className="mx-auto h-40 w-64 rounded-card object-cover"
            />
            <p className="mt-6 font-mono text-[10px] tracking-[0.3em] text-ink-soft">WELCOME</p>
            <h1 className="mt-2 font-display text-3xl font-semibold text-ink">云析 YunX</h1>
            <p className="mt-3 text-sm leading-relaxed text-ink-soft">
              网盘分享链接解析与高速下载工具。三步完成初始配置，随时可在「设置」中重新调整。
            </p>
            <ul className="mx-auto mt-6 w-fit space-y-2 text-left text-sm text-ink-soft">
              <li className="flex items-center gap-2">
                <Cloud size={15} className="text-clay" /> 登录网盘账号（夸克 / UC / 百度 / 139 / 迅雷 / 123）
              </li>
              <li className="flex items-center gap-2">
                <Link2 size={15} className="text-clay" /> 粘贴分享链接自动解析并高速下载
              </li>
              <li className="flex items-center gap-2">
                <ServerCog size={15} className="text-clay" /> 接入 PanSou 搜索全网公开分享资源
              </li>
            </ul>
            <button
              onClick={() => setStep(1)}
              className="mx-auto mt-8 flex items-center gap-1.5 rounded-ctrl bg-clay px-6 py-2 text-sm font-semibold text-white transition-colors hover:bg-clay-deep"
            >
              开始配置
              <ArrowRight size={14} />
            </button>
          </section>
        )}

        {/* 步骤 1：PanSou 搜索服务 */}
        {step === 1 && (
          <section className="animate-rise rounded-card bg-carrier p-8">
            <div className="flex items-center gap-2">
              <Search size={17} className="text-clay" strokeWidth={1.8} />
              <h2 className="font-display text-xl font-semibold text-ink">搜索服务（PanSou）</h2>
            </div>
            <p className="mt-2 text-sm leading-relaxed text-ink-soft">
              PanSou 是可自部署的网盘聚合搜索服务。填入服务根地址后可在「搜索」页搜全网公开分享资源；
              留空可跳过，之后在设置页随时补充。
            </p>
            <div className="mt-5 flex items-center gap-2">
              <input
                type="text"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.currentTarget.value)}
                placeholder="http://192.168.1.100:8888"
                spellCheck={false}
                className="h-10 min-w-0 flex-1 rounded-ctrl border border-ink/10 bg-carrier-deep px-3 font-mono text-sm text-ink placeholder:text-ink-soft/60 focus:border-clay focus:outline-none"
              />
              <button
                onClick={() => {
                  setBaseUrl(settings.pansouBaseUrl || "");
                }}
                className="shrink-0 rounded-ctrl border border-ink/15 px-3 py-2 text-xs text-ink-soft transition-colors hover:border-clay hover:text-clay-deep"
              >
                清除
              </button>
            </div>
            <button
              onClick={() => setBaseUrl(PUBLIC_PANSOU)}
              className="mt-2 rounded-ctrl bg-ink/5 px-2.5 py-1 text-[11px] text-ink-soft transition-colors hover:bg-ink/10 hover:text-ink"
            >
              使用公共演示站（{PUBLIC_PANSOU}，可能不稳定）
            </button>

            {/* 连通测试 */}
            {baseUrl.trim() && (
              <div className="mt-4 flex items-center gap-2">
                <button
                  onClick={test}
                  disabled={testing}
                  className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-3.5 py-1.5 text-xs font-medium text-ink transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
                >
                  {testing ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
                  测试连通性
                </button>
                {testResult && (
                  <span className={`truncate text-xs ${testResult.ok ? "text-cactus" : "text-clay-deep"}`}>
                    {testResult.ok
                      ? `服务正常 · ${testResult.ms}ms`
                      : `连接失败：${testResult.error}`}
                  </span>
                )}
              </div>
            )}

            {/* 搜索页入口开关 */}
            <div className="mt-6 border-t border-ink/10 pt-4">
              <label className="flex cursor-pointer items-center justify-between gap-4">
                <div>
                  <p className="text-sm text-ink-soft">在导航栏显示「搜索」页</p>
                  <p className="mt-0.5 text-xs text-ink-soft/70">关闭则隐藏搜索入口（服务地址保留）</p>
                </div>
                <button
                  role="switch"
                  aria-checked={showSearch}
                  onClick={() => setShowSearch(!showSearch)}
                  className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
                    showSearch ? "bg-clay" : "bg-ink/15"
                  }`}
                >
                  <span
                    className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
                      showSearch ? "left-[22px]" : "left-0.5"
                    }`}
                  />
                </button>
              </label>
            </div>

            {error && <p className="mt-3 text-xs text-clay-deep">{error}</p>}

            <div className="mt-8 flex items-center justify-between">
              <button
                onClick={() => setStep(0)}
                className="text-xs text-ink-soft transition-colors hover:text-ink"
              >
                上一步
              </button>
              <div className="flex items-center gap-2">
                <button
                  onClick={saveAndFinish}
                  disabled={saving}
                  className="flex items-center gap-1.5 rounded-ctrl border border-ink/15 px-4 py-2 text-xs font-medium text-ink-soft transition-colors hover:border-clay hover:text-clay-deep disabled:opacity-50"
                >
                  <SkipForward size={13} />
                  跳过
                </button>
                <button
                  onClick={saveAndFinish}
                  disabled={saving}
                  className="flex items-center gap-1.5 rounded-ctrl bg-clay px-6 py-2 text-sm font-semibold text-white transition-colors enabled:hover:bg-clay-deep disabled:opacity-50"
                >
                  {saving ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />}
                  完成
                </button>
              </div>
            </div>
          </section>
        )}

        {/* 步骤 2：完成（瞬间过渡） */}
        {step === 2 && (
          <section className="animate-rise rounded-card bg-carrier p-8 text-center">
            <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-cactus/20">
              <Check size={26} className="text-cactus" strokeWidth={2.2} />
            </div>
            <h2 className="mt-4 font-display text-2xl font-semibold text-ink">配置完成</h2>
            <p className="mt-2 text-sm text-ink-soft">开始使用云析吧，祝下载愉快。</p>
          </section>
        )}
      </div>
    </div>
  );
}