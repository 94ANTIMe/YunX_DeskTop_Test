# Handed-docs — 接手文档（唯一当前记录）

> 本文件是项目唯一的「当前记录」，双层结构：**🔴 当前断点区**（在途任务的实时恢复点，全文档唯一允许原地改写的区域；空闲时显式写「当前无在途任务」）+ **时间轴**（已完成 / 正式终止的任务，只追加，禁止覆盖或改写历史条目）。读写与灾难恢复协议见根 [AGENTS.md](../AGENTS.md)「接手文档」一节。

与 [decisions.md](project/decisions.md) 的分工：行为变更的规范级记录（ADR、变更记录）在 decisions.md；任务执行过程、断点与验证状态在这里。一次变更两边都相关时各写各的视角，互相引用标题即可，不复制内容。

## 时间轴条目模板

```markdown
- **YYYY-MM-DD · 任务标题**
  - 做了什么：一段话，讲「实际发生了什么变化」。
  - 触点：改动的关键文件 / 设置项 / IPC 字段。
  - 验证：按「自动测试 / 人工冒烟 / 未验证」三档，附命令与结果；不适用也要写明为什么不适用。
  - 给产品的一句话：这次变更对使用者意味着什么。
  - 遗留：未做完 / 已知限制 / 后续方向；没有就写「无」。
```

## 🔴 当前断点区

**状态：无在途任务。**

销案说明：2026-09-15 建档时登记的「未登记在途改动」（基线 `19850c6`，彼时推测为另一会话在修缺陷）已确认为同日**缺陷修复会话**的产物——该会话按缺陷审查清单完成了引擎生命周期 / 交互竞态 / 性能 / CI 批次修复，全部测试通过，归属与详情见时间轴「缺陷修复批次」条目。现场登记就此关闭，无遗留在途工作。

## 时间轴

（最新在上，只追加）

- **2026-09-15 · 缺陷修复批次（引擎生命周期 / 交互竞态 / 性能 / CI）**
  - 做了什么：按缺陷审查清单修复 5 个优先项与一批已证实缺陷，行为变更明细见 [decisions.md](project/decisions.md) 变更记录同日条目（两边各写各的视角，不复制）。要点：引擎崩溃可自愈（互斥 + 冷却）、复用存活 aria2 不再重复下载、`.torrent` 任务可断点恢复（种子落盘 `data_dir/torrents/`）、下载文件名净化防路径穿越（含删除原语）、`download_task` 7 天自动清理 + 查询索引；前端更新器收敛单例（修双实例双下载）、跨页解析排队续接（修静默丢弃）、剪贴板提示 10s 自动消失、进度条改 `scaleX` 合成层动画、日志页免无谓重渲染；CI 发布脚本移除 `eval` 注入面。
  - 触点：后端 `src-tauri/src/aria2.rs`（核心）、`src-tauri/src/baidupcs.rs`、`src-tauri/src/db/mod.rs`；前端 `src/hooks/useUpdate.ts`、`src/App.tsx`、`src/pages/{ResolvePage,SettingsPage,DownloadPage,LogsPage,SearchPage}.tsx`、`src/components/{ClipboardPrompt,CrossDriveSearchModal,BatchQueuePanel,UpdateBanner,PanFileManager}.tsx`；`.github/workflows/release.yml`。IPC 契约（`ipc.ts` ↔ `models.rs`）零字段变化；未升版本号。
  - 验证：自动测试——`pnpm test` 36/36 通过；`cargo test` 15/15 通过（含新增 `sanitize_out_path` 2 例）；`cargo check` 通过；`pnpm build`（tsc + vite）通过；`git diff --check` 通过。人工冒烟——未验证（本环境无法运行桌面端真机）：引擎崩溃自愈的实际触发路径、BT 恢复、双入口更新状态联动、剪贴板提示观感需真机复验。未验证——`release.yml` GitCode 上传改动（eval → 参数数组 + 域名校验）需下次真实发布验证。
  - 给产品的一句话：下载更可靠（引擎崩溃不再永久停摆、重启不重复下载文件）、跨页解析和设置不再「点了没反应」、隐藏页面更省资源，发布链路少一个被劫持面。
  - 遗留：安全专项（网盘 Cookie 明文入库、CSP null、`--rpc-secret` 进程命令行暴露、第三方加速服务信任边界）与 baidupcs 全局串行锁按用户授权另行立项；PR 级 CI / lint / clippy 缺失未动。
- **2026-09-15 · 引入接手文档体系**
  - 做了什么：模仿 `Web_SlayTheSpire` 项目的 AGENTS.md 体系并适配本项目：AGENTS.md 新增目录地图、接手文档协议（写方 / 读方 / 灾难恢复）、授权边界、沟通、事实与加载五节，硬性约束追加「断点纪律」；新建本文件作为唯一当前记录；workflow.md 增加第 0 步挂接断点流程；decisions.md 修订「根入口不展开细节」规则并补 ADR-0005；CONTRIBUTING.md 变更流程增加接手步骤。
  - 触点：`AGENTS.md`、`docs/Handed-docs.md`、`docs/project/workflow.md`、`docs/project/decisions.md`、`CONTRIBUTING.md`，均为纯文档，无代码触点。
  - 验证：自动测试不适用（纯文档变更，不涉及运行时行为）；相对链接已逐一人工核对（`../AGENTS.md`、`project/decisions.md` 等路径与实际目录一致）。
  - 给产品的一句话：换一个 AI（或隔几周回来的人类）接手时，一分钟内能知道「做到哪了、从哪继续、什么不能碰」，不再靠猜。
  - 遗留：断点协议需在下一个真实任务中跑一遍检验顺手程度，不顺再修订；工作区那批未登记改动的归属待用户澄清。
