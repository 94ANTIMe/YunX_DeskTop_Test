# 测试与交付

> 按风险选择测试强度；记录命令与结果；明确区分自动测试、人工冒烟、未验证项。

## 风险分级与最低测试要求

| 风险 | 典型改动 | 最低要求 |
| --- | --- | --- |
| 高 | IPC 字段 / settings / 数据库 / 下载引擎 / 更新器 | 新增或修改的单测（前端 + Rust 对应侧）+ 全量命令通过 + 人工冒烟（含旧数据） |
| 中 | 新页面 / 新交互 / 主题 | 组件或 hook 单测 + 全量命令通过 + 主题视觉清单（[ui-and-themes.md](ui-and-themes.md)） |
| 低 | 文案 / 注释 / 文档 | 全量命令通过；无需新测试 |

## 现有测试资产（改到对应区域时先看再改）

- `src/lib/themes.test.ts` —— 七套主题：ID 唯一、token 完整、原始双色保留、未知回退、**WCAG 对比度强制**（正文 ≥4.5、按钮文字新主题 ≥4.5 / warm 保持原貌 ≥3、状态色 ≥3、焦点环 ≥3）。
- `src/hooks/useAppearance.test.ts` —— 外观保存队列：顶部切换不丢配色、快速切换串行、失败回读回滚、未知主题规范化。
- `src/hooks/useTheme.test.tsx` —— 明暗 × 配色：启动缓存、未知缓存回退、hydrate 校准、系统联动仅跟随系统、CSS 变量注入。
- `src/pages/SettingsPage.test.tsx` —— 设置保存单飞合并、主题卡 radio 语义与键盘、致谢折叠（aria-expanded / inert / 收起退出可访问性树）。
- `src/components/LoginDialog.test.tsx`、`src/pages/ResolvePage.test.tsx` —— 登录与解析回归。
- `src-tauri/src/commands/settings.rs` tests —— 仅外观变更跳过 aria2、旧 JSON 缺 `colorTheme` 回退。

## 命令与结果记录

```bash
pnpm test                                   # 前端 vitest（根目录）
pnpm build                                  # tsc + vite build
cd src-tauri && cargo test && cargo check   # Rust
git diff --check                            # 空白错误
```

交付说明必须包含实际运行的命令与结果（通过数 / 失败数原文），例如：

```text
自动测试：pnpm test（31 passed）、cargo test（12 passed）、pnpm build 成功
人工冒烟：7 主题 × 明暗主界面走查通过
未验证项：多显示器缩放
```

禁止把「未验证」写成「已验证」；跑不了的命令明说原因。

## 自动测试 / 人工冒烟 / 未验证的边界

- **自动测试**：可重复、断言明确的逻辑（契约、状态机、对比度、队列行为）。UI 视觉正确性、系统级行为（托盘、自启、更新安装）不在此列。
- **人工冒烟**：真实运行 `pnpm tauri dev` 或安装包走一遍主流程；主题相关改动必须按视觉清单过一遍明暗两态。
- **未验证项**：明说没跑什么。宁可留一句「未验证多显示器」，不留一句「应该没问题」。
