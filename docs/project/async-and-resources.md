# 异步与资源生命周期

> 请求竞态、设置写入队列、任务状态一致性、会话释放与临时转存清理。涉及异步、下载、解析的修改必读。

## 请求竞态

- **页面级加载**：启动读取用 `let alive = true; return () => { alive = false; }` 模式（参照 App 的 `loadInitialSettings`），卸载 / 重载后丢弃过期响应。
- **可重入操作**（搜索、解析、取链）：后发优先——旧请求完成后不得覆盖新请求的结果；操作进行中禁用触发按钮或提供取消。
- **输入防抖与提交**：文本输入（PanSou 地址、代理）在失焦 / 显式提交时保存，不逐键触发 IPC。

## 设置写入队列（两条，勿混用）

1. **SettingsPage 通用队列**（页内 `persist` / `flushSettings`）：渲染 diff 合并 patch → 单飞（`saveInFlight` + `savePending`）→ 失败回读后端恢复。改动通用设置必须走它，不要直接 `ipc.updateSettings`。
2. **App 层外观队列**（`hooks/useAppearance.ts`）：明暗 / 配色专用；串行合并、失败回读回滚、未知主题规范化。后端对仅外观保存跳过 aria2。
3. 两个队列通过 `settings:updated` 事件与后端对账；队列空闲时才合并事件到本地 ref，避免覆盖未提交修改；队列清空后用 `getSettings` 兜底对账。

## 任务状态一致性

- 下载任务的唯一事实来源是后端：前端列表以 `downloads:updated` 事件为准，本地不做乐观状态机；暂停 / 继续 / 删除后等待事件刷新。
- 任务操作要幂等：重复点击暂停 / 删除不得产生副作用（按钮 busy 态 + 后端状态检查）。
- 全部完成后的关机 / 睡眠属不可逆动作：保留 60 秒取消窗口（`afterDownloadAction`），执行前有日志。

## 解析会话与临时转存（资源生命周期）

- 解析会话（`resolve.rs` 的 `ResolveSessions`）存于内存：下载直链取完后必须释放会话资源；文件夹收集的转存文件在下载完成后由 `cleanupId` 延迟清理（夸克）。
- 清理必须 best-effort 且记日志：清理失败不阻塞用户主流程，但要可追踪（日志页可见）。
- 应用退出：托盘常驻时引擎随主进程；`minimizeToTray=false` 时关闭窗口即退出并释放引擎。

## 副作用清理（前端）

- 事件订阅（`onSettingsUpdated` / `onDownloadsUpdated` / …）一律在 effect 返回函数中 `unlisten`。
- 定时器 / 通知（`window.setTimeout` 的提示消失）在组件卸载或状态重置时清理。
- `useTheme` / `useAppearanceSaver` 的 DOM 副作用（class / CSS 变量 / dataset）是幂等写入，无需手动清理，但新增类似副作用必须保证「最后写入者胜」。
