# Phase 2 修复 Stage 划分（stages）

> 清单真值源：`docs/review-phase2-fix/checklist.md`（37 项，逐项修复步骤）。
> 编排参数：`docs/review-phase2-fix/execution-plan.md`。
> Stage 脚本：`docs/review-phase2-fix/workflows/stage-NN-*.js`；verify 断言：`docs/review-phase2-fix/workflows/verify/stage-NN.md`。

**通用规则**：
- Stage 串行执行、每 Stage commit；Stage 间允许重复碰同一文件（串行 + commit 隔离无冲突）
- 文档同步固定最后 Stage（S10）——文档必须反映所有代码 Stage 完成后的最终状态
- 禁区（写入各脚本 PREAMBLE）：`compute_conpty_flags` 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
- 每 Stage commit 前跑该 Stage 全部门禁（TE-16 根因留痕：收尾 commit 后未全量复跑门禁）
- 并行 agent 不各自跑资源共享型测试（PTY/端口/全局锁）——真实执行统一由全量测试 agent 单点跑

**Stage 顺序依据**：S01 先解 CI 红与 fmt 基线；S02 门禁工具链本身变更（TS7 影响 tsc/eslint 口径）必须先于一切代码 Stage；S03/S04 后端先行（S03 不动 set_project_root 主体，S04 改之——顺序防冲突）；S05/S06 前端切换链（S05 先建 stopWatch，S06 重构同文件链）；S07~S09 独立小修；S10 文档固定最后（反映终态）。

---

## S01 基线 fmt + knip 零误报（TE-16、TE-12、TE-13）

- **改动项**：TE-16、TE-12、TE-13
- **分工**（pipeline 串行——knip 迭代依赖 fmt 后的稳定代码基线，且 B 收尾跑全门禁）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「fmt」 | TE-16 | `src-tauri/src/pty/shell.rs`、`src-tauri/src/pty/spawn.rs`（仅 cargo fmt 格式化） |
| B「knip」 | TE-12、TE-13 | `knip.json`（迭代至退出码 0）；可能删除真死代码文件/导出（逐条备注）；`.github/workflows/ci.yml` 仅核对不改动 |

- **实现要点**：checklist 第 1 节。knip 迭代法按输出分类逐项处理；**无「测试专用」注释的 unused export 不得直接入 ignoreExports**——须判断是否真死代码（真死代码删除并备注）；注册表家族 side-effect import 触发点入 entry（硬约束 #13 形态）
- **验证断言**：
  1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 退出码 0
  2. `npx knip --production` 退出码 0
  3. grep `.github/workflows/ci.yml` 含 `npx knip --production`
  4. `git diff` 显示 shell.rs/spawn.rs 仅格式变化（无逻辑行改动，Read diff 确认）
  5. 全门禁绿：clippy / tsc / eslint / L1 / L2
- **commit**：`fix: cargo fmt 基线修复 + knip 零误报配置（TE-16/12/13）`
- **人工验证点**：无

## S02 依赖升级（TE-06、TE-07、TE-14）

- **改动项**：TE-06、TE-07、TE-14
- **分工**（pipeline 串行——三项同改 package.json/package-lock.json，并行必冲突）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「dialog」 | TE-06 | `package.json`、`package-lock.json` |
| B「typescript」 | TE-07 | `package.json`、`package-lock.json`；若 eslint 不兼容则另升级 typescript-eslint 或加 overrides |
| C「dedupe」 | TE-14 | `package.json`（可能加 overrides）、`package-lock.json` |

- **实现要点**：checklist 第 2 节。B 必须先实查 `npm view typescript-eslint` peerDependencies 再动手（禁凭印象）；先 grep 确认 `@typescript/native` 零消费；C 先 `npm dedupe`，未收敛才加 overrides
- **验证断言**：
  1. `npm ls @tauri-apps/plugin-dialog` 输出 2.7.2
  2. `npm ls typescript` 单版本 7.x，且无 `@typescript/native`/`typescript6` 残留
  3. grep `package.json` 无 `@typescript/native`
  4. `npm ls @wdio/globals expect-webdriverio webdriverio` 各单版本
  5. `npx tsc --noEmit`、`npx eslint src/`、`npm test` 全绿
  6. `npx tauri build --debug --no-bundle` 退出码 0
- **commit**：`fix(deps): dialog 2.7.2 + typescript ^7.0.2 主字段直改 + WDIO dedupe（TE-06/07/14）`
- **人工验证点**：真实产物冒烟——开终端/编辑器/hooks 面板各一次（TS7 是门禁工具链变更，须真实构建兜底）

## S03 后端安全（SEC-15、SEC-17、BE-22、BE-24、BE-25）

- **改动项**：SEC-15、SEC-17、BE-22、BE-24、BE-25
- **分工**（并行 4，文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「shell」 | SEC-15 | `src-tauri/src/pty/shell.rs`、`src-tauri/src/pty/CLAUDE.md` |
| B「state」 | BE-24 | `src-tauri/src/state.rs`（仅 `apply_project_root` Err 臂）、`src-tauri/src/CLAUDE.md` |
| C「notify」 | BE-22、BE-25 | `src-tauri/src/notify/mod.rs`（两处改动同文件） |
| D「hooks」 | SEC-17 | `src-tauri/src/hooks/claude/config.rs`、`src-tauri/src/hooks/CLAUDE.md`、`.claude/test-inventory.md`（豁免行） |

- **实现要点**：checklist 第 3 节（每项含可照抄代码块）。SEC-15 三臂 match：双成功精确比较 / 双失败字符串回退 / 单侧失败拒绝；测试逐条核对现有 paths_match 用例并新增单侧失败拒绝用例
- **验证断言**：
  1. `src-tauri/src/pty/shell.rs` 的 `paths_match` 为三臂 match 且含 `_ => false` 臂（Read 确认）
  2. grep shell.rs 无「安全语义不弱化」残留
  3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿（含新增 `paths_match_single_side_failure_rejected`）
  4. grep `src-tauri/src/hooks/claude/config.rs` 含 `target: "audit"`
  5. grep `src-tauri/src/notify/mod.rs` 含 `eq_ignore_ascii_case` 且 `notify_watch` 校验块经 `spawn_blocking`（Read 确认）
  6. grep `src-tauri/src/state.rs` 含「写锁中毒」warn
  7. clippy + `cargo fmt --check` 过
- **commit**：`fix(security): shell 白名单 fallback 收窄 + watcher 校验异步化/大小写 + 锁中毒可观测 + user 层审计（SEC-15/17、BE-22/24/25）`
- **人工验证点**：真实 claude spawn 三 shell（pwsh/PowerShell/cmd）实测——Win11 本机 + Win10 另一台（SEC-15 收窄可能影响 alias 场景，自动化测试无法覆盖真实 Store 版 pwsh）

## S04 root 竞态 Mutex（SEC-16）

- **改动项**：SEC-16
- **豁免理由**（单 Stage 单项）：P1 核心修复，改 AppState 公共结构 + `set_project_root_impl` 签名 + Cargo feature 三处强耦合，独立 Stage 便于 review 与回滚
- **分工**（单 agent）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「mutex」 | SEC-16 | `src-tauri/src/state.rs`（AppState 字段 + new + impl 签名 + 命令层 + 测试适配）、`src-tauri/Cargo.toml`、`src-tauri/src/CLAUDE.md` |

- **实现要点**：checklist 第 4 节。tokio 补 `"sync"` feature；`set_project_root_impl` 全程持锁（canonicalize+apply 互斥）；新增并发串行化 L1 用例
- **验证断言**：
  1. grep `src-tauri/Cargo.toml` tokio features 含 `"sync"`
  2. grep `src-tauri/src/state.rs` 含 `project_root_lock: tokio::sync::Mutex<()>`
  3. `set_project_root_impl` 首行持锁（Read 确认 `lock.lock().await` 在 canonicalize 之前）
  4. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿（含 `set_project_root_serializes_concurrent_calls`）
  5. clippy + `cargo fmt --check` 过
- **commit**：`fix(state): set_project_root tokio::Mutex 串行化（SEC-16）`
- **人工验证点**：A→B 快速连切页面，旧项目文件操作应被沙箱拒绝（root 不串）

## S05 watcher 生命周期前端（BE-10、FE-38）

- **改动项**：BE-10、FE-38
- **分工**（单 agent——两项同改 Workspace.tsx 同一 SEC-01 effect）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「workspace-effect」 | BE-10、FE-38 | `src/workspace/Workspace.tsx`、`src/__tests__/workspace-switch-order.test.tsx`（或新增用例文件）、`src/workspace/CLAUDE.md` |

- **实现要点**：checklist 第 5 节（含两处可照抄代码块）。BE-10：activePageId 置 null 分支停旧 watcher；FE-38：setProjectRoot 成功后才 startWatch + 过期守卫
- **验证断言**：
  1. effect 含 `if (!activePageId)` 的 stopWatch 分支（Read 确认：置 null 时对 prevRootRef.current 调 stopWatch 并清 ref）
  2. `setProjectRoot(...).then(() => ...)` 链内含过期守卫（Read 确认 then 回调内比较 prevRootRef.current 与目标 root，不等则丢弃）
  3. `startWatch` 仅出现在 setProjectRoot 的 then 回调内（语义断言，不限变量名，须 Read 确认）
  4. `npm test` 全绿（含新增 stopWatch/startWatch 时序用例）
  5. `npx tauri build --debug --no-bundle` 过（关键路径构建级）
- **commit**：`fix(workspace): 空页面 stopWatch + effect await 串行（BE-10、FE-38）`
- **人工验证点**：删末页/移除活跃项目后，旧目录改动不再触发 fs-event（watcher 已停）

## S06 store 纯状态（FE-37、FE-36、BE-23）

- **改动项**：FE-37、FE-36、BE-23
- **分工**（pipeline 串行——A/B 同改 projects.ts）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「上提+toast」 | FE-37、BE-23 | `src/stores/projects.ts`（switchToPage 剥离 + import 清理）、`src/workspace/pageApis.ts`、6 个测试文件适配（`layout-switch.test.ts`、`projects.test.ts`、`workspace-multi-instance.test.tsx` 等直调 switchToPage 者）、`src/__tests__/pageapis.test.ts`、`src/stores/CLAUDE.md` |
| B「全局计数」 | FE-36 | `src/stores/projects.ts`（addPage）、`src/__tests__/projects.test.ts`、`src/stores/CLAUDE.md` |

- **实现要点**：checklist 第 6 节。A 先做（剥离后 switchToPage 纯状态化），B 后做（在剥离后的 addPage 上改全局计数）；生产调用点（Workspace/NavTree）零改动——均经 switchToPageShared 天然含 setProjectRoot
- **验证断言**：
  1. grep `src/stores/` 无 `setProjectRoot` 命中
  2. `projects.ts` addPage 上限校验为跨项目全局计数（语义断言：计数覆盖全部项目的 pages，不限变量名，须 Read 确认）
  3. `pageApis.ts` 的 switchToPageShared catch 含 `toast.show`（Read 确认）
  4. `npm test` 全绿
  5. `npx tauri build --debug --no-bundle` 过
- **commit**：`refactor(stores): switchToPage IPC 上提 + MAX_PAGES 全局化 + 切换失败 toast（FE-37/36、BE-23）`
- **人工验证点**：页面切换后文件树/终端 cwd 正常（setProjectRoot 链路经 switchToPageShared 无回归）；构造 setProjectRoot 失败场景 toast 实测

## S07 错误处理 + nonce（SEC-04、FE-08、FE-10、FE-42、FE-43、FE-44、FE-45）

- **改动项**：SEC-04、FE-08、FE-10、FE-42、FE-43、FE-44、FE-45
- **分工**（并行 3，文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「nonce」 | SEC-04 | `src/panels/html/HtmlPanel.tsx`（仅注释）、`src/panels/CLAUDE.md`、`src/__tests__/command-catalog.test.ts` |
| B「静默 catch」 | FE-08、FE-42、FE-45 | `src/panels/terminal/keyboard.ts`、`src/ipc/window.ts`、`src/stores/fontSize.ts`、`src/stores/keybindings.ts`、`src/stores/sideBar.ts`、`src/stores/projects.ts`（仅 2 处 catch 行）、keyboard/startup 相关测试 |
| C「diff/editor」 | FE-10、FE-43、FE-44 | `src/panels/diff/DiffPanel.tsx`、`src/panels/editor/useCodeMirror.ts`、`src/__tests__/diff-panel-stale-banner.test.tsx`、`src/__tests__/diff-panel.test.tsx`、`src/__tests__/use-code-mirror.test.ts` |

- **实现要点**：checklist 第 7 节。B 注意 projects.ts 仅改 2 处 catch 行（:254/:275），不动其他（S06 已改过的文件，Stage 间串行无冲突）
- **验证断言**：
  1. grep 上述目标文件无 `.catch(() => {})` 与 `} catch {` 空块残留
  2. grep `DiffPanel.tsx` 两处重载 catch 均含 `setDiffStale(true)`
  3. grep `DiffPanel.tsx`/`useCodeMirror.ts` 保存 toast 含 `getErrorMessage`
  4. `command-catalog.test.ts` 含 global 命令集守卫用例（grep 「global.closeTab」）
  5. grep `HtmlPanel.tsx` 无「拿不到本面板 nonce」
  6. `npm test` + `npm run test:l3` 全绿
- **commit**：`fix(frontend): 静默 catch 可观测化 + getErrorMessage 统一 + nonce 威胁模型守卫（SEC-04、FE-08/10/42/43/44/45）`
- **人工验证点**：无

## S08 explorer 增强（FE-39 验证、FE-40、FE-41）

- **改动项**：FE-39（验证项，零改动）、FE-40、FE-41
- **分工**（并行 2，文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「滚动跟随」 | FE-40 | `src/features/explorer/FileTree.tsx`、`src/__tests__/explorer-virtualization.test.tsx` |
| B「空目录行」 | FE-41 | `src/features/explorer/useFileTree.ts`、`src/__tests__/use-file-tree.test.ts` |

- **实现要点**：checklist 第 8 节（含可照抄代码块）。FE-39 零改动——仅 verify 断言既有嵌套用例存在且绿
- **验证断言**：
  1. grep `src/__tests__/nav-tree-history.test.tsx` 含「最深前缀」且 `npx vitest run nav-tree-history` 绿（FE-39）
  2. `FileTree.tsx` 含滚动跟随 effect（语义断言：selectedPath 变化且对应行索引在渲染窗口外时写 scrollTop 使其可见，须 Read 确认）
  3. `useFileTree.ts` 的 refreshSubtreeAt 含「目标已删」分支（Read 确认：readDir 失败且非根路径时从树中移除该目录行）
  4. `npx vitest run explorer-virtualization use-file-tree` 全绿
- **commit**：`fix(explorer): 选中滚动跟随 + 已删目录行清理（FE-40/41；FE-39 验证已固化）`
- **人工验证点**：无

## S09 稳定性（FE-35、FE-46、FE-47、FE-48）

- **改动项**：FE-35、FE-46、FE-47、FE-48
- **分工**（并行 2，文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「死代码+重试」 | FE-35、FE-46 | `src/panelRegistry.ts`、`src/__tests__/panel-registry.test.ts`、`knip.json`（联动清理）、`src/lib/ErrorBoundary.tsx`、`src/__tests__/error-boundary.test.tsx` |
| B「超时+abort」 | FE-47、FE-48 | `src/App.tsx`、`src/__tests__/close-handler.test.ts`、`src/workspace/pageApis.ts`、`src/features/agentHistory/restoreSession.ts`、`src/__tests__/pageapis.test.ts`、`src/__tests__/agent-history-restore.test.ts` |

- **实现要点**：checklist 第 9 节。FE-35 的 knip 联动：若 S01 为 terminalTabConfig 加了 ignoreExports 条目则一并删除
- **验证断言**：
  1. grep 全仓 `terminalTabConfig` 零命中
  2. `ErrorBoundary.tsx` inline variant 含「重试」按钮（Read 确认 onClick 清 error state）
  3. `App.tsx` 的 `ptyKillAll()` 包在 `Promise.race` 总超时内（Read 确认，与上方 Registry kill 同形）
  4. `pageApis.ts` 与 `restoreSession.ts` 两处轮询 setTimeout 均含 abort 清理（Read 确认 clearTimeout + addEventListener("abort")）
  5. `npx knip --production` 仍退出码 0
  6. `npm test` 全绿 + `npx tauri build --debug --no-bundle` 过
- **commit**：`fix(stability): 死代码清除 + ErrorBoundary 重试 + 关窗总超时 + waitFor abort 清理（FE-35/46/47/48）`
- **人工验证点**：多 session 场景关窗实测——总时长有界（不随 session 数线性增长）

## S10 文档同步（FE-31、DOC-11、DOC-12、DOC-13、DOC-14、TE-15）

- **改动项**：FE-31、DOC-11、DOC-12、DOC-13、DOC-14、TE-15
- **分工**（并行 3，文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| A「editor 文档」 | FE-31、DOC-13 | `src/panels/editor/CLAUDE.md`（新建）、`src/panels/CLAUDE.md`（FE-31 迁址交叉引用 + DOC-13 四处数字） |
| B「ipc/sideViews 文档」 | DOC-11、DOC-12、DOC-14 | `src/ipc/CLAUDE.md`、`src/__tests__/ipc-agent-history-contract.test.ts`（仅头注释）、`src/features/sideViews/CLAUDE.md` |
| C「汇总登记」 | TE-15 + 全程登记 | `.claude/adr.md`（D12~D20 + TE-07 结果 + TE-15 债务 + ADR-0009 FE-31 链接）、`.claude/test-inventory.md`（S03~S09 新用例登记）、`.claude/CLAUDE.md`（SEC-15/16/17 入编号索引）、`src/features/hooksConfig/CLAUDE.md`（TE-15 一句） |

- **实现要点**：checklist 第 10 节。C 负责把 S01~S09 执行期产生的登记义务全部收口（新用例入 test-inventory、妥协方案入 ADR）；DOC-13/14 以 test-inventory 为真值源校正
- **验证断言**：
  1. Glob `src/panels/editor/CLAUDE.md` 存在；`src/panels/CLAUDE.md` 无大段编辑器细节残留（编辑器专属节已迁出，留交叉引用）
  2. grep `src/ipc/CLAUDE.md` 无 `setFocus` 残留、含「9 用例」「18 用例」；grep `ipc-agent-history-contract.test.ts` 头注释含 18
  3. grep `src/panels/CLAUDE.md` 四处用例数与 test-inventory 一致（4/6/86/27）
  4. grep `src/features/sideViews/CLAUDE.md` 含 54/40
  5. grep `.claude/adr.md` 含 D12~D20 与「json-schema-library」；grep `.claude/CLAUDE.md` 编号索引含 SEC-15/SEC-16/SEC-17
  6. 全门禁终跑全绿：fmt / clippy / tsc / eslint / L1 / L2 / L3 / knip / e2eBuild
- **commit**：`docs: editor CLAUDE.md 新建 + 用例数校正 + Phase 2 决策/债务登记（FE-31、DOC-11~14、TE-15）`
- **人工验证点**：无（文档类）

---

## 人工验证点汇总（执行收尾统一实测）

| # | Stage | 验证点 |
|---|-------|--------|
| 1 | S02 | TS7 后真实产物冒烟（终端/编辑器/hooks 面板各开一次） |
| 2 | S03 | SEC-15 收窄后真实 claude spawn 三 shell 实测（Win11 本机 + Win10 另一台） |
| 3 | S04 | A→B 快速连切页面沙箱不串（旧项目文件操作被拒） |
| 4 | S05 | 删末页/移除活跃项目后旧目录改动不再触发 fs-event |
| 5 | S06 | 页面切换功能无回归 + setProjectRoot 失败 toast 实测 |
| 6 | S09 | 多 session 关窗总时长有界实测 |
