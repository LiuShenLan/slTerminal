# Debug 修复执行计划（故障A/B）

> 2026-07-18 · 输入：`docs/debug/refactor-regressions.md` + `docs/debug/debug-checklist.md`
> 本计划只供执行参考——**当前未执行**。执行时逐 Stage 串行，每 Stage 一个 Workflow + 一个 commit。

## 0. 背景与决策

| 项 | 结论 |
|----|------|
| 故障B（③④ 终端无输入） | Stage2 后端 `pty_write/resize/kill` 加必填 `panel_id`，前端 `src/ipc/pty.ts` 未同步 → 每次按键必败 |
| 故障A（①② 文件浏览器空目录） | Stage1 路径沙箱 × React 子 effect 先于父 effect：`fs_read_dir` 必在 `set_project_root` 前被拒，静默吞错无重试 |
| 故障A 方案 | **A1**：`setProjectRoot` 前置为页面切换动作的 await 前置条件（非 A2） |
| 范围 | 含防复发沉淀（DBG-4 契约守卫 + DBG-11 文档）与回归测试（DBG-9/10） |
| 修复顺序 | 先 Stage 1（故障B，确定性单点）→ 再 Stage 2（故障A）→ Stage 3 文档 |

## 1. Stage 总览与进度跟踪

| Stage | 内容 | ID | 脚本 | 状态 |
|-------|------|-----|------|------|
| 1 | PTY write/resize/kill 补 panelId + 契约守卫测试 | DBG-1~4 | `workflows/stage-01-pty-panelid.js` | 未开始 |
| 2 | setProjectRoot 前置（A1）+ 可观测性 + 回归测试 | DBG-5~10 | `workflows/stage-02-project-root-precede.js` | 未开始 |
| 3 | 文档同步 | DBG-11 | `workflows/stage-03-docs-sync.js` | 未开始 |

恢复规则：中断后从首个「未开始」Stage 继续，其前 commit 已落盘无需重做。

## 2. Stage 1 — PTY panelId 契约同步（DBG-1/2/3/4）

**commit message**：`fix: 故障B——PTY write/resize/kill 补 panelId 契约同步 + 契约守卫测试`

### agent 文件分工（零重叠）

| label | 负责 ID | 文件 |
|-------|---------|------|
| `pty-src-fix` | DBG-1, DBG-2 | `src/ipc/pty.ts`、`src/panels/terminal/useXterm.ts`、`src/panels/terminal/usePtyResize.ts`、`src/App.tsx` |
| `pty-contract-test` | DBG-3, DBG-4 | `src/__tests__/ipc-contract.test.ts` |

### 实现要点

- 契约写死（两 agent 不各自推断）：
  - `write(sessionId, panelId, data)` → `invoke("pty_write", { sessionId, panelId, data: Array.from(data) })`
  - `resize(sessionId, panelId, cols, rows)` → `invoke("pty_resize", { sessionId, panelId, cols, rows })`
  - `kill(sessionId, panelId)` → `invoke("pty_kill", { sessionId, panelId })`
- panelId 来源：useXterm/usePtyResize 闭包与 hook 入参现成的 `panelId`；`App.tsx:83` 改 `allSessions.forEach((entry, panelId) => …)` 取 Map key。
- **不改任何 Rust 代码**（后端签名已正确）。不改 `RegisteredTerminal` 结构。
- 并行 agent 不跑测试，统一由全量测试 agent 跑。

### 验证项

断言真值源：`workflows/verify/stage-01.md`。测试门禁：`tsc` / `eslint` / `clippy` / `npm test`。

### 人工验证点（commit 后、进 Stage 2 前）

`npx tauri build --debug --no-bundle` 后实测：① 打字有回显；② Ctrl+Shift+V 粘贴可执行；③ 拖拽分屏后 TUI 无错位；④ 关窗无 pwsh/cmd 残留。

## 3. Stage 2 — setProjectRoot 前置 + 可观测性 + 回归测试（DBG-5~10）

**commit message**：`fix: 故障A——setProjectRoot 前置为页面切换前置条件（方案A1）+ 可观测性与回归测试`

### agent 文件分工（零重叠）

| label | 负责 ID | 文件 |
|-------|---------|------|
| `timing-src-fix` | DBG-5, DBG-6, DBG-7, DBG-8 | `src/workspace/Workspace.tsx`、`src/App.tsx`、`src/features/explorer/useFileTree.ts`、`e2e-tests/helpers.ts` |
| `timing-test` | DBG-9, DBG-10 | `src/__tests__/`（新增 `workspace-switch-order.test.tsx`（或并入现有 workspace 测试）+ 新增 `explorer-sandbox-race.test.tsx`；适配受 async 影响的现有用例） |

> `App.tsx` 在 Stage 1 已被碰——Stage 间串行 + 各自 commit，无冲突（SKILL 规则允许）。本 Stage 改的是 `App.tsx:60-63` 启动恢复段，与 Stage 1 的 `:83-91` 关窗段不相邻。

### 实现要点

- **DBG-5 降级策略**：`await setProjectRoot(rootPath)` 失败时 `console.error("[slTerminal] 设置项目根路径失败:", err)` 后**仍继续切换**（Explorer 可能空目录，但终端可用——不劣于现状）。
- **DBG-5 不动** `Workspace.tsx:197-212` SEC-01 effect（保留兜底：服务 `pty_spawn` 等其他消费者 + E2E 直设路径；`prevRootRef` 幂等防重复）。
- **DBG-8**：helpers 两函数改 async；`__slterm_e2e_createProject` 把已有 `setProjectRoot` 调用从 `setActivePage` 之后移到之前并 await；`__slterm_e2e_switchToPage` 新增 rootPath 查找（遍历 projects）+ await。E2E 调用侧（`test.e2e.ts` 经 `browser.execute`）对返回 promise 天然 await，无需改测试脚本。
- **DBG-10 用例要点**：mock `../ipc/fs` 的 `setProjectRoot` 为手动控制 promise；触发页面切换；断言 promise resolve 前 `readDir` 调用次数为 0；resolve 后 `waitFor` 文件树渲染。
- 注意：`e2e-tests/helpers.ts` 不在根 tsconfig `include` 内（`tsc` 不覆盖），改动由 `npx vite build` 打包图验证 + 收尾 `build:e2e` 兜底。
- 并行 agent 不跑测试，统一由全量测试 agent 跑。

### 验证项

断言真值源：`workflows/verify/stage-02.md`。测试门禁：`tsc` / `eslint` / `clippy` / `npm test` / `npx vite build`。

### 人工验证点（commit 后）

build 后实测：① 打开操作页面文件浏览器立即显示项目文件（不再"空目录"）；② 切换不同项目页面文件树正确；③ 打开文件页签正常；④ 终端可启动 claude。

## 4. Stage 3 — 文档同步（DBG-11）

**commit message**：`docs: debug 修复文档同步（PTY panelId 契约 + sandbox 加载时序）`

### agent 文件分工

单 agent（`docs-sync`）：先读 `git log -3 --stat` 与两个 Stage 的实际 diff 再动笔，禁凭记忆写文档。

| 文件 | 同步内容 |
|------|---------|
| `src/ipc/CLAUDE.md` | `pty.ts` 封装的 write/resize/kill 签名含 panelId；契约断裂教训一句话 |
| `src/panels/CLAUDE.md` | 调用点描述同步（如文件表中 pty.write/kill 提及处） |
| `src/workspace/CLAUDE.md` | 「页面切换流」写入 setProjectRoot 前置时序（架构决策节） |
| `e2e-tests/CLAUDE.md` | `__slterm_e2e_createProject`/`__slterm_e2e_switchToPage` async 化 + setProjectRoot 先行 |
| `src-tauri/src/fs/CLAUDE.md` | 记录 sandbox 对前端时序要求：任何 fs/git/notify/pty 消费方须保证 project_root 已设置 |
| `.claude/test-inventory.md` | 新增用例（DBG-4 守卫 + DBG-9/10）计数 |

验证项：`workflows/verify/stage-03.md`。测试门禁同 Stage 1。

## 5. 执行规则（SKILL Step 5 要点）

1. **脚本调用**：`Workflow({ scriptPath: 'docs/debug/workflows/stage-NN-*.js' })`；首次调用前已做语法预检（见 §6）。
2. **verify 失败** → `Workflow({ scriptPath: 'docs/debug/workflows/fix-loop.js', args: { stage, failedItems, fixContext, verifyFile, constraints } })`，最多 3 轮，超过报告用户。
3. **no-return 分流**：verify 阶段 no-return → 主 agent inline spawn 单个 verify agent（复用脚本 prompt+schema+verify 文件+testResult）；重构/测试阶段 → `TaskStop` + `resumeFromRunId`。
4. **git add 路径限定**：`git add src/ e2e-tests/ docs/debug/`，commit 前 `git status` 甄别预期外改动；**禁止 `git add -A`**。
5. **时间盒**：后台运行每 15-20 分钟查 /workflows；单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户。
6. **并行纪律**：同 Stage 两 agent 文件零重叠（分工表为证）；并行 agent 不跑测试，统一由全量测试 agent 单点跑。
7. **禁区**：`compute_conpty_flags` 固定 0x7，任何 agent 不得修改 ConPTY flags（含 4 条守卫测试）。

## 6. 执行前准备（已完成）

- [x] checklist 与 stages 计划落盘（本文件 + `debug-checklist.md`）
- [x] 4 个 workflow 脚本 + 3 个 verify 断言文件落盘
- [x] 4 个 JS 脚本语法预检通过（`new Function` 仅编译不执行，命令见 SKILL 5.1）
- [ ] 用户确认启动 Stage 1

## 7. 收尾（全部 Stage 完成后）

1. 最终全量验证：`npx tsc --noEmit && npx eslint src/ && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && npm test && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
2. `npm run build:e2e` 验证打包图（含 `e2e-tests/helpers.ts`）；可选 `npm run wdio` 跑 L4
3. 人工实测清单（同两 Stage 人工验证点汇总，含 claude TUI 滚轮/输入）
4. 归档 commit：`docs: debug 修复计划与报告归档`（`git add docs/debug/`）
5. 收尾报告：Stage commit 列表、最终测试用例数、残留项（若有）
