# 云析 YunX Desktop

网盘分享链接解析与高速下载工具（Windows 桌面版）。粘贴分享链接，自动识别平台与提取码，多线程分片高速下载，支持多平台账号多账号切换、网盘聚合搜索、代理与凭据加密。

> 云析 Android 版同源项目：[CYQawa/YunX](https://github.com/CYQawa/YunX)

## ✨ 功能特性

- **链接解析**：自动识别平台与提取码，支持解析分享目录、**文件夹递归下载（还原目录结构）**、**懒加载目录树**
- **六大平台**：夸克、UC、迅雷、百度网盘、123 云盘、139（和彩云）
- **多账号管理**：每个平台可登录多个账号并一键切换，凭据以 **Windows DPAPI 加密** 持久化
- **高速下载**：aria2 分片并发下载，断点续传、限速、失败重试、代理支持
- **网盘聚合搜索**：对接自部署 PanSou 服务，搜索公开分享资源并一键转入解析
- **下载任务管理**：任务级控制（暂停 / 继续 / 删除 / 打开位置、一键清空），Dashboard 明细视图与速度监控
- **任务全貌**：并发数、分片数、限速、下载目录、每服务器连接数等均可配置
- **剪贴板监听**：自动识别复制的分享链接并提示解析（可在设置开关）
- **托盘常驻**：最小化到托盘后台继续下载，下载完成系统通知，可开机自启
- **收藏夹与解析历史**：收藏常用链接，解析记录可回看、再解析、删除
- **在线更新**：GitHub + GitCode 双源测速择优下载，静默安装覆盖并自动重启
- **日志系统**：解析 / 取链 / 下载全链路实时记录，便于排查

## 🛠️ 技术栈

- **桌面框架**：[Tauri 2](https://tauri.app/)（Rust 后端 + WebView2）
- **前端**：React 19 · TypeScript · Vite · Tailwind CSS v4
- **后端**：Rust（6 平台 API 封装、解析编排、WebView 登录、SQLite 持久化、DPAPI 凭据加密）
- **下载引擎**：[aria2](https://github.com/aria2/aria2) sidecar（JSON-RPC 管理、支持代理）
- **数据存储**：SQLite（`%APPDATA%\com.yunx.desktop\yunx.db`）

## 📦 环境要求

- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/)（stable，含 MSVC toolchain）
- Windows（Tauri 2 + WebView2；安装时自动引导 WebView2）

## ⚙️ PanSou 搜索（可选）

在「设置」页填入自部署 PanSou 服务根地址（如 `https://so.252035.xyz`）或本地地址 `http://192.168.1.100:8888`，即可在「搜索」页启用聚合搜索；留空则关闭。

## 🤝 开源致谢

本项目依赖 / 参考了以下开源项目，谨此致谢；各项目版权归其作者所有。

- [aria2](https://github.com/aria2/aria2) — 多线程高速下载引擎（sidecar）
- [PanSou](https://github.com/fish2018/pansou) — 网盘聚合搜索 API 服务（自部署对接）
- [TurboDL](https://github.com/henrique-coder/turbodl) — 多线程分片下载优化参考
- [YunX](https://github.com/CYQawa/YunX) — 云析 Android 版（同源项目）

## ⚠️ License Notice / 协议声明

本项目基于 [CYQawa/YunX](https://github.com/CYQawa/YunX) 进行二次开发，严格继承并遵循原作者的 **AGPL-3.0** 开源协议。

任何基于本项目的衍生作品，在网络服务或分发时，均必须遵守 AGPL-3.0 的开源义务。

## 免责声明

本项目仅供个人学习与技术交流，请勿用于商业用途。下载内容版权归原作者所有，请在下载后 24 小时内删除。使用本项目产生的任何后果由使用者自行承担。

## 📄 License

[AGPL-3.0](LICENSE)
