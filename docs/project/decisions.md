# 文档演进：ADR、变更记录与规范修订

> 本目录（`docs/project/`）是唯一权威规范；根入口 `AGENTS.md` / `CONTRIBUTING.md` 只做路由。规范与代码冲突时以实现为准，并按本文流程回修文档。

## ADR（架构决策记录）

任何「有取舍、影响后续开发」的决策（新增平台、新增设置、主题体系调整、队列 / 存储策略变化、发布渠道变化……）追加一条 ADR 到本文件末尾的 `## 决策记录`，模板：

```markdown
### ADR-NNNN：标题（YYYY-MM-DD）

- **状态**：proposed / accepted / superseded by ADR-XXXX
- **背景**：为什么需要决策（问题 + 约束）。
- **决策**：选了什么，一句话可执行。
- **备选与取舍**：考虑过的方案、放弃原因。
- **影响**：落点文件 / 需要同步的文档 / 对旧数据的兼容策略。
```

编号递增不复用；被取代的 ADR 保留原文并标注 superseded。

## 变更记录模板（重要行为变更追加到本文件 `## 变更记录`）

```markdown
- **YYYY-MM-DD · 变更摘要**：改了什么行为；触点（文件 / 设置项 / IPC 字段）；兼容性说明（旧数据如何回退）；相关 ADR。
```

## 规范修订流程

1. 发现规范缺失 / 过时 / 与实现冲突：先在本次变更的影响分析里登记，不顺手大改。
2. 修订对应 `docs/project/*.md`，修订内容必须在本次变更中已被实现验证（新规范先落地，再写进规范）。
3. 根入口（`AGENTS.md` / `CONTRIBUTING.md`）只增删路由与硬性约束条目，不展开细节（唯一例外：接手文档协议作为最高优先级规则驻留根入口，因其必须随入口加载才有效力，见 ADR-0005）。
4. 有取舍的修订补 ADR；纯勘误直接改并加变更记录。
5. 编辑器规则（`.cursor/rules` 等）只引用这些文档路径，不复制规范正文——避免双份维护。

## 扩展清单（新事物出现时逐项过）

- **新增平台**：`api/<platform>.rs` → commands → IPC 类型成对 → parser 识别规则 → 测试（含错误路径）→ README 平台列表 → 本页 ADR。
- **新增设置项**：IPC 成对 + 默认值 + 旧数据回退 → 保存队列归属（通用 / 外观）→ 是否跳过 aria2 → 设置页 UI（含键盘可用性）→ [interfaces-and-compat.md](interfaces-and-compat.md)。
- **新增配色主题**：见 [ui-and-themes.md](ui-and-themes.md) checklist；改 ID 语义或 token 契约时属 ADR 级变更。
- **新增文档 / 目录**：更新 `AGENTS.md` 规范地图，保持「唯一权威文档」原则。

---

## 变更记录

- **2026-09-21 · 独立 CLI 与 function-call 工具入口**：新增 `src-tauri/src/bin/yunx.rs` 与 `cli.rs`，支持解析、列文件、流式下载、脱敏日志和 OpenAI-compatible 工具定义；默认人类文本输出，工具入口稳定返回 JSON。CLI 每次命令内建立解析会话，不落盘平台令牌；不改变 Tauri IPC、桌面 aria2 生命周期和旧数据格式。相关 ADR-0006。

- **2026-09-15 · 缺陷修复批次：引擎生命周期 / 交互竞态 / 任务列表一致性 / CI 注入面**：后端——aria2 引擎连接级失败触发互斥自愈重拉（5s 冷却，与 Unauthorized 自愈共用互斥），崩溃不再永久停摆；复用存活引擎时按 tellActive/tellWaiting 存活 gid 去重，不再重复 addUri 造成同文件二次下载；`.torrent` 任务种子字节落盘（`data_dir/torrents/{id}.torrent`），重启可断点恢复，副本随任务删除/清空清理；入队文件名净化（`sanitize_out_path`：剥 `..`/盘符分量、Windows 保留名、尾点尾空格，aria2 `out` 与本地删除原语共用）；`download_task` 终态 7 天自动清理 + `(status, finish_time)` 索引（消除每秒持锁全表扫描）；`list_tasks` 与事件快照窗口/排序对齐（消前端僵尸行）；节流进度落盘改单事务批量；完成后的转存清理与关机/睡眠动作移出轮询循环异步执行（不再阻塞全部任务进度事件）；托盘 tooltip ≥5s 节流；baidupcs stdin 写入纳入超时（防 RUN_LOCK 永久挂起）。前端——`useUpdate` 收敛为模块级单例（App 横幅与设置页卡片共享状态，修双实例重复下载与 check 失败死引用），下载进度 300ms 合帧；跨页解析请求在解析进行中改为排队续接（不再静默丢弃）；剪贴板提示 10s 自动消失；设置页滑杆键盘调整失焦落盘；提示条定时器互踩修复（设置/搜索/网盘文件管理页）；跨盘搜同款 Enter 守卫 + 过期响应丢弃；进度条 width% 改 `transform: scaleX()` 合成层动画；日志页内容无变化跳过 setState；滑杆 `accent-[#d97757]` 改 `accent-clay`（UI 硬编码主题色清零）。CI——`release.yml` GitCode 上传改参数数组调用 curl 并校验上传 URL 域名，移除 `eval` 拼接（堵签名私钥失窃面）。兼容性：IPC 契约（`ipc.ts` ↔ `models.rs`）零字段变化；`download_task` 仅新增幂等索引，旧库迁移不受影响；`list_tasks` 展示窗口收窄为「进行中 + 24h 内终态」（与事件流一致，超窗记录本就不在事件里）。相关时间轴「缺陷修复批次」。
- **2026-09-15 · 引入接手文档与断点协议**：新建 `docs/Handed-docs.md` 作为唯一当前记录（🔴 断点区 + 只追加时间轴）；AGENTS.md 新增目录地图、接手文档协议（写方 / 读方 / 灾难恢复）、授权边界、沟通、事实与加载，硬性约束追加断点纪律（第 8 条）；workflow.md 变更流程前置第 0 步「接手与断点」；规范修订流程第 3 条为根入口协议开例外。兼容性：纯文档与流程变更，不涉及代码与数据。相关 ADR-0005。
- **2026-09-07 · 更新体验与引擎同步报错修复**：`update_settings` 改为返回结构化结果（引擎同步失败不再作为错误抛出，前端改为非阻塞提示、不回滚）；aria2 RPC 先读文本再解析并对瞬时传输失败重试一次（根治「error decoding response body」误报）；更新横幅增加字节进度、失败时发布页兜底与重试、下载可取消、版本迁移展示；更新器新增 GitCode 回退源（CI 镜像改写 URL 后的 latest.json）。相关 ADR-0003 / ADR-0004。
- **2026-09-07 · 多配色主题 + 致谢折叠 + 规范体系**：新增七套配色主题（`colorTheme` 设置，与明暗独立）；强调色按钮白字改为 `text-on-accent`；错误 / 成功 / 警告状态与强调色分离（`text-danger` / `text-success` / `text-warning`）；设置页外观区新增主题卡；开源致谢改原位折叠（默认收起）；外观保存走共享队列（`useAppearanceSaver`），后端对仅外观变更跳过 aria2；建立本规范目录。兼容性：旧 settings.json 缺 `colorTheme` 回退 `warm-editorial`，未知 ID 同样回退，其余设置不动。相关 ADR-0001 / ADR-0002。

## 决策记录

### ADR-0006：独立 CLI 采用无状态工具调用与共享 AppState（2026-09-21）

- **状态**：accepted
- **背景**：需要让脚本、SSH 和本地 AI 调用云析，但 Tauri command 依赖 GUI 的 AppHandle，且解析会话包含平台令牌，不能简单跨进程复用或落盘。
- **决策**：新增独立 `yunx` binary；CLI 在进程内创建 `AppState`，复用解析与日志模块；function call 使用 OpenAI-compatible 工具定义，输入携带分享链接，每次调用临时建立会话；CLI 下载先采用流式 HTTP，不改桌面 aria2。
- **备选与取舍**：让 CLI 控制已运行桌面端（需本地服务、鉴权和端口协议，首版范围过大）；持久化 session key（会把平台令牌带入磁盘，放弃）；直接复刻解析逻辑（行为容易分叉，放弃）。
- **影响**：新增 `pnpm cli` 开发命令和 `tools` / `tool` JSON 接口；CLI 当前跨命令不保留解析会话，下载首版要求文件位于解析首页列表；后续如需长任务进度，应单独设计本地守护进程或事件流协议。

### ADR-0001：主题 = 语义 token 注入而非 CSS 文件切换（2026-09-07）

- **状态**：accepted
- **背景**：七套配色 × 明暗 = 14 组 token；若每主题一份 CSS 文件，则 token 不可测、对比度无法在 CI 强制，页面还会出现主题条件分支。
- **决策**：token 全量放 `src/lib/themes.ts` 注册表，运行时注入 `documentElement`；`tokens.css` 只保留语义变量 → Tailwind 工具类的映射与旧名兼容层；`themes.test.ts` 强制对比度。
- **备选与取舍**：CSS `data-theme` 选择器切换（不可测试，放弃）；CSS-in-JS（引入依赖，放弃）。
- **影响**：新增主题只改 `themes.ts`；`--on-danger` 等不随主题变化的 token 留在 CSS；已知限制——JS 生效前首帧依赖 `:root`/`.dark` 静态兜底（warm）。

### ADR-0002：外观保存走独立共享队列并让引擎同步跳过外观（2026-09-07）

- **状态**：accepted
- **背景**：顶部明暗切换改为持久化后，与设置页保存队列并发写 settings.json，存在「整包覆盖丢字段」竞态；且主题保存不应触碰 aria2 引擎。
- **决策**：明暗 / 配色统一经 App 层 `useAppearanceSaver` 串行队列（即时预览 → 合并写盘 → 失败回读回滚）；Rust `update_settings` 用 `appearance_only` 检测，仅外观变化跳过 `aria2::apply_settings`。
- **备选与取舍**：新增 patch 式 IPC 命令（更彻底但破坏现有接口形状，留待 ADR 复审）；两条队列互不感知（竞态残留，放弃）。
- **影响**：设置页经 `onSettingsUpdated` 事件与队列清空后 `getSettings` 对账吸收其他来源变更；已知限制——外观保存与设置页保存极端并发时仍有窄窗口，靠对账兜底。

### ADR-0003：引擎同步失败降级为非阻塞结果，RPC 瞬时失败重试（2026-09-07）

- **状态**：accepted
- **背景**：设置保存成功但 aria2 RPC 偶发失败（引擎重启 / 繁忙时空响应导致 reqwest "error decoding response body"）时，整条保存命令返回错误，前端把「已保存」当失败处理并回滚表单——用户看到误导性报错。
- **决策**：两层修复——(1) `rpc_call_raw` 先读文本再 `serde_json` 解析（错误可读），`rpc_call` 对瞬时传输失败（通信失败 / 响应读取失败 / 响应解析失败）短退避 400ms 重试一次；(2) `update_settings` 返回 `{ engineSyncFailed, engineSyncError }`，引擎失败只写日志 + 非阻塞提示，不再使保存失败。
- **备选与取舍**：仅重试不改返回结构（引擎真正离线时仍会误报，放弃）；前端忽略错误（丢失「重启引擎后生效」的提示，放弃）。
- **影响**：`updateSettings` IPC 契约从 void 变为结果对象（向前兼容：新增字段）；设置页 / 引导页保存不再被引擎状态阻塞。

### ADR-0004：应用内更新增加 GitCode 回退源（2026-09-07）

- **状态**：accepted
- **背景**：更新器此前只有 GitHub 单源，国内网络拉取 `latest.json` / 安装包易失败，用户只能手动去发布页。
- **决策**：`tauri.conf.json` 端点按序 [GitHub, GitCode]（插件逐个尝试直到成功）；CI 发布时把 `latest.json` 的平台下载 URL 改写为 GitCode 后镜像上传（GitCode 资产 URL 已实测支持 `releases/latest/download/{file}` 直出）。前端同步补体验：字节级进度、失败时发布页兜底 + 重试、下载可取消、错误信息友好化。
- **备选与取舍**：自建测速择优（旧方案已腐化，复杂度高，放弃）；仅加端点不改 CI（GitCode 无 latest.json，回退必失败，放弃）。
- **影响**：`release.yml` GitCode 步骤新增 latest.json 镜像；端到端回退路径需在下次真实发布时验证（本地无法验证签名 + 双源链路）。

### ADR-0005：接手文档与断点协议（2026-09-15）

- **状态**：accepted
- **背景**：多会话 AI 协作下，任务状态只存在于会话记忆里，会话消失即在途工作断片；git 只记录已提交结果，不记录在途意图与「为什么这么做」。曾发现工作区存在无人登记的改动而无人能说明来源（2026-09-15，9 个前端文件）。
- **决策**：引入 `docs/Handed-docs.md` 作为唯一当前记录——🔴 断点区（唯一可原地改写，空闲时显式写「无在途任务」）+ 时间轴（只追加）；写方 / 读方 / 灾难恢复三协议驻留根 `AGENTS.md`（模仿 `Web_SlayTheSpire` 体系适配），并列入硬性约束第 8 条。
- **备选与取舍**：协议放 `docs/project/handoff.md` 仅由根入口路由（强制力弱一层，会话可能跳过，放弃）；纯靠 git + commit message 传递状态（无在途状态、无验证现场，放弃）。
- **影响**：workflow.md 前置第 0 步、文档同步增加时间轴收尾；规范修订流程第 3 条为根入口开唯一例外；时间轴（任务执行视角）与 decisions.md 变更记录（规范行为视角）分工明确，不互相复制。
