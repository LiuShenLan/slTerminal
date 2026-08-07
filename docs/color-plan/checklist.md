# 配色系统重构清单（checklist）

> 真值源：`docs/color-scheme-refactor-spec.md`（决策 D1-D8，唯一执行依据）。
> 组织方式说明：本清单**不用 P0-P4 优先级**——优先级由 Stage 依赖顺序表达（见 `stages.md`）；按模块前缀分组（SCH/FAC/CON/BOOT/TST/DOC/ACC）。
> 每项含：ID、位置（文件+行号，行号为重构前快照）、修复要点、溯源。
> spec 引用已经 plan 期逐条一手核实；三处修正见文末「修正记录」。

## SCH — 方案骨架（新建，spec §6.1）

- **SCH-01**：新建 `src/theme/schemes/types.ts`——`ColorScheme`/`UiTokens`/`TerminalPalette`/`EditorScheme`/`LibraryOverrides` 接口定义；**每个槽位带 JSDoc 消费注释**（说明该色在前端何处起作用，决策 D8 区域级注释于接口）。溯源：spec §4.1–§4.5、D8。
- **SCH-02**：新建 `src/theme/schemes/darcula.ts`——darcula 方案全量值：① ui 段 6 组（gitFile 7 / gitGutter 3 / explorer 5 / sidebar 8 / agentStatusUsage 3 / errorBanner 3）+ 23 标量；② terminal 段 25 键（16 色 + foreground/background/cursor/cursorAccent/selectionBackground + black 系等，照 spec §4.3 键表）；③ editor 段 = oneDark **直 import 透出**（`import { oneDark } from "@codemirror/theme-one-dark"`）+ overrides（lint 7 键 / searchMatch 4 键 / background）；④ libraries 段 = dockview 20 条 CSS 变量 + allotment 2 键（sash 色/拖动色）。**文件头注释交叉引用启动链 fail-safe 三处**（`index.html:10`、`src-tauri/tauri.conf.json:20`、`src/main.tsx:31` 的 `#1e1e1e`/`#1e1e2e`——它们先于方案加载，不随方案切换）。溯源：spec §4.2–§4.5、D1（全部应用可控色源）、D6（oneDark 作 editor 段引用）。
- **SCH-03**：新建 `src/theme/schemes/index.ts`——side-effect 注册文件：`schemeRegistry.register(darcula)`（照 `tabRules.ts`/`sideViewDefs.ts` 模式）；后续新增方案在此追加 import + register。溯源：spec §4.6。
- **SCH-04**：新建 `src/theme/schemeRegistry.ts`——`SchemeRegistry` 模块级单例（项目第 6 个注册表单例），七方法 API：`register(scheme)` / `get(id)` / `getActive()` / `setActive(id)`（未知 id → 回退 darcula + `console.warn`）/ `getAll()` / `getDefaultId()` / `_reset()`（仅测试）。溯源：spec §4.6、D2（切换+重载窗口）。
- **SCH-05**：新建 `src/theme/overrides.ts`——四导出（spec §4.8）：`dockviewVarStyle()`（dockview 20 条变量 → React style 对象，键为 CSS 变量名 camelCase 化）、`allotmentVarStyle()`（allotment 2 键 → style 对象）、`editorTheme`（= active 方案 editor 段 oneDark 透出）、`editorColorOverrides()`（active 方案 editor.overrides → CM6 `EditorView.theme` 扩展）。溯源：spec §4.8。

## FAC — facade 切换（spec §6.2）

- **FAC-01**：`src/theme/colors.ts` facade 化（spec §4.7）——**31 导出名不变、值改为代理** `schemeRegistry.getActive()`（D5：369 消费点零改动）。同步死配置清理（D7）：删 `DROPDOWN_BG`（:48 `"#2A2D2E"`）、`APP_BG_SECONDARY`（:51 `"#2b2b3c"`）、`GIT_GUTTER_COLORS.whitespaceOnly`（:26 `"#4C4638"`）、`EXPLORER_COLORS.selected`（:35 `"#37373D"`——**注意只删 EXPLORER_COLORS 内这处；`SIDEBAR_COLORS.selected`（:81）保留**，它是 sidebar 组 8 键之一）；增标量 `ON_ACCENT_FG`（accent 底色上的前景色，供 JsonMode 等场景）。`ROOT_CSS_VARS`（:110-112）键集合改为 `{ "--sl-bg-primary", "--sl-fg-primary" }`（删 `--sl-bg-secondary`，增 `--sl-fg-primary` = `#cdd6f4`，对齐 `App.css:6` 现状值）。溯源：spec §4.7、§9.2（死配置清单）、D7。
- **FAC-02**：`src/theme/index.ts` barrel——32 导出 → 31（随 FAC-01 删 2 增 1）；追加 `schemeRegistry`（含 `SchemeRegistry` 类型）、`./schemes`（ColorScheme 等类型 + darcula）、`./overrides` 四导出的 re-export。溯源：spec §6.2。
- **FAC-03**：`src/panels/terminal/theme.ts` adapter——`theme: { ...schemeRegistry.getActive().terminal }`（25 键展开进 xterm `ITheme`）；非色选项（`drawBoldTextInBrightColors`、`vtExtensions.kittyKeyboard`、scrollback 等）**原位保留不动**；文件头注释更新为「经 adapter 映射 active 方案 terminal 段，非独立主题定义」。溯源：spec §6.3 终端行。

## CON — 消费点迁移（spec §6.3）

- **CON-01**：`src/panels/editor/useCodeMirror.ts`——:289 `oneDark` → `editorTheme, editorColorOverrides()`（经 `src/theme/overrides.ts`，扩展数组中替换）；删 :20 `import { oneDark } from "@codemirror/theme-one-dark"`。溯源：spec §6.3。
- **CON-02**：`src/panels/gitshow/GitShowPanel.tsx`——:143 同 CON-01；删 :12 import。溯源：spec §6.3。
- **CON-03**：`src/panels/diff/DiffPanel.tsx`——:521/:566 两处同 CON-01；删 :25 import。溯源：spec §6.3。
- **CON-04**：`src/panels/hooksConfig/JsonMode.tsx`——:160 同 CON-01（删 :25 import）；**:213 `e.currentTarget.style.color = "#FFFFFF"` 硬编码 → `ON_ACCENT_FG` token**（硬约束 #6 违规收敛，违规名不限——须引用 token 而非任何字面量）。溯源：spec §6.3、§9.2。
- **CON-05**：`src/workspace/PageDockviewHost.tsx`——:369 `className="dockview-theme-dark"` 的根 div `style` 展开 `dockviewVarStyle()`（20 条 dockview CSS 变量内联注入，替代主题类暗色常量）；className 保留（布局样式仍由 dockview.css 提供）。溯源：spec §4.5/§6.3。
- **CON-06**：`src/workspace/Workspace.tsx`——根容器 style 合并 `allotmentVarStyle()`（2 键 CSS 变量）——CSS 变量继承同时覆盖 :224 外层 Allotment 与 `SideBarArea.tsx:72` 内层 Allotment 两处，**`SideBarArea.tsx` 本身不改**。溯源：spec §4.5/§6.3。

## BOOT — 启动序列（spec §5/§6.4）

- **BOOT-01**：`src/main.tsx` 启动序列重排（spec §5）——静态 import 由现状 6 个（react、react-dom/client、./App、./App.css、./lib、./theme）收敛为 3 个（react、react-dom/client、**`./lib/e2eEnabled` 深导入**——`./lib` barrel → ErrorBoundary:10-13 → theme 会在 setActive 前求值 facade，故必须绕开 barrel）。序列：① IPC wait + fail-safe（:29-33 不变）→ ② `loadSettings().catch(() => null)` + 动态 `import("./theme/schemeRegistry")` + `import("./theme/schemes")` + `setActive(settings?.colorScheme)` → ③ 动态 `import("./theme")` 取 `ROOT_CSS_VARS` 注入 → ④ E2E helpers（:42-45 逻辑不变，仅位置随链）→ ⑤ `await import("./App")` + render。**含 `src/__tests__/bootstrap.test.ts` 适配**（修正项 1：补 `../ipc/settings` mock，`loadSettings` resolve null，防真实 invoke）。溯源：spec §5、§6.4。
- **BOOT-02**：`src/App.tsx`——:23 `import "dockview-react/dist/styles/dockview.css"` 之后追加 `import "./App.css"`（App.css 从 main.tsx 移此，CSS 顺序：dockview.css 先、App.css 后）。溯源：spec §5/§6.4。
- **BOOT-03**：`src/App.css`——删 :5-7（注释行 + `--sl-fg-primary: #cdd6f4;` + `--sl-fg-secondary: #a6adc8;` 两处 hex 定义，改由 ROOT_CSS_VARS 注入 `--sl-fg-primary`；`--sl-fg-secondary` 死配置不复活）；var() 引用（:15/:16/:36/:37）保留不动。溯源：spec §6.4、§9.2。

## TST — 测试（spec §7）

- **TST-01**：`src/__tests__/colors.test.ts` 六处同步（spec §7.1 表）：① import 块删 `DROPDOWN_BG`/`APP_BG_SECONDARY` 增 `ON_ACCENT_FG`；② GIT_GUTTER_COLORS 4→3 键（删 whitespaceOnly case）；③ EXPLORER_COLORS 6→5 键（删 selected case）；④ 通用 UI 色标量删 2 增 ON_ACCENT_FG（计数断言 25→24）；⑤ ROOT_CSS_VARS describe 键集合 = `{--sl-bg-primary, --sl-fg-primary}`（len 2 不变）：删 `--sl-bg-secondary` 两条断言（toContain + 值断言），增 `--sl-fg-primary` 值 `#cdd6f4` 断言；⑥ 文件头注释（:7 提及的键集合描述）更新。溯源：spec §7.1。
- **TST-02**：新建 `src/__tests__/scheme-registry.test.ts`（~15 用例，spec §7.2）：register/get/getAll/getDefaultId、setActive 已知 id、setActive 未知 id 回退 darcula + console.warn、getActive 默认 darcula、重复注册覆盖、`_reset` 隔离、darcula 四段完整性（ui 6 组键数 7/3/5/8/3/3 + 23 标量、terminal 25 键、editor oneDark 透出非 undefined、libraries dockview 20 条 + allotment 2 键）。溯源：spec §7.2。
- **TST-03**：新建 `src/__tests__/overrides.test.ts`（~6 用例，spec §7.2）：dockviewVarStyle 键集合 20 条且值为 active 方案色、allotmentVarStyle 2 键、editorTheme === active 方案 editor 段、editorColorOverrides 返回 CM6 扩展（lint/searchMatch/background 键生效）、setActive 后输出跟随切换。溯源：spec §7.2。
- **TST-04**：四文件预期零改动验证——`theme.test.ts`（13 例）、`main-bootstrap.test.tsx`（1 例）、`gitshow-panel.test.tsx` + `hooks-config-jsonmode.test.tsx` 的 oneDark mock、`test/terminal/theme-options.test.ts`（L3，5 例）——**预期零改动**；逐一跑绿确认，失效才改（改动须记录理由）。`bootstrap.test.ts` 适配已并入 BOOT-01（修正项 1），不在本项。溯源：spec §7.1 波及面表。
- **TST-05**：`.claude/test-inventory.md` 同步——登记 `scheme-registry.test.ts`/`overrides.test.ts` 两新文件及用例数；`colors.test.ts` 用例数 89 → 更新为实际值。溯源：项目硬规则「用例清单同步」。

## DOC — 文档同步（spec §8，固定最后代码 Stage 之后）

- **DOC-01**：`CONTEXT.md` +4 术语（spec §3 表）：配色 token、配色方案（ColorScheme）、方案注册表（SchemeRegistry）、启动链 fail-safe 色。溯源：spec §8。
- **DOC-02**：`.claude/adr.md` +ADR-0002（spec §8.1 全文：配色方案系统——schemes/ + SchemeRegistry + facade 代理 + overrides 导出；含 D1-D8 决策依据）。溯源：spec §8。
- **DOC-03**：根 `.claude/CLAUDE.md`——硬约束 #6 改写（「配色单点：所有颜色只在 `theme/colors.ts` 定义为 token」→「配色单点：颜色定义于 `theme/schemes/<scheme>.ts`，组件经 `theme/colors.ts` facade token 引用，禁止硬编码」）；模块索引 src/theme 行职责更新（配色 token 单点 → 配色方案系统 + facade）。溯源：spec §8。
- **DOC-04**：`src/theme/CLAUDE.md` 重写（spec §8 表）：职责（配色方案系统单点）、架构决策（schemes/Registry/facade/overrides 四件 + 启动链时序 + 终端 adapter 例外的新表述）、文件表（+schemes/3 文件 + schemeRegistry.ts + overrides.ts）、新增方案步骤、测试模式（删「无独立测试文件」过时声明 + 删「既定例外」句）。溯源：spec §8。
- **DOC-05**：`src/panels/CLAUDE.md` 例外句改——硬约束 #6 的「终端配色是历史遗留的独立主题定义」→「终端配色经 `panels/terminal/theme.ts` adapter 映射 active 方案 terminal 段」。溯源：spec §8。
- **DOC-06**：`docs/color-implementation.md` + `docs/color-inventory.md` 更新——反映新架构现状；删「临时摸底」注记；落实 spec §9.2 两勘误（死配置清理结果回写）。溯源：spec §8/§9.2。

## ACC — 验收（spec §10）

- **ACC-01**：静态门禁全绿——`npx tsc --noEmit` + `npx eslint src/`。溯源：spec §10。
- **ACC-02**：`npm test`（L2）+ `npm run test:l3`（L3）全绿。溯源：spec §10。
- **ACC-03**：**零视觉截图对比（人工）**——重构前后主界面截图逐项一致（终端/编辑器/diff/侧栏/活动栏/dockview 页签/allotment sash）。溯源：spec §10、D1「零视觉变化」承诺。
- **ACC-04**：**降级冒烟（人工）**——`~/.slterminal/settings.json` 写 `colorScheme: "不存在"` → 启动回退 darcula + console.warn，界面正常。溯源：spec §4.6/§10。
- **ACC-05**：**五通道切换冒烟（人工）**——临时注册一个改单色的测试方案 → settings 指向它 → 重载窗口 → 五通道（React inline style / xterm ITheme / CM6 theme / dockview CSS 变量 / allotment CSS 变量）全部生效 → 删除临时方案还原。溯源：spec §4.5/§10、D2。
- **ACC-06**：L4 e2e 冒烟——`npm run wdio` 关键路径（启动/终端/编辑器）通过。溯源：spec §10。

## 修正记录（plan 期事实核查对 spec 的三处修正，已吸收进上表）

1. **`src/__tests__/bootstrap.test.ts`（3 用例）spec §7.1 漏列**：直接测 main.tsx（mock `../App` + `../App.css`），main.tsx 改动态 import 后须补 `../ipc/settings` mock（`loadSettings` resolve null）。属 BOOT-01 波及面，**随 Stage 04 完成**（Stage 04 门禁含 L2，不留 Stage 05）。
2. **spec §5 静态 import 描述是目标态措辞**：现状 6 个（react、react-dom/client、./App、./App.css、./lib、./theme），目标态 3 个（react、react-dom/client、`./lib/e2eEnabled` 深导入）。verify 断言按目标态写。
3. **git add 枚举补 `CONTEXT.md` 与 `.claude/adr.md`**：config.json `gitAddPaths` 白名单未含此两文件，execution-plan.md 中 Stage 06 精确路径正当扩展。
4. **BOOT-01 静态 import 目标态修正（执行期实证）**：spec §5 / stages.md / stage-04 脚本声称「静态 import 恰 3 个（含 `./lib/e2eEnabled` 深导入）」，实现时被否决——rolldown 不做跨模块常量折叠，main.tsx 引用 `E2E_ENABLED` 常量会使 `helpers-*.js` chunk 残留生产 dist（42KB，含 `installAllE2eHelpers`），CI 生产剥离守卫（ci.yml grep + e2e-build-config.test.ts）必 fail；A/B 对比实证：门控改为内联 `import.meta.env.DEV || import.meta.env.VITE_E2E === "1"`（与 e2eEnabled.ts 定义逐字一致，门控语义单点不变）后 helpers chunk 零残留、`VITE_E2E=1` 构建仍保留。**最终目标态：静态 import 恰 2 个（react、react-dom/client）+ 内联门控**，意图（静态面最小 + setActive 前不触碰 facade）达成且更彻底。**连带影响**：`src/lib/CLAUDE.md` 的「main.tsx 为 E2E_ENABLED 六站点之一」表述失效，须 Stage 06（DOC）同步修正（并入 DOC-03 范围）。
