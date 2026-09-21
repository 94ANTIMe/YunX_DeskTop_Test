# UI 与主题扩展

> 主题 token 契约、新增主题 checklist、状态色、键盘可用性、减少动态效果。改 UI 前必读。

## 主题体系（明暗 × 配色 双轴）

- **明暗模式**：`跟随系统 / 浅色 / 深色`（settings.darkMode，0/1/2），由 `useTheme` 解析成 effective，切换 `html.dark` class。
- **配色主题**：七套，注册表在 `src/lib/themes.ts`（稳定 ID + 中文名 + 原始双色 colorA/colorB + 浅/深两组完整 token）。与明暗完全独立：跟随系统时仅明暗联动，配色始终保留。
- **生效机制**：`useTheme` 把语义 token（`--app-bg`、`--accent` 等 17 个）注入 `documentElement`；`src/styles/tokens.css` 的 `@theme` 把语义变量别名成 Tailwind 工具类（含旧名兼容：`--color-ivory → var(--app-bg)`、`--color-clay → var(--accent)` 等）。页面只用工具类，不感知主题。
- **持久化**：settings.json 是来源；localStorage 仅启动预览缓存；启动校准见 [interfaces-and-compat.md](interfaces-and-compat.md)。

## 语义 token 清单

| 语义 | 变量 | 工具类（新旧） |
| --- | --- | --- |
| 页面底 / 卡片 / 输入面 / 选中底 | `--app-bg` `--surface-card` `--surface-input` `--surface-muted` | `bg-ivory` `bg-carrier` `bg-carrier-deep` `bg-oat` |
| 主 / 次文字 | `--text-primary` `--text-secondary` | `text-ink` `text-ink-soft`（`ink/N` 同时承担弱边框） |
| 主强调 / hover | `--accent` `--accent-strong` | `bg-clay` `text-clay` / `bg-clay-deep` `text-clay-deep` |
| 低饱和强调底 / 装饰 | `--accent-soft` `--accent-decor` | `bg-accent-soft` / `bg-cactus` `text-cactus` |
| **强调色上文字** | `--on-accent` | `text-on-accent`（禁止在强调底上用 `text-white`） |
| 危险按钮文字 | `--on-danger` | `text-on-danger` |
| 焦点环 | `--focus-ring` | `:focus-visible` 全局生效 |
| 状态色（与强调色分离） | `--status-success` `--status-warning` `--status-error` | `text-success` `text-warning` `text-danger` 及 `bg-*/10` |

**状态色使用规则**：错误 / 警告 / 成功语义必须用状态色（`text-danger` 等），强调色家族（clay / cactus）只做品牌强调与装饰；状态提示同时保留图标或文字，不能只靠颜色区分（色盲可读）。

## 硬性规则

1. 业务组件禁止硬编码主题色 hex（`accent-[#xxx]` 之类）；滑杆等原生控件的强调色已在 tokens.css 全局处理。
2. 原始色值（colorA/colorB）只出现在主题定义与预览色块；正文 / 悬停 / 焦点颜色按对比度派生（正文 ≥4.5:1，控件 ≥3:1，测试在 `src/lib/themes.test.ts` 强制）。
3. 页面与卡片用低饱和派生底色；高饱和原色只做强调与识别，不整页铺色。
4. 现有插画（`src/assets/art/`）原样保留，不重新生成、不做滤镜染色。

## 新增一套主题 checklist（全部在 `src/lib/themes.ts`，零页面改动）

1. `COLOR_THEMES` 追加一条：唯一 `id`（kebab-case，持久化用，永不改名——要改名时保留旧 ID 作 alias）、中文 `name`、原始 `colorA`（主强调）/ `colorB`（装饰）。
2. `light` / `dark` 两组 17 项 token 全量填写；原色过亮无法承载控件时（亮黄 / 亮绿 / 青）在浅色下派生加深并注释说明，深色下提亮；亮色强调配深色 `on-accent`。
3. 跑 `pnpm test`（themes.test.ts 校验 ID 唯一、token 完整、对比度达标）。
4. 人工冒烟：新主题 × 明暗 × 主要界面（按钮 / 输入 / 禁用 / 错误条 / 图表 / 弹窗）。
5. 若主题色接近状态色（如粉红 vs 错误红），确认错误提示仍靠图标 / 文字可辨。

主题卡 UI（设置页）自动读取注册表渲染（双色样本、迷你界面、名称、选中标记、方向键导航），无需改组件。

## 键盘可用性与可访问性

- 所有可操作元素可 Tab 到达、Enter / 空格触发；radio / tablist 语义配方向键与漫游 tabindex（参照设置页主题卡）。
- 折叠 / 弹层：触发按钮带 `aria-expanded` + `aria-controls`；收起的内容用 `visibility:hidden`（`.collapse-grid`）或 `inert` 保证退出键盘焦点顺序与可访问性树。
- 焦点环走 `:focus-visible` 全局样式（颜色 `--focus-ring`）；不要 `outline-none` 而不给替代。

## 动效

- 交互动效约 200ms，克制的入场（page-in / rise / drawer）而非循环动效；抽屉来去同路（空间一致性）。
- 一律尊重 `prefers-reduced-motion: reduce`（tokens.css 统一降级）；新增带过渡的组件必须在 reduce 分支下表现为淡入或直接切换。

## UI 视觉验收清单（主题相关改动跑一遍）

- [ ] 七套主题 × 浅 / 深：主要按钮（含 hover / disabled）、输入框、选中态、进度条、统计图表
- [ ] 错误提示、成功提示、警告（黄 / 绿 / 青主题重点验证文字可读性）
- [ ] 弹窗 / 抽屉 / 导航胶囊 / 滚动条
- [ ] 键盘走查：Tab 顺序、方向键选主题、折叠面板展开 / 收起

## 弹层、Toast 与 z 层级（2026-09-22 B 线新增约定）

- **弹层一律用 `src/components/ui/`**：居中弹窗用 `ui/Modal`（z-[70]、遮罩 `bg-black/40 backdrop-blur animate-fade`、面板 `animate-rise rounded-card bg-carrier shadow-capsule`、初始焦点 + 关闭还焦 + body 滚动锁、Esc/点遮罩可配）；右侧抽屉用 `ui/Drawer`（同 z-[70]，内置 220ms leaving 机制）；确认类用 `ui/ConfirmDialog`（danger 时 alertdialog 语义）。禁止再手写 `fixed inset-0` 遮罩（LoginDialog 为 WebView 登录壳遗留例外）。
- **z 层级**：内容 < 导航 z-10 < 弹层 z-[70] < 全局 Toast z-[80]。新层级出现时先查本表。
- **即时反馈用 `lib/toast.ts` + `ToastHost`**（success/error/info；队列上限 4、自动消失 error 7s 其余 4s、同类去重；`aria-live="polite"` 在 ToastHost 内）。页面不再自建 notice 计时器横幅；表单引导类错误（需持续可见的上下文提示）可保留内联。
- **控件原语**：按钮 `ui/Button`（primary/outline/danger/soft × sm/md/lg；实底禁用 opacity-50、描边 40；hover 挂 enabled:）、开关 `ui/Toggle`（role=switch）、下拉 `ui/Select`（原生封装）、设置行滑杆 `ui/SliderRow`（拖动即时 onInput、松手/失焦/触屏结束 onCommit）、载入占位 `ui/Skeleton`（aria-hidden + motion-reduce 降级）、空态 `EmptyState`（image 或 icon 变体）。新增同类控件先扩展原语，不再手写类名。
- **破坏性操作（删除/清空/登出/取消任务）必须过 `ui/ConfirmDialog`**，文案说清后果与是否可逆。
- **全局快捷键**：Ctrl+1~7 按可见 Tab 顺序切页、Ctrl+V 粘贴分享链接直达解析；新快捷键须在此登记并避开输入焦点。
