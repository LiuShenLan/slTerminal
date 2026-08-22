# slTerminal

面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。

- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）
- 架构：Tauri 2 外壳 + Rust 后端（拥有一切 OS 访问）+ React/TypeScript 前端（xterm.js 终端、CodeMirror 6 编辑器、Dockview 布局），前后端经 IPC 通信

## 构建与开发

| 命令 | 用途 |
|------|------|
| `npm run tauri dev` | 开发模式运行（仅保留为开发兜底） |
| `npx tauri build --debug --no-bundle` | 调试构建（exe + dll，**测试/使用固定流程**） |
| `.\\.claude\\package.ps1 -Version "0.1.0"` | 一键打包 release 单文件 exe → zip；加 `-Debug` 用 debug 模式 |

## 测试

四级测试金字塔（完整用例清单见 `.claude/test-inventory.md`）：

| 层级 | 内容 | 命令 |
|------|------|------|
| L1 | Rust 单元/集成 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |
| L2 | 前端单元/集成 | `npm test` |
| L3 | 终端 headless 渲染 | `npm run test:l3` |
| L4 | 端到端 E2E | `npm run e2e`（= `build:e2e` + `wdio`） |

静态检查门禁：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。

## 文档

- `.claude/CLAUDE.md` — 项目根指令：硬性开发约束（#1~#13）、Windows 关键坑、命令与测试策略、模块索引、需求编号索引
- `CONTEXT.md` — 领域术语表（项目/操作页面/面板/会话等概念的定义与同义词避讳）
- `.claude/adr.md` — 架构决策记录（ADR-0001~0009）
- `.claude/test-inventory.md` — 测试用例清单（四级测试全量登记 + 既定豁免清单）
