# 自动化测试用例清单

> **本文档是项目用例数唯一真值源。** 所有 CLAUDE.md、README、CI 配置中引用的用例数均以此文件为准。更新测试后必须同步本文档。

全量 **1566** 用例（Rust 243 + 前端 1193 + L3 116 + E2E 14），2026-07-19 实测更新。

> **计数口径**：前端 (L2) 用例数以 `grep -cE '^\s*(it|test)\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx` 展开的 `it`/`test` 块数为准（Vitest 实际运行数）；L3 同理 `test/terminal/*.test.ts`；Rust (L1) 以 `grep -c '#\[test\]'` 统计的 `#[test]` 属性数为准。L3 的 116 用例同时被 L2 (`npm test`) 和独立 L3 (`npm run test:l3`) 执行，但此处各层独立计数，不做去重。

## L1 — Rust 单元/集成测试（11 文件 / 243 用例）

运行：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/src/git/mod.rs` | 62 | status_to_str/git_status/git_diff/line_callback/dunce/序列化/repository |
| `src-tauri/src/pty/reader.rs` | 30 | ConPTY 启动序列剥离/DA1 查询检测/apply_startup_strip/should_inject_da1/mirror_da1_query/16KB 边界 |
| `src-tauri/src/pty/spawn.rs` | 28 | compute_conpty_flags/flag 常量/ConPtyMaster MasterPty trait/AttrList + env 注入/cwd 规范化/ring buffer 回放/session 隔离 |
| `src-tauri/src/fs/mod.rs` | 28 | read_dir/write_file/create_dir/delete/rename + 命令包装单测 |
| `src-tauri/src/notify/mod.rs` | 24 | FileWatcher 生命周期 + classify_by_kind 事件分类（全 7 种 EventKind） |
| `src-tauri/src/state.rs` | 15 | ring buffer append+eviction+换行边界/validate_path_within_root 沙箱 |
| `src-tauri/src/pty/shell.rs` | 15 | pwsh 发现/shell-integration.ps1 嵌入/UTF-16LE Base64 往返/which_full_path/shell 白名单 |
| `src-tauri/src/settings.rs` | 15 | 读写往返/文件不存在/JSON 损坏回退 .bak/浅合并/shadow 目录 + 命令包装单测 |
| `src-tauri/src/notify/pool.rs` | 13 | LruWatcherPool: 缓存命中/LRU 淘汰/pause_all_except/replace/remove/stop_all/Drop |
| `src-tauri/tests/pty_integration_tests.rs` | 7 | PTY 往返/OSC cwd 解析/resize 生效/kill 无孤儿/Custom ConPTY spawn/reattach |
| `src-tauri/src/error.rs` | 4 | 序列化/Display/From<io::Error>/SessionNotFound |
| `src-tauri/src/lib.rs` | 2 | ping 返回 pong/`get_windows_build_number` 返回数字 |

> `pty/mod.rs`、`pty/win_build.rs`、`main.rs` 不含 `#[test]`，不在此列。

## L2 — 前端单元/集成测试（78 文件 / 1193 用例）

运行：`npm test`（Vitest + jsdom）

### IPC 层（2 文件 / 51 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 50 | pty/fs/settings/notify/git 全模块 IPC 命令合约验证 + DBG-4 PTY 命令 payload 契约守卫（3 条：键集合精确匹配） |
| `src/__tests__/ipc-ping.test.ts` | 1 | mockIPC ping/pong 拦截 |

### 终端面板（14 文件 / 182 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-xterm-lifecycle.test.ts` | 71 | PTY spawn/exit/setupRetry/快捷键/rAF 轮询/ResizeObserver/字体/OSC 52/OSC 133/OSC 8/键盘委托 |
| `src/__tests__/use-xterm-output.test.ts` | 37 | DEC 2026/直写阈值/交替缓冲/Idle+Max 合帧/Uint8Array/非焦点降频/cancelPendingFlush/visibleRef 门控 |
| `src/__tests__/can-fit.test.ts` | 15 | 五条件守卫 + null/undefined 参数防护 |
| `src/__tests__/use-xterm-integration.test.ts` | 12 | 轻 mock（真实 Terminal/FitAddon，仅 mock ipc/pty）；rAF 轮询失败回退/term.onData→pty.write/visible 切换 WebGL 释放重建 |
| `src/__tests__/keyboard.test.ts` | 12 | `createTerminalShortcuts()` copy/paste/newline 经 active 指针派发 |
| `src/__tests__/tab-title-registry.test.ts` | 8 | register/match、大小写、覆盖、`_reset()`、单例校验 |
| `src/__tests__/terminal-registry.test.ts` | 7 | register/get/remove/has/幂等/clear |
| `src/__tests__/e2e-gating-terminal.test.ts` | 5 | E2E helper 终端门控（`__e2e_sessionReady`/`__e2e_writeToPty` 等） |
| `src/__tests__/tab-rules.test.ts` | 5 | side-effect import + `_reset()` 后手动注册 |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整链路 |
| `src/__tests__/terminal.test.tsx` | 4 | TerminalPanel 组件：loading 遮罩/Windows build/spawn |
| `src/__tests__/active-terminal.test.ts` | 4 | active 指针 set/get/覆盖、clear 仅匹配时生效 |
| `src/__tests__/detect-webgl.test.ts` | 3 | WebGL2 可用/不可用/抛异常 |
| `src/__tests__/terminal-strictmode.test.ts` | 2 | `smGuardRef` 防双重挂载 |

### 编辑器面板（6 文件 / 111 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-code-mirror.test.ts` | 34 | 字体扩展/Compartment reconfigure/handleSave/gitDiff/slterm:file-saved + file-saved-as event |
| `src/__tests__/language-mapping.test.ts` | 23 | 扩展名→CodeMirror 语言映射 + 未知回退 |
| `src/__tests__/git-gutter.test.ts` | 20 | StateEffect → RangeSet 映射/GutterMarker DOM/SpacerMarker |
| `src/__tests__/editor-confirm.test.ts` | 11 | dirty/clean 外部修改确认/订阅取消/kind 过滤 |
| `src/__tests__/editor.test.tsx` | 9 | EditorPanel 渲染/panelId/filePath 传递 + `overflow: clip` 样式 |
| `src/__tests__/editor-font.test.ts` | 8 | 字体 CSS 选择器（`.cm-scroller` vs `.cm-editor`） |
| `src/__tests__/editor-keyboard.test.ts` | 7 | `createEditorShortcuts()` save/toggleWordWrap 经 active 指针派发 |
| `src/__tests__/active-editor.test.ts` | 5 | active 指针 set/get/覆盖、clear 仅匹配时生效 |

### 工作区/布局/页签（12 文件 / 167 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/title-manager.test.ts` | 36 | terminal-N 递增/编辑器 basename/同名冲突相对路径/handleSaveAs/onDeletePage |
| `src/__tests__/panel-registry.test.ts` | 23 | 注册表/PANEL_TYPES/isValidPanelType/FILE_PANEL_TYPES |
| `src/__tests__/layout-serde.test.ts` | 21 | 旧格式修补/白名单过滤/深拷贝/嵌套 branch/activeGroup 保留 |
| `src/__tests__/workspace-header-actions.test.tsx` | 16 | RightHeader Watermark 按钮/页签操作 |
| `src/__tests__/workspace-switch-order.test.tsx` | 14 | DBG-9：`switchToPage` 时序契约——`setProjectRoot` 先于 `setActivePage` 生效/reject 降级/SEC-01 effect 兜底/兼容性排查 |
| `src/__tests__/workspace-defaulttab.test.tsx` | 14 | DefaultTab 渲染 + `onDidParametersChange` 事件结构回归 |
| `src/__tests__/workspace-file-panel-types.test.ts` | 11 | FILE_PANEL_TYPES/isAlwaysRenderPanel |
| `src/__tests__/default-layout-format.test.ts` | 8 | grid/panels/activeGroup/orientation 格式验证 |
| `src/__tests__/layout-switch.test.ts` | 7 | 页面切换集成/自切换守卫 |
| `src/__tests__/workspace-multi-instance.test.tsx` | 4 | 多 Dockview 实例惰性初始化/删除活跃页面 |
| `src/__tests__/workspace-e2e-ready.test.tsx` | 4 | `__slterm_e2e_workspaceReady` 标记同步性 |
| `src/__tests__/workspace.test.tsx` | 4 | Dockview 初始化/项目页面关联 |

### Store 状态管理（4 文件 / 77 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/projects.test.ts` | 41 | Project/Page CRUD/持久化/version 递增/ID 生成/expandedNodes |
| `src/__tests__/font-size.test.ts` | 16 | 默认值/clamp/loadFromDisk/debounce 持久化 |
| `src/__tests__/keybindings.test.ts` | 16 | setBinding/clearBinding/resetAll/sanitize/loaded 守卫/debounce |
| `src/__tests__/layout.test.ts` | 4 | activePageId 设置/清空/重复 |

### 资源管理器（10 文件 / 156 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/explorer-git-status.test.tsx` | 32 | gitStatusMap 查表着色/配色 token/F5 untracked/slterm:file-saved |
| `src/__tests__/file-icon.test.tsx` | 18 | 扩展名图标 + 目录图标 + git 状态着色 |
| `src/__tests__/explorer-refresh-preserve.test.tsx` | 17 | reloadPreservingExpanded 递归重建/边界容错/三条触发路径/竞态 |
| `src/__tests__/explorer-file-viewer.test.tsx` | 16 | handleOpenFile 面板分派/FileViewerRegistry/htmlviewer 回退 |
| `src/__tests__/use-file-tree.test.ts` | 15 | loadRoot/loadDirectory/toggleExpand/generation 取消 |
| `src/__tests__/explorer-root-contextmenu.test.tsx` | 14 | 根节点右键菜单/新建文件+文件夹 |
| `src/__tests__/explorer-delete.test.tsx` | 13 | ask 弹窗分支/右键菜单/操作失败 UI 通知 |
| `src/__tests__/explorer-notify.test.tsx` | 12 | startWatch 调用时机/loadRoot/toggleExpand |
| `src/__tests__/explorer-sandbox-race.test.tsx` | 13 | DBG-10：路径沙箱竞态回归——deferred `setProjectRoot` 验证 resolve 前 `readDir` 不被调用/resolve 后正常加载/reject 降级 |
| `src/__tests__/explorer-rootpath-clear.test.tsx` | 6 | rootPath 变化清空/快速切换 gen 丢弃/同值不清空 |

### 侧栏（1 文件 / 33 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sidebar-actions.test.ts` | 33 | 树结构/右键菜单/内联重命名/项目删除确认/布局 CSS/添加项目 |

### 侧栏视图（6 文件 / 118 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sideBarState.test.ts` | 50 | toggleViewPure/moveButtonPure/deriveLayout/reconcileZones/sanitizeSideBar + S1-S6 场景序列 |
| `src/__tests__/sideViewRegistry.test.ts` | 7 | register/getAll/get/重复注册覆盖/未注册 get→undefined/_reset 隔离 |
| `src/__tests__/sideBar.test.ts` | 19 | 默认值/toggle/move 经 store/loadFromDisk 5 分支/loaded 守卫/debounce saveSettings payload 键集合精确匹配 |
| `src/__tests__/activityBar.test.tsx` | 16 | 上下区按钮组/active 指示条/click→toggleView/dragStart dataTransfer 内容/dragOver computeDropTarget/drop→moveButton/dragEnd 清拖拽/hover |
| `src/__tests__/sideBarArea.test.tsx` | 13 | 四态布局/Allotment preferredSize splitRatio/display:none-flex 切换/保挂载/跨区卸载重建/props 透传/onChange→setSplitRatio/PANEL_BG token |
| `src/__tests__/workspace-sideviews.test.tsx` | 13 | 活动栏 pane 40px 固定/侧栏区 pane visible=anyOpen 四态/preferredSize 来自 store/onChange→setWidth/主区 pane/props 透传 |

### 快捷键/命令系统（8 文件 / 116 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/shortcuts.test.ts` | 50 | 注册/注销/引用计数/上下文栈/IME/setOverrides 重绑/解绑/降级/冲突/resolve/forceContext/export/list/监听器 spy |
| `src/__tests__/keystroke.test.ts` | 18 | formatKeystroke/parseKeystroke/isValidKeystrokeString/format∘parse 恒等 |
| `src/__tests__/global-commands.test.ts` | 13 | `createGlobalShortcuts(getApi)` 延迟求值/Ctrl+W 关闭/无面板透传 |
| `src/__tests__/command-catalog.test.ts` | 10 | 6 命令齐全/defaultKey 合法/commandFromMeta |
| `src/__tests__/reserved.test.ts` | 8 | isReserved 各 context/保留键命中/global 两集并集 |
| `src/__tests__/use-panel-focus.test.ts` | 5 | focusin→pushContext+onActivate/focusout→popContext+onDeactivate/卸载清理 |
| `src/__tests__/wire-keybindings.test.ts` | 3 | 立即应用/store 变更重应用/unsubscribe |

### 主题/配色/基础（5 文件 / 64 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/path.test.ts` | 27 | normalizePath/basename/isChildOf/relativePath 边界覆盖 |
| `src/__tests__/inject-script.test.ts` | 21 | HTML 脚本注入/`</script>` 转义/幂等/大小写不敏感/键盘转发+片段链接拦截 |
| `src/__tests__/colors.test.ts` | 12 | 配色 token 值校验（压缩后，原 61 条冗余断言合并） |
| `src/__tests__/theme.test.ts` | 12 | terminalOptions: ANSI 16 色/font/cursor/scrollback/kittyKeyboard |

### 启动/关闭（3 文件 / 18 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/close-handler.test.ts` | 11 | flush layout→saveAllProjects→localStorage/超时/异常 |
| `src/__tests__/startup-restore.test.ts` | 4 | localStorage 恢复/空/异常降级/Loading→ready |
| `src/__tests__/bootstrap.test.ts` | 3 | `__TAURI_INTERNALS__` 轮询/立即挂载/永不就绪 |

### 文件查看器/HTML（3 文件 / 68 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/html-panel.test.tsx` | 40 | 三态渲染/竞态取消/sandbox 属性/postMessage 键盘转发/origin 校验/片段链接拦截 |
| `src/__tests__/file-viewer-registry.test.ts` | 25 | 扩展名注册/策略链式调用/隐藏文件排除/大小写 |
| `src/__tests__/csp-config.test.ts` | 4 | tauri.conf.json CSP 不变量：script-src unsafe-inline/dangerousDisableAssetCspModification/default-src 严格 |

### E2E 辅助/门控测试（5 文件 / 21 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/e2e-build-config.test.ts` | 6 | VITE_E2E build 配置不变量 |
| `src/__tests__/app.test.tsx` | 5 | `__slterm_e2e_createProject` 行为/E2E pending 标记 |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | writeClipboard/createProject 函数可用性 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | pending 标记→localStorage 恢复交互 |
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

## L4 — E2E 端到端测试（1 文件 / 14 用例）

运行：`npm run e2e`（= `npm run build:e2e` + `npm run wdio`）  
技术栈：WDIO + `@wdio/tauri-service` 1.1.0 + embedded driver（`webview2-com` COM 直连 `ICoreWebView2`）

| 用例 | 覆盖范围 |
|------|---------|
| 应正常启动并显示 slTerminal 标题 | 应用启动 + 窗口标题验证 |
| 打开终端→写入文本→验证缓冲含 e2e_marker | 完整 PTY 通信链路（createProject→addPanel→spawn→write→read） |
| 终端面板可通过 E2E helper 写入文本并读取 | 键盘快捷键→剪贴板→终端粘贴 |
| 终端页签标题为 terminal-N | 页签默认标题 + 动态修改（`api.setTitle`） |
| 编辑器页签标题为文件名 | 编辑器标题 = basename + registerAndRecompute |
| 同名文件冲突时显示相对路径 | titleManager 冲突检测（`src/index.ts` vs `lib/index.ts`） |
| 关闭同名面板后剩余面板切回 basename | 关闭冲突面板后自动重算标题 |
| 聚焦编辑器后 Ctrl+S → 经 capture 路径真实写盘（mtime 更新） | capture 监听 + context 栈匹配 + 命令 handler + 写盘全链路 |
| 编辑器 dirty→clean 保存 | 外部写盘→auto-reload→Ctrl+S 保存新内容→磁盘断言 |
| iframe 内 Ctrl+W → postMessage 转发关闭该 HTML 页签 | postMessage 键盘桥：注入脚本→parent.postMessage→handleMessage 校验 origin→ShortcutRegistry 分发→global.closeTab（forwardGlobalShortcuts 已删除，FE-13） |
| 内联 `<script>` 与内联事件属性在预览中执行 | CSP 修复：真实 WebView2 强制 CSP 下经 srcdoc opaque origin 验证内联脚本+onclick 执行 |
| should preserve terminal content after switching to another page and back | H6 终端跨页面存活（多 Dockview 实例 + CSS 显隐） |
| 点击侧栏按钮切换视图显示/隐藏（R1/R2） | 活动栏按钮真实点击→open 状态变化→侧栏区 visible 联动→store 状态断言 |
| 拖拽按钮跨区移动（R6/R7） | DataTransfer 合成拖拽事件跨区→zones + open 跟随（R6 跟随替换/R7 未打开仅归属）→store 状态断言 |

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

- 2026-07-19：侧栏视图系统（SB-26）——新增 L2「侧栏视图」类目 6 文件 118 用例（sideBarState 50 + sideViewRegistry 7 + sideBar 19 + activityBar 16 + sideBarArea 13 + workspace-sideviews 13）；L4 新增「侧栏视图」describe 2 用例（12→14）。L2 1075→1193，全量 1446→1566。
- 2026-07-18：DBG-11 同步——纳入 Stage 1/2 新增用例（DBG-4 契约守卫 3 条、DBG-9 switchToPage 时序 14 条、DBG-10 explorer-sandbox-race 13 条），L2 1045→1075，全量 1416→1446。
- 2026-07-17：重写——实测全量用例数（L1=243, L2=1045, L3=116, L4=12），统一计数口径，标注 E2E 键盘局限，声明唯一真值源。纳入 Stage 9/10 新增用例。
- 2026-07-13（旧版）：全量 ~1234 用例（Rust 193 + 前端 1020 + L3 9 + E2E 12），计数失实且 L3 少报 107 用例。
