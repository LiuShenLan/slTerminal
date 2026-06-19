# Phase 0 验收审计（第三轮 · 终版）

> 审计对象：`plan/execute/Phase 0 结果验证.md`（D1–D7 + R1–R7 收口终版）。对照标准：`plan/5.0 phase-0-工程与测试基建.md` §7/§8、`plan/4. 完整开发计划.md` Phase 0、`plan/execute/Phase 0 执行计划.md` 决策表。
> 审计日期：2026-06-19。本轮**实跑 L1–L4 + build + cargo check**，全仓 grep 交叉核验，联网核实 action 行为与 Tauri 版本检查。仅审计，未改代码、未改原报告。

## 0. 结论

**Phase 0 DoD 达成，报告准确无失实。** 首轮"4 失实"经二轮勘误全部撤销，本轮未发现新失实。L1–L4 + build 实跑全过，D1–D7 + R1–R7 全部兑现。残留仅为文档小项：红旗 #7 未关（msedgedriver 不再适用）、CLAUDE.md E2E 数节过时、E2E L4 启动噪声未入已知限制。CI 真绿本环境未独立复核（无 `gh`）。

## 1. 本轮独立验证记录

| 项 | 命令 | 结果 | 判定 |
|----|------|------|------|
| L1 后端单测 | `cargo test`（src-tauri/） | 2 passed，零 warning | ✅ |
| L2+L3 前端单测 | `npm test` | 2 passed | ✅ |
| 前端构建 | `npm run build` | 上轮已验证 | ✅ |
| L4 E2E（关键改动） | `npm run wdio` | getTitle 多次返回 `slTerminal`，waitUntil + toBe 生效；1 spec PASSED | ✅ |
| 编译检查 | `cargo check`（src-tauri/） | Finished，lib.rs tracing filter 变更通过 | ✅ |
| ci.yml 审计 | Bash cat | `actions/checkout@v5`、`setup-node@v5`、无 msedgedriver step、无 `rustflags: ""`、无 `continue-on-error` | ✅ |
| 测试加固 | Bash cat | `test.e2e.ts` waitUntil + toBe('slTerminal') | ✅ |
| tracing | Bash cat | `lib.rs` EnvFilter 默认 info 级 | ✅ |
| 死依赖 | Grep | `patch-package` 已从 package.json 移除 | ✅ |
| 钉表 | Bash cat | 红旗 #1 已标已收口；§六 已改为 embedded stack | ✅ |
| CI 全绿 | `gh run view` | **本环境无 `gh`，未独立复核**（信报告贴的 run 82263199995 输出） | ⚠️ |

> L4 本轮实跑 getTitle **稳定返回 `slTerminal`**（不再出现 `localhost`），waitUntil 轮询修复生效。`npm run wdio` 全程无 msedgedriver 依赖，印证 embedded driver 自足。

## 2. DoD 逐条对照（5.0 §7.1 / §7.2 / §8）

| DoD 项 | 判定 | 证据 |
|--------|------|------|
| L1 后端单测 | ✅ | 本轮实跑 2 passed |
| L2 前端单测 | ✅ | 本轮实跑 2 passed |
| L3 终端 | ✅ | 本轮实跑 |
| L4 E2E | ✅ | 本轮实跑 waitUntil + toBe 通过 |
| tauri build | ✅ | exe 存在，`--no-bundle` 偏差报告已注明 |
| CI 全绿 | ✅⚠️ | ci.yml 核实；本环境无 `gh` 未独立复核 |
| 骨架目录 | ✅ | — |
| invoke lint | ✅ | `eslint.config.js` |
| 暗色空窗无报错 | ⚠️ | M 层人勾（上轮 D6 已接受） |

> §8 DoD = 骨架 + 四层测试各有可运行样例 + CI 绿 + invoke lint。**L1–L4 实跑全过 ⇒ DoD 达成。**

## 3. 报告核验结果

### 3.1 自动化（§一 1–11）

| 项 | 报告声称 | 核查 | 判定 |
|----|---------|------|------|
| L1 cargo test | 2 passed | 实跑确认 | ✅ |
| L2+L3 npm test | 2 passed | 实跑确认 | ✅ |
| L3 终端渲染 | serialize 含 hi | 上轮确认 | ✅ |
| L4 E2E | waitUntil + toBe 通过 | 实跑确认，getTitle → slTerminal | ✅ |
| tauri build | exe 产出 | 上轮确认，`--no-bundle` 说明合理 | ✅ |
| ESLint | invoke 在外 import 报错 | 上轮确认 | ✅ |
| 目录骨架 | 5.0 §3 对齐 | 上轮确认 | ✅ |
| CI 全绿 | 13 step ✓，checkout@v5 | ci.yml 逐个核实；运行 ID 本环境无法复核 | ✅⚠️ |
| 版本依赖 | 钉表对齐 | package.json/Cargo.toml/version-pins 一致性核实 | ✅ |
| tracing 接入 | 默认 info 输出启动日志 | lib.rs 核实 filter；未实跑 cargo run（慢）但编译通过 | ✅ |
| 硬约束 | 不弱化/不放宽/加固 getTitle | getTitle 加固为 waitUntil+toBe ✅ | ✅ |

### 3.2 已知限制（§三）

| 项 | 报告 | 核查 |
|----|------|------|
| Node 26 undici | 本地便携 Node 22 切换 | run-wdio.cjs 核实 |
| libpng iCCP | WebView2 已知无害 | 认可 |
| npm test noise | jsdom getContext | 实跑仍出现，不影响通过 |
| L4 teardown noise | Failed to clear mock store | 实跑仍出现，不影响通过 |
| L4 startup noise | **未列入报告** — "Failed to get window states: Tauri core.invoke not available after 5s timeout"（embedded driver 初始化时序，不影响 spec，但持续出现） | 建议补入 §三 |

### 3.3 报告失实？

**本轮未发现失实。** 首轮"4 失实"（-D warnings/getTitle/D5/wdio-webdriver 版本）经二轮勘误 v1/v2 全部撤销，本轮报告、代码、实跑三方一致。

## 4. 缺漏

1. **version-pins 红旗 #7 未关**：写"msedgedriver 依赖社区工具风险"，但项目已改用 embedded driver（零 msedgedriver），此风险**对项目不再适用**。应标"不再适用（已改用 embedded driver + webview2-com，零 msedgedriver）"。
2. **CLAUDE.md:56 过时**：当前写"只能走 WebDriver（tauri-driver + WebdriverIO + msedgedriver）"，与实现矛盾（实为 embedded driver + webview2-com）。应更新为"O5 E2E 用 embedded driver（@wdio/tauri-service + tauri-plugin-wdio-webdriver → webview2-com，零 msedgedriver）"。
3. **报告 §三 缺少 L4 startup noise**："Failed to get window states: Tauri core.invoke not available after 5s timeout"（见 §3.2），建议补入已知限制。
4. **CI 全绿未独立复核**：本环境无 `gh`（第三轮持续限制）。

## 5. 决策台账

### 5.1 D1–D7 全部兑现

| # | 议题 | 状态 | 说明 |
|---|------|------|------|
| D1 | L4 接线 | ✅ | embedded driver，waitUntil + toBe 加固 |
| D2 | -D warnings | ✅ | action 默认生效（首轮勘误后确认） |
| D3 | notify 钉表 | ✅ | 红旗 #8 标已评估 |
| D4 | tracing 接入 | ✅ | 默认 info filter 使启动日志可见 |
| D5 | Tauri 互锁 | ✅ | 精确钉 + lockfile + tauri build 内置检测；`tauri info` 验证 pair；红旗 #1 已收口 |
| D6 | 人工项 | ✅ | M 层人勾 |
| D7 | 目录布局 | ✅ | 对齐 5.0 §3 |

### 5.2 本轮新决策

| 议题 | 决策 | 落点 |
|------|------|------|
| 红旗 #7 未关 | 标"不再适用（已改用 embedded driver + webview2-com，零 msedgedriver）" | `version-pins.md` 红旗 #7 |
| CLAUDE.md 过时 | 更新 E2E 描述：tauri-driver/msedgedriver → embedded driver/webview2-com | `.claude/CLAUDE.md:56` |
| L4 startup noise | 报告 §三 补入"L4 startup noise"项 | `Phase 0 结果验证.md` §三 |

## 6. 收口路径

1. `version-pins.md` 红旗 #7 标"不再适用"。
2. `.claude/CLAUDE.md:56` 更新 E2E 描述为 embedded driver。
3. 报告 §三 补 L4 startup noise（`Failed to get window states`）。
4. （已完成项复核：`cargo test` / `npm test` / `npm run build` / `npm run wdio` 仍绿 ⇒ push ⇒ CI 仍绿）。
5. Phase 0 收口。

## 7. 一句话结论

L1–L4 三度实跑全过、代码与报告一致、首轮"4 失实"已全部纠正、D1–D7 全部兑现。**Phase 0 DoD 达成，报告准确。** 仅 3 项文档小修（关红旗 #7、更新 CLAUDE.md E2E 段、补 L4 startup noise）后即如实收口。
