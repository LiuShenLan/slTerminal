# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：`src/types/` 为生成物，只改 Rust 侧 derive 与导出测试，不手改生成物。

## 断言清单

- **CP-024-a（生成物落位）**：`ls src/types` = 9 域文件（pty/fs/git/notify/agent/agentHistory/hooksConfig/backgroundTasks/planBalance）+ index.ts + local.ts + hooksConfigGui.ts + CLAUDE.md；`grep -rliE "ts-rs|generated" src/types/*.ts | wc -l` = 9；9 域文件均含生成头注（Read 抽查 pty.ts/hooksConfig.ts/agentHistory.ts 三件）。
- **CP-024-b（消费面零改动）**：`grep -rnE 'from "(\.\./)+types(/[a-zA-Z]+)?"' src --include="*.ts" --include="*.tsx" | wc -l` = 74（58 文件）；`grep -rn "from .*types/hooksConfig\"" src | grep -i gui` 零命中（GUI 类型改指 hooksConfigGui）。
- **CP-024-c（导出测试与漂移守卫）**：`cargo test export_bindings -- --test-threads=1` 退出码 0；随后 `git status --porcelain -- src/types` 输出为空（生成物与仓内一致）。
- **CP-024-d（HookHandler 扩面）**：`src-tauri/src/hooks/claude/config.rs` 的 `HookHandler` 为 C13-3 全 16 字段矩阵（Read 逐字段核对：command/args/async/async_rewake/shell/url/headers/allowed_env_vars/server/tool/input/prompt/model/continue_on_block/if/timeout/status_message，全部 Option + serde default）；L1 `hook_handler_full_matrix_roundtrip` 用例存在（随全量覆盖）；SEC-05 type/command 校验语义不变（Read 确认校验层未动）。
- **CP-024-e（文档口径）**：`src/types/CLAUDE.md` 双边对应节已改写为「单源化（ts-rs）」口径（Read 确认：Rust derive 为唯一真源、9 域文件禁手改、刷新指令 = `cargo test export_bindings`）；根 CLAUDE.md 硬约束 #4 已改写；`src/ipc/CLAUDE.md` mockIPC 红线补「DTO 形状真值源 = Rust ts-rs 生成」。
- **CP-024-f（残面收口）**：`src/types/local.ts` 存在且含 TitleSource/BACKGROUND_TASK_IDS 等手写残面（Read 确认）；`src/types/hooksConfigGui.ts` 存在且头注「前端 GUI 模型，非后端 DTO」。

## 全量测试（全部通过为门禁）

1. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
4. `npx tsc --noEmit`
5. `npx eslint src/`
6. `npm test`
7. `npm run test:l3`
8. L4 抽验：`node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts`
