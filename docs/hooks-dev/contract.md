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

- **F4 通知**：`@tauri-apps/plugin-notification`（官方插件，`capabilities/` 显式放行，硬约束 #10；thin wrapper 聚合进 `src/ipc/`，照 clipboard/dialog 先例）。toast 点击回调经 **`sendClickableNotification` 工厂**（内部 `new Notification(...) + n.onclick = ...`）。任务栏闪烁经 **`getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`**，聚焦后以 `requestUserAttention(null)` 停止。失焦门控、三类事件、toast 点击跳转照 `feature-plan/phase2-notify-overview.md` F4 节。
- **F5 上下文用量**：transcript JSONL **在后端解析**——文件可达数百 MB，前端不直接读。hooks 模块新增命令 **`hooks_context_usage`**（参数 `{ transcriptPath: string }`，返回 `ContextUsage | null`，其中 `ContextUsage { inputTokens: number, outputTokens: number }`；实现：尾部读取（最后 64KB）+ 逆行扫描最后一条 `message.usage`，失败返回 null 降级）。
- F5 视图注册：`sideViewDefs.ts` 追加（id `agent-status`、title "Agent 状态"、icon 🤖、默认上区）。

## C13 阶段 3 专有契约（F6）

- **Schema 内嵌**：SchemaStore `claude-code-settings.json` 随 slTerminal 打包（Vite import JSON），不放运行时下载。建议位置 `src/features/hooksConfig/schema/`（具体路径阶段 3 计划定）。
- **matcher 语义表**：照 `feature-plan/phase3-config-panel.md` matcher 语义表（窄字符集→精确匹配 OR / 其他→JS 正则非锚定 / `*"`"`省略→全匹配 / 大小写敏感 / FileChanged、StopFailure 窄字符集）。语义引擎为纯函数，单点定义供测试工具与保存校验共用。
- **面板注册**：走硬约束 #5 全流程（`panels/hooksConfig/`（或计划定名）→ `panelRegistry.ts` → `PANEL_TYPES` 追加）。
- **单条启停**：禁用状态存 slTerminal 侧 settings（ADR-0002），四元组（层级+事件+matcher+command）标识。
- **保存安全**：JSON + Schema 双校验不过拒绝保存；原子写（临时文件 + rename，照 `settings.rs` tempfile 先例）；保存后提示"需重启 claude 会话生效"。

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
| 面板类型名/目录名 | 阶段 3 | 建议 `hooksConfig` |
| schema 内嵌具体路径 | 阶段 3 | C13 给建议 |
| 各阶段 Stage 划分与 ID 编号 | 各阶段 | 建议前缀 `P{N}-BE/FE/TE/DOC` |

## 人工验证点（各阶段 stages 必须含对应段）

- **阶段 1**：真实 claude 全状态机走查（启动🟡→⚡→🟡→⚡→✅→退出无图标）；Windows Terminal 启动 claude 无信号文件且行为无异常；删除信号目录后 claude 无报错；#23554 SessionStart 冻结最新版回归
- **阶段 2**：真实 toast 出现/点击跳转页签；窗口聚焦时零通知；注意态任务栏闪烁启停
- **阶段 3**：GUI 配置 PreToolUse 拦截 hook → 重启 claude → 真实生效
