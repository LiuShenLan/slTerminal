# Stage 划分（test-review-fix）

> 10 个 Stage，串行执行，每 Stage commit。Stage 内并行 agent ≤5 且**文件零重叠**（分工表 = 各 agent 触碰文件全集，含微改文件）。
> 真值源：每项修复细节以 `checklist.md` 对应 ID 六段式为准；每 Stage 断言以 `workflows/verify/stage-NN.md` 为准。
> 跨边界契约：Stage 06 的生产导出签名写死于本文件与脚本头（并行/串行 agent 不各自推断）。
> 偏离规则豁免：Stage 06 仅 2 项（生产重构 + L3 改写强耦合，须串行）；Stage 09 仅 4 项且全在单文件 ci.yml（1 agent 足够）。

## Stage 01：L2 实证 flaky + 异步等待（6 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-diff-panel | TQ-A-01 | src/__tests__/diff-panel.test.tsx |
| fix-explorer-timing | TQ-B-04, TQ-B-18 | src/__tests__/explorer-race-cleanup.test.tsx, src/__tests__/explorer-refresh-preserve.test.tsx |
| fix-commit-status | TQ-B-06 | src/__tests__/commit-view-status.test.ts |
| fix-navtree-waitfor | TQ-B-09 | src/__tests__/nav-tree.test.tsx, src/__tests__/nav-tree-history.test.tsx |
| fix-keyboard-mock | TQ-B-19 | src/__tests__/keyboard.test.ts |

- 实现要点：只改测试；fake timers 文件用 `vi.waitFor`（自动推进 fake timers）；TQ-A-01 修后须组合重跑验证（17 文件清单见 docs/test-review/02 复跑段，连跑 3 轮）。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test`
- 人工验证点：无。
- commit：`test(l2): 修复实证 flaky 与异步等待——diff-panel CM6 等待 + waitFor 稳定化 6 项`
- git add：`src/__tests__/`

## Stage 02：L2 隔离性/全局污染（6 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-global-stubs | TQ-A-02 | src/__tests__/workspace.test.tsx, workspace-multi-instance.test.tsx, workspace-switch-order.test.tsx, workspace-page-dockview.test.tsx, use-xterm-integration.test.ts |
| fix-setup-geometry | TQ-A-03 | src/__tests__/setup.ts |
| fix-sideview-isolation | TQ-B-02 | src/__tests__/sideBar.test.ts, sideBarArea.test.tsx, activityBar.test.tsx |
| fix-store-reset | TQ-B-10, TQ-B-15 | src/__tests__/helpers/workspace-setup.ts, src/__tests__/commit-view.test.tsx, nav-tree.test.tsx, explorer-crud-success.test.tsx, commit-open-file.test.ts |
| fix-profile-reset | TQ-B-14 | src/__tests__/nav-history-row.test.tsx |

- 实现要点：只改测试与测试 helper；useSideBar 的 import 路径以实际文件为准（sideBarState.ts vs stores/sideBar.ts 先 Read）。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test`
- 人工验证点：无。
- commit：`test(l2): 隔离性修复——全局 stub 恢复/sideViewDefs 阻断/store 统一重置 6 项`
- git add：`src/__tests__/`

## Stage 03：L2 替身/复制脱节 + testid 微改（7 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-watermark | TQ-A-04 | src/workspace/PageDockviewHost.tsx（仅加 export）, src/__tests__/workspace-header-actions.test.tsx |
| fix-barrel-mocks | TQ-A-05 | src/__tests__/use-xterm-error-toast.test.ts, diff-panel.test.tsx, diff-panel-stale-banner.test.tsx |
| fix-explorer-testids | TQ-B-01, TQ-B-05, TQ-B-17 | src/features/explorer/FileTree.tsx（仅加 2 处 data-testid）, src/__tests__/explorer-virtualization.test.tsx, explorer-crud-success.test.tsx, explorer-delete.test.tsx |
| fix-viewer-registry | TQ-B-11 | src/features/fileViewers/FileViewerRegistry.ts（抽 registerDefaultViewers 导出）, src/__tests__/file-viewer-registry.test.ts |
| fix-commit-menu | TQ-B-13 | src/features/commit/CommitFileList.tsx（仅加 data-testid）, src/__tests__/commit-context-menu-ui.test.tsx |

- 特殊纪律：**生产文件微改仅限「加 export / 加 data-testid 属性 / 抽函数原样移动」，禁止任何逻辑改动**；fix-loop constraints 传此纪律。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test`
- 人工验证点：无。
- commit：`test(l2): 替身脱节修复——生产组件/常量/testid 复用 7 项（最小生产微改）`
- git add：`src/__tests__/`, `src/workspace/PageDockviewHost.tsx`, `src/features/explorer/FileTree.tsx`, `src/features/fileViewers/FileViewerRegistry.ts`, `src/features/commit/CommitFileList.tsx`

## Stage 04：L2 断言强化 + 数据层 + 竞态（12 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-thresholds-docs | TQ-A-06, TQ-A-07, TQ-C-03 | src/panels/terminal/usePtyOutput.ts（仅加 export）, src/__tests__/use-xterm-output.test.ts, workspace-sideviews.test.tsx, scheme-registry.test.ts, src/theme/CLAUDE.md |
| fix-assertions | TQ-A-08, TQ-C-01 | src/__tests__/use-code-mirror.test.ts, editor-confirm.test.ts, overrides.test.ts, src/theme/overrides.ts（如需补导出） |
| fix-race-keybindings | TQ-B-03, TQ-B-16 | src/__tests__/explorer-sandbox-race.test.tsx, wire-keybindings.test.ts |
| fix-drop-contract-scan | TQ-B-07, TQ-C-02, TQ-C-04 | src/__tests__/activityBar.test.tsx, drop-target.test.ts（可能新建）, ipc-agent-history-contract.test.ts, no-claude-literals.test.ts |
| fix-focus-keyboard | TQ-B-08, TQ-B-12 | src/__tests__/explorer-delete.test.tsx, global-commands.test.ts, shortcuts.test.ts, src/__tests__/helpers/keyboard.ts（新建）, src/__tests__/CLAUDE.md |

- 实现要点：TQ-B-03 先观察实际行为再定断言（禁止放宽到时序无约束）；TQ-C-04 全量扫描若命中既有违例须报告，禁止默默加豁免；TQ-C-01 的 themeRules 提取方式先 Read overrides.ts 定。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test`
- 人工验证点：无。
- commit：`test(l2): 断言强化与数据层补全 12 项`
- git add：`src/__tests__/`, `src/panels/terminal/usePtyOutput.ts`, `src/theme/`

## Stage 05：L2 前端覆盖缺口补写（4 大项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| cov-terminal-hooks | TQ-COV-07 | src/__tests__/terminal-instance.test.ts, use-xterm-*.test.ts（按归属追加，不新建） |
| cov-dockview-host | TQ-COV-08 | src/__tests__/workspace-page-dockview.test.tsx |
| cov-nav-explorer | TQ-COV-09 | src/__tests__/nav-page-row.test.tsx（新建或并入 nav-tree.test.tsx）, explorer 相关测试文件 |
| cov-lowrisk-pack | TQ-COV-10 | src/__tests__/ipc-window.test.ts（新建）, sideview-defs 相关（新建或并入 sideBar 系）, nav-history-row.test.tsx, gitshow-panel.test.tsx 等 |

- 实现要点：先跑 `npm run test:coverage` 取各文件未覆盖分支清单再补测；每例断言用户可见行为（toast/console/DOM），不只「不 throw」；07 报告误写 DockviewHost.tsx 已翻案为 PageDockviewHost.tsx（checklist TQ-COV-08 留痕）。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test` 4. `npm run test:coverage`（复测对照目标见 checklist 收尾节）
- 人工验证点：无。
- commit：`test(l2): 前端覆盖缺口补写——终端错误分支/DockviewHost/NavPageRow 等`
- git add：`src/__tests__/`

## Stage 06：L3 复用生产实现（2 项，串行 pipeline）

**跨边界契约（写死，双方 agent 不各自推断）**：
- 新文件 `src/panels/terminal/oscHandlers.ts` 导出：`MAX_OSC52_PAYLOAD` / `registerOsc52(term, deps): IDisposable` / `registerOsc133(term, deps): IDisposable` / `makeLinkHandler(openUrl)`（签名全文见 checklist TQ-E-01 步骤 1 代码块）
- 新文件 `src/panels/terminal/keyEventHandler.ts` 导出：`handleTerminalKeyEvent(event): boolean`（见 checklist TQ-E-02 步骤 1）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| extract-osc-layer（串行①） | TQ-E-01 生产侧, TQ-E-02 生产侧 | src/panels/terminal/oscHandlers.ts（新建）, keyEventHandler.ts（新建）, useClipboardHandler.ts, useCommandDetection.ts, useXterm.ts |
| rewrite-l3（串行②） | TQ-E-01 L3 侧, TQ-E-02 L3 侧 | test/terminal/production-osc.test.ts, test/terminal/shortcut-dispatch.test.ts（新建） |

- 划分豁免：仅 2 项——生产抽取与 L3 改写强耦合（L3 依赖抽取后的最终导出形态），串行 pipeline。
- 特殊纪律：抽取为**行为不变**重构——handler 体逐字搬移，仅依赖改参数注入；禁止顺手改 OSC 语义。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `npm test` 4. `npm run test:l3`
- 人工验证点：**终端实测 OSC52**（`printf '\e]52;c;%s\a' <base64>` → 系统剪贴板）与 **OSC133 状态圆点**（claude 运行 → 页签圆点变化）各一次。
- commit：`refactor(terminal): OSC/按键注册层抽纯函数（oscHandlers/keyEventHandler）+ L3 复用生产实现`
- git add：`src/panels/terminal/`, `test/terminal/`

## Stage 07：L4 E2E 修复（5 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-e2e-sleeps | TQ-E-03, TQ-E-04 | e2e-tests/agent.e2e.ts, history.e2e.ts, mockcli.e2e.ts |
| fix-e2e-paste | TQ-E-05 | e2e-tests/terminal.e2e.ts |
| fix-launcher-restore | TQ-E-06 | e2e-tests/run-wdio.cjs |
| fix-e2e-reset | TQ-E-08 | e2e-tests/helpers.ts, e2e-tests/wdio.conf.ts, src/global.d.ts |

- 实现要点：sleep 三处改 waitUntil 时先 Read 上下文定「下一步要交互的行选择器」作条件；helpers.ts 不在根 tsconfig include——**门禁必须补 `npx vite build`**（构建图兜底，main.tsx 动态 import 它）。
- 门禁：1. `npx vite build` 2. `npm run e2e`（= build:e2e + wdio 全量）
- 人工验证点：**TQ-E-06 恢复失败路径**——人为占用/锁定 `~/.slterminal/hooks` 目录跑一次 `npm run wdio`，确认输出失败清单且进程退出码非 0。
- commit：`test(e2e): L4 修复——条件等待/吞错/粘贴断言/恢复报告/settings 隔离 5 项`
- git add：`e2e-tests/`, `src/global.d.ts`

## Stage 08：Rust 可测性 + L1 修复补测（8 项）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| rust-panic-hook | TQ-COV-01, TQ-L1-01 | src-tauri/src/main.rs, src-tauri/src/lib.rs, src-tauri/src/settings.rs（仅测试注释） |
| rust-pty-tests | TQ-COV-03, TQ-L1-03, TQ-L1-05 | src-tauri/src/pty/spawn.rs（仅测试模块）, src-tauri/tests/pty_integration_tests.rs, src-tauri/src/pty/CLAUDE.md |
| rust-hooks-tests | TQ-COV-04 | src-tauri/src/hooks/signal.rs（仅测试模块）, src-tauri/src/hooks/watcher.rs（仅测试模块） |
| rust-audit-log | TQ-COV-05 | src-tauri/Cargo.toml, src-tauri/src/hooks/claude/config.rs（仅测试模块） |
| rust-git-coverage | TQ-COV-06 | src-tauri/src/git/mod.rs（死函数删除时）, src-tauri/tests/git_*_tests.rs |

- 并行纪律：**并行 agent 不跑 `cargo test`**（target 锁 + ConPTY 串行红线）——只做 `cargo test --no-run` 编译级检查；全量由测试 agent 单点跑（cargo 命令排队属正常勿中止）。
- 实现要点：TQ-COV-06 先跑 llvm-cov --html 定位未执行函数再分流（死函数删 / 活函数补测）；TQ-COV-05 的写盘函数名以 config.rs 既有 `config_write_sync_*` 用例调用的真实函数为准。
- 门禁：1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（全量测试 agent 单点）
- 人工验证点：无。
- commit：`test(rust): 可测性抽取与 L1 覆盖补写——panic hook/PTY/hooks 信号链/SEC-17 审计/git 死函数 8 项`
- git add：`src-tauri/`

## Stage 09：CI 门禁（4 项，1 agent）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| fix-ci | TQ-CI-03, TQ-CI-06, TQ-CI-07, TQ-E-09 | .github/workflows/ci.yml, e2e-tests/wdio.conf.ts |

- 划分豁免：4 项全在单文件 ci.yml（+wdio.conf 一行），1 agent 串行。
- 实现要点：插入 fmt 步骤前先本地跑 `cargo fmt --check` 确认当前过（不过先 `cargo fmt` 修齐并单独 commit）；yaml 改动后用 node 解析校验语法。
- 门禁：1. 本地 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 2. `node -e "require('yaml').parse(...)"`（或以 js-yaml 解析 ci.yml 验证）3. `npm run e2e`（WDIO_RETRIES 改动冒烟）
- 人工验证点：无（CI 行为以下次 push 实际运行为准）。
- commit：`ci: 门禁补全——rustfmt/timeout/npm 缓存/E2E flakiness 观察面`
- git add：`.github/workflows/ci.yml`, `e2e-tests/wdio.conf.ts`

## Stage 10：文档/inventory 收尾（10 项 + 统一动作）

| agent | 负责项 | 触碰文件 |
|-------|--------|----------|
| sync-inventory | TQ-CI-01, TQ-CI-02, TQ-CI-05, TQ-COV-02, TQ-E-10, TQ-L1-02 | .claude/test-inventory.md, src/__tests__/CLAUDE.md |
| sync-module-docs | TQ-L1-04, TQ-L1-06 + 模块同步 | src-tauri/src/pty/CLAUDE.md, src-tauri/src/git/CLAUDE.md, src-tauri/src/hooks/CLAUDE.md, src/features/explorer/CLAUDE.md, src/features/fileViewers/CLAUDE.md, src/panels/CLAUDE.md, e2e-tests/CLAUDE.md |
| final-verify（串行收尾） | 收尾统一动作 | 只读（复跑+对照报告） |

- 实现要点：inventory 三处（表头/段头/段小计）以**当时实跑数**校准（Stage 01-08 净增用例后非 726/2635）；豁免清单更新——SEC-17 翻案（TQ-COV-05）、新增 PTY 残余豁免（TQ-COV-03）、lib.rs 细化（TQ-COV-02）、L4 按键条目细化（TQ-E-05）、L3 职责边界（TQ-E-10）、条件跳过（TQ-L1-02）、Mutex 中毒+大小判定（TQ-L1-04/06 在 pty/CLAUDE.md）。
- 收尾统一动作（checklist 末节）：①全量四级复跑全绿；②coverage 复测对照（前端行 ≥94.5% / Rust 行 ≥90%，重点文件达标或登记）；③人工验证点实测汇总。
- 门禁：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 4. `npm test` 5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 6. `npm run test:l3` 7. `npm run e2e`
- 人工验证点：汇总实测 Stage 06（OSC52 剪贴板/OSC133 圆点）+ Stage 07（恢复失败路径）——若 Stage 06/07 执行时已实测则复核记录即可。
- commit：`docs(test): inventory 校准与豁免清单更新 + 模块 CLAUDE.md 同步 + 覆盖复测收尾`
- git add：`.claude/test-inventory.md`, `src/**/CLAUDE.md`, `src-tauri/**/CLAUDE.md`, `e2e-tests/CLAUDE.md`, `docs/test-review-fix/`

## 人工验证点汇总（逐项检查——无法自动化验证的假设）

| 来源 | 验证点 | 实测方式 |
|------|--------|----------|
| Stage 06 | OSC52 剪贴板真实行为 | debug 构建终端内 `printf '\e]52;c;%s\a' <base64>`，系统剪贴板读回 |
| Stage 06 | OSC133 命令边界/状态圆点 | 终端运行 claude，页签状态圆点/标题变化 |
| Stage 07 | run-wdio 恢复失败可观测 | 占用 ~/.slterminal/hooks 后 `npm run wdio`，输出失败清单 + 退出码非 0 |
