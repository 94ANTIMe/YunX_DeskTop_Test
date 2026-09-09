# 版本与发布

> 仅在准备发布时阅读并执行；**日常修改不升版本号**。签名密钥更换、渠道迁移属重大变更，先补 ADR 再动手。

## 版本号同步（三处一致，仅发布时统一升版）

1. `package.json` 的 `version`
2. `src-tauri/tauri.conf.json` 的 `version`
3. `src-tauri/Cargo.toml` 的 package `version`

发布 tag 形如 `v0.5.1`，与上述版本一致（`v` 前缀）。

## 发布流程

1. 确认 `main` 分支全量检查通过（`pnpm test` / `pnpm build` / `cargo test` / `cargo check` / `git diff --check`）。
2. 三处版本号统一升版，单独一个 commit（如 `chore: bump version to 0.5.2`）。
3. 打 tag 推送：`git tag v0.5.x && git push origin main --tags` → 触发 `.github/workflows/release.yml`。
4. 构建产物：NSIS（`x64-setup.exe`）+ MSI + 更新器工件（`latest.json` 与签名文件）自动上传 GitHub Releases。

## 签名更新器（不可跳过）

- 应用内更新基于 `tauri-plugin-updater`，端点按序尝试：GitHub Releases 的 `latest.json` → GitCode 镜像的 `latest.json`（`tauri.conf.json` → `plugins.updater`），任一成功即用；安装模式 `passive`（进度条 + 自动重启），Windows 下 NSIS `currentUser` 静默覆盖。
- CI 强制校验 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 两个 Secrets，缺失即失败——这是有意的保护，不要绕过。
- 私钥与密码只存 GitHub Secrets；本地不落盘、不进日志、不进提交。换钥 = 全量重新发布并补 ADR。
- README 的更新描述必须与 `tauri.conf.json` 实际端点一致（历史上曾写过多源测速后与实现脱节，现双源已落地：GitCode 镜像的 latest.json 由 CI 在发布时把平台 URL 改写为 GitCode 后上传）。

## 渠道职责

- **GitHub Releases**：更新器首选源（`latest.json` + 安装包 + 签名）。
- **GitCode 镜像**：更新器回退源 + 手动下载渠道。CI 在发布时上传安装包 / MSI，并把 `latest.json` 的平台 URL 改写为 GitCode 后上传（`release.yml` GitCode 步骤第 3 段）；默认仓库名由 `vars.GITCODE_REPOSITORY` 控制，与 `tauri.conf.json` 里硬编码的回退端点需保持一致（改仓库名时两处同步）。镜像内容与 GitHub 严格一致，不单独修包。
- 已知边界：GitCode 镜像依赖 `GITCODE_TOKEN` Secret；Token 缺失或该步骤失败时，更新器仍走 GitHub，不影响发布。

## 发布检查清单

- [ ] 三处版本号一致且与 tag 一致
- [ ] 全量检查通过（命令见 [testing.md](testing.md)）
- [ ] CI 绿：签名校验、NSIS + MSI 构建、更新器工件生成
- [ ] GitHub Release 页 `latest.json` 可被旧版本拉到（更新链路自测：从上一版点「检查更新」）
- [ ] GitCode 镜像同步
- [ ] 如有行为/接口变更：`docs/project/` 与 README 已同步
