# 重构阶段划分（Step 4 产物）

> 配套清单：`docs/refactoring-checklist.md`（77 个修复项，ID 唯一）。
> 本文档定义 11 个 Stage 的执行计划，供 Step 5（Workflow 执行）与 Step 6（收尾）直接消费。

## 执行规则（Step 5 约束，每 Stage 必须遵守）

1. **每 Stage 一个 Workflow**：并行重构 →（可选）串行重构 → 全量测试 → 逐项验证，模板 `.claude/skills/systematic-changes/templates/stage-workflow.js`。
2. **单 Stage 并行 agent ≤ 5**；改动同一文件的项并入同一 agent 或串行 pipeline，**不允许两个并行 agent 改同一文件**。
3. **全量测试命令**（每 Stage 必跑，来自 `config.json` 的 `fullCheck`）：
   ```
   npx tsc --noEmit && npx eslint src/ && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && npm test && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
   ```
   （L1 必须 `--test-threads=1`，ConPTY 并发 spawn 死锁。）
4. **逐项验证**：验证 agent 按本文件每 Stage 的「验证项」逐条检查，按 `verify-schema.json` 返回 `{ allFixed, failedItems, details }`。
5. **修复循环**：`allFixed=false` → 启动修复 Workflow（模板 `fix-workflow.js`），最多 **3 轮**；超过则人工介入。
6. **提交**：`allFixed=true` → `git commit -m "refactor: StageN — <描述>"` → 进入下一 Stage。Stage 间严格串行，直接 main 分支，不用 worktree。
7. **范围纪律**：每个 agent 只改本文件分配给自己的项，不顺手改无关代码（surgical changes）；发现新问题记录到 commit message 末尾，不现场修。

## Stage 总览

| Stage | 描述 | 项数 | 并行 agent | 涉及主要文件 |
|-------|------|------|-----------|-------------|
| 1 | 路径沙箱启用 + capabilities 收紧 | 3 | 1（串行） | state.rs、fs/mod.rs、lib.rs、capabilities、projects.ts、ipc/ |
| 2 | spawn.rs 安全与异步化 | 8 | 2 | spawn.rs、shell.rs |
| 3 | spawn.rs 低危打磨 + reader.rs | 6 | 2 | spawn.rs、reader.rs |
| 4 | 后端 fs/git/settings/notify | 8 | 3 | fs/mod.rs、git/mod.rs、settings.rs、notify/mod.rs |
| 5 | 前端高危：编辑器竞态 + HTML 注入 | 4 | 2 | useCodeMirror.ts、HtmlPanel.tsx、injectScript.ts |
| 6 | 终端 hook 架构（约束 #8） | 7 | 2 | useXterm/usePtyOutput/usePtyResize/TerminalRegistry/useTerminalInstance |
| 7 | 前端中危修复 | 11 | 5 | Workspace/PageDockviewHost/main/useFileTree/ExplorerPanel 等 |
| 8 | 前端低危清理（约束 #6） | 5 | 5 | panelRegistry/shortcuts/SidebarTree/FileTree/colors |
| 9 | 核心路径补测 | 6 | 5 | spawn.rs 测试、use-xterm-integration、title-manager 等 |
| 10 | 测试质量整改 | 6 | 4 | 各测试文件、setup.ts、pool.rs |
| 11 | 文档同步 | 13 | 3 | 全部 CLAUDE.md、test-inventory.md |

**依赖顺序说明**：Stage 2/3 都改 `spawn.rs`、Stage 9/10 都改 `pty_integration_tests.rs` 与 `use-xterm-output.test.ts`——Stage 间串行 + 每 Stage commit，无冲突。Stage 11 必须最后（文档反映最终代码）。Stage 9 依赖 Stage 2/3/6 的最终代码形态（补测针对改后实现）。

---

## Stage 1 — 路径沙箱启用 + capabilities 收紧（3 项）

**改动项**：SEC-01、SEC-06、DOC-05c

**Agent 划分**（1 个串行 agent，改动跨前后端但逻辑强耦合）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| sandbox-agent | SEC-01 + SEC-06 + DOC-05c | `state.rs`、`fs/mod.rs`（fs_read_dir 补校验）、`lib.rs`（注册 `set_project_root`）、`capabilities/default.json`（放行新命令 + 移除 `wdio-webdriver:default`）、`src/ipc/fs.ts`（setProjectRoot wrapper）、`src/stores/projects.ts`（切换项目时调用） |

**SEC-01 实现要点**：`validate_path_within_root` 相对路径先以 project_root 为基准拼绝对再 `dunce::canonicalize`；目标不存在拒绝；移除"None 跳过"（`#[cfg(test)]` 豁免保留）；`fs_read_dir` 加 `State<AppState>` 参数并校验。DOC-05c 顺带：`JobHandle` 非 Windows 零占位始终初始化（消除 E0063）。

**验证项（__VERIFY_ITEMS__）**：
- SEC-01a：`grep "set_project_root" src-tauri/src/lib.rs` 在 generate_handler! 中；`capabilities/default.json` 含该命令
- SEC-01b：`grep "validate_path_within_root" src-tauri/src/fs/mod.rs` 中 fs_read_dir 已调用
- SEC-01c：state.rs 中不存在 `None => return Ok(())` 跳过逻辑（cfg(test) 豁免除外）
- SEC-06：`capabilities/default.json` 不含 `wdio-webdriver`
- DOC-05c：state.rs 的 job_object 字段无 `#[cfg(windows)]` 条件初始化残留

**commit**：`refactor: Stage1 — 启用路径沙箱（set_project_root）+ 移除生产 wdio 权限`

---

## Stage 2 — spawn.rs 安全与异步化（8 项）

**改动项**：SEC-02、SEC-07、SEC-08、BE-01、BE-02、BE-11、BE-12、BE-14

**Agent 划分**（2 个并行，文件不重叠）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| pty-async-agent | SEC-02（spawn 侧）+ SEC-08 + BE-01 + BE-02 + BE-12 + BE-14 | `src-tauri/src/pty/spawn.rs`、`state.rs`（PtySession 加 panelId 字段） |
| shell-agent | SEC-02（shell 白名单侧）+ SEC-07 + BE-11 | `src-tauri/src/pty/shell.rs` |

**要点**：BE-01 对齐 `pty_kill` 现有 async+spawn_blocking 模式；BE-12 锁收窄到 `create_conpty_pair`+`spawn_conpty_child`；SEC-08 在 PtySession 记录 panelId，write/resize/kill 校验。**注意**：改 `compute_conpty_flags` 不在本 Stage 范围（flags 固定 0x7 勿动）；shell.rs 测试 7 条必须全绿。

**验证项**：
- BE-01：`grep "pub async fn pty_spawn\|pub async fn pty_write\|pub async fn pty_resize\|pub async fn pty_reattach" spawn.rs` 四处命中 + `spawn_blocking` 出现
- SEC-02：shell.rs 存在白名单校验函数，`resolve_shell_info` 拒绝白名单外路径
- SEC-08：`grep "panel_id" state.rs` PtySession 含归属字段
- BE-02：`add_to_job_object` 失败路径有 `kill()` 调用
- BE-12：SPAWN_LOCK guard 作用域不含 `take_writer`/CPR 注入

**commit**：`refactor: Stage2 — PTY 命令异步化 + shell 白名单 + session 归属校验`

---

## Stage 3 — spawn.rs 低危打磨 + reader.rs（6 项）

**改动项**：BE-07、BE-08、BE-09、BE-10、BE-16、BE-13

**Agent 划分**（2 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| spawn-hardening-agent | BE-07~10 + BE-16 | `src-tauri/src/pty/spawn.rs` |
| reader-agent | BE-13 | `src-tauri/src/pty/reader.rs` |

**要点**：BE-13 修改后必须跑全部 strip 相关测试（pty/CLAUDE.md 修改注意事项 #5）；BE-10 改 Vec 后确认 `conpty_custom::tests` 13 条全绿。

**验证项**：
- BE-07：`WaitForSingleObject` 返回值有 `WAIT_OBJECT_0` 显式比较
- BE-09：spawn.rs 无 `expect("...lock poisoned...")` 残留
- BE-10：`build_env_block` 无 HashMap 迭代
- BE-13：reader.rs 启动剥离在"本轮全为启动序列"时不置 `startup_drained`
- BE-16：`grep -c "SAFETY" spawn.rs` ≥ 6

**commit**：`refactor: Stage3 — ConPTY 错误处理加固 + 启动剥离跨边界修复`

---

## Stage 4 — 后端 fs/git/settings/notify（8 项）

**改动项**：SEC-04、BE-03、SEC-05、BE-06、BE-15、BE-05、TE-13、TE-14

**Agent 划分**（3 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| fs-agent | SEC-04 + BE-03 + TE-14（fs 部分） | `src-tauri/src/fs/mod.rs` |
| git-agent | SEC-05 + BE-06 + TE-13 | `src-tauri/src/git/mod.rs` |
| settings-notify-agent | BE-05 + TE-14（settings 部分）+ BE-15 | `src-tauri/src/settings.rs`、`src-tauri/src/notify/mod.rs` |

**要点**：TE-13 把 `compute_diff_hunks` 副本消除改为调纯函数；git 测试必须用 `dunce::canonicalize`（8.3 短名坑，见 git/CLAUDE.md）；TE-14 新增用例计入 L1 总数（Stage 11 文档同步依据）。

**验证项**：
- SEC-04：fs_rename 目标为目录返回 Err；为文件先 remove_file
- SEC-05：git_status/git_diff 入口有 `validate_path_within_root`
- BE-06：`get_or_open_repo` 无 `workdir.starts_with(&search)` 分支
- BE-15：`notify_watch` 中 `FileWatcher::start` 在池锁外
- TE-13：`grep "compute_diff_hunks" git/mod.rs` 仅剩 1 处（纯函数本体）

**commit**：`refactor: Stage4 — fs/git/settings/notify 安全与正确性修复`

---

## Stage 5 — 前端高危：编辑器竞态 + HTML 注入（4 项）

**改动项**：FE-01、FE-19、FE-02、SEC-03

**Agent 划分**（2 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| editor-agent | FE-01 + FE-19 | `src/panels/editor/useCodeMirror.ts` |
| html-agent | FE-02 + SEC-03 | `src/panels/html/HtmlPanel.tsx`、`src/lib/injectScript.ts`、`src/__tests__/inject-script.test.ts`、`src/__tests__/html-panel.test.tsx` |

**要点**：FE-01 复用 useFileTree 的 generation 模式；SEC-03 需实测 WebView2 中 srcdoc iframe 的 `e.origin` 值（预期 `"null"`），在代码注释记录结论；FE-02 转义在 `injectScript` 纯函数内完成并补测试。

**验证项**：
- FE-01：useCodeMirror.ts 存在 gen 比对（`genRef.current !== gen` 或 AbortController）
- FE-02：injectScript.ts 含 `</script>` 转义逻辑 + 对应测试
- SEC-03：HtmlPanel.tsx `handleMessage` 校验 `e.origin`，注入脚本无 `postMessage(..., "*")`
- FE-19：repoDir 计算无 `slice(0, lastIndexOf("/"))` 裸逻辑

**commit**：`refactor: Stage5 — 编辑器加载竞态修复 + HTML 注入面收敛`

---

## Stage 6 — 终端 hook 架构（约束 #8）（7 项）

**改动项**：FE-23、FE-08、TE-16、FE-17、FE-14、FE-15、FE-12

**Agent 划分**（2 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| session-agent | FE-23 + FE-08 + TE-16 + FE-17 | `useXterm.ts`、`usePtyOutput.ts`、`usePtyResize.ts`、`TerminalRegistry.ts` |
| instance-agent | FE-14 + FE-15 + FE-12 | `useTerminalInstance.ts`、`useFontSizeBridge.ts`（删除） |

**要点**：FE-23 是高风险重构——删除 `sessionIdRef` 后 write/resize/exit 全改经 `TerminalRegistry.get(panelId)`；同步更新 panels/CLAUDE.md 引用的文件表（Stage 11 统一做亦可）；use-xterm 系列测试（111 条）必须全绿，TE-16 的 NF3 断言随行为修正。

**验证项**：
- FE-23：`grep "sessionIdRef" src/panels/terminal/` 无命中；write/resize 经 Registry 取 sessionId
- TE-16：usePtyOutput 直写路径有 visibleRef 门控
- FE-12：`useFontSizeBridge.ts` 文件不存在、无 import 残留
- FE-17：`getAll` 返回副本
- FE-14/15：fonts.ready 有取消标志；无重复 dispose 代码块

**commit**：`refactor: Stage6 — 会话元数据单点化（约束 #8）+ 终端 hook 清理`

---

## Stage 7 — 前端中危修复（11 项）

**改动项**：FE-03、FE-04、FE-05、FE-06、FE-07、FE-09、FE-10、FE-16、FE-18、FE-22、DOC-10

**Agent 划分**（5 个并行，文件不重叠）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| workspace-agent | FE-03 | `src/workspace/Workspace.tsx` |
| dockview-agent | FE-04 + FE-22 | `src/workspace/PageDockviewHost.tsx` |
| bootstrap-agent | FE-05 + FE-18 | `src/main.tsx`、`src/global.d.ts`（新增）、`src/lib/ErrorBoundary.tsx` |
| explorer-agent | FE-06 + FE-07 | `src/features/explorer/useFileTree.ts`、`ExplorerPanel.tsx` |
| misc-agent | FE-09 + FE-10 + FE-16 + DOC-10 | `TerminalPanel.tsx`、`layoutSerde.ts`、`ipc/notify.ts`、`types/notify.ts` |

**验证项**：
- FE-03：Workspace.tsx render 函数体无 `pageCallbacksRef.current.delete/set`
- FE-04：PageDockviewHost 有 disposable 收集 + cleanup dispose
- FE-05：main.tsx setInterval 有最大次数与 clearInterval
- FE-07：ExplorerPanel addPanel 包裹 try-catch 且成功后才注册标题
- FE-16：两处 `detail` 可选性一致

**commit**：`refactor: Stage7 — 前端生命周期与竞态修复`

---

## Stage 8 — 前端低危清理（约束 #6）（5 项）

**改动项**：FE-11、FE-13、FE-20、FE-21、FE-24

**Agent 划分**（5 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| registry-agent | FE-11 | `src/panelRegistry.ts` |
| deadcode-agent | FE-13 | `forwardGlobalShortcuts.ts`（删）、`features/shortcuts/index.ts`、`src/__tests__/forward-global-shortcuts.test.ts`（删） |
| sidebar-agent | FE-20 + FE-24（SidebarTree 部分） | `src/features/sidebar/SidebarTree.tsx` |
| filetree-agent | FE-21 + FE-24（FileTree 部分） | `src/features/explorer/FileTree.tsx` |
| theme-agent | FE-24（token 主体） | `src/theme/colors.ts`、`src/App.css` |

**要点**：theme-agent 先定 token 名（如 `GIT_GUTTER_COLORS` 同款风格新增 `APP_BG_PRIMARY` 等 + `SHADOW_MENU`），sidebar/filetree agent 按约定 token 名引用——同 Stage 并行不冲突（不同文件）。

**验证项**：
- FE-24：`grep -rn "#1e1e2e\|#2b2b3c\|rgba(0,0,0,0.5)" src/ --include="*.tsx" --include="*.css"` 无命中（colors.ts 除外）
- FE-13：forwardGlobalShortcuts.ts 与其测试文件不存在、index.ts 无 re-export
- FE-20：SidebarTree.tsx 函数体内无组件定义
- FE-21：FileTree.tsx 无裸 `8 + (depth + 1) * 16` 表达式

**commit**：`refactor: Stage8 — 配色单点化（约束 #6）+ 死代码删除`

---

## Stage 9 — 核心路径补测（6 项）

**改动项**：TE-01、TE-02、TE-03、TE-05、TE-06、TE-07

**Agent 划分**（5 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| pty-test-agent | TE-01 | `src-tauri/src/pty/spawn.rs`（`#[cfg(test)]`）、`src-tauri/tests/pty_integration_tests.rs` |
| xterm-test-agent | TE-02 | `src/__tests__/use-xterm-integration.test.ts`（新增） |
| title-test-agent | TE-03 | `src/__tests__/title-manager.test.ts` |
| layout-test-agent | TE-05 + TE-06 | `src/__tests__/layout-serde.test.ts` |
| shortcut-test-agent | TE-07 | `src/__tests__/shortcuts.test.ts` |

**要点**：TE-01 的 Rust 测试遵守 `--test-threads=1` 与 SPAWN_LOCK；新增用例数记录到 commit message（供 Stage 11 更新 test-inventory）；TE-02 参照现有 use-xterm 测试的 `vi.hoisted()` 模式但只 mock `ipc/pty`。

**验证项**：
- TE-01：spawn.rs 测试模块含 env 注入/cwd 规范化/ring buffer 回放断言；集成测试含 reattach 用例
- TE-02：新文件存在且 `npm test` 通过；断言覆盖 80×24 回退、onData→pty.write、visible 切换
- TE-03：title-manager.test.ts 含 onDeletePage 用例
- TE-05：`grep "覆盖率 0%" layout-serde.test.ts` 无命中
- TE-07：shortcuts.test.ts 含 addEventListener spy 断言

**commit**：`refactor: Stage9 — 核心路径测试补齐（PTY 命令/useXterm 编排/titleManager）`

---

## Stage 10 — 测试质量整改（6 项）

**改动项**：TE-04、TE-08、TE-09、TE-10、TE-11、TE-12

**Agent 划分**（4 个并行）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| rust-test-agent | TE-04 + TE-08 | `src-tauri/tests/pty_integration_tests.rs`、`src-tauri/src/notify/pool.rs`（测试模块） |
| xterm-test-fix-agent | TE-09 + TE-11（use-xterm 部分） | `src/__tests__/use-xterm-output.test.ts`、`use-xterm-lifecycle.test.ts` |
| dom-test-fix-agent | TE-10 + TE-11（DOM 部分） | `close-handler/editor-confirm/e2e-gating/explorer-*/use-file-tree/html-panel` 等测试文件 |
| setup-agent | TE-12 | `src/__tests__/setup.ts` |

**要点**：TE-10/TE-11 涉及文件多但改动机械（加 timeout 选项、换 fake timers）；改完全量 `npm test` 必须无新增 flaky。

**验证项**：
- TE-04：`grep "thread::sleep" pty_integration_tests.rs` 仅剩轮询内短 sleep
- TE-08：pool.rs 测试中 make_test_watcher 线程监听 stop_rx
- TE-09：use-xterm-output.test.ts 无 `if (writeCalls.length > 0)` 条件断言
- TE-10：抽查 10 处 waitFor 均带 `{ timeout: ... }`
- TE-12：setup.ts 含 notify mock 警告注释

**commit**：`refactor: Stage10 — 测试可信度整改（去 flaky/假阳性）`

---

## Stage 11 — 文档同步（13 项）

**改动项**：DOC-01~09、DOC-11、DOC-12、DOC-13、TE-17（DOC-10 已在 Stage 7 完成）

**Agent 划分**（3 个并行，按文件分组）：

| Agent | 负责项 | 文件 |
|-------|--------|------|
| root-docs-agent | DOC-01（根部分）+ DOC-03 + DOC-04 + TE-17 | 根 `.claude/CLAUDE.md`、`.claude/test-inventory.md` |
| frontend-docs-agent | DOC-02（shortcuts 部分）+ DOC-06~09 + DOC-11 + DOC-12 | `src/` 下各 CLAUDE.md |
| backend-docs-agent | DOC-01（pty 部分）+ DOC-05 + DOC-02（e2e 部分）+ DOC-13 | `src-tauri/src/pty/CLAUDE.md`、`e2e-tests/CLAUDE.md` |

**要点**：DOC-03 用 `grep -cE '^\s*(it|test)\('` 实测各文件用例数后重写 test-inventory.md，统一口径为 vitest 展开用例数，并把 Stage 9 新增用例计入；所有 CLAUDE.md 中的用例数改为引用 test-inventory.md；DOC-01 明确"Tauri 2 自定义命令默认放行"并修改约束 #10 原文。

**验证项**：
- DOC-03：test-inventory.md 中 L3 为 116（含 Stage 9 新增则更多）、总数与各级之和一致
- DOC-01：根 CLAUDE.md 约束 #10 不再要求"逐条 capabilities 放行自定义命令"
- DOC-04：根 CLAUDE.md 无 `claude/` 模块引用
- DOC-09：panels/CLAUDE.md 无 `useFontSizeBridge` 引用
- 全部：`grep -rn "allow-same-origin" **/CLAUDE.md` 描述与现行 sandbox 一致

**commit**：`docs: Stage11 — CLAUDE.md 与 test-inventory 全量同步`

---

## Step 6 — 收尾（全部 Stage 完成后）

1. **最终全量验证**（主 agent 直接执行，非 Workflow）：
   ```
   npx tsc --noEmit && npx eslint src/ && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && npm test && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && npm run test:l3
   ```
   另执行 `npm run e2e`（VITE_E2E=1 构建 + wdio）做 L4 终验。
2. **CLAUDE.md 同步确认**：Stage 11 已统一处理；收尾仅抽查根 CLAUDE.md 是否与最终代码一致。
3. **最终 commit**：`docs: 重构收尾——审查文档归档`（docs/ 下 review-*.md 与 refactoring-*.md 一并提交）。
4. **输出收尾报告**：11 个 Stage 的 commit 列表、最终测试用例数、未修复项（若有，含原因）。

## 风险与注意事项

| 风险 | 缓解 |
|------|------|
| Stage 2 的 BE-01 改动 PTY 核心路径，回归风险高 | pty_integration_tests 5 条 + conpty_custom 13 条必须全绿；改后实测真实 claude TUI（人工验证点，不进 Workflow） |
| Stage 6 的 FE-23 触碰终端写/resize 全链路 | use-xterm 111 条测试全绿为门禁；TE-02（Stage 9）补集成测试 |
| SEC-01 启用沙箱后，沙箱外路径操作将被拒绝（行为变更） | 前端 projects store 在打开/切换项目时立即 set_project_root；E2E 辅助 `__slterm_e2e_createProject` 同样接入 |
| 文档 Stage 的用例数随 Stage 9/10 增删变化 | Stage 11 在最后执行，以实测为准 |
| `compute_conpty_flags` 固定 0x7 不可动 | 所有 Stage prompt 显式声明禁止改 ConPTY flags |
