# CONTRIBUTING.md — 开发流程入口

开始前先读 [`AGENTS.md`](AGENTS.md)（规范地图与硬性约束）。本文档只讲开发环境与交付流程；代码层面的详细规范在 [`docs/project/`](docs/project/)。

## 环境搭建

- Node.js 18+、pnpm、Rust stable（MSVC toolchain）、Windows 10+（WebView2 随系统分发）
- 首次构建：`pnpm install`，然后 `pnpm tauri dev`（首次会编译 Rust，较慢）
- 下载引擎 aria2 / BaiduPCS-Go 以 sidecar 形式放置于 `src-tauri/binaries/`，无需单独安装

## 日常开发

```bash
pnpm tauri dev     # 桌面调试（热更新）
pnpm test          # 前端单测（vitest）
cargo test         # Rust 单测（在 src-tauri/ 下）
pnpm build         # 交付前验证 tsc + vite 构建
```

## 变更流程（对 AI 与人类同等适用）

1. **需求与验收条件**：先明确要改什么、怎样算完成。没有验收条件的修改不开始。
2. **影响分析**：按 [architecture.md](docs/project/architecture.md) 判断改动落在哪几层；涉及 IPC / settings / 数据库时读 [interfaces-and-compat.md](docs/project/interfaces-and-compat.md)。
3. **实现**：遵守 [AGENTS.md](AGENTS.md) 硬性约束；不与本次需求无关的顺手重构。
4. **回归**：按 [testing.md](docs/project/testing.md) 的风险分级执行并记录命令与结果。
5. **文档同步**：行为变化同步更新对应 `docs/project/` 文档与 README；架构决策补 ADR（[decisions.md](docs/project/decisions.md)）。

## 提交规范

- Commit message 用 conventional commits（`feat:` / `fix:` / `docs:` / `refactor:` / `ci:`），描述可用中文。
- 提交前：`git diff --check` 无空白错误；相关测试通过。
- 修复类提交重点回归「旧数据兼容」；新功能类提交重点补测试与文档。
- 不在功能提交里夹带版本号变更（见 [release.md](docs/project/release.md)）。

## 提交说明模板

交付说明请明确三档验证状态，避免把「未验证」包装成「已验证」：

```text
自动测试：pnpm test（31 通过）、cargo test（12 通过）
人工冒烟：设置页切换 7 套主题 × 明暗，均正常；旧 settings.json 兼容未验证
未验证项：多显示器缩放下的截图行为
```
