# Phase 3 清单 — Hooks 双模式配置面板（F6）

> 编号前缀：P3-BE（后端）、P3-FE（前端）、P3-TE（测试）、P3-DOC（文档）。
> 跨阶段共享契约见 `docs/hooks-dev/contract.md`（C13 阶段 3 专节），本清单不重复定义契约字段。
> 标记 **【待执行期确认】** 的项已在 stages.md 中给出推荐值，执行前由主 agent 最终拍板。

---

## Stage 1 — 后端三层配置读写命令

### P3-BE-01：新建 `src-tauri/src/hooks/config.rs`

- 实现三层配置路径纯函数：
  - `user` 层 → `~/.claude/settings.json`（`dirs::home_dir()` 解析，绕过 project_root 沙箱）
  - `project` 层 → `<projectPath>/.claude/settings.json`
  - `local` 层 → `<projectPath>/.claude/settings.local.json`
- project/local 层的 `project_path` 入参须经 `crate::state::validate_path_within_root` 沙箱校验。
- 父目录不存在时自动创建（`create_dir_all`）。

### P3-BE-02：实现 `hooks_config_read` 命令

- 签名：`hooks_config_read(layer: String, project_path: Option<String>) -> Result<serde_json::Value, AppError>`
- 文件不存在或 JSON 损坏时返回 `Ok(Value::Null)`，不抛错（方便面板首次创建）。
- `layer` 仅允许 `"user"` / `"project"` / `"local"`，非法返回 `AppError::Validation`。

### P3-BE-03：实现 `hooks_config_write` 命令

- 签名：`hooks_config_write(layer: String, content: serde_json::Value, project_path: Option<String>) -> Result<(), AppError>`
- 要求 `content` 为 JSON Object，否则返回 `AppError::Validation`。
- 原子写入：tempfile → `flush` → `persist`（照 `settings.rs` 先例，但 Phase 3 明确不做 `.bak`）。
- project/local 层写入前校验 `project_path` 在 project_root 沙箱内。

### P3-BE-04：在 `src-tauri/src/hooks/mod.rs` 暴露配置命令

- 前置：阶段 1 已完成 `hooks` 模块骨架（`mod.rs` / `inject.rs` / `signal.rs`）。
- 新增 `pub mod config;`，并在 `mod.rs` 的 Tauri 命令注册列表中加入 `config::hooks_config_read` / `config::hooks_config_write`。
- 复用阶段 1 的模块入口与错误处理风格。

### P3-BE-05：在 `src-tauri/src/lib.rs` 注册命令

- 在 `generate_handler!` 宏中追加 `hooks_config_read`、`hooks_config_write`。
- 与阶段 1 的 `hooks_inject`、`hooks_uninstall`、`hooks_injection_status` 并列。

### P3-BE-06：user 层路径绕过沙箱

- user 层命令体内不得调用 `validate_path_within_root`。
- 路径解析使用 `dirs::home_dir()`（`Cargo.toml` 已依赖 `dirs = "6"`）。
- 目录不存在时自动创建。

### P3-BE-07：project/local 层路径沙箱校验

- 仅在 `layer` 为 `"project"` / `"local"` 时读取 `project_path`。
- 使用 `validate_path_within_root(&project_path)` 校验；未通过时返回 `AppError::PathNotAllowed`。
- 通过后将 `.claude/settings.json` 或 `.claude/settings.local.json` 拼接到 project_path 之后。

### P3-BE-08：扩展/复用 `AppError`

- 非法 `layer` 与非法 `content` 统一走 `AppError::Validation`。
- IO 错误走 `AppError::Io` / `AppError::IoKind`。

### P3-TE-01：L1 测试 — user 层读取与原子写

- 使用 `tempfile::tempdir()` 隔离，覆盖：文件不存在返回 Null、合法 JSON 读取、原子写后内容正确、父目录自动创建。

### P3-TE-02：L1 测试 — project/local 层路径与沙箱

- 覆盖：project 与 local 路径解析正确、沙箱校验失败返回 PathNotAllowed、损坏 JSON 返回 Null。

---

## Stage 2 — IPC 封装、DTO 与 matcher 语义引擎

### P3-FE-05：新建 `src/ipc/hooksConfig.ts`

- 封装命令：
  - `readHooksConfig(layer, projectPath?) -> Promise<unknown>` → 调用 `hooks_config_read`
  - `writeHooksConfig(layer, content, projectPath?) -> Promise<void>` → 调用 `hooks_config_write`
- `layer` 类型使用 `"user" | "project" | "local"`。
- `invoke` 只出现在本文件（硬约束 #1）。

### P3-FE-06：新建 `src/types/hooksConfig.ts`

- 定义：`HooksLayer = "user" | "project" | "local"`
- 定义 GUI 模型类型：`HooksConfigGui`、`HookEventGroup`、`HookMatcherGroup`、`HookHandlerGui`（字段矩阵与必填项照 F6 handler 字段表）。
- 定义禁用记录类型：`DisabledHookKey = { layer, event, matcher, command }`。

### P3-FE-08：新建 `src/panels/hooksConfig/matcherEngine.ts`

- 纯函数 `matchHook(matcher: string, toolName: string, event?: string) -> { matched: boolean; mode: "exact-or" | "regex" | "all" }`
- 严格按 F6 matcher 语义表实现：
  - 窄字符集（字母/数字/`_`/`-`/空格/`\|`/`,`）→ 精确匹配 OR
  - 含其他字符 → JS 正则非锚定
  - `""` / `"*"` / 省略 → 全匹配
  - 大小写敏感
  - `FileChanged` / `StopFailure` 窄字符集仅字母/数字/`_`/`|`（连字符/空格/逗号强制走正则）

### P3-FE-10：新建 `src/panels/hooksConfig/configModel.ts`

- 定义原始 JSON 类型（`HooksConfigJson`）与 GUI 模型（`HooksConfigGui`）。
- 实现 `jsonToGui(json)` 与 `guiToJson(gui)` 双向转换。
- 实现 `filterDisabled(config, disabledKeys)`：从配置中剔除被禁用条目。

### P3-TE-05：L2 测试 — matcher 语义全表

- 覆盖：精确匹配 OR、JS 正则、全匹配、大小写敏感、`FileChanged`/`StopFailure` 窄字符集强制正则。
- 每个语义分支至少 2 条用例。

### P3-TE-06：L2 测试 — configModel 双向转换

- 覆盖：空配置、单事件单 matcher 单 handler、多事件多 handler、字段缺失/多余容错。

---

## Stage 3 — 面板骨架、注册与数据 hook

### P3-FE-01：新建 `src/panels/hooksConfig/` 目录结构

- 文件清单：`index.ts`、`HooksConfigPanel.tsx`、`useHooksConfig.ts`、`JsonMode.tsx`、`GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`、`MatcherTester.tsx`、`configModel.ts`、`matcherEngine.ts`、样式文件（可选）。

### P3-FE-02：实现 `HooksConfigPanel.tsx` 骨架

- 顶部工具栏：层级切换器（user/project/local，显示优先级 local>project>user）、模式切换（GUI | JSON）、注入状态条、保存按钮。
- 中部为模式切换容器，Stage 3 先渲染占位文案。
- 加载态/错误态占位（照 gitshow/diff 三态模式）。

### P3-FE-03：在 `src/panelRegistry.ts` 注册面板

- 新增 `PANEL_HOOKS_CONFIG = "hooksConfig"`。
- 在 `panelRegistry` 对象追加 `hooksConfig: HooksConfigPanel`。
- 在 `PANEL_TYPES` 数组追加 `PANEL_HOOKS_CONFIG`。
- `FILE_PANEL_TYPES` 与 `isAlwaysRenderPanel` 不加入 hooksConfig（无 filePath、无需始终挂载）。

### P3-FE-04：在 `src/panels/index.ts` 导出

- `export { HooksConfigPanel } from "./hooksConfig";`

### P3-FE-15：实现 `useHooksConfig.ts`

- 从 `useProjects` + `useLayout` 推导当前活跃项目 `rootPath`。
- 状态：`layer`、`configJson`、`guiModel`、`dirty`、`error`、`loading`。
- 加载：调用 `readHooksConfig(layer, rootPath)`，Null 视为 `{}`；面板挂载时调用 `useHooksConfigStore.getState().loadFromDisk()` 加载禁用状态。
- 保存：先 JSON+Schema 校验，再调用 `writeHooksConfig`。
- 切换 layer 时若 dirty 提示未保存。

### P3-FE-18：新建 `src/stores/hooksConfig.ts`

- 状态：`disabledHooks: DisabledHookKey[]`、`loaded: boolean`。
- `loadFromDisk()`：从 `src/ipc/settings` 读 `disabledHooks` 并 sanitize；store 不在 App init 中加载，由 `useHooksConfig.ts` 在面板挂载时调用。
- `saveToDisk()`：调用 `saveSettings({ disabledHooks })`（后端浅合并，不擦其他段）。
- `disableHook(key)` / `enableHook(key)` / `isDisabled(key)`。
- 导出 `cancelPendingSave()` 供 `App.tsx` 关窗冲刷。

### P3-FE-18b：在 `App.tsx` 关窗钩子中冲刷 hooksConfig 保存

- 从 `src/stores/hooksConfig` 导入 `cancelPendingSave`（重命名避免与 projects 的 `cancelPendingSave` 冲突）。
- 在 `registerCloseHandler` 的保存序列中，与其他 store 一并调用。

### P3-TE-07：L2 测试 — store 禁用状态持久化

- 覆盖：loadFromDisk sanitize、disable/enable、debounce 保存 payload 键集合精确匹配 `{ disabledHooks }`。

### P3-TE-08：L2 测试 — panelRegistry 与面板渲染

- 覆盖：PANEL_TYPES 包含 hooksConfig、`isValidPanelType` 识别、`HooksConfigPanel` 渲染三态、层级切换器存在。
- 同步更新 `src/__tests__/panel-registry.test.ts`：预期 6 个面板键、`PANEL_TYPES` 长度为 6、新增 `hooksConfig` 专项断言。

---

## Stage 4 — Schema 内嵌与 JSON 模式

### P3-FE-07：内嵌 SchemaStore 官方 schema

- 位置：`src/features/hooksConfig/schema/claude-code-settings.json`。
- 将 SchemaStore `https://json.schemastore.org/claude-code-settings.json` 内容复制到该文件（离线可用、版本随 slTerminal 发布更新）。
- Vite `import schema from ".../schema/claude-code-settings.json"` 可加载。

### P3-FE-11：实现 `JsonMode.tsx`

- 使用 CodeMirror 6 + `@codemirror/lang-json`。
- 集成 `codemirror-json-schema` 提供 schema 补全/校验/悬停文档/错误波浪线（新增依赖）。
- 事件导航侧栏：30+ 事件按九大分组，点击跳转到对应 JSON 段落（简单文本搜索定位）。
- 集成 `MatcherTester.tsx` 作为浮动/内联工具。
- 非法 JSON 时通过 `onValidationChange` 通知父组件。

### P3-TE-09：L2 测试 — JSON 模式渲染与 Schema 校验

- mock `readHooksConfig`，验证 CM6 EditorView 创建、schema 扩展注册、错误时 onValidationChange 被调用。

### P3-TE-10：L2 测试 — 事件导航

- 验证九大分组事件名渲染、点击后编辑器选区跳到对应事件键位置。

---

## Stage 5 — GUI 表单模式（Master-Detail）

### P3-FE-12：实现 `GuiMode.tsx`

- Master-Detail 布局：左侧事件树，右侧详情区。
- 状态：`selectedEvent`、`selectedMatcherIndex`、`selectedHandlerIndex`。
- 提供添加/删除事件、matcher 组、handler 的回调。

### P3-FE-13：实现 `EventTree.tsx`

- 三级树：事件分组（可折叠）→ 事件名 → matcher 组 → handler 摘要。
- 显示各事件下 hook 数量。
- 选中态高亮（配色从 `theme/colors.ts` 取 token，硬约束 #6）。

### P3-FE-14：实现 `HandlerForm.tsx`

- 根据 `type` 渲染 5 种 handler 专用表单，字段矩阵与必填项照 F6 表：
  - `command`：command*、args、async、asyncRewake、shell、timeout、if、allowedEnvVars
  - `http`：url*、method、headers、body、timeout、allowedEnvVars
  - `mcp_tool`：server*、tool*、args、timeout
  - `prompt`：prompt*、timeout
  - `agent`：prompt*、description、subagent_type、model（sonnet/opus/haiku/fable）、timeout
- 事件 → handler 支持矩阵约束：Notification/SessionEnd/PreCompact/PostCompact 不允许 prompt/agent；SessionStart/Setup 不允许 http/prompt/agent。
- 切换 type 时保留公共字段、清除不适用的字段。

### P3-TE-11：L2 测试 — handler 表单字段矩阵

- 覆盖：5 种 type 必填字段渲染、事件支持矩阵过滤、type 切换清理字段。

### P3-TE-12：L2 测试 — 事件树结构

- 覆盖：分组渲染、hook 计数、选中回调、添加/删除事件。

---

## Stage 6 — 双模式同步与保存安全

### P3-FE-16：实现双模式同步

- JSON 合法编辑后实时调用 `jsonToGui` 更新 GUI 模型。
- GUI 编辑后实时调用 `guiToJson` 更新 JSON 文本。
- JSON 非法时禁止切换到 GUI 模式，并在工具栏显示错误提示。
- 两模式共享同一份 `configJson` 与脏状态。

### P3-FE-17：实现保存安全

- 保存按钮触发：JSON.parse 语法校验 → Schema 校验（使用 `ajv` 或 `codemirror-json-schema` 底层校验器）→ 任一失败则弹窗提示、拒绝调用 `writeHooksConfig`。
- 保存成功后显示提示条：「hooks 改动需重启 claude 会话生效」。
- 不做 `.bak`（与 F6 决策一致）。

### P3-TE-13：L2 测试 — 双模式同步

- 覆盖：GUI 新增事件 → JSON 文本含该事件；JSON 合法修改 → GUI 树更新；JSON 非法 → 切 GUI 被阻止。

### P3-TE-14：L2 测试 — 保存拒绝与提示

- 覆盖：语法错误保存被拒、Schema 错误保存被拒、合法保存成功显示重启提示。

---

## Stage 7 — 单条启停（ADR-0002）与 F2 并入

### P3-FE-19：实现禁用 UI

- 每条 handler 右侧显示启停 checkbox。
- 禁用条目在事件树中视觉区分（如置灰 + 文字删除线）。
- 常驻提示文案：「禁用条目由 slTerminal 托管，不出现在配置文件中」。
- 失效禁用记录：当四元组（layer+event+matcher+command）在配置中找不到匹配时，在 UI 标记为「失效的禁用记录」。

### P3-FE-20：保存时过滤禁用条目

- 在 `useHooksConfig.ts` 的保存逻辑中，先将 `configJson` 经 `filterDisabled(config, disabledHooks)` 剔除禁用条目，再写入磁盘。
- 重新启用时条目按原位置恢复（依赖 `disabledHooks` 记录中保存的完整四元组，保存前重新插回）。

### P3-FE-21：在面板内集成 F2 注入/卸载按钮

- 复用契约 C6 三条命令：`hooks_inject`、`hooks_uninstall`、`hooks_injection_status`（封装在 `src/ipc/hooks.ts`，阶段 1 已完成）。
- 按钮位置：面板顶部工具栏或独立状态条。

### P3-FE-22：显示注入状态

- 调用 `hooks_injection_status` 获取状态，显示：已注入/未注入/版本过旧。
- 注入/卸载操作后刷新状态。

### P3-TE-15：L2 测试 — 禁用状态往返

- 覆盖：禁用 → 保存时 IPC 调用 content 不含禁用条目 → store 持久化 disabledHooks → 重载后禁用状态保留 → 重新启用后条目恢复。

### P3-TE-16：L2 测试 — 失效禁用记录

- 覆盖：手动修改 JSON 使四元组失配 → UI 显示失效标记 → 重新启用或删除失效记录后标记消失。

---

## Stage 8 — 面板入口命令

### P3-FE-23：新增 `global.openHooksConfig` 命令

- 在 `src/features/shortcuts/commandCatalog.ts` 的 `COMMAND_CATALOG` 追加：
  - id: `global.openHooksConfig`
  - title: "打开 Hooks 配置"
  - category/context: `global`
  - defaultKey: **【待执行期确认，推荐 `Ctrl+Shift+H`】**
  - priority: 10
- 默认键须经 `reserved.test.ts` 同模式校验为非保留键。

### P3-FE-24：在 `src/features/shortcuts/globalCommands.ts` 提供 handler

- handler 通过 `getDockviewApi().addPanel({ id: generatePanelId(), component: "hooksConfig", params: {} })` 打开面板。
- 无活跃页面时返回 `false` 透传。

### P3-FE-25：在 `src/App.tsx` 注册命令

- 在 `registry.register([...])` 中加入 `...createGlobalShortcuts(...)` 已包含；确认 `globalCommands.ts` 的扩展无需修改 App.tsx 调用点。
- 实际只需确保 `createGlobalShortcuts` 返回新命令。

### P3-TE-17：L2 测试 — 入口命令

- 覆盖：`commandCatalog` 含 `global.openHooksConfig`、`createGlobalShortcuts` 返回该命令、handler 调用 `addPanel`、无 api 时透传。
- 同步更新 `src/__tests__/command-catalog.test.ts`：`EXPECTED_IDS` 加入 `global.openHooksConfig`、长度预期改为 10、补充该命令的 `commandFromMeta` 断言。

---

## Stage 9 — L4 E2E 关键路径

### P3-TE-18：L4 测试 — 面板打开与保存链路

- 用例：通过 E2E helper 打开 hooksConfig 面板 → 在 JSON 模式输入合法 hooks 配置 → 点击保存 → 断言 `hooks_config_write` 真实写盘（检查目标 settings.json mtime 变化）。
- 因 E2E 键盘输入限制（TE-17），保存按钮用 `.click()` 触发；JSON 输入通过 CM6 helper 或 `__slterm_e2e_*` 扩展注入。

---

## Stage 10 — 文档同步

### P3-DOC-01：更新 `src/panels/CLAUDE.md`

- 在「当前面板类型」与文件清单中加入 `hooksConfig` 面板。
- 描述：双模式编辑（JSON/GUI）、三层配置、单条启停、F2 并入。

### P3-DOC-02：更新 `src/ipc/CLAUDE.md`

- 模块映射表追加 `src/ipc/hooksConfig.ts` ↔ `hooks/`：`hooks_config_read`、`hooks_config_write`。
- 说明与阶段 1 的 `src/ipc/hooks.ts` 区分（后者为 C6 注入/事件命令）。

### P3-DOC-03：更新 `src/stores/CLAUDE.md`

- Store 清单追加 `hooksConfig.ts`：禁用状态、`disabledHooks` 段、`loadFromDisk`/`saveToDisk` 模式。
- 新增 Store 规则已满足：文件放 `src/stores/`、`index.ts` re-export、持久化走 `src/ipc/settings`。

### P3-DOC-04：更新 `src/features/shortcuts/CLAUDE.md`

- 命令目录追加 `global.openHooksConfig`。
- 扩展指南示例更新（全局命令 factory 模式）。

### P3-DOC-05：更新 `.claude/test-inventory.md`

- 新增 Phase 3 测试文件与用例数（按实际执行后统计填入）。
- 全量用例数累加更新。

---

## 依赖与开放项汇总

| 依赖 | 说明 |
|------|------|
| 阶段 1 完成 | `src-tauri/src/hooks/mod.rs`、`src/ipc/hooks.ts`、C6 三条命令已存在 |
| SchemaStore JSON | 执行期下载/复制 `claude-code-settings.json` 到 `src/features/hooksConfig/schema/` |
| 新增 npm 依赖 | `codemirror-json-schema`（JSON 模式 schema 支持）、`ajv`（保存前 schema 校验） |
| 【待执行期确认】 | `global.openHooksConfig` 默认键（推荐 `Ctrl+Shift+H`） |
