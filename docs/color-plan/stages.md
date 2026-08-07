# 配色系统重构 Stage 划分（stages）

> 真值源：`docs/color-plan/checklist.md`（34 项 ID）。Stage 镜像 spec §11 七段执行序。
> Stage 内文件零重叠（分工表证明）；Stage 间串行 + 每 Stage commit。
> 验证项为机械断言纲要，执行期完整断言见 `workflows/verify/stage-NN.md`（唯一真值源）。

## 跨边界契约（写死，两边 agent 不各自推断）

### C1 — Stage 01 → 02 契约：facade 31 导出名精确清单

facade 化后 `src/theme/colors.ts` 导出**恰 31 个**，名称集合如下（逐字，不多不少）：

- 组 5 个：`GIT_FILE_COLORS`（7 键）、`GIT_GUTTER_COLORS`（3 键，删 whitespaceOnly）、`EXPLORER_COLORS`（5 键，删 selected）、`SIDEBAR_COLORS`（8 键，**selected 保留**）、`AGENT_STATUS_USAGE_COLORS`（3 键）
- ERROR_BANNER 标量 3 个：`ERROR_BANNER_BG`、`ERROR_BANNER_BORDER`、`ERROR_BANNER_FG`
- 其他标量 22 个：`PANEL_BG`、`SIDEBAR_BG`、`SECONDARY_BG`、`APP_BG`、`APP_BG_PRIMARY`、`EDITOR_BG`、`SIDEBAR_FG`、`ERROR_FG`、`PLACEHOLDER_FG`、`BUTTON_FG`、`DIM_FG`、`INPUT_BG`、`INPUT_BORDER`、`FOCUS_BORDER`、`ACTIVE_SELECTION_BG`、`EXPLORER_SELECTION_BG`、`SEPARATOR_BG`、`CONTEXT_MENU_BORDER`、`SHADOW_MENU`、`HTML_PANEL_LOADING_FG`、`HTML_PANEL_IFRAME_BG`、`ON_ACCENT_FG`（新增，值 `"#FFFFFF"`——对齐 JsonMode.tsx:213 现值，保 D1 零视觉变化）
- CSS 变量桥 1 个：`ROOT_CSS_VARS`（键集合恰 `{ "--sl-bg-primary", "--sl-fg-primary" }`）

删除：`DROPDOWN_BG`、`APP_BG_SECONDARY`、`GIT_GUTTER_COLORS.whitespaceOnly`、`EXPLORER_COLORS.selected`、`ROOT_CSS_VARS["--sl-bg-secondary"]`。

### C2 — Stage 01 → 03 契约：overrides.ts 四导出签名

```ts
dockviewVarStyle(): Record<string, string>        // 20 条 dockview CSS 变量（键如 "--dv-group-view-background-color"）→ 值取 active 方案 libraries.dockview
allotmentVarStyle(): Record<string, string>       // 2 键（"--sash-size" 不动；色键为 sash 色/拖动色，照 spec §4.5 键名）
editorTheme: Extension                            // = getActive().editor.theme（darcula = oneDark 透出）
editorColorOverrides(): Extension                 // getActive().editor.overrides → EditorView.theme({...})（lint 7/searchMatch 4/background）
```

消费方式统一 `import { dockviewVarStyle, allotmentVarStyle, editorTheme, editorColorOverrides } from "../../theme"`（经 barrel；相对层级按消费文件现状）。

### C3 — Stage 01/02 → 03 契约：ColorScheme 接口四段形状（签名级）

```ts
interface ColorScheme {
  id: string; label: string;
  ui: UiTokens;              // 6 组（gitFile/gitGutter/explorer/sidebar/agentStatusUsage/errorBanner）+ 标量槽位（含 ON_ACCENT_FG）
  terminal: TerminalPalette; // 25 键（清单照 spec §4.3），结构兼容 xterm ITheme
  editor: EditorScheme;      // { theme: Extension; overrides: {...} }，darcula.theme = oneDark 直 import 透出
  libraries: LibraryOverrides; // { dockview: 20 条映射; allotment: 2 键 }
}
```

终端 adapter 形状（FAC-03）：`theme: { ...schemeRegistry.getActive().terminal }` 展开进 `terminalOptions.theme`。

---

## Stage 01 — 方案骨架（SCH-01~05，新建 5 文件）

**分工表**（文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-schemes | SCH-01/02/03 | `src/theme/schemes/types.ts`、`src/theme/schemes/darcula.ts`、`src/theme/schemes/index.ts`（全新建） |
| A2-registry | SCH-04/05 | `src/theme/schemeRegistry.ts`、`src/theme/overrides.ts`（全新建） |

**实现要点**：
- A1：types.ts 每槽位 JSDoc 消费注释（D8）；darcula.ts 全量值照 spec §4.2–4.5 搬运（ui 值 = colors.ts 现状值逐字，terminal 值 = panels/terminal/theme.ts 现状 theme 段逐字，dockview 值 = dockview-theme-dark 现状解析值照 spec §4.5）——**D1 零视觉变化：值一律搬运现状，禁止新造**；文件头注释交叉引用 fail-safe 三处（index.html:10 / tauri.conf.json:20 / main.tsx:31）。
- A2：schemeRegistry 照项目注册表先例（TabTitleRegistry/SideViewRegistry）模块级单例 + `_reset()` 仅测试；setActive 未知 id → 回退 `getDefaultId()`（`"darcula"`）+ `console.warn`。
- 两 agent 并行只写新文件，**不 import 彼此产物以外的既有文件改动为零**；A1 的 schemes/index.ts import schemeRegistry（A2 产物）——跨 agent 编译依赖，tsc 门禁在合并后统一跑（分工允许：A2 先落接口形状已在 C3 写死）。

**验证项**（纲要）：5 文件存在且 tsc 绿；types.ts 含 5 接口 + JSDoc；darcula 四段键数（ui 6 组 7/3/5/8/3/3 + 23 标量含 ON_ACCENT_FG、terminal 25、libraries dockview 20 + allotment 2）；registry 七方法签名；overrides 四导出签名合 C2。

**门禁**：`npx tsc --noEmit` + `npx eslint src/`。
**commit**：`refactor(theme): 配色方案系统骨架——schemes/ + SchemeRegistry + overrides`

---

## Stage 02 — facade 切换（FAC-01/02 + TST-01）

**分工表**（单 agent——三文件强耦合，导出清单与测试断言必须同视野）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-facade | FAC-01/02、TST-01 | `src/theme/colors.ts`、`src/theme/index.ts`、`src/__tests__/colors.test.ts` |

**实现要点**：
- colors.ts 全文重写为 facade：31 导出（C1 清单逐字）值代理 `schemeRegistry.getActive()`；各组/标量用 getter 或模块求值（照 spec §4.7 定的实现形态）；文件头注释更新为 facade 语义 + 例外说明。
- index.ts：31 导出同步 + 追加 `schemeRegistry`/`./schemes`/`./overrides` re-export。
- colors.test.ts 六处同步（TST-01 逐条）。
- **本 Stage 不动 main.tsx**——ROOT_CSS_VARS 求值时机风险由 Stage 04 解决；facade 在 import 时按默认 darcula 求值，与现状视觉一致（D1）。

**验证项**：colors.ts 导出名集合 = C1 恰 31；无 `DROPDOWN_BG`/`APP_BG_SECONDARY`/`whitespaceOnly`/`--sl-bg-secondary` 残留（全仓 grep）；`SIDEBAR_COLORS.selected` 仍在；colors.test.ts 六处改完且 L2 该文件绿。

**门禁**：tsc + eslint + `npm test`。
**commit**：`refactor(theme): colors.ts facade 化——31 导出代理 active 方案 + 死配置清理`

---

## Stage 03 — 消费点迁移（FAC-03 + CON-01~06）

**分工表**（文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-cm | CON-01/02/03/04 | `src/panels/editor/useCodeMirror.ts`、`src/panels/gitshow/GitShowPanel.tsx`、`src/panels/diff/DiffPanel.tsx`、`src/panels/hooksConfig/JsonMode.tsx` |
| A2-shell | CON-05/06、FAC-03 | `src/workspace/PageDockviewHost.tsx`、`src/workspace/Workspace.tsx`、`src/panels/terminal/theme.ts` |

**实现要点**：
- A1：四处 oneDark → `editorTheme, editorColorOverrides()`（C2 签名，经 barrel import）；删四处 `@codemirror/theme-one-dark` import；JsonMode.tsx:213 事件导航 hover 色 `"#FFFFFF"` → `ON_ACCENT_FG`（**语义式要求：须引用 token，不限实现写法**）。
- A2：PageDockviewHost.tsx:369 根 div `style={{ ...dockviewVarStyle() }}`（className="dockview-theme-dark" 保留）；Workspace.tsx 根容器 style 合并 `...allotmentVarStyle()`（**SideBarArea.tsx 不改**——CSS 变量继承覆盖 :224 外层 + SideBarArea.tsx:72 内层两处 Allotment）；terminal/theme.ts adapter（C3），非色选项原位保留。
- **预期零改动声明**：`theme.test.ts`/`gitshow-panel.test.tsx`/`hooks-config-jsonmode.test.tsx`/L3 `theme-options.test.ts` 不应失效（值未变，仅来源换）；失效即停，查值漂移而非改测试。

**验证项**：全仓 grep 无 `@codemirror/theme-one-dark` import 残余（仅 darcula.ts 一处保留）；四处消费点含 `editorColorOverrides`；PageDockviewHost/Workspace 含对应 VarStyle 展开；JsonMode 无 `#FFFFFF` 字面量（语义式核查）；terminal/theme.ts 无 25 键色值字面量残留；L2+L3 绿。

**门禁**：tsc + eslint + `npm test` + `npm run test:l3`。
**commit**：`refactor(theme): 消费点迁移——oneDark 四处/dockview/allotment/终端 adapter/JsonMode 违规收敛`

---

## Stage 04 — 启动序列（BOOT-01~03）

**分工表**（单 agent——启动链时序强耦合，四文件同视野）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-boot | BOOT-01/02/03 | `src/main.tsx`、`src/App.tsx`、`src/App.css`、`src/__tests__/bootstrap.test.ts` |

**实现要点**：
- main.tsx 目标态（修正项 2，执行期再修正见 checklist 修正记录 4）：静态 import 恰 2 个（react、react-dom/client）+ E2E 门控内联字面量（`import.meta.env.DEV || import.meta.env.VITE_E2E === "1"`，rolldown 不折叠跨模块常量，引用 E2E_ENABLED 常量会残留 helpers chunk 生产 dist——A/B 实证）；序列 ①IPC wait+fail-safe 不变 → ②loadSettings().catch(()=>null) + 动态 import schemeRegistry/schemes + setActive → ③动态 import theme + ROOT_CSS_VARS 注入 → ④E2E helpers 逻辑原位（仅随链）→ ⑤await import("./App") + render。**E2E 时序不变量：helpers 注入在 setActive 之后**。
- bootstrap.test.ts 适配（修正项 1）：补 `../ipc/settings` mock（loadSettings resolve null），防真实 invoke；`../App`/`../App.css` mock 保留。
- App.tsx:23 dockview.css import 后追加 `import "./App.css"`（CSS 顺序：dockview.css 先）。
- App.css 删 :5-7，var() 引用不动。
- **vite build 门禁**：动态 import 图属构建级行为，tsc 覆盖不到，必须 `npx vite build` 验证产物图。

**验证项**：main.tsx 静态 import 恰 3 个（修正项 2 目标态）；无 `from "./lib"`（barrel）与 `from "./theme"` 静态 import；setActive 调用在 ROOT_CSS_VARS 注入与 E2E helpers 之前（Read 顺序断言）；bootstrap.test.ts 含 settings mock 且 L2 绿；`npx vite build` 成功。

**人工验证点**：E2E helpers 时序（④在 setActive 之后——jsdom 无法验证真实 WebView2 时序，L4 兜底）；CSS 加载顺序（dockview.css 先 App.css 后——构建产物内顺序肉眼抽查）。

**门禁**：tsc + eslint + `npm test` + `npx vite build`。
**commit**：`refactor(theme): 启动序列——main.tsx 动态 import 链 + App.css 归位`

---

## Stage 05 — 测试补全（TST-02~05）

**分工表**（文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-registry-test | TST-02 | `src/__tests__/scheme-registry.test.ts`（新建） |
| A2-overrides-test | TST-03 | `src/__tests__/overrides.test.ts`（新建） |
| 收尾（串行） | TST-04/05 | TST-04 失效才触的四文件 + `.claude/test-inventory.md` |

**实现要点**：
- A1/A2 并行写新测试（_registry/_overrides 产物已存在，接口稳定）；`_reset()` 隔离模式照 tab-title-registry.test.ts 先例。
- 收尾 agent：TST-04 四文件逐一跑绿确认（**预期零改动**；失效须记录理由再改）；TST-05 登记两新文件用例数 + colors.test.ts 实际用例数。

**验证项**：两新测试文件存在且用例数达标（registry ≥12、overrides ≥5）；L2 全绿；inventory 中两新文件已登记且 colors.test.ts 计数与实际一致（grep `^ok` 计数核对）。

**门禁**：`npm test`（含新文件）。
**commit**：`test(theme): scheme-registry/overrides 测试新增 + test-inventory 同步`

---

## Stage 06 — 文档同步（DOC-01~06）

**分工表**（文件零重叠）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A1-glossary | DOC-01/02 | `CONTEXT.md`、`.claude/adr.md` |
| A2-claudemd | DOC-03/04/05 | `.claude/CLAUDE.md`、`src/theme/CLAUDE.md`、`src/panels/CLAUDE.md` |
| A3-colordocs | DOC-06 | `docs/color-implementation.md`、`docs/color-inventory.md` |

**实现要点**：
- A2：根 CLAUDE.md 硬约束 #6 改写 + 模块索引 src/theme 行；src/theme/CLAUDE.md 重写（删「既定例外」「无独立测试文件」两过时句）；panels/CLAUDE.md 仅改例外句一处（surgical）。
- A3：两文档反映新架构现状；删「临时摸底」注记；§9.2 两勘误回写。
- **只改文档，禁动任何代码**。

**验证项**：grep 断言——根 CLAUDE.md 无旧 #6 措辞「所有颜色只在 theme/colors.ts 定义为 token」；theme/CLAUDE.md 无「既定例外」「无独立测试文件」；adr.md 含 ADR-0002；CONTEXT.md 含 4 新术语；两 color 文档无「临时摸底」；markdown 语法抽查（标题层级/表格闭合）。

**门禁**：无编译门禁——verify grep 断言承担 + markdown 语法检查。
**commit**：`docs(theme): 配色方案文档同步——CONTEXT/ADR/CLAUDE.md/color 两文档`

---

## Stage 07 — 验收（ACC-01~06）

**分工表**（串行单 agent + 三项人工）：

| label | 负责项 | 说明 |
|-------|--------|------|
| A1-accept | ACC-01/02/06 | 静态门禁 + L2/L3 全绿 + `npm run wdio` 关键路径 |
| 人工 | ACC-03/04/05 | 见下「人工验证点」 |

**实现要点**：ACC-06 前须 `npm run build:e2e`（VITE_E2E=1 门控）；wdio 跑 terminal/editor 关键 spec。

**人工验证点**（收尾实测项，逐条签字）：
1. **ACC-03 零视觉截图对比**：重构前后主界面截图逐项一致（终端/编辑器/diff/侧栏/活动栏/dockview 页签/allotment sash）——D1 承诺的最终判定。
2. **ACC-04 降级冒烟**：settings.json 写 `colorScheme: "不存在"` → 回退 darcula + console.warn。
3. **ACC-05 五通道切换冒烟**：临时注册改单色方案 → 指向 → 重载 → 五通道（React style/xterm/CM6/dockview 变量/allotment 变量）全生效 → 还原。

**门禁**：spec §10 六项全过。
**commit**：无（验收 Stage 不 commit）。

---

## 人工验证点汇总（固定段）

| Stage | 验证点 | 原因 |
|-------|--------|------|
| 04 | E2E helpers 时序 = ④在 setActive 之后 | jsdom 无法验证真实 WebView2 启动时序，L4/人工兜底 |
| 04 | CSS 加载顺序 = dockview.css 先、App.css 后 | 构建产物内顺序非静态断言可证 |
| 07 | ACC-03/04/05 三项 | 视觉一致性/运行时降级/五通道生效不可自动化 |

## 偏离豁免登记

- **Stage 02/04 单 agent**：强耦合（导出清单与测试断言/启动链时序须同视野），并行拆分反而引入跨 agent 契约风险——划分理由即豁免说明。
- **Stage 06 无编译门禁**：纯文档 Stage，verify grep 断言 + markdown 语法检查承担。
- **Stage 07 无 commit**：验收不产生代码变更。
