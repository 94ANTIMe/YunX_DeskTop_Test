# AGENTS.md — AI 与开发者的项目入口

云析桌面版（Tauri 2 + React 19 + Rust，Windows）网盘分享链接解析与高速下载工具，发布走 GitHub + GitCode 双渠道应用内更新。

**本文件是所有修改（人类或 AI）的必读入口**，只记录项目差异项与最高优先级规则；通用工程实践（任务规划、自测、诚实汇报、失败恢复）直接依赖模型内置能力，不复述。权威规范全部在 [`docs/project/`](docs/project/) —— 修改任何代码前，先按下表找到对应文档并遵守；文档与代码冲突时，以当前实现为准并回过来修订文档（见 [文档演进](docs/project/decisions.md)）。

## 目录地图

| 路径 | 职责 | 约束 |
| --- | --- | --- |
| `src/` | React 前端（`pages/` `components/` `hooks/` `lib/` `styles/`） | 颜色只走 token；`lib/ipc.ts` 与 Rust 侧成对修改 |
| `src-tauri/` | Rust 后端（`src/commands/`、`src/services/`、`models.rs`）与 Tauri 配置 | IPC 契约与 TS 侧成对修改 |
| `src-tauri/binaries/` | aria2 / BaiduPCS-Go sidecar 二进制（已入库） | 不随手替换或重命名（Tauri 按目标三元组命名） |
| `docs/project/` | 唯一权威规范（见下方规范地图） | 与代码冲突以实现为准，回修文档 |
| `docs/Handed-docs.md` | 唯一「当前记录」（断点区 + 时间轴） | 每次任务开始 / 里程碑 / 结束维护，见下节 |
| `dist/`、`src-tauri/target/`、`node_modules/` | 构建产物与依赖 | 不手工编辑，可随时重建 |
| `.github/workflows/release.yml` | 签名发布与 GitCode 镜像 | 仅发布时触碰（见 release.md） |

## 规范地图

| 文档 | 内容 | 何时必读 |
| --- | --- | --- |
| [docs/project/architecture.md](docs/project/architecture.md) | 前端 / IPC / Rust 分层职责与新增功能的放置原则 | 新增任何功能、文件归属拿不准时 |
| [docs/project/workflow.md](docs/project/workflow.md) | 变更流程：接手断点 → 需求 → 影响分析 → 实现 → 回归 → 文档同步 | 开始任何修改之前 |
| [docs/project/interfaces-and-compat.md](docs/project/interfaces-and-compat.md) | IPC 字段同步、错误规范、设置默认值、数据库迁移、旧数据兼容与回退 | 改动 `src/lib/ipc.ts` ↔ `src-tauri/src/models.rs`、settings、数据库 |
| [docs/project/ui-and-themes.md](docs/project/ui-and-themes.md) | 主题 token 契约、新增主题 checklist、状态色、键盘可用性、减少动态效果 | 改 UI、配色、交互、动效 |
| [docs/project/async-and-resources.md](docs/project/async-and-resources.md) | 请求竞态、设置写入队列、任务状态一致性、会话与临时转存清理 | 涉及异步、下载任务、解析会话 |
| [docs/project/testing.md](docs/project/testing.md) | 按风险选测试、命令与结果记录、自动 / 人工 / 未验证的区分 | 交付任何修改之前 |
| [docs/project/release.md](docs/project/release.md) | 版本号同步、签名更新器、Secrets、GitHub 更新渠道与 GitCode 镜像 | 仅在准备发布时（日常修改不升版） |
| [docs/project/decisions.md](docs/project/decisions.md) | ADR 模板、变更记录模板、规范修订流程 | 做出架构决策、修订规范时 |
| [docs/Handed-docs.md](docs/Handed-docs.md) | 当前断点区（在途任务）与任务时间轴 | 每次任务开始与结束 |

`CONTRIBUTING.md` 是人类贡献者的流程入口（环境搭建、提交与验收）。父目录 `../.trae/documents/` 为历史方案参考，引用其中内容前必须核对当前实现。

## 硬性约束（违反 = 不予合并）

1. **前后端字段成对修改**：IPC 类型在 `src/lib/ipc.ts`（TS）与 `src-tauri/src/models.rs`（Rust，camelCase 序列化）成对出现；settings 新增字段必须带默认值 + 旧数据兼容（缺字段 / 未知枚举回退，不清空其他设置）。
2. **UI 颜色只走 token**：业务组件禁止硬编码主题色 hex；颜色一律用 Tailwind 工具类（经 `src/styles/tokens.css` 语义层）。新增配色主题 = 在 `src/lib/themes.ts` 追加注册项 + token，不改页面。
3. **错误 / 警告 / 成功状态与强调色分离**：语义用 `text-danger` / `text-warning` / `text-success`，装饰才用 `text-clay` 家族；强调色按钮文字用 `text-on-accent`，禁止 `text-white`。
4. **外观保存不碰下载引擎**：明暗 / 配色变更经 `useAppearanceSaver` 队列；后端对仅外观变更跳过 aria2 同步（`commands/settings.rs::appearance_only`）。
5. **交互四件套**：可聚焦控件有键盘操作与 `:focus-visible` 环；折叠 / 弹层管理 `aria-expanded` / `inert`；过渡约 200ms 且尊重 `prefers-reduced-motion`；异步请求处理竞态与卸载后 setState。
6. **测试按风险交付**：见 [testing.md](docs/project/testing.md)；交付说明必须区分「自动测试通过 / 人工冒烟通过 / 未验证」三档。
7. **日常修改不升版本号**；版本号仅在准备发布时三处同步（见 [release.md](docs/project/release.md)）。
8. **断点纪律**：任务开始建断点、里程碑先刷新断点再继续、完成压缩为时间轴条目（协议见下节「接手文档」）。

## 接手文档（强制，最高优先级规则）

`docs/Handed-docs.md` 是唯一当前记录，双层结构：**🔴 当前断点区**（在途任务的实时恢复点，全文档唯一允许原地改写的区域；空闲时显式写「当前无在途任务」）+ **时间轴**（已完成 / 正式终止的任务，只追加，禁止覆盖或改写历史条目）。

三条原则：

1. **断点不是收尾动作**：刷新断点是开发步骤的一部分；会话随时可能消失，最坏只允许损失一个里程碑。
2. **里程碑 = 实现 + 验证 + 刷新断点**：三者完成后才允许进入下一阶段。
3. **断点不是绝对真相**：它只是上一任对现场的声明，接手方必须实时核验后才可信。

### 写方协议（执行任务的 AI / 开发者）

1. 任务开始后、实质修改前，先在断点区建立断点块（目标、验收标准、基线 commit、初始计划）；禁止写了大量代码后才补。
2. 以下任一事件发生后，必须**先刷新断点、再继续**：一组 `pnpm test` / `cargo test` 通过；`pnpm build` / `cargo check` 通过；失败原因确认；bug 根因确认；IPC 契约或设置项变更完成；架构决策确定；子功能完成；实验方向被证伪；即将切换主要文件组；进入明显不同的阶段；达到「会话此刻消失、新会话也应能从这里继续」的状态。
3. 标准循环：读断点 → 小阶段 → 验证 → 刷新断点 → 继续。不连续做多个阶段最后一起记录。
4. 任务完成：最终验证 → 断点内容压缩成时间轴条目（按 Handed-docs 内模板，含「给产品的一句话」）追加到时间轴最上方 → 断点区恢复「当前无在途任务」→ 按授权执行 commit。
5. 任务主动中止：刷新最终断点，状态改「阻塞 / 被中断」；未来仍要续做的保留断点块，本阶段正式结束的写时间轴。

### 读方协议（新会话接手顺序：断点 → Git → 验证 → 续做）

1. 先读断点区：有块 = 有任务没做完，默认续做它（用户明确要求做别的除外），不得直接开新任务。
2. 核验 Git：`git status`、`git diff`，对照工作区与断点记载是否一致、有无断点未提的改动、有无额外 commit。
3. 实际跑一次任务所需的最小验证（`pnpm test` / `cargo test` 等），不因断点写着「测试已通过」就轻信。
4. 发现文档状态 ≠ 真实状态：先把实况写回断点，再继续。
5. 从断点「下一步」直接续做，不重新规划整个任务；「已完成并验证」项默认跳过；「不要重做 / 已排除方向」不得重查（除非有新证据）。
6. 无在途任务时，读时间轴最近 3~5 条了解现状，避免重复劳动或破坏约定。

### 灾难恢复协议（断点缺失或明显过期时）

从 `git status` + `git diff` + 最近 commit + 最后一条时间轴 + 测试实况重建任务状态，建立新断点块后再继续。**禁止**不建断点凭感觉直接改代码。工作区出现无人登记的改动时同样适用：先把现场登记进断点区，不回滚、不夹带进自己的提交。

## 常用命令

```bash
pnpm test          # 前端 vitest
pnpm build         # tsc + vite build
pnpm tauri dev     # 桌面开发调试
cargo test         # 在 src-tauri/ 下运行 Rust 测试
cargo check        # 快速类型检查
```

## 授权边界

以下操作须用户逐项明确授权后才能执行；「继续」不扩大授权范围，已授权的事项不重复请示：

- git 写操作中不可逆的部分：push、rebase、reset、删分支、强制覆盖、删 tag；
- 发布：版本号三处同步、打 tag、触发 release.yml（日常修改不升版本，见硬性约束 7）；
- 删除或大量改动未跟踪 / 未登记的文件（先弄清来源，见灾难恢复协议）；
- 修改本 AGENTS.md 自身的规则条款。

凭据（密码、密钥、令牌、Cookie）不写入任何文件、提交或记忆；更新器签名私钥只存在于 GitHub Secrets / 本地密钥库，不落入仓库。

## 沟通

- 中文交流。结果用「实际发生了什么变化」描述；技术名词首次出现时括号加一句白话；解释架构或方案时，结尾附一小段「给产品的理解」。
- 「完成 / 通过 / 已验证」必须附证据（命令与输出、测试结果）；**文件已改 ≠ 测试通过 ≠ 生效**，三者分别陈述，按「自动测试 / 人工冒烟 / 未验证」三档汇报（模板见 [CONTRIBUTING.md](CONTRIBUTING.md)）。
- 每次回复结尾给出下一步的具体动作和预期结果；没有剩余必做项时明说，不为凑数造任务。

## 事实与加载

每类事实只有一个权威来源，引用时指向它而不是复制：

| 事实 | 权威来源 |
| --- | --- |
| 怎么改（规范） | `docs/project/` 对应文档 |
| 做到哪了（在途任务） | [docs/Handed-docs.md](docs/Handed-docs.md) 断点区 |
| 做过什么（任务史） | [docs/Handed-docs.md](docs/Handed-docs.md) 时间轴 |
| 行为变更史 / 决策取舍 | [decisions.md](docs/project/decisions.md)（变更记录 + ADR） |
| 版本与发布状态 | git tag + [release.md](docs/project/release.md) |
| 代码现状 | 实时检索代码，不凭记忆或旧摘要 |

- 易变事实（工作区状态、测试结果、版本号、任务进度）以实时核验为准；记忆、摘要、旧回执不能替代。
- 新任务默认只读：本文件 + 断点区 + 时间轴最近几条 + 命中的专题文档。**不默认全量扫描 `src/` 与 `src-tauri/`**，按需检索。
