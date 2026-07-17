# Review: 测试用例质量

## 统计概览（静态核对）

| 层级 | 实际用例数 | test-inventory.md 声明 | 差异 |
|---|---|---|---|
| L1 Rust | 195 | 193 | +2 |
| L2 前端 | 1026（将 `it.each`/`for...of` 表驱动调用按 1 条计） | 1016 | +10 |
| L3 xterm headless | 116 | 9 | **+107** |
| L4 E2E | 12 | 12 | 一致 |

> 注：若把 `it.each` 数组中的每个 case 单独计数，前端用例数会更多；inventory 对部分文件按展开 case 计、对部分文件按定义计，口径不统一。

---

## [严重级别:高] L3 测试用例数与清单严重不符

**位置**：`.claude/test-inventory.md:165-172`

**问题**：
- 清单称 L3 只有 **2 文件 / 9 用例**，且只列出 `terminal-serialize.test.ts`（5 条）和 `keyboard.test.ts`（4 条）。
- 实际 `test/terminal/` 下有 **4 个文件**：
  - `terminal-serialize.test.ts`：41 条
  - `keyboard.test.ts`：36 条
  - `ansi-correctness.test.ts`：30 条
  - `osc.test.ts`：9 条
- 合计 **116 条**，清单低估了 90% 以上，会误导决策者认为 L3 覆盖不足。

**建议**：
按实际文件与用例更新 `.claude/test-inventory.md` 的 L3 章节；同步更新顶部总用例数（去重后唯一用例数也应重新计算）。

---

## [严重级别:中] 前端部分文件用例数与清单不一致

**位置**：`.claude/test-inventory.md:7-138`

**问题**：

| 文件 | 本次计数 | 清单数字 | 差异 | 说明 |
|---|---|---|---|---|
| `src/__tests__/colors.test.ts` | 9 | 61 | -52 | 清单按 `it.each` 展开项计数，本次按定义数计数 |
| `src/__tests__/forward-global-shortcuts.test.ts` | 28 | 6 | +22 | 清单仅统计旧 contentDocument 路径，未含注入脚本结构验证（22 条） |
| `src/__tests__/html-panel.test.tsx` | 36 | 21 | +15 | 清单未统计 E10–E14 等边界与 postMessage 桥用例 |
| `src/__tests__/use-xterm-lifecycle.test.ts` | 72 | 74 | -2 | 微小差异，可能清单将 `it.each` 数组展开计数 |
| `src/__tests__/keystroke.test.ts` | 18 | 26 | -8 | 清单按 `format∘parse` 字符串数组（8 项）展开计数 |
| `src-tauri/src/git/mod.rs` | 61 | 58 | +3 | 清单未计入 3 条新增测试 |

此外，以下实际存在的前端测试文件未在清单中列出：
- `src/__tests__/e2e-build-config.test.ts`
- `src/__tests__/e2e-enabled.test.ts`
- `src/__tests__/e2e-gating-terminal.test.ts`
- `src/__tests__/e2e-gating-workspace.test.tsx`
- `src/__tests__/inject-script.test.ts`

**建议**：
统一 inventory 计数口径（建议按 Vitest 实际展开的测试用例数），并补充缺失文件。

---

## [严重级别:高] 核心 PTY 命令与生命周期无单元/集成测试

**位置**：
- `src-tauri/src/pty/spawn.rs:599-884`（`pty_spawn`/`pty_write`/`pty_resize`/`pty_kill`/`pty_reattach`）
- `src-tauri/src/pty/spawn.rs:891-964`（Job Object `add_to_job_object` / `create_and_assign_job`）
- `src-tauri/src/pty/reader.rs:31-153`（`reader_loop` 全 I/O 编排）
- `src-tauri/src/pty/win_build.rs`（Windows build 号获取逻辑仅在 `lib.rs` 包装层间接验证）

**问题**：
- `spawn.rs` 的 13 条单元测试全部集中在 `conpty_custom` 子模块（flags、常量、`ConPtyMaster` trait、`AttrList` 创建），**未触及 5 个 Tauri 命令本身**。
- 以下关键生产路径无任何自动化测试：
  - `pty_spawn` 中的 SPAWN_LOCK 串行化、`extra_envs` 环境变量注入（`COLORTERM/TERM/TERM_PROGRAM`）、CPR 注入、cwd 反斜杠规范化、Channel + ring buffer 创建、reader 线程启动。
  - `pty_kill` 的 `spawn_blocking` 中 `kill → join reader → drop master` 路径。
  - `pty_reattach` 的 Channel 替换、ring buffer 回放、Exit 事件补发。
  - `pty_write`/`pty_resize` 的 session 查找、writer/master 锁定流程。
  - Windows Job Object 创建、`KILL_ON_JOB_CLOSE` 设置、子进程分配。
- `reader_loop` 的三个 match 分支（`Ok(0)` EOF、`Ok(n)` 数据、`Err(e)` 读错误）均依赖 `Mutex/RwLock/Channel` 与系统调用，未做单元测试；文件内注释已承认“已尽力”。
- `pty_integration_tests.rs` 仅验证 cmd.exe 基本 roundtrip、resize、kill、自定义 ConPTY spawn，**未覆盖** `pty_reattach`、ring buffer 回放、DA1 实际注入、Job Object 效果。

**建议**：
- 对 `pty_spawn`/`pty_kill`/`pty_reattach` 增加基于 mock `AppState` + 假 Channel 的单元测试（至少验证 env 注入、ring buffer 回放、session 移除不级联卡死）。
- 在 `pty_integration_tests.rs` 增加 `pty_reattach` 端到端用例：断开 Channel → 累积输出 → reattach → 验证回放。
- 对 Job Object 增加至少一个“父进程退出后子进程被清理”的集成测试（可用子进程自监控实现）。

---

## [严重级别:高] `useXterm` 真实编排层大量分支未被测试覆盖

**位置**：
- `src/panels/terminal/useXterm.ts:304-343`（rAF 轮询 + fit + spawn + `term.onData`）
- `src/panels/terminal/useXterm.ts:363-407`（`windowsBuildNumber` effect、字体大小 effect）
- `src/panels/terminal/useXterm.ts:374-388`（可见性切换 WebGL 释放/重建）
- `src/panels/terminal/usePtyOutput.ts:199-234`（直写/合帧分流与 64KB 上限）
- `src/panels/terminal/usePtyResize.ts`（ResizeObserver cleanup、NaN 守卫）

**问题**：
- `useXterm` 测试采用“mock 全部 7 个子 hook”的策略，导致真实 hook 中的编排逻辑大多只验证 mock 调用，属于**测 mock 而非测实现**。
- 未覆盖的真实分支包括：
  - `pollFitAndSpawn` 在 `canFit` 失败或 `fitAddon.fit()` 抛异常时回退 `80×24` 的路径。
  - `term.onData` 把键盘输入经 `pty.write` 发到后端的完整链路。
  - `windowsBuildNumber` 独立 effect 设置 `term.options.windowsPty`。
  - `visible=false` 时释放 `webglAddon`、`visible=true` 时 `tryLoadWebgl() + flushBuffer()`。
  - 字体大小变化触发的 fit + `pty.resize` 异常/无 session 分支。
  - `usePtyOutput` 的 64KB 上限丢弃逻辑、`MAX_PENDING_BYTES` 回环。
  - `usePtyResize` 的 `ResizeObserver.disconnect()` 与 `clearTimeout(resizeTimer)` cleanup。
- `usePtyOutput.ts:199-201` 直写路径不检查 `visibleRef`，导致“非焦点终端仅累积不渲染”的设计文档与实现不一致；测试 NF3 记录了这一偏差，说明实现未满足设计语义。

**建议**：
- 为 `useXterm` 增加“轻 mock”集成测试：保留真实 `Terminal`/`FitAddon`，仅 mock `pty` IPC，验证 rAF 轮询、NaN 回退、`term.onData` 写 PTY、visible 切换 WebGL/flush。
- 对 `usePtyOutput` 单独测试 64KB 上限丢弃、非焦点直写门控（如决定保留直写，则更新文档）。
- 对 `usePtyResize` 验证 cleanup 时 `disconnect()` 与 timer 清理。

---

## [严重级别:高] `titleManager.onDeletePage` 完全未测

**位置**：
- `src/workspace/titleManager.ts:173-176`
- `src/__tests__/title-manager.test.ts`（29 条，无 `onDeletePage` 用例）

**问题**：
- `titleManager` 负责页签标题冲突检测、终端编号、编辑器注册表。`onDeletePage` 在页面删除时清理 registry 与 counters，是防止内存泄漏和标题错乱的边界逻辑，但测试文件中无任何用例覆盖。
- `handleSaveAs` 也仅测了简单另存为，未覆盖另存为后产生同名冲突或目标路径在项目树外的分支。

**建议**：
- 补充 `onDeletePage` 用例：删除页面后再为该页面取终端标题应从 `terminal-0` 重新开始。
- 补充 `handleSaveAs` 冲突分支用例。

---

## [严重级别:中] Rust 集成测试依赖固定睡眠与实际 cmd.exe，存在 flaky 风险

**位置**：
- `src-tauri/tests/pty_integration_tests.rs:28-63`、`77-91`、`96-155`

**问题**：
- `pty_roundtrip` 使用 `std::thread::sleep(Duration::from_millis(800))` 等待 cmd.exe 启动。
- `pty_kill_no_orphan` 使用 `sleep(1000ms)` + 5×200ms 轮询。
- `pty_spawn_custom_conpty` 使用 `sleep(500ms)`。
- 这些测试依赖真实系统 `cmd.exe` 和 ConPTY，在 CI 负载高或 Windows 版本差异时容易超时或不稳定。

**建议**：
- 将固定 sleep 改为基于输出 marker 的轮询（上限保持 10s），减少不必要的等待。
- 如可能，用假 shell 或仅验证 `CreateProcessW` 返回的 PID 来降低对环境依赖。

---

## [严重级别:中] `layout-serde.test.ts` 顶部注释与事实不符

**位置**：
- `src/__tests__/layout-serde.test.ts:3`

**问题**：
- 注释称 `layoutSerde 覆盖率 0%，核心逻辑（旧格式修补、白名单校验、返回成败）完全无测试`。
- 实际该文件有 16 条用例，覆盖了 `component→contentComponent` 迁移、leaf id 生成、`activeGroup` 填充、白名单过滤、`fromJSON` 抛异常回退、深拷贝等。

**建议**：
删除或修正该误导性注释，避免新成员误以为无需补充测试。

---

## [严重级别:中] `layoutSerde` 嵌套布局与已有 `activeGroup` 保留未覆盖

**位置**：
- `src/workspace/layoutSerde.ts:20-67`
- `src/__tests__/layout-serde.test.ts`

**问题**：
- `patchLegacyLayout` 只处理 `grid.root.data` 一层 leaf；对多层 branch 的嵌套布局未覆盖。
- 现有测试有 `activeGroup` 的用例没有显式断言原值未被覆盖。
- 未测 `component` 无效但 `contentComponent` 缺失的白名单分支，以及 `panels` 不是 object 的边界。

**建议**：
- 增加嵌套 branch → leaf 的旧格式修补用例。
- 显式断言已有 `activeGroup` 不被覆盖。

---

## [严重级别:中] `ShortcutRegistry` 监听器生命周期与若干边界未测

**位置**：
- `src/features/shortcuts/ShortcutRegistry.ts:60-72`（`ensureListenerInstalled/Removed`）
- `src/features/shortcuts/ShortcutRegistry.ts:135-138`（`setOverrides(undefined)`）
- `src/features/shortcuts/ShortcutRegistry.ts:151-159`（`exportContextBindings` 受覆盖层影响）
- `src/__tests__/shortcuts.test.ts:44` 条用例

**问题**：
- 没有 spy `window.addEventListener/removeEventListener` 来验证引用计数为 0 时监听器真正被移除。
- `setOverrides(undefined)` 的 `?? {}` 兜底未覆盖。
- `exportContextBindings` 未验证用户覆盖后导出的 keystroke 是否变化。
- 同 context 同键冲突时仅测了 warning，未验证运行时按 priority/栈顺序选 winner。
- `resolve` 未测 handler 返回 `false` 的分支。

**建议**：
- 增加监听器安装/移除的 spy 断言。
- 补充覆盖层对 `exportContextBindings` 影响的用例。

---

## [严重级别:中] `notify/pool.rs` 测试对 stop/Drop 行为断言不足

**位置**：
- `src-tauri/src/notify/pool.rs:93-123`（`pause_all_except` / `evict_lru`）
- `src-tauri/src/notify/pool.rs:289-331`（相关测试）

**问题**：
- `p4_insert_at_capacity_evicts_lru` 只检查池大小，未验证被淘汰 watcher 的线程/句柄已释放。
- `p10_insert_same_path_replaces_old_watcher` 只检查池大小为 1，未验证旧 watcher 已 stop。
- `p9_drop_stops_all_watchers` 仅 drop 后期待不 panic，无资源释放断言。
- `p12_paused_watcher_does_not_process_events` 只检查 `paused` 原子布尔切换，未验证事件真被忽略，测试名与内容不匹配。
- `make_test_watcher` 中的 `stop_tx` 与线程句柄使用的不是同一通道，`FileWatcher::stop()` 对该测试句柄实际上不起作用，削弱了 stop/Drop 相关测试的可信度。

**建议**：
- 让 `make_test_watcher` 的线程真正监听 `stop_rx`，并在测试中验证 `stop()` / `Drop` 后线程结束。
- 对 LRU 淘汰和替换用例增加“旧 watcher `is_running()` 变 false”的断言。

---

## [严重级别:中] 部分测试断言过宽或条件断言存在假阳性风险

**位置**：
- `src/__tests__/use-xterm-output.test.ts:984-1007`、`1197-1228`
- `src/__tests__/use-xterm-output.test.ts:811-834`（NF3 标题与断言不一致）
- `src/__tests__/keystroke.test.ts:112-121`（`format∘parse` 用 `for...of` 循环但 inventory 计为 26 条，脚本静态计数为 18 条）
- `src/__tests__/colors.test.ts`、`language-mapping.test.ts`、`file-viewer-registry.test.ts` 等（使用 `it.each`）

**问题**：
- 两处 CPF 测试用 `if (writeCalls.length > 0)` 条件断言，若未发生任何写入则自动通过。
- NF3 标题写“visible=false 期间直写路径也被抑制”，实际断言是直写路径**不被**抑制，标题与实现选择矛盾。
- `it.each` / `for...of` 表驱动导致静态计数与 inventory 计数口径不一致，`keystroke.test.ts` 实际 18 条而 inventory 写 26 条；`colors.test.ts` 实际 4 条 `it(` 而 inventory 写 61 条（按 each case 计数）。

**建议**：
- 将条件断言改为 `expect(writeCalls.length).toBeGreaterThan(0)` 后再断言内容。
- 统一 inventory 计数口径（按实际 `it`/`test`/`it.each` 产生的测试用例数），或注明“静态行数 vs 展开用例数”。
- 将 NF3 标题改为与实现一致，或在实现中按文档补上门控。

---

## [严重级别:中] 大量 `waitFor` 未显式设置超时

**位置**：
- `src/__tests__/close-handler.test.ts:173,317`
- `src/__tests__/editor-confirm.test.ts:131,216,221,264,268`
- `src/__tests__/e2e-gating-terminal.test.ts:167,180,201,214`
- `src/__tests__/explorer-delete.test.tsx:185,217,296,303,309,326,334,382,397,415,430,448,463`
- `src/__tests__/explorer-file-viewer.test.tsx`（多处）
- `src/__tests__/explorer-git-status.test.tsx:128,148,160,172,205,214,230,244,257,273,291,308,364,388,440,472,479,493,502,516`
- `src/__tests__/explorer-notify.test.tsx:254,287,301,324,370`
- `src/__tests__/explorer-refresh-preserve.test.tsx`（多处）

**问题**：
- 上述位置大量使用 `await waitFor(() => ...)` 未传 `{ timeout: ... }`。
- Vitest 默认 `waitFor` 超时为 1000ms。在 CI 负载高或异步 IPC mock 延迟时，可能因刚好超时而 flaky。

**建议**：
- 对涉及异步 IPC / DOM 渲染的 `waitFor` 显式设置合理的超时（如 2000-5000ms），并在断言失败时给出清晰消息。

---

## [严重级别:中] 多处 debounce/定时器测试使用真实 `setTimeout`

**位置**：
- `src/__tests__/use-xterm-output.test.ts`：大量使用 `await new Promise((r) => setTimeout(r, 150))` 测试 Idle/Max 双定时器合帧。
- `src/__tests__/use-xterm-lifecycle.test.ts`：使用 `setTimeout(r, 150)` 测试 ResizeObserver 100ms debounce。
- `src/__tests__/use-file-tree.test.ts`：`setTimeout(..., 200)` / `setTimeout(..., 300)` 模拟异步延迟与 generation 竞争。
- `src/__tests__/explorer-notify.test.tsx`：`setTimeout(r, 50)` 跨越 fs-event 200ms debounce。
- `src/__tests__/html-panel.test.tsx`：多处 `setTimeout(r, 10)` 等待 postMessage / setState。

**问题**：
- 真实 timer 让测试执行时间变长，且易受调度抖动影响，容易出现 flaky。
- 同类 debounce 测试（`font-size.test.ts`、`keybindings.test.ts`、`projects.test.ts`、`explorer-refresh-preserve.test.tsx`）已正确使用 `vi.useFakeTimers()`，说明项目内有成熟模式但未推广。

**建议**：
- 将上述文件改为 `vi.useFakeTimers()` + `vi.advanceTimersByTime()` 控制时间，断言定时器触发次数与刷新内容。

---

## [严重级别:中] 全局 mock 存在跨测试泄漏风险

**位置**：
- `src/__tests__/setup.ts:58`：`vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)`
- `src/__tests__/setup.ts:61-64`：`vi.mock("../ipc/notify", () => ({ onFsEvent: () => () => {}, startWatch: () => Promise.resolve() }))`

**问题**：
- `ipc/notify` 全局 mock 对所有测试生效。`ipc-contract.test.ts`、`editor-confirm.test.ts` 等通过 `vi.mock("../ipc/notify", importOriginal)` 显式覆盖；若新测试忘记覆盖，会拿到空 mock，可能隐藏真实 bug。
- `HTMLCanvasElement.prototype.getContext` 被全局替换。`detect-webgl.test.ts` 用 `vi.spyOn(...)` 再次覆盖，若某测试 `mockRestore` 后未恢复 setup 的 mock，canvas 会回退到真实 jsdom（未实现报错）。

**建议**：
- 在 `setup.ts` 注释中增加警告，说明全局 notify mock 必须在需要真实行为的测试文件内显式覆盖。
- 对 canvas spy 使用 `beforeAll`/`afterAll` 包装或在 setup 中返回 restore 函数，降低泄漏风险。

---

## [严重级别:低] 模块级 `src/__tests__/CLAUDE.md` 用例数也不一致

**位置**：
- `src/__tests__/CLAUDE.md:10`：`html-panel.test.tsx` 写 39 条，实际 36 条。
- `src/__tests__/CLAUDE.md:38`：`csp-config.test.ts` 写 6 条，实际 4 条。

**问题**：
模块级测试文档同样未随用例增减同步更新，会误导维护者。

**建议**：
同步修正 `src/__tests__/CLAUDE.md` 中的用例数。

---

## [严重级别:低] `git/mod.rs` 测试通过副本验证 diff 行回调逻辑

**位置**：
- `src-tauri/src/git/mod.rs:1327-1389`（`compute_diff_hunks` 辅助函数与生产代码逻辑重复）

**问题**：
- 测试内嵌了一个 `compute_diff_hunks` 副本来验证 hunk 合并行为，而不是直接调用 `git_diff` 命令包装。若生产代码修改但副本未同步，测试会绿但实际行为已变。

**建议**：
将 hunk 合并逻辑提取为可单独测试的纯函数，测试直接调用该函数，而非维护副本。

---

## [严重级别:低] `fs/mod.rs` / `settings.rs` 命令包装层未直接测试

**位置**：
- `src-tauri/src/fs/mod.rs:32-258`
- `src-tauri/src/settings.rs:34-100`

**问题**：
- `fs` 与 `settings` 测试内联模拟了命令内的 CRLF/JSON 逻辑，但未调用 `fs_read_file`/`fs_write_file`/`save_settings`/`load_settings` 命令包装本身。
- 路径 sandbox 校验、`spawn_blocking` task join 错误分支均未覆盖。

**建议**：
- 对命令包装函数本身增加单元测试，至少验证参数透传与错误映射。
- 补充路径 sandbox 校验用例。

---

## [严重级别:低] E2E 快捷键用例无法断言真实 OS 按键效果

**位置**：
- `e2e-tests/test.e2e.ts:152-232`（终端 Ctrl+Shift+V）
- `e2e-tests/test.e2e.ts:443-556`（编辑器 Ctrl+S）
- `e2e-tests/test.e2e.ts:559-622`（HTML Ctrl+W）

**问题**：
- embedded WDIO 驱动无法将 OS 级按键投递进 WebView2 页面，因此 E2E 用例通过 `dispatchEvent` 合成 keydown 或在页面内调用 helper 写标记来验证。
- 虽然这验证了应用内 handler/IPC/写盘链路，但**未覆盖从 OS 键盘到 WebView2 的真实输入路径**；若该层回归（如 WebView2 加速键拦截变化），E2E 无法发现。

**建议**：
- 在文档中明确标注 E2E 键盘测试的局限性（当前已有部分注释，可在测试清单中也说明）。
- 考虑增加一个使用真实 `browser.keys` 的最小用例作为“冒烟”，即使其稳定性较低也可单独标记为 optional。

---

## 实际 review 过的文件清单

### 后端（Rust）
- `src-tauri/src/pty/spawn.rs`
- `src-tauri/src/pty/reader.rs`
- `src-tauri/src/pty/shell.rs`
- `src-tauri/src/pty/win_build.rs`
- `src-tauri/src/pty/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/notify/mod.rs`
- `src-tauri/src/notify/pool.rs`
- `src-tauri/src/git/mod.rs`
- `src-tauri/src/fs/mod.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/pty_integration_tests.rs`

### 前端（TypeScript）
- `src/panels/terminal/useXterm.ts`
- `src/panels/terminal/usePtyOutput.ts`
- `src/panels/terminal/usePtyResize.ts`
- `src/panels/terminal/useCommandDetection.ts`
- `src/panels/terminal/useFontSizeBridge.ts`
- `src/panels/terminal/useTerminalInstance.ts`
- `src/panels/terminal/webgl.ts`
- `src/workspace/layoutSerde.ts`
- `src/__tests__/layout-serde.test.ts`
- `src/workspace/titleManager.ts`
- `src/__tests__/title-manager.test.ts`
- `src/features/shortcuts/ShortcutRegistry.ts`
- `src/__tests__/shortcuts.test.ts`
- `src/__tests__/setup.ts`
- `src/__tests__/csp-config.test.ts`
- `src/__tests__/html-panel.test.tsx`
- `src/__tests__/keystroke.test.ts`
- `src/__tests__/colors.test.ts`
- `e2e-tests/test.e2e.ts`

### 清单/文档
- `.claude/test-inventory.md`
- `src/__tests__/CLAUDE.md`
- `test/terminal/terminal-serialize.test.ts`
- `test/terminal/keyboard.test.ts`
- `test/terminal/ansi-correctness.test.ts`
- `test/terminal/osc.test.ts`
