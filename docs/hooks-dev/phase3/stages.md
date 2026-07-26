# Phase 3 Stage 划分 — Hooks 双模式配置面板

> 本文件为执行期唯一 Stage 真值源。各 Stage 脚本 `workflows/stage-NN-*.js` 与验证断言 `workflows/verify/stage-NN.md` 均引用本文件。
> 跨边界契约（IPC 命令名/签名/DTO/事件名）写死于 contract.md C13 与本文件头部注释，并行 agent 不各自推断。

---

## 全局跨边界契约（所有 Stage 共享）

### 新增 IPC 命令

| Rust 命令 | 参数 | 返回 | 前端 wrapper |
|-----------|------|------|--------------|
| `hooks_config_read` | `layer: String`, `project_path: Option<String>` | `Result<serde_json::Value, AppError>` | `src/ipc/hooksConfig.ts` `readHooksConfig` |
| `hooks_config_write` | `layer: String`, `content: serde_json::Value`, `project_path: Option<String>` | `Result<(), AppError>` | `src/ipc/hooksConfig.ts` `writeHooksConfig` |

- `layer` 仅允许 `"user"` / `"project"` / `"local"`。
- user 层路径：`~/.claude/settings.json`；project 层：`<projectPath>/.claude/settings.json`；local 层：`<projectPath>/.claude/settings.local.json`。
- project/local 层 `project_path` 须经后端 `validate_path_within_root` 沙箱校验。

### 复用契约 C6 命令（F2 并入）

| Rust 命令 | 前端 wrapper | 用途 |
|-----------|--------------|------|
| `hooks_inject` | `src/ipc/hooks.ts` `injectHooks` | 注入 hook 脚本与配置段 |
| `hooks_uninstall` | `src/ipc/hooks.ts` `uninstallHooks` | 卸载 hook 配置与脚本 |
| `hooks_injection_status` | `src/ipc/hooks.ts` `getInjectionStatus` | 查询注入状态 |

### 面板类型名

- 目录：`src/panels/hooksConfig/`
- 面板类型常量：`PANEL_HOOKS_CONFIG = "hooksConfig"`
- 组件：`HooksConfigPanel`

### 新增全局命令

- id: `global.openHooksConfig`
- title: "打开 Hooks 配置"
- context: `global`
- defaultKey: **Ctrl+Shift+H（待执行期确认）**
- handler 通过 `window.__dockviewApi.addPanel` 打开 `hooksConfig` 面板

---

## Stage 01 — 后端三层配置读写命令

**内容**：实现 `hooks_config_read` / `hooks_config_write` 及 L1 测试。

**ID 列表**：P3-BE-01、P3-BE-02、P3-BE-03、P3-BE-04、P3-BE-05、P3-BE-06、P3-BE-07、P3-BE-08、P3-TE-01、P3-TE-02

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-config | P3-BE-01/02/03/06/07/08 + 单元测试 | `src-tauri/src/hooks/config.rs`（新建） |
| backend-register | P3-BE-04/05 | `src-tauri/src/hooks/mod.rs`、`src-tauri/src/lib.rs` |

*注：两个 agent 文件零重叠；backend-register 导入 config.rs 中的命令函数名（已写死于契约）。*

**实现要点**

- `config.rs` 中路径解析与文件 IO 全部在 `spawn_blocking` 内执行（硬约束 #3）。
- user 层使用 `dirs::home_dir()`，不经过 `validate_path_within_root`（照 `settings.rs`/`projects.rs` 先例绕过沙箱）。
- project/local 层必须校验 `project_path` 在 `project_root` 沙箱内。
- 写操作使用 `NamedTempFile::new_in()` + `persist` 实现原子写；Phase 3 明确不做 `.bak`。
- 文件不存在或 JSON 损坏时读命令返回 `Ok(Value::Null)`。

**验证项**

1. `src-tauri/src/hooks/config.rs` 存在并实现 `hooks_config_read` / `hooks_config_write`。
2. `src-tauri/src/lib.rs` 的 `generate_handler!` 包含上述两条命令。
3. L1 测试覆盖 user 层读取/写入、project/local 路径解析、沙箱失败分支、原子写。
4. `cargo clippy` 与 `cargo test` 通过。

**Commit message**

```
feat: 后端 hooks 配置三层读写命令（user/project/local）

test: L1 覆盖路径解析、沙箱、原子写
test: 用例数待 Stage 完成后回填 test-inventory

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 无。L1 测试已覆盖真实文件 IO（tempdir 隔离），本 Stage 无 UI/视觉假设。

---

## Stage 02 — IPC 封装、DTO 与 matcher 语义引擎

**内容**：前端 IPC 层、类型定义、matcher 纯函数、configModel 双向转换及 L2 测试。

**ID 列表**：P3-FE-05、P3-FE-06、P3-FE-08、P3-FE-10、P3-TE-05、P3-TE-06

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-ipc | P3-FE-05/06 | `src/ipc/hooksConfig.ts`（新建）、`src/ipc/index.ts`、`src/types/hooksConfig.ts`（新建） |
| frontend-model | P3-FE-08/10 + 测试 | `src/panels/hooksConfig/matcherEngine.ts`（新建）、`src/panels/hooksConfig/configModel.ts`（新建） |

*注：文件零重叠；`src/ipc/index.ts` 仅追加一行 re-export。*

**实现要点**

- `src/ipc/hooksConfig.ts` 是唯一调用 `invoke` 的位置（硬约束 #1）。
- `matcherEngine.ts` 必须为纯函数（无 DOM/React/IO），单点供 matcher tester 与测试共用。
- 严格按 F6 matcher 语义表实现：`FileChanged` / `StopFailure` 窄字符集强制走正则。
- `configModel.ts` 中 `jsonToGui` 对非对象/非数组输入应降级为空模型，不抛错。

**验证项**

1. `src/ipc/hooksConfig.ts` 封装 `hooks_config_read` / `hooks_config_write`，参数名 snake_case → camelCase 转换正确。
2. `matcherEngine` 全表语义测试通过（精确 OR、正则、全匹配、大小写敏感、FileChanged/StopFailure 窄字符集）。
3. `configModel` 双向转换测试覆盖空配置、多事件多 handler、字段缺失。
4. `npx tsc --noEmit` 与 `npm test` 相关测试通过。

**Commit message**

```
feat: 前端 hooks 配置 IPC 封装 + matcher 语义引擎 + 配置模型

test: L2 matcher 语义全表 + configModel 双向转换

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- matcher 语义是否与 claude 官方行为一致：F6 已规定语义，但真实 claude 的 `FileChanged` / `StopFailure` 窄字符集规则以官方文档为最终依据；如官方后续放宽，需回改 matcherEngine。

---

## Stage 03 — 面板骨架、注册与数据 hook

**内容**：创建 `src/panels/hooksConfig/` 目录、实现面板骨架、注册面板类型、实现 `useHooksConfig` 与禁用状态 store。

**ID 列表**：P3-FE-01、P3-FE-02、P3-FE-03、P3-FE-04、P3-FE-15、P3-FE-18、P3-FE-18b、P3-TE-07、P3-TE-08

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-store | P3-FE-18 + 测试 | `src/stores/hooksConfig.ts`（新建）、`src/stores/index.ts` |
| frontend-panel-core | P3-FE-01/02/03/04/15/18b + 测试 | `src/panels/hooksConfig/index.ts`、`src/panels/hooksConfig/HooksConfigPanel.tsx`、`src/panels/hooksConfig/useHooksConfig.ts`、`src/panelRegistry.ts`、`src/panels/index.ts`、`src/App.tsx`、`src/__tests__/panel-registry.test.ts` |

*注：文件零重叠；store agent 先完成，panel-core agent 后启动（sequential pipeline）。`App.tsx` 仅增加 `cancelPendingSave` 冲刷调用。*

**实现要点**

- `HooksConfigPanel.tsx` 先渲染占位 UI，顶部工具栏预留：层级切换器、GUI/JSON 模式切换、保存按钮、注入状态条。
- `useHooksConfig` 从 `useProjects` + `useLayout` 推导活跃项目 `rootPath`；rootPath 为空时 project/local 层禁用。
- 加载：Null → 空对象 `{}`；切换 layer 前若 dirty 弹窗确认；面板挂载时调用 `useHooksConfigStore.getState().loadFromDisk()` 加载禁用状态。
- `panelRegistry.ts` 追加 `hooksConfig`，`PANEL_TYPES` 同步追加；不加入 `FILE_PANEL_TYPES` / `isAlwaysRenderPanel`。
- `src/stores/hooksConfig.ts` 持久化 `disabledHooks` 段到 `~/.slterminal/settings.json`，模式照 `keybindings.ts`。
- `src/App.tsx` 关闭钩子中与其他 store 一并调用 `cancelPendingSave`（从 `src/stores/hooksConfig` 导入，重命名避免冲突）。
- 同步更新 `src/__tests__/panel-registry.test.ts`，使其预期 6 个面板键与 `PANEL_TYPES.length === 6`。

**验证项**

1. `PANEL_TYPES` 包含 `"hooksConfig"`，`isValidPanelType` 识别该类型；`panel-registry.test.ts` 已同步为 6 键/6 长度。
2. `HooksConfigPanel` 渲染三态（loading/content/error），工具栏包含层级切换器与模式切换。
3. `useHooksConfig` 在 layer 变化时调用 `readHooksConfig`，rootPath 为空时不调用 project/local 层；挂载时加载 disabledHooks。
4. `hooksConfig` store 测试通过：load/sanitize/disable/enable/debounce payload。
5. `App.tsx` 关闭序列调用 `cancelPendingSave`（hooksConfig）。

**Commit message**

```
feat: hooksConfig 面板骨架、注册与数据 hook

feat: hooksConfig store 管理禁用状态持久化
feat: App.tsx 关窗钩子冲刷 hooksConfig 待保存状态
test: L2 面板注册与 store 测试（含 panel-registry.test.ts 同步）

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 面板占位 UI 在真实布局中的视觉比例；层级切换器禁用态样式。

---

## Stage 04 — Schema 内嵌与 JSON 模式

**内容**：内嵌 SchemaStore JSON、实现 `JsonMode.tsx` 与 `MatcherTester.tsx`。

**ID 列表**：P3-FE-07、P3-FE-11、P3-TE-09、P3-TE-10

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-json-mode | P3-FE-07/11 + 测试 | `src/features/hooksConfig/schema/claude-code-settings.json`（新建）、`src/panels/hooksConfig/JsonMode.tsx`（新建）、`src/panels/hooksConfig/MatcherTester.tsx`（新建）、`package.json`（新增依赖）、`HooksConfigPanel.tsx`（接入 JSON 模式） |

*注：单 agent 避免 JsonMode 与 HooksConfigPanel 集成不同步。*

**实现要点**

- 将 SchemaStore `claude-code-settings.json` 复制到 `src/features/hooksConfig/schema/`。
- 新增 npm 依赖 `codemirror-json-schema` 与 `ajv`（保存前 schema 校验）。
- `JsonMode.tsx` 使用 CM6 + `@codemirror/lang-json` + schema 扩展；提供 `onValidationChange(isValid, diagnostics)` 回调。
- 事件导航侧栏：30+ 事件按九大分组，点击后在编辑器内定位到对应事件键（简单文本搜索 + `setSelection`）。
- `MatcherTester.tsx` 使用 `matcherEngine.ts`，输入 matcher + toolName → 显示命中结果与匹配模式。

**验证项**

1. `src/features/hooksConfig/schema/claude-code-settings.json` 存在且可被 Vite JSON import。
2. `JsonMode` 渲染 CM6 EditorView，schema 扩展被注册。
3. JSON 非法时 `onValidationChange` 返回 `isValid=false`。
4. 事件导航侧栏点击后编辑器选区跳到目标事件键。
5. L2 测试通过。

**Commit message**

```
feat: JSON 模式 — CM6 + SchemaStore schema + 事件导航 + matcher 测试工具

test: L2 JSON 模式渲染、校验、事件导航

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实 WebView2 中 schema 补全/悬停文档/错误波浪线是否生效（jsdom 无法验证 CM6 lint 渲染）。
- Schema 版本与线上 claude 的兼容性（schema 滞后问题为已知风险，F6 已接受）。

---

## Stage 05 — GUI 表单模式（Master-Detail）

**内容**：实现 `GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`。

**ID 列表**：P3-FE-12、P3-FE-13、P3-FE-14、P3-TE-11、P3-TE-12

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-gui-tree | P3-FE-12/13 + 测试 | `src/panels/hooksConfig/GuiMode.tsx`（新建）、`src/panels/hooksConfig/EventTree.tsx`（新建）、`HooksConfigPanel.tsx`（接入 GUI 模式） |
| frontend-handler-form | P3-FE-14 + 测试 | `src/panels/hooksConfig/HandlerForm.tsx`（新建） |

*注：两个 agent 文件零重叠；GuiMode 与 EventTree 共用，由同一 agent 保证一致性。*

**实现要点**

- `EventTree.tsx` 三级树：事件分组 → 事件 → matcher 组 → handler 摘要；选中态颜色使用 `theme/colors.ts` token（硬约束 #6）。
- `HandlerForm.tsx` 根据 `type` 渲染 5 种表单；事件 → handler 支持矩阵约束可选类型（Notification/SessionEnd/PreCompact/PostCompact 禁用 prompt/agent；SessionStart/Setup 禁用 http/prompt/agent）。
- 切换 type 时保留公共字段（如 `timeout`），清除不适用的字段。
- `GuiMode.tsx` 管理 `selectedEvent / selectedMatcherIndex / selectedHandlerIndex`，提供添加/删除事件、matcher、handler 的回调。

**验证项**

1. `EventTree` 渲染九大事件分组与事件名，hook 计数正确。
2. `HandlerForm` 5 种 type 必填字段正确渲染。
3. 事件 → handler 支持矩阵过滤测试通过。
4. type 切换时不适用的字段被清除。
5. L2 测试通过。

**Commit message**

```
feat: GUI 模式 — Master-Detail 事件树 + 5 种 handler 专用表单

test: L2 事件树与 handler 字段矩阵

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实高分辨率/窄窗口下 Master-Detail 布局是否可用。
- 表单字段较多时滚动与间距体验。

---

## Stage 06 — 双模式同步与保存安全

**内容**：JSON ↔ GUI 双向同步、保存前双重校验、保存成功提示。

**ID 列表**：P3-FE-16、P3-FE-17、P3-TE-13、P3-TE-14

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-sync-save | P3-FE-16/17 + 测试 | `src/panels/hooksConfig/HooksConfigPanel.tsx`、`src/panels/hooksConfig/useHooksConfig.ts`、`src/panels/hooksConfig/configModel.ts` |

**实现要点**

- 共享状态 `configJson` 与 `guiModel` 存放于 `useHooksConfig`。
- JSON 合法变更 → 调用 `jsonToGui` → 更新 GUI。
- GUI 变更 → 调用 `guiToJson` → 更新 JSON 文本。
- JSON 非法时模式切换按钮禁用，并显示错误提示文案。
- 保存流程：`JSON.parse` 语法校验 → `ajv` schema 校验 → `filterDisabled` 剔除禁用条目 → `writeHooksConfig`。
- 保存成功后显示提示条：「hooks 改动需重启 claude 会话生效」。

**验证项**

1. GUI 新增事件后 JSON 文本同步更新。
2. JSON 合法修改后 GUI 树同步更新。
3. JSON 非法时无法切换到 GUI 模式。
4. 语法错误 / schema 错误保存被拒绝。
5. 合法保存成功后显示重启提示。
6. L2 测试通过。

**Commit message**

```
feat: 双模式实时同步 + 保存安全（JSON/Schema 双校验）

test: L2 双模式同步与保存拒绝/提示

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实大配置（数百行 JSON）下双向同步性能。
- 保存拒绝弹窗文案清晰度。

---

## Stage 07 — 单条启停（ADR-0002）与 F2 并入

**内容**：单条 hook 禁用状态、保存过滤、失效禁用记录、F2 注入/卸载/状态显示。

**ID 列表**：P3-FE-19、P3-FE-20、P3-FE-21、P3-FE-22、P3-TE-15、P3-TE-16

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-disable | P3-FE-19/20 + 测试 | `src/stores/hooksConfig.ts`、`src/panels/hooksConfig/HooksConfigPanel.tsx`、`src/panels/hooksConfig/EventTree.tsx`、`src/panels/hooksConfig/HandlerForm.tsx` |
| frontend-f2 | P3-FE-21/22 + 测试 | `src/panels/hooksConfig/HooksConfigPanel.tsx`（注入按钮/状态条）、`src/ipc/hooks.ts`（已存在，仅复用） |

*注：`HooksConfigPanel.tsx` 被两个 agent 共享，需串行执行（disable 先，f2 后）或合并为一个 agent。为减少文件冲突，本 Stage 内使用 sequential pipeline：frontend-disable 完成后 frontend-f2 启动。*

**实现要点**

- 禁用四元组：`{ layer, event, matcher, command }`。
- 禁用状态存 `~/.slterminal/settings.json` 的 `disabledHooks` 段（ADR-0002）。
- 保存时从 `configJson` 中剔除匹配四元组的条目，再写盘。
- 重新启用时按四元组将条目插回原位置（若原位置因外部修改已不存在，则标记为失效）。
- 面板顶部常驻提示：「禁用条目由 slTerminal 托管，不出现在配置文件中」。
- 注入/卸载按钮复用阶段 1 的 `src/ipc/hooks.ts`；状态条显示 injected / notInjected / outdated。

**验证项**

1. 禁用条目保存时从 `writeHooksConfig` 的 content 中剔除。
2. 重新启用后条目回到配置。
3. 外部修改导致四元组失配时 UI 显示「失效的禁用记录」。
4. 注入/卸载按钮调用 `hooks_inject` / `hooks_uninstall`。
5. 状态条根据 `hooks_injection_status` 显示 injected/notInjected/outdated。
6. L2 测试通过。

**Commit message**

```
feat: 单条 hook 启停（ADR-0002）+ F2 注入/卸载并入面板

test: L2 禁用往返与失效禁用记录

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实场景：在 slTerminal 外修改 `.claude/settings.json` 后回到面板，失效禁用记录是否准确。
- 注入/卸载操作对真实 claude 配置的影响（最终由 Stage 10 端到端验证兜底）。

---

## Stage 08 — 面板入口命令

**内容**：新增 `global.openHooksConfig` 全局命令，绑定默认键并注册。

**ID 列表**：P3-FE-23、P3-FE-24、P3-FE-25、P3-TE-17

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-shortcut | P3-FE-23/24/25 + 测试 | `src/features/shortcuts/commandCatalog.ts`、`src/features/shortcuts/globalCommands.ts`、`src/__tests__/command-catalog.test.ts`、`src/App.tsx`（确认无需改动调用点） |

**实现要点**

- 在 `COMMAND_CATALOG` 追加 `global.openHooksConfig`，默认键推荐 `Ctrl+Shift+H`（待执行期确认）。
- 在 `createGlobalShortcuts` 中追加 handler：通过 `getDockviewApi().addPanel` 打开 `hooksConfig` 面板。
- 确保 `command-catalog.test.ts` 的默认键合法性守卫覆盖新命令（非保留键）。
- 同步更新 `src/__tests__/command-catalog.test.ts`：`EXPECTED_IDS` 加入 `global.openHooksConfig`、长度预期改为 10、补充该命令的 `commandFromMeta` 断言。
- `App.tsx` 已使用 `...createGlobalShortcuts(...)`，无需修改调用点。

**验证项**

1. `COMMAND_CATALOG` 包含 `global.openHooksConfig`，元数据完整。
2. `createGlobalShortcuts` 返回该命令，handler 调用 `addPanel({ component: "hooksConfig" })`。
3. 无 DockviewApi 时 handler 返回 `false` 透传。
4. `src/__tests__/command-catalog.test.ts` 已同步：`EXPECTED_IDS` 含 `global.openHooksConfig`、长度预期为 10、补充该命令的元数据与默认键非保留键断言。
5. L2 测试通过。

**Commit message**

```
feat: 全局快捷键打开 Hooks 配置面板

test: L2 openHooksConfig 命令注册与 handler

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 默认键 `Ctrl+Shift+H` 在真实 WebView2 中是否被浏览器/Tauri 默认行为拦截；如拦截则执行期改用 `Ctrl+Alt+H`。

---

## Stage 09 — L4 E2E 关键路径

**内容**：补充 E2E 用例，验证面板打开与真实保存链路。

**ID 列表**：P3-TE-18

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| e2e-tests | P3-TE-18 | `e2e-tests/test.e2e.ts`（必要时扩展 `e2e-tests/helpers.ts`） |

**实现要点**

- 用例：程序化打开 hooksConfig 面板 → JSON 模式写入合法 hooks 配置 → 点击保存 → 断言目标 settings.json 的 mtime 更新且内容正确。
- 保存按钮使用 `.click()`；JSON 文本输入通过 CM6 helper 或新增 `__slterm_e2e_*` helper（如需要）。
- 本 Stage 仅追加 L4 用例，不改生产代码。

**验证项**

1. `npm run build:e2e` 成功。
2. `npm run wdio` 新增用例通过。

**Commit message**

```
test: L4 E2E hooks 配置面板打开与保存链路

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- L4 必须在真实 Windows + WebView2 环境运行；本 Stage 本身即为人工/端到端验证。

---

## Stage 10 — 文档同步

**内容**：更新各子路径 CLAUDE.md 与 test-inventory.md。

**ID 列表**：P3-DOC-01、P3-DOC-02、P3-DOC-03、P3-DOC-04、P3-DOC-05

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| docs-update | P3-DOC-01/02/03/04/05 | `src/panels/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src/stores/CLAUDE.md`、`src/features/shortcuts/CLAUDE.md`、`.claude/test-inventory.md` |

**实现要点**

- 按各模块 CLAUDE.md 的既有格式追加 `hooksConfig` 相关条目，不在根 CLAUDE.md 展开细节。
- `test-inventory.md` 新增 Phase 3 测试文件与用例数，并更新全量总数。
- 文档描述须与 Stage 完成后的真实代码一致（不可照抄计划草案）。

**验证项**

1. 5 份 CLAUDE.md 均已更新且与代码一致。
2. `test-inventory.md` 新增 Phase 3 文件/用例数，全量总数正确。
3. 文档中 IPC 命令名、面板类型名、store 字段名与代码一致。

**Commit message**

```
docs: 同步 Phase 3 hooks 配置面板文档与测试清单

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 文档与代码的一致性需人工抽查（特别是 test-inventory 用例数与实际 grep 结果一致）。

---

## Stage 依赖图

```
Stage 01 ─┐
Stage 02 ─┼─> Stage 03 ─> Stage 04 ─> Stage 06
          │                │
          │                └─> Stage 05 ─┘
          │                                  │
          └──────────────────────────────────┘
                                           ↓
Stage 07 ─> Stage 08 ─> Stage 09 ─> Stage 10
```

- Stage 01/02 可并行启动。
- Stage 03 依赖 Stage 01/02 完成（IPC 与后端命令就绪）。
- Stage 04/05 依赖 Stage 03，可并行（修改不同文件）。
- Stage 06 依赖 Stage 04/05。
- Stage 07 依赖 Stage 06。
- Stage 08 依赖 Stage 03（面板已注册）。
- Stage 09 依赖所有代码 Stage 完成。
- Stage 10 固定最后。

---

## 门禁命令汇总

每 Stage 的全量测试 agent 执行以下命令子集（按本 Stage 触碰文件选择）：

| Stage | 门禁命令 |
|-------|----------|
| 01 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`cargo test --manifest-path src-tauri/Cargo.toml p3_hooks_config -- --test-threads=1` |
| 02-08 | `npx tsc --noEmit`、`npx eslint src/`、`npm test`（全量 L2） |
| 09 | `npm run build:e2e`、`npm run wdio` |
| 10 | 仅文档检查，无代码门禁 |

*注：Stage 09 不跑 L1/L2；Stage 10 不跑测试。*
