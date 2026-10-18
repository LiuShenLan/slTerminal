# slTerminal

面向 Windows 原生、为 AI 编码 CLI 调优的终端模拟器。

slTerminal 是单进程 GPUI 应用：单窗口单实例、仅暗色模式、GPU 加速渲染。AI CLI 直接跑在 Windows 上（不走 WSL），所有主流 AI CLI 均为一等公民。

## 功能

- **原生终端核心** — VT 解析、ConPTY、滚动回看、选择、复制/粘贴、Shell 集成，以及 Win10 的新版 ConPTY 捆绑回退。
- **标签页与分屏** — 终端标签页、网格分屏、窗格缩放、拖拽停靠、终端标签页会话恢复。
- **AI CLI 集成** — 27 家 AI CLI 的屏幕检测规则、Claude Code / Kimi / Codex / Pi / OpenCode 的 hook 安装、Agent 状态投影、Claude Code 与 Codex 会话历史，以及供 Agent 自发现控制面的 runtime skill。
- **Runtime API** — 本机 loopback 的 JSON Lines 控制面，版本化信封，`slterm ctl` / `slterm env`、资源动词与单实例移交。
- **文件与编辑** — 文件浏览器、源码编辑器、Markdown/图片标签页、只读双栏 diff、git gutter、Markdown 数学渲染与终端数学覆盖层。
- **设置与主题** — 设置中心（应用/外观/配置/交互/高级/本地化/重置 + Agent 组）；八套内置暗色主题（默认 Linear）；中英双语界面。
- **系统集成** — 托盘、Windows toast 通知、任务栏进度、开机启动、窗口材质与壁纸、更新检查与下载。
- **安全** — 凭据只存 Windows 凭据管理器、设置写审计、严格白名单的本地加密备份。
- **打包** — 本地可复现发布链：Inno Setup 安装器、便携 zip、SHA-256 校验与体积预算钉测试。

本版中导航树、Commit、Agent 全局文件三个侧栏视图为已注册占位，仅文件浏览器完整接线。

## 环境要求

- Windows 10 1809（build 17763）或更新版本，x64。
- 由 `rust-toolchain.toml` 钉版的 Rust 工具链（1.97.1）。
- 发布与打包脚本要求 PowerShell 7。
- 构建安装器需要 Inno Setup 6。

## 构建

```powershell
cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook
```

调试产物为 `target/debug/slterm.exe`。从 pwsh/cmd 直接启动 GUI 会同步阻塞父 shell；日常使用请走安装器快捷方式或 `Start-Process slterm`。

发布构建：

```powershell
pwsh -File scripts/release.ps1 -Version 0.3.0
```

产物（安装器、便携 zip、`SHA256SUMS`）落 `dist/`。人工核验步骤见 `scripts/release-checklist.md`。

## 测试

```powershell
cargo test --workspace -- --test-threads=1
cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
python scripts/check_architecture.py
python scripts/check_brand_regress.py staged
```

定向测试必须使用显式 `lib_tests` target；`--lib` 或裸 filter 可能绕过测试 manifest 并以零输出崩溃。见 `.claude/adr.md` ADR-0028。

## 文档

- `.claude/CLAUDE.md` — 项目规则与开发约束（模块细节按最近目录的 `CLAUDE.md` 渐进加载）。
- `CONTEXT.md` — 领域术语表。
- `.claude/adr.md` — 架构决策记录。
- `.claude/test-exemptions.md` — 自动化测试豁免登记。
- `assets/runtime-control-api.md` — 随包 Runtime API 事实文档；机器可读 schema 见 `assets/runtime-api-v1.schema.json`。
- `release-notes/` — 按版本的发布说明。
- `CHANGELOG.md` — 双语变更日志，随安装器与便携包内置。

## 许可证

GPL-3.0。见 `LICENSE` 与 `THIRD-PARTY-NOTICES`。
