# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **S06-01**（MC-502）：选择行 L2 用例存在且绿（依 npm test）——能力过滤（hasConfigEditor=false 的 profile 不出现）/ 按钮 = iconSrc 16×16 logo + displayName / 选中态背景高亮走 theme token（语义式：Read 确认高亮色值来自 theme/colors.ts facade token，非硬编码色值）/ 单 CLI 也渲染选择行 / 点击切换 → 编辑器重挂载且 IPC 携新 cliId
- **S06-02**（MC-503）：selectedCli 持久化 L2 用例存在且绿（依 npm test）——`api.updateParameters({ ...params, selectedCli })` 写入 + **显式** `onLayoutChange(saveLayout(api))` 调用（语义式：Read `HooksConfigPanel.tsx` 确认 updateParameters 后有显式保存调用，不只依赖 updateParameters 自身）；挂载时读 params 恢复；缺省/失效回退首个 hasConfigEditor CLI
- **S06-03**（MC-505）：dirty 守卫 L2 用例存在且绿（依 npm test）——dirty 时切换 `dialog.ask` 确认/取消两分支；非 dirty 直接切换；askGuard 防循环复用（Read 确认）
- **S06-04**（MC-507）：空态 L2 用例存在且绿（依 npm test）——无任何 hasConfigEditor profile → 渲染「无可配置 CLI」占位，不渲染编辑器
- **S06-05**（MC-222/506）：保存提示条文案 = `profile.capabilities?.hooks?.restartHint` 驱动（语义式：Read 确认文案来源，非硬编码字符串）；`data-e2e="hooks-restart-hint"` 选择器保留（grep 命中）；注入状态条三态数据源 = `agent_hooks_injection_status(selectedCliId)`（Read 确认）
- **S06-06**（MC-504/508）：claude 编辑器测试（hooks-config-* 9 文件）在 hub 内全绿（依 npm test）；`features/hooksConfig/`（schema 单点）与 panels/hooksConfig/ 其余文件（configModel/EventTree/GuiMode/HandlerForm/eventsCatalog/matcherEngine/MatcherTester/JsonMode/index）零改动（git diff 或 Read 抽查确认行为零改动）
- **S06-07**（中间态回收）：`src/panels/hooksConfig/` 内 ipc 调用（readHooksConfig/writeHooksConfig/agent_hooks_inject/uninstall/injection_status）的 cliId 实参来自 hub 选中态（语义式：Read 确认实参来源 = selectedCli 状态/props 传递链；`CLAUDE_CLI_ID` 不得作为 ipc 实参出现——grep `CLAUDE_CLI_ID` 于 panels/hooksConfig/ 零命中，或仅出现在与 ipc 实参无关处则 Read 说明）
- **S06-08**（MC-501/D-15）：入口零改动——面板 id `hooksConfig-{pageId}`（grep 命中 `pageApis.ts`）、侧栏菜单流程不变；open-hooks-config-panel / sidebar-actions / default-layout-format 测试绿（预期零改动通过，依 npm test）
- **S06-09**（D-14）：hooks.e2e hub 用例全绿——选择行渲染 / project 层保存写盘 + merge 保留字段经 hub / 注入按钮三态 / `data-e2e="hooks-restart-hint"` 断言（依 npm run e2e）
- **S06-10**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（hooks-config-* hub 改造 + 新增 hub 用例、E2E hub 用例条目，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——hooks.e2e hub 用例在此层验证；最后单独串行执行，禁与其他命令并行）
