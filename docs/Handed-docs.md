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

**状态：空闲——当前无在途任务。**

- 三线总任务全部完成：A 夸克缺陷修复（装机 D:\YunX，**真机复测待用户**）、B 前端界面升级（真机主题/键盘走查待做）、C 后端解耦。条目见时间轴。
- 本地 commit 已到 `093aa69` 及文档提交，**均未 push**；全程未升版本号。
- 已知暂缓项（按需再立项）：① ipc.ts 分域与 ResolvePage 区块深拆（理由见 ADR-0007 执行结果注记）；② LoginDialog 壳未迁 ui/Modal（WebView 登录）；③ 各平台真机解析/下载抽测未做。
- 若用户真机复测夸克打回：按时间轴 A 线条目的预案插小型修复批次（单独 commit）。
- 2026-09-22 10:14 重新打包（含 A+B+C 三线全部改动，基线 `0980401`）并静默安装 D:\YunX：`yunx-desktop.exe` 体积 21211648 → 21240320 字节、时间戳 Sep 22 02:04 → Sep 22 10:13（NSIS 保留打包内 mtime=构建时刻），安装校验通过。此前断点/时间轴提到的「装机 02:04」为 A 线单批次旧包，已被覆盖。
- 2026-09-22 10:34 发现用户侧同时跑了 3 个云析实例（应用无单实例保护），日志「失联恢复」连发即此因；且 10:33 那次安装因实例锁文件被静默跳过。已修复重挂加在途互斥（`0cf0874`）并在实例退出后重装：D:\YunX 主程序 21238784 字节、时间戳 10:32，校验通过。**待立项：tauri-plugin-single-instance 单实例保护（新增依赖，需用户授权）。**

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

**状态：进行中——C 线解耦模块化（三线总任务第三线；A、B 线已完成见时间轴）。**

- 目标：后端结构解耦、行为零变化。顺序与验收：C1 ADR-0007（结构决策入档）→ C2 斩断 api↔resolve 循环依赖（`load_account_cookie` 下沉 `credentials.rs`，`cargo check` 证依赖方向）→ C3 api 公共层（quark/uc refresh_session 合并、公共轮询 helper、错误映射归一）→ C4 `PanPlatform` trait + resolve.rs 三段 match 改注册表（逐平台迁移，quark 先行，每迁一个全量测试）→ C5 aria2.rs 拆 `aria2/` 子模块（rpc/engine/tasks/policy，statics 收敛）→ C6 models.rs 拆 models/ 分域 + ipc.ts 分域（re-export 保路径）→ C7 前端收尾（STATUS_TEXT/平台注册表单一来源、ResolvePage 区块拆分）。
- 验收：每步 `cargo test`（27）+ `pnpm test`（56）+ `cargo check` 全绿、独立 commit、断点刷新；全程行为零变化（IPC 字段、DB 结构、UI 行为不变）。
- 基线 commit：`6431a18`（B 线完成点）。
- 已完成：C1 ADR-0007 入档（决策与取舍，规范性 architecture.md 修订随各步落地——按「新规范先落地再写进规范」规则拆分执行）。
- 已完成（C2 斩环，2026-09-22）：新增 `src-tauri/src/credentials.rs`（`load_account_cookie` 从 resolve 下沉，含「刚登录未落库」窗口期语义注释）；`quark_fetch_ctx` 移入 `api/quark.rs`（夸克域逻辑归 api）；`is_captcha_blocked`/`captcha_hint` 移入 `api/baidu.rs`（百度域错误映射归 api）；`api/pan_files.rs`、`api/baidaccel.rs` 全部改引 credentials/baidu/quark——api 层对 resolve 的引用清零（grep 验证），api→resolve 单向依赖成立。验证：`cargo test` 27/27、`cargo check` 0 警告、`pnpm test` 56/56。
- 已完成（C3 api 公共层，2026-09-22）：`api/mod.rs` 上提 `set_cookies`（原 quark/uc 各自手抄）与 `refresh_puus_session`（夸克/UC 会话刷新同构逻辑合并，剥 __puus → config → 合并轮换 Cookie）；两平台 `refresh_session` 变一行委托。**轮询 helper 与错误映射归一经评估不做**：六套轮询中四个是事件循环（aria2 poll/clipboard/login/subscription）语义各异，baidupcs(300ms 无计数) 与 quark(1s 带失败计数) 形态不同，强行统一是为合并而合并；各家错误结构（pan123 code/xunlei error_code/baidu errno）本质是不同 API 契约，统一层依赖 C4 trait 先定义每平台错误语义，随 C4 一并考虑。
- 已完成（C4 主体，2026-09-22）：`resolve.rs` 定义 `PanPlatform` trait（fill_session/list_files/fetch_link，原生 async fn in trait 静态分发，无新依赖），已迁移 7/9 平台：Quark（试点）、Uc、C139、Pan123、Xunlei（clone_runtime 助手收敛运行时克隆）、Direct、Magnet——三段 match 的对应分支变一行委托，编排体内聚到各 impl。行为还原细节：Pan123 list 的 `dir_changed` 语义改由 impl 读 `session.last_dir` 自行判定（调用方回写时序不变）；Xunlei 运行时克隆/登录校验收敛 `clone_runtime`。Baidu（accel 双路由 + official 回退）待迁，是 C4 最后一步。验证：`cargo test` 27/27、`cargo check` 0 警告、`pnpm test` 56/56。
- 已完成（C4 收官，2026-09-22）：BaiduPlatform 迁入（fill/list/fetch_link 三段，accel 双路由收敛为 impl 内分支，list 的两个守卫臂合一）。**9/9 平台全部完成 PanPlatform trait 化**；resolve.rs 三段 match 各剩 9 行一行委托；新增平台 = api/<plat>.rs 落接口 + impl PanPlatform + 三行注册。验证：`cargo test` 27/27、0 警告、`pnpm test` 56/56、`pnpm build` 通过。
- 已完成（C5，2026-09-22）：`aria2.rs` → `aria2/` 目录四模块——`policy.rs`（200 行：状态映射/并发调参/僵死守护/路径净化/代理参数等纯函数，测试随迁）、`rpc.rs`（180 行：RPC_PORT/rpc_secret/rpc_url/APP_HANDLE/rpc_call_raw/rpc_call 及自愈触发引用）、`engine.rs`（465 行：tracker 集群/HEAL 冷却/spawn_sidecar/start/resolve_download_dir/live_engine_gids/resume_pending_tasks/resume_torrent_task）、`mod.rs`（1190 行：任务状态机 = 入队/暂停恢复/poll_loop/列表详情/apply_settings，即 ADR 的 tasks 域驻留根模块）。statics（APP_HANDLE/HEAL_LOCK/LAST_*/TRACKERS）随域归位。外部调用方（commands/subscription/cli/lib）路径零变化（glob 再导出）。验证：`cargo test` 27/27、0 警告、`pnpm test` 56/56、`pnpm build` 通过。
- 已完成（C6 Rust 侧 + C7 注册表，2026-09-22）：① `models.rs`（420 行）拆为 `models/` 六域文件——platform/resolve/download/account/settings/search，mod.rs 仅域声明 + glob 再导出，`crate::models::X` 外部路径零变化；② 新增 `src/lib/download-status.ts` 状态语义单一来源（STATUS_* 常量 + statusText），DownloadPage/TaskDetailDrawer/DownloadSummary/useDownloads 的本地 STATUS_TEXT 副本与全部状态魔法数字收编（含终态 toast 检测）。验证：`cargo test` 27/27、0 警告、`pnpm test` 56/56、`pnpm build` 通过。
- 当前计划（C 线收尾两件事）：① ipc.ts 拆 `lib/ipc/` 域模块（index.ts 聚合，外部 import 路径不变）；② ResolvePage（约 1000 行）拆区块组件 + hooks。完成后写时间轴条目、断点复位。
- 不改：行为与语义（纯结构重构）、IPC 契约、DB 结构、版本号；不 push。
- 若用户真机复测打回夸克修复：插一个小型 Rust 修复批次（单独 commit），再继续 C 线。
- 提交均为本地 commit（`e7ca036`…`30eaaf5` 及文档提交），**未 push**；不升版本号。

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

**状态：进行中——三线总任务：A 夸克缺陷收尾 → B 前端界面整体升级 → C 解耦模块化（当前在 A 线）。**

- 目标：A 线补齐夸克下载卡死（「等待队列 / 下载中 0 速度」）的遗留缺口并打包真机复测；B 线按用户提供的 UI 升级方案执行（纯前端，零 IPC/Rust 触点）；C 线解耦重构（平台 trait、aria2 拆分、models/ipc 分域）。三线串行，每阶段 = 实现 + 验证 + 刷新断点 + 独立 commit。
- 验收：A 线六项修复（A1 夸克 lowest-speed-limit 僵死中止 / A2 恢复重取链 / A3 引擎自愈后重挂任务 / A4 resume 查真实状态落库 / A5 max_concurrent_downloads 暴露设置页并热更 / A6 轮询连续失败带错退出 + 登录文案区分场景）各配锁定测试；`cargo test` + `pnpm test` + `pnpm build` 全绿；打包静默安装 D:\YunX 后用户真机复测夸克链接通过。
- 基线 commit：`ba7c257`；批次 commit：`e7ca036`（夸克修复四文件：轮询 60s、夸克并发 4/4、`__puus` 随直链下发、错误码可读化——即上一任务成果，已先提交）。
- 已完成（2026-09-22 现场核验）：上一断点「cargo test 被 cli.rs 阻塞」的记载**已过时作废**——cli.rs（626 行、含 4 个测试）与 bin/yunx.rs 已由 CLI 会话补全，23 个测试静态自洽（见时间轴 09-21 CLI 条目）；工作区两批未提交改动已按「先夸克修复、后 CLI」顺序分批提交，保证每个提交点可独立编译。
- 已完成（Phase 0 基线验证）：`pnpm test` 36/36 通过、`cargo test` 23/23 通过、`cargo check` 通过；工作区干净。当前正在 A1。
- 已完成（A 线里程碑 1：恢复链路，A1+A2+A4）：① 夸克任务 addUri 带 `lowest-speed-limit=1024`（0 速度约 30s 中止转失败，不再永久占并发槽），参数组装抽成纯函数 `http_task_options`/`stall_guard_options` 并配 2 个锁定测试；② download_task 表新增 `fetch_ctx_json` 列（schema + ALTER 迁移 + 旧库测试），`DownloadLink.fetchCtx` IPC 成对字段（models.rs/ipc.ts，skip_serializing_if 空），夸克转存/直取/个人文件三条取链路线都写入 `{"fid":...}` 上下文，`resolve::refresh_quark_download_link` 恢复前刷新 `__puus` 并重跑取链（失败回退旧直链只记日志）；③ resume 重写：不再无条件强写「下载中」，unpause/重入队后 `tell_status_mapped` 查 aria2 真实状态落库（查不到回退排队态）。`cargo test` 26/26 通过、`cargo check` 通过。
- 当前计划：A3（引擎自愈后重挂任务 + poll 失联分支不冻结）→ A5（并发设置暴露 + 热更）→ A6（poll_task 错误退出 + 登录文案）→ A 线回归 → 打包装机。
- 已完成（A 线里程碑 2：自愈重挂与小修，A3+A6；A5 核实为零改动）：① A3——poll_loop 批量查态后检测「活跃 gid 全部失联」→ 后台触发 `remount_if_detached`（复用启动恢复 `resume_pending_tasks` + 10s 冷却 + 引擎健康复检），不在 respawn_engine 原地重挂（会与 rpc_call 重试 addUri 撞车双入队）；取链刷新抽成 `refreshed_target` 助手，恢复/重挂两路共用（成功回写 DB）；② A6——`quark::poll_task` 连续 3 次非 200 带错误码+消息失败（`poll_non_ok_failure` 纯函数 + 测试），不再静默吞满 60s；夸克「请先登录」文案统一为 `QUARK_LOGIN_HINT`（覆盖刚登录未落库的竞态窗口，resolve.rs 4 处）；③ A5——核实 `maxConcurrentDownloads` 前端接口/默认值/设置页滑杆/`apply_settings` 热更**均已存在**，此前调研按 snake_case 误报为缺失，零改动。
- A 线回归（2026-09-22）：`cargo test` 27/27、`pnpm test` 36/36、`pnpm build` 成功、`cargo check` 通过。**真机夸克复测：未验证（待用户）**。
- 已完成（B1 全局感知层，2026-09-22）：① `src/lib/toast.ts` 单例（success/error/info，队列上限 4、自动消失、同类去重；error 级 7s 其余 4s 由实现细化）+ `src/components/ToastHost.tsx`（右下 z-[80]，全库首个 aria-live 容器）；② `src/hooks/useDownloads.ts` 单例 store：收编事件订阅/mergeTasks 去重/速度采样（暴露 getSpeedHistory），派生 activeCount(0/1/2)与 totalSpeed(status1 与 DownloadSummary 同口径)，终态迁移自动 toast（首次全量不弹），`useDownloadsState(active)` 用 INERT 哨兵快照保住「隐藏页零重渲染」；③ TopCapsule 下载 pill 加任务数角标+速度小字（非聚焦 span + aria-label），App 接线并挂 ToastHost；④ B1.4 迁移：ResolvePage（notice+error 全量）、SettingsPage（notice/保存失败/目录/代理测试 → toast，loadError 重试页保留）、PanFileManager（全量）、SearchPage（notice + 订阅等操作错误 → toast；「请先配置 PanSou」表单引导错误**保留内联**——与计划的差异点，理由：引导错误需持续可见）。DownloadPage 改为 store 订阅，删掉自身订阅/合并/采样/暂存逻辑。测试更新：ResolvePage/SettingsPage 测试挂载 ToastHost。验证：`pnpm test` 44/44（存量 36 + toast 4 + store 4）、`pnpm build` 通过。
- 已完成（B2.1+B2.2 基础组件层原语，2026-09-22）：新增 `src/components/ui/`——`Button`（variant primary/outline/danger/soft × size sm/md/lg，实底禁用 50/描边 40，enabled:hover 防残留）、`IconButton`（aria-label 必填）、`Toggle`（role=switch，md h-6 w-11 / sm h-5 w-9，旋钮 bg-on-accent）、`Select`（原生封装 {value,label}[]）、`Skeleton`（aria-hidden + motion-reduce:animate-none）、`SliderRow`（拖动即时 onInput / 松手失焦 onCommit）、`Modal`（z-[70]、遮罩 bg-black/40 backdrop-blur animate-fade、面板 animate-rise、初始焦点+关闭还焦+body 滚动锁、Esc/点遮罩可配）、`Drawer`（收编 220ms leaving 机制，同 TaskDetailDrawer）、`ConfirmDialog`（基于 Modal，danger=alertdialog）；`EmptyState` 扩展 icon 变体。单测 primitives.test.tsx（9 例）+ Modal.test.tsx（4 例）。验证：`pnpm test` 56/56、`pnpm build` 通过。页面尚未迁移（B2.3 待做）。坑位：一次 Write 报成功但 Skeleton.tsx 实际未落盘，测试导入失败才暴露——离奇失败先查文件真实存在性。
- 已完成（B2.3 弹层迁移，2026-09-22）：① TaskDetailDrawer → ui/Drawer（保留数据快照与 2s 轮询，滑出动画/Esc/滚动锁交由 Drawer）；② BatchQueuePanel → ui/Drawer（Drawer 新增 bodyClassName 支持多段布局：固定粘贴区 + 滚动列表）；③ DownloadPage 的 ConfirmDialog 切换为 ui/ConfirmDialog，旧 components/ConfirmDialog.tsx 删除；④ CrossDriveSearchModal → ui/Modal（Modal 扩展 ReactNode 标题 + bodyClassName）；⑤ SearchPage 新建订阅弹层 → ui/Modal（取消/订阅按钮移入 footer）；⑥ ResolvePage 收藏夹 + 解析记录浮层 → ui/Modal（清空记录按钮移入 title 区）。**LoginDialog 有意未迁**（WebView 登录壳，改动风险大于收益；B4 复评或长期保留）。验证：`pnpm test` 56/56、`pnpm build` 通过。主题 7×2 走查待真机（B4 前做）。
- 已完成（B3 交互升级八项，2026-09-22）：① 3.2 破坏性确认 ×8——ResolvePage 删收藏/删单条记录/清空记录（一个 ConfirmDialog + confirmAsk 状态机）、LogsPage 清空日志、SearchPage 删订阅、DrivePage 登出两入口（confirmLogout{platform,key}）、DownloadPage 取消单任务（removeTask 打开确认，doRemoveTask 执行）；② 3.3 全局快捷键——Ctrl+1~7 按可见 Tab 顺序切页（search 关闭时自动占位后移）、Ctrl+V 非输入焦点粘贴链接直接 goResolve；③ 3.4 DrivePage 账号下拉 mousedown 点击外部 + Esc 关闭（data-account-menu 标记）；④ 3.5 StatsPage 日柱 tabIndex=0 + role=img aria-label + group-focus-visible 明细浮层；⑤ 3.6 Onboarding 跳过/完成 → setStep(2)，完成页「进入云析」→ onDone（三步流程修复）；⑥ 3.7 骨架屏 ×4——DrivePage 账号卡（loaded 标志修「闪未登录」）、DownloadPage 初始列表（store.loaded 修「闪空态」）、StatsPage KPI+图表、LogsPage 列表；⑦ 3.8 LogsPage 行抽 LogRowItem + memo（toggleRow 稳定回调）；⑧ 3.1 解析结果多选批量下载——selectedFids Set + 行首 checkbox（aria-label）+ 底部操作条（已选 N 项/总大小/全选本页/清除/下载选中），downloadSelected 串行取链入队带失败计数，batchBusy 独立守卫；单文件下载互斥放宽为「仅本行在飞禁点」（downloadingFid 改 Set，跨行可并行）。验证：`pnpm test` 56/56、`pnpm build` 通过。
- 当前计划：B4 清债（SettingsPage 5 滑杆→SliderRow、3 select→Select、2 Toggle 收编；全库 grep 手写按钮/遮罩/原生 select 残留清零；最终回归 pnpm test+build+cargo check；decisions.md 变更记录 + ui-and-themes.md 补 Modal/Toast/z 层级约定 + 时间轴条目 + 断点复位）→ C 线 C1→C7。
- 已完成（A 线打包与安装，2026-09-22）：`pnpm tauri build`（跳更新器签名）产出 NSIS/MSI；静默安装到 `D:\YunX` 成功——`yunx-desktop.exe` 时间戳 Sep 15 → **Sep 22 02:04**（本次构建时刻，NSIS 保留打包内 mtime）、体积 21155328 → 21211648 字节。坑位记录：git-bash 直接调 NSIS 会把 `/S /D=` 当路径转换导致安装器卡在交互向导，需 `MSYS2_ARG_CONV_EXCL="*"` 前缀。**夸克真机复测：未验证——等用户用测试链接复测**。
- B 线方案要点（执行以用户原文为准）：B1 全局感知层（useDownloads 单例 store、TopCapsule 下载角标、全局 Toast、四页 notice 迁移）；B2 基础组件层 `src/components/ui/`（Button/IconButton/Toggle/Select/Skeleton/SliderRow/EmptyState + Modal/Drawer/ConfirmDialog 统一，逐页迁移 + 7 主题抽查）；B3 交互升级（解析多选批量下载、破坏性确认×8、Ctrl+1~7 / Ctrl+V 快捷键、DrivePage 下拉关闭、StatsPage 键盘可达、Onboarding 三步修复、骨架屏×4、LogsPage memo）；B4 清债（SettingsPage 控件收编、grep 残留清零、文档同步）。硬边界：零新依赖、零 Rust/IPC 触点、不升版本、不 push、颜色只走 token、交互四件套。已确认砍掉：日志页虚拟化。
- 不改：夸克 API 字段识别、发布配置；三线全程不升版本号、不 push。

销案说明：2026-09-15 建档时登记的「未登记在途改动」（基线 `19850c6`，彼时推测为另一会话在修缺陷）已确认为同日**缺陷修复会话**的产物——该会话按缺陷审查清单完成了引擎生命周期 / 交互竞态 / 性能 / CI 批次修复，全部测试通过，归属与详情见时间轴「缺陷修复批次」条目。现场登记就此关闭，无遗留在途工作。

## 时间轴

（最新在上，只追加）

- **2026-09-22 · C 线：后端解耦（斩环 / api 公共层 / 平台 trait 化 / aria2 与 models 分域）**
  - 做了什么：按 ADR-0007 完成 Rust 侧结构解耦——C2 `credentials.rs` 承接凭据读取，api 层对 resolve 引用清零（环斩断）；`quark_fetch_ctx` 归位 api/quark、百度验证码助手归位 api/baidu；C3 `set_cookies`/`refresh_puus_session` 上提 api 公共层（quark/uc 去重）；C4 `PanPlatform` trait 落地，9/9 平台（夸克/UC/百度/139/123/迅雷/直链/磁力）编排从三段巨型 match 迁入独立 impl，新增平台从「改 6 处」变「impl + 三行注册」；C5 `aria2.rs` 拆 policy/rpc/engine/mod(tasks) 四模块，6 个全局 static 随域归位；C6 `models.rs` 拆六域文件。C7 状态语义单一来源 `lib/download-status.ts`（STATUS_* + statusText），四处本地副本与魔法数字收编。轮询 helper 与错误映射统一经评估不做（形态各异，强行统一为负收益）；ipc.ts 分域与 ResolvePage 深拆暂缓（理由见 ADR-0007 执行结果注记）。
  - 触点：新增 `credentials.rs`、`models/`（6 文件）、`aria2/`（mod/policy/rpc/engine）、`lib/download-status.ts`；改 `resolve.rs`、`api/{mod,quark,uc,baidu,pan_files,baidaccel}.rs`、`lib.rs`、`DownloadPage/TaskDetailDrawer/DownloadSummary/useDownloads`。IPC 契约与 DB 结构零变化。
  - 验证：自动测试——每步 `cargo test` 27/27、`cargo check` 0 警告、`pnpm test` 56/56、`pnpm build` 通过；每步独立 commit（7063154…093aa69）。人工冒烟——解耦为纯结构重构，行为回归依赖既有 83 例自动化测试与后续真机使用；未做专门的真机全平台解析回归（未验证项：各平台真实分享链接解析/下载抽测）。
  - 给产品的一句话：内部焕新、外表无感——九个网盘的对接逻辑各自归位，以后接新网盘是「培训一个新快递员」而不是「翻修整栋楼」，这段代码的下一位维护者（人或 AI）都能快速定位。
  - 遗留：ipc.ts 分域与 ResolvePage 区块深拆（暂缓，理由入档）；architecture.md 增量修订随暂缓项一并处理；各平台真机抽测未做。

- **2026-09-22 · B 线：前端界面整体升级（全局感知层 / 基础组件层 / 交互升级 / 清债）**
  - 做了什么：按用户提供的 ui-upgrade-plan.md 执行四阶段。B1 全局感知层：`useDownloads` 下载任务单例 store（事件合并/速度采样/终态迁移自动 toast/派生统计，隐藏页零重渲染保持）、全局 Toast（`lib/toast.ts` + ToastHost，aria-live）、顶栏下载角标+速度、四页反馈迁移 toast；B2 基础组件层：`components/ui/` 九个原语（Button/IconButton/Toggle/Select/Skeleton/SliderRow/Modal/Drawer/ConfirmDialog）+13 例单测，全部弹层（任务详情/批量队列/搜同款/订阅/收藏/历史/确认）迁入统一壳层，删旧 ConfirmDialog；B3 交互升级：解析多选批量下载、破坏性确认×8、Ctrl+1~7/Ctrl+V 快捷键、DrivePage 下拉 click-outside+Esc、StatsPage 柱状图键盘可达、Onboarding 三步修复、骨架屏×4、LogsPage 行 memo；B4 清债：SettingsPage 5 滑杆/3 下拉/2 开关收编 ui 原语，全库手写 select/range/switch/遮罩清零（LoginDialog 为登记例外），新约定写入 ui-and-themes.md，变更记录入 decisions.md。
  - 触点：新增 `lib/toast.ts`、`hooks/useDownloads.ts`、`components/ui/*`、`docs/project/ui-upgrade-plan.md`；改 App/TopCapsule/ToastHost/七页/TaskDetailDrawer/BatchQueuePanel/CrossDriveSearchModal/EmptyState/decisions/ui-and-themes/Handed-docs。IPC 与 Rust 零变化（cargo check 证明）。
  - 验证：自动测试——`pnpm test` 56/56（存量 36 语义保持 + toast 4 + store 4 + ui 原语 13 中计入 Modal 4 与 primitives 9，总数对齐）、`pnpm build` 通过、`cargo check` 通过（证零 Rust 触点）。人工冒烟——未验证：7 主题×明暗视觉走查、键盘走查（无鼠标解析→批量下载→切页）、真机批量下载与 toast 观感、Onboarding 新流程，需真机执行。
  - 给产品的一句话：界面没有多出一个新功能，但下载进度随时挂在顶栏、操作反馈不再挤动页面、每个危险操作都有回头路、所有按钮弹窗长得一样——软件从「能用」变「稳」。
  - 遗留：真机主题/键盘走查待做；LoginDialog 壳未迁（WebView 登录，登记例外）；SettingsPage 深拆按边界不做。

- **2026-09-22 · A 线：夸克下载卡死修复（六项 + 打包装机）**
  - 做了什么：修「等待下载队列 / 下载中 0 速度」三类根因——①夸克任务 `lowest-speed-limit` 低速僵死中止（http_task_options/stall_guard_options 纯函数锁定）；②恢复重取链：download_task 加 `fetch_ctx_json` 列，夸克三条取链路线写上下文，恢复/重试/重挂前刷新 `__puus` 重取直链（失败回退旧链）；③引擎失联自动重挂（poll 检测 gid 全失联 → 复用 resume_pending_tasks，10s 冷却）；④resume 查 aria2 真实状态落库（不再假「下载中」）；⑤`max_concurrent_downloads` 设置项核实为已实现（此前调研误报）；⑥转存轮询连续失败快速报错 + 登录文案区分竞态。完成后本地打包（跳更新器签名）静默安装 D:\YunX 并校验时间戳（Sep 22 02:04）。
  - 触点：`aria2.rs`、`resolve.rs`、`api/quark.rs`、`api/pan_files.rs`、`models.rs`（DownloadLink.fetchCtx）、`db/schema.rs`+`db/mod.rs`（fetch_ctx_json 列迁移）、`commands/download.rs`、`subscription.rs`、`ipc.ts`（fetchCtx 成对）、`lib/download.ts`、`ResolvePage.tsx`、`PanFileManager.tsx`。
  - 验证：自动测试——`cargo test` 27/27（新增锁定 5 例）、`cargo check`、`pnpm test`、`pnpm build` 全过。人工冒烟——**未验证：真机夸克下载复测待用户执行**（装机已完成，D:\YunX\yunx-desktop.exe 时间戳已确认为本批构建）。
  - 给产品的一句话：夸克下载不再「排队卡死、假下载」，僵死任务会自动报错可重试，重开应用/网络恢复后任务自己活过来。
  - 遗留：真机复测结论未知；若打回按断点区预案插小型修复批次。坑位：git-bash 调 NSIS 需 `MSYS2_ARG_CONV_EXCL="*"`。

- **2026-09-21 · 独立 CLI 与 function-call 工具入口**
  - 做了什么：新增独立 Rust CLI `yunx` 与本地 JSON 工具调用入口，支持解析、列文件、流式下载、脱敏日志查询；提供 `tools` 输出 OpenAI-compatible function definition，`tool` 统一返回 JSON；补充 `pnpm cli`、README、架构说明与 ADR-0006。CLI 每次命令内建立解析会话，不落盘平台令牌，桌面端原有 aria2 流程不变。
  - 触点：`src-tauri/src/cli.rs`、`src-tauri/src/bin/yunx.rs`、`src-tauri/src/lib.rs`、`package.json`、`README.md`、`docs/project/{architecture,decisions}.md`。
  - 验证：自动测试——`pnpm test` 36/36 通过；`cargo test` 23/23 通过；`cargo check` 通过；`pnpm build` 成功；`git diff --check` 通过。人工冒烟——`cargo run --bin yunx -- help`、`tools`、临时 APPDATA 下 `logs --json` 与 `tool get_logs` 均成功。未验证——真实网盘解析、取链和 CLI 下载尚未用账号/真实链接验收；CLI 下载首版只支持解析首页文件，未做长驻进度流。
  - 给产品的一句话：云析现在既能作为 Windows 桌面工具，也能被脚本、SSH 和本地 AI 以稳定 JSON 工具调用。
  - 遗留：如需跨命令会话、长任务进度或直接接入 MCP/模型，再单独设计本地守护进程与事件流协议；不阻塞本次首版。

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
