# 项目架构与边界

> 权威分层规范。新增功能前先在这里找到它该放的位置；放错位置的代码即使能跑也不予合并。

## 总览

```
src/                       前端（React 19 + TypeScript + Tailwind v4）
  pages/                   七个常驻挂载的页面（App 用 hidden 切换可见性，状态不丢）
  components/              跨页面复用的 UI 组件
  hooks/                   状态逻辑复用（useTheme / useAppearance / useUpdate）
  lib/                     与 UI 无关的纯逻辑：ipc.ts（IPC 封装 + 类型）、themes.ts（主题注册表）、format.ts、tabs.ts、download.ts
  styles/tokens.css        唯一的样式入口：语义 token 层 + @theme 别名 + 动效
src-tauri/src/             后端（Rust）
  commands/                Tauri command 层：参数校验、调用业务层、事件通知（薄）
  api/                     各网盘平台 API 封装（quark / uc / baidu / xunlei / pan123 / c139 / pansou …）
  db/                      SQLite（schema、账号存储）；业务表见 schema.rs
  aria2.rs                 下载引擎 sidecar 管理（JSON-RPC、任务编排、BT tracker 注入）
  resolve.rs / parser.rs   解析编排与链接识别
  subscription.rs          订阅追剧
  models.rs                跨 IPC 的数据结构（Settings 等；serde camelCase 对齐前端）
  state.rs                 AppState（DB 连接、HTTP 客户端、解析会话表、设置缓存）
  login.rs / clipboard.rs / tray.rs / crypto.rs / logger.rs / error.rs
```

## 职责边界

- **页面组件**只做布局与交互编排；可复用的状态逻辑进 `hooks/`，纯计算进 `lib/`。
- **commands 层保持薄**：不写业务规则，业务在 `aria2.rs` / `api/` / `resolve.rs` 等模块；commands 负责把结果转成 models.rs 的结构并 `emit` 事件。
- **IPC 是唯一前后端通道**：前端不直接访问 Rust 模块；Rust 不感知页面结构。字段契约见 [interfaces-and-compat.md](interfaces-and-compat.md)。
- **样式只有 tokens.css 一个入口**：Tailwind 类经 `@theme` 语义别名生效；组件内不写 `<style>`、不写硬编码颜色（见 [ui-and-themes.md](ui-and-themes.md)）。

## 新增功能的放置原则

| 要加的东西 | 放在哪 |
| --- | --- |
| 新页面 / Tab | `pages/` + `lib/tabs.ts` 注册 + App 常驻挂载（或按需懒加载，需说明理由） |
| 跨页面的交互组件 | `components/` |
| 可复用状态逻辑 | `hooks/`（命名 `useXxx`；与 DOM/主题相关的副作用必须在 hook 内清理） |
| 新 Tauri 命令 | `commands/<域>.rs` 注册到 `commands/mod.rs` 与 `lib.rs` invoke handler；返回 `AppResult<T>` |
| 新平台 API | `api/<platform>.rs`，复用 `state.http`，遵守现有限速 / 日志约定 |
| 新设置项 | IPC 字段成对加（`ipc.ts` + `models.rs`），settings.json 默认值 + 旧数据回退；仅外观类字段注意走外观保存队列（[interfaces-and-compat.md](interfaces-and-compat.md)） |
| 新数据表 / 列 | `db/schema.rs` 迁移函数，非破坏性（见 [interfaces-and-compat.md](interfaces-and-compat.md)） |
| 新配色主题 | `src/lib/themes.ts` 注册表（checklist 见 [ui-and-themes.md](ui-and-themes.md)） |

## 关键机制速查

- **页面常驻**：App 不卸载页面（`hidden` 切换），因此页面内 state 天然「切页不丢」；隐藏页在 `memo` 下零 reconcile。新增页面必须维持这一约定或说明放弃原因。
- **设置保存双队列**：SettingsPage 自身维护通用设置保存队列（单飞 + 失败回读回滚）；明暗 / 配色走 App 层 `useAppearanceSaver` 共享队列。两个队列都通过 `settings:updated` 事件与后端对账。
- **下载引擎**：所有 aria2 交互收敛在 `aria2.rs`；设置里的引擎相关字段（限速 / 并发 / 代理）由 `update_settings` 同步，仅外观变更时跳过（`appearance_only`）。
