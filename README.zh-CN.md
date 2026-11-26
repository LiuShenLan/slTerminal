# slTerminal

面向 Windows 原生、为 AI 编码 CLI 调优的终端模拟器。

slTerminal 是单进程 GPUI 应用：单窗口单实例、仅暗色模式、GPU 加速渲染。AI CLI 直接跑在 Windows 上（不走 WSL），所有主流 AI CLI 均为一等公民。

## 功能

- **原生终端核心** — VT 解析、ConPTY、有界滚动回看与只读搜索、选择、复制/粘贴守卫、Shell 集成、补全，以及 Win10 的新版 ConPTY 捆绑运行时。
- **项目与工作区** — 单窗口单实例；项目切换保留运行中的终端；面板组、分隔线、布局与逐视图状态按稳定 `ViewKey` 恢复。
- **共享文档** — 同一真实文件跨项目只有一份实时 Document；Editor、Markdown 与 Diff 共享文本、撤销、脏状态与外部冲突；自动保存与崩溃恢复支持显式重授权。
- **侧栏** — 活动栏、带计划式文件操作与回收站删除的文件浏览器、Commit 视图与 Agent 全局文件根。
- **AI CLI 集成** — Claude Code、Codex、OpenCode、Kimi、Pi 与 OhMyPi 的 hook 安装与状态信号；终端画面状态兜底与 Agent 能力范围。
- **外观与设置** — 仅暗色；内置与自定义纯颜色主题（默认 Linear）、窗口材质、字体与本地背景图；中英双语界面。
- **Runtime 控制面** — 仅 loopback 的 Runtime v2：发现、快照、项目激活事件与面板/终端操作；token 只存 Windows 凭据管理器。
- **安全** — PathPolicy 授权、设置写审计、严格白名单的本地加密备份。
- **打包** — 本地可复现发布链：Inno Setup 安装器、便携 zip、SHA-256 校验与体积预算钉测试。

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
python scripts/check_doc_policy.py full
python scripts/check_traceability.py
python scripts/check_brand_regress.py staged
```

定向测试必须使用显式 `lib_tests` target；`--lib` 或裸 filter 可能绕过测试 manifest 并以零输出崩溃。见 `.claude/docs/testing/strategy.md`。

## 文档

- `.claude/CLAUDE.md` — 项目规则与开发约束（模块细节按最近目录的 `CLAUDE.md` 渐进加载）。
- `CONTEXT.md` — 领域术语表。
- `.claude/docs/architecture/overview.md` — 架构、依赖与 ownership 边界。
- `.claude/docs/testing/strategy.md` 与 `.claude/docs/testing/exemptions.md` — 测试策略与豁免登记。
- `.claude/docs/release/packaging.md` — 发布链、体积预算与合规。
- `.claude/docs/operations/commands.md` — 完整命令矩阵。
- `release-notes/` — 按版本的发布说明。
- `CHANGELOG.md` — 双语变更日志，随安装器与便携包内置。

## 许可证

GPL-3.0。见 `LICENSE` 与 `THIRD-PARTY-NOTICES`。
