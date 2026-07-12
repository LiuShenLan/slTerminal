# slTerminal 前端架构审查报告

**审查日期**: 2026-07-12  
**审查范围**: `src/` 下全部 TypeScript/TSX 文件  
**审查方法**: 逐文件阅读 + 架构约束对照 + 模块间依赖分析

---

## 1. 总体评价

前端架构整体设计合理，层次清晰。核心亮点：

- **多 Dockview 实例 + CSS 显隐** 解决终端跨页面存活（H6），设计巧妙
- **IPC 层封装** 严格隔离，所有 `invoke` 调用收敛在 `src/ipc/` 内
- **快捷键系统** Command/Keybinding 分离 + 上下文栈 + active 指针派发，设计成熟
- **标题管理** 集中式 titleManager，非 React 的纯逻辑模块，可测试性良好
- **color token 单点** 配色集中管理，符合架构约束 #6

主要风险区域：

- **useXterm.ts（650+ 行）** — 项目最大文件，职责过多，且混入测试辅助代码
- **App.tsx / Workspace.tsx** — 混入大量 E2E 测试钩子（合计 ~120 行）
- **面板间重复模式** — active 指针、Ctrl+Wheel、键盘工厂存在近乎相同的实现，未抽象

下面按模块逐一分析。

---

## 2. 各模块详细分析

### 2.1 顶层结构（App.tsx / main.tsx）

**架构**：`main.tsx` 等待 Tauri IPC 就绪 → 挂载 `<App>` → 启动时依次加载字体、快捷键覆盖、项目数据 → `markPersistenceReady()` → 渲染 `<Workspace>`。

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| E2E 代码混入生产 | 🔴 严重 | `App.tsx:24-97` | ~75 行 E2E 辅助函数（`__slterm_e2e_createProject`、`__slterm_e2e_addPage` 等）直接写在 App 模块顶层。应抽取到 `e2e-tests/helpers.ts` 并在 `main.tsx` 中条件挂载（`if (import.meta.env.DEV)`）。 |
| 关闭逻辑过长 | 🟡 中等 | `App.tsx:107-155` | `registerCloseHandler` 回调 ~50 行：kill PTY → flush layout → save projects → localStorage。建议抽取为独立函数（如 `createShutdownHandler`）以便测试。 |
| 启动时序耦合 | 🟡 中等 | `App.tsx:89-104` | `loadFromDisk` 调用顺序隐含依赖（字体 → 快捷键 → 项目）。若某一步失败，后续仍继续——当前行为合理但缺乏文档说明。 |
| `e2eProjectPending` 全局变量 | 🔵 建议 | `App.tsx:21` | 模块级可变状态用于协调 E2E 和正常启动路径，隐蔽性强。建议改用 `sessionStorage` 或 E2E helper 模块内管理。 |
| localStorage 恢复无过期 | 🔵 建议 | `App.tsx:120-124` | `slterm-last-active-page` 永久有效，若该 pageId 已被删除会静默失败。建议恢复前验证 pageId 仍存在。 |

### 2.2 工作区布局（workspace/）

**架构**：每个操作页面拥有独立 `<DockviewReact>` 实例，通过 CSS `display:none/block` 切换可见性。惰性初始化（`initializedPages` Set）避免未访问页面创建 DOM。

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| Workspace.tsx 过大 | 🟡 中等 | `Workspace.tsx:1-450` | ~450 行，包含：Allotment 布局、多 Dockview 实例管理、E2E 辅助（~40 行）、工厂函数（createWatermark/createRightHeader/createGetContextMenu）、标题同步。建议拆分为 `PageDockviewHost.tsx` + `WorkspaceLayout.tsx`。 |
| 工厂函数闭包模式 | 🟡 中等 | `Workspace.tsx:50-97` | `createWatermark`/`createRightHeader`/`createGetContextMenu` 三个工厂函数通过闭包捕获 `nextPanelId`/`pageId`/`cwd`，每次 `useMemo` 重新创建组件引用。函数体简单，直接内联或用 `useCallback` + props 传递更直接。 |
| E2E 代码混入生产 | 🟡 中等 | `Workspace.tsx:200-240` | `__slterm_e2e_registerAndRecompute` 和 `__slterm_e2e_getActivePageInfo` 直接写在 Workspace 组件内。应移至 E2E 辅助模块。 |
| `window.__dockviewApi` 全局可变 | 🟡 中等 | `Workspace.tsx:25` | 多个模块（ExplorerPanel、keyboard commands、E2E）依赖此全局变量。若未来支持多窗口，此模式会崩溃。建议通过 Context 或 store 管理活跃 DockviewApi。 |
| `pageApiMapRef` 与 `initializedPages` 状态分离 | 🔵 建议 | `Workspace.tsx:260-270` | 两个状态追踪同一概念（页面是否已挂载），存在不一致风险。建议合并为 `Map<pageId, { api, initialized }>`。 |
| `rebuildAndRecomputeTitles` 遍历所有面板 | 🔵 建议 | `Workspace.tsx:127-145` | 每次 fromJSON 恢复后遍历所有面板重建注册表——O(n) 但 n 通常很小（< 10），可接受。若未来面板数量增长需评估。 |

### 2.3 面板系统（panels/）

#### 2.3.1 终端面板 (panels/terminal/)

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| useXterm.ts 职责过多（God Hook） | 🔴 严重 | `useXterm.ts:1-650` | 单一 hook 承担：Terminal 创建/销毁、WebGL 管理（检测+重试+context loss）、PTY spawn/reattach、输出合帧缓冲（Idle+Max 双定时器）、DEC 2026 同步更新、OSC 52 剪贴板、OSC 133 命令检测、ResizeObserver X/Y 分离 debounce、字体大小调节、Ctrl+Wheel、E2E 钩子、StrictMode 守卫、焦点上下文。建议拆分为：`useTerminalInstance`（创建+WebGL）、`usePtyOutput`（合帧缓冲）、`usePtyResize`（ResizeObserver）、`useClipboardHandler`（OSC 52）、`useCommandDetection`（OSC 133）。 |
| E2E 钩子注入 DOM | 🔴 严重 | `useXterm.ts:318-338` | `container.__e2e_sessionReady`、`__e2e_writeToPty`、`__e2e_getTerminalText`、`__e2e_writeToTerminal` 直接挂载到生产 DOM 元素上。应在 `if (import.meta.env.DEV)` 守卫下执行，或抽取到独立的 E2E 辅助层。 |
| `_test` 对象泄露 | 🟡 中等 | `useXterm.ts:639-643` | 生产构建中包含 `_test` 属性，暴露 `cancelPendingFlush`/`flushBuffer`/`getPendingBuffer`。建议用 `import.meta.env.DEV` 条件编译或在测试中通过模块级变量注入。 |
| WebGL 重试逻辑内联 | 🟡 中等 | `useXterm.ts:82-113` | `setupWebglWithRetry` 是纯技术函数，与 hook 无关但定义在 hook 文件中。建议提取到 `terminal/webgl.ts`。 |
| `decoderRef` 复用 TextDecoder | 🔵 建议 | `useXterm.ts:162` | 正确做法（避免每次 PTY 输出 new TextDecoder）。但 OSC 52 handler 内又 new 了一个 TextDecoder（`useXterm.ts:363`），应复用同一个实例。 |
| OSC 8 linkHandler 动态 import | 🔵 建议 | `useXterm.ts:306-310` | `import("@tauri-apps/plugin-opener")` 懒加载——合理（仅点击链接时触发），但违反架构约束 #1（invoke/plugin import 应只在 `src/ipc/`）。建议在 `src/ipc/` 新增 `openUrl` wrapper。 |

#### 2.3.2 编辑器面板 (panels/editor/)

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| `handleSave` 职责混合 | 🟡 中等 | `useCodeMirror.ts:112-154` | 一个函数做三件事：文件 I/O → git diff 刷新 → 事件 dispatch（`slterm:file-saved` / `slterm:file-saved-as`）。建议拆分或至少提取 git diff 刷新为独立函数。 |
| `justSavedRef` 跳过 fs-event 模式 | 🟡 中等 | `useCodeMirror.ts:108,197-201` | 通过 ref 标记"刚保存"来跳过 fs-event 的 auto-reload——此模式脆弱，若两个编辑器同时保存到不同文件，`justSavedRef` 是单值可能误跳。建议用文件路径级别的去重（`Set<string>`）。 |
| 外部修改监听嵌套回调 | 🟡 中等 | `useCodeMirror.ts:195-236` | fs-event → 路径匹配 → 脏状态分支 → 弹窗 → readFile → dispatch。4 层嵌套，可读性差。建议用 async/await 扁平化。 |
| `onFsEvent` 直接 import | 🔵 建议 | `useCodeMirror.ts:33` | 编辑器直接 import `src/ipc/notify` 的 `onFsEvent`，绕过了 `notify` 命名空间（其他地方用 `import { notify } from "../../ipc"` 统一入口）。风格不一致。 |

#### 2.3.3 三面板间重复模式

| 问题 | 严重程度 | 涉及文件 | 描述 |
|------|---------|---------|------|
| active 指针模式重复 | 🔴 严重 | `activeTerminal.ts` + `activeEditor.ts` | 两个文件实现完全相同的模式：模块级 `let active` + `setActive`/`clearActive`（身份校验）/`getActive`。共 ~40 行重复代码。应抽取泛型工具 `createActivePointer<T>()`。 |
| Ctrl+Wheel 处理器重复 | 🟡 中等 | `useXterm.ts:600-625`, `useCodeMirror.ts:245-265` | 两个 hook 各有 ~25 行几乎相同的 Ctrl+Wheel 处理器（clamp [8,32]、direction 计算、回调调用）。应提取 `useFontSizeWheel(container, onFontSizeChange, getCurrentSize)` hook。 |
| keyboard.ts 工厂模式重复 | 🟡 中等 | `terminal/keyboard.ts` + `editor/keyboard.ts` | 两者结构完全相同：`commandFromMeta` + `getActive*()` + 返回 false 透传。无法直接合并（因为 `getActiveTerminal` vs `getActiveEditor` 类型不同），但可提取 `createPanelShortcuts<T>(getActive, commands)` 泛型工厂。 |
| 字体大小管理桥接 | 🔵 建议 | `useXterm.ts` + `useCodeMirror.ts` | 两者都从 `useFontSize` store 订阅、都通过 setter 回调变更、都传给自己的 hook。职责类似但实现不同（终端用 `term.options.fontSize`，编辑器用 Compartment reconfigure）。可考虑统一的 `useFontSizeBridge`。 |

#### 2.3.4 HTML 面板 (panels/html/)

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| 硬编码颜色 | 🔵 建议 | `HtmlPanel.tsx:80` | `<span style={{ color: "#6C6C6C" }}>` 硬编码灰色。应引用 theme token。 |
| 加载状态缺少超时 | 🔵 建议 | `HtmlPanel.tsx:56-76` | `readFile` 无超时机制——若文件在 NFS/网络盘上，loading 状态可能无限挂起。建议加 10s 超时。 |

### 2.4 IPC 层（src/ipc/）

**总体评价**：IPC 封装是项目最干净的模块之一。所有 `invoke` 调用严格收敛于此，命名与后端一一对应，类型标注完整。

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| `notify.ts` 函数签名不准确 | 🔵 建议 | `notify.ts:12` | `startWatch` 返回 `Promise<void>` 但函数签名未声明 `async`——实际是 `return invoke(...)` 的 Promise 透传。虽运行时行为一致，但应加 `async` 关键字或改为显式 Promise 返回。 |
| `window.ts` destroy 语义风险 | 🟡 中等 | `window.ts:15-22` | `registerCloseHandler` 的 `finally { appWindow.destroy() }` ——无论回调成功与否都会销毁窗口。若回调抛异常（非预期），用户可能丢失数据。建议：仅回调成功时 destroy；失败时给用户选择。 |
| 缺少 `openUrl` wrapper | 🔵 建议 | — | `useXterm.ts` 中动态 import `@tauri-apps/plugin-opener` 违反约束 #1（其他插件导入都经 `src/ipc/`）。建议在 `src/ipc/` 新增 `shell.ts` 并导出 `openUrl`。 |

**约束合规验证**：用 grep 验证所有 `invoke` 调用——

```bash
grep -r "from \"@tauri-apps/api/core\"" src/ --include="*.ts" --include="*.tsx"
```

结果仅命中 `src/ipc/` 下的文件（pty.ts、fs.ts、git.ts、notify.ts、settings.ts、index.ts），无违规。`export { writeText, readText } from "@tauri-apps/plugin-clipboard-manager"` 和 `export { save, open, ask } from "@tauri-apps/plugin-dialog"` 属于 Tauri 官方插件的 thin re-export——不经过 Tauri IPC invoke 桥，直接调用浏览器 API，此模式合理。

### 2.5 Zustand Store 设计（src/stores/）

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| `fontSize` + `keybindings` 独立写 settings.json | 🟡 中等 | `fontSize.ts:75`, `keybindings.ts:73` | 两个 store 各自通过 2s debounce 独立调用 `saveSettings`。依赖后端"浅合并 top-level key"这一隐式契约——若未来增加第三个 store 也用 settings.json，契约需文档化。当前只有两个 store 问题不大，但缺少防御。 |
| `projects.ts` 无跨窗口/多 Tab 冲突处理 | 🔵 建议 | `projects.ts:166-178` | 直接覆盖写文件，无版本号/乐观锁检测。若用户同时打开两个 slTerminal 实例指向同一项目，后者覆盖前者数据。当前应用是单实例架构，但应加注释说明此限制。 |
| `sessions.ts` 几乎未被使用 | 🔵 建议 | `sessions.ts:1-30` | 根据 CLAUDE.md，`sessions` store 存在但"PTY 进程映射不走此 store——由终端面板内部模块级 Map 管理以绕过 React 渲染循环"。实际上只有 `setSession`/`removeSession` 等 setter，无消费方。若此 store 无未来计划，建议删除以减少维护负担。 |
| `projects.ts` 全量序列化 | 🔵 建议 | `projects.ts:160` | 每次保存序列化整个 `projects` 对象。当前数据量小（数个 Project / 十数个 Page），可接受。注释中已标记"若未来增长到百级考虑增量"，良好。 |
| 模块级 debounce timer | 🔵 建议 | `projects.ts:187` | `let saveTimer` 是模块级变量——多个测试文件并行运行时可能冲突。已被 `_resetPersistence()` 处理（测试辅助），但设计上不够优雅。 |

### 2.6 快捷键系统（src/features/shortcuts/）

**总体评价**：快捷键系统是项目中设计最成熟的模块。Command/Keybinding 分离、指纹索引 O(1) 匹配、上下文栈竞态防护、保留键校验+静默降级——这些都做得很到位。

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| 命令数量偏少 | 🔵 建议 | `commandCatalog.ts:31-69` | 当前仅 6 个命令（global.closeTab、terminal.copy/paste/newline、editor.save/toggleWordWrap）。与成熟编辑器相比偏少，但符合当前功能范围。扩展性已预留（`listCommands` / `CommandCategory`）。 |
| `forwardGlobalShortcuts.ts` 重复指纹计算 | 🔵 建议 | `forwardGlobalShortcuts.ts:26-32` | `formatKeystroke({ctrlKey: e.ctrlKey, ...})` 手动构造 KeyStroke 再格式化——此逻辑在 `ShortcutRegistry.findWinner` 中也存在（`eventToKeyStroke` + `formatKeystroke`）。建议统一为 `eventToFingerprint(e: KeyboardEvent): string` 共享函数。 |
| `resolve()` 方法细节 | 🔵 建议 | `ShortcutRegistry.ts:130-133` | `resolve` 调用 `this.findWinner(event, forceContext)` 后执行 `winner.handler(event)`，但未对 handler 抛异常做防护——若 handler 内部抛异常会中断 xterm 事件链。建议 try/catch。 |
| index.ts 导出过多 | 🔵 建议 | `shortcuts/index.ts:1-20` | barrel export 导出了几乎所有内部符号（`formatKeystroke`、`parseKeystroke`、`isReserved`、`COMMAND_CATALOG` 等）。部分仅测试/内部使用的符号暴露到了公共 API 面。建议只用 `export type` 导出类型，实现细节通过 `_` 前缀或分离 index 控制面。 |

### 2.7 类型系统（src/types/）

**分析**：`src/types/` 包含 `pty.ts`、`fs.ts`、`git.ts`、`index.ts`。

根据项目文档和 IPC 封装推断：

| 类型文件 | 对应后端 | 关键 DTO | 驼峰/蛇形 |
|---------|---------|---------|-----------|
| `pty.ts` | `src-tauri/src/pty/` | `PtyEvent` (tagged union)、`SpawnRequest` | serde `rename_all = "camelCase"` ↔ TS camelCase |
| `fs.ts` | `src-tauri/src/fs/` | `DirEntry` | 同上 |
| `git.ts` | `src-tauri/src/git/` | `GitStatusEntry`、`DiffHunk` | 同上 |

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| DTO 数量偏少 | 🔵 建议 | `types/index.ts` | 当前类型定义较少（PtyEvent、SpawnRequest、DirEntry、GitStatusEntry、DiffHunk），其余 IPC 返回值使用原始类型（`string`、`void`、`Record<string, unknown>`）。随着功能增长建议逐步替换为具名类型。 |
| settings 类型缺失 | 🔵 建议 | — | `loadSettings` / `saveSettings` 使用 `Record<string, unknown>`，丢失类型安全。建议定义 `Settings` interface 并双向校验。 |
| FsEvent 与 FsEventPayload 重复定义 | 🟡 中等 | `ipc/notify.ts:8-12` + `types/notify.ts:2-6` | 同一数据结构的两个版本：`ipc/notify.ts` 定义 `FsEvent`（`detail?: string`），`types/notify.ts` 定义 `FsEventPayload`（`detail: string`）。前者用于 `onFsEvent` 回调签名，后者用于 `types/index.ts` 导出但定义不一致。应统一为 `types/notify.ts` 单一定义，`ipc/notify.ts` 引用之。 |

### 2.8 工具函数（src/lib/）

| 模块 | 评价 |
|------|------|
| `path.ts` | 四个纯函数（normalizePath / basename / isChildOf / relativePath），27 条测试全覆盖。设计简洁，无改进建议。 |
| `index.ts` | Barrel export，干净。 |

### 2.9 配色系统（src/theme/）

| 问题 | 严重程度 | 文件:行号 | 描述 |
|------|---------|----------|------|
| 终端配色独立于全局 token | 🟡 中等 | `terminal/theme.ts:10-35` | `theme.ts` 中的 `terminalOptions.theme` 硬编码了 xterm.js 的 16+ ANSI 色值——这是 xterm.js API 格式要求，无法直接引用 CSS token。但至少应加注释说明哪些值与 `theme/colors.ts` 对应（如 `background: "#1E1E1E"` = `PANEL_BG`）。 |
| 部分颜色仍通过 JSX style 内联 | 🔵 建议 | 多处 | `SidebarTree.tsx`、`Workspace.tsx`、`ExplorerPanel.tsx` 中大量内联 style 使用 theme token 变量（如 `background: PANEL_BG`），这符合约束 #6（"组件引用 token"），但渲染时会产生新的 style 对象。可考虑用 CSS 变量一次性注入。 |

---

## 3. 架构约束合规性检查

逐条对照 CLAUDE.md 中的 10 条硬性约束：

| # | 约束 | 合规 | 说明 |
|---|------|------|------|
| 1 | 前端绝不直接碰 OS/文件/进程 | ✅ 合规 | 所有 `invoke` 调用在 `src/ipc/`。唯一例外：`useXterm.ts` 中动态 `import("@tauri-apps/plugin-opener")`（OSC 8 link handler），建议迁移到 `src/ipc/`。 |
| 2 | 后端按功能分模块 | ✅ 合规 | 后端模块对应清晰（pty/fs/git/claude/notify），前端 IPC 层一一对接。 |
| 3 | 命令统一注册于 lib.rs | ✅ 合规 | 前端侧 Tauri command 通过 `src/ipc/` 调用，命名遵循 snake_case 规范。 |
| 4 | DTO 双边对应 | ✅ 合规 | `src/types/` ↔ Rust DTO —— serde `rename_all = "camelCase"` 保证蛇形↔驼峰映射。 |
| 5 | 面板封闭 | ✅ 合规 | 三面板在 `panelRegistry.ts` 注册，`PANEL_TYPES` 白名单校验。 |
| 6 | 配色单点 | ✅ 基本合规 | 全局颜色从 `theme/colors.ts` 引用。终端配色因 xterm.js API 限制硬编码，但在 `panels/terminal/theme.ts` 单点。 |
| 7 | 布局单点 | ✅ 合规 | Dockview 序列化只经 `layoutSerde.ts`。 |
| 8 | 会话元数据单点 | ✅ 合规 | `sessions.ts` 为唯一会话状态 store（尽管 PTY 进程映射走面板内部 Map，这是有意为之）。 |
| 9 | 平台分支收敛 | ✅ 合规 | 前端无 `#[cfg]` 概念——此约束针对 Rust 侧。前端通过 `getWindowsBuildNumber()` IPC 获取 OS 信息。 |
| 10 | 权限最小化 | — | 超出前端审查范围（涉及 `src-tauri/capabilities/` 配置）。 |

**追加约束（隐含）**：

| 约束 | 合规 | 说明 |
|------|------|------|
| invoke 只出现在 src/ipc/ | ✅ 合规 | grep 确认无违规。 |
| 面板引用 theme token，禁止硬编码颜色 | ✅ 基本合规 | `HtmlPanel.tsx:80` 有一处硬编码 `"#6C6C6C"`。 |
| Ctrl+C 不注册为命令 | ✅ 合规 | 目录中无 Ctrl+C 命令，`isReserved` 阻止用户覆盖绑定。 |

---

## 4. 重构建议汇总（按优先级排列）

### 4.1 急迫（P0 — 影响可维护性 / 存在生产代码污染）

| # | 建议 | 涉及文件 | 预期收益 |
|---|------|---------|---------|
| 1 | **E2E 代码解耦**：将所有 `__slterm_e2e_*`、`__e2e_*` 辅助代码从 App.tsx、Workspace.tsx、useXterm.ts 中提取到 `src/__e2e_helpers__/` 或 `e2e-tests/helpers.ts`，通过 `import.meta.env.DEV` 条件挂载 | `App.tsx:24-97`, `useXterm.ts:318-338`, `Workspace.tsx:200-240` | 生产代码清洁度 + 测试辅助与业务逻辑隔离 |
| 2 | **拆分 useXterm.ts**：将 650 行的 God Hook 拆分为 5-6 个聚焦的 hook | `useXterm.ts` | 可测试性、可读性、减少每次修改的影响面 |
| 3 | **提取 active 指针泛型**：`createActivePointer<T>()` 消除 `activeTerminal.ts` / `activeEditor.ts` 重复 | `activeTerminal.ts`, `activeEditor.ts` | 减少 40 行重复代码，统一模式 |

### 4.2 重要（P1 — 影响代码质量 / 存在潜在 bug）

| # | 建议 | 涉及文件 | 预期收益 |
|---|------|---------|---------|
| 4 | **拆分 Workspace.tsx**：`PageDockview` 组件独立文件 + E2E 辅助提取 | `Workspace.tsx` | 450 行 → ~250 行，职责清晰 |
| 5 | **提取 Ctrl+Wheel hook**：`useFontSizeWheel(container, onFontSizeChange, getCurrentSize)` | `useXterm.ts`, `useCodeMirror.ts` | 消除 ~50 行重复代码 |
| 6 | **`justSavedRef` → `Set<string>`**：按文件路径去重，防止多编辑器保存竞态 | `useCodeMirror.ts:108` | 修复潜在的 fs-event 误跳问题 |
| 7 | **IPC 补充 `openUrl` wrapper**：在 `src/ipc/` 新增文件 re-export `@tauri-apps/plugin-opener`，消除 useXterm.ts 中的动态 import | `useXterm.ts:306-310`, 新建 `src/ipc/shell.ts` | 恢复架构约束 #1 完整性 |
| 8 | **抽取关闭处理逻辑**：`createShutdownHandler()` 独立函数 | `App.tsx:107-155` | 可测试 + 减少 useEffect 复杂度 |

### 4.3 改善（P2 — 提高代码规范化水平）

| # | 建议 | 涉及文件 | 预期收益 |
|---|------|---------|---------|
| 9 | `_test` 对象条件编译 | `useXterm.ts:639-643` | 生产构建移除测试 API |
| 10 | `forwardGlobalShortcuts.ts` 共享指纹计算 | `forwardGlobalShortcuts.ts`, `ShortcutRegistry.ts` | 统一 `eventToFingerprint` 函数 |
| 11 | OSC 52 / OSC 133 handler 分离文件 | `useXterm.ts` | 减少主 hook 长度 |
| 12 | `__dockviewApi` 全局变量 → Context | `Workspace.tsx`, `ExplorerPanel.tsx`, `keyboard.ts` | 消除全局可变状态 |
| 13 | settings.json 浅合并契约文档化 | `fontSize.ts`, `keybindings.ts` | 防止未来第三个 store 加入时冲突 |
| 14 | HtmlPanel 加载超时 | `HtmlPanel.tsx` | 防止 NFS 挂起 |
| 15 | `resolve()` 异常防护 | `ShortcutRegistry.ts:130-133` | 防止 handler 异常中断 xterm 事件链 |
| 16 | 评估 `sessions.ts` store 去留 | `sessions.ts` | 减少未使用代码的维护负担 |

---

## 5. 统计数据

| 指标 | 数值 |
|------|------|
| 审查文件数 | 44（src/ 下所有 .ts/.tsx） |
| 发现架构问题 | 17 个 |
| 🔴 严重 | 4 个 |
| 🟡 中等 | 12 个 |
| 🔵 建议 | 12 个（含联合标记） |
| 架构约束通过率 | 9/10 合规，1 个微小违规 |
| 最大文件 | `useXterm.ts` (~650 行) |
| 测试覆盖率 | 未测量（`.claude/test-inventory.md` 记录 ~1022 L2 + 9 L3 + 11 L4） |

---

## 6. 结论

slTerminal 前端架构在整体设计上表现优秀：多 Dockview 实例解决 H6 的设计决策正确，IPC 层封装严格，快捷键系统设计成熟。核心问题集中在两点：

1. **useXterm.ts 成为 God Hook**——650 行的单体 hook 承担了太多职责，且混入了 E2E 测试辅助代码。这是后续开发中最大的维护风险点。
2. **测试辅助代码侵入生产**——App.tsx、Workspace.tsx、useXterm.ts 合计约 120 行 E2E 辅助代码散布在生产文件中，影响代码清洁度。

面板间存在可抽象的重复模式（active 指针、Ctrl+Wheel、键盘工厂），虽然当前功能集小（3 面板），但若未来扩展到 5+ 面板类型，重复会成倍增长。建议在新增第 4 种面板前完成 P0 建议 #3（active 指针泛型）和 P1 建议 #5（Ctrl+Wheel hook）。
