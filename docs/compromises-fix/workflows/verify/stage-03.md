# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 起 L1 定向命令 = `cargo test <filter> -- --test-threads=1`（S02 已拆红线）。

## 断言清单

- **CP-003**：`grep -n "nodejs.org\|https\.get\|require('https')" e2e-tests/run-wdio.cjs` 零命中；`npm pkg get engines` 输出 `{"node":">=22"}`；`npm ls @types/node` 为 @26.x；run-wdio.cjs 文件头注释为 CP-003 新口径（Read 确认：Node >= 22 直跑 + 显式预置不自动下载）；`.claude/test-exemptions.md` 含 run-wdio 启动器豁免行（Read 确认）。**人工验证点**：Node 26 下 `npm run e2e` 全量实跑通过，输出无「下载便携 Node 22」字样。
- **CP-045**：`grep -rn "slterm-e2e-home" e2e-tests/CLAUDE.md e2e-tests/run-wdio.cjs .claude/test-exemptions.md` 所有命中均带 pid/唯一名语义（Read 逐处确认）；`docs/compromises.md` CP-045 已勾选销项并补注记。
- **CP-046**：`grep -n "SETTINGS_SENTINEL_KEYS\|snapSettingsSentinels" e2e-tests/run-wdio.cjs` ≥4 命中；`grep -n "expectFile(" e2e-tests/run-wdio.cjs` 命中 = 1（仅 statuslineBackup）；`grep "哨兵键" e2e-tests/CLAUDE.md` ≥2 命中；**人工负向验证**：向真实屋 settings.json 注入 hooks 键后跑单 spec → 退出码 1 且 stderr 含「哨兵键 "hooks"」（验证后撤销）。
- **CP-029**：`WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec editor.e2e.ts` 连续 3 轮退出码 0；环境出口：`grep "Windows notify 环境级故障" .claude/test-exemptions.md` 零命中且归因行已改写为实测根因（Read 确认）；产品出口：`grep "editor.e2e.ts" .claude/test-exemptions.md` 零命中；探针清理：`e2e-tests/probe-fsevent.e2e.ts` 不存在且 `grep probe e2e-tests/wdio.conf.ts` 零命中；定责结论已落档（commit body 或 test-exemptions 注明两出口之一）。**人工验证点**：定责结论经用户确认。
- **CP-030**：`e2e-tests/wdio.conf.ts` beforeSuite 含 TQ-E-10 `document.hasFocus()` fast-fail 探针（Read 确认）；`e2e-tests/cli-aliases.e2e.ts` 四处真实手势（setValue/click 元素命令）落位且交互时序断言（添加后输入框清空）存在（Read 确认）；`grep -c "setInputValue" e2e-tests/cli-aliases.e2e.ts` = 0。**人工负向验证一次**：Alt-Tab 失焦跑单 spec → beforeSuite 抛错非零退出。
- **CP-028**：`grep -c "aria-expanded" src/features/navTree/NavProjectRow.tsx src/features/navTree/NavPageRow.tsx src/features/navTree/NavHistoryNode.tsx` 各 ≥1；`grep -c "for (let i = 0; i < 6" e2e-tests/agent.e2e.ts e2e-tests/mockcli.e2e.ts` 各 = 0；`src/features/navTree/CLAUDE.md` 数据属性契约节含 aria-expanded 登记（Read 确认）。
- **CP-041**：`.claude/test-exemptions.md` 无 mockcli 两行（`grep -c "mockcli" .claude/test-exemptions.md` = 0）；`grep -n "SLTERM_MOCKCLI_PROJECTS_DIR" e2e-tests/run-wdio.cjs src-tauri/src/agent_history/provider.rs src-tauri/src/agent_history/mock.rs` 各 ≥1；`cargo test mock -- --test-threads=1` 绿；`WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts` 三轮 exit 0（含新增第三 describe 两用例）；生产形态守卫：不设 env 时 `resolve_provider("mockcli")` 仍 Validation（L1 用例锁死，随全量 cargo test 覆盖）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
5. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
6. `npx vite build`（e2e-tests/helpers.ts 等 tsc include 外文件的构建级兜底）
7. `npm run e2e`（本 Stage 为 e2e 基建族，L4 全量为硬门禁；Node 26 直跑）
