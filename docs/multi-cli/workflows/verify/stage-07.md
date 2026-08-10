# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **S07-01**（AC-4 夹具）：`src/__tests__/helpers/mockCliProfile.ts` 存在——mockcli profile 全字段（id/displayName/tabTitle = "mockcli"、commands = ["mockcli"]、iconSrc = "/cli-icons/mockcli.png"、hooks + history 全能力桩）+ 注册/清理辅助（Read 确认）；mockcli 仅测试环境注册（语义式：grep `mockcli` 于 `src/` 生产代码——排除 `src/__tests__/` 与 E2E_ENABLED 门控段——零命中；门控段内注册点 Read 确认在 `import.meta.env` 内联门控分支内）
- **S07-02**（AC-4 五点）：`src/__tests__/mock-cli-profile.test.tsx` 五点用例逐点存在且绿（依 npm test + Read 确认每点断言语义）：① OSC 133 命中（matchByCommand → 页签标题/logo/agentSession.cliId = "mockcli"）② eventToStatus 与 classifyNotification 被真实调用（spy 入参断言，useXterm 事件路径与通知调度路径各一）③ 历史聚合 UI mock 条目 + 行 logo 按 session.cliId ④ hub 选择行两枚按钮 + 切换渲染桩编辑器 + selectedCli 持久化恢复 ⑤ pty.write 注入内容 = mock buildRestoreInput 桩输出（可识别前缀）
- **S07-03**（AC-5 守卫）：`src/__tests__/no-claude-literals.test.ts` 存在且绿（依 npm test）；守卫测试自身用 fs 枚举扫描目录（Read 确认非硬编码文件清单——新增文件自动纳入）；扫描目录清单 = 通用层七路径（Read 确认：`src/lib/`、`src/panels/terminal/`、`src/features/agentStatus/`、`src/features/agentHistory/`、`src/features/notifications/`、`src/ipc/`、`src/types/`）；匹配口径正确（Read 确认：值等于 "claude" 的字符串字面量 + 10 个 claude 事件名字符串字面量 + `~/.claude` 路径；import 路径指向 profiles/claude/ 豁免）
- **S07-04**（E2E mock 冒烟）：`__slterm_e2e_registerMockCliProfile` helper 存在且 E2E_ENABLED 内联门控形态未变（grep 命中 + Read 确认 `import.meta.env` 字面量形态）；mock 冒烟用例（注册 mockcli → OSC 133 C 注入 → 页签标题/logo 断言）存在且绿（依 npm run e2e）
- **S07-05**（MC-4 终态）：通用层消费方一律经 profile 注册表取能力（语义式：抽查 `useCommandDetection.ts`、`useXterm.ts`、`useAgentStatus.ts`、`useAgentNotifications.ts`、`AgentStatusRow.tsx`、`HistorySessionRow.tsx`、`restoreSession.ts`、`historyContextMenu.ts`、`HooksConfigPanel.tsx`——能力取值均经 `cliProfileRegistry` / `matchByCommand` / profile.capabilities，无绕过注册表的 claude 硬编码分支）
- **S07-06**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（mock-cli-profile / no-claude-literals / E2E mock 冒烟条目，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2——AC-4 五点与 AC-5 守卫在此层验证）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——mock 冒烟在此层验证；最后单独串行执行，禁与其他命令并行）
