# 云析桌面版 · 前端界面整体升级计划（B 线执行蓝本）

> 来源：用户 2026-09-22 会话原文，作为三线总任务中 B 线（前端界面整体升级）的权威执行蓝本落盘。
> 执行顺序：B1 → B2 → B3 → B4，每阶段 = 实现 + 验证 + 刷新断点（[Handed-docs.md](../Handed-docs.md)）+ 独立 commit。
> 与 C 线（解耦模块化）的分工：useDownloads store、ui/ 组件层归本线；STATUS_TEXT 单一来源、平台注册表、ipc 分域、ResolvePage 区块拆分归 C 线。

## 目标与边界

把 UI 从「骨相成熟、皮相靠复制粘贴」升级为「有基础组件层、全局感知层、交互补齐」。四个阶段串行，每阶段含验证 + 刷新断点 + 独立 commit。

**硬边界**：纯前端改造——IPC 契约零变化、Rust 零改动、零新依赖、不升版本号、不 push。颜色全部走现有 token，新组件遵守交互四件套（focus-visible / aria / 200ms 过渡尊重 reduced-motion / 竞态处理）。已确认砍掉：日志页虚拟化（改 React.memo 轻量优化）。

## 阶段 1：全局感知层（体感见效最快）

**1.1 下载状态单例 store** `src/hooks/useDownloads.ts`：照抄 `useUpdate.ts` 的模块级单例 + `useSyncExternalStore` 模式。收编 DownloadPage 现有的 `onDownloadsUpdated` 订阅与 `mergeTasks` 去重合并（`DownloadPage.tsx:62-84`），派生：活跃任务数、总速度（status===1 求和，与 DownloadSummary 同口径，`formatSpeed` 空串兜底）、status 迁移检测（→完成/失败回调）。Rust poll_loop 已做变更检测后广播，多订阅者零增量 IPC。

**1.2 TopCapsule 下载指示器**：下载 pill 加任务数角标 + 速度小字；角标为非聚焦 span，`aria-label` 补充「N 个任务进行中」。App 订阅 store 后传 props（`App.tsx:182-188`），点击即切下载页。

**1.3 全局 Toast**：`src/lib/toast.ts` 单例（success/error/info，队列上限 4，自动消失、同类去重）+ `src/components/ToastHost.tsx`（fixed 右下，ClipboardPrompt 同位但 z-[80] 高于弹层；`animate-rise`；全库首个 `aria-live="polite"` 容器）。下载完成/失败由 store 迁移检测自动弹 toast（带文件名，失败带原因）。

**1.4 页面反馈迁移**：ResolvePage / SearchPage / SettingsPage / PanFileManager 的 notice/error 内联横幅与各自 flashNotice 计时器 → `toast.success` / `toast.error`。保留：LoginDialog 弹层内反馈、文件夹收集进度条（持续态）。

验证：`pnpm test` + `pnpm build`；冒烟：下载进行中切到设置页看角标与速度、完成时看 toast。**刷新断点。**

## 阶段 2：基础组件层 `src/components/ui/`（工程量最大）

**2.1 纯新增原语 + 单测**（先建组件不动页面）：
- `Button`：variant primary / outline / danger / soft，size sm/md/lg，统一 disabled 档位（实底 opacity-50、描边 40）与 `enabled:hover` 防残留；
- `IconButton`、`Toggle`（md h-6 w-11 / sm h-5 w-9，`role="switch"`）、`Select`（原生封装，`{value,label}[]`）、`Skeleton`（animate-pulse，token 化 + reduced-motion 降级）、`SliderRow`（label/codeLabel/min/max/step/unit/value/onInput/onCommit，保留现有「拖动即时、松手/失焦提交」手势）；
- `EmptyState` 扩展 icon 变体（现有 image 变体保留）。

**2.2 Modal + Drawer 统一**：统一 z-[70]、遮罩 `bg-black/40 backdrop-blur animate-fade`、面板 `rounded-card bg-carrier shadow-capsule animate-rise`、Esc + 点遮罩可配、`role="dialog"/"alertdialog"` + `aria-modal`、初始焦点与关闭还焦、body 滚动锁；Drawer 收编 TaskDetailDrawer / BatchQueuePanel 已同构的 leaving/220ms 机制；ConfirmDialog 改基于 Modal。

**2.3 页面迁移**（按页小步，每页迁完即验）：8 个弹层 → Modal/Drawer；再逐页替换手写按钮/Toggle/select 类名。迁移原则：只换 className 不改逻辑，次要视觉变体向标准形收敛。

验证：每页 `pnpm test` + `pnpm build`；全部完成后按 ui-and-themes.md 视觉验收清单跑 7 主题 × 明暗抽查。**刷新断点。**

## 阶段 3：交互升级

- **3.1 解析结果多选批量下载**：`selectedFids: Set<string>` + 行首 checkbox；列表底操作条（已选 N 项 · 总大小 · 下载选中 · 全选/清除）；复用 `lib/download.ts` 的 `fetchAndEnqueue`/`collectFolder`，batchBusy 独立守卫；放宽现互斥——单文件按钮只在本行下载时禁用。
- **3.2 破坏性操作确认 ×8**：删收藏 / 删单条与清空历史 / 清空日志 / 删订阅 / 登出（两入口）/ 取消下载任务，复用 ConfirmDialog，文案按调研表。
- **3.3 全局快捷键**（App.tsx）：Ctrl+1~7 切页（按 hiddenTabs 过滤后取位）；Ctrl+V 全局解析（paste 事件 → 目标非 input/textarea → `looksLikeLink` 过滤 → 现成 `goResolve`，与剪贴板提示天然去重）。
- **3.4 DrivePage 账号下拉**：click-outside（mousedown + data 属性标记）+ Esc 关闭。
- **3.5 StatsPage 柱状图键盘可达**：柱加 `tabIndex={0}` + `group-focus-visible:block` + `role="img"` aria-label（PlatformBars 本就常显文本，不动）。
- **3.6 Onboarding 三步流程修复**：`saveAndFinish`/跳过 → `setStep(2)`，step 2 加「进入云析」→ `onDone`（死代码复活）。
- **3.7 骨架屏 ×4**：DrivePage 账号卡（修「闪未登录」最误导处）、DownloadPage 初始列表（修「闪空态」）、StatsPage KPI+图表、LogsPage 列表。
- **3.8 LogsPage 行组件 React.memo**（虚拟化的替代项）。

验证：`pnpm test` + `pnpm build`；键盘走查（无鼠标完成解析→批量下载→切页）；真机冒烟批量下载。**刷新断点。**

## 阶段 4：清债与收尾

- SettingsPage：5 处滑杆 → SliderRow、3 处 select → Select、2 处 Toggle 收编（1081 行显著瘦身；更深拆分不承诺）。
- 全库 grep 扫描：手写按钮类名 / 弹层遮罩 / 原生 select 残留清零。
- 最终回归：`pnpm test` 全绿 + `pnpm build` + `cargo check`（证零 Rust 触点）。
- 文档同步：decisions.md 变更记录 + Handed-docs 时间轴条目 + 断点区复位；Modal/Toast/z 层级等新约定补进 ui-and-themes.md。

## 测试与汇报策略

按 testing.md 三档交付：自动测试（新组件/store/快捷键/批量选择逻辑各配单测，套 SettingsPage.test.tsx 的 loadOriginal 部分 mock + fireEvent 范式；存量 36 例全程保持绿）／人工冒烟（主题走查、下载全流程、快捷键——需真机，我在交付说明中列出待验证清单）／未验证项如实标注。

## 明确不做

日志页 react-window 虚拟化（已确认砍掉）、下载列表虚拟化（memo 已够）、SettingsPage 完全拆分、DrivePage 下拉改造为 Select（保留多行账号结构，只补关闭行为）、任何新依赖、任何 Rust/IPC 触点。

**给产品的理解**：这次升级不新增任何一个业务功能，而是把界面从「能用」提到「稳」——下载状态随时可见、操作反馈不再挤动页面、所有危险操作有回头路、按钮弹窗从此长得一样。用户说不出哪变了，但会觉得「这软件靠谱了」。
