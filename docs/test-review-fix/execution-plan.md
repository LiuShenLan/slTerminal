# 执行编排参数（test-review-fix）

> 只写任务特定编排参数；通用执行规则见 `/systematic-changes-execute`（单一真值源，不复制）。
> 配套：`checklist.md`（64 项六段式）/ `stages.md`（10 Stage 分工+要点）/ `workflows/`（脚本+verify）。

## Stage 表

| Stage | 名称 | 脚本 | verify | 门禁命令（全量测试 agent 执行） |
|-------|------|------|--------|------------------------------|
| 01 | L2 flaky+异步等待 | workflows/stage-01-l2-timing.js | verify/stage-01.md | tsc / eslint / npm test |
| 02 | L2 隔离性 | workflows/stage-02-l2-isolation.js | verify/stage-02.md | 同上 |
| 03 | L2 替身脱节+testid | workflows/stage-03-l2-decouple.js | verify/stage-03.md | 同上 |
| 04 | L2 断言强化+数据层 | workflows/stage-04-l2-assertions.js | verify/stage-04.md | 同上 |
| 05 | L2 覆盖补写 | workflows/stage-05-l2-coverage.js | verify/stage-05.md | tsc / eslint / npm test / npm run test:coverage |
| 06 | L3 复用生产 | workflows/stage-06-l3-production.js | verify/stage-06.md | tsc / eslint / npm test / npm run test:l3 |
| 07 | L4 修复 | workflows/stage-07-l4-e2e.js | verify/stage-07.md | npx vite build / npm run e2e |
| 08 | Rust 可测性+L1 | workflows/stage-08-rust.js | verify/stage-08.md | clippy / fmt / cargo test（单点） |
| 09 | CI 门禁 | workflows/stage-09-ci.js | verify/stage-09.md | fmt 本地 / ci.yml yaml 解析 / npm run e2e |
| 10 | 文档收尾 | workflows/stage-10-docs.js | verify/stage-10.md | 全量 7 条（tsc/eslint/clippy/npm test/cargo test/test:l3/e2e） |

## commit message 与 git add 路径

| Stage | commit | git add |
|-------|--------|---------|
| 01 | `test(l2): 修复实证 flaky 与异步等待——diff-panel CM6 等待 + waitFor 稳定化 6 项` | `src/__tests__/` |
| 02 | `test(l2): 隔离性修复——全局 stub 恢复/sideViewDefs 阻断/store 统一重置 6 项` | `src/__tests__/` |
| 03 | `test(l2): 替身脱节修复——生产组件/常量/testid 复用 7 项（最小生产微改）` | `src/__tests__/` + 4 个生产微改文件（见 stages.md） |
| 04 | `test(l2): 断言强化与数据层补全 12 项` | `src/__tests__/`, `src/panels/terminal/usePtyOutput.ts`, `src/theme/` |
| 05 | `test(l2): 前端覆盖缺口补写——终端错误分支/DockviewHost/NavPageRow 等` | `src/__tests__/` |
| 06 | `refactor(terminal): OSC/按键注册层抽纯函数（oscHandlers/keyEventHandler）+ L3 复用生产实现` | `src/panels/terminal/`, `test/terminal/` |
| 07 | `test(e2e): L4 修复——条件等待/吞错/粘贴断言/恢复报告/settings 隔离 5 项` | `e2e-tests/`, `src/global.d.ts` |
| 08 | `test(rust): 可测性抽取与 L1 覆盖补写——panic hook/PTY/hooks 信号链/SEC-17 审计/git 死函数 8 项` | `src-tauri/` |
| 09 | `ci: 门禁补全——rustfmt/timeout/npm 缓存/E2E flakiness 观察面` | `.github/workflows/ci.yml`, `e2e-tests/wdio.conf.ts` |
| 10 | `docs(test): inventory 校准与豁免清单更新 + 模块 CLAUDE.md 同步 + 覆盖复测收尾` | `.claude/test-inventory.md`, 各模块 CLAUDE.md, `docs/test-review-fix/` |

（commit 尾部统一加 `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`）

## fix-loop args 规范

每 Stage verify 失败时调用 `workflows/fix-loop.js`：
- `stage`: Stage 编号（1-10）
- `failedItems`: verify 返回的 failedItems（TQ-XX 编号）
- `fixContext`: verify details 原文
- `verifyFile`: `docs/test-review-fix/workflows/verify/stage-NN.md`（与 Stage 同一真值源）
- `constraints`: Stage 特殊纪律——**Stage 03 传**「生产文件微改仅限加 export / data-testid / 抽函数原样移动，禁止逻辑改动」；**Stage 06 传**「行为不变重构，禁止改 OSC/按键语义」；**Stage 01/02/04/05 传**「只改测试与测试 helper」；其余传空。

## 进度跟踪表（执行期填写）

| Stage | 状态 | verify 结果 | fix-loop 轮数 | commit hash |
|-------|------|------------|---------------|-------------|
| 01 | ☐ | — | 0 | — |
| 02 | ☐ | — | 0 | — |
| 03 | ☐ | — | 0 | — |
| 04 | ☐ | — | 0 | — |
| 05 | ☐ | — | 0 | — |
| 06 | ☐ | — | 0 | — |
| 07 | ☐ | — | 0 | — |
| 08 | ☐ | — | 0 | — |
| 09 | ☐ | — | 0 | — |
| 10 | ☐ | — | 0 | — |

## 项目禁区（写入各脚本 PREAMBLE，来源 config.json forbiddenZones）

- compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。

## 收尾验收（Stage 10 内执行，checklist 末节为唯一口径）

- 全量四级复跑全绿（L1 `--test-threads=1` / L2 / L3 / L4）。
- coverage 对照：前端行 ≥94.5%（基线 93.93%）/ Rust 行 ≥90%（基线 87.70%）；重点文件达标或逐条登记豁免。
- inventory 三处一致（表头 = 段头 = 段小计之和 = 实跑数）。
- 人工验证点 3 项实测（stages.md 汇总表）。
