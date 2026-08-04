# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **TRM-01**：lifecycle 与 output 两文件间重复用例消除（同名/同断言用例只存一处，Read 抽查 cancelPendingFlush 与 ResizeObserver 合帧类）；`await Promise.resolve()` 时序已改 fake timers 或显式 flush helper（Read 抽查无裸 `await Promise.resolve()` 等待 spawn 注册）
- **TRM-02**：`setBufferType` 在测试与 `xterm-test-utils.ts` 中零残留（grep 零命中）；交替缓冲行为改由 resize/fit 链路断言（Read 确认替代用例存在）
- **TRM-03**：`use-xterm-output.test.ts`、`e2e-gating-terminal.test.ts` 的 mock 中 `hooks:` 字段零残留（grep `hooks:` 于两文件 mock 区零命中；目标模块未导出字段不存在于 mock）
- **TRM-04**：usePtyOutput 64KB 淘汰（恰好/超过/多块）、退出码透传（含 0 与非空数字）、E2E 缓冲行数截断各有用例
- **TRM-05**：TerminalPanel 1.5s 超时隐藏遮罩（fake timers）、`handleTabStateChange` active=false 恢复原标题、`windowsPty` 更新各有用例
- **TRM-06**：webgl `setupWebglWithRetry` 全分支有用例（fake timers：context loss 指数退避序列、重试耗尽回退 DOM、`cancel()` 清定时器）
- **TRM-07**：useTerminalInstance 四分支各有用例（fonts.ready catch、fontSize undefined、prevFontSize 相同跳过、webglAddon 已存在不重复加载）
- **TRM-08**：terminal-registry 测试含 `getAll` 只读视图 / `_size` 计数 / `_dump` 不抛（或 JSDoc `@internal` 标注，二选一，Read 确认）
- **NAH-02**：`setClaudeSession` merge 语义三条用例（增量 `{status}` 保留 transcriptPath + lastEventAt 更新、undefined 字段不覆盖、null 清空）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
