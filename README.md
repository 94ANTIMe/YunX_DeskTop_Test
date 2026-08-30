# 云析 YunX Desktop

网盘分享链接解析与高速下载工具（桌面版）。粘贴分享链接，自动识别平台与提取码，多线程分片高速下载，支持多平台账号登录、网盘聚合搜索。

> 云析 Android 版同源项目：[CYQawa/YunX](https://github.com/CYQawa/YunX)

## ✨ 功能特性

- **链接解析**：自动识别平台与提取码，解析分享目录、文件夹递归下载
- **六大平台**：夸克、UC、迅雷、百度网盘、123 云盘、139（和彩云）
- **高速下载**：aria2 分片并发下载，断点续传、限速、失败重试
- **网盘聚合搜索**：对接自部署 PanSou 服务，全网搜索公开分享资源并一键解析下载
- **多平台账号登录**：WebView 扫码 / 账号密码 / 短信验证，Cookie 与 Token 持久化
- **任务管理**：每个下载任务独立管理（暂停 / 继续 / 删除 / 打开位置），一键清空全部记录
- **收藏夹与解析历史**：收藏常用链接，解析记录可回看、再解析、删除
- **日志系统**：每一步操作实时记录，便于排查

## 🖥️ 界面

暖色编辑风（象牙 / 黏土橙 / 近黑），顶部悬浮胶囊导航，六个栏目：

| 栏目 | 说明 |
| --- | --- |
| 解析 | 粘贴链接 → 建会话 → 文件树导航 → 取链下载 / 收藏 |
| 网盘 | 多平台账号登录与文件管理 |
| 搜索 | PanSou 聚合搜索 → 结果一键转入解析 |
| 下载 | aria2 任务实时列表与单任务管理 |
| 日志 | 解析 / 取链 / 下载全链路日志 |
| 设置 | 下载目录、并发、分片、限速、PanSou 地址等 |

## 🛠️ 技术栈

- **桌面框架**：[Tauri 2](https://tauri.app/)（Rust 后端 + WebView2）
- **前端**：React 19 · TypeScript · Vite · Tailwind CSS v4
- **后端**：Rust（平台 API 封装、解析编排、WebView 登录、SQLite 持久化）
- **下载引擎**：[aria2](https://github.com/aria2/aria2) sidecar（JSON-RPC 管理）
- **数据存储**：SQLite（`%APPDATA%\com.yunx.desktop\yunx.db`）

## 📦 环境要求

- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/)（stable，含 MSVC toolchain）
- Windows（Tauri 2 + WebView2；安装时自动引导 WebView2）

## 🚀 开发与构建

```bash
# 安装依赖
pnpm install

# 启动开发模式（热更新）
pnpm tauri dev

# 打包（NSIS 安装包 + MSI）
pnpm tauri build
```

打包产物位于 `src-tauri/target/release/bundle/`。

## ⚙️ PanSou 搜索（可选）

在「设置」页填入自部署 PanSou 服务根地址（如 `http://192.168.1.100:8888`），即可在「搜索」页启用聚合搜索；留空则关闭。

## 🤝 开源致谢

本项目依赖 / 参考了以下开源项目，谨此致谢；各项目版权归其作者所有。

- [aria2](https://github.com/aria2/aria2) — 多线程高速下载引擎（sidecar）
- [PanSou](https://github.com/fish2018/pansou) — 网盘聚合搜索 API 服务（自部署对接）
- [TurboDL](https://github.com/henrique-coder/turbodl) — 多线程分片下载优化参考
- [YunX](https://github.com/CYQawa/YunX) — 云析 Android 版（同源项目）

## 📄 License

[GPL-2.0](LICENSE)
