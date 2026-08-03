# 自动化测试深度 Review 汇总

> 生成日期 2026-08-04 | 15 个领域报告去重汇总 | 各报告详见本目录 01-15 编号文件
> 全量基线：2619 用例（L1 Rust 449 + L2 前端 2020 + L3 116 + L4 34）| 前端插桩行覆盖 87.6%

## 一、总览统计

### 1.1 领域 × 严重级问题数矩阵

| 领域 | 报告编号 | 🔴 严重 | 🟡 中等 | 🟢 可维护 | 小计 |
|------|----------|--------|--------|----------|------|
| L1 PTY + State | 01 | 4 | 9 | 2 | 15 |
| L1 Git | 02 | 4 | 9 | 2 | 15 |
| L1 Hooks | 03 | 4 | 6 | 3 | 13 |
| L1 History + FS + Notify | 04 | 0 | 9 | 2 | 11 |
| L1 Settings + Projects + Error + Lib | 05 | 2 | 5 | 2 | 9 |
| L2 终端面板 | 06 | 2 | 4 | 4 | 10 |
| L2 编辑器 + Diff/GitShow | 07 | 4 | 5 | 3 | 12 |
| L2 工作区/布局 + 启动关闭 | 08 | 4 | 7 | 2 | 13 |
| L2 资源管理器 + 侧栏 | 09 | 2 | 8 | 4 | 14 |
| L2 侧栏视图 + Commit | 10 | 3 | 10 | 2 | 15 |
| L2 Hooks 配置面板 | 11 | 3 | 6 | 3 | 12 |
| L2 快捷键 + 主题 + Store | 12 | 1 | 7 | 6 | 14 |
| L2 IPC + HTML + E2E 门控 | 13 | 1 | 6 | 10 | 17 |
| L2 通知/Agent 状态 + Claude 历史 | 14 | 3 | 6 | 3 | 12 |
| L3 终端 headless + L4 E2E | 15 | 7 | 6 | 2 | 15 |
| **去重前总计** | — | **43** | **103** | **49** | **195** |
| **去重后估计** | — | **~30** | **~70** | **~38** | **~138** |

说明：去重后估计为按“同一根因合并”后的近似值；下文第二章至第四章为逐条去重后的清单，并标注全部来源报告编号。

### 1.2 关键源码覆盖率基线

| 源文件/模块 | 行覆盖 | 主要缺口 | 来源报告 |
|-------------|--------|----------|----------|
| `src-tauri/src/pty/spawn.rs` | 51.8% | Job Object 孤儿防护、pty_spawn 校验、四个 Tauri 命令、spawn_conpty_child 组合 | 01 |
| `src-tauri/src/pty/reader.rs` | 70.5% | reader_loop I/O 编排、非 Windows 分支、部分 OSC/CSI 分支 | 01 |
| `src-tauri/src/git/mod.rs` | 85.5% | 五个 Tauri 命令包装层、conflict 分支、diff hunk 边界 | 02 |
| `src-tauri/src/hooks/` | 75.5% | 信号处理全流程、三个注入命令、watcher 双通道事件循环 | 03 |
| `src-tauri/src/notify/mod.rs` | 既定豁免 | FileWatcher::start / notify_watch 命令无 AppHandle 测试 | 04 |
| `src/panels/terminal/webgl.ts` | 26.4% | setupWebglWithRetry 指数退避/耗尽回退/cancel | 06、15 |
| `src/panels/diff/DiffPanel.tsx` | 63.9% | 保存链、占位刷新、.git 刷新、外部修改脏确认、滚动重绑定 | 07 |
| `src/workspace/PageDockviewHost.tsx` | 44.8% | 真实 DefaultTab/Watermark/RightHeader/handleReady/onSaveAs | 08 |
| `src/workspace/pageApis.ts` | 42.2% | switchToPageShared / switchToPageAndFocus 页面切换核心 | 08 |
| `src/features/explorer/ExplorerPanel.tsx` | 67.5% | 在终端中打开、CRUD 成功刷新、焦点/hover/错误 dismiss | 09 |
| `src/features/commit/useCommitStatus.ts` | — | debounce 清理、200ms 去抖 | 10 |
| `src/features/sideViews/ActivityBar.tsx` | — | drop index 落点、resolveTargetZone 边界阈值 | 10 |

### 1.3 按六维度的问题分布

| 维度 | 主要表现 | 涉及报告 | 占比估算 |
|------|----------|----------|----------|
| **断言有效性** | 假测试、循环断言、测试内联重写被测逻辑、字面量自断言、名实不符 | 02、05、07、12、15 | ~25% |
| **覆盖度** | 核心命令层/集成路径零覆盖、边界分支未命中、关键错误路径缺失 | 01、02、03、06、07、08、09、11、14、15 | ~35% |
| **测试设计** | generation 竞态、debounce、async 命令包装、生产配置未加载 | 03、09、11、14、15 | ~15% |
| **Mock 使用** | mock 污染/漂移、jsdom 无法覆盖真实 WebView2、生产构建分支不可测 | 06、08、13、15 | ~12% |
| **稳定性风险** | 固定超时等待、timer 未清理、E2E settings 污染、外网下载依赖 | 04、10、12、15 | ~10% |
| **结构/可维护性** | 测试重复、单文件过大、过时测试、 stale inventory | 02、06、10、12、15 | ~8% |

---

## 二、🔴 严重问题清单（会导致漏报真实 bug）

### 2.1 L1：Tauri 命令包装层大面积零覆盖 / 测试重写被测逻辑

这是跨 L1 5 个模块的最突出共性问题：测试不调用真实 async Tauri 命令，而是把命令内部逻辑在测试里重写一遍，导致 `spawn_blocking`、`TaskJoin` 映射、路径沙箱、错误消息契约、RwLock 路径全部无回归。

- **[01] `pty_spawn/pty_write/pty_resize/pty_kill/pty_reattach` 命令零 Rust 单元覆盖** — `src-tauri/src/pty/spawn.rs:756-1183`
  - 尺寸超限、shell 白名单、cwd 沙箱校验、SEC-08 panel_id 归属校验均未在 L1 验证。
  - 关键改法：抽取 `validate_spawn_request` / `validate_session_ownership` 纯函数并补单元测试；对命令层做最小集成测试。

- **[01] Job Object 孤儿防护核心逻辑零 L1 覆盖** — `src-tauri/src/pty/spawn.rs:1185-1263`
  - `CreateJobObjectW`/`SetInformationJobObject`/`AssignProcessToJobObject` 调用参数与 `JobHandle::drop` 未被验证。
  - 关键改法：抽取 limit flags / job_name 构造逻辑做单元测试；L4 增加“杀掉 slterminal.exe 后检查子进程残留”。

- **[02] Git 五个 Tauri 命令函数全部未被命令层测试覆盖** — `src-tauri/src/git/mod.rs:127-582`
  - `git_status`/`git_diff`/`git_file_at_head`/`git_rollback`/`git_unstage` 的 `State<AppState>`、路径沙箱、`spawn_blocking`、错误消息契约全部未测。
  - 关键改法：构造最小 `AppState` 并 `await` 调用命令；抽取 `rollback_in_spawn_blocking` 等同步函数独立测试。

- **[02] 大量测试 inline 重写 git2 调用序列，不调用被测命令** — `git/mod.rs:2227-2703`
  - `git_rollback_restores_*`、`git_unstage_*`、`git_file_at_head_reads_*` 等复制生产代码，无法守护命令函数。
  - 关键改法：改为直接调用命令函数；保留 git2 行为测试但标注为底层原语。

- **[02] `git_rollback_two_step_*` 系列与当前生产实现脱节** — `git/mod.rs:2421-2621`
  - 生产已改为 `std::fs::write(blob) + index.add_path`，测试仍验证废弃的 `reset_default + checkout_index`。
  - 关键改法：删除或重写为当前命令路径；如保留需显式注释“已废弃，仅文档”。

- **[03] Hooks 三个注入/卸载/状态命令与信号处理核心链路零 L1 覆盖** — `src-tauri/src/hooks/inject.rs:191-423`、`signal.rs:52-79`
  - `hooks_inject`/`hooks_uninstall`/`hooks_injection_status`、`process_signal_file`（读→emit→删）均未在 L1 调用。
  - 关键改法：将 `inject_impl`/`uninstall_impl`/`status_impl` 抽出为可注入路径的同步函数；用 tempdir 模拟信号文件并验证 emit 回调。

- **[03] `HookSignalWatcher::start` 双通道事件循环与 `start_signal_watcher` 全局入口零 L1 覆盖** — `src-tauri/src/hooks/watcher.rs:46-136`、`mod.rs:63-84`
  - notify 实时 + 3s 轮询补漏、目录重建恢复、幂等启动等 win10 关键兜底逻辑未在 L1 验证。
  - 关键改法：将事件循环拆为 `run_one_tick` 纯函数；提供集成测试在临时目录真实启动 watcher。

- **[04] `FileWatcher::start()` 与 `notify_watch` 命令在 L1 零覆盖** — `src-tauri/src/notify/mod.rs:62-157`、`214-270`
  - debouncer 创建、watch 注册、事件循环、pause/resume、Tauri event 发射全部无 L1。
  - 关键改法：文档明确“L1 既定豁免，由 L4 守卫”；如可行，抽 `EventEmitter` trait 让 L1 可测事件循环。

- **[04] `fs/mod.rs` 写文件测试与实现同构，无法捕获实现偏差** — `src-tauri/src/fs/mod.rs:write_file_tests`
  - 测试重写 `use_crlf` 检测与 `final_content` 转换，若生产改为永远 CRLF/LF，期望会跟随变。
  - 关键改法：改为直接调用 `fs_write_file` 命令，或使用固定输入/输出断言。

- **[05] `settings.rs` 全部核心用例未调用真实 `save_settings` / `load_settings`** — `src-tauri/src/settings.rs:114-498`
  - `.bak` 备份/恢复、原子写入、`spawn_blocking`、`TaskJoin`、路径解析均未与命令集成验证。
  - 关键改法：用 `tokio::runtime::Runtime` `block_on` 调用真实命令；让 `app_data_dir()` 在测试中可注入 tempdir。

- **[05] `projects.rs` 测试只覆盖 I/O 内核，未覆盖 Tauri 命令包装层** — `src-tauri/src/projects.rs:64-81`
  - `save_projects`/`load_projects` 的 `app_data_dir()`、`spawn_blocking`、`TaskJoin` 未测。
  - 关键改法：新增 2 条测试直接 `await save_projects(...)` / `await load_projects()`。

### 2.2 L2：核心交互路径覆盖失真或严重缺失

- **[06] `webgl.ts` 核心渲染重试路径覆盖严重不足（26.4%）** — `src/panels/terminal/webgl.ts`
  - context loss 指数退避、重试耗尽回退 DOM、`cancel()` 清理定时器全部未测。
  - 关键改法：补测 `setupWebglWithRetry` 各分支；L4 增加真实 WebView2 context loss 场景。

- **[07] `diff-panel.test.tsx` 保存链用例有名无实** — `src/__tests__/diff-panel.test.tsx:169-194`
  - 注释声称验证 `writeFile → gitDiff → updateDiffGutter`，实际只断言 mock 函数已定义。
  - 关键改法：真实触发保存，断言 `writeFile` → `gitDiff` 重调 → 双侧 gutter/占位刷新。

- **[07] `DiffPanel.tsx` 行覆盖仅 63.9%，关键路径大面积缺失** — `src/panels/diff/DiffPanel.tsx`
  - 保存后刷新链、占位刷新同步、左侧 `.git` 变更刷新、外部修改脏确认、滚动同步重绑定、大文件分支均无 L2 防护。
  - 关键改法：按源码分支补全 L2 用例，优先覆盖保存链与占位刷新。

- **[08] `PageDockviewHost.tsx` 真实组件覆盖严重不足（44.8%）** — `src/workspace/PageDockviewHost.tsx`
  - 真实 `DefaultTab`、`Watermark`、`RightHeader`、`handleReady`、`onSaveAs` 均未在 L2 直接测试。
  - 关键改法：补真实 `DefaultTab` 渲染测试、`RightHeader` addPanel、`handleReady` 不兜底创建终端、`onSaveAs` 重算标题。

- **[08] `pageApis.ts` 页面切换核心函数无 L2 测试** — `src/workspace/pageApis.ts`
  - `switchToPageShared`（`setProjectRoot` 先于 `setActivePage` 的 DBG-5/DBG-9 契约）、`switchToPageAndFocus` 轮询聚焦均无回归。
  - 关键改法：直接调用 `switchToPageShared` 并断言 await 顺序与 `__dockviewApi` 重指；验证轮询超时降级。

- **[08] `App.tsx` 启动恢复顺序未断言 `setProjectRoot` 先于 `setActivePage`** — `src/App.tsx:76-84`
  - `startup-restore.test.ts` 验证了状态流转，但未锁定 DBG-6 关键顺序。
  - 关键改法：在启动恢复测试中 spy 并断言 `setProjectRoot` 在 `setActivePage` 之前完成。

- **[09] ExplorerPanel「在终端中打开」功能完全无测试覆盖** — `src/features/explorer/ExplorerPanel.tsx:251-262`
  - `handleOpenInTerminal` 未验证 `cwd`、panelId 格式、`renderer: "always"`、params 字段。
  - 关键改法：右键菜单触发后断言 `addPanel` 参数含 `component="terminal"`、`params.cwd` 正确、`renderer` 为 always。

- **[09] CRUD 成功路径未验证文件树刷新与状态重置** — `src/features/explorer/ExplorerPanel.tsx:321-350`
  - 删除/重命名/新建成功后 `refresh()`、`setRenamingPath(null)`、`setNewFileName(null)` 未断言。
  - 关键改法：为每个 CRUD 操作补充成功路径断言。

- **[10] `sideBar.ts` 的 `cancelPendingSave()` 零覆盖** — `src/stores/sideBar.ts:143-149`
  - App 关闭钩子依赖此函数冲掉待保存 timer，无任何用例验证。
  - 关键改法：触发状态变更产生待保存 timer，调用 `cancelPendingSave()` 后推进 fake timers，断言 `saveSettings` 不再调用。

- **[10] ActivityBar 拖拽测试不校验 `moveButton` 的 `index` 参数** — `src/__tests__/activityBar.test.tsx:260-495`
  - 所有 drop 用例只断言 zone，`computeDropTarget` 落点索引完全无守卫。
  - 关键改法：每个 drop 用例追加 `expect(moveSpy.mock.calls[0][2]).toBe(expectedIndex)`。

- **[10] `useCommitStatus` 的 debounce 清理与去抖未覆盖** — `src/features/commit/useCommitStatus.ts:88-108`
  - 200ms debounce 与 unmount 时 `clearTimeout` 未测，可能导致重复 `gitStatus` 或 React 状态警告。
  - 关键改法：连续触发 fs-event 验证仅 1 次 `gitStatus`；激活 timer 后卸载验证 `clearTimeout`。

- **[11] JsonMode 测试未锁定两个 `linter()` 的包装顺序** — `src/__tests__/hooks-config-jsonmode.test.tsx:158-181`
  - 只断言 options，未断言 `[0]` 是 `jsonParseLinter` 还是 `jsonSchemaLinter`；交换后语法错误会进入 schema linter。
  - 关键改法：追加 `linterCalls[0][0]` / `linterCalls[1][0]` 身份断言。

- **[11] `useHooksConfig.load()` 的 generation 竞态取消无测试守卫** — `src/panels/hooksConfig/useHooksConfig.ts:110-129`
  - 快速切层时旧请求结果覆盖当前层的风险未覆盖。
  - 关键改法：模拟过期 resolve，断言最终 configJson 仍为目标层数据。

- **[11] `HooksConfigPanel.handleJsonChange` 对非法 JSON 的 catch 无回归** — `src/panels/hooksConfig/HooksConfigPanel.tsx:146-155`
  - 用户编辑过程中非法 JSON 可能让组件崩溃，当前测试全部只传合法 JSON。
  - 关键改法：向 `onChange` 传入非法文本，断言 `configJson` 保持原快照、保存按钮禁用。

- **[12] `colors.test.ts` 表驱动用例只断言测试常量自身** — `src/__tests__/colors.test.ts`
  - `GIT_FILE_COLORS` / `GIT_GUTTER_COLORS` / `EXPLORER_COLORS` / `SIDEBAR_COLORS` 等用 `expected` 字面量断言 hex 格式，不读 `colors.ts` 实际值。
  - 关键改法：改为 `expect(GIT_FILE_COLORS[key]).toBe(expected)`。

- **[14] `historyModel.ts` 四态同源回退分支未覆盖** — `src/features/claudeHistory/historyModel.ts:131`
  - `sessionId` 缺失时回退到 `transcriptPath` basename 去 `.jsonl` 是历史区与活跃区 emoji 同源的关键路径。
  - 关键改法：构造 `sessionId: null` 注册表条目，断言 `deriveActiveSessionStatuses().get("abc") === "working"`。

- **[14] `TerminalRegistry.setClaudeSession` merge 语义未断言** — `src/panels/terminal/TerminalRegistry.ts`
  - 未验证 `undefined` 字段不覆盖旧值、缺 `lastEventAt` 时自动填充，这是 F5 双通道建行/三通道删行的核心保证。
  - 关键改法：增量更新后断言旧 `transcriptPath` 保留且 `lastEventAt` 被更新。

- **[14] `useClaudeNotifications` 事件分类表缺少系统化断言** — `src/features/notifications/useClaudeNotifications.ts:76,131,139,143`
  - `Stop`/`StopFailure`/`PostToolUseFailure`/`Notification` 分类错误会导致 toast 类型错误，但无表驱动断言。
  - 关键改法：导出 `classifyEvent` 并新增事件 × notificationType 表驱动测试。

### 2.3 L3/L4：端到端与 headless 渲染路径失真

- **[15] L3 `keyboard.test.ts` 约 30 条用例是同义反复的字节透传** — `test/terminal/keyboard.test.ts`
  - `term.input('\x01')` → 断言 `onData` 收到 `\x01`，未经过 `attachCustomKeyEventHandler` + `ShortcutRegistry.resolve` 真实链路。
  - 关键改法：降级为“xterm.js 基础行为回归”并明确标注；L2 补充真实键位场景。

- **[15] L3 完全不覆盖生产主题配置与自定义 OSC 处理器** — `src/panels/terminal/theme.ts`、`useClipboardHandler.ts`、`useCommandDetection.ts`、`useXterm.ts`
  - OSC 52 剪贴板、OSC 133 命令边界/页签标题、OSC 8 超链接、`kittyKeyboard`、`drawBoldTextInBrightColors` 均未在 L3 断言。
  - 关键改法：用生产 `terminalOptions` 创建 headless Terminal 并触发对应 OSC，mock IPC 断言回调。

- **[15] L4 不验证真实 OS 输入，键盘链路是“伪端到端”** — `e2e-tests/test.e2e.ts`
  - 编辑器 Ctrl+S、HTML Ctrl+W、终端 Ctrl+Shift+V 均使用 `dispatchEvent` 或 helper，未验证从 OS 按键到应用的完整链路。
  - 关键改法：文档标注为“半端到端”；未来 WDIO 支持真实输入时替换为 `browser.keys()`。

- **[15] L4 hooks 注入污染 `~/.claude/settings.json`，无 E2E 隔离备份** — `e2e-tests/run-wdio.cjs`
  - FIX-TE-04 只备份 `~/.slterminal/settings.json`，不备份 `~/.claude/settings.json`；E2E 异常退出会残留 slterm matcher。
  - 关键改法：扩展备份范围，exit 时还原 `~/.claude/settings.json` 并清理 hooks 目录。

- **[15] L4 信号文件用例绕过真实 hook 上报脚本** — `e2e-tests/test.e2e.ts`
  - 直接在 Node 侧写 `.json` 到信号目录，未验证 `slterm-hook-reporter.js` 读取 stdin、注入 `SLTERM_PANEL_ID`、C10 任何路径 exit 0。
  - 关键改法：增加真实调用 hook reporter 脚本并向 stdin 写 JSON 的用例。

---

## 三、🟡 中等问题清单（可信度/稳定性）

### 3.1 边界分支未覆盖

- **[01] `strip_conpty_startup` 非 Windows 分支与未覆盖 OSC/CSI 分支** — `src-tauri/src/pty/reader.rs:166-235`
  - 非 Windows 原样返回、OSC 1/3/4/9 保留、CSI 3J 等分支未测。

- **[01] `ring_buffer_append` 无换行长行淘汰边界未覆盖** — `src-tauri/src/state.rs:201-218`
  - `map_or` 右侧“1024 字节内无换行则按 1024 原量淘汰”分支未测，超长单行可能死循环。

- **[01] `resolve_shell_info` 自动检测回退逻辑与 `build_cmdline` 引号处理未测** — `src-tauri/src/pty/shell.rs:94-127`、`src-tauri/src/pty/spawn.rs:81-99`
  - pwsh→powershell→cmd 顺序、程序路径/参数含空格加引号未验证。

- **[01] `ConPtyMaster::resize` HPCON invalid 分支未覆盖** — `src-tauri/src/pty/spawn.rs:201-217`
  - drop 后 resize 应静默更新 size 而不调 Win32 API。

- **[02] `status_to_str` 的 conflict 分支与 `compute_diff_hunks` 关键边界未覆盖** — `src-tauri/src/git/mod.rs:42-43`、`315-363`
  - conflict 着色、修改后多余新增行、`prev_was_del` flush 分支缺失。

- **[02] Git 路径沙箱失败分支与 `From` 实现未覆盖** — `src-tauri/src/git/mod.rs`、`src-tauri/src/error.rs:49-63`
  - 五个命令的 `validate_path_within_root` 拒绝分支、`serde_json::Error`/`git2::Error`/`tokio::task::JoinError` 转换未测。

- **[03] `hooks_context_usage` async 命令包装及 config IO 异常分支未覆盖** — `src-tauri/src/hooks/usage.rs:34-42`、`config.rs`
  - `home_dir()` 失败、`persist` 失败、命令包装层参数透传未测。

- **[04] `fs/mod.rs` 多个正常异常路径未覆盖** — `src-tauri/src/fs/mod.rs:221` 等
  - `fs_delete` 路径不存在、`fs_create_dir`/`fs_delete` root 外拒绝、`TaskJoin` panic 映射未测。

- **[04] `notify/pool.rs` 替换旧 watcher 分支未真正覆盖** — `src-tauri/src/notify/pool.rs:66`
  - 测试先手动 `pool.remove(&path)` 再 insert，导致 `insert` 内部 `remove -> stop` 分支未执行。

- **[05] `app_data_dir()` 与 `persist` 失败映射分支未覆盖** — `src-tauri/src/settings.rs:10-20`、`63-64`，`projects.rs:25-28`
  - `current_exe` 失败、exe 无父目录、`NamedTempFile::persist` 失败。

- **[06] `usePtyOutput.ts` 64KB 缓冲上限淘汰与 E2E 缓冲截断未覆盖** — `src/panels/terminal/usePtyOutput.ts:191-217`
  - `pendingBufSizeRef` 超过 `MAX_PENDING_BYTES` 时丢弃最旧数据块。

- **[06] `TerminalPanel.tsx` 分支覆盖不足（42.85%）** — `src/panels/terminal/TerminalPanel.tsx`
  - 1.5s 超时隐藏遮罩、`handleTabStateChange` active=false 分支、`windowsPty` 更新未测。

- **[07] `useCodeMirror.ts` 大文件处理与保存失败路径缺乏直接回归** — `src/panels/editor/useCodeMirror.ts:150-276`
  - >10MB 拒绝、>1MB confirm 取消、`fs.writeFile` reject 无编辑器侧直接测试。

- **[07] `gitGutter.ts` 四个 dispatch wrapper 函数未被直接测试** — `src/panels/editor/gitGutter.ts:261-328`
  - `updateDiffGutter`/`clearDiffGutter`/`updateHeadDiffGutter`/`clearHeadDiffGutter` 调错 StateEffect 无法发现。

- **[10] `resolveTargetZone` 边界阈值与 `moveButtonPure` R7 场景未真正锁定** — `src/features/sideViews/ActivityBar.tsx:93-99`、`sideBarState.ts:105-151`
  - 测试中 clientY 远离中点；目标区非空时未打开跨区未验证。

- **[10] `SideBarArea` 的 `total <= 0` 除零守卫与 `sanitizeSideBar` NaN/Infinity 分支未覆盖** — `src/features/sideViews/SideBarArea.tsx:75-82`、`sideBarState.ts:67-70`

- **[12] ShortcutRegistry `forceContext` 平局 tie-breaker 反向分支未覆盖** — `src/features/shortcuts/ShortcutRegistry.ts:236-242`
  - 仅覆盖 `a=terminal,b=global`，未覆盖 `a=global,b=terminal`。

- **[12] `getStatusIcon(null)` 与 `theme.ts` `kittyKeyboard` 未断言** — `src/lib/claudeStatus.ts:23-26`、`src/panels/terminal/theme.ts:43`

- **[14] 通知去重缓存截断逻辑无覆盖** — `src/features/notifications/useClaudeNotifications.ts:132-133`
  - 200→100 截断分支未测。

### 3.2 稳定性风险

- **[04] `FileWatcher` Drop 测试依赖固定 `sleep(100ms)`** — `src-tauri/src/notify/mod.rs:567`
  - 慢 CI runner 上可能 flaky；应改为轮询等待 + 超时。

- **[04] `claude_history/scan.rs` 环境变量无 RAII 清理** — `src-tauri/src/claude_history/scan.rs:163-169`
  - panic 可能导致 `SLTERM_CLAUDE_PROJECTS_DIR` 残留，污染后续用例。

- **[07] `diff-panel.test.tsx` 滚动同步测试依赖固定 200ms 延时** — `src/__tests__/diff-panel.test.tsx:198-291`

- **[08] `workspace-switch-order.test.tsx` 使用 3000ms 超时** — `src/__tests__/workspace-switch-order.test.tsx`

- **[10] `commit-view.test.tsx` rootPath 切换用例混用 fake timers 与 `waitFor`** — `src/__tests__/commit-view.test.tsx:532-642`

- **[12] Store debounce 测试未在 afterEach 清理活跃 timer** — `src/__tests__/projects.test.ts`、`font-size.test.ts`、`keybindings.test.ts`
  - 残留 timer 可能在后续测试触发 `save_projects`/`saveSettings`。

- **[15] L4 使用固定 `browser.pause(500)` 多处** — `e2e-tests/test.e2e.ts:1669`、`2012`、`2018`、`2184`、`2190`

- **[15] L4 launcher 依赖外网下载 Node 22 便携版** — `e2e-tests/run-wdio.cjs:102-134`

### 3.3 Mock 使用合理性与断言有效性

- **[02] `git_status` 状态测试使用弱断言 `any(...)**` — `src/__tests__/git/mod.rs:691-837`
  - 未精确验证路径、状态字符串、条目数量。

- **[02] `git_file_at_head_unborn_branch_err` 未调用被测函数** — `src-tauri/src/git/mod.rs:2145-2157`
  - 只验证 `git2::Repository::head()` 返回 UnbornBranch，未验证命令错误消息。

- **[03] `inject_adds_10_events` 弱断言未守卫 matcher 结构** — `src-tauri/src/hooks/inject.rs:651-661`
  - 只检查键存在，未断言 `type`、`timeout`、`command` 字段。

- **[03] serde camelCase 测试使用 `contains` 弱断言** — `src-tauri/src/hooks/mod.rs:98-144`、`signal.rs:144-174`
  - 无法捕获字段值或类型错误。

- **[06] mock 对象混入不属于目标模块的 `hooks:` 字段** — `src/__tests__/use-xterm-output.test.ts`、`e2e-gating-terminal.test.ts`
  - `@xterm/addon-fit`、`TerminalRegistry`、`TabTitleRegistry` mock 被 copy-paste 污染。

- **[08] `layout-serde.test.ts` mock 的 `isValidPanelType` 仅允许 3 种面板** — `src/__tests__/layout-serde.test.ts`
  - 与真实 `PANEL_TYPES`（6 种）不一致，新版面板白名单过滤未验证。

- **[08] `workspace-defaulttab.test.tsx` 测试手写 MockDefaultTab 而非生产组件** — `src/__tests__/workspace-defaulttab.test.tsx`
  - 存在 `event.params.tabIcon` 误写为 `event.tabIcon` 的漂移风险。

- **[08] `workspace-switch-order.test.tsx` 的时序契约是手动模拟** — `src/__tests__/workspace-switch-order.test.tsx`
  - 未真正驱动 `switchToPageShared` 的 await/setActivePage 顺序。

- **[13] HTML postMessage origin/source 三层校验的 L2 测试使用 jsdom 模拟** — `src/__tests__/html-panel.test.tsx`
  - 无法代表真实 WebView2；缺少 `source` 不匹配、`origin` 非 `"null"` 等负面用例。

- **[13] 注入脚本逻辑仅做字符串包含检查** — `src/__tests__/html-panel.test.tsx`
  - 未验证注入位置、`</script>` 转义、事件监听器绑定顺序。

- **[14] `AgentStatusView` 标题覆盖使用了 mock 的 history** — `src/features/agentStatus/AgentStatusView.tsx:118`
  - `titleBySessionId` 真实派生逻辑未端到端验证。

### 3.4 测试设计质量

- **[02] 环境隔离不完整：依赖系统 git 全局配置** — `src-tauri/src/git/mod.rs:594-636`
  - 未在 `init_temp_repo` 中统一设置 `core.autocrlf`、`core.safecrlf`、`init.defaultBranch`。

- **[02] 测试名称与测试内容不一致** — `src-tauri/src/git/mod.rs:1109-1262`
  - `git_diff_returns_hunks`、`git_diff_new_file_no_head` 等命名暗示精确验证，实际只做了存在性断言。

- **[04] `claude_history/scan.rs` `scan_multiple_sessions_sorted_input_order` 命名误导** — `src-tauri/src/claude_history/scan.rs:240-260`
  - 对结果排序后再比较，不验证扫描顺序，但用例名暗示测试顺序。

- **[06] `use-xterm-lifecycle.test.ts` 与 `use-xterm-output.test.ts` 存在约 14 条重复用例**
  - `cancelPendingFlush` 与 ResizeObserver 合帧测试在两边几乎逐字复制。

- **[06] `setBufferType(..., "alternate")` 为虚假测试** — `src/__tests__/use-xterm-lifecycle.test.ts`
  - 源码未读取 `terminal.buffer.type`，测试仅给 mock 挂载不会被读取的属性。

- **[09] `useFileTree.fullRefresh` 实际未被调用，且 F8 测试命名误导** — `src/features/explorer/useFileTree.ts:191-206`
  - F8 用例断言的是初始 mount 调用 `gitStatus`，非 `fullRefresh()` 执行结果。

- **[12] `global-commands.test.ts` 用例名与断言不符** — `src/__tests__/global-commands.test.ts:166-174`
  - 声称验证 handler 不传播异常，实际只创建命令对象、未调用 handler。

- **[12] `projects.test.ts` 部分守卫用例 codify 了可疑行为** — `src/__tests__/projects.test.ts:179-371`
  - 对不存在 pageId/projectId 的操作仍递增 version 被锁定为强契约。

- **[15] L4 “拖拽跨区”用例名不副实** — `e2e-tests/test.e2e.ts:1018-1143`
  - 实际调用 store helper，未真实触发 DataTransfer/DnD 事件。

- **[15] L4 恢复编排不验证 claude 真实进入会话** — `e2e-tests/test.e2e.ts:3169-3235`
  - 断言到终端缓冲含 `claude --resume <id>` 即停止。

---

## 四、🟢 可维护性问题清单

### 4.1 测试结构与组织

- **[02] 单文件 88 条测试，setup 工厂与测试逻辑混合** — `src-tauri/src/git/mod.rs:584-2718`
  - 建议按命令拆分为独立测试文件；`init_temp_repo` 提取到共享 test_utils。

- **[02] `ci_l1_uses_single_test_thread` 不属于 git 领域测试** — `src-tauri/src/git/mod.rs:2000-2011`
  - 验证 CI 配置字符串，造成领域污染。

- **[06] `xterm-test-utils.ts` 中的 `setBufferType` 已成为与源码脱节的死辅助函数** — `src/__tests__/helpers/xterm-test-utils.ts`
  - 导致依赖它的交替缓冲测试失真。

- **[08] `panel-registry.test.ts` 与 `workspace-file-panel-types.test.ts` 对 `FILE_PANEL_TYPES` 重复断言**

- **[10] `commit-view.test.tsx` 过长且职责混合（850+ 行）** — `src/__tests__/commit-view.test.tsx`
  - 建议拆分为状态机、分派去重、右键菜单 UI 三个文件。

- **[15] L4 `test.e2e.ts` 单文件 3236 行，设置代码高度重复**
  - 建议提取 `withProjectAndTerminal` helper 并按领域拆分 spec 文件。

### 4.2 命名、注释与文档

- **[03] `.claude/test-inventory.md` 对 hooks 模块描述含 stale 条目（“notification 权限声明”）** — `.claude/test-inventory.md`

- **[04] `claude_history/ops.rs` 空串 UUID 验证的消息断言恒真** — `src-tauri/src/claude_history/ops.rs:139-148`
  - `bad = ""` 时 `msg.contains(bad)` 恒为 true。

- **[09] `explorer-delete.test.tsx` E6 标题与断言矛盾** — `src/__tests__/explorer-delete.test.tsx:536-547`
  - 标题称 handler 返回 false，断言却是 `deleteSelected` 被调用一次。

- **[12] 多个测试用例名称与断言不一致** — `global-commands.test.ts`、`projects.test.ts:339-342`

- **[15] L3 `ansi-correctness.test.ts` 256 色优化用例注释与断言冲突** — `test/terminal/ansi-correctness.test.ts:70-81`
  - 注释断言 serialize 会优化为标准 SGR，但 `expect` 要求原始 256 色序列存在。

### 4.3 调试辅助与低风险覆盖

- **[06] `TerminalRegistry.ts` `getAll` / `_size` / `_dump` 三个接口未被调用** — `src/panels/terminal/TerminalRegistry.ts`
  - 函数覆盖率从 100% 降至 75%；若被 Agent Status/历史区依赖则为真实缺口。

- **[12] `command-catalog.test.ts` 的 `commandFromMeta` 只覆盖 9 条命令中的 5 条** — `src/__tests__/command-catalog.test.ts:76-135`

- **[12] `colors.test.ts` 缺少 `EXPLORER_SELECTION_BG` 等 token 校验** — `src/__tests__/colors.test.ts`

- **[12] `ErrorBoundary.tsx` 未覆盖 `variant="inline"` 分支** — `src/lib/ErrorBoundary.tsx:51-62`

- **[13] `src/ipc/index.ts` 的 `ping()` wrapper 0% 覆盖** — `src/ipc/index.ts:19-21`
  - `ipc-ping.test.ts` 直接 `invoke('ping')`，未使用导出的 `ping()`。

- **[13] 四个 IPC 契约测试文件高度重复，可参数化** — `src/__tests__/ipc-*.test.ts`

- **[14] `SessionActionDialog` 空 actions 防御分支未覆盖** — `src/features/claudeHistory/SessionActionDialog.tsx:42`

- **[14] `HistorySessionRow` 图标优先级分支未覆盖** — `src/features/claudeHistory/HistorySessionRow.tsx:50`

---

## 五、覆盖率缺口专题

### 5.1 🔴 核心逻辑零覆盖清单

| 源文件 | 未覆盖核心逻辑 | 风险 | 来源报告 |
|--------|----------------|------|----------|
| `src-tauri/src/pty/spawn.rs:1185-1263` | Job Object 孤儿防护 | 父进程异常退出时子进程残留 | 01 |
| `src-tauri/src/pty/spawn.rs:756-970` | `pty_spawn` 尺寸/白名单/cwd 校验 | 非法请求可穿透 | 01 |
| `src-tauri/src/pty/spawn.rs:977-1183` | `pty_write/resize/kill/reattach` + SEC-08 | panel 归属校验失效 | 01 |
| `src-tauri/src/git/mod.rs:127-582` | 5 个 Tauri 命令包装层 | 路径沙箱/错误契约/缓存行为无回归 | 02 |
| `src-tauri/src/hooks/inject.rs:191-423` | 注入/卸载/状态三命令 | settings.json merge/非法 JSON/版本比对无回归 | 03 |
| `src-tauri/src/hooks/signal.rs:52-79` | `process_signal_file` 读→emit→删 | 信号消费链路断裂 | 03 |
| `src-tauri/src/hooks/watcher.rs:46-136` | 双通道事件循环 | win10 轮询补漏兜底失效 | 03 |
| `src-tauri/src/notify/mod.rs:62-270` | `FileWatcher::start` / `notify_watch` | 文件监听入口无 L1 | 04 |
| `src-tauri/src/settings.rs:38-106` | `save_settings` / `load_settings` | 配置持久化命令层无回归 | 05 |
| `src-tauri/src/projects.rs:64-81` | `save_projects` / `load_projects` | 项目数据命令层无回归 | 05 |
| `src/panels/terminal/webgl.ts` | `setupWebglWithRetry` 全部核心路径 | GPU 渲染稳定性无回归 | 06、15 |
| `src/panels/diff/DiffPanel.tsx` | 保存链、占位刷新、.git 刷新、脏确认 | diff 面板关键交互裸奔 | 07 |
| `src/workspace/PageDockviewHost.tsx` | 真实 DefaultTab/Watermark/RightHeader/handleReady | 页签/水印/右键菜单行为无 L2 | 08 |
| `src/workspace/pageApis.ts` | `switchToPageShared` / `switchToPageAndFocus` | 页面切换顺序与聚焦无回归 | 08 |
| `src/features/explorer/ExplorerPanel.tsx` | `handleOpenInTerminal`、CRUD 成功刷新 | 高频操作无回归 | 09 |
| `src/features/commit/useCommitStatus.ts` | debounce 清理与去抖 | 重复刷新/卸载状态警告 | 10 |
| `src/stores/sideBar.ts:143-149` | `cancelPendingSave` | 关窗竞态写盘 | 10、12 |
| `src/panels/hooksConfig/useHooksConfig.ts` | `load()` generation 竞态、`reload()` askGuard | 配置错层/弹窗循环 | 11 |
| `src/panels/hooksConfig/HooksConfigPanel.tsx` | `handleJsonChange` catch、`handleUninstall` catch | 非法 JSON/卸载失败崩溃 | 11 |
| `src/features/claudeHistory/historyModel.ts` | 四态同源回退分支 | 历史区与活跃区 emoji 不同步 | 14 |
| `src/panels/terminal/theme.ts` | 生产主题配置与 `kittyKeyboard` | 颜色/键盘协议/光标配置漂移 | 12、15 |
| `src/panels/terminal/useClipboardHandler.ts` | OSC 52 剪贴板拦截 | 中文复制/CJK 解码回归 | 15 |
| `src/panels/terminal/useCommandDetection.ts` | OSC 133 命令边界 + TabTitleRegistry | F3 四态/页签标题无 L3 | 15 |

### 5.2 🟡 边界分支未覆盖（节选）

- `reader.rs` 非 Windows 原样返回、OSC 1/3/4/9 保留、CSI 3J（01）
- `state.rs` ring_buffer 无换行淘汰、`canonicalize_or_ancestor` relative path（01）
- `shell.rs` pwsh→powershell→cmd 回退顺序、`which_full_path` PATH 顺序（01）
- `git/mod.rs` conflict 状态、`compute_diff_hunks` 多余新增/prev_was_del（02）
- `fs/mod.rs` `fs_delete` not found、root 外拒绝、TaskJoin 映射（04）
- `notify/pool.rs` insert 替换旧 watcher 的 stop 分支（04）
- `TerminalPanel.tsx` 1.5s 超时、active=false 恢复、windowsPty 更新（06）
- `useCodeMirror.ts` 大文件拒绝/confirm 取消、保存失败（07）
- `GitShowPanel.tsx` 大文件警告 header、params 切换时 view 销毁（07）
- `ActivityBar.tsx` resolveTargetZone 中点边界、drop index 落点（10）
- `sideBarState.ts` / `stores/sideBar.ts` clamp NaN/Infinity（10、12）
- `ShortcutRegistry.ts` forceContext 平局反向分支（12）
- `claudeStatus.ts` `getStatusIcon(null)`（12）

### 5.3 已确认属“既定豁免”的清单

| 项目 | 豁免原因 | 当前兜底 | 来源报告 |
|------|----------|----------|----------|
| `notify/mod.rs` `FileWatcher::start` / `notify_watch` | L1 无 `AppHandle`，手动构造 FileWatcher 是既定模式 | L2 mock `onFsEvent` + L4 真实监听 | 04 |
| `claude_history/ops.rs` / `scan.rs` 命令包装 | `CLAUDE.md` 明确“命令包装不直测” | L4 E2E | 04、05 |
| `lib.rs` `run()` 函数 | Tauri 运行时胶水，L1 无法直接启动完整应用 | L4 E2E 启动标题等用例 | 05 |
| `reader.rs` `reader_loop` I/O 编排 | 依赖 Mutex/RwLock/Channel/系统调用，无法纯函数化 | L3/L4 覆盖 | 01 |
| `src/ipc/dialog.ts` / `e2eEnabled.ts` 生产 false 分支 | `E2E_ENABLED` 在 L2 恒 true，结构性 DCE 缺口 | CI dist grep + L4 | 13 |
| HTML postMessage 真实 WebView2 行为 | jsdom 无法模拟 opaque origin 与 WebView2 CSP | L4 E2E | 13 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU，PASSTHROUGH_MODE 回归无法自动化 | L4 真实使用 + 文档注释 | 15 |
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys()` | 合成事件 + L4 部分链路 | 15 |

---

## 六、跨报告共性问题（模式归纳）

### 6.1 模式一：L1 Tauri 命令包装层测试缺失（5/5 L1 报告）

**形态**：Rust 测试大量调用底层同步 helper 或 inline 重写 git2/fs/git2 API，不 `await` 真实的 `#[tauri::command]` async 函数。

**涉及领域**：PTY（01）、Git（02）、Hooks（03）、History/FS/Notify（04）、Settings/Projects（05）。

**系统性改法**：
1. 将命令函数体内的核心逻辑拆为可独立测试的同步函数（如 `rollback_in_spawn_blocking`）。
2. 在 L1 用 `tokio::runtime::Runtime::block_on` 或 `#[tokio::test]` 直接 `await` 命令函数。
3. 构造最小 `AppState`（`Default::default()` 或测试专用构造器）并注入 tempdir 路径，覆盖路径沙箱拒绝分支。
4. 对每个命令至少补 3 条用例：happy path、路径沙箱拒绝、异常错误消息契约。

### 6.2 模式二：Async 数据竞态与 Debounce 清理无回归（4 个 L2 报告）

**形态**：`genRef` 过期结果丢弃、`setTimeout` 清理、`debounceRef` 清理在快速切换/卸载场景未覆盖。

**涉及领域**：`useFileTree`（09）、`useHooksConfig.load`（11）、`useClaudeHistory.scan`（14）、`useCommitStatus` debounce（10）、Store `cancelPendingSave`（10、12）。

**系统性改法**：
1. 对每条 async load 路径增加“旧请求延迟 resolve → 断言被丢弃”用例。
2. 对每条 `setTimeout`/`setInterval` 增加卸载清理断言（fake timers + unmount）。
3. 将 Store 测试的 `afterEach` 统一改为调用 `cancelPendingSave()` 或 `vi.runOnlyPendingTimers()`。

### 6.3 模式三：Mock 污染与生产构建分支不可测（3 个报告）

**形态**：mock 对象写入目标模块未导出的字段（`hooks:`）；L2 无法命中 `E2E_ENABLED=false` / 生产 tree-shake 分支；jsdom 无法模拟 WebView2 opaque origin。

**涉及领域**：终端面板 mock（06）、E2E gating mock（13）、HTML postMessage（13）、IPC wrapper（13）。

**系统性改法**：
1. 清理所有 mock 中的虚假字段；如需要额外依赖，改为在测试文件内单独 mock 该依赖。
2. 对 `E2E_ENABLED` 等编译期常量，增加 AST/正则断言其为字面量表达式，确保 DCE。
3. 对 WebView2/CSP/postMessage 等环境相关行为，在 L2 用例中显式标注“jsdom 模拟，真实行为由 L4 验收”。

### 6.4 模式四：集成路径与真实组件渲染覆盖不足（3 个 L2 报告）

**形态**：测试手写 Mock 组件或只验证 prop 透传，不驱动真实 `Workspace.tsx`/`PageDockviewHost.tsx`/`App.tsx`/`SidebarTree.tsx` 行为。

**涉及领域**：Workspace（08）、Explorer（09）、Commit（10）、Agent Status（14）。

**系统性改法**：
1. 对核心集成函数（`switchToPageShared`、`openCommitFile`、`restoreSession`）补直接调用测试。
2. 对真实组件至少补 1 条“端到端 mount + 用户动作 + 断言 DOM/回调”用例，而非仅 mock 子组件。
3. 对布局/页签类测试，使用真实 Dockview/Allotment 渲染或更细粒度的 DOM 断言。

### 6.5 模式五：L3/L4 端到端路径失真（报告 15）

**形态**：L3 headless 不跑生产 renderer/OSC handler；L4 使用合成事件、绕过真实 hook 脚本、污染用户配置。

**系统性改法**：
1. L3 明确限定为“网格状态正确性”，增加生产 `terminalOptions` + OSC 52/133/8 处理器测试。
2. L4 拆分 spec 文件、提取共享 setup helper、替换 `browser.pause` 为 `waitUntil`。
3. 扩展 FIX-TE-04 备份 `~/.claude/settings.json` 并清理 hooks 目录；增加真实调用 hook reporter 脚本的用例。

---

## 七、重构前优先修复建议（Top 10）

按“修了对重构保护价值最大”排序。

| 优先级 | 问题 | 建议动作 | 涉及层级 | 关联报告 |
|--------|------|----------|----------|----------|
| 1 | L1 Tauri 命令包装层大面积零覆盖 | 为 pty/git/hooks/history/fs/notify/settings/projects 命令补最小集成测试，构造 `AppState` 并 `await` 调用 | L1 | 01-05 |
| 2 | `webgl.ts` 核心渲染重试逻辑仅 26.4% 覆盖 | 补测 `setupWebglWithRetry` 的 context loss/退避/回退/cancel；L4 加真实场景 | L2/L4 | 06、15 |
| 3 | `DiffPanel.tsx` 保存链/占位刷新/`.git` 刷新缺失 | 重写 `diff-panel.test.tsx` 保存链用例；补占位刷新、外部修改脏确认 | L2 | 07 |
| 4 | `pageApis.ts` 页面切换顺序与 `PageDockviewHost` 真实组件未测 | 直接测试 `switchToPageShared` 的 `setProjectRoot` 先序；补真实 DefaultTab/Watermark/RightHeader | L2 | 08 |
| 5 | `colors.test.ts` 自断言测试常量 | 改为断言 `colors.ts` 实际导出值，防止 token 漂移 | L2 | 12 |
| 6 | `useHooksConfig.load()` generation 竞态与 JsonMode linter 顺序 | 补过期请求丢弃用例；锁定 `jsonParseLinter`/`jsonSchemaLinter` 包装顺序 | L2 | 11 |
| 7 | `sideBar.ts` / Store `cancelPendingSave` 零覆盖 | 触发 timer 后调用并断言保存不再触发；统一各 store afterEach 清理 | L2 | 10、12 |
| 8 | L4 hooks 注入污染 `~/.claude/settings.json` | 扩展 FIX-TE-04 备份与还原范围 | L4 | 15 |
| 9 | `historyModel.ts` 四态同源回退 + `TerminalRegistry` merge 语义 | 补 `sessionId: null` 回退用例；补增量更新保留旧字段用例 | L2 | 14 |
| 10 | `useCommitStatus` debounce 清理、ActivityBar drop index、Explorer CRUD 成功刷新 | 补去抖/卸载清理/落点索引/CRUD 成功路径断言 | L2 | 09、10 |

---

## 八、附录：各报告原始问题索引

| 报告 | 问题编号范围 | 严重级分布 |
|------|--------------|------------|
| 01 | P-1 ～ P-15 | 🔴4 🟡9 🟢2 |
| 02 | P-1 ～ P-15 | 🔴4 🟡9 🟢2 |
| 03 | P-01 ～ P-13 | 🔴4 🟡6 🟢3 |
| 04 | P-1 ～ P-11 | 🟡9 🟢2 |
| 05 | P-1 ～ P-9 | 🔴2 🟡5 🟢2 |
| 06 | 1 ～ 15 | 🔴2 🟡4 🟢4 |
| 07 | R1-R4 / Y1-Y5 / G1-G3 | 🔴4 🟡5 🟢3 |
| 08 | R1-R4 / Y1-Y7 / G1-G2 | 🔴4 🟡7 🟢2 |
| 09 | H1-H2 / M1-M8 / L1-L4 | 🔴2 🟡8 🟢4 |
| 10 | P1-P15 | 🔴3 🟡10 🟢2 |
| 11 | R1-R3 / Y1-Y6 / G1-G3 | 🔴3 🟡6 🟢3 |
| 12 | P-1 ～ P-14 | 🔴1 🟡7 🟢6 |
| 13 | P1-P17 | 🔴1 🟡6 🟢10 |
| 14 | 1 ～ 12 | 🔴3 🟡6 🟢3 |
| 15 | P-1 ～ P-15 | 🔴7 🟡6 🟢2 |
