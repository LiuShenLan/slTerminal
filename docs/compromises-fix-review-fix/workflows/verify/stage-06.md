# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TE-04**：`rg '"hooks", "statusLine", "env"' e2e-tests/run-wdio.cjs` 命中；Read 哨兵注释确认 env 入哨兵理由 + 误报面口径；`rg "唯一可能写入" e2e-tests/` 零命中（失实声明绝迹）。
- **TE-04（文档面）**：Read `e2e-tests/CLAUDE.md`——「防复发校验」节哨兵键为三键（hooks/statusLine/env）口径；「已知并发误报面」节 env 已移除合法放行、改写为「e2e 运行期间勿动 claude env 配置」限制。
- **TE-05**：`rg "WDIO_WORKER_ID" e2e-tests/wdio.conf.ts` 命中；Read 确认判定形态 = 首 worker（`workerId === undefined || workerId.startsWith("0-")`）fast-fail，其余 console.warn 降级继续（不 throw）；「定向运行官方形态」节已改写（多 spec 降级 warn 但 $ 族 +5s 惩罚登记——Read 确认）。
- **TE-06**：`rg "function writeFakePlanEnv" e2e-tests/` 仅命中 `e2e-tests/node-helpers.ts`；`rg "writeFakePlanEnv" e2e-tests/settings.e2e.ts e2e-tests/background-tasks.e2e.ts` 各命中 import/调用（无本地函数定义）；node-helpers.ts 含 mkdirSync（Read 确认）；两 spec 各自 solo 绿（重构 agent 自验报告承载——verify 抽查报告原文，必要时实跑 `node e2e-tests/run-wdio.cjs --spec background-tasks.e2e.ts` 复核）。
- **TE-06（文档面）**：`e2e-tests/CLAUDE.md`「E2E helper 命名与挂载位置」节含 node-helpers.ts 分工条（Node API 禁进 helpers.ts——Read 确认）。
- **TE-08（二选一，与代码现状一致）**：分支 a——`rg "fonts.check" e2e-tests/markdown.e2e.ts` 命中（双对照断言：NoSuchFontXyzQq false + KaTeX_Main true，Read 确认）；或分支 b——`rg "fontProbe\|slterm_font_probe" src/panels/docViewer/ e2e-tests/markdown.e2e.ts` 命中（buildInjectedScript 段 + 白名单第四类型 + PreviewFrame 收束 + waitUntil 断言链路齐全，Read 逐件确认）。两分支互斥——按代码现状判定哪支落地；两支皆无判 not_fixed；两支皆有判 partial（超面）。
- **TE-08（实证留痕）**：重构 agent（katex-font-probe）报告含 `[TE-08 探针]` JSON 实证输出原文 + 落地分支声明（由测试/重构结果转述——verify 对照代码现状与报告分支一致）。
- **TE-09**：`rg "coreInvokeWarnCount" e2e-tests/run-wdio.cjs` 命中；Read 确认 runWdio 与 fallback 两通道均 spawn `stdio:['inherit','pipe','pipe']` + FORCE_COLOR + 转发计数 + close 打印计数行；`rg "execSync" e2e-tests/run-wdio.cjs` 零命中（cliArgs 拼串弃用）；`rg "正常速度" e2e-tests/CLAUDE.md` 零命中（矛盾句绝迹）；e2e 全量绿且输出含 `[wdio-launcher] tauri-service core.invoke WARN 计数` 行（测试 agent 结果承载）。
- **TE-09（文档面）**：Read `e2e-tests/CLAUDE.md`「$()/elementClick 触发焦点检查」节——「前提满足后命令语义正确但每 focus 命令仍确定性 +5s」口径存在；节末 WARN 可观测化段存在（基线数字允许「待填记」占位——全量基线填记入 Stage commit body 主 agent 动作）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run e2e`（= build:e2e + wdio 全量；前提：窗口前台聚焦；时长 20+ 分钟属正常）
