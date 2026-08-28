# 收尾报告（conda-profile-fix / B17）

2026-08-29 执行完毕。计划与编排产物见本目录（checklist.md / stages.md / execution-plan.md / workflows/）。

## Stage commit 列表

| Stage | commit | 内容 |
|-------|--------|------|
| 01 | `50b061a` | `fix(pty): 移除 spawn PowerShell 的 -NoProfile——恢复用户 profile 加载，修复 conda activate 失效（B17）`（src-tauri/src/pty/shell.rs，+38/-5） |
| 02 | `fd2f682` | `docs(pty): B17 文档同步——profile 加载红线 + 编号登记 + 用例清单/豁免登记`（3 个 md，+9/-6） |

修复循环 0 轮（两 Stage 均一次通过，verify allFixed=true）。

## 最终验证结果

| 层级 | 命令 | 结果 |
|------|------|------|
| 静态 | `cargo clippy -D warnings` / `cargo fmt --check` / `tsc --noEmit` / `eslint src/` | 全绿 |
| L1 | `cargo test --test-threads=1` | **809 passed** 全绿（含新增 `test_pwsh_args_no_noprofile_b17`） |
| L2 | `npm test` | 2755 用例——首跑 1 例 wait-for 超时（flaky），重跑全绿；本变更零前端改动，与 flaky 无关 |
| L3 | `npm run test:l3` | 142/142 全绿 |
| L4 | `npm run e2e`（build:e2e + wdio） | 39/40——唯一失败为 `editor.e2e.ts` dirty→clean 用例，系豁免表已登记的 Windows notify 环境级故障（2026-08-23 实证，非代码缺陷），与本变更无关 |

最终用例数：全量 **3746**（Rust 809 + 前端 2755 + L3 142 + E2E 40），test-inventory.md 已同步（三处计数一致，实跑双核对）。

## 未修复项

无。

## 根因与修复摘要

- **根因**：`shell.rs` 的 `build_pwsh_command`/`build_pwsh_info` 固定注入 `-NoProfile` → 用户 profile（conda init 钩子）不加载 → `conda` 落到 PATH：win11 命中 `Scripts\conda.exe` 报 CondaError；win10 命中 `Library\bin\conda.bat`，激活在 cmd 子进程内完成随退出蒸发（零输出 exit 0）
- **修复**：删两处 `-NoProfile`，profile 原生加载（先于 -EncodedCommand 执行，OSC prompt 包装链顺序不变）
- **决策留痕**：`e2e-tests/terminal.e2e.ts:412` 的 `-NoProfile` 不动（一次性辅助命令）；不加设置开关（YAGNI）

## 人工验证（MANUAL-B17，交付用户实测）

自动化全部通过，以下依赖真实 conda/miniforge 环境 + 交互会话，已登记豁免（test-inventory.md 豁免表 B17 行）：

1. **win11 本机**：`npx tauri build --debug --no-bundle` → 启动 slTerminal → 新终端页签 → `conda activate claude` → 提示符出现 `(claude)` 前缀；`python --version` 正常
2. **win10 部署机（miniforge）**：同法验证前缀 + `python` 可启动
3. **冒烟**：切目录后 OSC cwd 跟踪正常；prompt 无转义序列泄漏；`claude` 启动后页签标题/图标切换正常
