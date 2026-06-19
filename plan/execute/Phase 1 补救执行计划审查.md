# Phase 1 补救执行计划审查

审查日期：2026-06-19 | 审查依据：`plan/execute/Phase 1 验收审计.md` + `plan/execute/Phase 1 结果验证.md` + 4 agent 网络并行检索交叉核验
审查对象：`plan/execute/Phase 1 补救执行计划.md`

---

## 一、关键发现（需修正）

### 1.1 D1 Ctrl+C 逻辑与需求冲突 + xterm.js 源码证伪 `hasSelection` 检查（严重）

**核查结果**（4 agent 并行网络检索 + xterm.js 源码 `CoreBrowserTerminal.ts`/`Keyboard.ts` 精确核验）：

✅ **审查意见确认正确。D1 必须删除 Ctrl+C+hasSelection→复制逻辑。**

补充证据链：
1. **xterm.js `_keyDown` 完全不检查 `hasSelection()`**（`CoreBrowserTerminal.ts:845-935`）——Ctrl+C 键码 `String.fromCharCode(67-64)` = `\x03`，`Keyboard.ts` 在 `default` 分支求值，无任何 `hasSelection()` 引用
2. **浏览器 `copy` 事件与 `keydown` 事件独立触发**——Ctrl+C 有选区时：浏览器先触发 `copy`（xterm.js `copyHandler` 填充剪贴板），**同时**触发 `keydown` → `_keyDown` 发送 `\x03` 到 PTY。两者都发生，不是二选一
3. **需求基线**：`plan/1. 需求与分阶段计划.md` 第 15 行——"复制 = Ctrl+Shift+C（Ctrl+C 保留为中断，供 claude 取消用）"
4. **要阻止 `\x03` 发送，必须在 `attachCustomKeyEventHandler` 中返回 `false`**——自定义 handler 是 `_keyDown` 第一条逻辑（`CoreBrowserTerminal.ts:849-851`），返回 `false` 则整个方法退出

**修正结论**：D1 Ctrl+C 逻辑应为"**永不拦截**"。Ctrl+Shift+C/V 保持不变。删除 hasSelection 检查。

---

### 1.2 D5 假设右键菜单已存在——实际上需要创建（严重）

**核查结果**（本地源码审查 + Dockview 6.6 文档核验）：

✅ **审查意见确认正确。Workspace.tsx 当前无右键菜单、无工具栏、无面板创建 UI 入口。**

补充证据：
1. `src/workspace/Workspace.tsx` 当前仅渲染 `<DockviewReact>` + `watermarkComponent`，无 `getTabContextMenuItems` 回调、无工具栏按钮
2. Dockview 6.6 的标签右键菜单需显式提供 `getTabContextMenuItems` 回调才有内容。内置菜单项仅限 `close`/`closeOthers`/`closeAll`，"新建终端"需用自定义 `{ label, action }` 对象
3. 结果验证 §三.2 明确记录"Dockview 面板创建无 UI 入口"为已知限制

**修正结论**：D5 应**创建**面板入口（getTabContextMenuItems 回调 + 工具栏按钮），而非"修复"不存在的东西。同时传入 `renderer: 'always'`。

---

### 1.3 D5 `renderer: 'always'` 的紧迫性被高估（中低）

**核查结果**（Dockview 源码调用链裁决——`doSetActivePanel()` vs `doRemovePanel()` 分离分析）：

✅ **审查意见确认正确。PTY 在 `onlyWhenVisible` 下也不会被杀。**

补充证据（Dockview 源码 5 条独立调用链确认）：
1. **`doSetActivePanel()`（标签切换）**：调 `removeChild` 摘除 DOM → 但**不调 `dispose()`** → Portal 不从 portalStore 移除 → React 组件保持挂载 → `useEffect` cleanup **不执行**
2. **`doRemovePanel()`（关闭面板）**：调 `removeChild` + **调 `dispose()`** → Portal 从 portalStore 移除 → React 组件卸载 → cleanup 执行
3. `ReactPanelContentPart` 未实现 `onShow()`/`onHide()`（空操作），切换标签时不触发任何组件生命周期
4. `onlyWhenVisible` 的实际影响：DOM 脱离文档树 → `getBoundingClientRect` 返回全 0 → xterm.js `ResizeObserver` 报告 0×0——终端渲染归零，但实例和 PTY 连接完好
5. `renderer: 'always'` 的价值：避免 DOM 脱离 → 保持 xterm.js 渲染/尺寸正确——这是 **UX 优化**（防尺寸归零+闪烁），不是 PTY 保活保障

**修正结论**：D5 renderer 严重程度从"PTY 被杀"降为"UX 缺陷"。审计 §3.5 "PTY 存活无保障"判定需修正——PTY 实际上有保障。

---

## 二、中等发现（需调整）

### 2.1 D4 + D2 依赖时序冲突

**问题**：补救计划 D4 把 `@xterm/addon-serialize` 移到 devDependencies，D2 在 useXterm.ts 中 `import { SerializeAddon } from "@xterm/addon-serialize"` 并加载。生产构建时此 addon 不在 dependencies 中——如果代码路径无条件走到 `loadAddon(new SerializeAddon())`，运行时报错。

补救计划 §7 提到了"用 `import.meta.env.DEV` 条件编译"但 D2 agent 操作中没有对应步骤。

**修正结论**：D2 操作中明确加条件守卫：
```typescript
// 仅 DEV/E2E 模式暴露 serialize hook
if (import.meta.env.DEV) {
  const serializeAddon = new SerializeAddon();
  term.loadAddon(serializeAddon);
  window.__e2e_getTerminalText = () => serializeAddon.serialize();
}
```
`import.meta.env.DEV` 在 Vite 生产构建时被静态替换为 `false`，整个分支被 tree-shake 移除——不会引用 `@xterm/addon-serialize`。

**落点**：D2 操作第①②步

---

### 2.2 D3 `key_encoding_ctrl_c` 测试修改歧义

**核查结果**（`@xterm/addon-serialize` 源码 + headless API 双重核验）：

✅ **审查意见确认正确。`\x03` 控制字符无法通过 serialize 验证。**

补充证据：
1. **`\x03`（ETX）是 C0 控制字符**，被 xterm.js parser 的 InputHandler 消费——不进 buffer 单元格。`serialize()` 从 buffer 单元格属性反向重建 ANSI 序列，**无法恢复原始控制字符**
2. **正确测试方式**：headless `term.input('\x03')` → `onData` 收到 `'\x03'`字节。headless Terminal 支持 `input()` 方法（v6.0.0 `.d.ts` 确认），不需要 DOM KeyboardEvent
3. **方案 A（推荐）**：用 headless `term.input('\x03')` + `onData` 断言收到 `\x03`——验证"按键产生正确字节序列"

**修正结论**：D3 ctrl_c 测试改为 headless `term.input('\x03')` + `onData` 断言。

---

### 2.3 E2E test D2/D5 未明确面板创建入口对测试的影响

**问题**：E2E 测试中通过 `window.__dockviewApi.addPanel()` 创建面板。如果 D5 创建了右键菜单（而非暴露在此 API），E2E 测试可能需要通过右键菜单交互来创建终端面板，而非直接调用 API。

**建议**：保持 E2E 测试中通过 API 创建面板（更快更稳定），只需在 `addPanel` 中传入 `renderer: 'always'`。右键菜单仅做人工验收时的 UI 入口。这样 D2 和 D5 互不依赖。

**落点**：D2/D5 操作

---

## 三、确认正确的部分

| D# | 项 | 验证结果 |
|----|----|---------|
| D4 | `@codemirror/basic-setup` 冗余 | ✅ npm 官方标记 deprecated——"In version 6.0, this package has been renamed to just 'codemirror'"。`codemirror` v6 的 `basicSetup` 完全覆盖。代码无任何 import 引用此包 |
| D4 | `@xterm/addon-serialize` + `@xterm/headless` → devDependencies | ✅ 仅测试使用，移到 devDependencies 正确（但需配合 D2 条件加载） |
| D2 | `SerializeAddon` 与 `WebglAddon` 兼容 | ✅ agent 检索确认 Buffer API 与渲染层完全独立 |
| D1 | `prevent-default` 不干扰 Ctrl+C | ✅ `AreBrowserAcceleratorKeysEnabled = false` 保留文本编辑键 |
| D1 | IME 防御可推后 | ✅ 合理——xterm.js #5023/#5734 已关闭（v7.0.0） |
| D3 | shift_tab 永真断言修复 | ✅ `expect(result).toContain('\x1b[Z')` 是真验证 |
| D5 | E2E 同步传入 renderer | ✅ 方向正确（但需先创建面板入口） |

---

## 四、遗漏项

### 4.1 未考虑 Port 22 便携 Node 方案对 D4 的影响

当前 `npm run wdio` 脚本使用便携 Node 22（`.temp/node22/node.exe`），D4 的 `npm install` 应在便携 Node 22 环境下运行以确保 `package-lock.json` 的 engine 兼容性。补救计划未提及此点。

修正：D4 agent 操作最后一步用项目根目录下的 Node 22 可执行文件运行 `npm install`。

### 4.2 缺少回归验证后的 DoD 收口步骤

补救计划 §5.3 DoD 含"Phase 1 结果验证文档更新反映修复后状态"，但未给具体操作。建议在 W2 验证通过后由人工（或在 workflow 中由最后一个 agent）更新结果验证文档。

---

## 五、决策台账争议项

| # | 补救计划决策 | 审查意见 |
|---|-----------|---------|
| D1 | Ctrl+C + hasSelection → 复制 + preventDefault | ❌ 与需求 Ctrl+C=中断冲突，应简化为不拦截 Ctrl+C |
| D2 | `__e2e_getTerminalText` 通过 serialize 返回文本 | ✅ 但需加 DEV guard |
| D3 | key_encoding_ctrl_c 验证 `\x03` | ⚠️ serialize 可能不保留控制字符 |
| D4 | 移除 `@codemirror/basic-setup` | ✅ 确认安全 |
| D5 | 修复右键菜单 addPanel + renderer:'always' | ❌ 右键菜单不存在，需创建。renderer 紧迫性被高估 |

---

## 六、审查总结（核查后定论）

**总体评价**：补救计划结构完整。经 4 agent 大规模并行网络检索 + 本地源码深度审查（xterm.js CoreBrowserTerminal.ts/Keyboard.ts、Dockview doSetActivePanel/doRemovePanel 调用链、SerializeAddon buffer 重建机制、npm @codemirror/basic-setup 废弃状态）后：

| 审查意见 | 核查结果 | 证据 |
|---------|---------|------|
| §1.1 D1 Ctrl+C 与需求冲突 | ✅ **确认**——xterm.js `_keyDown` 不检查 hasSelection()，需求基线 Ctrl+C=SIGINT | xterm.js 源码 + 需求文档第 15 行 |
| §1.2 D5 右键菜单不存在 | ✅ **确认**——Workspace.tsx 无 getTabContextMenuItems，需创建 | 本地源码审查 |
| §1.3 D5 renderer 紧迫性高估 | ✅ **确认**——`doSetActivePanel` 不调 dispose()，PTY 存活有保障 | Dockview 5 条调用链裁决 |
| §2.1 D4+D2 条件加载缺失 | ✅ **确认**——需 `import.meta.env.DEV` guard | Vite 静态替换机制 |
| §2.2 D3 `\x03` 控制字符问题 | ✅ **确认**——C0 控制字符不进 buffer，serialize 无法保留 | SerializeAddon 源码 |
| D4 @codemirror/basic-setup | ✅ **确认**——npm 官方标记 deprecated | npm registry |

**最终：6/6 条全部确认正确。** 审查文档的 3 个核心修正意见全部成立：
1. **D1 → Ctrl+C 永不拦截**（删除 hasSelection 复制逻辑）
2. **D5 → 创建面板入口 + renderer:'always'**（而非"修复"不存在的菜单）
3. **D2 → 加 DEV guard**（`import.meta.env.DEV` 条件加载 SerializeAddon）
