# AGENTS.md — AI 与开发者的项目入口

云析桌面版（Tauri 2 + React 19 + Rust，Windows）网盘分享链接解析与高速下载工具。

**本文件是所有修改（人类或 AI）的必读入口。** 它只做路由，权威规范全部在 [`docs/project/`](docs/project/) —— 修改任何代码前，先按下表找到对应文档并遵守；文档与代码冲突时，以当前实现为准并回过来修订文档（见 [文档演进](docs/project/decisions.md)）。

## 规范地图

| 文档 | 内容 | 何时必读 |
| --- | --- | --- |
| [docs/project/architecture.md](docs/project/architecture.md) | 前端 / IPC / Rust 分层职责与新增功能的放置原则 | 新增任何功能、文件归属拿不准时 |
| [docs/project/workflow.md](docs/project/workflow.md) | 变更流程：需求 → 影响分析 → 实现 → 回归 → 文档同步 | 开始任何修改之前 |
| [docs/project/interfaces-and-compat.md](docs/project/interfaces-and-compat.md) | IPC 字段同步、错误规范、设置默认值、数据库迁移、旧数据兼容与回退 | 改动 `src/lib/ipc.ts` ↔ `src-tauri/src/models.rs`、settings、数据库 |
| [docs/project/ui-and-themes.md](docs/project/ui-and-themes.md) | 主题 token 契约、新增主题 checklist、状态色、键盘可用性、减少动态效果 | 改 UI、配色、交互、动效 |
| [docs/project/async-and-resources.md](docs/project/async-and-resources.md) | 请求竞态、设置写入队列、任务状态一致性、会话与临时转存清理 | 涉及异步、下载任务、解析会话 |
| [docs/project/testing.md](docs/project/testing.md) | 按风险选测试、命令与结果记录、自动 / 人工 / 未验证的区分 | 交付任何修改之前 |
| [docs/project/release.md](docs/project/release.md) | 版本号同步、签名更新器、Secrets、GitHub 更新渠道与 GitCode 镜像 | 仅在准备发布时（日常修改不升版） |
| [docs/project/decisions.md](docs/project/decisions.md) | ADR 模板、变更记录模板、规范修订流程 | 做出架构决策、修订规范时 |

`CONTRIBUTING.md` 是人类贡献者的流程入口（环境搭建、提交与验收）。父目录 `../.trae/documents/` 为历史方案参考，引用其中内容前必须核对当前实现。

## 硬性约束（违反 = 不予合并）

1. **前后端字段成对修改**：IPC 类型在 `src/lib/ipc.ts`（TS）与 `src-tauri/src/models.rs`（Rust，camelCase 序列化）成对出现；settings 新增字段必须带默认值 + 旧数据兼容（缺字段 / 未知枚举回退，不清空其他设置）。
2. **UI 颜色只走 token**：业务组件禁止硬编码主题色 hex；颜色一律用 Tailwind 工具类（经 `src/styles/tokens.css` 语义层）。新增配色主题 = 在 `src/lib/themes.ts` 追加注册项 + token，不改页面。
3. **错误 / 警告 / 成功状态与强调色分离**：语义用 `text-danger` / `text-warning` / `text-success`，装饰才用 `text-clay` 家族；强调色按钮文字用 `text-on-accent`，禁止 `text-white`。
4. **外观保存不碰下载引擎**：明暗 / 配色变更经 `useAppearanceSaver` 队列；后端对仅外观变更跳过 aria2 同步（`commands/settings.rs::appearance_only`）。
5. **交互四件套**：可聚焦控件有键盘操作与 `:focus-visible` 环；折叠 / 弹层管理 `aria-expanded` / `inert`；过渡约 200ms 且尊重 `prefers-reduced-motion`；异步请求处理竞态与卸载后 setState。
6. **测试按风险交付**：见 [testing.md](docs/project/testing.md)；交付说明必须区分「自动测试通过 / 人工冒烟通过 / 未验证」三档。
7. **日常修改不升版本号**；版本号仅在准备发布时三处同步（见 [release.md](docs/project/release.md)）。

## 常用命令

```bash
pnpm test          # 前端 vitest
pnpm build         # tsc + vite build
pnpm tauri dev     # 桌面开发调试
cargo test         # 在 src-tauri/ 下运行 Rust 测试
cargo check        # 快速类型检查
```
