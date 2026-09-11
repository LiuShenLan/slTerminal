# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-02**：Read `src/features/explorer/useFileTree.ts` restoreExpanded 无快照分支确认：`restoringRef.current = false;` 保留且**无** `commitViewState()` 随行（语义式——提交统一由 rootNodes 渲染落定后的 commit effect 承担）；Read 恢复队列耗尽分支确认 commitViewState 保留（恢复窗口最终提交兜底，语义不同不动）；Read 首帧失败 catch 分支确认无新增 commitViewState。
- **FE-01**：`rg "toHaveBeenCalledTimes\(1\)" src/__tests__/use-file-tree.test.ts` ≥ 1 命中；Read 确认该断言位于 FE-01-2 用例（无快照场景首帧落地）的 waitFor 内。
- **FE-03**：`rg "suppressGuard" src/features/explorer/useFileTree.ts` ≥ 2 命中（setTimeout 创建 + cleanup clearTimeout）；Read 确认：兜底回调内含 gen 校验 + restoringRef 检查 + commitViewState 上呼 + console.warn；effect deps 含 commitViewState；超时值 10_000。
- **FE-04**：Read :496 区注释确认解除点列举含「loadRoot 首帧失败 catch」与「超时兜底」（5 点列举完整）。
- **FE-02/03（文档面）**：Read src/features/explorer/CLAUDE.md「提交时机」句确认含「无快照分支不显式 commitViewState」与「抑制兜底 = 10s 超时（FE-03）」两口径。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npx vitest run use-file-tree`
4. `npm test`
