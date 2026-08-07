# 配色系统重构 需求规格文档

> 本文档是配色系统重构的唯一执行依据。所有决策已经 grilling 确认（8 项），开发时按 §6 文件清单与 §7 测试规格执行，验收按 §10。
> 现状依据：[`color-implementation.md`](./color-implementation.md)、[`color-inventory.md`](./color-inventory.md)（权威，两处勘误见 §9.2）。
> 状态：需求已冻结（2026-08-07 确认），代码实施另行发起。

## 1. 背景与目标

前端颜色散落在 6+ 条独立色源，改色需多点人工同步且有漏网点。本次重构：

1. **单点配置**：全部应用可控色源收拢到 `src/theme/`，每条颜色的消费位置有注释可查
2. **可扩展架构**：注册表模式（项目既有惯例）实现配色方案机制，新增方案 / 一键切换预留接口

**核心约束**：

- **零视觉变化**——纯重构，所有覆盖值取现行有效值，不改任何渲染结果
- **仅暗色系**方案（项目定位约束，不可违背）
- 切换 = 手编 settings.json + 重载窗口生效（非运行期即时切换）

## 2. 已确认决策表（8 项）

| # | 决策点 | 结论 |
|---|--------|------|
| D1 | 统一边界 | 全部应用可控色源（colors.ts 32 token + xterm 22 色 + oneDark + dockview `--dv-*` + Allotment + `App.css --sl-fg-*` + 关键库默认显式化）；启动链 fail-safe 保持硬编码 + 注释交叉引用 |
| D2 | 切换形态 | settings.json `colorScheme` 段持久化 + 重载窗口生效 |
| D3 | 切换入口 | 手编 settings.json；registry 预留 `listSchemes()` 供未来设置 UI 枚举（照 keybindings 先例） |
| D4 | 文件组织 | 完整 `schemes/` 目录 |
| D5 | 消费方迁移 | Facade 保留同名导出，369 处消费点零改动 |
| D6 | oneDark | 作 `scheme.editor.theme` 引用（theme + overrides 结构），不完全 token 化 |
| D7 | 死配置清理 | 全清：3 死 token + 2 零消费 CSS 变量删、`--sl-fg-primary` 收编、JsonMode 违规收敛 |
| D8 | 注释粒度 | 区域级注释放 `schemes/types.ts` 接口槽位；行号不入注释；inventory 保留为精确全量清单 |

## 3. 术语（实施时写入 CONTEXT.md）

- **配色 token**（Color Token）：UI 颜色的语义命名槽位（如 panelBg/focusBorder）。组件只引用 token，禁止硬编码颜色（硬约束 #6）。token 定义在配色方案的 ui 段，经 colors.ts facade 导出供组件消费。
- **配色方案**（Color Scheme）：一套完整配色定义的注册单元——ui token 取值 + 终端调色板 + 编辑器主题引用与覆盖 + 三方库变量覆盖四段。仅暗色系（定位约束）。当前内置 darcula 一套。
- **方案注册表**（SchemeRegistry）：配色方案的模块级单例注册表。方案经 register 注册、setActive 激活；激活方案在启动时（React 挂载前）解析，切换方案需重载窗口生效。
- **启动链 fail-safe 色**：React 挂载前防白闪的硬编码色（index.html body 底色、tauri.conf.json 窗口底色、main.tsx 超时错误页）。不在配色方案系统内，与方案色值手动同步。

## 4. 目标架构

```
src/theme/
├── schemes/
│   ├── types.ts        — ColorScheme 接口 + 每槽位区域级注释（需求 1 注释落点，唯一含消费注释的文件）
│   ├── darcula.ts      — 内置默认方案：当前全部色值（仅组名分节注释，消费注释一律在 types.ts）
│   └── index.ts        — side-effect 注册（照 tabRules/sideViewDefs 模式）
├── schemeRegistry.ts   — SchemeRegistry 模块级单例（项目第 6 个注册表单例）
├── colors.ts           — facade：31 个同名导出，值 = getActive() 对应槽位（模块求值取一次）
├── overrides.ts        — 库覆盖应用：dockviewVarStyle / allotmentVarStyle / editorTheme / editorColorOverrides()
└── index.ts            — barrel（追加 schemes/registry/overrides 导出）
```

注释可达性：`darcula.ts` 对象字面量标注 `: ColorScheme` 类型后，编辑器 hover 槽位即显示 types.ts 的 JSDoc——注释单点不漂移，新方案零注释负担。

### 4.1 ColorScheme 接口（types.ts）

```ts
export interface ColorScheme {
  id: string;                    // "darcula"
  label: string;                 // 展示名（未来设置 UI 用）
  ui: UiTokens;                  // UI token 槽位（§4.2）
  terminal: TerminalPalette;     // xterm 调色板 25 键（§4.3）
  editor: EditorScheme;          // CM 主题引用 + 覆盖（§4.4）
  libraries: LibraryOverrides;   // dockview + allotment（§4.5）
}
```

### 4.2 UiTokens 与 darcula 值（消费注释格式示例 + 全量值表）

注释格式（types.ts 每槽位 JSDoc：语义 + 消费区域，区域级；高频 token 标计数 +「以 grep 为准」）：

```ts
export interface UiTokens {
  /** 文件名 git 状态色——Commit 视图文件名（CommitFileList）、FileIcon、FileTree 行内状态色 */
  gitFile: {
    /** 已修改文件 */ modified: string;
    // ...
  };
  // ...
}
```

**darcula.ui 全量值**（与现状逐值一致）：

| 槽位 | 值 | 消费区域（注释内容） |
|------|----|----------------------|
| gitFile.modified / added / untracked / deleted / renamed / conflict / ignored | `#6897BB` `#629755` `#D1675A` `#6C6C6C` `#3A8484` `#D5756C` `#848504` | Commit 视图文件名、FileIcon、FileTree 行内状态色 |
| gitGutter.modified / added / deleted | `#374752` `#384C38` `#656E76` | 编辑器/diff gutter 标记（gitGutter.ts）~~whitespaceOnly 删除~~ |
| explorer.bg / fg / hover / arrowClosed / arrowOpen | `#1E1E1E` `#D4D4D4` `#2A2D2E` `#6C6C6C` `#D4D4D4` | 文件树 + agentStatus/claudeHistory/commit 借用 ~~selected 删除~~ |
| sidebar.bg / fg / hover / selected / border / contextMenuBorder / contextMenuShadow / treeGuide | `#252526` `#D4D4D4` `#2A2D2E` `#37373D` `#444` `#454545` `0 4px 12px rgba(0,0,0,0.5)` `#3C3C3C` | 侧栏树/活动栏/agent 行/右键菜单/树形引导线（27 处 10 文件） |
| errorBanner.bg / border / fg | `#5A1D1D` `#8B0000` `#F48771` | ExplorerPanel 沙箱错误横幅 |
| agentStatusUsage.low / medium / high | `#629755` `#BBB529` `#F44747` | AgentStatusRow 用量条（<50/50-80/>80） |
| panelBg | `#1E1E1E` | 全部面板背景（21 处，以 grep 为准） |
| sidebarBg | `#252526` | 右键菜单底色（4 处） |
| secondaryBg | `#2D2D2D` | 页签按钮/弹窗次级背景（3 处） |
| appBg | `#1e1e2e` | App 根容器 |
| appBgPrimary | `#1e1e2e` | → ROOT_CSS_VARS → `--sl-bg-primary` 全局背景 |
| appFg | `#cdd6f4` | → `--sl-fg-primary` 全局默认文字色（收编自 App.css:6） |
| editorBg | `#282C34` | 编辑器类面板容器背景（5 处） |
| sidebarFg | `#D4D4D4` | 侧栏/hooks 配置面板主要文字（27 处，以 grep 为准） |
| errorFg | `#F44747` | 错误文案/状态（17 处） |
| placeholderFg | `#808080` | 占位符/禁用项/关闭按钮（5 处） |
| buttonFg | `#CCCCCC` | 按钮文字（7 处） |
| dimFg | `#999999` | 次要说明文字（10 处） |
| inputBg | `#3C3C3C` | 输入框背景（11 处） |
| inputBorder | `#6C6C6C` | 输入框/卡片边框——全应用最高频（34 处，以 grep 为准） |
| focusBorder | `#007ACC` | 聚焦边框/活动指示条（10 处） |
| activeSelectionBg | `#094771` | 列表/树选中背景（6 处） |
| separatorBg | `#444` | 分隔线（8 处） |
| contextMenuBorder | `#454545` | 右键菜单边框（5 处） |
| shadowMenu | `rgba(0,0,0,0.5)` | 弹窗遮罩阴影 |
| htmlPanelLoadingFg | `#6C6C6C` | 「加载中…」文案（15 处） |
| htmlPanelIframeBg | `#FFFFFF` | HTML 预览 iframe 白底 |
| onAccentFg | `#FFFFFF` | 强调底色上的文字（收编 JsonMode:213 事件导航 hover） |
| explorerSelectionBg | `#094771` | 文件树/历史行选中背景 |

**删除项**：`DROPDOWN_BG`、`APP_BG_SECONDARY`、`GIT_GUTTER_COLORS.whitespaceOnly`、`EXPLORER_COLORS.selected`。

### 4.3 TerminalPalette（25 键，ITheme 兼容）

```ts
export interface TerminalPalette {
  foreground: string; background: string; cursor: string; cursorAccent: string;
  selectionBackground: string; selectionForeground: string;
  /** 滚动条滑块（默认/hover/激活）——ITheme 原生支持，显式化原运行期派生值 */
  scrollbarSliderBackground: string; scrollbarSliderHoverBackground: string; scrollbarSliderActiveBackground: string;
  black: string; red: string; green: string; yellow: string;
  blue: string; magenta: string; cyan: string; white: string;
  brightBlack: string; brightRed: string; brightGreen: string; brightYellow: string;
  brightBlue: string; brightMagenta: string; brightCyan: string; brightWhite: string;
}
```

**darcula.terminal 值**：22 色与现状一致（见 inventory §2）；滚动条三键 = `rgba(212,212,212,0.2)` / `rgba(212,212,212,0.4)` / `rgba(212,212,212,0.5)`（= foreground 20%/40%/50% 等价，零视觉变化）。

**例外（勘误 1）**：OSC 8 链接色不可配置——xterm 6.x `ITheme` 无 `link` 键（`xterm.d.ts:372-445` 一手确认），列入 §9.1 豁免。

### 4.4 EditorScheme

```ts
export interface EditorScheme {
  theme: Extension;             // darcula = oneDark（@codemirror/theme-one-dark 直接 import 透出，
                                // 测试 mock 包继续生效）
  overrides: {
    /** 编辑器背景——对齐 ui.editorBg */ background: string;                     // "#282C34"
    /** lint 诊断色——JsonMode 语法/schema 波浪线（@codemirror/lint 现行有效值） */
    lint: { error: string; warning: string; info: string; hint: string;
            activeBackground: string; tooltipBackground: string; tooltipBorder: string; };
    /** 搜索匹配高亮——editor/diff/gitshow（oneDark 现行有效值） */
    searchMatch: { match: string; matchOutline: string; selected: string; selectionMatch: string; };
  };
}
```

**darcula.editor.overrides 值**（= 现行有效值，零视觉变化）：

- lint：`#f11` / `orange` / `#999` / `#66d` / `#ffdd9980` / `#2e343e` / `#444`
- searchMatch：`#72a1ff59` / `#457dff` / `#6199ff2f` / `#aafe661a`
- background：`#282C34`

### 4.5 LibraryOverrides

```ts
export interface LibraryOverrides {
  dockview: Record<string, string>;          // "--dv-*" → 值（20 条，下表）
  allotment: { separatorBorder: string; focusBorder: string };
}
```

**darcula.libraries.dockview 20 条**（值 = 现行有效值；与 ui 同值的条目在 darcula.ts 中引用 ui 槽位构造，值单点定义）：

| 变量 | 值 | 引用 |
|------|----|------|
| `--dv-group-view-background-color` | `#1E1E1E` | = ui.panelBg |
| `--dv-tabs-and-actions-container-background-color` | `#252526` | = ui.sidebarBg |
| `--dv-activegroup-visiblepanel-tab-background-color` | `#1E1E1E` | |
| `--dv-activegroup-hiddenpanel-tab-background-color` | `#2D2D2D` | = ui.secondaryBg |
| `--dv-inactivegroup-visiblepanel-tab-background-color` | `#1E1E1E` | |
| `--dv-inactivegroup-hiddenpanel-tab-background-color` | `#2D2D2D` | |
| `--dv-tab-divider-color` | `#1E1E1E` | |
| `--dv-separator-border` | `#444` | = ui.separatorBg（现值 rgb(68,68,68) 等价） |
| `--dv-paneview-header-border-color` | `rgba(204,204,204,0.2)` | |
| `--dv-activegroup-visiblepanel-tab-color` | `#FFFFFF` | 现值 `white` 等价 |
| `--dv-activegroup-hiddenpanel-tab-color` | `#969696` | |
| `--dv-inactivegroup-visiblepanel-tab-color` | `#8F8F8F` | |
| `--dv-inactivegroup-hiddenpanel-tab-color` | `#626262` | |
| `--dv-drag-over-background-color` | `rgba(83,89,93,0.5)` | |
| `--dv-icon-hover-background-color` | `rgba(90,93,94,0.31)` | |
| `--dv-floating-box-shadow` | `0 8px 32px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.25)` | |
| `--dv-floating-border` | `1px solid rgba(255,255,255,0.1)` | |
| `--dv-tabs-container-scrollbar-color` | `#888888` | 现值 `#888` 等价 |
| `--dv-scrollbar-background-color` | `rgba(255,255,255,0.25)` | 显式化 dockview.css:29 全局兜底 |
| `--dv-paneview-active-outline-color` | `dodgerblue` | 拖拽 outline 现值 |

**darcula.libraries.allotment**：`separatorBorder: rgba(128,128,128,0.35)`、`focusBorder: #007fd4`。

### 4.6 SchemeRegistry API

```ts
interface SchemeRegistryAPI {
  register(scheme: ColorScheme): void;      // 同 id 覆盖（项目惯例）
  get(id: string): ColorScheme | undefined;
  getAll(): ColorScheme[];                  // 注册序
  listSchemes(): Array<{ id: string; label: string }>;  // 未来设置 UI 枚举（D3 预留）
  getActive(): ColorScheme;                 // 默认 darcula；activeId 未注册时回退 darcula
  setActive(id: string): void;              // 未知 id → console.warn + 回退 darcula
  _reset(): void;                           // 测试隔离：清空注册表 + active 复位
}
```

### 4.7 facade 映射表（colors.ts，31 个顶层导出）

实现：文件顶部 `const { ui } = schemeRegistry.getActive();`，逐导出取值。

| 导出名 | 映射 |
|--------|------|
| GIT_FILE_COLORS / GIT_GUTTER_COLORS / EXPLORER_COLORS / SIDEBAR_COLORS / AGENT_STATUS_USAGE_COLORS | 直接引用 `ui.gitFile` / `ui.gitGutter` / `ui.explorer` / `ui.sidebar` / `ui.agentStatusUsage`（对象引用相等） |
| 22 个标量（PANEL_BG…ON_ACCENT_FG，含 EXPLORER_SELECTION_BG） | `ui.panelBg` 等逐键 |
| ERROR_BANNER_BG / BORDER / FG | `ui.errorBanner.bg` / `.border` / `.fg` |
| ROOT_CSS_VARS | `{ "--sl-bg-primary": ui.appBgPrimary, "--sl-fg-primary": ui.appFg }` |

求值时序保证：`colors.ts` 只能在 `setActive` 之后首次求值——由 main.tsx 动态 import 链保证（§5）；测试环境无 main.tsx，`getActive()` 默认 darcula，值正确。

### 4.8 overrides.ts API

```ts
export const dockviewVarStyle: Record<string, string>;    // = getActive().libraries.dockview
export const allotmentVarStyle: Record<string, string>;   // { "--separator-border": ..., "--focus-border": ... }
export const editorTheme: Extension;                      // = getActive().editor.theme
export function editorColorOverrides(): Extension;        // EditorView.theme({lint/search/背景规则}, {dark:true})
```

应用机制：

- **dockview**：`PageDockviewHost.tsx:369` 的 `dockview-theme-dark` div `style={dockviewVarStyle as React.CSSProperties}`——inline 自定义属性优先级最高，无样式表加载顺序问题
- **allotment**：`Workspace.tsx` 根容器 style 合并 `allotmentVarStyle`，CSS 变量继承覆盖 `Workspace.tsx:224` 与 `SideBarArea.tsx:72` 两处 Allotment
- **CM**：4 个 oneDark 导入点扩展数组中 `oneDark` 替换为 `editorTheme, editorColorOverrides()`

## 5. main.tsx 启动序列（关键设计）

现状问题：`import App from "./App"`（:3 静态）+ `import { E2E_ENABLED } from "./lib"`（:5，经 lib barrel → ErrorBoundary → theme 链）会使 facade 在 `bootstrap()` 运行前求值——必须重构为动态 import。

目标代码结构：

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { E2E_ENABLED } from "./lib/e2eEnabled";   // 深导入：绕开 lib barrel → ErrorBoundary → theme 链

async function bootstrap() {
  // ① IPC 就绪等待 + fail-safe 分支（现状不变，main-bootstrap.test.tsx 覆盖路径不受影响）

  // ② 配色方案解析（必须在 App 模块图求值前完成）
  const { loadSettings } = await import("./ipc/settings");
  const settings = await loadSettings().catch(() => null);   // 失败降级 null → darcula，不阻塞启动
  const { schemeRegistry } = await import("./theme/schemeRegistry");
  await import("./theme/schemes");                            // side-effect：注册内置方案
  const schemeId = typeof settings?.colorScheme === "string" ? settings.colorScheme : "darcula";
  schemeRegistry.setActive(schemeId);

  // ③ ROOT_CSS_VARS 注入（现有逻辑，改动态 import）
  const { ROOT_CSS_VARS } = await import("./theme");
  for (const [prop, value] of Object.entries(ROOT_CSS_VARS)) {
    document.documentElement.style.setProperty(prop, value as string);
  }

  // ④ E2E helpers（现状不变，保持在 setActive 之后）
  if (E2E_ENABLED) {
    import("../e2e-tests/helpers").then((m) => m.installAllE2eHelpers());
  }

  // ⑤ App 动态导入 + render
  const { default: App } = await import("./App");
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode><App /></React.StrictMode>,
  );
}
bootstrap();
```

CSS 顺序保持：`import "./App.css"` 从 main.tsx 删除，改在 `App.tsx` 的 `import "dockview-react/dist/styles/dockview.css"`（:23）**之后**追加 `import "./App.css"`——精确保持现有加载顺序（dockview.css 先、App.css 后）。

settings.json 新增段：`{ "colorScheme": "darcula" }`（缺省/非法值 → darcula；后端浅合并不需要改动——colorScheme 只读不写，无新 store，勿增实体）。

## 6. 文件级修改清单

### 6.1 新建（5）

| 文件 | 内容 |
|------|------|
| `src/theme/schemes/types.ts` | ColorScheme/UiTokens/TerminalPalette/EditorScheme/LibraryOverrides 接口 + 每槽位 JSDoc 消费注释 |
| `src/theme/schemes/darcula.ts` | darcula 方案全量值（§4.2–4.5）；文件头注释交叉引用启动链 fail-safe 三处（改 appBgPrimary/panelBg/errorFg 时手动同步 index.html:10/tauri.conf.json:20/main.tsx:31） |
| `src/theme/schemes/index.ts` | `schemeRegistry.register(darcula)` side-effect |
| `src/theme/schemeRegistry.ts` | §4.6 API 单例 |
| `src/theme/overrides.ts` | §4.8 四个导出 |

### 6.2 重构（3）

| 文件 | 改动 |
|------|------|
| `src/theme/colors.ts` | facade 化（§4.7）；删 DROPDOWN_BG/APP_BG_SECONDARY/whitespaceOnly/selected；增 ON_ACCENT_FG；ROOT_CSS_VARS 换 `--sl-fg-primary` |
| `src/theme/index.ts` | barrel 追加 schemeRegistry/schemes/overrides 导出 |
| `src/panels/terminal/theme.ts` | adapter：`theme` 段 = `...getActive().terminal`（25 键展开进 ITheme）；非色选项原位保留；文件头注释更新（不再是独立色源） |

### 6.3 消费点修改（9）

| 文件 | 改动 |
|------|------|
| `src/panels/editor/useCodeMirror.ts` | :289 `oneDark` → `editorTheme, editorColorOverrides()`；删 :20 import |
| `src/panels/gitshow/GitShowPanel.tsx` | :143 同上；删 :12 import |
| `src/panels/diff/DiffPanel.tsx` | :521/:566 同上；删 :25 import |
| `src/panels/hooksConfig/JsonMode.tsx` | :160 同上；删 :25 import；**:213 `style.color = "#FFFFFF"` → `ON_ACCENT_FG`** |
| `src/workspace/PageDockviewHost.tsx` | :369 div style 展开 `dockviewVarStyle` |
| `src/workspace/Workspace.tsx` | 根容器 style 合并 `allotmentVarStyle` |
| `src/main.tsx` | §5 启动序列重排 |
| `src/App.tsx` | :23 dockview.css import 后追加 `import "./App.css"` |
| `src/App.css` | 删 :5-7（注释 + `--sl-fg-primary`/`--sl-fg-secondary` hex 定义）；`var()` 引用保留 |

## 7. 测试规格

### 7.1 修改（1 文件确定 + 4 文件验证）

**`src/__tests__/colors.test.ts`**（断言数随删除/新增变化，以实施后实际为准更新 test-inventory）：

| 位置 | 改动 |
|------|------|
| import 块 | 删 `DROPDOWN_BG`、`APP_BG_SECONDARY`；增 `ON_ACCENT_FG` |
| GIT_GUTTER_COLORS describe | 4 → 3 键计数断言；删 whitespaceOnly case |
| EXPLORER_COLORS describe | 6 → 5 键计数断言；删 selected case |
| 通用 UI 色 cases | 删 DROPDOWN_BG/APP_BG_SECONDARY 两行；增 `ON_ACCENT_FG: "#FFFFFF"`；25 → 24 计数断言 |
| ROOT_CSS_VARS describe | 键集合 = `{"--sl-bg-primary", "--sl-fg-primary"}`（len 2 不变）；删 `--sl-bg-secondary` 两条断言；增 `--sl-fg-primary` 值为 `#cdd6f4` |
| 文件头注释 :7 | 「通用 UI 色 20 个独立 token」→ 更新为实际口径（顺带修正既定过时注释） |

**预期零改动（实施时验证，失效再调整）**：

| 文件 | 理由 |
|------|------|
| `theme.test.ts`（L2） | adapter 后 terminalOptions 结构/值相同（theme 段多 3 滚动条键——若有键集合断言需同步） |
| `main-bootstrap.test.tsx` | fail-safe 分支在方案解析之前 return，不受影响 |
| `gitshow-panel.test.tsx` / `hooks-config-jsonmode.test.tsx` | darcula.ts 直接 import oneDark 包透出，包 mock 继续生效；失效则改 mock theme facade |
| `test/terminal/theme-options.test.ts`（L3） | 16 色值不变 |

### 7.2 新增（2 文件）

**`src/__tests__/scheme-registry.test.ts`**（约 15 用例）：

1. register + get 返回同一方案
2. 同 id register 覆盖（项目惯例）
3. get 未注册 id → undefined
4. getAll 按注册序返回
5. listSchemes 返回 `[{id, label}]`
6. getActive 默认返回 darcula
7. setActive 已注册 id → getActive 切换
8. setActive 未知 id → console.warn + 回退 darcula
9. _reset 清空注册表 + active 复位（后续 getActive 在重注册后仍可用）
10. darcula 完整性：ui 六组键数 = 7/3/5/8/3/3，标量 23 键全非空字符串
11. darcula.terminal 25 键全非空
12. darcula.editor.overrides 全槽位非空（lint 7 + searchMatch 4 + background）
13. darcula.libraries.dockview 键全 `--dv-` 前缀且 ≥20 条；allotment 2 键非空
14. facade 一致性抽样：`PANEL_BG === getActive().ui.panelBg`、`GIT_FILE_COLORS === getActive().ui.gitFile`（引用相等）、`ROOT_CSS_VARS["--sl-fg-primary"] === getActive().ui.appFg`
15. `terminalOptions.theme` 键集合 = TerminalPalette 25 键（adapter 契约，可并入 theme.test.ts）

**`src/__tests__/overrides.test.ts`**（约 6 用例）：

1. dockviewVarStyle 与 `getActive().libraries.dockview` 内容一致、键全 `--dv-` 前缀
2. allotmentVarStyle 键集合 = `{"--separator-border", "--focus-border"}`
3. editorTheme 引用 === `getActive().editor.theme`
4. editorColorOverrides() 返回非空 Extension
5. editorColorOverrides 规则值来源 `getActive().editor.overrides`（两连调用同源断言）
6. ON_ACCENT_FG 被 JsonMode 消费（组件级，可并入 hooks-config-jsonmode.test.tsx）

### 7.3 test-inventory 同步

登记 scheme-registry / overrides 两新文件用例数；colors.test.ts 用例数更新（项目规则：用例清单同步）。

## 8. 文档同步清单

| 文件 | 改动 |
|------|------|
| `CONTEXT.md` | 追加 §3 四术语（配色 token / 配色方案 / 方案注册表 / 启动链 fail-safe 色），格式照现有条目 |
| `.claude/adr.md` | 追加 ADR-0002（§8.1 全文） |
| 根 `.claude/CLAUDE.md` | 硬约束 #6 改：「配色单点：所有颜色只在 `theme/` 配色方案系统定义（`schemes/` 注册方案 → `colors.ts` facade 导出 token）；组件引用 token，禁止硬编码颜色（既定例外：启动链 fail-safe 色，见 src/theme/CLAUDE.md）」；模块索引 src/theme 行职责更新 |
| `src/theme/CLAUDE.md` | 重写：职责 = 配色方案系统单点；架构决策（四段接口 / registry / facade / overrides / main.tsx 启动序列 / 注释落点）；文件表；「新增方案步骤」（新文件 + register 一行 + settings.json 指向 + 重载）；测试模式（scheme-registry/overrides/colors 三文件）；删「既定例外」「无独立测试文件」两条过时声明 |
| `src/panels/CLAUDE.md` | 硬约束 #6 例外句改：「终端配色经 `terminal/theme.ts` adapter 映射 active 方案 terminal 段，色源在 `theme/schemes/`，非独立主题定义」 |
| `docs/color-implementation.md` / `color-inventory.md` | 收尾更新为新架构现状（删「临时摸底」注记），继续充当颜色域全量参考；inventory §6.1 两处勘误（§9.2） |

### 8.1 ADR-0002 全文（实施时写入 .claude/adr.md）

```markdown
## 0002 配色方案单点（schemes/ + 注册表 + facade + 重载切换）

**Status**: accepted（2026-08-07）

**上下文**：颜色散落在 6+ 条独立色源（colors.ts 32 token、xterm 22 色、oneDark、dockview --dv-*、Allotment、App.css --sl-fg-*、库默认色），改色多点人工同步且有漏网点（数值重复不联动、死 token、1 处真违规硬编码）。需求：全部应用可控色源收拢单点 + 每色注释消费位置 + 为新增方案与一键切换留扩展接口。

**决策**：
- src/theme/ 下设 schemes/（ColorScheme 四段：ui/terminal/editor/libraries + darcula 内置方案）+ SchemeRegistry 模块级单例（项目注册表惯例第 6 例）。
- colors.ts 改 facade：31 个同名导出值代理 active 方案，369 处消费点零改动。
- 方案切换 = settings.json `colorScheme` 段手编 + 重载窗口生效；main.tsx 启动时 React 挂载前解析（App 改动态 import 保证 facade 求值晚于 setActive）。
- 三方库色经 overrides 通道：dockview --dv-* 与 Allotment 变量内联注入挂载点；oneDark 作 editor.theme 引用 + lint/search/背景 token 化覆盖。
- 零视觉变化：所有覆盖值取现行有效值；死配置全清（3 死 token + 2 零消费 CSS 变量 + 1 违规收敛）。

**被否决的备选**：
- 运行期即时切换（token 全面响应式）：369 处常量消费 + xterm/CM 创建期消费需全量改造，代价 vs 暗色系低频切换收益不成比例。
- oneDark 完全 token 化自绘语法色板：每方案需 10+ 语法色定义，工作量与审美风险大；editor 段引用已预留未来自定义。
- 启动链 fail-safe 收编：index.html/tauri.conf.json 为静态层无法用 TS token，保持硬编码 + 注释交叉引用。

**后果**：
- 新增方案 = schemes/ 新文件 + register 一行，消费方/测试守卫零改动。
- 注释单点在 types.ts 接口槽位（消费位置与方案无关），新方案零注释负担。
- main.tsx 静态 import 图收敛为 react/react-dom/lib/e2eEnabled；E2E helpers 与 ROOT_CSS_VARS 注入保持原相对顺序。
```

## 9. 边界与豁免

### 9.1 不纳入方案的色源（显式记录）

| 色源 | 处置 |
|------|------|
| 启动链 fail-safe（index.html:10 `#1e1e2e`、tauri.conf.json:20、main.tsx:31 `#1e1e1e`/`#f44747`） | 保持硬编码；darcula.ts 文件头注释交叉引用，改 ui 对应值时手动同步 |
| OSC 8 链接色 | **不可配置**（xterm 6.x ITheme 无 link 键，勘误 1） |
| xterm IME composition-view（`#000`/`#FFF`）、viewport 衬底（`#000`） | 库内部默认，不入方案 |
| CM placeholder `#888`（@codemirror/view baseTheme） | 库默认，不入方案 |
| dockview tab group 色（未用 API）、`.dv-debug` 调试色、透明类变量 | 不入方案 |
| emoji / ANSI 内容色 | 内容层，不可配置 |
| WebView2 原生层（滚动条/::selection/focus ring/placeholder/右键菜单/表单控件） | `color-scheme: dark` 驱动，不可配置 |

### 9.2 inventory 勘误（实施时同步修正文档）

1. §6.1「改色需在 terminal/theme.ts 显式配置 theme.link」——**失实**：xterm 6.x `ITheme` 无 `link` 键（`node_modules/@xterm/xterm/typings/xterm.d.ts:372-445` 全键列表一手确认）
2. §6.1 xterm 滚动条「运行期算法计算」——结论正确但遗漏：`ITheme.scrollbarSliderBackground/Hover/Active` 三键可显式覆盖，本重构已显式化

## 10. 验收标准

1. **零视觉变化**：所有覆盖值 = 现行有效值（§4.2–4.5 逐值）；重构前后主界面截图对比一致
2. 静态门禁：`npx tsc --noEmit` + `npx eslint src/` 绿（无 Rust 改动，免 clippy/fmt）
3. `npm test`（L2）全绿 + `npm run test:l3`（L3）绿
4. **降级冒烟**：settings.json 写 `colorScheme: "不存在"` → 启动回退 darcula + console.warn，应用正常
5. **切换链路冒烟**：临时注册改动单色的测试方案 → settings.json 指向 → 重载 → 五通道（token 区 / dockview 页签 / Allotment 分割线 / CM 背景 / xterm）全部生效；验证后删除临时方案
6. `npm run e2e` 关键路径冒烟（启动序列重排属关键路径）

## 11. 实施步骤（有序）

1. `schemes/types.ts` + `schemes/darcula.ts` + `schemes/index.ts` + `schemeRegistry.ts`（新结构落地，旧 colors.ts 不动，全程可编译）
2. `colors.ts` facade 化 + `overrides.ts` + `index.ts` barrel（值来源切换，colors.test.ts 应仍绿——值未变，仅 DROPDOWN_BG 等删除项此时同步改测试）
3. 消费点 6 文件（oneDark 4 处 / JsonMode 违规 / PageDockviewHost / Workspace / terminal theme adapter）
4. `main.tsx` + `App.tsx` + `App.css`（启动序列）
5. 测试同步（§7.1 修改 + §7.2 新增）
6. 文档同步（§8 全部）
7. 验收（§10 六项）

## 12. 风险与对策

| 风险 | 对策 |
|------|------|
| main.tsx 动态 import 化改变模块求值顺序，E2E helpers/启动时序回归 | 保持 ①→⑤ 相对顺序与现状等价；L4 冒烟验证 |
| App.css/dockview.css 加载顺序翻转致同优先级规则失效 | App.css import 挪入 App.tsx 紧随 dockview.css，顺序逐一对齐现状 |
| 库覆盖值抄错导致视觉漂移 | 逐值对照 inventory §5；colors.test.ts / theme-options L3 字面量守卫兜底；截图对比验收 |
| oneDark 包 mock 失效 | darcula.ts 直 import 包透出优先；失效则两测试改 mock theme facade |
| `terminalOptions.theme` 多 3 滚动条键使键集合断言变红 | theme.test.ts / L3 theme-options 实施时验证同步 |
| 测试环境 setActive 从未调用 | registry 默认 active = darcula，colors.test.ts 等直接 import 即得正确值 |
