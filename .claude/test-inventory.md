# 自动化测试用例清单

> **本文档是项目用例数唯一真值源。** 所有 CLAUDE.md、README、CI 配置中引用的用例数均以此文件为准。更新测试后必须同步本文档。

全量 **2658** 用例（Rust 449 + 前端 2059 + L3 116 + E2E 34），2026-08-05 更新。

> **计数口径**：前端 (L2) 用例数以 `grep -cE '^\s*(it|test)\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx` 展开的 `it`/`test` 块数为准（Vitest 实际运行数）；L3 同理 `test/terminal/*.test.ts`；Rust (L1) 以 `grep -c '#\[test\]'` 统计的 `#[test]` 属性数为准。L3 的 116 用例同时被 L2 (`npm test`) 和独立 L3 (`npm run test:l3`) 执行，但此处各层独立计数，不做去重。

## L1 — Rust 单元/集成测试（23 文件 / 449 用例）

运行：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/src/git/mod.rs` | 88 | status_to_str/git_status/git_diff/git_file_at_head/recurse_untracked_dirs/oldPath/rename_detection/line_callback/dunce/序列化/repository/rollback/unstage/commit_file |
| `src-tauri/src/pty/reader.rs` | 36 | ConPTY 启动序列剥离（含 OSC 1/3/4/9 保留/CSI 3J/平台守卫）/DA1 查询检测/apply_startup_strip/should_inject_da1/mirror_da1_query/eof_exit_code/16KB 边界 |
| `src-tauri/src/pty/spawn.rs` | 28 | compute_conpty_flags/flag 常量/ConPtyMaster MasterPty trait/AttrList + env 注入/cwd 规范化/ring buffer 回放/session 隔离 |
| `src-tauri/src/fs/mod.rs` | 28 | read_dir/write_file/create_dir/delete/rename + 命令包装单测 |
| `src-tauri/src/notify/mod.rs` | 24 | FileWatcher 生命周期 + classify_by_kind 事件分类（全 7 种 EventKind） |
| `src-tauri/src/state.rs` | 24 | ring buffer append+eviction+换行边界/validate_path_within_root 沙箱/canonicalize_or_ancestor |
| `src-tauri/src/hooks/usage.rs` | 28 | parse_usage_line 全分支（合法 JSON/缺字段/缺 message/非法 JSON/空串/大值 u64/类型不匹配/额外字段忽略/cache 字段提取/缺失默认 0/显式 0/单 cache 字段）+ scan_transcript_usage 集成（逆行命中/无 usage 回溯/文件不存在/空文件/损坏行跳过）+ ContextUsage serde camelCase（含缺 cache 字段反序列化）+ TRANSCRIPT_TAIL_BYTES 常量 + hooks_context_usage 端到端（多条 usage/空文件/损坏行跳过/大文件 >128KB 仅读尾部 64KB） |
| `src-tauri/src/hooks/inject.rs` | 22 | 注入幂等（空 settings/已有用户 hooks/已注入升级）/卸载干净（handler 级剔除：独立组/混组保用户 handler/全 slterm 组删除/无 slterm 零写盘）/状态检测（injected/outdated/notInjected）/非法 JSON 中止/版本比对/notification 权限声明 |
| `src-tauri/src/settings.rs` | 17 | 读写往返/文件不存在/JSON 损坏回退 .bak/浅合并/shadow 目录 + 命令包装单测 |
| `src-tauri/src/pty/shell.rs` | 15 | pwsh 发现/shell-integration.ps1 嵌入/UTF-16LE Base64 往返/which_full_path/shell 白名单 |
| `src-tauri/src/notify/pool.rs` | 13 | LruWatcherPool: 缓存命中/LRU 淘汰/pause_all_except/replace/remove/stop_all/Drop |
| `src-tauri/src/projects.rs` | 12 | 序列化往返/ID 生成/路径校验 |
| `src-tauri/src/hooks/signal.rs` | 9 | parse_signal_file 全分支（合法/缺 panelId/空 panelId/非法 JSON/空串/仅空白/optionals null）+ camelCase 序列化+反序列化往返 |
| `src-tauri/src/hooks/mod.rs` | 8 | InjectionStatus/HookInjectionStatus serde（camelCase）+ parse_signal_file 快速冒烟（合法/缺 panelId/非法 JSON/空串） |
| `src-tauri/tests/pty_integration_tests.rs` | 8 | PTY 往返/OSC cwd 解析/resize 生效/kill 无孤儿/Custom ConPTY spawn/reattach/env 注入 |
| `src-tauri/src/hooks/watcher.rs` | 15 | is_signal_file（.json/.JSON/.tmp/无扩展名）+ collect_signal_files（多文件收集/.tmp 与无扩展名排除/空目录/目录不存在→空）+ poll_once 轮询补漏（逐个处理注入闭包/幂等二次不处理/目录删除重建后恢复/非 json 忽略/无文件零调用——win10 实证修复）+ HookSignalWatcher 生命周期（stop 幂等/Drop join 线程） |
| `src-tauri/src/hooks/config.rs` | 18 | parse_layer（三层合法/非法拒绝）+ resolve_config_path（user→home/.claude/settings.json/project+local 沙箱校验/缺失 project_path Validation/子树外 PathNotAllowed）+ read_hooks_subtree（文件不存在 Null/无 hooks 键 Null/子树提取/损坏 Err）+ write_hooks_subtree（原子写/父目录自动创建/merge 保留其他字段/损坏拒绝覆盖/非 Object hooks 拒绝无副作用/非 Object 根拒绝/null 根视空对象） |
| `src-tauri/src/error.rs` | 4 | 序列化/Display/From<io::Error>/SessionNotFound |
| `src-tauri/src/lib.rs` | 2 | ping 返回 pong/`get_windows_build_number` 返回数字 |
| `src-tauri/src/claude_history/jsonl.rs` | 28 | parse_head（cwd 收集/首条可见 prompt 跳过 4 类/未知 type/EOF 截断/200 字符截断/头部标题 last-wins）+ 大文件头尾窗口协同（>512KB 中部标题收不到/尾部 64KB 命中）+ parse_tail_title（custom 恒优先/ai 兜底/截断行/空文件）+ resolve_title 回退链 5 态 + tail 优先（F7，决策 22） |
| `src-tauri/src/claude_history/scan.rs` | 14 | resolve_projects_root（env 覆盖/空 env 回退 home/默认）+ 排除 3 类（agent-*/非 UUID/subagents）+ 多目录收集 + 扫描根缺失空数组 + 损坏/空文件降级条目 + 完整字段回退 + cwdExists（真/假）+ env 端到端 + mtime（存在/缺失）+ 尾部 custom-title 覆盖头部 summary（F7，SEC-02/BE-06，决策 23/26） |
| `src-tauri/src/claude_history/ops.rs` | 7 | validate_session_id（UUID 双形态接受/5 类非法拒绝）+ delete（jsonl+同名目录范围/仅 jsonl/不存在 Err/非法 id）+ 越界防护（扫描根外哨兵文件不触碰）（F7，SEC-01；rename 命令与测试已随功能移除——问题 7 修复） |
| `src-tauri/src/claude_history/mod.rs` | 7 | HistorySession serde camelCase 七键集合精确匹配 + 反序列化 + roundtrip + TitleSource 五变体序列化/反序列化 + is_uuid_filename（合法/非法/agent 形态）（F7） |

> `pty/mod.rs`、`pty/win_build.rs`、`main.rs` 不含 `#[test]`，不在此列。
> claude_history 模块 56 用例为 grep `#[test]` 计数（jsonl 28 + scan 14 + ops 7 + mod 7），env 测试依赖 L1 `--test-threads=1` 门禁（`std::env::set_var` 全局可变）。

## L2 — 前端单元/集成测试（121 文件 / 2059 用例）

运行：`npm test`（Vitest + jsdom）

### IPC 层（4 文件 / 100 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 65 | pty/fs/settings/notify/git/hooks/notification 全模块 IPC 命令合约验证 + DBG-4 PTY 命令 payload 契约守卫（3 条：键集合精确匹配） |
| `src/__tests__/ipc-hooks-contract.test.ts` | 22 | hooks_inject/hooks_uninstall/hooks_injection_status/hooks_context_usage 四维验证（命令名/参数结构/返回值/异常传播）+ ContextUsage 四字段键集合精确匹配守卫（DBG-4 模式）+ HookEventPayload 8 字段契约完整性 + onHookEvent listen 绑定 |
| `src/__tests__/ipc-ping.test.ts` | 1 | mockIPC ping/pong 拦截 |
| `src/__tests__/ipc-claude-history-contract.test.ts` | 8 | claude_history_scan/delete 两命令四维验证（命令名/参数结构 camelCase sessionId/正常返回/异常传播——rename 已随功能移除，问题 7 修复），照 ipc-hooks-config-contract 模式（F7） |

### 终端面板（15 文件 / 218 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-xterm-lifecycle.test.ts` | 81 | PTY spawn/exit/setupRetry/快捷键/rAF 轮询/ResizeObserver/字体/OSC 52/OSC 133（C/D 匹配注册命令 + setClaudeSession 写入）/OSC 8/键盘委托/hook-event 过滤与状态更新（setClaudeSession 携 sessionId+transcriptPath+status——问题 2 四态同源/SessionEnd→null/Notification 非 attention status undefined/**payload sessionId/transcriptPath 空串 → 携 undefined 归一——空串防御**）/F3 四态 emoji |
| `src/__tests__/use-xterm-output.test.ts` | 37 | DEC 2026/直写阈值/交替缓冲/Idle+Max 合帧/Uint8Array/非焦点降频/cancelPendingFlush/visibleRef 门控 |
| `src/__tests__/can-fit.test.ts` | 15 | 五条件守卫 + null/undefined 参数防护 |
| `src/__tests__/terminal-registry.test.ts` | 18 | register/get/remove/has/幂等/claudeSession 缺省保留旧值/setClaudeSession 全分支（merge/null 清空/no-op/缺 lastEventAt 自动填/undefined 不覆盖/**sessionId/status 存储与 merge 保留/status 显式 null 清空**——问题 2）/sessionChange 事件裸 panelId 结构/`_reset` |
| `src/__tests__/tab-title-registry.test.ts` | 13 | register/match（首 token 匹配——含带参命中/空命令行/仅空白/首 token 无规则仍 null）、大小写、覆盖、`_reset()`、单例校验 |
| `src/__tests__/use-xterm-integration.test.ts` | 12 | 轻 mock（真实 Terminal/FitAddon，仅 mock ipc/pty）；rAF 轮询失败回退/term.onData→pty.write/visible 切换 WebGL 释放重建 |
| `src/__tests__/keyboard.test.ts` | 12 | `createTerminalShortcuts()` copy/paste/newline 经 active 指针派发 |
| `src/__tests__/terminal-registry-subscribe.test.ts` | 7 | TerminalRegistry.subscribe（register 通知/remove 通知/sessionChange 通知/setClaudeSession 触发 sessionChange/退订） |
| `src/__tests__/tab-rules.test.ts` | 6 | side-effect import + `_reset()` 后手动注册（首 token 语义——"claude update" 命中 claude 规则） |
| `src/__tests__/e2e-gating-terminal.test.ts` | 5 | E2E helper 终端门控（`__e2e_sessionReady`/`__e2e_writeToPty` 等） |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整链路 |
| `src/__tests__/terminal.test.tsx` | 4 | TerminalPanel 组件：loading 遮罩/Windows build/spawn |
| `src/__tests__/active-terminal.test.ts` | 4 | active 指针 set/get/覆盖、clear 仅匹配时生效 |
| `src/__tests__/detect-webgl.test.ts` | 3 | WebGL2 可用/不可用/抛异常 |
| `src/__tests__/terminal-strictmode.test.ts` | 2 | `smGuardRef` 防双重挂载 |

### 编辑器面板（8 文件 / 125 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-code-mirror.test.ts` | 34 | 字体扩展/Compartment reconfigure/handleSave/gitDiff/slterm:file-saved + file-saved-as event |
| `src/__tests__/git-gutter.test.ts` | 28 | StateEffect → RangeSet 映射/GutterMarker DOM/SpacerMarker + HEAD 侧 old 行号映射/buildHeadRangeSet/headDiffGutter |
| `src/__tests__/language-mapping.test.ts` | 23 | 扩展名→CodeMirror 语言映射 + 未知回退 |
| `src/__tests__/editor-confirm.test.ts` | 11 | dirty/clean 外部修改确认/订阅取消/kind 过滤 |
| `src/__tests__/editor.test.tsx` | 9 | EditorPanel 渲染/panelId/filePath 传递 + `overflow: clip` 样式 |
| `src/__tests__/editor-font.test.ts` | 8 | 字体 CSS 选择器（`.cm-scroller` vs `.cm-editor`） |
| `src/__tests__/editor-keyboard.test.ts` | 7 | `createEditorShortcuts()` save/toggleWordWrap 经 active 指针派发 |
| `src/__tests__/active-editor.test.ts` | 5 | active 指针 set/get/覆盖、clear 仅匹配时生效 |

### 工作区/布局/页签（15 文件 / 213 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/open-hooks-config-panel.test.ts` | 5 | openHooksConfigPanel：API 就绪立即 addPanel 参数精确（hooksConfig-{pageId} 规则）/面板已存在 focus 不新建（同页单例）/API 延迟注册轮询命中/永不注册 5s 超时降级/pageId 变化 panelId 跟随 |
| `src/__tests__/title-manager.test.ts` | 44 | terminal-N 递增/编辑器 basename/同名冲突相对路径/handleSaveAs/onDeletePage/suffix 标题生成/冲突重算保留后缀/findExistingEditor 匹配隔离 |
| `src/__tests__/panel-registry.test.ts` | 32 | 注册表 6 面板（terminal/editor/htmlviewer/gitshow/diff/hooksConfig 六键 + hooksConfig 注册项为函数组件）/PANEL_TYPES 含 hooksConfig 长度 6/isValidPanelType/FILE_PANEL_TYPES（5 面板，不含 hooksConfig）/isAlwaysRenderPanel |
| `src/__tests__/layout-serde.test.ts` | 21 | 旧格式修补/白名单过滤/深拷贝/嵌套 branch/activeGroup 保留 |
| `src/__tests__/workspace-defaulttab.test.tsx` | 21 | 生产 DefaultTab 渲染（WRK-05：非手写 Mock）——tabIcon emoji/img 分支/onDidParametersChange 扁平事件结构回归（event.tabIcon 非 event.params.tabIcon）/标题更新/关闭按钮 |
| `src/__tests__/workspace-page-dockview.test.tsx` | 7 | PageDockview 真实组件（WRK-01）——handleReady 空布局不兜底创建终端/保存布局恢复+标题重算/损坏布局回退 Watermark/Watermark 按钮 addPanel/RightHeader「+」addPanel/onSaveAs 重算标题/rootPath 空忽略事件 |
| `src/__tests__/pageapis.test.ts` | 11 | pageApis（WRK-02）——switchToPageShared：DBG-5/9 await 时序（invocationCallOrder）/幂等/reject 降级/__dockviewApi 重指（D7）/rootPath 空与幽灵页面跳过；switchToPageAndFocus：立即命中/延迟命中/5s 超时降级（100ms×50） |
| `src/__tests__/workspace-header-actions.test.tsx` | 16 | RightHeader Watermark 按钮/页签操作 |
| `src/__tests__/workspace-switch-order.test.tsx` | 14 | DBG-9：真实驱动（WRK-06——点击页面行触发 switchToPage/switchToPageShared 断言 setProjectRoot 先于 setActivePage）reject 降级/SEC-01 effect 兜底/兼容性排查 |
| `src/__tests__/workspace-file-panel-types.test.ts` | 13 | FILE_PANEL_TYPES/isAlwaysRenderPanel（5 面板：terminal/editor/htmlviewer/gitshow/diff） |
| `src/__tests__/default-layout-format.test.ts` | 8 | grid/panels/activeGroup/orientation 格式验证 |
| `src/__tests__/layout-switch.test.ts` | 7 | 页面切换集成/自切换守卫 |
| `src/__tests__/workspace-multi-instance.test.tsx` | 6 | 多 Dockview 实例惰性初始化/删除活跃页面/H6 实例 identity（WRK-09：同一 api 对象跨切换不销毁重建 + 终端面板不 dispose + __dockviewApi 重指） |
| `src/__tests__/workspace-e2e-ready.test.tsx` | 4 | `__slterm_e2e_workspaceReady` 标记同步性 |
| `src/__tests__/workspace.test.tsx` | 4 | Dockview 初始化/项目页面关联 |

### Store 状态管理（4 文件 / 81 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/projects.test.ts` | 45 | Project/Page CRUD/持久化/version 递增/ID 生成/expandedNodes |
| `src/__tests__/font-size.test.ts` | 16 | 默认值/clamp/loadFromDisk/debounce 持久化 |
| `src/__tests__/keybindings.test.ts` | 16 | setBinding/clearBinding/resetAll/sanitize/loaded 守卫/debounce |
| `src/__tests__/layout.test.ts` | 4 | activePageId 设置/清空/重复 |

### 资源管理器（20 文件 / 252 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/explorer-git-status.test.tsx` | 32 | gitStatusMap 查表着色/配色 token/F5 untracked/slterm:file-saved/F8 mount 单次加载 |
| `src/__tests__/explorer-delete.test.tsx` | 22 | ask 弹窗分支/右键菜单/操作失败 UI 通知/横幅 dismiss+5s 自动消失+卸载清理（EXP-04）/E6 编号 17-22 统一（EXP-11） |
| `src/__tests__/file-icon.test.tsx` | 18 | 扩展名图标 + 目录图标 + git 状态着色 |
| `src/__tests__/explorer-refresh-preserve.test.tsx` | 17 | reloadPreservingExpanded 递归重建/边界容错/三条触发路径/竞态 |
| `src/__tests__/explorer-file-viewer.test.tsx` | 19 | handleOpenFile 面板分派/FileViewerRegistry/htmlviewer 回退/防御分支（无 dockviewApi/addPanel 抛错/getPanel undefined 回退新建，EXP-10） |
| `src/__tests__/explorer-keyboard.test.ts` | 15 | `createExplorerShortcuts()` delete/open/rename 经 active 指针派发 + actions ref 模式闭包不过期 |
| `src/__tests__/use-file-tree.test.ts` | 15 | loadRoot/loadDirectory/toggleExpand/generation 取消 |
| `src/__tests__/explorer-selection.test.tsx` | 17 | FileTree 选中模型（单击选中/双击打开/空白取消/hover 不覆盖选中态）+ 非选中行 hover enter/leave 高亮（EXP-04） |
| `src/__tests__/explorer-root-contextmenu.test.tsx` | 14 | 根节点右键菜单/新建文件+文件夹 |
| `src/__tests__/explorer-sandbox-race.test.tsx` | 13 | DBG-10：路径沙箱竞态回归——deferred `setProjectRoot` 验证 resolve 前 `readDir` 不被调用/resolve 后正常加载/reject 降级 |
| `src/__tests__/explorer-notify.test.tsx` | 12 | startWatch 调用时机/loadRoot/toggleExpand |
| `src/__tests__/explorer-input-boundary.test.tsx` | 10 | 内联输入框边界（EXP-06）：重命名 Enter 空名/失焦提交/失焦空值/重名提交/Escape + 文件夹级新建文件/文件夹 Escape/blur 空值/Enter |
| `src/__tests__/explorer-rename-state.test.tsx` | 8 | 重命名状态上提（renamingPath 由 ExplorerPanel 管理） |
| `src/__tests__/explorer-open-in-terminal.test.tsx` | 7 | 「在终端中打开」（EXP-01）：菜单入口 + addPanel 参数（component=cwd=panelId=renderer）+ 无去重 |
| `src/__tests__/explorer-race-cleanup.test.tsx` | 6 | useFileTree 竞态清理（EXP-07）：旧 loadRoot 延迟 resolve 丢弃/refresh null 分支/fs-event 去抖卸载清理/file-saved 缺 path 仍刷新/卸载清理/gitStatus 过期丢弃 |
| `src/__tests__/activeExplorer.test.ts` | 6 | active 指针 set/get/覆盖、clear 仅匹配时生效（同 activeTerminal/activeEditor 模式） |
| `src/__tests__/explorer-rootpath-clear.test.tsx` | 6 | rootPath 变化清空/快速切换 gen 丢弃/同值不清空 |
| `src/__tests__/explorer-rename-keyboard.test.tsx` | 5 | F2 快捷键 → renameSelected 集成 |
| `src/__tests__/explorer-focus.test.tsx` | 6 | ExplorerPanel 焦点管理（tabIndex/usePanelFocus 集成）+ focusin/focusout 上下文栈链路（EXP-04） |
| `src/__tests__/explorer-crud-success.test.tsx` | 4 | CRUD 成功路径（EXP-02）：删除/重命名/新建文件/新建文件夹 → IPC + refresh（readDir 二次）+ 状态重置 |

### 侧栏（1 文件 / 38 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sidebar-actions.test.ts` | 38 | 树结构/右键菜单/内联重命名/项目删除确认/布局 CSS/添加项目/**打开 Hooks 配置菜单入口（页面行含项/先切页后开面板顺序/项目行含项/有页切 pages[0]/无页新建后切换）** |

### 侧栏视图（6 文件 / 132 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sideBarState.test.ts` | 50 | toggleViewPure/moveButtonPure/deriveLayout/reconcileZones/sanitizeSideBar + S1-S6 场景序列 |
| `src/__tests__/sideViewRegistry.test.ts` | 7 | register/getAll/get/重复注册覆盖/未注册 get→undefined/_reset 隔离 |
| `src/__tests__/sideBar.test.ts` | 19 | 默认值/toggle/move 经 store/loadFromDisk 5 分支/loaded 守卫/debounce saveSettings payload 键集合精确匹配 |
| `src/__tests__/activityBar.test.tsx` | 29 | 渲染结构(3)/active(2)/toggle(2)/title(1)/dragStart(2)/dragOver+drop同zone(3)/dragEnd(1)/防御(1)/hover(2)/跨区状态机(5)/zone边界(3)/指示线清理(4)——拖拽向外层容器派发，clientY 经 installRectSpy mock |
| `src/__tests__/sideBarArea.test.tsx` | 14 | 四态布局/Allotment preferredSize splitRatio/display:none-flex 切换/保挂载/跨区卸载重建/props 透传/onChange→setSplitRatio/PANEL_BG token/首次双开 splitRatio 重置 |
| `src/__tests__/workspace-sideviews.test.tsx` | 13 | 活动栏 pane 40px 固定/侧栏区 pane visible=anyOpen 四态/preferredSize 来自 store/onChange→setWidth/主区 pane/props 透传 |

### Commit 视图（2 文件 / 48 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/commit-view.test.tsx` | 35 | 状态机四态/mock gitStatus+onFsEvent/列表渲染（文件名 GIT_FILE_COLORS token/计数/排序/空态）/折叠交互/双击分派 4 类状态（mock dockApi+真实 titleManager）/去重聚焦/fs-event 200ms debounce/rootPath 切换清空重载 |
| `src/__tests__/commit-context-menu.test.ts` | 13 | getContextMenuItems 状态→菜单映射（ROLLBACK_STATES/DELETE_STATES）+ action 执行流程（ask 确认→IPC→refresh） |

### hooks 配置面板（11 文件 / 205 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-hooks-config-contract.test.ts` | 12 | hooksConfig IPC 四维验证（readHooksConfig/writeHooksConfig 命令名/参数结构/返回值/异常传播）（Stage 01-02 遗留补登） |
| `src/__tests__/hooks-config-catalog.test.ts` | 19 | eventsCatalog 事件元数据（30 事件 x 10 组/HANDLER_TYPES/HANDLER_FIELD_MATRIX/纯查询函数）（Stage 03 遗留补登） |
| `src/__tests__/hooks-config-matcher.test.ts` | 21 | matcherEngine.matchHook 全分支（exact-or/regex/all/受限窄字符集/非法正则防御）（Stage 03 遗留补登） |
| `src/__tests__/hooks-config-model.test.ts` | 17 | configModel jsonToGui/guiToJson 双向转换/round-trip/容错/isSltermManaged（Stage 03 遗留补登） |
| `src/__tests__/hooks-config-panel.test.tsx` | 20 | 面板三态（loading/content/损坏错误态）/层级切换器禁用逻辑/保存按钮初始禁用/visibilitychange 轻量重读（可见触发/hidden 态不触发/不可见跳过/面板内点击不触发/ask 弹窗打开期间回归不二次弹窗，验收 #1/#2.1 防回归）/JSON 错误提示单行截断（nowrap+ellipsis+title，验收 1.2）/JsonMode 接入（value 序列化传递）/注入状态条与注入/卸载按钮 |
| `src/__tests__/hooks-config-jsonmode.test.tsx` | 17 | TE-09：CM6 EditorView 创建 + schema 扩展注册（jsonSchemaHover/jsonSchemaLinter + hooks 子 schema + height theme + linter needsRefresh，无自动补全）+ 非法 JSON/schema 违规触发 onValidationChange + 外部 value 同步 + MatcherTester 试测（exact-or/regex/受限字符集）；TE-10：十大分组 + 30 事件按钮渲染 + findEventPosition 纯函数 + 点击跳转选区（setSelection + scrollIntoView）+ 无副作用守卫 |
| `src/__tests__/hooks-config-gui.test.tsx` | 25 | GUI 模式（P3-FE-12）：Master-Detail 渲染/事件树增删事件与 matcher 组/详情区 handler 增删/选中态派生守卫（事件删除/重载回退空态）/注入段禁删（三层删除按钮禁用）/不支持 matcher 事件省略 matcher 输入；P3-FE-14：HandlerForm 接入（选中渲染表单/字段编辑上抛新模型/切换跟随/托管只读） |
| `src/__tests__/hooks-config-sync.test.tsx` | 8 | 双模式同步（P3-FE-16）：JSON 编辑 → guiModel 重算/GUI 编辑 → configJson 回写/非法 JSON 禁切 GUI + 禁用保存/dirty 与 saved 状态流转 |
| `src/__tests__/hooks-config-handlerform.test.tsx` | 38 | TE-11：5 种 type 必填字段渲染（官方版字段名断言：mcp_tool 为 input/http 无 method+body/agent 无 description+subagent_type/无 once）+ switchHandlerType 纯函数（保留通用字段清除不适用字段/extraFields）+ 事件支持矩阵过滤（B 档无 prompt/agent、SessionStart/Setup 仅 command/mcp_tool、未知事件兜底、当前 type 不在列表仍显示）+ type 切换交互 + 字段编辑（合法 JSON 上报/非法保留草稿不触发/清空删键）+ 注入段禁改（只读+禁删+不渲染禁用 checkbox+编辑零变更+非托管无锁定行） |

### Diff/GitShow 面板（3 文件 / 65 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/diff-alignment.test.ts` | 16 | computeAlignment 纯函数全分支：纯新增/纯删除/等行修改/多删少/多增少/多 hunk 合并/空 hunks |
| `src/__tests__/diff-panel.test.tsx` | 30 | mock gitFileAtHead+fs.readFile+gitDiff+onFsEvent、双栏渲染、加载态+错误占位、保存后刷新链 |
| `src/__tests__/gitshow-panel.test.tsx` | 19 | mock gitFileAtHead、三态（loading/content/error）、readOnly 断言、oldPath 优先 |

### 快捷键/命令系统（7 文件 / 114 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/shortcuts.test.ts` | 53 | 注册/注销/引用计数/上下文栈/IME/setOverrides 重绑/解绑/降级/冲突/resolve/forceContext/export/list/监听器 spy |
| `src/__tests__/keystroke.test.ts` | 18 | formatKeystroke/parseKeystroke/isValidKeystrokeString/format∘parse 恒等 |
| `src/__tests__/global-commands.test.ts` | 13 | `createGlobalShortcuts(getApi)` 延迟求值/Ctrl+W 关闭/返回一条命令（Hooks 配置入口已迁移侧栏菜单）/无面板透传 |
| `src/__tests__/command-catalog.test.ts` | 13 | 9 命令齐全/defaultKey 合法且非自身保留/commandFromMeta |
| `src/__tests__/reserved.test.ts` | 9 | isReserved 各 context/保留键命中/global 两集并集 |
| `src/__tests__/use-panel-focus.test.ts` | 5 | focusin→pushContext+onActivate/focusout→popContext+onDeactivate/卸载清理 |
| `src/__tests__/wire-keybindings.test.ts` | 3 | 立即应用/store 变更重应用/unsubscribe |

### 主题/配色/基础（6 文件 / 108 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/claude-status.test.ts` | 30 | `eventToStatus` 纯函数全分支：10 事件 × notificationType 组合（PreToolUse→⚡/PostToolUse→⚡/Notification 三类→🟡/其他→null/SessionEnd→✅/Error→❌/Stop/Result/Permission/Idle→null）+ OSC 133 C 设 🟡 语义注释说明 + getStatusIcon |
| `src/__tests__/path.test.ts` | 27 | normalizePath/basename/isChildOf/relativePath 边界覆盖 |
| `src/__tests__/inject-script.test.ts` | 21 | HTML 脚本注入/`</script>` 转义/幂等/大小写不敏感/键盘转发+片段链接拦截 |
| `src/__tests__/colors.test.ts` | 13 | 配色 token 值校验 + AGENT_STATUS_USAGE_COLORS 三 token（low #629755 / medium #BBB529 / high #F44747）hex 合法性 |
| `src/__tests__/theme.test.ts` | 12 | terminalOptions: ANSI 16 色/font/cursor/scrollback/kittyKeyboard |
| `src/__tests__/panelId.test.ts` | 5 | parseTerminalPageId（正常段/含连字符 pageId/非数字尾段/非 terminal 前缀/两段→null） |

### 通知/Agent 状态（3 文件 / 75 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/notifications.test.ts` | 25 | useClaudeNotifications hook（sendToastNotification 两参数无 onClick/hook-event 通知调度/窗口失焦门控/toast 去重/transcript_path 提取/任务栏闪烁三分类全覆盖/Notification 类型过滤/应用重新聚焦后 flush 积压/并发竞态） |
| `src/__tests__/agent-status-hook.test.ts` | 39 | useAgentStatus hook 行建模新语义全分支：纯 shell 无行（claudeSession null）→empty/无活跃项目→no-root/sessionChange（非 null）建行+携 transcriptPath 拉 usage/OSC 133 C 通道建行（sessionChange 携 matchedCommand）/hook 事件建行（行不存在时，**携 payload.sessionId**——问题 6 标题覆盖前置）/SessionEnd 删行/sessionChange(null) 删行/remove 删行（deps [] 稳定订阅——remove 不丢失）/切项目初始扫描只建活会话+主动拉 usage（**携 sessionId**）/matchedCommand-only 行 sessionId 缺省不报错/reconcile 对账兜底/四态 emoji 映射/transcriptPath null 跳过/跨项目过滤/倒序排列/错误降级/cache 字段包含/contextUsage 静默 catch+可观测 console.error/**now ticker（初始≈Date.now/推进 59s 不变/60s 更新 +60000/unmount 后清理——问题 1b 定时刷新）** |
| `src/__tests__/agent-status-view.test.tsx` | 22 | AgentStatusView 组件（F7 三下拉框适配 + 问题 1/4 修复）：no-root 占位/empty 占位（纯 shell 无行——"当前项目无运行中的 claude 会话"）/活跃区两行渲染/点击行调用 switchToPageAndFocus/**双行布局结构断言（标题与用量条不在同一 flex 行——问题 1）**/行1 标题 12px 粗体+行2 11px（问题 4 三级字号）/**相对时间格式（formatRelativeTime，问题 1）**/状态图标仍在行1（E2E 兼容）/用量条新口径（input+cacheRead+cacheCreation）/usage 为 null 或 undefined 显示 '--'/分段颜色断言（<50% low/#629755、50-80% medium/#BBB529、>80% high/#F44747）/切换项目行清空 + 三下拉框结构（活跃展开/历史区收起/受控切换/历史区挂载 ClaudeHistorySections）/**now prop 驱动时间重算（固定 lastEventAt + now 推进 → 1 分钟前→15 分钟前——问题 1b）/now 缺省回退 Date.now()/AgentStatusView 行时间随 60s ticker 推进（fake timers 全链路）** |

### Claude 历史会话（6 文件 / 115 用例，F7；问题 2-7 修复后重计）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/claude-history-model.test.ts` | 40 | 纯函数全分支：isCurrentProject（大小写/斜杠/null/空串/前缀不匹配——决策 24）/groupByCwd（组内+组间排序/未知目录组/空数组）/matchesSearch（标题+prompt/大小写/空白）/formatRelativeTime（六档边界+跨年+mtime=0——决策 26）/deriveActiveSessionStatuses（sessionId 优先/transcriptPath basename 回退/双无跳过/status null 不产出/四态透传/空注册表——问题 2 四态同源） |
| `src/__tests__/claude-history-view.test.tsx` | 33 | 受控 props 注入（sections 不再内部调 useClaudeHistory——问题 6）/三区结构与默认态/区块标题 13px 粗体（问题 4）/展开触发 scan/搜索过滤/组默认收起+计数（问题 3）/空态四文案/菜单矩阵（3 项——复制/分支恢复/删除，无重命名，问题 7）/双击分派（普通恢复/孤儿无操作/**运行中 → SessionActionDialog：切换反查 panelId → switchToPageAndFocus 参数/反查不到 toast/取消关闭，无分支恢复——问题 5**）/删除 ask 确认流程/AgentStatusView 集成（**标题覆盖：同 sessionId 磁盘标题覆盖 row.title/无匹配回退/标题 null 不覆盖——问题 6**；引导线样式） |
| `src/__tests__/claude-history-hook.test.tsx` | 13 | 状态机流转（idle/loading/ready/error）/scan 成功与失败/removeLocal 不触发重扫/activeStatuses 初值与 subscribe 驱动重算（Map<sessionId, status>）/卸载清理订阅（updateLocalTitle 已移除——问题 7） |
| `src/__tests__/claude-history-row.test.tsx` | 15 | 双行渲染（行1 12px 粗体/行2 11px——问题 4）/四态标记（working→⚡/attention→🟡/done→✅/error→❌/null 无标记——问题 2）/✗ 孤儿标记与四态并存/单击选中高亮/双击三分派/右键回调委托 |
| `src/__tests__/claude-history-action-dialog.test.tsx` | 7 | SessionActionDialog（问题 5 新建）：标题/消息/动作按钮渲染/action 回调/Esc 与遮罩取消/空 actions 防御 |
| `src/__tests__/claude-history-restore.test.ts` | 7 | 四步编排（mock stores/projects、workspace/pageApis、ipc/pty、TerminalRegistry、ipc/notification）：已开项目跳过入列/无页建页/切页/addPanel 参数（cwd/id 格式）/pty.write 内容（普通/fork/`\r` 结尾——决策 25）/防重入/失败 toast/无 cwd 防御性 throw |

> 原 `claude-history-input-dialog.test.tsx`（12 用例）已随重命名功能移除（问题 7 修复）。

### 启动/关闭（3 文件 / 18 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/close-handler.test.ts` | 11 | flush layout→saveAllProjects→localStorage/超时/异常 |
| `src/__tests__/startup-restore.test.ts` | 4 | localStorage 恢复/空/异常降级/Loading→ready |
| `src/__tests__/bootstrap.test.ts` | 3 | `__TAURI_INTERNALS__` 轮询/立即挂载/永不就绪 |

### 文件查看器/HTML（3 文件 / 69 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/html-panel.test.tsx` | 40 | 三态渲染/竞态取消/sandbox 属性/postMessage 键盘转发/origin 校验/片段链接拦截 |
| `src/__tests__/file-viewer-registry.test.ts` | 25 | 扩展名注册/策略链式调用/隐藏文件排除/大小写 |
| `src/__tests__/csp-config.test.ts` | 4 | tauri.conf.json CSP 不变量：script-src unsafe-inline/dangerousDisableAssetCspModification/default-src 严格 |

### E2E 辅助/门控测试（8 文件 / 27 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/e2e-build-config.test.ts` | 6 | VITE_E2E build 配置不变量 |
| `src/__tests__/app.test.tsx` | 5 | `__slterm_e2e_createProject` 行为/E2E pending 标记 |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | writeClipboard/createProject 函数可用性 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | pending 标记→localStorage 恢复交互 |
| `src/__tests__/dialog-e2e-hook.test.ts` | 3 | E2E ask 钩子守卫（claude-history Stage 06）：未设置钩子走真实 ask 不改默认行为/设 true 直接返回不调真实 ask/设 false 直接返回 false |
| `src/__tests__/e2e-enabled.test.ts` | 2 | E2E_ENABLED 真值表 + 常量与纯函数一致性 |
| `src/__tests__/e2e-gating-workspace.test.tsx` | 2 | `__slterm_e2e_workspaceReady` 存在性 |
| `src/__tests__/error-boundary.test.tsx` | 3 | 正常透传/抛错 UI/`__sltermError` 赋值 |

## L3 — 终端 headless 测试（4 文件 / 116 用例）

运行：`npm run test:l3`（Vitest + `@xterm/headless`，`environment: 'node'`）

> L3 的 116 用例同时被 `npm test` (L2) 包含执行。独立 L3 运行使用 `vitest.l3.config.ts`（仅匹配 `test/terminal/**/*.test.ts`）。

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `test/terminal/terminal-serialize.test.ts` | 41 | 基本文本序列化、多行输出、ANSI 颜色保留、大块数据、光标定位、scrollback、resize reflow、SGR 属性叠加、多语言字符、交替屏幕、所有 ED/EL/IL/DL 擦除操作、DECSC/DECRC |
| `test/terminal/keyboard.test.ts` | 36 | Ctrl 组合键（A-Z 全表 + Backslash/Slash）、Alt+字母/Enter、功能键 F1-F12、Home/End/PgUp/PgDn/Insert/Delete、方向键 + Ctrl+方向键、CSI u 协议（基本键/Shift/Ctrl/Ctrl+Shift/Alt 修饰）、退格/回车/制表符渲染 |
| `test/terminal/ansi-correctness.test.ts` | 30 | ANSI 颜色正确性——16 色前景/背景、256 色（标准/216 色立方/灰度）、TrueColor 24-bit 前景/背景/混合、SGR 属性（粗体/斜体/下划线/双下划线/慢闪/反显/隐藏/删除线/弱化/上划线）、SGR 组合叠加、SGR 重置/子参数重置、DEC 私有模式（DECTCEM/DECOM/DECAWM）、DECSC/DECRC、RIS、DECSTBM |
| `test/terminal/osc.test.ts` | 9 | OSC 序列——标题（OSC 0/2 BEL/ST）、调色板（OSC 4 单索引/多索引）、嵌入完整性（OSC 在正常输出中/穿插文本/后紧跟文本不丢失） |

## L4 — E2E 端到端测试（1 文件 / 34 用例，32 active + 2 skip）

运行：`npm run e2e`（= `npm run build:e2e` + `npm run wdio`）  
技术栈：WDIO + `@wdio/tauri-service` 1.1.0 + embedded driver（`webview2-com` COM 直连 `ICoreWebView2`）

| 用例 | 状态 | 覆盖范围 |
|------|------|---------|
| 应正常启动并显示 slTerminal 标题 | active | 应用启动 + 窗口标题验证 |
| 打开终端→写入文本→验证缓冲含 e2e_marker | active | 完整 PTY 通信链路（createProject→addPanel→spawn→write→read） |
| 终端面板可通过 E2E helper 写入文本并读取 | active | 键盘快捷键→剪贴板→终端粘贴 |
| 终端页签标题为 terminal-N | active | 页签默认标题 + 动态修改（`api.setTitle`） |
| 编辑器页签标题为文件名 | active | 编辑器标题 = basename + registerAndRecompute |
| 同名文件冲突时显示相对路径 | active | titleManager 冲突检测（`src/index.ts` vs `lib/index.ts`） |
| 关闭同名面板后剩余面板切回 basename | active | 关闭冲突面板后自动重算标题 |
| 聚焦编辑器后 Ctrl+S → 经 capture 路径真实写盘（mtime 更新） | active | capture 监听 + context 栈匹配 + 命令 handler + 写盘全链路 |
| 编辑器 dirty→clean 保存 | active | 外部写盘→auto-reload→Ctrl+S 保存新内容→磁盘断言 |
| iframe 内 Ctrl+W → postMessage 转发关闭该 HTML 页签 | active | postMessage 键盘桥：注入脚本→parent.postMessage→handleMessage 校验 origin→ShortcutRegistry 分发→global.closeTab（forwardGlobalShortcuts 已删除，FE-13） |
| 内联 `<script>` 与内联事件属性在预览中执行 | skip | CSP 修复：真实 WebView2 强制 CSP 下经 srcdoc opaque origin 验证内联脚本+onclick 执行（skip 原因：执行断言不稳定） |
| should preserve terminal content after switching to another page and back | active | H6 终端跨页面存活（多 Dockview 实例 + CSS 显隐） |
| 点击侧栏按钮切换视图显示/隐藏（R1/R2） | active | 活动栏按钮真实点击→open 状态变化→侧栏区 visible 联动→store 状态断言 |
| 拖拽按钮跨区移动（R6/R7） | active | DataTransfer 合成拖拽事件跨区→zones + open 跟随（R6 跟随替换/R7 未打开仅归属）→store 状态断言 |
| Commit 视图应显示真实的 Changes 与 Unversioned 文件列表 | active | git CLI 搭建真实仓库（init+commit+modified+untracked）→toggleSideView("commit")→断言 Changes/Unversioned 条目数与文件名 |
| 双击 modified 文件应打开 diff 页签 | active | 双击 modified 文件 → 断言 __dockviewApi 出现 title 含 `(git diff)` 的面板且存在 `data-e2e="diff-left"`/`diff-right` |
| 注入 hooks 后状态回显应为 injected | active | `hooks.inject()` → `hooks.getInjectionStatus()` → 断言 `{ status: "injected" }`；`hooks.uninstall()` → 断言 `{ status: "notInjected" }` |
| Node 写信号文件后终端页签出现 emoji 并随后消失 | active | Node 子进程写 `~/.slterminal/hooks-events/` 信号文件（PreToolUse→⚡）→ 断言 DefaultTab 渲染对应 emoji；写 SessionEnd→✅ → 短暂显示后恢复无图标 |
| Agent Status 视图可通过活动栏按钮打开 | active | toggleSideView("agent-status") → 断言视图容器出现 + AGENT STATUS 标题栏渲染 |
| Agent Status 纯 shell 终端无行（行建模新语义——不自动建行） | active | 创建纯 shell 终端 → TerminalRegistry 初始扫描 `claudeSession` 为 null → 断言 `agent-status-row` **不出现**（反转向：旧语义自动建🟡行） |
| Agent Status 动态四态（首个信号即建行→PreToolUse→⚡, Stop→✅, SessionEnd→行消失） | active | Node 原子写 `.tmp→.json` 信号文件 → 首个 PreToolUse 信号到达即 hook 事件建行（断言行含 ⚡——无需等待静态行）、Stop→✅（断言行含 ✅）、SessionEnd→行消失（断言 `agent-status-row` 移除） |
| R2 变体：切项目往返后用量保持（contextUsage 全链路 + cache 字段） | active | Node 端预写假 transcript JSONL（含 `message.usage` 四字段：input/cache_read/cache_creation/output）→ 信号文件携真实 transcriptPath 建行 → 初始扫描主动拉取 contextUsage（后端真实解析尾行）→ 断言行含百分比数值 → 切项目往返 → 用量数值保持（初始扫描携 transcriptPath 主动拉取） |
| R3 变体：SessionEnd 删行 + 切项目往返不复活 | active | hook 事件建行 → SessionEnd 信号 → 行消失 → 切项目往返 → 行仍不存在（claudeSession 已 null，初始扫描不建行） |
| R4 变体：会话终端关页签删行（remove 事件 + ref 稳定订阅） | active | hook 事件建行 → `__dockviewApi.removePanel(panel)` → 行消失（remove 事件 + ref 稳定订阅——R4 原始竞态不重现） |
| hooks 配置面板保存链路（P3-TE-18） | active | tempdir 项目 → 打开 hooksConfig 面板（`__dockviewApi.addPanel`）→ 切 project 层 → JSON 模式经 `__slterm_e2e_setHooksConfigJson` 注入合法 hooks 配置 → 程序化点击保存 → 真实 IPC `hooks_config_write` 写盘 → 断言 `.claude/settings.json` 含 hooks 子树且 **merge 保留其他字段**（走 project 层，不碰真实 `~/.claude/settings.json`） |
| toast 触发链路需人工验证（失焦 + 权限请求 / Stop / 错误） | skip | 通知 toast 全链路验证（skip 原因：权限弹窗需用户交互，自动化不可行） |
| 展开「全部项目历史会话」→ fixture 6 条会话行展示，agent-*/非 UUID/subagents 不出现 | active | 展开全部区触发扫描 → 断言 6 行（7 形态中 6 条）→ agent-misc.jsonl（形态6 平铺）/not-a-uuid.jsonl（非 UUID）/subagents（形态7 子目录）不出现（SEC-02：扫描根 = SLTERM_CLAUDE_PROJECTS_DIR 指向 .tmp-claude-projects 副本，fixture 经 run-wdio.cjs 重建） |
| 标题回退链：custom-title / ai-title / 首条 prompt 三会话行各显示预期标题 | active | 形态1 行显 custom-title（E2E自定义标题一）；形态2 行显 ai-title（赢 summary）；形态3 无标题行回退首条可见 prompt（isMeta/数组/< 开头/空白 4 类干扰行跳过），title 非 null 不显 UUID 前 8 位 |
| 搜索过滤：关键词仅保留匹配行；无结果显示「无匹配的会话」 | active | 搜索框（agent-history-search）注入关键词 → 仅 501 行保留 → 无结果关键词 →「无匹配的会话」→ 清空恢复 6 行（React 受控 input：原生 setter + input 事件，禁 browser.keys——键盘输入限制） |
| 复制恢复命令：右键 → 剪贴板内容为 `cd '<cwd>' && claude --resume <id>` | active | 右键 507 行 → 菜单「复制恢复命令」→ 剪贴板断言（clipboard-manager read_text 经 __TAURI_INTERNALS__.invoke；allow-read-text 已放行） |
| 孤儿行 ✗ 标记展示 + 双击无反应（无新面板/无页面切换） | active | 形态5（cwd 指向不存在路径）行含 ✗、普通行不含；双击孤儿行 → 活跃页面与面板集合不变（分派矩阵：孤儿 → 无操作） |
| 历史区四态：信号文件驱动 → 历史区行显示与活跃区一致的四态 emoji（⚡→✅→消失） | active | 创建项目+终端 → 展开全部历史区（fixture 501 行可见）→ 原子写 PreToolUse 信号文件（sessionId = 501 UUID，transcriptPath 指向副本）→ 断言 agent-status-row 与历史区 501 行**均含 ⚡**（两区同源 TerminalRegistry——问题 2 修复）；Stop → 两区均 ✅；SessionEnd → 活跃行消失 + 历史区标记清除 |
| 删除：ask 确认（E2E 钩子）→ 行消失 + 副本文件删除 | active | 右键孤儿行 → 「删除」→ ask 经 `window.__slterm_e2e_dialogAsk` 钩子返回 true（src/ipc/dialog.ts E2E_ENABLED 门控，生产 tree-shake——embedded WDIO 无法操作原生对话框）→ 行消失 + Node 侧断言副本 .jsonl 不存在（SEC-02：只动副本） |
| 恢复编排：双击普通行 → 项目入列 + 页面切换 + 终端注入 claude --resume（不断言真实进入会话） | active | 双击 507 行（cwd = run-wdio.cjs 创建的 E2E 临时项目目录）→ 断言 activePage rootPath = 该目录（addProject + switchToPageShared）+ 终端 PTY 就绪 + `__e2e_getTerminalText` 含 `claude --resume <id>`（pty.write 注入）；不断言 claude 成功进入会话（fixture id 非真实，真实成功属人工验证） |

### E2E settings.json 隔离机制（FIX-TE-04）

`run-wdio.cjs` 启动时备份 `~/.slterminal/settings.json`（存在时复制为 `settings.json.e2e-bak`），`process.on('exit')` 中同步还原——原文件存在时用备份覆盖 E2E 产物，原文件不存在时删除运行期间产生的 `settings.json`。node22 直跑/下载/fallback 三路径均受 `exit` 钩子覆盖。

### E2E 键盘输入限制（TE-17）

embedded WDIO 驱动**无法将 OS 级按键（`browser.keys`）投递进 WebView2 页面**——`browser.keys` 发出的 keydown 不会到达页面 DOM。所有键盘用例均改用合成事件路径：

- **终端**：`Ctrl+Shift+V` 用例发完按键后直接 `__e2e_writeToTerminal` 写标记验证终端可操作
- **编辑器保存**：合成 `focusin` 事件激活 context → `browser.execute` dispatch 合成 `keydown` 到 `window` → `ShortcutRegistry` window capture 真实捕获 → `editor.save` → 真实 IPC `fs.writeFile`，以 mtime 变化断言
- **HTML iframe**：`browser.execute` 在 `iframe.contentDocument` 上 dispatch 合成 `keydown` → 注入脚本 `postMessage` 到父窗口 → `handleMessage` 校验 `origin === "null"`（srcdoc opaque origin）→ `ShortcutRegistry` 分发

唯一"不真实"处是事件来源为 JS dispatch 而非 OS 键盘（驱动能力所限）；监听/上下文匹配/命令 handler/写盘/消息转发全链路均在真实二进制中执行。

## 静态检查门禁

| 门 | 命令 | 说明 |
|----|------|------|
| TypeScript | `npx tsc --noEmit` | 全量类型检查 |
| ESLint | `npx eslint src/` | 前端代码规范 |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Rust 代码规范 |

## 历史变更

- 2026-08-05（text-fix-plan Stage 09 EXP-01~11 explorer 测试补全）：资源管理器 16→20 文件 213→252 用例（+39）。新增 4 文件——explorer-open-in-terminal 7（EXP-01 在终端中打开 addPanel 参数/无去重）、explorer-crud-success 4（EXP-02 删除/重命名/新建文件/新建文件夹成功路径——IPC+refresh+状态重置）、explorer-input-boundary 10（EXP-06 重命名/文件夹级新建输入框 Escape/空名/重名/失焦边界）、explorer-race-cleanup 6（EXP-07 旧请求延迟 resolve 丢弃/rootPath null 回调/fs-event 去抖卸载清理/file-saved 缺 path/卸载清理/gitStatus 过期丢弃）。扩展现有 4 文件——explorer-focus 3→6（EXP-04 focusin/focusout 上下文栈 spy）、explorer-selection 14→17（EXP-04 非选中行 hover enter/leave）、explorer-delete 19→22（EXP-04 横幅 dismiss/5s 自动消失/卸载清理；E6 编号 17-22 统一 + 标题与断言对齐 EXP-11）、explorer-file-viewer 16→19（EXP-10 无 dockviewApi/addPanel 抛错无孤记录/getPanel undefined 回退新建）。EXP-03：删除 useFileTree.fullRefresh 死代码（无调用方），F8 改名「mount 单次加载」+ 断言 readDir/gitStatus 各一次。L2 2020→2059（117→121 文件）。全量 2619→2658。
- 2026-08-02（Claude 历史会话 Stage 07 文档同步补登，F7）：L1 新增 claude_history 模块 4 文件 62 用例（jsonl 28 + scan 14 + ops 13 + mod 7——扫描/降级/env 覆盖/回退链/SEC-01 校验/追加写，计数为 grep `#[test]`）。L2 新增「Claude 历史会话」类目 6 文件 109 用例（model 37 + view 28 + hook 14 + input-dialog 12 + row 11 + restore 7）+ IPC 层 ipc-claude-history-contract 12（88→100，三命令四维验证）+ E2E 辅助 dialog-e2e-hook 3（Stage 06 漏登补登，24→27，ask 钩子守卫）+ agent-status-view 11→15（+4 三下拉框适配，Stage 05 产物补登）。L1 384→446（19→23 文件），L2 1783→1911（109→117 文件）。全量 2317→2507。
- 2026-08-02（Claude 历史会话视图 E2E，TE-01..04）：新增 describe「Claude 历史会话视图」8 用例——fixture 7 形态（custom-title/ai-title/prompt 回退/无 cwd/孤儿/agent-* 平铺/subagents 子目录）+ 搜索过滤 + 复制恢复命令（剪贴板 read_text 断言）+ 孤儿行 ✗ 双击无反应 + 重命名（副本尾部 custom-title 行 Node 断言）+ 删除（ask invoke 拦截降级方案 + 副本文件删除 Node 断言）+ 恢复编排（项目入列/页面切换/终端缓冲含 claude --resume）。新增 fixtures/claude-projects/（9 文件）+ run-wdio.cjs 副本重建与占位符替换 + SLTERM_CLAUDE_PROJECTS_DIR/SLTERM_E2E_PROJECT_DIR env 注入（SEC-02：只动 .tmp-claude-projects 副本）+ .gitignore 条目。L4 26→34（32 active + 2 skip），全量 2309→2317。
- 2026-08-01（Hooks 配置入口迁移）：删 global.openHooksConfig 快捷键命令（Ctrl+Shift+H）——hooks-config-entry.test.ts 整文件删除（7）、command-catalog 14→13（-1 入口契约）、global-commands 13（断言改 1）；新增侧栏右键菜单入口——sidebar-actions 33→38（+5 菜单入口）、新建 open-hooks-config-panel.test.ts（+5 同页单例/轮询/超时）。L2 1781→1783，全量 2307→2309。
- 2026-08-01（外部修改检测改 visibilitychange）：hooks-config-panel 19→20（+1 visibilityState=hidden 不触发；改 3 个 window focus 用例为 visibilitychange 派发——jsdom 需 defineProperty 设 visibilityState 再 dispatch，afterEach Reflect.deleteProperty 还原）。L2 1780→1781，全量 2306→2307。
- 2026-08-01（验收修复 5：卸载改 handler 级剔除）：inject.rs remove_slterm_matchers 组级→handler 级（混组保用户 handler，组空才删组）；20→22 用例（+2：混组保用户 handler/全 slterm 组删除）。L1 382→384，全量 2304→2306。
- 2026-08-01（验收修复 4：JSON 模式三修）：删自动补全（jsonCompletion + jsonLanguage.data.of + @codemirror/autocomplete 直接依赖移除，传递依赖仍在）；加 EditorView.theme height:100% + overflow:clip（竖向滚动条，验收 1.3）；hooks-config-panel 18→19（+1 错误提示单行截断断言，验收 1.2）；jsonmode 17（断言改 height theme，用例数不变）。L2 1779→1780，全量 2303→2304。
- 2026-08-01（验收修复 3：GUI 接入 HandlerForm）：hooks-config-gui 21→25（+4：选中渲染表单/字段编辑上抛新模型/切换选中表单跟随/托管 handler 表单只读）。L2 1775→1779，全量 2299→2303。
- 2026-08-01（验收修复 2：外部修改检测改 window focus）：hooks-config-panel 17→18（删 3 个 focusin relatedTarget 用例——机制移除；改/增 4 个 window focus 用例——可见触发/不可见跳过/面板内点击不触发/ask 弹窗期间回归不二次弹窗）。L2 1774→1775，全量 2298→2299。
- 2026-08-01（验收修复 1：删除单条启停）：删除 hooks-config-store.test.ts（21 用例）+ hooks-config-disable.test.tsx（10 用例）；model 22→17（删 filterDisabled describe 5 条）；sync 9→8（删 filterDisabled 保存链路用例）；panel 17（去失效记录条描述，用例数不变）。L2 1811→1774，全量 2335→2298。
- 2026-08-01（验收 #1 修复）：hooks-config-panel.test.tsx 15→17（+2 focusin relatedTarget 判定——面板内焦点转移不重读/面板外进入重读）。L2 1809→1811。全量 2333→2335。
- 2026-08-02（agent 侧栏 7 项人工审查修复）：L1：claude_history/ops.rs 13→7（删 rename 命令 6 条测试——问题 7 全链路移除），L1 446→440。L2：claude 历史会话 6 文件 109→115（model 37→40 +3 deriveActiveSessionStatuses——问题 2 四态同源；view 28→33 +5 受控 props/组收起计数/双击动作弹窗/标题覆盖/字号断言——问题 3/4/5/6；hook 14→13 -1 删 updateLocalTitle——问题 7；row 11→15 +4 四态标记/字号——问题 2/4；删 input-dialog 12 + 新建 action-dialog 7——问题 5/7）；agent-status-hook 35→36（+1 sessionId 字段——问题 6）；agent-status-view 15→19（+4 双行布局/相对时间——问题 1/4）；use-xterm-lifecycle 79→80（+1 HUK9 Notification status undefined——问题 2）；terminal-registry 15→18（+3 sessionId/status 存储与 merge——问题 2）；ipc-claude-history-contract 12→8（-4 rename 四维——问题 7）；colors 13 不变（断言 7→8 token 计数）。L2 1911→2013（117 文件）。L4：E2E 删重命名用例 +1 历史区四态用例（34 不变）；历史区行操作用例前置展开全部组（组默认收起——问题 3）。全量 2333→2603。
- 2026-08-01（Phase 3 Stage 10 全量重算）：按计数口径实跑重写。L1：新增 hooks/config.rs（18 用例，P3-BE 读写命令纯逻辑），usage.rs 23→28（+5 cache 字段用例），L1 359→382（18→19 文件）。L2：新增 hooks-config-entry（7）+ hooks-config-gui（21）+ hooks-config-sync（9），hooks-config-panel 9→15（+6 注入状态条/失效记录条）；panel-registry 29→32（+3 hooksConfig 六面板注册）、command-catalog 13→14（+1 openHooksConfig 入口命令契约）；修正「主题/配色/基础」类目标头 113→108（原与文件实际和 108 不符）。L2 1717→1809（106→111 文件）。L4：新增 hooks 配置面板保存链路（P3-TE-18），23→24 active（25→26 总，含 2 skip）。全量 2227→2333。
- 2026-08-01（Phase 3 Stage 07）：新增 hooks-config-disable.test.tsx（10 用例，P3-TE-15/16——禁用状态往返/失效禁用记录/事件树启停 checkbox）；hooks-config-handlerform.test.tsx 托管断言适配（lockRow 移除禁用 checkbox——C13-8 禁禁用=不渲染，P3-FE-19）。hooks 配置面板 8→9 文件 159→169。L2 1717→1727。全量 2217→2227。
- 2026-08-01（Phase 3 Stage 04）：新增 hooks-config-jsonmode.test.tsx（17 用例，P3-TE-09/10——CM6 渲染/schema 扩展注册/非法 JSON 校验上报/事件导航跳转/MatcherTester 试测）；hooks-config-panel.test.tsx 9 用例适配 JsonMode 接入（占位文案断言 → JsonMode value 传递断言）。补登 Stage 01-03 遗留 6 文件：ipc-hooks-config-contract（12）+ hooks-config-catalog（19）+ hooks-config-matcher（21）+ hooks-config-model（22）+ hooks-config-store（21）+ hooks-config-panel（9）。vitest.config.ts 新增 `server.deps.inline: ["codemirror-json-schema"]`（0.8.1 ESM dist 无扩展名相对导入，Node ESM 无法解析）。L2 1593→1717（99→106 文件）。全量 2093→2217。
- 2026-07-29（Phase 2 FIX-DOC-03）：Stage 01-04 完成后按 `npm test` 实跑重写。L2：ipc-hooks-contract 21→22（+1 ContextUsage 四字段键集合守卫）、notifications 32→25（-7 删 toast 路由化 6 用例+首 token/sendToastNotification 适配）、agent-status-hook 31→35（+4 行建模新语义——纯 shell 无行/双通道建行/三通道删行/初始扫描携 transcriptPath 拉 usage/reconcile 对账/cache 字段/contextUsage 静默 catch 可观测）、agent-status-view 11 不变（用量新口径重算）、tab-title-registry 8→13（+5 首 token 匹配——带参命中/空命令行/仅空白/首 token 无规则）、terminal-registry 7→15（+8 setClaudeSession 全分支+sessionChange 事件+幂等保留旧 session）、terminal-registry-subscribe 3→7（+4 sessionChange 通知/setClaudeSession 触发 sessionChange）。L2 1578→1593。L4：Agent Status 静态行反转（纯 shell 无行）+ 动态四态首个信号即建行 + 新增 R2/R3/R4 变体 3 条防复发；22→23（21 active + 2 skip）。全量 2075→2091。
- 2026-07-28（Phase 2 FIX-DOC-01）：按实跑全量重写。L1：mod.rs 剔除 hooks_context_usage（不存在）、usage.rs 改正函数名为 parse_usage_line/scan_transcript_usage。L2：ipc-hooks-contract 16→21（+5 条 contextUsage 合约 + HookEventPayload 字段约束）、notifications 33→32（-1，剔 4 项不存在描述并补 sendClickableNotification 回调绑定/onclick 聚焦路由）、agent-status-hook 21→31（+10，剔"轮询"改事件驱动 + FE-03 TerminalRegistry.subscribe 增删 + FE-04 dockviewApi 标题查找与回退 + FE-05 null 不覆盖 + FE-06 无额外订阅）、agent-status-view 8→11（+3，剔 tooltip/加载态/错误态 + 补用量条分段颜色断言 + 点击路由 switchToPageAndFocus）、colors 12→13（+1 AGENT_STATUS_USAGE_COLORS 三 token）。补登 2 新文件：panelId.test.ts（5）+ terminal-registry-subscribe.test.ts（3）。L2 1552→1578，97→99 文件。L4：Agent Status 原 skip 拆为 3 条 active（视图存在性 + 静态行渲染 + 动态四态信号文件），toast 保持 it.skip，CSP 脚本用例保持 skip；18→22（20 active + 2 skip）。全量 2045→2075。
- 2026-07-27：Stage 2 通知/Agent 状态——L1 hooks 模块拆分为 5 文件（mod.rs 10→8、inject.rs 12→20、新增 signal.rs 9 + watcher.rs 6 + usage.rs 23），L1 274→318。L2 IPC 层 ipc-contract 53→65（+12：notification + hooks_context_usage 合约）、ipc-hooks-contract 8→16（+8：hooks_context_usage）；新增「通知/Agent 状态」类目 3 文件 62 用例（notifications 33 + agent-status-hook 21 + agent-status-view 8），L2 1415→1497。L4 新增 Agent Status 视图 1 用例，17→18。全量 1822→1949。
- 2026-07-26：hooks 宿主侧增强（P1-DOC）——L1 新增「hooks」模块 2 文件 22 用例（mod.rs 10 + inject.rs 12）+ pty/spawn.rs 28→29（+1 env 注入）。L2 新增「hooks」IPC 合约 1 文件 8 用例（ipc-hooks-contract）+「claude-status」纯函数 1 文件 14 用例；终端面板 use-xterm-lifecycle 71→77（+6 hook-event 过滤/F3 四态）。L4 新增 hooks 注入/信号 2 用例，15→17。L1 251→274，L2 1387→1415，L4 15→17，全量 1769→1822。
- 2026-07-19：commit 视图（CV-DOC）——L1 git/mod.rs 62→70（+8：git_file_at_head/recurse_untracked_dirs/oldPath/rename_detection），L1 243→251。L2 新增「Commit 视图」类目 1 文件 28 用例 +「Diff/GitShow 面板」3 文件 40 用例（diff-alignment 16 + diff-panel 11 + gitshow-panel 13），既有文件增量：ipc-contract 50→53 + git-gutter 20→28 + panel-registry 23→29 + title-manager 36→44 + workspace-file-panel-types 11→13。L2 1207→1387。L4 新增「commit 视图」describe 2 用例，14→15。全量 1580→1769。
- 2026-07-19（fix）：跨区拖拽修复 + 中线 zone 判定 + splitRatio 重置——activityBar 16→29（+13 跨区/边界/清理），sideBarArea 13→14（+1 splitRatio 重置）。L2 1193→1207，全量 1566→1580。
- 2026-07-19：侧栏视图系统（SB-26）——新增 L2「侧栏视图」类目 6 文件 118 用例（sideBarState 50 + sideViewRegistry 7 + sideBar 19 + activityBar 16 + sideBarArea 13 + workspace-sideviews 13）；L4 新增「侧栏视图」describe 2 用例（12→14）。L2 1075→1193，全量 1446→1566。
- 2026-07-18：DBG-11 同步——纳入 Stage 1/2 新增用例（DBG-4 契约守卫 3 条、DBG-9 switchToPage 时序 14 条、DBG-10 explorer-sandbox-race 13 条），L2 1045→1075，全量 1416→1446。
- 2026-07-17：重写——实测全量用例数（L1=243, L2=1045, L3=116, L4=12），统一计数口径，标注 E2E 键盘局限，声明唯一真值源。纳入 Stage 9/10 新增用例。
- 2026-07-13（旧版）：全量 ~1234 用例（Rust 193 + 前端 1020 + L3 9 + E2E 12），计数失实且 L3 少报 107 用例。
