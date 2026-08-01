# Phase 3 Stage 划分 — Hooks 双模式配置面板

> 本文件为执行期唯一 Stage 真值源。各 Stage 脚本 `workflows/stage-NN-*.js` 与验证断言 `workflows/verify/stage-NN.md` 均引用本文件。
> 跨边界契约（IPC 命令名/签名/DTO/事件名）写死于 contract.md C13（2026-07-31 修订版）与本文件头部，并行 agent 不各自推断。
> **2026-07-31 修订**：对照 Phase 2 完成后代码现状 + 官方文档核实 + 8 项用户拍板全面修订（hooks 子树读写 / 官方字段矩阵 / eventsCatalog / 注入段保护 / 同页单例 / E2E 走 project 层 / 失真修正）。

---

## 全局跨边界契约（所有 Stage 共享）

### 新增 IPC 命令（hooks 子树级，契约 C13-1）

| Rust 命令 | 参数 | 返回 | 前端 wrapper |
|-----------|------|------|--------------|
| `hooks_config_read` | `layer: String`, `project_path: Option<String>` | `Result<serde_json::Value, AppError>` — 该层 **hooks 子树**；文件不存在/无 hooks 键 → `null`；JSON 损坏 → `Err` | `src/ipc/hooksConfig.ts` `readHooksConfig(layer, projectPath?)` |
| `hooks_config_write` | `layer: String`, `hooks: serde_json::Value`, `project_path: Option<String>` | `Result<(), AppError>` — 后端 **read-modify-write** 合并 `hooks` 键，保留其他字段；损坏 → `Err` | `src/ipc/hooksConfig.ts` `writeHooksConfig(layer, hooks, projectPath?)` |

- `layer` 仅允许 `"user"` / `"project"` / `"local"`。
- user 层路径：`~/.claude/settings.json`；project 层：`<projectPath>/.claude/settings.json`；local 层：`<projectPath>/.claude/settings.local.json`。
- project/local 层 `project_path` 须经后端 `validate_path_within_root` 沙箱校验；user 层不校验（绕过）。

### 复用契约 C6 命令（F2 并入）——注意实际 wrapper 命名

| Rust 命令 | 前端 wrapper（`src/ipc/hooks.ts`，经 `hooks` namespace 调用） | 用途 |
|-----------|--------------|------|
| `hooks_inject` | `hooks.inject()` | 注入 hook 脚本与配置段 |
| `hooks_uninstall` | `hooks.uninstall()` | 卸载 hook 配置与脚本 |
| `hooks_injection_status` | `hooks.getInjectionStatus()` | 查询注入状态 |

### 面板类型与入口

- 目录：`src/panels/hooksConfig/`；面板类型常量：`PANEL_HOOKS_CONFIG = "hooksConfig"`；组件：`HooksConfigPanel`
- **同页单例**：面板 id = `hooksConfig-{pageId}`；入口 handler 先 `getPanel(id)` 查重 → 命中 `focus()`，未命中 `addPanel`
- 入口命令：id `global.openHooksConfig` / title "打开 Hooks 配置" / context `global` / defaultKey `Ctrl+Shift+H`（执行期实测，被拦截降级 `Ctrl+Alt+H`）/ priority 10
- **代码库不存在 `generatePanelId`**——禁止引用；id 按单例规则拼接

### 事件元数据目录（eventsCatalog 真值源，写死）

30 事件 × 10 组（真值源 `docs/hooks/D2/02-settings-json-schema.md` §4.5）。列：事件 | 分组 | matcher 支持（×=省略 matcher 键） | 匹配目标 | handler 支持（A=全 5 种 / B=command+http+mcp_tool / C=command+mcp_tool）

| # | 事件 | 分组 | matcher | 匹配目标 | handler |
|---|------|------|---------|---------|---------|
| 1 | SessionStart | 会话生命周期 | ✓ | source（startup/resume/clear/compact） | C |
| 2 | SessionEnd | 会话生命周期 | ✓ | reason | B |
| 3 | Setup | 会话生命周期 | ✓ | 触发标志 | C |
| 4 | UserPromptSubmit | 用户交互 | × | — | A |
| 5 | UserPromptExpansion | 用户交互 | ✓ | 命令名称 | A |
| 6 | PreToolUse | 工具调用 | ✓ | 工具名 | A |
| 7 | PermissionRequest | 工具调用 | ✓ | 工具名 | A |
| 8 | PermissionDenied | 工具调用 | ✓ | 工具名 | A |
| 9 | PostToolUse | 工具调用 | ✓ | 工具名 | A |
| 10 | PostToolUseFailure | 工具调用 | ✓ | 工具名 | A |
| 11 | PostToolBatch | 工具调用 | × | — | A |
| 12 | Notification | 通知与消息 | ✓ | notification_type | B |
| 13 | MessageDisplay | 通知与消息 | × | — | B* |
| 14 | SubagentStart | 子代理与任务 | ✓ | 子代理类型名 | B |
| 15 | SubagentStop | 子代理与任务 | ✓ | 子代理类型名 | A |
| 16 | TaskCreated | 子代理与任务 | × | — | A |
| 17 | TaskCompleted | 子代理与任务 | × | — | A |
| 18 | TeammateIdle | 子代理与任务 | × | — | A |
| 19 | PreCompact | 上下文管理 | ✓ | manual/auto | B |
| 20 | PostCompact | 上下文管理 | ✓ | manual/auto | B |
| 21 | Stop | 停止与错误 | × | — | A |
| 22 | StopFailure | 停止与错误 | ✓（窄字符集仅字母/数字/`_`/`\|`） | 错误类型 | B |
| 23 | ConfigChange | 配置与文件变更 | ✓ | 配置来源 | B |
| 24 | CwdChanged | 配置与文件变更 | × | — | B |
| 25 | FileChanged | 配置与文件变更 | ✓（窄字符集仅字母/数字/`_`/`\|`） | 文件名模式（basename） | B |
| 26 | InstructionsLoaded | 配置与文件变更 | ✓ | 加载原因 | B |
| 27 | WorktreeCreate | 工作树 | × | — | B |
| 28 | WorktreeRemove | 工作树 | × | — | B |
| 29 | Elicitation | 启发式交互 | ✓ | MCP 服务器名称 | B |
| 30 | ElicitationResult | 启发式交互 | ✓ | MCP 服务器名称 | B |

> \* MessageDisplay 的 handler 支持档为保守推断（依据 D1 §6.7 默认超时表含 command/http/mcp_tool；prompt/agent 未核实，保守不展示）。执行期若官方文档明确，回改 eventsCatalog 与本表。
> 不支持 matcher 的 10 事件（× 列）：UserPromptSubmit、PostToolBatch、MessageDisplay、TaskCreated、TaskCompleted、TeammateIdle、Stop、CwdChanged、WorktreeCreate、WorktreeRemove——GUI 省略 matcher 输入、`guiToJson` 省略 `matcher` 键但保留数组包裹。

### handler 字段矩阵（官方版，契约 C13-3）

| 类型 | 专有字段（\*=必填） | 通用字段 |
|------|------|------|
| command | `command`\*、`args[]`、`async`、`asyncRewake`、`shell` | `if`（仅工具事件）、`timeout`、`statusMessage` |
| http | `url`\*`、headers{}`、`allowedEnvVars[]` | 同上 |
| mcp_tool | `server`\*`、tool`\*`、`input{}` | 同上 |
| prompt | `prompt`\*`、model`、`continueOnBlock` | 同上 |
| agent | `prompt`\*`、model` | 同上 |

`once` 不展示（settings.json 中无效）；`asyncTimeout` 非配置字段。

### 注入段识别（C13-8）

`isSltermManaged(handler)` = `handler.command` 含 `slterm-hook-reporter` 子串（照 C9 识别规则）。GUI 标记「slTerminal 托管」+ 禁删/禁禁用/表单只读；JSON 模式不限制。

---

## Stage 01 — 后端三层 hooks 子树读写命令

**内容**：实现 `hooks_config_read`（返回 hooks 子树）/ `hooks_config_write`（read-modify-write merge）及 L1 测试。

**ID 列表**：P3-BE-01、P3-BE-02、P3-BE-03、P3-BE-04、P3-BE-05、P3-BE-06、P3-BE-07、P3-BE-08、P3-TE-01、P3-TE-02

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-config | P3-BE-01/02/03/06/07/08 + 单元测试 | `src-tauri/src/hooks/config.rs`（新建） |
| backend-register | P3-BE-04/05 | `src-tauri/src/hooks/mod.rs`、`src-tauri/src/lib.rs` |

*注：两个 agent 文件零重叠；backend-register 导入 config.rs 中的命令函数名（已写死于契约）。*

**实现要点**

- 路径解析与文件 IO 全部在 `spawn_blocking` 内执行（硬约束 #3）。
- read：提取 `hooks` 子树返回；文件不存在/无 hooks 键 → `Ok(Value::Null)`；**JSON 损坏 → `Err`**（不返回 Null，防 merge 丢字段）。
- write：`hooks` 必须为 Object；读原文件（不存在视为 `{}`，损坏 → `Err` 拒绝）→ 替换根对象 `hooks` 键 → `NamedTempFile::new_in()` + `persist` 原子写；不做 `.bak`。
- user 层使用 `dirs::home_dir()`，不经过 `validate_path_within_root`（照 `settings.rs`/`projects.rs` 先例）；project/local 层必须校验。

**验证项**

1. `src-tauri/src/hooks/config.rs` 存在并实现 `hooks_config_read` / `hooks_config_write`，签名为契约写死的 `(layer, project_path)` / `(layer, hooks, project_path)`。
2. read 返回 hooks 子树（非整文件）；文件不存在/无 hooks 键 → Null；损坏 → Err（语义式：须 Read 代码确认损坏路径返回 Err 而非 Null）。
3. write 为 read-modify-write：测试断言文件预置 `permissions`/`env` 字段在写入后原样保留；损坏文件写入被拒。
4. `src-tauri/src/lib.rs` 的 `generate_handler!` 包含两条命令。
5. L1 测试覆盖 user 层读取/写入、project/local 路径解析、沙箱失败分支、原子写、merge 保留、损坏拒绝。
6. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 与 `cargo test --manifest-path src-tauri/Cargo.toml hooks::config -- --test-threads=1` 通过。

**Commit message**

```
feat: 后端 hooks 配置三层读写命令（hooks 子树级 + merge 保留）

test: L1 覆盖路径解析、沙箱、原子写、merge 保留、损坏拒绝
test: 用例数待 Stage 完成后回填 test-inventory

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 无。L1 测试已覆盖真实文件 IO（tempdir 隔离），本 Stage 无 UI/视觉假设。

---

## Stage 02 — IPC 封装、DTO、eventsCatalog、matcher 引擎与 configModel

**内容**：前端 IPC 层、类型定义、事件元数据目录、matcher 纯函数、configModel 双向转换及 L2 测试。

**ID 列表**：P3-FE-05、P3-FE-06、P3-FE-26、P3-FE-08、P3-FE-10、P3-TE-05、P3-TE-06、P3-TE-19

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-ipc | P3-FE-05/06 | `src/ipc/hooksConfig.ts`（新建）、`src/ipc/index.ts`、`src/types/hooksConfig.ts`（新建） |
| frontend-model | P3-FE-26/08/10 + 测试 | `src/panels/hooksConfig/eventsCatalog.ts`（新建）、`src/panels/hooksConfig/matcherEngine.ts`（新建）、`src/panels/hooksConfig/configModel.ts`（新建） |

*注：文件零重叠；`src/ipc/index.ts` 仅追加一行 re-export。*

**实现要点**

- `src/ipc/hooksConfig.ts` 是唯一调用 `invoke` 的位置（硬约束 #1）；write payload 字段名为 `hooks`（非 `content`）。
- `eventsCatalog.ts` 按本文件头部「事件元数据目录」全表写死（30 事件 × 10 组 × matcher 支持 × 匹配目标 × handler 支持档）+ 5 种 handler 字段矩阵常量；纯数据 + 纯查询函数，零 DOM/React。
- `matcherEngine.ts` 必须为纯函数（无 DOM/React/IO），单点供 matcher tester 与测试共用；注释写明版本前提（逗号/空格 v2.1.191+、连字符 v2.1.195+）。
- `configModel.ts`：`jsonToGui` 对非对象/非数组输入降级为空模型；`guiToJson` 对不支持 matcher 的事件省略 `matcher` 键；`isSltermManaged` 按 `slterm-hook-reporter` 子串判定。

**验证项**

1. `src/ipc/hooksConfig.ts` 封装 `hooks_config_read` / `hooks_config_write`，参数名 snake_case ↔ camelCase 转换正确，write payload 键为 `hooks`。
2. `eventsCatalog`：30 事件齐全唯一、10 分组、handler 支持档与本文件头部表一致、10 个无 matcher 事件标记正确、5 种 handler 字段矩阵与契约一致。
3. `matcherEngine` 全表语义测试通过（精确 OR、正则、全匹配、大小写敏感、FileChanged/StopFailure 窄字符集）。
4. `configModel` 双向转换测试覆盖空配置、多事件多 handler、字段缺失、无 matcher 事件省略键、`isSltermManaged`、`filterDisabled`。
5. `npx tsc --noEmit` 与 `npm test` 相关测试通过。

**Commit message**

```
feat: 前端 hooks 配置 IPC 封装 + eventsCatalog + matcher 语义引擎 + 配置模型

test: L2 matcher 语义全表 + configModel 双向转换 + eventsCatalog 常量守卫

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- matcher 语义与 claude 官方行为一致性已官方核实（2026-07-31，含版本前提）；如官方后续放宽，需回改 matcherEngine。
- MessageDisplay 的 handler 支持档为保守推断（见头部表注），执行期若官方明确可回改。

---

## Stage 03 — 面板骨架、注册与数据 hook

**内容**：创建 `src/panels/hooksConfig/` 目录、实现面板骨架、注册面板类型、实现 `useHooksConfig` 与禁用状态 store。

**ID 列表**：P3-FE-01、P3-FE-02、P3-FE-03、P3-FE-04、P3-FE-15、P3-FE-18、P3-FE-18b、P3-TE-07、P3-TE-08

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-store | P3-FE-18 + 测试 | `src/stores/hooksConfig.ts`（新建）、`src/stores/index.ts` |
| frontend-panel-core | P3-FE-01/02/03/04/15/18b + 测试 | `src/panels/hooksConfig/index.ts`、`src/panels/hooksConfig/HooksConfigPanel.tsx`、`src/panels/hooksConfig/useHooksConfig.ts`、`src/panelRegistry.ts`、`src/panels/index.ts`、`src/App.tsx`、`src/__tests__/panel-registry.test.ts` |

*注：文件零重叠；store agent 先完成，panel-core agent 后启动（sequential pipeline）。`App.tsx` 仅增加 `cancelPendingSave` 冲刷调用（照 `cancelSideBarSave` 重命名先例）。*

**实现要点**

- `HooksConfigPanel.tsx` 先渲染占位 UI，顶部工具栏预留：层级切换器、GUI/JSON 模式切换、保存按钮、注入状态条；三态 + 配置损坏错误态（"配置文件损坏，请先修复"）。
- `useHooksConfig` 从 `useProjects` + `useLayout` 推导活跃项目 `rootPath`；rootPath 为空时 project/local 层禁用。
- 加载：null → 空对象 `{}`；切换 layer 前若 dirty 弹窗确认；面板挂载时调用 `useHooksConfigStore.getState().loadFromDisk()`。
- **轻量重读**：切层 / 面板聚焦（focusin）时重新 `readHooksConfig`；dirty 时 dialog.ask 提示，用户确认丢弃才覆盖（照编辑器外部修改先例）。
- `panelRegistry.ts` 追加 `hooksConfig`，`PANEL_TYPES` **末尾**追加；不加入 `FILE_PANEL_TYPES` / `isAlwaysRenderPanel`。
- `src/stores/hooksConfig.ts` 持久化 `disabledHooks` 段到 `~/.slterminal/settings.json`，模式照 `keybindings.ts`。
- 同步更新 `src/__tests__/panel-registry.test.ts`：6 键 / `PANEL_TYPES` 长度 6 / **`toEqual` 精确数组断言末尾追加 `"hooksConfig"`** / 索引断言顺延。

**验证项**

1. `PANEL_TYPES` 包含 `"hooksConfig"`（末尾），`isValidPanelType` 识别；`panel-registry.test.ts` 已同步（6 键 / 长度 6 / 精确数组含 hooksConfig）。
2. `HooksConfigPanel` 渲染三态 + 损坏错误态，工具栏包含层级切换器与模式切换。
3. `useHooksConfig` 在 layer 变化时调用 `readHooksConfig`，rootPath 为空时不调用 project/local 层；挂载时加载 disabledHooks；聚焦/切层触发重读，dirty 时提示。
4. `hooksConfig` store 测试通过：load/sanitize/disable/enable/debounce payload 键集合精确匹配 `{ disabledHooks }`。
5. `App.tsx` 关闭序列调用 hooksConfig 的 `cancelPendingSave`。

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

**内容**：内嵌 SchemaStore JSON（核实自包含性）、实现 `JsonMode.tsx` 与 `MatcherTester.tsx`。

**ID 列表**：P3-FE-07、P3-FE-11、P3-TE-09、P3-TE-10

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-json-mode | P3-FE-07/11 + 测试 | `src/features/hooksConfig/schema/claude-code-settings.json`（新建）、`src/panels/hooksConfig/JsonMode.tsx`（新建）、`src/panels/hooksConfig/MatcherTester.tsx`（新建）、`package.json` + `package-lock.json`（新增依赖）、`src/panels/hooksConfig/HooksConfigPanel.tsx`（接入 JSON 模式） |

*注：单 agent 避免 JsonMode 与 HooksConfigPanel 集成不同步。*

**实现要点**

- 将 SchemaStore `claude-code-settings.json` 复制到 `src/features/hooksConfig/schema/`；**核实自包含性**——`codemirror-json-schema` 仅支持本地 `$ref`，含远程 `$ref` 需预打包展开。
- 提取 `properties.hooks` 子 schema 供编辑器与保存校验使用（对齐 hooks 子树编辑范围）。
- 新增 npm 依赖 `codemirror-json-schema` + `@codemirror/lint` + `@codemirror/autocomplete`（peer deps）+ `json-schema-library`（codemirror-json-schema 底层，Stage 06 保存校验直接 import——一并加入 dependencies 显式声明，不依赖 node_modules 平铺）；**不引 ajv**。
- `JsonMode.tsx` 使用 CM6 + `@codemirror/lang-json` + `jsonCompletion`/`jsonSchemaHover`/`jsonSchemaLinter`；提供 `onValidationChange(isValid, diagnostics)` 回调。
- 事件导航侧栏：30 事件按 eventsCatalog 十组，点击后在编辑器内定位到对应事件键（简单文本搜索 + `setSelection`）。
- `MatcherTester.tsx` 使用 `matcherEngine.ts`，输入 matcher + toolName（+event 感知窄字符集）→ 显示命中结果与匹配模式。

**验证项**

1. `src/features/hooksConfig/schema/claude-code-settings.json` 存在且可被 Vite JSON import；自包含性核实结果记录在代码注释（有无远程 `$ref`）。
2. `JsonMode` 渲染 CM6 EditorView，schema 扩展被注册（hooks 子 schema）。
3. JSON 非法时 `onValidationChange` 返回 `isValid=false`。
4. 事件导航侧栏点击后编辑器选区跳到目标事件键。
5. `package.json` 新增依赖为 `codemirror-json-schema`/`@codemirror/lint`/`@codemirror/autocomplete`，**无 ajv**（语义式：grep `"ajv"` 不出现）。
6. L2 测试通过。

**Commit message**

```
feat: JSON 模式 — CM6 + SchemaStore hooks 子 schema + 事件导航 + matcher 测试工具

test: L2 JSON 模式渲染、校验、事件导航

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实 WebView2 中 schema 补全/悬停文档/错误波浪线是否生效（jsdom 无法验证 CM6 lint 渲染）。
- Schema 版本与线上 claude 的兼容性（schema 滞后问题为已知风险，F6 已接受）；schema 自包含性核实结论。

---

## Stage 05 — GUI 表单模式（Master-Detail）

**内容**：实现 `GuiMode.tsx`、`EventTree.tsx`、`HandlerForm.tsx`。

**ID 列表**：P3-FE-12、P3-FE-13、P3-FE-14、P3-TE-11、P3-TE-12

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-gui-tree | P3-FE-12/13 + 测试 | `src/panels/hooksConfig/GuiMode.tsx`（新建）、`src/panels/hooksConfig/EventTree.tsx`（新建）、`src/panels/hooksConfig/HooksConfigPanel.tsx`（接入 GUI 模式） |
| frontend-handler-form | P3-FE-14 + 测试 | `src/panels/hooksConfig/HandlerForm.tsx`（新建） |

*注：两个 agent 文件零重叠；GuiMode 与 EventTree 共用，由同一 agent 保证一致性。注入段识别 `isSltermManaged` 与事件元数据来自 Stage 02 的 configModel/eventsCatalog（契约已写死，不各自推断）。*

**实现要点**

- `EventTree.tsx` 三级树：事件分组 → 事件 → matcher 组 → handler 摘要；选中态颜色使用 `theme/colors.ts` token（硬约束 #6）；注入段条目标记「slTerminal 托管」并禁删。
- `HandlerForm.tsx` 根据 `type` 渲染 5 种表单，字段矩阵按契约官方版（mcp_tool 为 `input`、http 无 method/body、agent 无 description/subagent_type、`once` 不展示）；事件 → handler 支持矩阵约束可选类型（eventsCatalog 驱动）；不支持 matcher 的事件不渲染 matcher 输入框；注入段 handler 表单只读 + 禁删 + 禁禁用。
- 切换 type 时保留通用字段（如 `timeout`），清除不适用的字段。
- `GuiMode.tsx` 管理 `selectedEvent / selectedMatcherIndex / selectedHandlerIndex`，提供添加/删除事件、matcher、handler 的回调。

**验证项**

1. `EventTree` 渲染十大事件分组与事件名，hook 计数正确；注入段条目有托管标记且删除按钮禁用。
2. `HandlerForm` 5 种 type 必填字段正确渲染（字段名官方版断言）。
3. 事件 → handler 支持矩阵过滤测试通过（SessionStart/Setup 仅 command+mcp_tool；B 档事件无 prompt/agent）。
4. 不支持 matcher 的事件无 matcher 输入框。
5. type 切换时不适用的字段被清除。
6. L2 测试通过。

**Commit message**

```
feat: GUI 模式 — Master-Detail 事件树 + 5 种 handler 专用表单（官方字段矩阵）

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

- 共享状态 `configJson`（hooks 子树）与 `guiModel` 存放于 `useHooksConfig`。
- JSON 合法变更 → 调用 `jsonToGui` → 更新 GUI；GUI 变更 → 调用 `guiToJson` → 更新 JSON 文本。
- JSON 非法时模式切换按钮禁用，并显示错误提示文案。
- 保存流程：`JSON.parse` 语法校验 → `json-schema-library`（`compileSchema(hooksSubSchema).validate(data)`）schema 校验 → `filterDisabled` 剔除禁用条目 → `writeHooksConfig(layer, filtered, projectPath?)`。
- 保存成功后显示提示条：「hooks 改动需重启 claude 会话生效」。

**验证项**

1. GUI 新增事件后 JSON 文本同步更新。
2. JSON 合法修改后 GUI 树同步更新。
3. JSON 非法时无法切换到 GUI 模式。
4. 语法错误 / schema 错误保存被拒绝。
5. 合法保存成功后显示重启提示；`writeHooksConfig` 调用 payload 为 hooks 子树（键集合精确匹配 `{ layer, hooks, projectPath? }`）。
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
| frontend-f2 | P3-FE-21/22 + 测试 | `src/panels/hooksConfig/HooksConfigPanel.tsx`（注入按钮/状态条；复用 `src/ipc/hooks.ts`，不改动） |

*注：`HooksConfigPanel.tsx` 被两个 agent 共享，本 Stage 内使用 sequential pipeline：frontend-disable 完成后 frontend-f2 启动。*

**实现要点**

- 禁用四元组：`{ layer, event, matcher, command }`；禁用状态存 `~/.slterminal/settings.json` 的 `disabledHooks` 段（ADR-0002）。
- 保存时从 `configJson` 中剔除匹配四元组的条目再写盘；重新启用时按四元组插回原位置（原位置因外部修改不存在则标记失效）。
- 面板顶部常驻提示：「禁用条目由 slTerminal 托管，不出现在配置文件中」。
- 注入段条目（`isSltermManaged`）不渲染禁用 checkbox（C13-8 禁禁用）。
- 注入/卸载按钮复用 `hooks.inject()` / `hooks.uninstall()`（**实际命名无 Hooks 后缀**）；状态条经 `hooks.getInjectionStatus()` 显示 injected / notInjected / outdated。
- **inject/uninstall 完成后自动重读 user 层配置**（操作改写 `~/.claude/settings.json`）。

**验证项**

1. 禁用条目保存时从 `writeHooksConfig` 的 hooks 中剔除。
2. 重新启用后条目回到配置。
3. 外部修改导致四元组失配时 UI 显示「失效的禁用记录」。
4. 注入/卸载按钮调用 `hooks_inject` / `hooks_uninstall`（经 `hooks.inject()`/`hooks.uninstall()` wrapper）；操作后触发 user 层重读。
5. 状态条根据 `getInjectionStatus()` 显示 injected/notInjected/outdated。
6. 注入段条目无禁用 checkbox。
7. L2 测试通过。

**Commit message**

```
feat: 单条 hook 启停（ADR-0002）+ F2 注入/卸载并入面板

test: L2 禁用往返与失效禁用记录

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 真实场景：在 slTerminal 外修改 `.claude/settings.json` 后回到面板（聚焦重读），失效禁用记录是否准确。
- 注入/卸载操作对真实 claude 配置的影响（最终由阶段 3 端到端人工验证兜底）。

---

## Stage 08 — 面板入口命令

**内容**：新增 `global.openHooksConfig` 全局命令，绑定默认键并注册（同页单例）。

**ID 列表**：P3-FE-23、P3-FE-24、P3-FE-25、P3-TE-17

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-shortcut | P3-FE-23/24/25 + 测试 | `src/features/shortcuts/commandCatalog.ts`、`src/features/shortcuts/globalCommands.ts`、`src/__tests__/command-catalog.test.ts`（`src/App.tsx` 仅确认无需改动） |

**实现要点**

- 在 `COMMAND_CATALOG` 追加 `global.openHooksConfig`，默认键 `Ctrl+Shift+H`（执行期实测，被拦截降级 `Ctrl+Alt+H`）。
- `createGlobalShortcuts(getDockviewApi)` 追加 handler：id = `hooksConfig-{activePageId}` → `getPanel(id)` 命中 `focus()`；未命中 `addPanel({ id, component: "hooksConfig", title: "Hooks 配置", params: { panelId: id } })`；无页面/无 api 返回 `false`。
- **禁止引用 `generatePanelId`**（不存在）。
- 同步更新 `src/__tests__/command-catalog.test.ts`：`EXPECTED_IDS` 加入 `global.openHooksConfig`、长度预期改为 10、补充元数据与默认键非保留键断言。

**验证项**

1. `COMMAND_CATALOG` 包含 `global.openHooksConfig`，元数据完整。
2. `createGlobalShortcuts` 返回该命令；handler 首次 `addPanel({ component: "hooksConfig" })`、重复触发 `getPanel` 命中聚焦不新建。
3. 无 DockviewApi / 无活跃页面时 handler 返回 `false` 透传。
4. `command-catalog.test.ts` 已同步（EXPECTED_IDS 10 条 + 该命令断言）。
5. L2 测试通过。

**Commit message**

```
feat: 全局快捷键打开 Hooks 配置面板（同页单例）

test: L2 openHooksConfig 命令注册与 handler

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- 默认键 `Ctrl+Shift+H` 在真实 WebView2 中是否被浏览器/Tauri 默认行为拦截；如拦截则执行期改用 `Ctrl+Alt+H`。

---

## Stage 09 — L4 E2E 关键路径

**内容**：补充 E2E 用例，验证面板打开与真实保存链路（project 层）。

**ID 列表**：P3-TE-18

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| e2e-tests | P3-TE-18 | `e2e-tests/test.e2e.ts`（必要时扩展 `e2e-tests/helpers.ts`） |

**实现要点**

- 用例：`__slterm_e2e_createProject`（tempdir 项目）→ 打开 hooksConfig 面板 → 切到 **project 层** → JSON 模式写入合法 hooks 配置（含预置 `permissions` 等其他字段的场景）→ 点击保存 → 断言 `<tempdir>/.claude/settings.json` mtime 更新 + hooks 内容正确 + **其他字段 merge 后保留**。
- **禁止写 user 层**（C13-9：不碰真实 `~/.claude/settings.json`）。
- 保存按钮使用 `.click()`；JSON 文本输入通过 CM6 helper 或新增 `__slterm_e2e_*` helper（如需要）。
- 本 Stage 仅追加 L4 用例，不改生产代码。

**验证项**

1. `npm run build:e2e` 成功。
2. `npm run wdio` 新增用例通过。
3. 用例中断言目标文件为 tempdir 项目内 `.claude/settings.json`（语义式：不出现 home 目录 user 层路径）。

**Commit message**

```
test: L4 E2E hooks 配置面板打开与保存链路（project 层）

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

**人工验证点**

- L4 必须在真实 Windows + WebView2 环境运行；本 Stage 本身即为人工/端到端验证。

---

## Stage 10 — 文档同步

**内容**：更新各子路径 CLAUDE.md、test-inventory.md，回查契约一致性。

**ID 列表**：P3-DOC-01、P3-DOC-02、P3-DOC-03、P3-DOC-04、P3-DOC-05、P3-DOC-06

**Agent 文件分工表**

| label | 负责项 | 文件 |
|-------|--------|------|
| docs-update | P3-DOC-01/02/03/04/05/06 | `src/panels/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src/stores/CLAUDE.md`、`src/features/shortcuts/CLAUDE.md`、`src-tauri/src/hooks/CLAUDE.md`、`.claude/test-inventory.md`、`docs/hooks-dev/contract.md`（仅 P3-DOC-06 回查时） |

**实现要点**

- 按各模块 CLAUDE.md 的既有格式追加 `hooksConfig` 相关条目，不在根 CLAUDE.md 展开细节。
- `src-tauri/src/hooks/CLAUDE.md` 追加 `config.rs` 文件行与两条命令说明（hooks 子树读写语义）。
- `test-inventory.md` 新增 Phase 3 测试文件与用例数，并更新全量总数。
- P3-DOC-06：对照 contract.md C13 逐项核实最终实现（命令签名/字段矩阵/事件目录/面板 id 规则），偏差回议后修订文档或代码。
- 文档描述须与 Stage 完成后的真实代码一致（不可照抄计划草案）。

**验证项**

1. 6 份文档均已更新且与代码一致。
2. `test-inventory.md` 新增 Phase 3 文件/用例数，全量总数正确。
3. 文档中 IPC 命令名、面板类型名、store 字段名与代码一致（grep 核对）。
4. C13 各项与最终实现一致（逐项核对记录）。

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
Stage 02 ─┼─> Stage 03 ─> Stage 04 ─> Stage 06 ─> Stage 07 ─> Stage 09 ─> Stage 10
          │                │                          ↑
          │                └─> Stage 05 ──────────────┘
          └─> Stage 08 ────────────────────────────────────↑
```

- Stage 01/02 可并行启动。
- Stage 03 依赖 Stage 01/02 完成（IPC 与后端命令就绪）。
- Stage 04/05 依赖 Stage 03，可并行（修改不同文件）。
- Stage 06 依赖 Stage 04/05。
- Stage 07 依赖 Stage 06。
- Stage 08 依赖 Stage 03（面板已注册）+ Stage 02（无直接文件依赖，可随 03 后任意点）。
- Stage 09 依赖所有代码 Stage 完成。
- Stage 10 固定最后。

---

## 门禁命令汇总

每 Stage 的全量测试 agent 执行以下命令子集（按本 Stage 触碰文件选择）：

| Stage | 门禁命令 |
|-------|----------|
| 01 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`cargo test --manifest-path src-tauri/Cargo.toml hooks::config -- --test-threads=1` |
| 02-08 | `npx tsc --noEmit`、`npx eslint src/`、`npm test -- <本 Stage 测试文件 filter>`（filter 按 execution-plan 命名约定表，非全量 L2） |
| 09 | `npm run build:e2e`、`npm run wdio`；若改动 `e2e-tests/helpers.ts` 追加 `npx vite build`（该文件不在根 tsconfig include，需构建级门禁兜底） |
| 10 | 仅文档检查，无代码门禁 |

*注：Stage 09 不跑 L1/L2；Stage 10 不跑测试。*
