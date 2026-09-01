# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

配色方案系统单点（硬约束 #6）。`schemes/<scheme>.ts` 定义值，`SchemeRegistry` 管理注册与 active 切换，`colors.ts` 代理导出 token，`overrides.ts` 注入三方库（Dockview / Allotment / CM6）。终端配色经 `panels/terminal/theme.ts` adapter 映射 active 方案 terminal 段。

## 关键约束与决策

### 值文件结构

- `schemes/types.ts` 定义 `ColorScheme` 及 ui/terminal/editor/libraries 四段接口。槽位带区域级消费注释（D8），新方案对象标注 `: ColorScheme` 后编辑器 hover 即可见注释，零额外注释负担。
- `schemes/linear.ts` 是唯一内置默认方案（id `linear`），为用户定制方案（值契约锚点 = ADR-0003 定稿）。
- `schemes/index.ts` side-effect 注册 `linear`；新增方案在此追加。

### SchemeRegistry

模块级单例，与 `CliProfileRegistry` 等同模式。`setActive` 未知 id 时 `console.warn` + 回退 `linear`（ACC-04 降级冒烟）。`getActive()` 回退语义依赖 `linear` 恒已注册。

### colors.ts facade

不定义任何颜色值，import 时取 `schemeRegistry.getActive().ui` 逐 token 代理导出。`ROOT_CSS_VARS` 由 `main.tsx` 注入 `document.documentElement`，App.css 仅 `var()` 引用。

### overrides.ts 五导出

- `dockviewVarStyle()` / `allotmentVarStyle()`：active 方案 libraries 段 → CSS 变量 style 对象。
- `editorTheme`：模块级常量 = active 方案 `editor.theme`（linear 为 oneDark 直 import 透出）。
- `editorColorOverrides()`：active 方案 `editor.overrides` → CM6 `EditorView.theme` 扩展。
- `editorSyntaxHighlight()`：active 方案 `editor.overrides.syntax` → CM6 `syntaxHighlighting` 扩展。

函数形导出支持 D2 热切换；`editorTheme` 为常量，新窗口重载生效。

### CM6 层叠陷阱（ACC-05）

`@codemirror/view` 的 `mountStyles()` 将 styleModule facet 数组 `concat(baseTheme)` 后 **`reverse()`** 再注入 `<style>` 标签——先声明的主题规则排最后，同特异性下恒胜。

- **`editorColorOverrides`** 靠选择器特异性决胜：规则前缀 `"&.cm-editor"` / `"&.cm-editor .cm-content"` / `"&.cm-editor .cm-gutters"` 等，使 compiled 选择器特异性高于 oneDark。
- **`editorSyntaxHighlight`** 与 oneDark 的 HighlightStyle 是同机制竞争，无法靠选择器前缀决胜——消费点扩展数组必须置于 `editorTheme` 之前（`[editorSyntaxHighlight(), editorTheme, editorColorOverrides(), ...]`）。

**改覆盖前必读**：与 oneDark 同值时不暴露、改值即暴露；平级选择器会因 reverse 层叠恒输。新增 syntax 映射只能靠数组顺序，不得后置 `editorTheme`。

### 启动链时序（main.tsx）

静态 import 面最小化，`./lib` barrel / theme 会在 `setActive` 前求值 facade，故全部经动态 import 绕开。序列：

1. IPC 就绪等待 + fail-safe（超时错误页）
2. `loadSettings()` + 注册 linear + `schemeRegistry.setActive(settings?.colorScheme)`
3. `import("./theme")` 取 `ROOT_CSS_VARS` 注入 `documentElement`
4. E2E helpers
5. `import("./App")` + render

**启动链 fail-safe 三处静态色**（先于方案加载，不随方案切换，改 linear 对应 ui 值须手动同步）：`index.html` body background、`tauri.conf.json` window backgroundColor、`main.tsx` 超时错误页。交叉引用登记于 `schemes/linear.ts` 文件头注释。

### 终端 adapter

`panels/terminal/theme.ts` 不再是独立主题定义，而是展开 `schemeRegistry.getActive().terminal` 25 键进 xterm `ITheme`；非色选项原位保留。新面板的终端类渲染器配色同样走方案系统，不再登记新例外。

## 外部坑/红线

- **启动链 fail-safe 三处静态色**：改 linear 对应 ui 值后必须手动同步三处硬编码。
- **CM6 reverse 层叠**：新增 overrides 覆盖必须验证选择器特异性；syntax HighlightStyle 只能靠扩展数组顺序。
- **colors.ts 不定义颜色值**：所有消费只引用 facade token；禁止硬编码颜色。
- **editorTheme 常量**：D2 切换不生效，需重载窗口。

## 测试模式

- **L2**：守注册/切换/降级/完整性；守 CM6 层叠选择器形态（ACC-05 防回归）；守 token 集合。
- jsdom 的 `getComputedStyle` 不支持 `<style>` 规则层叠，断言用规则文本/选择器形态而非 computed。
- token 值正确性由消费方测试断言（如 git-gutter / explorer-git-status）。

## 运行

```bash
npx vitest run scheme-registry overrides
npm test
```
