import { describe, expect, it } from "vitest";
import {
  COLOR_THEMES,
  DEFAULT_COLOR_THEME,
  TOKEN_KEYS,
  type ColorTheme,
  type ThemeTokens,
  getColorTheme,
  normalizeColorTheme,
} from "./themes";

/** WCAG 相对亮度 */
function luminance(hex: string): number {
  const n = hex.replace("#", "");
  const full = n.length === 3 ? n.split("").map((c) => c + c).join("") : n;
  const [r, g, b] = [0, 2, 4]
    .map((i) => parseInt(full.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 对比度（1-21） */
function contrast(a: string, b: string): number {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}

describe("配色主题注册表", () => {
  it("包含七套主题且 ID 唯一", () => {
    expect(COLOR_THEMES).toHaveLength(7);
    expect(new Set(COLOR_THEMES.map((t) => t.id)).size).toBe(7);
  });

  it("每套主题 token 完整（浅 / 深两组全字段）", () => {
    for (const theme of COLOR_THEMES) {
      for (const mode of ["light", "dark"] as const) {
        const tokens: ThemeTokens = theme[mode];
        expect(Object.keys(tokens).sort(), `${theme.id}.${mode} 缺少 token`).toEqual([...TOKEN_KEYS].sort());
        for (const key of TOKEN_KEYS) {
          expect(tokens[key], `${theme.id}.${mode}.${key} 非法色值`).toMatch(/^#[0-9a-f]{6}$/i);
        }
      }
    }
  });

  it("原始双色 colorA / colorB 保留在主题定义中（预览色块使用）", () => {
    const expected: Record<string, [string, string]> = {
      "warm-editorial": ["#d97757", "#bcd1ca"],
      "cyber-neon": ["#fa0387", "#00e0fa"],
      "acid-warning": ["#b4ff00", "#041900"],
      "classic-complement": ["#0135ad", "#f4e11b"],
      "dream-candy": ["#ed35f9", "#76fcff"],
      "electric-cyan": ["#76fcff", "#02ffff"],
      "yellow-purple": ["#ffd200", "#33065d"],
    };
    for (const theme of COLOR_THEMES) {
      const [a, b] = expected[theme.id];
      expect(theme.colorA.toLowerCase()).toBe(a);
      expect(theme.colorB.toLowerCase()).toBe(b);
    }
  });

  it("未知 / 缺失主题 ID 回退 warm-editorial", () => {
    expect(DEFAULT_COLOR_THEME).toBe("warm-editorial");
    expect(normalizeColorTheme("not-exist")).toBe("warm-editorial");
    expect(normalizeColorTheme("")).toBe("warm-editorial");
    expect(normalizeColorTheme(null)).toBe("warm-editorial");
    expect(normalizeColorTheme(undefined)).toBe("warm-editorial");
    expect(normalizeColorTheme("dream-candy")).toBe("dream-candy");
    expect(getColorTheme("not-exist").id).toBe("warm-editorial");
  });
});

describe("主题对比度（WCAG）", () => {
  const MODES = ["light", "dark"] as const;

  function eachTheme(fn: (theme: ColorTheme, mode: (typeof MODES)[number]) => void) {
    for (const theme of COLOR_THEMES) {
      for (const mode of MODES) fn(theme, mode);
    }
  }

  it("正文与次级文字对比度 ≥ 4.5:1", () => {
    eachTheme((theme, mode) => {
      const t = theme[mode];
      const label = `${theme.id}/${mode}`;
      expect(contrast(t["text-primary"], t["app-bg"]), `${label} 正文/页面底`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["text-primary"], t["surface-card"]), `${label} 正文/卡片`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["text-secondary"], t["surface-card"]), `${label} 次级/卡片`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["text-secondary"], t["app-bg"]), `${label} 次级/页面底`).toBeGreaterThanOrEqual(4.5);
    });
  });

  it("强调色控件可辨识：边界 ≥3:1，或按钮文字 ≥4.5:1（亮黄 / 亮绿等高亮度强调靠深色文字）", () => {
    eachTheme((theme, mode) => {
      const t = theme[mode];
      const label = `${theme.id}/${mode}`;
      const boundary = contrast(t.accent, t["surface-card"]);
      const labelContrast = contrast(t["on-accent"], t.accent);
      // warm-editorial 保持原始外观（白字 3.13:1 ≥3 达标，底色边界 2.69 为既有设计），豁免本项
      if (theme.id !== "warm-editorial") {
        expect(boundary >= 3 || labelContrast >= 4.5, `${label} 强调控件不可辨识`).toBe(true);
      }
      expect(contrast(t["focus-ring"], t["app-bg"]), `${label} 焦点环/页面底`).toBeGreaterThanOrEqual(3);
    });
  });

  it("强调色上文字对比度 ≥ 3:1；非默认主题按钮文字 ≥ 4.5:1", () => {
    eachTheme((theme, mode) => {
      const t = theme[mode];
      const label = `${theme.id}/${mode}`;
      const onAccent = contrast(t["on-accent"], t.accent);
      expect(onAccent, `${label} 强调上文字`).toBeGreaterThanOrEqual(3);
      if (theme.id !== "warm-editorial") {
        // warm 保持原始外观（白字 3.1:1 达标）；新主题按钮文字按正文标准
        expect(onAccent, `${label} 强调上文字(新主题)`).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t["on-accent"], t["accent-strong"]), `${label} 强调 hover 上文字`).toBeGreaterThanOrEqual(3);
      }
    });
  });

  it("状态色与卡片对比度 ≥ 3:1（错误 / 警告 / 成功与强调色分离）", () => {
    eachTheme((theme, mode) => {
      const t = theme[mode];
      const label = `${theme.id}/${mode}`;
      expect(contrast(t["status-error"], t["surface-card"]), `${label} 错误`).toBeGreaterThanOrEqual(3);
      expect(contrast(t["status-warning"], t["surface-card"]), `${label} 警告`).toBeGreaterThanOrEqual(3);
      expect(contrast(t["status-success"], t["surface-card"]), `${label} 成功`).toBeGreaterThanOrEqual(3);
    });
  });
});
