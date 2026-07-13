# 自动化测试用例清单

全量 **~1234** 用例（Rust 193 + 前端 1020 + L3 9 + E2E 12），2026-07-13 实测更新。

> L3 9 用例同时被 `npm test`（L2）和 `npm run test:l3`（L3）覆盖，此处保留独立列出。去重后唯一用例数 **1220**。

## 前端测试（67 文件 / 1016 用例）— Vitest + jsdom

> `npm test` 实际运行 67 文件 1016 用例（含 `test/terminal/` 下 2 文件 9 用例）。L3 用例详见独立章节。

### IPC 层（2 文件 / 48 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 47 | pty/fs/settings/notify/git 全模块 IPC 命令合约验证 |
| `src/__tests__/ipc-ping.test.ts` | 1 | mockIPC ping/pong 拦截 |

### 终端面板（12 文件 / 175 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-xterm-output.test.ts` | 37 | DEC 2026/直写阈值/交替缓冲/Idle+Max 合帧/Uint8Array/非焦点降频/cancelPendingFlush |
| `src/__tests__/use-xterm-lifecycle.test.ts` | 74 | PTY spawn/exit/setupRetry/快捷键/rAF 轮询/ResizeObserver/字体/OSC 52/OSC 133/OSC 8/键盘委托 |
| `src/__tests__/can-fit.test.ts` | 15 | 五条件守卫 + null/undefined 参数防护 |
| `src/__tests__/keyboard.test.ts` | 12 | `createTerminalShortcuts()` copy/paste/newline 经 active 指针派发 |
| `src/__tests__/tab-title-registry.test.ts` | 8 | register/match、大小写、覆盖、`_reset()`、单例校验 |
| `src/__tests__/terminal-registry.test.ts` | 7 | register/get/remove/has/幂等/clear |
| `src/__tests__/tab-rules.test.ts` | 5 | side-effect import + `_reset()` 后手动注册 |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整链路 |
| `src/__tests__/terminal.test.tsx` | 4 | TerminalPanel 组件：loading 遮罩/Windows build/spawn |
| `src/__tests__/active-terminal.test.ts` | 4 | active 指针 set/get/覆盖、clear 仅匹配时生效 |
| `src/__tests__/detect-webgl.test.ts` | 3 | WebGL2 可用/不可用/抛异常 |
| `src/__tests__/terminal-strictmode.test.ts` | 2 | `smGuardRef` 防双重挂载 |

### 编辑器面板（8 文件 / 111 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-code-mirror.test.ts` | 28 | 字体扩展/Compartment reconfigure/handleSave/gitDiff/slterm:file-saved event |
| `src/__tests__/language-mapping.test.ts` | 23 | 扩展名→CodeMirror 语言映射 + 未知回退 |
| `src/__tests__/git-gutter.test.ts` | 20 | StateEffect → RangeSet 映射/GutterMarker DOM/SpacerMarker |
| `src/__tests__/editor-confirm.test.ts` | 11 | dirty/clean 外部修改确认/订阅取消/kind 过滤 |
| `src/__tests__/editor.test.tsx` | 9 | EditorPanel 渲染/panelId/filePath 传递 + `overflow: clip` 样式 |
| `src/__tests__/editor-font.test.ts` | 8 | 字体 CSS 选择器（`.cm-scroller` vs `.cm-editor`） |
| `src/__tests__/editor-keyboard.test.ts` | 7 | `createEditorShortcuts()` save/toggleWordWrap 经 active 指针派发 |
| `src/__tests__/active-editor.test.ts` | 5 | active 指针 set/get/覆盖、clear 仅匹配时生效 |

### 工作区/布局/页签（11 文件 / 136 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/title-manager.test.ts` | 29 | terminal-N 递增/编辑器 basename/同名冲突相对路径/handleSaveAs |
| `src/__tests__/panel-registry.test.ts` | 23 | 注册表/PANEL_TYPES/isValidPanelType/FILE_PANEL_TYPES |
| `src/__tests__/layout-serde.test.ts` | 16 | 旧格式修补/白名单过滤/深拷贝 |
| `src/__tests__/workspace-header-actions.test.tsx` | 16 | RightHeader Watermark 按钮/页签操作 |
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

### 资源管理器（9 文件 / 143 用例）

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
| `src/__tests__/explorer-rootpath-clear.test.tsx` | 6 | rootPath 变化清空/快速切换 gen 丢弃/同值不清空 |

### 侧栏（1 文件 / 33 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sidebar-actions.test.ts` | 33 | 树结构/右键菜单/内联重命名/项目删除确认/布局 CSS/添加项目 |

### 快捷键/命令系统（8 文件 / 115 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/shortcuts.test.ts` | 44 | 注册/注销/引用计数/上下文栈/IME/setOverrides 重绑/解绑/resolve/export |
| `src/__tests__/keystroke.test.ts` | 26 | formatKeystroke/parseKeystroke/isValidKeystrokeString |
| `src/__tests__/global-commands.test.ts` | 13 | `createGlobalShortcuts(getApi)` 延迟求值/ Ctrl+W 关闭 |
| `src/__tests__/command-catalog.test.ts` | 10 | 6 命令齐全/defaultKey 合法/commandFromMeta |
| `src/__tests__/reserved.test.ts` | 8 | isReserved 各 context/保留键命中/global 两集并集 |
| `src/__tests__/forward-global-shortcuts.test.ts` | 6 | 命中全局键→preventDefault+重放/非全局键不转发/detach |
| `src/__tests__/use-panel-focus.test.ts` | 5 | focusin→pushContext+onActivate/focusout→popContext+onDeactivate/卸载清理 |
| `src/__tests__/wire-keybindings.test.ts` | 3 | 立即应用/store 变更重应用/unsubscribe |

### 主题/配色（2 文件 / 73 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/colors.test.ts` | 61 | GIT_FILE_COLORS/GIT_GUTTER_COLORS/EXPLORER_COLORS/通用 UI hex 校验 |
| `src/__tests__/theme.test.ts` | 12 | terminalOptions: ANSI 16 色/font/cursor/scrollback/kittyKeyboard |

### 启动/关闭（3 文件 / 18 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/close-handler.test.ts` | 11 | flush layout→saveAllProjects→localStorage/超时/异常 |
| `src/__tests__/startup-restore.test.ts` | 4 | localStorage 恢复/空/异常降级/Loading→ready |
| `src/__tests__/bootstrap.test.ts` | 3 | `__TAURI_INTERNALS__` 轮询/立即挂载/永不就绪 |

### E2E 辅助测试（3 文件 / 11 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/app.test.tsx` | 5 | `__slterm_e2e_createProject` 行为/E2E pending 标记 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | pending 标记→localStorage 恢复交互 |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | writeClipboard/createProject 函数可用性 |

### 通用工具/其他（5 文件 / 80 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/path.test.ts` | 27 | normalizePath/basename/isChildOf/relativePath 边界覆盖 |
| `src/__tests__/file-viewer-registry.test.ts` | 25 | 扩展名注册/策略链式调用/隐藏文件排除/大小写 |
| `src/__tests__/html-panel.test.tsx` | 21 | 三态渲染/竞态取消/sandbox 属性/forwardGlobalShortcuts |
| `src/__tests__/csp-config.test.ts` | 4 | tauri.conf.json CSP 不变量：script-src unsafe-inline/dangerousDisableAssetCspModification/default-src 严格/仅内联边界守卫 |
| `src/__tests__/error-boundary.test.tsx` | 3 | 正常透传/抛错 UI/`__sltermError` 赋值 |

## Rust 后端测试（12 文件 / 193 用例）— `cargo test`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/src/git/mod.rs` | 58 | status_to_str/git_status/git_diff/line_callback/dunce/序列化/repository |
| `src-tauri/src/pty/reader.rs` | 28 | ConPTY 启动序列剥离/DA1 查询检测/apply_startup_strip/should_inject_da1/mirror_da1_query |
| `src-tauri/src/notify/mod.rs` | 24 | FileWatcher 生命周期（创建/stop/Drop/替换/幂等）+ classify_by_kind 事件分类（全 7 种 EventKind） |
| `src-tauri/src/fs/mod.rs` | 16 | read_dir（列表/过滤 .git/创建目录/删除/重命名）；write_file（CRLF/LF 保留/新建默认 CRLF/混合行尾） |
| `src-tauri/src/state.rs` | 15 | PtyState 空初始化/AppState 创建/ring buffer append+eviction+换行边界/validate_path_within_root 沙箱（root_opt=None/根内/子目录/根外/不存在的路径/路径穿越/符号链接/canonicalize 失败） |
| `src-tauri/src/pty/spawn.rs` | 14 | compute_conpty_flags（Win10/Win11 21H2/Win11 22H2+）/flag 常量值/ConPtyMaster MasterPty trait/AttrList |
| `src-tauri/src/notify/pool.rs` | 12 | LruWatcherPool: 缓存命中/LRU 淘汰/pause_all_except/replace/remove/stop_all/Drop |
| `src-tauri/src/settings.rs` | 8 | 读写往返/文件不存在/JSON 损坏回退 .bak/无 .bak 降级/浅合并/shadow 目录 |
| `src-tauri/src/pty/shell.rs` | 7 | pwsh 发现/shell-integration.ps1 嵌入/UTF-16LE Base64 往返/which_full_path |
| `src-tauri/src/error.rs` | 4 | 序列化/Display/From<io::Error>/SessionNotFound 序列化 |
| `src-tauri/src/lib.rs` | 2 | ping 返回 pong/`get_windows_build_number` 返回数字 |

**集成测试**（`src-tauri/tests/`，1 文件 / 5 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/tests/pty_integration_tests.rs` | 5 | PTY 往返/OSC cwd 解析/resize 生效/kill 无孤儿进程/Custom ConPTY spawn |

Rust 模块中无测试的文件（4 个）：`pty/mod.rs`、`pty/build.rs`、`main.rs`、`error.rs` 外其余源码文件均含测试。

> **注意**：`src-tauri/tests/integration_test.rs` 不存在（旧文档错误引用）。`state.rs` 含 `tests` 模块（5 条 ring buffer）+ `sandbox_tests` 模块（10 条 validate_path_within_root），总计 15 条。

## L3 终端 headless 测试（2 文件 / 9 用例）— `npm run test:l3`

> 同时被 `npm test`（L2）包含执行。独立 L3 运行使用 `vitest.l3.config.ts`（`environment: 'node'`）。

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `test/terminal/terminal-serialize.test.ts` | 5 | 基本文本序列化、多行输出（12 行）、ANSI 颜色保留（红/绿/粗体+重置码）、大块数据（>1000 字符、哨兵验证）、CSI 光标定位（CUP `\x1b[5;10H`） |
| `test/terminal/keyboard.test.ts` | 4 | pwsh 提示符渲染快照、Claude TUI ANSI 颜色快照、Shift+Tab CBT 编码（`\x1b[Z`）、Ctrl+C ETX 编码（`\x03` onData） |

## E2E 端到端测试（1 文件 / 12 用例）— `npm run wdio`

| 用例 | 覆盖范围 |
|------|---------|
| 应正常启动并显示 slTerminal 标题 | 应用启动 + 窗口标题验证 |
| 打开终端→写入文本→验证缓冲含 e2e_marker | 完整 PTY 通信链路（createProject→addPanel→spawn→write→read） |
| 终端面板可通过 E2E helper 写入文本并读取 | 键盘快捷键→剪贴板→终端粘贴 |
| 终端页签标题为 terminal-N | 页签默认标题 + 动态修改（`api.setTitle`） |
| 编辑器页签标题为文件名 | 编辑器标题 = basename + registerAndRecompute |
| 同名文件冲突时显示相对路径 | titleManager 冲突检测（`src/index.ts` vs `lib/index.ts`） |
| 关闭同名面板后剩余面板切回 basename | 关闭冲突面板后自动重算标题 |
| 聚焦编辑器后 Ctrl+S → 经 capture 路径真实写盘（mtime 更新） | C1: capture 监听 + context 栈匹配 + 命令 handler + 写盘全链路 |
| 编辑器 dirty→clean 保存 | 外部写盘→auto-reload→Ctrl+S 保存新内容→磁盘断言 |
| iframe 内 Ctrl+W → 转发关闭该 HTML 页签 | forwardGlobalShortcuts 真实转发+重放+closeTab 全链路 |
| 内联 &lt;script&gt; 与内联事件属性在预览中执行 | CSP 修复：真实 WebView2 强制 CSP 下经同源 contentDocument 验证内联脚本+onclick 执行 |
| should preserve terminal content after switching to another page and back | H6 终端跨页面存活（多 Dockview 实例 + CSS 显隐） |

E2E 架构：`@wdio/tauri-service` 1.1.0 embedded driver（`webview2-com` COM 直连 `ICoreWebView2`，零 msedgedriver 依赖）。

## 静态检查门禁

| 门 | 命令 | 说明 |
|----|------|------|
| TypeScript | `npx tsc --noEmit` | 全量类型检查 |
| ESLint | `npx eslint src/` | 前端代码规范 |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Rust 代码规范 |
