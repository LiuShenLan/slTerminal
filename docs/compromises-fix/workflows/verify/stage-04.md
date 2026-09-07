# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> L1 定向命令维持旧形态 = `cargo test --test lib_tests <filter> -- --test-threads=1`（**S02 未拆红线**：embed-manifest 通道实测失实转登记，2026-09-07 执行期翻案）；文中裸 `cargo test <filter>` 断言按此形态执行或以全量（无 filter）覆盖判定。

## 断言清单

- **CP-005**：`rg "std::sync::(Mutex|RwLock)" src-tauri/` 零命中；`rg "锁中毒|poison" src-tauri/src` 零命中（测试名/注释残留即红）；`src-tauri/src/CLAUDE.md` :53 节已重写为 parking_lot 口径（Read 确认）；adr.md ADR-0009 09#14 行已改写（Read 确认）；pty/git/CLAUDE.md 豁免表「Mutex 中毒分支」行已删（grep 零命中）。
- **CP-008**：`rg "is_ignored" src-tauri/src` 零命中；`cargo test git_status -- --test-threads=1` 绿（随全量覆盖）；`src/types/git.ts` 注释值集无 `| ignored`（Read 确认）；`src-tauri/src/git/CLAUDE.md` :21 节末句已改写（Read 确认）。
- **CP-010**：`rg "pty_conpty_status" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 三处全命中；`src/types/pty.ts` 含 `ConptyStatus`（camelCase 三键 attempted/bundled/fallbackReason）；`src/ipc/pty.ts` 含 `getConptyStatus` wrapper；`src/App.tsx` 启动序列有一次性 toast 调用（attempted && !bundled 分支，Read 确认）；`cargo test conpty -- --test-threads=1` 绿。**人工验证点**：Win10 实机删 `%LOCALAPPDATA%\slterminal\conpty\` → 回退 + 启动 toast；Win11 恒静默。
- **CP-034**：`rg "output_ring|ring_buffer_append|RING_BUFFER_CAPACITY|RwLock<Option<Channel|pty_reattach|reattach" src-tauri/src` 零命中；`rg "ring" src-tauri/src/pty/` 零命中（注释残留即红）；pty/CLAUDE.md :58 登记行已删；test-exemptions.md:13 无「channel 锁/ring buffer」措辞（Read 确认）。
- **CP-011**：`rg "随 (PtySession )?Drop 兜底|随 Drop 兜底" src-tauri/src` 零命中；`rg "\.join\(\)" src-tauri/src` 零命中（仅余 join_with_timeout 内部，Read 甄别）；`src-tauri/src/pty/reader.rs` 含 `join_with_timeout` 与 `plan_cleanup_after_join_timeout`（Read 确认）；spawn.rs 含 `CLOSE_PSEUDO_CONSOLE_TIMEOUT` 与 pty-cleaner 监督线程分支（Read 确认）；`.claude/test-exemptions.md` 新增 pty_kill 监督线程豁免行。**人工验证点**：Win10 实机高负载会话 kill，应用不挂起、3s 内 IPC 返回。
- **CP-014**：`cargo test paths_match -- --test-threads=1` 与 `cargo test file_identity -- --test-threads=1` 绿（随全量覆盖）；`grep -n "eq_ignore_ascii_case(&b)" src-tauri/src/pty/shell.rs` 零命中；`src-tauri/Cargo.toml` windows features 含 `Win32_Storage_FileSystem`；`fallback_both_unopenable_rejected` 用例存在（同名同串也拒绝）；adr.md:229 D15 行尾已追加销记。
- **CP-043**：`grep -n "agent_hooks_confirm_inject" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 三处各 1 命中；`grep -rn "warn_if_suspicious_statusline" src-tauri/src/` 零命中；`cargo test suspicious -- --test-threads=1` 与 `cargo test injection_status_roundtrip -- --test-threads=1` 绿；`npx vitest run ipc-agent-hooks-contract settings-hooks-page` 绿（随全量覆盖）；前端确认流内联确认条含 `data-e2e="hooks-confirm-inject"`（Read 确认）；hooks/CLAUDE.md SEC-12 节已改写为确认流口径。

## 全量测试（全部通过为门禁）

1. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
4. `npx tsc --noEmit`
5. `npx eslint src/`
6. `npm test`
