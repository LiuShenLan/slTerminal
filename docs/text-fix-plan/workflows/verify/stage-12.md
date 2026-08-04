# Stage 12 逐项验证断言（唯一真值源）

> stage-12 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **STS-01**：colors.test.ts 中 `expect(expected).toMatch(HEX6_RE)` 式自断言零残留（grep 确认五组 GIT_FILE/GIT_GUTTER/EXPLORER/SIDEBAR/AGENT_STATUS_USAGE 全部改读真实导出值 `expect(X_COLORS[key]).toBe(expected)`）
- **STS-02**：global-commands 用例名与断言一致（真调 handler 断言行为，或改名"factory 在 getter 抛异常时仍能创建命令对象"——二选一，Read 确认无"名 handler 不传播异常"实只 toBeDefined）
- **STS-03**：shortcuts 测试含 forceContext 反向注册顺序用例（global 在前、terminal 在后 + `forceContext="terminal"`，覆盖 aForced=0/bForced=1 方向）
- **STS-04**：claudeStatus 测试含 `getStatusIcon(null)===""` 与 `getStatusIcon("working")==="⚡"`
- **STS-05**：theme.test.ts 含 `terminalOptions.vtExtensions?.kittyKeyboard === true` 断言
- **STS-06**：projects/font-size/keybindings 三 store 测试 afterEach 含 `cancelPendingSave()` 或 `vi.runOnlyPendingTimers()`+`vi.clearAllTimers()` 等价清理（Read 确认无残留 timer 风险）
- **STS-07**：projects codify 用例有注释"已知当前行为（无影响操作仍 bump version），非强契约"（Read 确认注释存在）
- **STS-08**：commandFromMeta 参数化遍历全 9 命令（统一断言 id/context/defaultKey/handler；grep EXPECTED_IDS 循环或等价）
- **STS-09**：EXPLORER_SELECTION_BG/HTML_PANEL_LOADING_FG/HTML_PANEL_IFRAME_BG 已入 uiTokenCases（配合 STS-01 真实值断言）
- **STS-10**：①renamePage 不存在 projectId 用例存在（状态不变、version 不变）；②"markPersistenceReady 应允许后续 save"用例补实际 save 断言或改名
- **STS-11**：①inject-script 时间断言（`elapsed < 500ms` 或等价）零残留（grep 确认，结果断言保留）；②projects 状态不变断言改 `structuredClone` 深拷贝快照比对（Read 确认无同引用快照）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
