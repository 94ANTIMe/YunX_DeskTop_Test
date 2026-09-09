# 接口与兼容

> IPC 字段同步、错误规范、设置默认值、数据库迁移、旧数据兼容与回退策略。改动任何跨前后端的东西之前必读。

## IPC 字段同步（成对修改）

- TS 侧：`src/lib/ipc.ts`（类型 + `DEFAULT_SETTINGS` + 命令封装）。
- Rust 侧：`src-tauri/src/models.rs`（`#[serde(rename_all = "camelCase")]`）+ 对应 command。
- 新增字段 checklist：
  1. TS interface 与 Rust struct 成对添加；
  2. `DEFAULT_SETTINGS`（TS）与 `Default for Settings`（Rust）与 serde `#[serde(default = ...)]` 三处默认值一致；
  3. 前端使用处对缺失值有回退（`{ ...DEFAULT_SETTINGS, ...(await ipc.getSettings()) }` 模式）；
  4. 若是 settings 字段：考虑保存队列归属（通用设置走 SettingsPage 队列；外观类走 `useAppearanceSaver`）与是否需要跳过 aria2 同步。

## 错误规范

- Rust 返回 `AppResult<T>`（`error.rs`）；用户可读的中文 message，错误细节进日志（`state.log`）。
- 前端统一用 `errMsg(e)` 展示；错误展示必须给恢复路径（重试按钮 / 自动回读），不只弹一句失败。
- 日志中禁止出现凭据（参照 `aria2.rs::proxy_log_summary` 的脱敏约定）。

## 设置（settings.json 是唯一持久化来源）

- localStorage 仅作启动预览缓存：`yunx-theme`（明暗）、`yunx-color-theme`（配色）。启动时先应用缓存，后端 `get_settings` 回来后用 `useTheme.hydrate` 校准。
- 未知主题 ID / 缺失字段 / 非法数值一律回退默认值，**绝不清空或重置其他设置**；Rust 侧对缺失字段用 serde default（见 `models.rs::default_color_theme`），对空 `colorTheme` 在 `update_settings` 中兜底。
- 明暗模式保持历史数值语义：`0 跟随系统 / 1 浅色 / 2 深色`（`THEME_MODE_VALUE` / `themeModeFromValue`）。
- 仅外观变更（`darkMode` / `colorTheme`）时，后端 `update_settings` 跳过 aria2 引擎同步（`appearance_only`）；外观保存一律经 `useAppearanceSaver` 队列：即时预览 → 串行落盘 → 失败用后端回读值回滚页面、控件与缓存。
- **引擎同步失败不作为错误**：`update_settings` 返回 `{ engineSyncFailed, engineSyncError }`（`UpdateSettingsResult`）。设置落盘成功即 Ok，引擎同步失败（引擎未启动 / 重启中 / RPC 异常）只产生非阻塞提示，前端不报错、不回滚、不回读；RPC 层对瞬时传输失败（空响应 / 连接抖动）先重试一次。
- 设置事件：每次成功保存后后端 `emit("settings:updated")`；前端监听处刷新本地 ref（设置页在其保存队列空闲时才合并，避免覆盖未提交修改，队列清空后用 `getSettings` 对账）。

## 数据库迁移（非破坏性）

- 建表 / 加列在 `db/schema.rs`；用 `ALTER TABLE ... ADD COLUMN` 或「新表 + 拷贝」两种方式，禁止 DROP / RENAME 已有列。
- 旧库打开后必须能直接用：缺列给默认值，缺表建表；迁移失败要留日志并让应用降级可用，而不是崩溃。
- 清空类操作（清任务 / 清历史 / 清日志）不得触碰统计聚合等独立表。

## 旧数据兼容与回退策略（总纲）

1. 读到旧数据：补默认值，不报错（settings、DB 同理）。
2. 读到非法数据：回退默认值并记日志，不崩、不扩散（未知主题 ID → warm-editorial）。
3. 写入失败：读回后端真实值恢复 UI，让用户重试，不静默吞掉。
4. 升级安装：应用数据目录不变，全部兼容逻辑以上述三条为准。
