# Review: 文档与代码一致性

## 审计说明

本次 review 覆盖项目内所有 `CLAUDE.md`、`.claude/test-inventory.md` 以及对应源码，重点核对文件清单、函数签名/行为、架构机制、测试用例数、命令脚本和需求编号索引。以下仅列出发现的问题。

---

## [严重级别:高] 根 CLAUDE.md 硬性约束 #10 / pty/CLAUDE.md 修改注意事项与实际权限模型不符

- **位置**：根 `.claude/CLAUDE.md:46`（约束 #10）、`src-tauri/src/pty/CLAUDE.md:175`（修改注意事项 #1） vs `src-tauri/capabilities/default.json:7-13`、`src-tauri/src/lib.rs:23-29`
- **问题**：文档要求“新增 Tauri 命令后必须在 `capabilities/` 显式放行，不用通配 `*`”，但 `capabilities/default.json` 实际使用 `core:default`、`opener:default`、`dialog:default` 等通配权限；`lib.rs` 更明确说明 Tauri 2 自定义命令由 `invoke_handler` 自动对所有窗口放行，无需逐条声明，当前采用 Tauri 2 默认行为。PTY 的 5 条命令（`pty_spawn`/`pty_write`/`pty_resize`/`pty_kill`/`pty_reattach`）均未在 capabilities 中显式列出。
- **建议**：要么收紧 capabilities 为逐命令白名单并删除 lib.rs 的默认放行说明，要么更新根文档约束 #10 与 pty 修改注意事项以匹配实际权限模型。

## [严重级别:高] HTML 面板全局键转发机制文档多处过时

- **位置**：`src/features/shortcuts/CLAUDE.md:179`、`src/features/shortcuts/forwardGlobalShortcuts.ts:1-9`、`e2e-tests/CLAUDE.md:80`
- **代码**：`src/panels/html/HtmlPanel.tsx:32-34`、`src/panels/html/HtmlPanel.tsx:45-61`、`src/panels/html/HtmlPanel.tsx:88-112`
- **问题**：多份文档声称 HTML 面板使用 `<iframe sandbox="allow-scripts allow-same-origin">` 并通过 `attachGlobalShortcutForwarder(iframe.contentDocument)` 转发按键。实际生产代码使用 `sandbox="allow-scripts"`（不含 `allow-same-origin`），键盘转发走“注入脚本 `window.parent.postMessage` + 父窗口 `message` 监听”路径；`attachGlobalShortcutForwarder` 未在生产代码调用，仅在 `src/__tests__/forward-global-shortcuts.test.ts` 中使用。
- **建议**：更新 `src/features/shortcuts/CLAUDE.md`、`forwardGlobalShortcuts.ts` 文件头注释、`e2e-tests/CLAUDE.md` 以描述当前 postMessage 注入脚本机制，并明确 `attachGlobalShortcutForwarder` 为旧路径/测试保留。

## [严重级别:高] test-inventory.md 对 L3 测试用例数严重失实

- **位置**：`.claude/test-inventory.md:3-5`、`165-167`
- **代码**：`test/terminal/*.test.ts` 共 4 文件、`vitest.l3.config.ts:6`
- **问题**：文档称“全量 ~1234 用例（Rust 193 + 前端 1020 + L3 9 + E2E 12）”，又称“`npm test` 实际运行 67 文件 1016 用例（含 `test/terminal/` 下 2 文件 9 用例）”。实际 L3 有 4 文件 116 条用例（terminal-serialize 41、ansi-correctness 30、keyboard 36、osc 9），且 `vitest.config.ts` 仅包含 `src/__tests__/`；`test/terminal/` 由独立命令 `npm run test:l3` 运行，并不被 `npm test` 包含。
- **建议**：更新 test-inventory.md 的 L3 章节、总数以及“npm test 含 L3”的描述。

## [严重级别:中] pty/CLAUDE.md 对 JobHandle 跨平台设计与实际代码不一致

- **位置**：`src-tauri/src/pty/CLAUDE.md:32`（JobHandle 跨平台设计） vs `src-tauri/src/state.rs:31-33`、`src-tauri/src/pty/spawn.rs:747-750`
- **问题**：文档称“非 Windows 平台 `JobHandle` 为零大小占位类型，`state.rs` 无需条件编译”。实际 `PtySession` 结构体中 `job_object` 字段无条件声明为 `Option<crate::pty::spawn::JobHandle>`，而在 `pty_spawn` 中该字段用 `#[cfg(windows)] job_object: job_handle,` 初始化。非 Windows 平台下该字段缺失初始化值，会导致 `E0063 missing field job_object` 编译错误。
- **建议**：要么在 `PtySession` 结构体定义与初始化处统一使用 `#[cfg(windows)]`（并在非 Windows 下提供默认初始化），要么如文档所述让 `JobHandle` 在非 Windows 下为零大小占位并始终初始化。

## [严重级别:高] 根 CLAUDE.md 规划目录结构包含不存在的后端模块

- **位置**：根 `.claude/CLAUDE.md:33`
- **代码**：`src-tauri/src/` 实际目录结构
- **问题**：根文档在规划目录结构中列出后端功能模块 `claude/`，但 `src-tauri/src/` 下实际不存在 `claude/` 目录或文件（现有模块为 `pty/`、`fs/`、`git/`、`notify/`、`settings/`）。
- **建议**：从根目录结构规划中移除 `claude/`，或补充说明其尚未实现/已移除。

## [严重级别:中] workspace/CLAUDE.md 对 Workspace.tsx 职责描述失实

- **位置**：`src/workspace/CLAUDE.md:28`
- **代码**：`src/workspace/Workspace.tsx:1-185`、`src/App.tsx:3,176`
- **问题**：文档称 `Workspace.tsx`“阶段1清理后已废弃，实际渲染委托给 PageDockviewHost”。实际上 `Workspace.tsx` 仍是 `App.tsx` 直接渲染的活跃主组件，负责 Allotment 三栏布局、页面生命周期（`ensurePageInitialized`/`switchToPage`/`onDeletePage`）、`window.__dockviewApi` 映射与 E2E 就绪标记；仅将单个操作页面的 Dockview 渲染委托给 `PageDockviewHost`。
- **建议**：修正为“Workspace.tsx 是主组件，负责三栏布局与页面生命周期，单页面 Dockview 渲染委托给 PageDockviewHost”。

## [严重级别:中] stores/CLAUDE.md 新增 Store 规则与现有实现不一致

- **位置**：`src/stores/CLAUDE.md:71-76`
- **代码**：`src/stores/fontSize.ts:4`、`src/stores/keybindings.ts:10`
- **问题**：文档规则第 3 条要求“如需持久化，委托给 `src/ipc/fs`”，但 `fontSize` 与 `keybindings` store 实际通过 `src/ipc/settings` 的 `loadSettings`/`saveSettings` 持久化。
- **建议**：将规则改为“持久化委托给 `src/ipc/settings`（fontSize/keybindings）或 `src/ipc/fs`（projects）”，或统一说明“持久化 IPC 层”而不限定具体文件。

## [严重级别:中] src/ipc/CLAUDE.md 未记录 index.ts 的 ping() 命令

- **位置**：`src/ipc/CLAUDE.md:24`
- **代码**：`src/ipc/index.ts:12-15`
- **问题**：文档称 `index.ts` 只是 barrel export，统一对外暴露各子模块。实际 `index.ts` 还导出了 `ping()` 函数，直接调用 `invoke("ping")`。
- **建议**：在模块映射表中补充 `index.ts` 的 `ping()` 职责说明。

## [严重级别:中] useXterm 实际组合的字体 hook 与文档不符

- **位置**：`src/panels/CLAUDE.md:160-162`、`src/panels/CLAUDE.md:206`
- **代码**：`src/panels/terminal/useXterm.ts:35`、`src/panels/terminal/useXterm.ts:116`
- **问题**：文档称 useXterm 组合 7 个子 hook 之一为 `useFontSizeBridge.ts`，但实际调用的是 `src/lib/useFontSizeWheel.ts`；`src/panels/terminal/useFontSizeBridge.ts` 存在但未被 `useXterm.ts` 导入或使用，也未从 `terminal/index.ts` 导出。
- **建议**：将文档中的 `useFontSizeBridge.ts` 改为 `src/lib/useFontSizeWheel.ts`，或删除/重命名未使用的 `useFontSizeBridge.ts`。

## [严重级别:中] HTML 注入脚本结构与行为描述与实际不符

- **位置**：`src/panels/CLAUDE.md:50-54`
- **代码**：`src/panels/html/HtmlPanel.tsx:45-61`
- **问题**：文档称注入脚本包含“两部分”（键盘转发、片段链接拦截），且片段拦截执行 `preventDefault + scrollIntoView + try { location.hash = ... }`。实际注入脚本包含三部分：CSS 样式注入（`.slterm-target`）、键盘转发、片段链接拦截；片段拦截代码中只有 `preventDefault`、`scrollIntoView`、`classList.add/remove`、`dataset.sltermHash`，无任何 `location.hash` 赋值。
- **建议**：更新文档以匹配实际三部分结构，并删除或修正 `location.hash` 相关描述。

## [严重级别:中] layoutSerde.ts 注释与“不自动创建默认终端”策略矛盾

- **位置**：`src/workspace/layoutSerde.ts:70-74`
- **代码**：`src/workspace/PageDockviewHost.tsx:267-274`
- **问题**：`loadLayout` 函数注释称“调用方应在失败时回退到默认布局（创建默认终端）”，但 `workspace/CLAUDE.md` 与 `PageDockviewHost.tsx` 已明确“不自动创建默认终端”，空白页面由 Watermark 组件接管显示。
- **建议**：删除或更新 `layoutSerde.ts` 中关于“创建默认终端”的过时注释。

## [严重级别:中] 根 CLAUDE.md L1/L2 用例数估算失实

- **位置**：根 `.claude/CLAUDE.md:71-74`
- **代码**：实际 `cargo test` 190 条、`npm test` 1020 条
- **问题**：文档称 L1 ~196、L2 ~1095。实际 L1 为 190（git 61、pty reader 28、pty spawn 13、pty shell 7、notify 24、notify pool 12、fs 16、state 15、settings 8、error 4、lib 2、integration 5），L2 为 1020。
- **建议**：更新为实际数字，或改为“约”并定期同步；推荐以 `.claude/test-inventory.md` 作为单一真值源。

## [严重级别:低] 多处测试用例数统计与代码实际不符

- **位置与差异**：
  - `src/panels/CLAUDE.md:195`：useXterm 测试声称 37+74=111，实际 `use-xterm-output.test.ts` 37 + `use-xterm-lifecycle.test.ts` 71
  - `src/panels/CLAUDE.md:239`：`workspace-defaulttab.test.tsx` 声称 13，实际 14
  - `src/panels/CLAUDE.md:245`：`keyboard.test.ts` 声称 13，实际 12
  - `src/panels/CLAUDE.md:246`：`editor-keyboard.test.ts` 声称 5，实际 7
  - `src/panels/CLAUDE.md:247`：`activeEditor.test.ts` 声称 4，实际 5
  - `src/panels/CLAUDE.md:249`：`shortcuts.test.ts` 声称 41，实际 44
  - `src/panels/CLAUDE.md:250`：`globalCommands.test.ts` 声称 12，实际 13
  - `src/panels/CLAUDE.md:265`：`html-panel.test.tsx` 声称 18，实际 36
  - `src/panels/CLAUDE.md:278`：`ansi-correctness.test.ts` 声称 29，实际 30
  - `src/panels/CLAUDE.md:280`：L3 `keyboard.test.ts` 声称 37，实际 36
  - `src/features/shortcuts/CLAUDE.md:171-183`：`shortcuts.test.ts` 声称 41（实际 44）、`globalCommands.test.ts` 声称 12（实际 13）、`forward-global-shortcuts.test.ts` 声称 6（实际 28）、`keyboard.test.ts` 声称 13（实际 12）、`editor-keyboard.test.ts` 声称 5（实际 7）
  - `src/__tests__/CLAUDE.md:9`：`html-panel.test.tsx` 声称 39，实际 36
  - `src/__tests__/CLAUDE.md:55`：`csp-config.test.ts` 声称 6，实际 4
- **建议**：统一用 `grep -E '^\s*it\s*\(|^\s*test\s*\('` 统计各测试文件并更新文档，或引用 `.claude/test-inventory.md` 作为唯一统计源。

## [严重级别:低] useXterm.ts 行数描述略有偏差

- **位置**：`src/panels/CLAUDE.md:162`
- **代码**：`src/panels/terminal/useXterm.ts` 共 422 行
- **问题**：文档称 useXterm.ts 是“~410行”，实际 422 行。
- **建议**：改为“约 420 行”或直接删除行数描述。

---

## 实际 review 过的文档与代码文件清单

### 文档
- 根 `.claude/CLAUDE.md`
- `.claude/test-inventory.md`
- `src-tauri/src/pty/CLAUDE.md`
- `src-tauri/src/notify/CLAUDE.md`
- `src-tauri/src/git/CLAUDE.md`
- `src-tauri/src/fs/CLAUDE.md`
- `src/panels/CLAUDE.md`
- `src/workspace/CLAUDE.md`
- `src/stores/CLAUDE.md`
- `src/features/shortcuts/CLAUDE.md`
- `src/features/explorer/CLAUDE.md`
- `src/features/sidebar/CLAUDE.md`
- `src/features/fileViewers/CLAUDE.md`
- `src/ipc/CLAUDE.md`
- `src/lib/CLAUDE.md`
- `src/__tests__/CLAUDE.md`
- `e2e-tests/CLAUDE.md`

### 代码（关键文件）
- `src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json`
- `src-tauri/src/pty/spawn.rs`、`reader.rs`、`shell.rs`、`state.rs`
- `src-tauri/src/notify/mod.rs`、`pool.rs`
- `src-tauri/src/git/mod.rs`、`fs/mod.rs`
- `src/panels/terminal/useXterm.ts`、`usePtyOutput.ts`、`usePtyResize.ts`、`useClipboardHandler.ts`、`useCommandDetection.ts`、`keyboard.ts`、`theme.ts`、`useTerminalInstance.ts`
- `src/panels/editor/useCodeMirror.ts`、`keyboard.ts`、`EditorPanel.tsx`
- `src/panels/html/HtmlPanel.tsx`
- `src/workspace/Workspace.tsx`、`PageDockviewHost.tsx`、`layoutSerde.ts`、`titleManager.ts`
- `src/stores/projects.ts`、`fontSize.ts`、`keybindings.ts`
- `src/features/shortcuts/ShortcutRegistry.ts`、`commandCatalog.ts`、`reserved.ts`、`forwardGlobalShortcuts.ts`
- `src/features/explorer/useFileTree.ts`
- `src/features/fileViewers/FileViewerRegistry.ts`
- `src/ipc/index.ts`
- `src/lib/e2eEnabled.ts`、`injectScript.ts`
- `src/panelRegistry.ts`
- `package.json`、`.github/workflows/ci.yml`、`.claude/package.ps1`
- `src/__tests__/*.test.ts(x)`、`test/terminal/*.test.ts`（用例数统计）
- `e2e-tests/test.e2e.ts`、`e2e-tests/helpers.ts`
