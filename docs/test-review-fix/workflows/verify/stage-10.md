# Stage 10 逐项验证断言（唯一真值源）

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-CI-01**：`.claude/test-inventory.md` 表头总数 = L1/L2/L3/L4 四段头之和（机械核对，取当时实跑数）；含「登记纪律（TQ-CI-01）」小节（grep 命中）。
- **TQ-CI-02**：inventory L2 段头 = 20 段小计之和 = `npm test` 实跑数（机械核对）；抽查 3 段的段内文件实跑数与段小计一致（vitest 分段实跑为据）。
- **TQ-CI-05**：`src/__tests__/CLAUDE.md` 含「全局 mock 清单」小节（grep 命中）。
- **TQ-COV-02**：inventory 豁免表 `lib.rs run()` 行兜底层级含 `start_signal_watcher_impl` 与 `TQ-COV-02`（grep 命中）。
- **TQ-E-10**：inventory L3 段含「职责边界（TQ-E-10）」小节（grep 命中）。
- **TQ-L1-02**：inventory 含「条件跳过用例」小节（grep 命中）。
- **TQ-L1-04**：`src-tauri/src/pty/CLAUDE.md` 含「Mutex 中毒分支无回归用例」登记（grep 命中）。
- **TQ-L1-06**：`src-tauri/src/pty/CLAUDE.md` 含「vendor 升级大小判定假设」登记（grep 命中）。
- **SEC-17 豁免更新**：inventory 豁免表 SEC-17 行改为「已由 L1 tracing-test 断言锁死（TQ-COV-05）」或删除该行（grep 命中）。
- **模块 CLAUDE.md 同步**：`src/features/explorer/CLAUDE.md`（tree-node-row / explorer-inline-input testid 句）、`src/features/fileViewers/CLAUDE.md`（registerDefaultViewers 句）、`src/panels/CLAUDE.md`（oscHandlers / keyEventHandler 句）、`e2e-tests/CLAUDE.md`（恢复失败非零退出 + beforeSuite resetSettings + 观察面句）、`src-tauri/src/git/CLAUDE.md`（函数覆盖口径句）、`src-tauri/src/hooks/CLAUDE.md`（SEC-17 更新句）——逐文件 grep 对应关键字命中。
- **收尾验收**：全量 7 门禁命令通过；coverage 复测前端行 ≥ 94.5%、Rust 行 ≥ 90%（或未达标重点文件已逐条登记豁免，grep inventory 登记句命中）；coverage 对照数字写入 commit body 或收尾说明（Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
7. `npm run e2e`
