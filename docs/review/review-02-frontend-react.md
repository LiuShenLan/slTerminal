# Review: 前端 TS/React 代码质量

## [严重级别:高] useCodeMirror 文件加载存在过期写入竞态

**位置**

- `src/panels/editor/useCodeMirror.ts:232-289`

**问题**

`initEditor` 是 fire-and-forget 异步函数，仅靠组件级单一 `mountedRef` 做 unmount 防护。当 `filePath` 快速切换时：

1. 旧 effect 的 cleanup 把 `mountedRef.current = false`；
2. 新 effect 的 `initEditor` 立刻把 `mountedRef.current = true`；
3. 旧 `initEditor` 中 `await fs.readFile(filePath)` 完成后检查 `mountedRef.current`，此时为 `true`，于是继续创建 `EditorView` 并写入 `viewRef.current`；
4. 新 `initEditor` 随后完成，其创建的 `EditorView` 被旧实例覆盖，导致用户看到旧文件内容，且旧 view 的 cleanup 永远不会执行（DOM 泄漏、事件监听器泄漏）。

**建议**

引入 generation/abort controller：在 `useEffect` 入口生成本次 effect 的 gen，异步回调完成后比对 `genRef.current !== gen` 则直接返回；或直接给 `fs.readFile` 挂载 `AbortController`，cleanup 中 abort。

## [严重级别:高] HtmlPanel 注入脚本未对宿主 HTML 做转义，存在注入/执行异常风险

**位置**

- `src/panels/html/HtmlPanel.tsx:45-61`

**问题**

`INJECTED_SCRIPT` 被直接拼接到用户 HTML 字符串中。若用户 HTML 在任意位置出现 `</script>`，会提前闭合注入的 `<script>` 标签，导致后续脚本代码被当作 HTML 解析，轻则键盘转发/片段拦截失效，重则可能执行非预期脚本（虽然 iframe 已 sandbox，但仍会破坏功能）。当前 `injectScript` 仅按 marker 做幂等，未处理宿主 HTML 中的 `</script>`。

**建议**

注入前对用户 HTML 做转义（将 `</script>` 拆成 `<\/script>` 或 `</scr" + "ipt>`），或把脚本放在单独 blob/data URL 中引入。

## [严重级别:中] Workspace 在 render 阶段直接修改 ref

**位置**

- `src/workspace/Workspace.tsx:168-182`

**问题**

组件函数体执行期间直接遍历并增删 `pageCallbacksRef.current`：

```ts
for (const key of pageCallbacksRef.current.keys()) {
  if (!activePageIds.has(key)) {
    pageCallbacksRef.current.delete(key);
  }
}
for (const page of allPages) {
  if (!pageCallbacksRef.current.has(page.pageId)) {
    pageCallbacksRef.current.set(page.pageId, { ... });
  }
}
```

React 不保证 render 阶段副作用可安全执行（严格模式/并发特性下可能重复调用），直接在 render 中写入 ref 属于反模式，可能产生 double-registration 或读取到半一致状态。

**建议**

将回调 map 的维护逻辑移到 `useEffect`/`useLayoutEffect` 中，或使用 `useMemo` 返回稳定回调并保证 cleanup。

## [严重级别:中] PageDockviewHost handleReady 中 Dockview API 监听器未显式释放

**位置**

- `src/workspace/PageDockviewHost.tsx:262-304`

**问题**

`handleReady` 内注册了 `api.onDidLayoutFromJSON`、`api.onDidLayoutChange`、`api.onDidRemovePanel` 三个监听器，但未保存返回的 disposable，也未在组件卸载或 `handleReady` 重新触发时清理。虽然 Dockview 的 `api.dispose()` 理论上会清理，但若 `handleReady` 因 StrictMode 等原因被触发两次，或组件卸载路径未走到 Dockview 的 dispose，监听器会泄漏。

**建议**

将 disposable 收集到 ref 数组中，在 `useEffect` cleanup 或组件卸载时统一 `dispose()`。

## [严重级别:中] main.tsx bootstrap 的 setInterval 无清理也无最大轮次

**位置**

- `src/main.tsx:8-19`

**问题**

等待 `window.__TAURI_INTERNALS__` 时使用 `setInterval(..., 50)`，没有任何最大重试次数，也没有在组件/应用卸载时清理。若 Tauri 注入失败，interval 会永久运行。

**建议**

增加最大轮次（如 200 次/10 秒）并在超时后给出错误提示；同时在 React root 创建失败或应用关闭路径中清理 interval。

## [严重级别:中] useFileTree refreshExpanded 的 gitStatus 结果未做 generation 防护

**位置**

- `src/features/explorer/useFileTree.ts:166-184`

**问题**

`refreshExpanded` 中 `reloadPreservingExpanded()` 读取 `rootPathRef.current`，能拿到最新路径；但随后 `gitStatus(rp)` 的 `setGitStatusMap(map)` 未检查 generation。在 `rootPath` 快速切换时，旧 rootPath 的 `gitStatus` 可能晚于新 rootPath 的 `gitStatus` resolve，导致 gitStatusMap 被旧数据覆盖。

**建议**

在 `refreshExpanded` 内读取 `genRef.current` 快照，回调中比对，不匹配则丢弃。

## [严重级别:中] ExplorerPanel 新增面板未处理 addPanel 异常

**位置**

- `src/features/explorer/ExplorerPanel.tsx:100-110`

**问题**

`dockApi.addPanel(...)` 调用后没有 `try-catch`，也没有检查返回值。若 Dockview 抛出异常（如重复 panelId 或组件未注册），异常会向上传播，且 `titleManager.registerEditor` 仍会在 addPanel 失败后执行，造成标题管理器与 Dockview 状态不一致。

**建议**

`try { dockApi.addPanel(...) } catch { return }`，只在 addPanel 成功后注册 titleManager。

## [严重级别:中] usePtyOutput setupRetry 的 onData 监听未在组件卸载前显式清理

**位置**

- `src/panels/terminal/usePtyOutput.ts:156-178`

**问题**

`setupRetry` 注册 `term.onData(...)` 并把 disposable 存入 `retryDisposableRef.current`，但 `useXterm` cleanup 中只把 `doSpawnRef.current = null`，没有 dispose retryDisposable。虽然 Terminal dispose 会一并清理 onData，但在 spawn 失败到面板卸载之间若发生多次 retry 注册/切换，旧的 disposable 会被覆盖但未被 dispose。

**建议**

在 `useXterm` cleanup 中调用 `retryDisposableRef.current?.dispose()`，或在 `setupRetry` 开始处 dispose 旧监听（已做）的基础上确保 cleanup 路径也 dispose 一次。

## [严重级别:中] TerminalPanel 使用非空断言访问可能缺失的 icon

**位置**

- `src/panels/terminal/TerminalPanel.tsx:72`

**问题**

```ts
api.updateParameters({ ...params, tabIcon: state.icon! });
```

`TabState.icon` 类型为 `string | null | undefined`，虽然 `state.active === true` 时通常由 `tabTitleRegistry.match` 提供，但类型系统并未保证，非空断言隐藏了运行时传入 `undefined` 的风险。

**建议**

去掉 `!`，使用 `state.icon ?? null` 或显式判断。

## [严重级别:中] layoutSerde 使用 any 绕过 Dockview 类型

**位置**

- `src/workspace/layoutSerde.ts:97-98`

**问题**

```ts
api.fromJSON(layout as any, { reuseExistingPanels: true });
```

`layout` 经过白名单过滤和旧格式修补后，仍用 `as any` 传入 `fromJSON`。虽然 Dockview 的 `fromJSON` 类型可能较严格，但此处没有提供最小类型封装，任何未来字段变更都不会被 TypeScript 捕获。

**建议**

定义与 Dockview `SerializedDockview` 兼容的接口，或至少使用 `as Parameters<DockviewApi["fromJSON"]>[0]` 而非 `any`。

## [严重级别:低] panelRegistry.isAlwaysRenderPanel 逻辑冗余

**位置**

- `src/panelRegistry.ts:59-61`

**问题**

```ts
return type === PANEL_TERMINAL || (type !== PANEL_EDITOR && isValidPanelType(type));
```

`type === PANEL_TERMINAL` 已被第二个条件覆盖（terminal 不是 editor 且是 valid），第一个分支冗余，增加阅读成本。

**建议**

简化为 `return type !== PANEL_EDITOR && isValidPanelType(type);`。

## [严重级别:低] useFontSizeBridge 为未使用的重复死代码

**位置**

- `src/panels/terminal/useFontSizeBridge.ts`

**问题**

该 hook 与 `src/lib/useFontSizeWheel.ts` 逻辑几乎完全相同，且未被任何生产代码 import（仅在 `useXterm.ts` 注释中被提及）。保留两份实现会导致未来修改时遗漏同步。

**建议**

删除 `useFontSizeBridge.ts`，统一使用 `useFontSizeWheel`；或保留其一并删除另一个。

## [严重级别:低] forwardGlobalShortcuts 为未使用的死代码

**位置**

- `src/features/shortcuts/forwardGlobalShortcuts.ts`

**问题**

该函数用于 `allow-same-origin` iframe 的键盘桥，但实际 `HtmlPanel` 使用 `sandbox="allow-scripts"`（不含 `allow-same-origin`）+ `postMessage` 方案转发键盘事件。`forwardGlobalShortcuts` 虽被 `features/shortcuts/index.ts` re-export，但没有生产代码调用。

**建议**

若 postMessage 方案为最终方案，删除 `forwardGlobalShortcuts.ts` 及其 re-export；若保留作为备选，应在代码或文档中明确其使用场景。

## [严重级别:低] useTerminalInstance 中 document.fonts.ready Promise 未取消

**位置**

- `src/useTerminalInstance.ts:123-135`

**问题**

`document.fonts.ready.then(...)` 没有取消机制。组件卸载后，promise resolve 仍会执行 `requestAnimationFrame`，虽然内部检查了 `isDisposedRef.current`，但若 `isDisposedRef` 被重置（重新挂载同一 hook 的新实例），可能误执行到旧分支。

**建议**

使用 AbortController 或 `useEffect` cleanup 中设置取消标志，并在 rAF 回调中双重检查取消状态与容器是否仍属于当前组件。

## [严重级别:低] useTerminalInstance dispose 与 useEffect cleanup 代码重复

**位置**

- `src/panels/terminal/useTerminalInstance.ts:159-180` 与 `208-224`

**问题**

两套几乎相同的清理逻辑分别位于 effect cleanup 和 `dispose` 回调中。未来修改（如新增 addon 或清理 E2E helper）容易只改一处。

**建议**

提取为内部 `performDispose()` 函数，由 cleanup 和 `dispose` 回调共用。

## [严重级别:低] ipc/notify.ts 的 FsEvent 与 types/notify.ts 类型不一致

**位置**

- `src/ipc/notify.ts:6-10`
- `src/types/notify.ts:2-6`

**问题**

`ipc/notify.ts` 中 `FsEvent.detail` 为可选（`detail?: string`），而 `types/notify.ts` 中 `FsEventPayload.detail` 为必填（`detail: string`）。类型不一致可能导致消费方假设 detail 一定存在而运行时得到 `undefined`。

**建议**

统一 `FsEventPayload` 与 `FsEvent` 的定义，推荐 `detail?: string` 并在消费处做好空值处理。

## [严重级别:低] TerminalRegistry.getAll 返回可写 Map

**位置**

- `src/panels/terminal/TerminalRegistry.ts:39-41`

**问题**

```ts
getAll(): ReadonlyMap<string, RegisteredTerminal> {
  return registry;
}
```

返回类型声称为 `ReadonlyMap`，但实际返回的是普通 `Map`，调用方仍可强制转换后修改内部状态。

**建议**

返回 `new Map(registry)` 副本，或明确暴露只读迭代接口。

## [严重级别:低] main.tsx 与 ErrorBoundary 使用 any 访问 window 扩展属性

**位置**

- `src/main.tsx:10,14`
- `src/lib/ErrorBoundary.tsx:42`

**问题**

使用 `(window as any).__TAURI_INTERNALS__` 和 `(window as any).__sltermError` 绕过类型检查。虽然功能上可行，但丢失了类型安全，也缺少全局声明扩展。

**建议**

在 `src/global.d.ts` 中声明 `interface Window { __TAURI_INTERNALS__: ...; __sltermError?: ...; }`，去掉 `as any`。

## [严重级别:低] useCodeMirror handleSave 对根路径文件的 git 仓库路径计算有误

**位置**

- `src/panels/editor/useCodeMirror.ts:166-171`

**问题**

```ts
const repoDir = normalizedPath.lastIndexOf("/") >= 0
  ? normalizedPath.slice(0, normalizedPath.lastIndexOf("/"))
  : ".";
```

对 `D:/file.txt` 会得到 `D:`，这不是有效的 git 仓库路径；对 `C:/` 会得到空字符串。`gitDiff` 会因此失败并被 catch，但错误被静默吞掉。

**建议**

使用路径工具（如 `dirname`）处理边界，或在无父目录时跳过 `gitDiff`。

## [严重级别:低] SidebarTree 内联定义子组件导致每次渲染重建

**位置**

- `src/features/sidebar/SidebarTree.tsx:115-142,147-183,185-294`

**问题**

`Toolbar`、`ProjectRow`、`PageRow` 在 `SidebarTree` 函数体内直接定义，每次渲染都会创建新的组件类型，React 无法复用组件实例，子树会完全 unmount/remount，丢失焦点和局部状态（如 PageRow 的内联编辑框可能意外失焦）。

**建议**

将这些子组件提取到模块顶层定义。

## [严重级别:低] FileTree 内联输入框 paddingLeft 硬编码魔法数字

**位置**

- `src/features/explorer/FileTree.tsx:388-465,525-599`

**问题**

多处 `paddingLeft: 8 + (depth + 1) * 16 + 12 + 4 + 14` 等硬编码计算重复出现，缺乏命名常量，维护困难且容易在调整缩进/图标大小时出错。

**建议**

提取 `ICON_WIDTH`、`ARROW_WIDTH`、`INDENT` 等常量并统一计算。

## [严重级别:低] PageDockviewHost rebuildAndRecomputeTitles 访问非公共 API

**位置**

- `src/workspace/PageDockviewHost.tsx:165`

**问题**

```ts
const component = panel.view?.contentComponent;
```

`panel.view` 可能不是 Dockview 公共类型的一部分，未来版本升级时可能断裂。

**建议**

通过 `panel.api.params` 或面板自身注册时携带的类型信息判断，而非访问内部 `view` 属性。

## Review 覆盖范围

实际 review 过的文件清单：

- `src/App.tsx`
- `src/main.tsx`
- `src/panelRegistry.ts`
- `src/ipc/pty.ts`
- `src/ipc/fs.ts`
- `src/ipc/git.ts`
- `src/ipc/notify.ts`
- `src/ipc/settings.ts`
- `src/ipc/window.ts`
- `src/ipc/index.ts`
- `src/ipc/clipboard.ts`
- `src/ipc/dialog.ts`
- `src/ipc/shell.ts`
- `src/stores/projects.ts`
- `src/stores/fontSize.ts`
- `src/stores/keybindings.ts`
- `src/stores/layout.ts`
- `src/workspace/Workspace.tsx`
- `src/workspace/PageDockviewHost.tsx`
- `src/workspace/layoutSerde.ts`
- `src/workspace/titleManager.ts`
- `src/workspace/index.ts`
- `src/panels/terminal/useXterm.ts`
- `src/panels/terminal/useTerminalInstance.ts`
- `src/panels/terminal/usePtyOutput.ts`
- `src/panels/terminal/usePtyResize.ts`
- `src/panels/terminal/TerminalPanel.tsx`
- `src/panels/terminal/webgl.ts`
- `src/panels/terminal/useClipboardHandler.ts`
- `src/panels/terminal/useCommandDetection.ts`
- `src/panels/terminal/useFontSizeBridge.ts`
- `src/panels/terminal/TerminalRegistry.ts`
- `src/panels/terminal/TabTitleRegistry.ts`
- `src/panels/terminal/keyboard.ts`
- `src/panels/terminal/activeTerminal.ts`
- `src/panels/terminal/theme.ts`
- `src/panels/editor/useCodeMirror.ts`
- `src/panels/editor/EditorPanel.tsx`
- `src/panels/editor/gitGutter.ts`
- `src/panels/editor/activeEditor.ts`
- `src/panels/editor/keyboard.ts`
- `src/panels/html/HtmlPanel.tsx`
- `src/features/explorer/ExplorerPanel.tsx`
- `src/features/explorer/useFileTree.ts`
- `src/features/explorer/FileTree.tsx`
- `src/features/fileViewers/FileViewerRegistry.ts`
- `src/features/sidebar/SidebarTree.tsx`
- `src/features/shortcuts/ShortcutRegistry.ts`
- `src/features/shortcuts/usePanelFocus.ts`
- `src/features/shortcuts/forwardGlobalShortcuts.ts`
- `src/features/shortcuts/wireKeybindings.ts`
- `src/features/shortcuts/keystroke.ts`
- `src/features/shortcuts/reserved.ts`
- `src/features/shortcuts/commandCatalog.ts`
- `src/features/shortcuts/globalCommands.ts`
- `src/features/shortcuts/types.ts`
- `src/features/shortcuts/index.ts`
- `src/lib/useFontSizeWheel.ts`
- `src/lib/activePointer.ts`
- `src/lib/injectScript.ts`
- `src/lib/ErrorBoundary.tsx`
- `src/lib/e2eEnabled.ts`
- `src/lib/path.ts`
- `src/lib/index.ts`
- `src/types/pty.ts`
- `src/types/fs.ts`
- `src/types/git.ts`
- `src/types/notify.ts`
- `src/types/index.ts`
- `src/theme/colors.ts`
- `src/theme/index.ts`
- `src/panels/index.ts`
