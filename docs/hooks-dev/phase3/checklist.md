# Phase 3 清单 — Hooks 双模式配置面板（F6）

> 编号前缀：P3-BE（后端）、P3-FE（前端）、P3-TE（测试）、P3-DOC（文档）。
> 跨阶段共享契约见 `docs/hooks-dev/contract.md` C13（2026-07-31 全量修订版），本清单不重复定义契约字段。
> **2026-07-31 修订说明**：Phase 2 完成后对照代码现状 + 官方文档核实 + 8 项用户拍板全面修订。核心变化：① 后端读写改为 **hooks 子树级**（read 返回子树、write 后端 read-modify-write merge 保留其他字段）；② handler 字段矩阵以官方核实为准；③ 新增 eventsCatalog 事件元数据单点（30 事件 × D2 十组）；④ 注入段 GUI 禁改保护；⑤ 入口面板同页单例；⑥ E2E 走 project/local 层；⑦ wrapper 名/test filter/不存在的 generatePanelId 等失真修正。

---

## Stage 1 — 后端三层 hooks 子树读写命令

### P3-BE-01：新建 `src-tauri/src/hooks/config.rs`

- 实现三层配置路径纯函数：
  - `user` 层 → `~/.claude/settings.json`（`dirs::home_dir()` 解析，绕过 project_root 沙箱）
  - `project` 层 → `<projectPath>/.claude/settings.json`
  - `local` 层 → `<projectPath>/.claude/settings.local.json`
- project/local 层的 `project_path` 入参须经 `crate::state::validate_path_within_root` 沙箱校验。
- 父目录不存在时自动创建（`create_dir_all`，仅写入路径）。

### P3-BE-02：实现 `hooks_config_read` 命令（返回 hooks 子树）

- 签名：`hooks_config_read(layer: String, project_path: Option<String>) -> Result<serde_json::Value, AppError>`
- 读文件 → 提取并返回 **`hooks` 子树**（非整文件）。
- 文件不存在、或文件合法但无 `hooks` 键 → 返回 `Ok(Value::Null)`（面板首次创建场景）。
- **JSON 损坏 → 返回 `Err`（AppError::Validation 或 Io 系）**，不返回 Null——防止面板在损坏文件上编辑后 merge 丢其他字段（对齐 C9 注入的非法中止先例）。
- `layer` 仅允许 `"user"` / `"project"` / `"local"`，非法返回 `AppError::Validation`。

### P3-BE-03：实现 `hooks_config_write` 命令（read-modify-write merge）

- 签名：`hooks_config_write(layer: String, hooks: serde_json::Value, project_path: Option<String>) -> Result<(), AppError>`
- 要求 `hooks` 为 JSON Object（hooks 子树），否则返回 `AppError::Validation`。
- **read-modify-write**：读原文件（不存在视为 `{}`）→ 将根对象 `hooks` 键替换为入参 → 写回。原样保留 `permissions`/`env`/`$schema` 等其他字段。
- 原文件 JSON 损坏 → 返回 `Err` 拒绝写入（不覆盖用户文件）。
- 原子写入：tempfile → `flush` → `persist`（照 `settings.rs` 先例，明确不做 `.bak`）。
- project/local 层写入前校验 `project_path` 在 project_root 沙箱内。

### P3-BE-04：在 `src-tauri/src/hooks/mod.rs` 暴露配置命令

- 新增 `pub mod config;`，并在 `mod.rs` 的导出中加入 `config::hooks_config_read` / `config::hooks_config_write`。
- 复用模块既有错误处理风格。

### P3-BE-05：在 `src-tauri/src/lib.rs` 注册命令

- 在 `generate_handler!` 宏中追加 `hooks_config_read`、`hooks_config_write`，与既有 `hooks_inject` 等并列。

### P3-BE-06：user 层路径绕过沙箱

- user 层命令体内不得调用 `validate_path_within_root`。
- 路径解析使用 `dirs::home_dir()`（`Cargo.toml` 已依赖 `dirs = "6"`）。

### P3-BE-07：project/local 层路径沙箱校验

- 仅在 `layer` 为 `"project"` / `"local"` 时读取 `project_path`（缺失返回 Validation）。
- 使用 `validate_path_within_root(&project_path)` 校验；未通过时返回 `AppError::PathNotAllowed`。
- 通过后拼接 `.claude/settings.json` 或 `.claude/settings.local.json`。

### P3-BE-08：扩展/复用 `AppError`

- 非法 `layer`、非法 `hooks`、JSON 损坏统一走 `AppError::Validation`（或沿用模块既有变体风格）。
- IO 错误走 `AppError::Io` / `AppError::IoKind`。
- 阻塞 I/O 全部在 `spawn_blocking` 内（硬约束 #3）。

### P3-TE-01：L1 测试 — user 层读取与原子写

- 使用 `tempfile::tempdir()` 隔离，覆盖：文件不存在返回 Null、无 hooks 键返回 Null、合法 hooks 子树读取、原子写后内容正确、父目录自动创建。

### P3-TE-02：L1 测试 — merge 保留、损坏拒绝与沙箱

- 覆盖：**write 后其他字段原样保留**（文件含 `permissions`/`env` 时 merge 不丢）、**损坏 JSON read/write 均报错**、project/local 路径解析正确、沙箱校验失败返回 PathNotAllowed、非 Object hooks 返回 Validation。

---

## Stage 2 — IPC 封装、DTO、eventsCatalog、matcher 语义引擎与 configModel

### P3-FE-05：新建 `src/ipc/hooksConfig.ts`

- 封装命令：
  - `readHooksConfig(layer, projectPath?) -> Promise<unknown>` → 调用 `hooks_config_read`（返回 hooks 子树或 null）
  - `writeHooksConfig(layer, hooks, projectPath?) -> Promise<void>` → 调用 `hooks_config_write`（payload 字段 `hooks`）
- `layer` 类型使用 `"user" | "project" | "local"`。
- `invoke` 只出现在本文件（硬约束 #1）；`src/ipc/index.ts` 追加 barrel export。

### P3-FE-06：新建 `src/types/hooksConfig.ts`

- 定义：`HooksLayer = "user" | "project" | "local"`
- 定义原始 JSON 类型：`HooksConfigJson`（= settings.json 的 `hooks` 子树：`Record<事件名, MatcherGroupJson[]>`）、`MatcherGroupJson`、`HookHandlerJson`。
- 定义 GUI 模型类型：`HooksConfigGui`、`HookEventGroup`、`HookMatcherGroup`、`HookHandlerGui`（字段矩阵照 C13-3 官方版）。
- 定义禁用记录类型：`DisabledHookKey = { layer, event, matcher, command }`。

### P3-FE-26：新建 `src/panels/hooksConfig/eventsCatalog.ts`（事件元数据单点）

- 30 事件 × 10 组完整映射（真值源 `docs/hooks/D2/02-settings-json-schema.md` §4.5，全表写死于 stages.md Stage 02）。
- 每事件元数据：所属分组、是否支持 matcher、matcher 匹配目标（工具名/notification_type/source/trigger 等）、支持的 handler 类型（照 C13-4 三档矩阵）。
- 5 种 handler 字段矩阵常量（照 C13-3：各类型字段清单 + 必填项 + 通用字段 if/timeout/statusMessage；once 不展示）。
- 纯数据 + 纯查询函数，零 DOM/React，供 EventTree / HandlerForm / JsonMode 导航 / MatcherTester 共用（单一真值源）。

### P3-FE-08：新建 `src/panels/hooksConfig/matcherEngine.ts`

- 纯函数 `matchHook(matcher: string, toolName: string, event?: string) -> { matched: boolean; mode: "exact-or" | "regex" | "all" }`
- 严格按 C13-5 matcher 语义表实现：
  - 窄字符集（字母/数字/`_`/`-`/空格/`\|`/`,`）→ 精确匹配 OR
  - 含其他字符 → JS 正则非锚定
  - `""` / `"*"` / 省略 → 全匹配
  - 大小写敏感
  - `FileChanged` / `StopFailure` 窄字符集仅字母/数字/`_`/`|`（连字符/空格/逗号强制走正则）
- 注释写明版本前提：逗号/空格分隔需 claude v2.1.191+、连字符需 v2.1.195+。

### P3-FE-10：新建 `src/panels/hooksConfig/configModel.ts`

- 实现 `jsonToGui(json)` 与 `guiToJson(gui)` 双向转换（输入为 hooks 子树）。
- 实现 `filterDisabled(config, disabledKeys)`：从配置中剔除被禁用条目。
- 实现 `isSltermManaged(handler)`：`command` 含 `slterm-hook-reporter` 子串判定（识别规则照 C9），供 GUI 注入段禁改标记。
- 不支持 matcher 的事件（eventsCatalog 标记）：`guiToJson` 省略 `matcher` 键但保留数组包裹。
- `jsonToGui` 对非对象/非数组输入降级为空模型，不抛错。

### P3-TE-05：L2 测试 — matcher 语义全表

- 覆盖：精确匹配 OR（`|` 与 `,`）、JS 正则、全匹配、大小写敏感、`FileChanged`/`StopFailure` 窄字符集强制正则。
- 每个语义分支至少 2 条用例。

### P3-TE-06：L2 测试 — configModel 双向转换

- 覆盖：空配置、单事件单 matcher 单 handler、多事件多 handler、字段缺失/多余容错、不支持 matcher 事件省略 matcher 键、`isSltermManaged` 判定、`filterDisabled` 剔除。

### P3-TE-19：L2 测试 — eventsCatalog 常量表

- 覆盖：30 事件齐全且唯一、10 分组齐全、每事件 handler 支持矩阵与 C13-4 一致（三档抽查 + 全量断言）、不支持 matcher 的 10 事件标记正确、5 种 handler 字段矩阵与 C13-3 一致。

---

## Stage 3 — 面板骨架、注册与数据 hook

### P3-FE-01：新建 `src/panels/hooksConfig/` 目录结构

- 文件清单：`index.ts`、`HooksConfigPanel.tsx`、`useHooksConfig.ts`、`JsonMode.tsx`、`GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`、`MatcherTester.tsx`、`eventsCatalog.ts`、`configModel.ts`、`matcherEngine.ts`。

### P3-FE-02：实现 `HooksConfigPanel.tsx` 骨架

- 顶部工具栏：层级切换器（user/project/local，显示优先级 local>project>user）、模式切换（GUI | JSON）、注入状态条、保存按钮。
- 中部为模式切换容器，Stage 3 先渲染占位文案。
- 加载态/错误态占位（照 gitshow/diff 三态模式）；**配置损坏错误态**（read 返回 Err 时显示"配置文件损坏，请先修复"）。

### P3-FE-03：在 `src/panelRegistry.ts` 注册面板

- 新增 `PANEL_HOOKS_CONFIG = "hooksConfig"`。
- 在 `panelRegistry` 对象追加 `hooksConfig: HooksConfigPanel`。
- 在 `PANEL_TYPES` 数组**末尾**追加 `PANEL_HOOKS_CONFIG`。
- `FILE_PANEL_TYPES` 与 `isAlwaysRenderPanel` 不加入 hooksConfig（无 filePath、无需始终挂载）。

### P3-FE-04：在 `src/panels/index.ts` 导出

- `export { HooksConfigPanel } from "./hooksConfig";`

### P3-FE-15：实现 `useHooksConfig.ts`

- 从 `useProjects` + `useLayout` 推导当前活跃项目 `rootPath`；rootPath 为空时 project/local 层禁用（仅 user 层可用）。
- 状态：`layer`、`configJson`（hooks 子树）、`guiModel`、`dirty`、`error`、`loading`。
- 加载：调用 `readHooksConfig(layer, rootPath)`，null 视为 `{}`；面板挂载时调用 `useHooksConfigStore.getState().loadFromDisk()` 加载禁用状态。
- 保存：先 JSON+Schema 校验，再调用 `writeHooksConfig`。
- **轻量重读（外部修改检测）**：切层 / 面板聚焦（focusin）时重新 `readHooksConfig`；`dirty` 时提示未保存（dialog.ask，照编辑器先例），用户选择丢弃才覆盖。
- 切换 layer 时若 dirty 同样提示。

### P3-FE-18：新建 `src/stores/hooksConfig.ts`

- 状态：`disabledHooks: DisabledHookKey[]`、`loaded: boolean`。
- `loadFromDisk()`：从 `src/ipc/settings` 读 `disabledHooks` 并 sanitize；store 不在 App init 中加载，由 `useHooksConfig.ts` 在面板挂载时调用。
- `saveToDisk()`：调用 `saveSettings({ disabledHooks })`（后端浅合并，不擦其他段）。
- `disableHook(key)` / `enableHook(key)` / `isDisabled(key)`。
- 导出 `cancelPendingSave()` 供 `App.tsx` 关窗冲刷。

### P3-FE-18b：在 `App.tsx` 关窗钩子中冲刷 hooksConfig 保存

- 从 `src/stores/hooksConfig` 导入 `cancelPendingSave`（重命名避免与 projects 的 `cancelPendingSave` 冲突，照 `cancelSideBarSave` 先例）。
- 在 `registerCloseHandler` 的保存序列中，与其他 store 一并调用。

### P3-TE-07：L2 测试 — store 禁用状态持久化

- 覆盖：loadFromDisk sanitize、disable/enable、debounce 保存 payload 键集合精确匹配 `{ disabledHooks }`。

### P3-TE-08：L2 测试 — panelRegistry 与面板渲染

- 覆盖：PANEL_TYPES 包含 hooksConfig、`isValidPanelType` 识别、`HooksConfigPanel` 渲染三态、层级切换器存在。
- 同步更新 `src/__tests__/panel-registry.test.ts`：`panelRegistry` 预期 6 个键、`PANEL_TYPES` 长度 6、**`toEqual` 精确数组断言追加 `"hooksConfig"`（末尾）**、新增 `hooksConfig` 专项断言。

---

## Stage 4 — Schema 内嵌与 JSON 模式

### P3-FE-07：内嵌 SchemaStore 官方 schema + hooks 子 schema

- 位置：`src/features/hooksConfig/schema/claude-code-settings.json`。
- 将 SchemaStore `https://json.schemastore.org/claude-code-settings.json` 内容复制到该文件（离线可用、版本随 slTerminal 发布更新）。
- **执行期核实 schema 自包含性**：`codemirror-json-schema` 仅支持本地 `$ref`；若含远程 `$ref` 需预打包展开。
- 提取 `properties.hooks` 子 schema 供 JSON 模式与保存校验使用（对齐 hooks 子树编辑范围）。

### P3-FE-11：实现 `JsonMode.tsx`

- 使用 CodeMirror 6 + `@codemirror/lang-json`（已有依赖）。
- 集成 `codemirror-json-schema`（新增依赖）：`jsonCompletion` 补全 + `jsonSchemaHover` 悬停 + `jsonSchemaLinter` 波浪线；同步新增 `@codemirror/lint`、`@codemirror/autocomplete`（peer deps，当前 package.json 缺失）。
- **不引 ajv**——保存前校验用其底层 `json-schema-library`（`compileSchema(schema).validate(data)`）。
- 事件导航侧栏：30 事件按 eventsCatalog 十组，点击跳转到对应 JSON 段落（简单文本搜索定位）。
- 集成 `MatcherTester.tsx` 作为浮动/内联工具。
- 非法 JSON 时通过 `onValidationChange` 通知父组件。

### P3-TE-09：L2 测试 — JSON 模式渲染与 Schema 校验

- mock `readHooksConfig`，验证 CM6 EditorView 创建、schema 扩展注册、错误时 onValidationChange 被调用。

### P3-TE-10：L2 测试 — 事件导航

- 验证十大分组事件名渲染、点击后编辑器选区跳到对应事件键位置。

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
- **注入段标记**：`isSltermManaged` 命中条目显示「slTerminal 托管」标记并禁删（照 C13-8）。

### P3-FE-14：实现 `HandlerForm.tsx`

- 根据 `type` 渲染 5 种 handler 专用表单，字段矩阵照 C13-3 官方版：
  - `command`：command*、args、async、asyncRewake、shell + 通用字段（if/timeout/statusMessage）
  - `http`：url*、headers、allowedEnvVars + 通用字段
  - `mcp_tool`：server*、tool*、input + 通用字段
  - `prompt`：prompt*、model、continueOnBlock + 通用字段
  - `agent`：prompt*、model + 通用字段
  - `once` 不展示（settings.json 中无效）。
- 事件 → handler 支持矩阵约束（eventsCatalog 驱动）：仅 command/http/mcp_tool 档事件禁用 prompt/agent；SessionStart/Setup 仅允许 command/mcp_tool。
- **不支持 matcher 的事件**：matcher 组不渲染 matcher 输入框（eventsCatalog 驱动）。
- 切换 type 时保留通用字段、清除不适用的字段。
- **注入段 handler 禁改**：`isSltermManaged` 命中的 handler 表单只读 + 禁删 + 禁禁用（C13-8）。

### P3-TE-11：L2 测试 — handler 表单字段矩阵

- 覆盖：5 种 type 必填字段渲染（官方版字段名断言：mcp_tool 为 `input`、http 无 method/body、agent 无 description/subagent_type）、事件支持矩阵过滤、type 切换清理字段、注入段禁改。

### P3-TE-12：L2 测试 — 事件树结构

- 覆盖：十大分组渲染、hook 计数、选中回调、添加/删除事件、注入段标记与禁删、不支持 matcher 事件无 matcher 输入。

---

## Stage 6 — 双模式同步与保存安全

### P3-FE-16：实现双模式同步

- JSON 合法编辑后实时调用 `jsonToGui` 更新 GUI 模型。
- GUI 编辑后实时调用 `guiToJson` 更新 JSON 文本。
- JSON 非法时禁止切换到 GUI 模式，并在工具栏显示错误提示。
- 两模式共享同一份 `configJson`（hooks 子树）与脏状态。

### P3-FE-17：实现保存安全

- 保存按钮触发：`JSON.parse` 语法校验 → `json-schema-library` schema 校验（hooks 子 schema）→ 任一失败则弹窗提示、拒绝调用 `writeHooksConfig`。
- 校验通过 → `filterDisabled` 剔除禁用条目 → `writeHooksConfig(layer, filtered, projectPath?)`。
- 保存成功后显示提示条：「hooks 改动需重启 claude 会话生效」。
- 不做 `.bak`（与 F6 决策一致）；其他字段保留由后端 merge 保证（P3-BE-03）。

### P3-TE-13：L2 测试 — 双模式同步

- 覆盖：GUI 新增事件 → JSON 文本含该事件；JSON 合法修改 → GUI 树更新；JSON 非法 → 切 GUI 被阻止。

### P3-TE-14：L2 测试 — 保存拒绝与提示

- 覆盖：语法错误保存被拒、Schema 错误保存被拒、合法保存成功显示重启提示、保存调用 payload 为 hooks 子树（键集合精确匹配）。

---

## Stage 7 — 单条启停（ADR-0002）与 F2 并入

### P3-FE-19：实现禁用 UI

- 每条 handler 右侧显示启停 checkbox（注入段条目除外——禁禁用，C13-8）。
- 禁用条目在事件树中视觉区分（如置灰 + 文字删除线）。
- 常驻提示文案：「禁用条目由 slTerminal 托管，不出现在配置文件中」。
- 失效禁用记录：当四元组（layer+event+matcher+command）在配置中找不到匹配时，在 UI 标记为「失效的禁用记录」。

### P3-FE-20：保存时过滤禁用条目

- 在 `useHooksConfig.ts` 的保存逻辑中，先将 `configJson` 经 `filterDisabled(config, disabledHooks)` 剔除禁用条目，再写入磁盘。
- 重新启用时条目按原位置恢复（依赖 `disabledHooks` 记录中保存的完整四元组，保存前重新插回）。

### P3-FE-21：在面板内集成 F2 注入/卸载按钮

- 复用契约 C6 三条命令，前端 wrapper 为 `src/ipc/hooks.ts` 的 **`inject()` / `uninstall()` / `getInjectionStatus()`**（阶段 1 已完成，注意实际命名无 Hooks 后缀）。
- 按钮位置：面板顶部工具栏或独立状态条。
- **注入/卸载操作完成后自动重读 user 层配置**（操作会改写 `~/.claude/settings.json`，C13-8）。

### P3-FE-22：显示注入状态

- 调用 `getInjectionStatus()` 获取状态，显示：已注入/未注入/版本过旧。
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
  - defaultKey: `Ctrl+Shift+H`（执行期实测，若被 WebView2 拦截降级 `Ctrl+Alt+H`）
  - priority: 10
- 默认键须经 `reserved.test.ts` 同模式校验为非保留键。

### P3-FE-24：在 `src/features/shortcuts/globalCommands.ts` 提供 handler（同页单例）

- `createGlobalShortcuts(getDockviewApi)` 现有签名不变，追加第二条命令。
- handler 逻辑：取 `useLayout.getState().activePageId` → 面板 id = **`hooksConfig-{pageId}`**（单例规则，契约 C13-7）→ `getDockviewApi()?.getPanel(id)` 命中则 `focus()` 返回 true；未命中 `addPanel({ id, component: "hooksConfig", title: "Hooks 配置", params: { panelId: id } })`。
- 无活跃页面或无 DockviewApi 时返回 `false` 透传。
- **不存在 `generatePanelId` 函数**——不得引用；id 按上述单例规则拼接。

### P3-FE-25：在 `src/App.tsx` 注册命令

- `App.tsx` 已 `registry.register([...createGlobalShortcuts(...)])`，工厂返回值追加命令后无需修改调用点——仅需确认。

### P3-TE-17：L2 测试 — 入口命令

- 覆盖：`commandCatalog` 含 `global.openHooksConfig`、`createGlobalShortcuts` 返回该命令、handler 首次 `addPanel`、**重复触发 `getPanel` 命中聚焦不新建**、无 api 时透传。
- 同步更新 `src/__tests__/command-catalog.test.ts`：`EXPECTED_IDS` 加入 `global.openHooksConfig`、长度预期改为 10、补充该命令的 `commandFromMeta` 断言。

---

## Stage 9 — L4 E2E 关键路径

### P3-TE-18：L4 测试 — 面板打开与保存链路（走 project 层）

- 用例：`__slterm_e2e_createProject` 创建 tempdir 项目 → 打开 hooksConfig 面板（经 `__dockviewApi.addPanel` 或合成快捷键）→ 切到 **project 层** → JSON 模式输入合法 hooks 配置 → 点击保存 → 断言 `<tempdir>/.claude/settings.json` 真实写盘（mtime + 内容），且预置的其他字段（如 `permissions`）merge 后保留。
- **禁止写 user 层**——不碰真实 `~/.claude/settings.json`（C13-9）。
- 保存按钮用 `.click()` 触发（E2E 键盘输入限制，见 `e2e-tests/CLAUDE.md`）；JSON 输入通过 CM6 helper 或新增 `__slterm_e2e_*` 扩展注入（必要时扩展 `e2e-tests/helpers.ts`，该文件不在根 tsconfig include，本 Stage 门禁须含 `npx vite build` 构建级验证）。

---

## Stage 10 — 文档同步

### P3-DOC-01：更新 `src/panels/CLAUDE.md`

- 在「当前面板类型」与文件清单中加入 `hooksConfig` 面板。
- 描述：双模式编辑（JSON/GUI）、hooks 子树三层配置、单条启停、注入段保护、F2 并入。

### P3-DOC-02：更新 `src/ipc/CLAUDE.md`

- 模块映射表追加 `src/ipc/hooksConfig.ts` ↔ `hooks/`：`hooks_config_read`、`hooks_config_write`。
- 说明与 `src/ipc/hooks.ts` 区分（后者为 C6 注入/事件/用量命令）。

### P3-DOC-03：更新 `src/stores/CLAUDE.md`

- Store 清单追加 `hooksConfig.ts`：禁用状态、`disabledHooks` 段、`loadFromDisk`/`saveToDisk` 模式。

### P3-DOC-04：更新 `src/features/shortcuts/CLAUDE.md`

- 命令目录追加 `global.openHooksConfig`（含同页单例语义）。

### P3-DOC-05：更新 `.claude/test-inventory.md`

- 新增 Phase 3 测试文件与用例数（按实际执行后统计填入）。
- 全量用例数累加更新。

### P3-DOC-06：回查契约一致性

- 对照 `docs/hooks-dev/contract.md` C13 逐项核实最终实现（命令签名、字段矩阵、事件目录、面板 id 规则），偏差修订文档或代码（回议后择一）。
- 确认 `src-tauri/src/hooks/CLAUDE.md` 追加 `config.rs` 文件行与命令说明。

---

## 依赖与开放项汇总

| 依赖 | 说明 |
|------|------|
| 阶段 1 完成 | `src-tauri/src/hooks/mod.rs`、`src/ipc/hooks.ts`（`inject`/`uninstall`/`getInjectionStatus`）、C6 三条命令已存在 |
| SchemaStore JSON | 执行期下载/复制 `claude-code-settings.json` 到 `src/features/hooksConfig/schema/`，并核实自包含性（本地 `$ref`） |
| 新增 npm 依赖 | `codemirror-json-schema`（含底层 `json-schema-library`）、`@codemirror/lint`、`@codemirror/autocomplete`；**不引 ajv** |
| 【待执行期确认】 | `global.openHooksConfig` 默认键（推荐 `Ctrl+Shift+H`，被拦截降级 `Ctrl+Alt+H`） |
| 【执行期核实】 | claude-code-settings.json schema 是否自包含（远程 `$ref` 需预打包） |
