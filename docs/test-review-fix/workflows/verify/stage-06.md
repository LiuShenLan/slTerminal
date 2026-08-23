# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-E-01**：`src/panels/terminal/oscHandlers.ts` 存在且导出 `MAX_OSC52_PAYLOAD`/`registerOsc52`/`registerOsc133`/`makeLinkHandler`（Read 确认签名与 checklist TQ-E-01 步骤 1 一致）；`useClipboardHandler.ts` 不再含 `registerOscHandler(52`（grep -c 为 0）；`useCommandDetection.ts` 不再含 `registerOscHandler(133`（grep -c 为 0）；`useXterm.ts` linkHandler 改经 `makeLinkHandler`（grep 命中）；`test/terminal/production-osc.test.ts` import 生产 `oscHandlers`（grep 命中）且不再含复制的 OSC52/133 handler 体（语义式：文件内无 `term.parser.registerOscHandler(` 直接调用，须 Read 确认注册经生产函数）。
- **TQ-E-01 行为不变（语义式）**：对照 `git diff`，oscHandlers.ts 的 handler 体为原 hook 内联体逐字搬移、仅依赖改参数注入（visibleRef→isVisible、writeText/matchByCommand/setAgentSession/onTabStateChange 注入）——判定逻辑、阈值、返回 true/false 语义零变化（须 Read diff 逐块确认，防顺手改 OSC 语义）。
- **TQ-E-02**：`src/panels/terminal/keyEventHandler.ts` 存在且导出 `handleTerminalKeyEvent`（Read 确认）；`useXterm.ts` 的 `attachCustomKeyEventHandler` 改传该函数（grep 命中）；`test/terminal/shortcut-dispatch.test.ts` 存在且 ≥ 3 用例（grep -c `it(` ≥ 3），含 Ctrl+Shift+C 消费拦截与未注册键透传断言（Read 确认）。
- **人工验证点登记**：`docs/test-review-fix/stages.md` 人工验证点汇总表含 OSC52 剪贴板与 OSC133 状态圆点两条（Read 确认存在——收尾实测依据）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
