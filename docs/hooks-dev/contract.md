# Hooks 宿主侧增强 — 跨阶段共享契约（唯一真值源）

> 本文件由主 agent 在三个阶段开发计划制定前写死。三个阶段的 checklist/stages/workflow 脚本引用本契约，**不得各自推断、不得偏离**；确需偏离时回议并修订本文件。
> 依据：`docs/hooks-dev/feature-plan/`（14 轮 grilling 拍板）+ 2026-07-26 四项补充决策（见下）。
> 代码事实均已一手核实（`file:line` 标注）。

## 已确认决策（2026-07-26 补充）

| # | 决策点 | 结论 |
|---|--------|------|
| D1 | hook 脚本形态 | **Node.js 单文件脚本**。claude 用户必有 node（claude 经 npm 安装）；JSON stdin/stdout 处理天然；不受 PowerShell ExecutionPolicy 限制 |
| D2 | feature-plan【推导默认】条目 | 直接采纳为既定决策，不再逐项复核 |
| D3 | 测试范围 | L1/L2 全纳入 + L4 仅关键路径 + 人工验证点单列；L3 不涉及（无 xterm 渲染变更） |
| D4 | 计划落盘目录 | `docs/hooks-dev/phase1/`、`phase2/`、`phase3/` |

---

## C1 信号 JSON 字段契约

hook 脚本每条事件写一个信号（JSON），字段集**写死**（8 字段，camelCase——脚本为 JS 产出，后端 serde 按 camelCase 解析，前端 DTO 同名）：

| 字段 | 类型 | 来源 | 说明 |
|------|------|------|------|
| `panelId` | string | 环境变量 `SLTERM_PANEL_ID` | 页签路由，**必须**；缺失时该条事件丢弃并写日志 |
| `event` | string | stdin JSON `hook_event_name` | 四态映射依据，取值为 C9 的 10 事件之一 |
| `timestamp` | number | 脚本生成（`Date.now()`，毫秒） | 排序/去重 |
| `sessionId` | string | stdin JSON `session_id` | 会话标识 |
| `transcriptPath` | string | stdin JSON `transcript_path` | 阶段 2 定位 transcript JSONL |
| `cwd` | string | stdin JSON `cwd` | 辅助信息（项目归属校验） |
| `toolName` | string? | stdin JSON `tool_name`（仅工具事件） | 预留，可缺省 |
| `notificationType` | string? | stdin JSON `notification_type`（仅 Notification） | 区分权限请求/空闲/其他，可缺省 |

## C2 信号目录

- 路径：`~/.slterminal/hooks-events/`（`dirs::home_dir()` 解析，依赖已有 `dirs = "6"`，`src-tauri/Cargo.toml:45`）。
- **不放进 exe 同级**：hook 脚本与信号目录均被用户全局配置（`~/.claude/settings.json`）引用，exe 可移动会导致路径悬空。注意本项目 `settings.json` 实际位于 exe 同级（`settings.rs:10` `app_data_dir()`），hooks 相关目录是**另一套位置**（home 下），两处不混。
- 信号并发策略（单事件单文件 or JSONL 追加）留**阶段 1 计划**定，两备选须在 checklist 中显式决策并写理由；契约只定：同页签高频事件写入不得相互损坏，后端读取容忍半写文件（解析失败跳过+日志，不 panic）。

## C3 SLTERM_PANEL_ID 注入点

- 位置：`src-tauri/src/pty/spawn.rs` `pty_spawn` 的 `extra_envs`（当前 3 变量，`spawn.rs:790-794`）追加第 4 个 `("SLTERM_PANEL_ID", request.panel_id)`。
- **非 Windows fallback 路径同步注入**（`spawn.rs:854-856` 的 `cmd.env(...)` 三处同款）。
- panelId 取值来源：`pty_spawn` 的 `request.panel_id`（前端 `TerminalRegistry` 的 Map 键，格式 `terminal-{pageId}-{seq}`）——语义式约束：不接受字面量/推导值。
- 与 COLORTERM/TERM/TERM_PROGRAM 同一时机（spawn 阶段环境块），不加 shell 类型判断。

## C4 后端 hooks 模块

- 新建 `src-tauri/src/hooks/`（硬约束 #2 按功能分模块，不塞进 notify/pty）。建议文件（阶段 1 可调整）：`mod.rs`（模块入口 + watcher 管理 + Tauri 命令）、`inject.rs`（注入/卸载/状态检测）、`signal.rs`（信号解析纯函数）。
- **信号目录监听**：模块内部自管理 watcher（复用 `notify`/`notify-debouncer-full` 依赖， crates 已在），**不经 `notify_watch` 命令、不经 `validate_path_within_root`**——监听路径是固定的 home 常量目录，非用户输入，沙箱不适用。
- **注入/卸载命令读写 `~/.claude/settings.json`**：走 home 目录专属路径解析（`dirs::home_dir()`），**绕过 project_root 路径沙箱**（照 `settings.rs`/`projects.rs` 先例）。
- 所有 Tauri 命令：`lib.rs` `generate_handler!` 注册、返回 `Result<_, AppError>`、阻塞 I/O 用 `spawn_blocking`（硬约束 #3）。
- 事件推送：Tauri Event `hook-event`（`app_handle.emit`，照 `fs-event` 先例），payload 为 C6 DTO。
- 平台分支：`#[cfg(windows)]` 如确需出现只在本模块明确处，业务逻辑不撒 cfg（硬约束 #9）。

## C5 前端 IPC 层

- 新建 `src/ipc/hooks.ts`（invoke 单点，硬约束 #1），在 `src/ipc/index.ts` barrel export。
- 后端模块与 IPC 文件一一对应：`hooks.ts` ↔ `src-tauri/src/hooks/`。
- 事件封装：`onHookEvent(cb)` 封装 `listen<HookEventPayload>("hook-event")`，返回 unsubscribe（照 `onFsEvent` 模式）。

## C6 IPC 命令与 DTO（写死）

| 命令（Rust snake_case） | 参数 | 返回 | 用途 |
|------|------|------|------|
| `hooks_inject` | 无 | `Result<HookInjectionStatus, AppError>` | 落盘脚本 + merge 注入 user 层 settings.json，返回注入后状态 |
| `hooks_uninstall` | 无 | `Result<(), AppError>` | 移除配置段 + 删脚本目录 + 清信号目录 |
| `hooks_injection_status` | 无 | `Result<HookInjectionStatus, AppError>` | 查询注入状态（面板/入口显示用） |

DTO（Rust `snake_case` ↔ JS `camelCase` 双边对应，硬约束 #4）：

```jsonc
// HookInjectionStatus
{ "status": "injected" | "notInjected" | "outdated",  // serde camelCase 枚举
  "version": number | null }                           // 已注入脚本版本（未注入为 null）

// HookEventPayload（hook-event 事件 payload = C1 字段集原样透传）
{ "panelId": string, "event": string, "timestamp": number,
  "sessionId": string, "transcriptPath": string, "cwd": string,
  "toolName": string | null, "notificationType": string | null }
```

## C7 四态状态机与 emoji 单点

- 四态：`working` ⚡ / `attention` 🟡 / `done` ✅ / `error` ❌；无图标 = 默认（非 claude/已退出）。
- **映射单点**：新建 `src/lib/claudeStatus.ts`——事件名 → 四态的映射纯函数 + emoji 常量。阶段 1（页签图标）与阶段 2（F5 视图行图标）共用，禁止两处各写映射表。
- 状态机完整表照 `feature-plan/phase1-status-core.md` F3 节（含：OSC 133 C 启动→🟡、OSC 133 D 退出→无图标降级路径；Notification 三类 type→🟡、其他 type 不改变状态；事件驱动覆盖、无定时器）。
- 生命周期 = 事件驱动覆盖；后台页面/非聚焦页签同样实时更新。

## C8 tabRules 删除范围（阶段 1）

- **删除**：`src/panels/terminal/tabRules.ts` 中 claude 规则的自定义**图标**切换（注册项的 icon 部分）。
- **保留**：标题切换（claude 运行时页签标题仍变规则标题）；`useCommandDetection` 的 OSC 133 C/D 检测机制（改作四态启动/退出触发器）。
- `DefaultTab` 的 `params.tabIcon` 渲染机制保留（四态图标复用同一通道，经 `api.updateParameters({ tabIcon })`）。

## C9 注入配置段契约（F2）

- 位置：user 层 `~/.claude/settings.json`（仅此一层）。
- 10 事件（写死）：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`Stop`、`StopFailure`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`PermissionRequest`。
- 每事件一个 matcher 组：`{ "matcher": "", "hooks": [{ "type": "command", "command": "node \"<脚本绝对路径>\"", "timeout": 5 }] }`。matcher 为空串 = 匹配全部；`timeout: 5`（秒）写死。
- **识别与幂等**：slTerminal 配置段按 `command` 含脚本文件名（`slterm-hook-reporter`）子串精确识别；注入幂等（已存在则替换为当前版本，不重复追加）；用户既有任何配置段原样保留。
- **版本过旧判定**：比对磁盘脚本与内嵌模板（内容 hash 或版本常量比对，实现留阶段 1 定），不一致 → `outdated`。
- **非法中止**：settings.json 本身非法（JSON 语法错误）时注入中止并报错提示，不强行改写。
- **卸载**：移除全部含标记的 matcher 组（空数组事件键清理）+ 删 `~/.slterminal/hooks/` + 清空 `~/.slterminal/hooks-events/`。
- **不写非标准字段**进 settings.json（ADR-0002 理由：schema 校验/污染）。

## C10 Node hook 脚本契约（F1）

- 路径：`~/.slterminal/hooks/slterm-hook-reporter.js`（注入时由后端从内嵌模板写盘；模板随 slTerminal 发布更新）。
- 行为：读 stdin 全部 → `JSON.parse` → 按 C1 组装信号 → 写入 C2 目录。
- **任何代码路径 exit code 恒为 0**：stdin 为空/JSON 解析失败/无 `SLTERM_PANEL_ID`/信号目录不存在或不可写/写文件异常——全部 `process.exit(0)` 静默退出，绝不向 stderr 输出（exit 2 会阻断 claude，stderr 污染界面）。
- 脚本含版本常量（如 `SCRIPT_VERSION = 1`），供 C9 版本过旧检测。
- 不经网络、不读其他文件、单文件零依赖（Node ≥ 18 内置 API 足够）。

## C11 阶段间依赖

- 阶段 2/3 计划引用本契约 + 阶段 1 产出物（`src-tauri/src/hooks/`、`src/ipc/hooks.ts`、`src/lib/claudeStatus.ts`），计划中标注"前置：阶段 1 完成"，**不重复定义**契约内容。
- 阶段 3 的 F2 入口并入 = 复用 C6 三条命令，不新增注入相关命令。

## C12 阶段 2 专有契约（F4/F5）

> 2026-07-26 回填修订：经阶段 2 计划期对 Tauri v2 源码一手核实，`sendNotification` 的 `Options` 无 `onClick` 字段、JS 侧无 `flashFrame` API，原表述已更正为下述实现路径。
> 2026-07-31 对账修订：阶段 2 fix 后 toast 改走 Tauri 原生 `sendNotification`（无点击路由），原 `sendClickableNotification` / `new Notification()` 路径已废弃——未打包 Win32 WebView2 无 AUMID，Web Notification API 为残缺 shim（`phase2/review-findings.md` 不符合项 #2）。

- **F4 通知**：`@tauri-apps/plugin-notification`（官方插件，`capabilities/` 显式放行，硬约束 #10；thin wrapper 聚合进 `src/ipc/`，照 clipboard/dialog 先例）。toast 经 **Tauri 原生 `sendNotification` 通道**（`src/ipc/notification.ts` 的 `sendToastNotification(title, {body})` 工厂）——**无 onClick 点击路由**（banner 可能被系统抑制仅进通知中心；回窗引导由任务栏闪烁承担）。任务栏闪烁经 **`getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`**（三类事件均闪烁），聚焦后以 `requestUserAttention(null)` 停止。失焦门控、三类事件照 `feature-plan/phase2-notify-overview.md` F4 节（toast 点击跳转页签的诉求已放弃）。
- **F5 上下文用量**：transcript JSONL **在后端解析**——文件可达数百 MB，前端不直接读。hooks 模块新增命令 **`hooks_context_usage`**（参数 `{ transcriptPath: string }`，返回 `ContextUsage | null`，其中 `ContextUsage` 四字段：`inputTokens: number`（输入 token）、`outputTokens: number`（输出 token，信息字段不计占用）、`cacheReadInputTokens: number`（缓存读取输入 token，serde default 兼容旧 transcript 缺失，缺省 0）、`cacheCreationInputTokens: number`（缓存创建输入 token，同上缺省 0）；实现：尾部读取（最后 64KB）+ 逆行扫描最后一条 `message.usage`，失败返回 null 降级）。**用量口径**：总占用 = `inputTokens + cacheReadInputTokens + cacheCreationInputTokens`，上限 200_000（`CLAUDE_CONTEXT_LIMIT`）；`outputTokens` 不计占用保留为信息字段。
- F5 视图注册：`sideViewDefs.ts` 追加（id `agent-status`、title "Agent 状态"、icon 🤖、默认上区）。

## C13 阶段 3 专有契约（F6）

> 2026-07-31 全量修订：Phase 2 完成后对照代码现状 + 官方文档核实（3 路并行检索，证据见 `docs/hooks/D1`/`D2` 与检索报告），8 项决策经用户拍板。本节为阶段 3 唯一契约真值源。

### C13-1 编辑范围与后端命令

- **编辑范围**：面板只编辑 settings.json 的 **`hooks` 子树**（feature-plan「明确不做 hooks 之外字段」）。
- **后端命令**（`src-tauri/src/hooks/config.rs`，`lib.rs` 注册）：

| 命令 | 参数 | 返回 | 语义 |
|------|------|------|------|
| `hooks_config_read` | `layer: String`, `project_path: Option<String>` | `Result<serde_json::Value, AppError>` | 返回该层 `hooks` 子树；文件不存在或无 `hooks` 键 → `Ok(Value::Null)`；**JSON 损坏 → `Err`**（防止后续 merge 丢其他字段，对齐 C9 非法中止先例） |
| `hooks_config_write` | `layer: String`, `hooks: serde_json::Value`, `project_path: Option<String>` | `Result<(), AppError>` | `hooks` 必须为 Object；后端 **read-modify-write**（读原文件 → 替换/插入 `hooks` 键 → 原子写），原样保留其他字段；原文件损坏 → `Err` 拒绝 |

- `layer` 仅 `"user"` / `"project"` / `"local"`；非法走 `AppError::Validation`。
- user 层 = `~/.claude/settings.json`（`dirs::home_dir()`，绕过沙箱，照 `settings.rs`/`projects.rs` 先例）；project/local 层 `project_path` 经 `validate_path_within_root` 沙箱校验后拼接 `.claude/settings.json` / `.claude/settings.local.json`。
- 原子写：`NamedTempFile::new_in()` + `persist`（照 `settings.rs` 先例），不做 `.bak`。
- 前端 wrapper：`src/ipc/hooksConfig.ts` `readHooksConfig(layer, projectPath?)` / `writeHooksConfig(layer, hooks, projectPath?)`（`layer: "user" | "project" | "local"`）。

### C13-2 事件清单与分组

30 事件 × 10 组，以 `docs/hooks/D2/02-settings-json-schema.md` §4.5 为真值源，全表写死于 `phase3/stages.md` Stage 02（feature-plan 原「九大分组」表述已修订）。事件元数据（分组/matcher 支持/匹配目标/handler 支持矩阵）集中于前端单点 `eventsCatalog.ts`，供 EventTree / HandlerForm / JsonMode 导航 / MatcherTester 共用。

### C13-3 handler 字段矩阵（2026-07-31 官方文档核实，替代 feature-plan 原表）

| 类型 | 字段（\* = 必填） |
|------|------|
| `command` | `command`\*、`args[]`、`async`、`asyncRewake`、`shell` + 通用字段 |
| `http` | `url`\*`、headers{}`、`allowedEnvVars[]` + 通用字段（**无 method/body**——固定 POST，body 恒为事件 JSON） |
| `mcp_tool` | `server`\*`、tool`\*`、`input{}` + 通用字段（字段名是 **`input`** 非 `args`） |
| `prompt` | `prompt`\*`、model`、`continueOnBlock` + 通用字段 |
| `agent` | `prompt`\*`、model` + 通用字段（**无 description/subagent_type**——那是内置 Agent 工具的输入参数，非 hook handler 字段） |
| 通用 | `if`（仅工具事件求值：PreToolUse/PostToolUse/PostToolUseFailure/PermissionRequest/PermissionDenied）、`timeout`、`statusMessage`、`once`（仅 skill frontmatter 生效，settings.json 中忽略——**GUI 不展示**） |

- `asyncTimeout` 是异步执行的**返回值字段**，不是配置字段（D1 §6.1 原记载已修正）。
- 官方核实记录：`docs/hooks/D1/01-hooks-official-docs.md` §6 已同步修订。

### C13-4 事件 → handler 支持矩阵（官方核实，与 feature-plan 一致）

- **全 5 种**：`PermissionDenied`、`PermissionRequest`、`PostToolBatch`、`PostToolUse`、`PostToolUseFailure`、`PreToolUse`、`Stop`、`SubagentStop`、`TaskCompleted`、`TaskCreated`、`TeammateIdle`、`UserPromptExpansion`、`UserPromptSubmit`
- **仅 command/http/mcp_tool**：`ConfigChange`、`CwdChanged`、`Elicitation`、`ElicitationResult`、`FileChanged`、`InstructionsLoaded`、`Notification`、`PostCompact`、`PreCompact`、`SessionEnd`、`StopFailure`、`SubagentStart`、`WorktreeCreate`、`WorktreeRemove`
- **仅 command/mcp_tool**：`SessionStart`、`Setup`

### C13-5 matcher 语义

照 feature-plan matcher 语义表（官方核实全真：窄字符集→精确匹配 OR / 其他→JS 正则非锚定 / `*`、`""`、省略→全匹配 / 大小写敏感 / FileChanged、StopFailure 窄字符集仅字母/数字/`_`/`|`）。**版本前提**写入 matcherEngine 注释：逗号/空格分隔需 claude v2.1.191+、连字符需 v2.1.195+。语义引擎为纯函数，单点定义供测试工具与保存校验共用。

**不支持 matcher 的事件**（GUI 省略 matcher 输入、保存时省略 `matcher` 键但保留数组包裹）：`UserPromptSubmit`、`PostToolBatch`、`Stop`、`TeammateIdle`、`TaskCreated`、`TaskCompleted`、`WorktreeCreate`、`WorktreeRemove`、`MessageDisplay`、`CwdChanged`。

### C13-6 Schema 与校验栈

- Schema 内嵌：SchemaStore `claude-code-settings.json` 随 slTerminal 打包（Vite import JSON），位置 `src/features/hooksConfig/schema/`；JSON 模式使用其 `properties.hooks` **子 schema**（对齐 C13-1 编辑范围）。**执行期核实 schema 是否自包含**——`codemirror-json-schema` 仅支持本地 `$ref`，若有远程 `$ref` 需预打包。
- 校验栈：`codemirror-json-schema`（补全 `jsonCompletion` / 悬停 `jsonSchemaHover` / 波浪线 `jsonSchemaLinter`）+ 其底层 `json-schema-library` 做保存前独立校验（**`new Draft07(schema).validate(data)`**——2026-08-01 执行期确认 `json-schema-library@9.3.5` 无 `compileSchema` 导出，真实 API 为 `Draft07` 类构造 + `.validate`，见 `src/features/hooksConfig/schema/index.ts`）——**不引 ajv**。新增依赖：`codemirror-json-schema`、`@codemirror/lint`、`@codemirror/autocomplete`（当前 package.json 缺后两者；`@codemirror/lang-json` 已有）。

### C13-7 面板与入口

- 面板注册：硬约束 #5 全流程——`src/panels/hooksConfig/` → `src/panelRegistry.ts` 注册 → `PANEL_TYPES` 追加；常量 `PANEL_HOOKS_CONFIG = "hooksConfig"`；不加入 `FILE_PANEL_TYPES` / `isAlwaysRenderPanel`。
- **同页单例**：面板 id 规则 `hooksConfig-{pageId}`；入口命令 handler 先 `getPanel(id)` 查重 → 命中则 `focus()`，未命中才 `addPanel`。
- 入口命令：`global.openHooksConfig`，context `global`，defaultKey `Ctrl+Shift+H`（执行期实测，若被 WebView2 拦截降级 `Ctrl+Alt+H`），priority 10。

### C13-8 注入段保护与外部修改

- **注入段保护**：`command` 含 `slterm-hook-reporter` 子串的条目（识别规则照 C9）在 GUI 标记「slTerminal 托管」并**禁删/禁禁用**；JSON 模式不限制（用户对自己文件有最终权利）；面板内 inject/uninstall 操作后自动重读 user 层。
- **外部修改（轻量策略）**：切层 / 面板聚焦时重读配置；有未保存修改（dirty）时提示，不做 fs-event 监听（user 层文件不在 `notify_watch` 沙箱内）。
- **单条启停**：禁用状态存 slTerminal 侧 settings（ADR-0002），四元组（层级+事件+matcher+command）标识；保存时 `filterDisabled` 剔除后写盘；失配记录 UI 标记「失效的禁用记录」。
- **保存安全**：JSON + Schema 双校验不过拒绝保存；原子写；保存后提示「需重启 claude 会话生效」。

### C13-9 E2E 约束

- 面板保存链路 E2E 走 **project/local 层**（tempdir 项目），不碰真实 `~/.claude/settings.json`。
- 现有 `__slterm_e2e_injectHooks` / `__slterm_e2e_uninstallHooks` / `__slterm_e2e_getHookInjectionStatus` helpers **保留**（L4 hooks 注入/卸载用例依赖）。

---

## 留给各阶段计划的开放项（subagent 可定，不属契约）

| 开放项 | 归属 | 备注 |
|--------|------|------|
| 信号并发策略（单文件 vs JSONL 追加） | 阶段 1 | checklist 中显式决策写理由 |
| hooks 模块内部文件划分 | 阶段 1 | C4 为建议结构 |
| 版本过旧比对实现（hash vs 常量） | 阶段 1 | C9 只定原则 |
| 阶段 1 L4 关键路径用例选取 | 阶段 1 | 建议页签图标流转 |
| `hooks_context_usage` 命令名与 DTO 字段 | 阶段 2 | 定后回填 C12 |
| F4 toast 文案格式 | 阶段 2 | phase2 文件有推导默认 |
| 面板类型名/目录名 | ~~阶段 3~~ | **已定（2026-07-31）**：`hooksConfig` / `src/panels/hooksConfig/`（C13-7） |
| schema 内嵌具体路径 | ~~阶段 3~~ | **已定（2026-07-31）**：`src/features/hooksConfig/schema/claude-code-settings.json`（C13-6） |
| 各阶段 Stage 划分与 ID 编号 | 各阶段 | 建议前缀 `P{N}-BE/FE/TE/DOC` |

## 人工验证点（各阶段 stages 必须含对应段）

- **阶段 1**：真实 claude 全状态机走查（启动🟡→⚡→🟡→⚡→✅→退出无图标）；Windows Terminal 启动 claude 无信号文件且行为无异常；删除信号目录后 claude 无报错；#23554 SessionStart 冻结最新版回归
- **阶段 2**：真实 toast 出现/点击跳转页签；窗口聚焦时零通知；注意态任务栏闪烁启停
- **阶段 3**：GUI 配置 PreToolUse 拦截 hook → 重启 claude → 真实生效
