# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TQ-A-01**：`src/__tests__/diff-panel.test.tsx` 脏态弹窗用例（:620-672 区域）中 `rightView.dispatch` 之前存在 `await waitFor(() => expect(getDiffView(container, "diff-right")).toBeTruthy())`（Read 确认）；同文件所有 `getDiffView(...)!` 后紧跟 dispatch 的用例均同样前置 waitFor（语义式：不存在未经 waitFor 确认非空即 dispatch 的调用点，须 Read 逐处确认）。
- **TQ-B-04**：`src/__tests__/explorer-race-cleanup.test.tsx` G3 用例（:131-161 区域）中，初始加载等待改为 `waitFor`/`vi.waitFor` 断言 `rootNodes.length === 1`（Read 确认）；不再存在 `advanceTimersByTimeAsync(0)` 后紧跟 rootNodes 同步断言的形态。
- **TQ-B-06**：`src/__tests__/commit-view-status.test.ts`（:233-294 区域）`resolveOld(...)` 之后的断言经 `vi.waitFor` 轮询（Read 确认）；不存在孤立 `advanceTimersByTimeAsync(10)` 后直接断言 textContent 的形态。
- **TQ-B-09**：`src/__tests__/nav-tree.test.tsx` 三段（:410-452/:655-714/:876-933 附近）与 `nav-tree-history.test.tsx` 对应段（:195-250 附近）中，fireEvent 后的 style.color/backgroundColor/textContent 断言均包在 `waitFor` 内（语义式：不存在 fireEvent 后同一同步块内紧跟样式断言而不经 waitFor 的形态，须 Read 逐段确认）。
- **TQ-B-18**：`src/__tests__/explorer-refresh-preserve.test.tsx` R17 用例（:455-478 区域）gitStatus mock 为可控 resolved 且终态断言经 `waitFor`（Read 确认）。
- **TQ-B-19**：`src/__tests__/keyboard.test.ts` beforeEach 中 `readTextMock` 与 `writeTextMock` 使用 `mockReset()`（grep `mockReset` 命中）且 reset 后补默认 `mockResolvedValue`（Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`

## 补充验证（执行 agent 在门禁通过后补跑并记录结果）

- diff-panel 组合重跑：按 `docs/test-review/02-l2-workspace-panels.md` 复跑段 17 文件清单 `npx vitest run <清单>` 连续 3 轮全绿（TQ-A-01 实证 flaky 的回归标尺）。
