# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

配色方案系统单点（硬约束 #6）——颜色定义于 `schemes/<scheme>.ts` 方案值文件，`SchemeRegistry` 管理方案注册与 active 切换，`colors.ts` 为 facade 代理导出 token，`overrides.ts` 将方案值注入三方库（dockview / allotment / CM6）。方案切换（D2）后五个通道（React inline style / xterm ITheme / CM6 theme / dockview CSS 变量 / allotment CSS 变量）随 active 方案更新。配色源自 JetBrains IDEA 暗色主题（Darcula）。终端配色经 `panels/terminal/theme.ts` adapter 映射 active 方案 terminal 段（见 @../panels/CLAUDE.md）。

## 架构决策

### schemes/ 值文件（定义层）

- **`types.ts`**：`ColorScheme` / `UiTokens` / `TerminalPalette` / `EditorScheme` / `LibraryOverrides` 接口定义。每个槽位带**区域级消费注释**（语义 + 消费区域/组件，行号不入注释，决策 D8）——**权威消费注释**，新方案值文件对象标注 `: ColorScheme` 后编辑器 hover 即显示 JSDoc，新方案零注释负担。**注释双处并存（2026-08-08 用户需求）**：内置默认方案 `darcula.ts` 另含 UI 区域速查注释（逐值标注影响的前端区域，可读性优先）；types.ts 为权威，darcula.ts 速查与其保持一致。
- **`darcula.ts`**：内置默认方案（id `"darcula"`，即 settings.json `colorScheme` 段缺省值）。四段全量值：**ui 段** 6 组（gitFile 7 / gitGutter 3 / explorer 5 / sidebar 8 / agentStatusUsage 3 / errorBanner 3）+ 23 标量；**terminal 段** 25 键（前景/背景/光标/选区 + ANSI 16 色 + 滚动条滑块，ITheme 兼容）；**editor 段** = oneDark 直 import 透出（决策 D6）+ overrides（lint 7 键 / searchMatch 4 键 / background）；**libraries 段** = dockview 20 条 CSS 变量 + allotment 2 键。值一律搬运现状（D1 零视觉变化，禁止新造）。
- **`index.ts`**：side-effect 注册文件（照 `sideViewDefs.ts` 模式）——import 时 `schemeRegistry.register(darcula)`。新增方案在此追加。

### SchemeRegistry 注册表（`schemeRegistry.ts`）

模块级单例（项目第 6 个注册表单例，同 `CliProfileRegistry` 模式），七方法：`register(scheme)`（同 id 覆盖）/ `get(id)` / `getActive()` / `setActive(id)` / `getAll()` / `getDefaultId()` / `_reset()`（仅测试）。**`setActive` 未知 id → `console.warn` + 回退默认 darcula**（ACC-04 降级冒烟）。内置方案经 `schemes/index.ts` side-effect 注册，本文件不直接 import 具体方案；`getActive()` 回退语义依赖 darcula 恒已注册。

### colors.ts facade（代理层）

**本文件不定义任何颜色值**——import 时取 `schemeRegistry.getActive().ui` 段逐 token 代理导出（31 导出名不变，D5：369 消费点零改动），组件只引用 facade token（硬约束 #6）。`ROOT_CSS_VARS` = `{ "--sl-bg-primary", "--sl-fg-primary" }`，由 main.tsx 注入 `document.documentElement`，App.css 仅 `var()` 引用。求值时机由启动链保证（见下）——本文件首次求值发生在 `setActive` 之后；测试环境无启动序列，`getActive()` 默认 darcula，值正确。

### overrides.ts 四导出（库注入层）

- `dockviewVarStyle()` — active 方案 `libraries.dockview` 20 条 CSS 变量 → React style 对象（键为变量名原样），PageDockviewHost 根 div 内联注入
- `allotmentVarStyle()` — 2 键（`--separator-border` / `--focus-border`），Workspace 根容器注入，CSS 变量继承覆盖内层 SideBarArea
- `editorTheme` — **模块级常量** = active 方案 `editor.theme`（darcula 为 oneDark 透出），求值时机由启动链保证
- `editorColorOverrides()` — active 方案 `editor.overrides` → CM6 `EditorView.theme` 扩展（lint 7 键 / searchMatch 4 键 / background，`dark: true`）

函数形导出每次调用取当前 active 方案（支持 D2 热切换）；`editorTheme` 为常量（新窗口重载生效）。

### editorColorOverrides 的 CM6 层叠（ACC-05 实证，改覆盖前必读）

`@codemirror/view` 的 `mountStyles()` 将 styleModule facet 数组 `concat(baseTheme)` 后 **`reverse()`** 再注入 `<style>` 标签——**扩展数组中先声明的主题（`editorTheme`/oneDark）规则排最后，同特异性下恒胜**，"后声明应胜出"的直觉在此失效。overrides 覆盖与 oneDark 同值时不暴露、改值即暴露（D1 零视觉变化下的隐性失效；ACC-05 五通道冒烟实证，守卫测试见下）。

**现行修复形态（特异性方案，不依赖扩展数组顺序，四处消费点零改动）**：与 oneDark 竞争的规则提升选择器特异性——

- background 键 `"&"` → `"&.cm-editor"`（编译为 `.ͼx.cm-editor`，0,2,0 > oneDark 的 `&` 0,1,0）
- searchMatch 三键（`.cm-searchMatch` / `.cm-searchMatch.cm-searchMatch-selected` / `.cm-selectionMatch`）前缀 `"&.cm-editor "`（0,3,0/0,4,0 > oneDark 对应 0,2,0/0,3,0）
- lint 规则（oneDark 无 lint 规则，无竞争）与 lint tooltip（`.cm-tooltip.cm-tooltip-lint` 0,3,0 已赢 oneDark `.cm-tooltip` 0,1,0）不动

**新增/修改 overrides 覆盖时**：与 oneDark 竞争的选择器必须保持 `.cm-editor` 前缀形态（平级选择器会因 reverse 层叠恒输，且与 oneDark 同值时不报错——测试无法发现，只能实测改值验证）。

### 启动链时序（main.tsx，spec §5）

静态 import 面最小化（仅 react、react-dom/client）——`./lib` barrel → ErrorBoundary → theme 会在 `setActive` 前求值 facade，故**必须绕开 barrel，全部经动态 import**。序列：

```
① IPC 就绪等待 + fail-safe（超时错误页）
② loadSettings().catch(() => null)      // 失败降级 null → darcula，不阻塞启动
   + import("./theme/schemeRegistry") + import("./theme/schemes")（注册 darcula）
   + schemeRegistry.setActive(settings?.colorScheme)   // 未知/非字符串 → 注册表内部回退
③ import("./theme") 取 ROOT_CSS_VARS 注入 documentElement
④ E2E helpers（dev serve 或 VITE_E2E=1 构建，内联门控）
⑤ import("./App") + render               // App 模块图内静态引用 theme token，须在 ② 之后求值
```

**启动链 fail-safe 三处**（先于方案加载，不随方案切换，改 darcula 对应 ui 值须手动同步）：`index.html:10` body background、`src-tauri/tauri.conf.json:20` window backgroundColor、`main.tsx:31` 超时错误页（#1e1e1e/#1e1e2e 等）。交叉引用登记于 `schemes/darcula.ts` 文件头注释（spec §9.1）。

### 终端 adapter（既定例外的收敛表述）

`panels/terminal/theme.ts` **不再是独立主题定义**——`theme: { ...schemeRegistry.getActive().terminal }` 将 active 方案 terminal 段 25 键展开进 xterm `ITheme`；非色选项（`drawBoldTextInBrightColors`、`vtExtensions.kittyKeyboard`、scrollback 等）原位保留不动。例外范围收敛：仅 terminal 段经 adapter 映射，新面板的终端类渲染器配色同样走方案系统，不再登记新例外（见 @../panels/CLAUDE.md）。

## 文件

| 文件 | 职责 |
|------|------|
| `colors.ts` | facade（代理 active 方案 ui 段，31 导出名不变；`ROOT_CSS_VARS` 供 main.tsx 注入根 CSS 变量） |
| `index.ts` | barrel export：colors.ts 31 导出 + schemeRegistry/SchemeRegistry + schemes（darcula + ColorScheme 等类型）+ overrides 四导出 |
| `schemes/types.ts` | 接口定义（ColorScheme/UiTokens/TerminalPalette/EditorScheme/LibraryOverrides），槽位区域级消费注释（D8） |
| `schemes/darcula.ts` | 内置默认方案（id darcula）全量值：ui 6 组 + 23 标量 / terminal 25 键 / editor oneDark 透出 + overrides / libraries dockview 20 + allotment 2；文件头 fail-safe 交叉引用 |
| `schemes/index.ts` | side-effect 注册文件：`schemeRegistry.register(darcula)`；新增方案在此追加 |
| `schemeRegistry.ts` | SchemeRegistry 模块级单例（七方法，未知 id 回退 darcula + console.warn，`_reset` 仅测试） |
| `overrides.ts` | 四导出：`dockviewVarStyle` / `allotmentVarStyle` / `editorTheme` / `editorColorOverrides` |

## 新增配色方案的步骤

1. 新建 `schemes/<name>.ts` 值文件——对象标注 `: ColorScheme`（编辑器 hover 即显示 types.ts 槽位 JSDoc，零注释负担）；值搬运参考现状（D1 零视觉变化）
2. 在 `schemes/index.ts` 追加 `import { <name> } from "./<name>";` + `schemeRegistry.register(<name>);`
3. settings.json 写 `"colorScheme": "<id>"` → 重载窗口生效（D2）；消费方（colors.ts facade / overrides.ts / 终端 adapter）零改动
4. 改动 darcula 对应 ui 值后，手动同步启动链 fail-safe 三处静态色（见 darcula.ts 文件头注释）

## 测试模式

> 用例数为快照，最新计数以 `.claude/test-inventory.md` 为准。

- **`scheme-registry.test.ts`**（18 用例）：register/get/getAll/getDefaultId、setActive 已知 id、setActive 未知 id 回退 darcula + console.warn、getActive 默认 darcula、重复注册覆盖、`_reset` 隔离、darcula 四段完整性（ui 6 组键数 7/3/5/8/3/3 + 23 标量、terminal 25 键、editor oneDark 透出非 undefined、libraries dockview 20 条 + allotment 2 键）
- **`overrides.test.ts`**（7 用例）：dockviewVarStyle 键集合 20 条且值为 active 方案色、allotmentVarStyle 2 键、editorTheme === active 方案 editor 段、editorColorOverrides 返回 CM6 扩展（lint/searchMatch/background 键生效）、setActive 后输出跟随切换、**「层叠胜出（ACC-05 修复守卫）」——断言 background 规则选择器为 `/^\.ͼ[0-9a-z]+\.cm-editor$/` 形态、searchMatch 选择器带 `.cm-editor ` 前缀 + 负断言无裸平级选择器（防回归写回）。注意：jsdom 的 `getComputedStyle` 不支持 `<style>` 规则层叠，守卫用规则文本/选择器形态断言而非 computed**
- **facade/消费方断言**：`colors.test.ts` 守 token 集合与 ROOT_CSS_VARS 键集合；token 值正确性由消费方测试断言（如 `git-gutter.test.ts` 的 GutterMarker DOM 颜色断言、`explorer-git-status.test.tsx` 着色断言）。新增 token 时同步更新消费方断言

### 运行

```bash
npx vitest run scheme-registry overrides   # 仅方案系统测试
npm test                                   # L2 全量
```
