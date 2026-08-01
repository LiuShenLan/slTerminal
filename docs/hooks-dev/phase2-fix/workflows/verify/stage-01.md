# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 行号引用为修复前快照（checklist 实证 2026-07-28），修复后可能漂移——以符号名定位为准。

## 断言清单

- **V1（PF2-FE-01）**：`src/panels/terminal/TabTitleRegistry.ts` 的 `match(command)` 先取首 token（`command.trim().split(/\s+/)[0]`）再查表——Read 源码确认。语义验证：`match("claude --resume x")` 命中、`match("  claude")` 命中（trim 生效）、`match("claudeX")` 不命中（首 token 精确匹配，非前缀）。调用方 `useCommandDetection.ts` 传完整命令行、不改（match 内部消化首 token）。
- **V2（PF2-FE-02）**：`src/panels/terminal/TerminalRegistry.ts` 满足契约 1——① `RegisteredTerminal.claudeSession` 为**可选**字段（既有 stub 工厂 `src/__tests__/terminal-registry.test.ts:13-27` 四字段字面量不含 claudeSession 仍编译通过，tsc 全绿即证）；② `setClaudeSession` 五分支 Read 源码逐条确认：merge（部分键更新保留其余）/ `null` 清空 / panelId 不存在 no-op 不 notify / 缺 `lastEventAt` 自动填 `Date.now()` / `undefined` 键不覆盖旧值；③ `ClaudeSessionInfo` 无 running 布尔（二态模型，存在即运行中）。
- **V3（PF2-FE-02）**：`RegistryEvent.type` 含 `"sessionChange"`；notify payload 不带 session 数据（语义式：payload 仅 `{ type, panelId }` 裸结构，不限写法——listener 经 `get()` 读现值）；`register` 幂等覆盖时调用方未传 claudeSession 字段则保留旧值（Read register 实现确认）。
- **V4（PF2-FE-03）**：`src/panels/terminal/useCommandDetection.ts` 签名含 `panelId`；OSC 133 C 且 rule 命中路径调 `setClaudeSession(panelId, { matchedCommand: rule.command })`；OSC 133 D 且 `isCommandRunningRef.current === true` 路径调 `setClaudeSession(panelId, null)`；调用点 `src/panels/terminal/useXterm.ts`（原 :205）传入 panelId。
- **V5（PF2-FE-04）**：`src/panels/terminal/useXterm.ts` hook 事件订阅（原 :349-357）追加 session 写入——SessionEnd/Exit → `setClaudeSession(panelId, null)`；其他事件 → merge `{ transcriptPath: payload.transcriptPath ?? undefined }`（`?? undefined` 或等价空值转换，不限写法——null 不覆盖旧 transcriptPath）；eventToStatus/onTabStateChange 现有链路不动。
- **V6（PF2-FE-05）**：`src/features/agentStatus/useAgentStatus.ts` 行生命周期与契约 5 逐条对应（Read 源码确认，语义式不限函数名）——① 建行双通道：sessionChange（经 `get()` 读 session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）；② 删行三通道：sessionChange（session null）∨ SessionEnd/Exit ∨ remove；③ 初始扫描遍历 `getAll()` 只建 claudeSession 非 null 的行，且行携 transcriptPath 时主动调 `contextUsage` 拉取一次；④ registry subscribe effect deps 为 `[]`（语义式：deps 不含 rows/projectPageIds 等易变值，listener 经 ref 读最新状态——remove 事件不丢失）；⑤ 存在 reconcile 对账路径（初始扫描/事件处理时行在 registry 中不存在或 session 为 null → 移除）；⑥ `AgentSessionRow.usage` 类型引用 `ContextUsage`（import from `src/types/hooks`，不再内联两字面量类型）。
- **V7（PF2-FE-06）**：`src/features/agentStatus/AgentStatusView.tsx`（原 :94）空态文案 =「当前项目无运行中的 claude 会话」（grep 命中）。
- **V8（PF2-FE-07）**：`src/features/agentStatus/useAgentStatus.ts` usage 拉取（原 :169-171）的 catch 内已补 `console.error`（grep 命中该 catch 块）。
- **V9（PF2-TE-01/02/03/05/08）**：测试重写完成——① `src/__tests__/tab-rules.test.ts` 无 `match("claude update")).toBeNull()` 旧断言（grep 零命中），存在「带参命令行按首 token 命中」语义用例；② `src/__tests__/agent-status-hook.test.ts` 覆盖：纯 shell 无行 / sessionChange 建行 / hook 事件建行 / SessionEnd 删行 / sessionChange(null) 删行 / remove 删行 / 初始扫描只建活会话 / 初始扫描携 transcriptPath 主动拉 usage / reconcile 对账（Read 测试文件逐条点名）；③ `src/__tests__/terminal-registry.test.ts` + `terminal-registry-subscribe.test.ts` 含 setClaudeSession 五分支 + sessionChange 裸结构 + register 保留旧 session 用例；④ `src/__tests__/use-xterm-lifecycle.test.ts` 含 OSC 133 C/D 与 hook 事件的 setClaudeSession 写入断言。
- **V10（门禁）**：全量测试三命令全绿（见下）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
