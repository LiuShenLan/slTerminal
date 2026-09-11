# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-01**：`rg "网络/HTTP/解析" scripts/check-ts7-trigger.mjs` 1 命中；`rg "网络/解析" scripts/check-ts7-trigger.mjs` 仅剩含 HTTP 的同一行（Read 确认标签已覆盖三类失败语义）。
- **TE-02**：`rg "exclude" vitest.config.ts` 零命中（Read 确认 include 行保留为 `src/__tests__/**/*.test.{ts,tsx}`）。
- **TE-03**：`rg "StringDecoder" e2e-tests/run-wdio.cjs` 命中；Read wireWarnCounting 确认：tail 残串变量在函数闭包内（语义式——每流独立，禁模块级共享，该函数被 stdout/stderr 调两次）；匹配输入为 `tail + s` 拼接串且残串截取长度 = 模式长度-1；转发写 dst 的是本 chunk 解码文本（不含 tail 前缀重复输出）。
- **TE-04**：`rg "process.exit\(code \?\? 1\)" e2e-tests/run-wdio.cjs` 2 命中（runWdio 与 fallback 两通道）；`rg "process.exit\(code\)" e2e-tests/run-wdio.cjs` 零命中。
- **FE-06**：`rg "compromises-fix-review-fix/\*\*" knip.json` 零命中；`rg "compromises-fix-review-fix/workflows" knip.json` 与 `rg "compromises-fix-review2-fix/workflows" knip.json` 各 1 命中；`npx knip --production` exit 0（测试 agent 结果承载）。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `node --check scripts/check-ts7-trigger.mjs`
4. `node --check e2e-tests/run-wdio.cjs`
5. `npm test`（用例计数 = 3261 不缩水——TE-02 删 exclude 不得吞用例）
6. `npx knip --production`
