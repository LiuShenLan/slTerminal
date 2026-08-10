# 02 域 — hooks 信号链路（注入 / 事件通道 / 用量 / 配置读写）

> 总规格：`docs/spec/multi-cli/00-需求规格.md`（决策 D5/D10）。现状基线：`docs/analysis/02-hooks.md`（29 项全量盘点）。

## 1. 域范围与分层结论

02 域 29 项优化按抽象决策分两层：

| 层 | 内容 | 抽象结论 |
|----|------|---------|
| **通道层（泛化为 CLI 无关）** | 信号文件单事件单文件+原子 rename（02-9）、HookEventPayload→AgentEventPayload（02-10）、读→emit→删（02-11）、notify+3s 轮询双通道 watcher（02-12）、WATCHER 全局静态+幂等启动（02-13）、事件广播+前端订阅（02-14） | 机制零改动；payload 增加可选 cliId；事件名/类型名去 claude 化 |
| **能力层（profile.hooks 可选能力）** | 10 事件注入（02-1）、settings.json merge 注入（02-2）、matcher 结构（02-3）、识别/剔除（02-4）、版本检测（02-5）、reporter 脚本（02-6）、exit(0) 契约（02-7/8）、用量扫描（02-16/17）、三层配置路径（02-18）、子树 merge（02-19）、schema 内嵌（02-20）、Draft07 校验（02-21）、注入段保护（02-22）、注入入口与重读（02-23/24）、保存提示（02-25）、hooks 协议知识（02-26）、双模式编辑（02-28） | 全部归 claude profile / 后端 claude provider；接口由 profile 能力声明，实现保留 claude 专属 |

## 2. 需求条目 — 通道层

| 编号 | 需求 | 优先级 |
|------|------|--------|
| MC-201 | `HookEventPayload` → `AgentEventPayload`：8 字段（panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType）语义不变，**新增可选 `cliId`**（serde `default`，缺省前端按 `"claude"` 兼容）；serde 键集合测试同步 | 必须 |
| MC-202 | Tauri 广播事件 `hook-event` → `agent-event`；前端 `onHookEvent` → `onAgentEvent`（`listen<AgentEventPayload>("agent-event")` 封装照 onFsEvent 模式不变）；三处消费方（useXterm 页签、useAgentStatus 行建模、useAgentNotifications 通知）同步迁移 | 必须 |
| MC-203 | watcher 双通道（notify 50ms debounce + 3s 轮询补漏 + 目录自动重建）**零行为改动**；信号文件目录 `~/.slterminal/hooks-events/` 保持单目录全 CLI 共用（路由靠 payload.panelId + cliId，不按 CLI 分目录） | 必须 |
| MC-204 | `process_signal_file` 读→emit→删契约不变；解析失败/emit 失败仍删文件的容错语义不变（win10 实证防线，勿削弱轮询补漏） | 必须 |
| MC-205 | 前端事件消费 profile 解析顺序：`payload.cliId`（显式）→ `TerminalRegistry.get(panelId)?.agentSession?.cliId`（反查）→ `"claude"`（缺省兼容旧信号）；解析结果决定 eventToStatus / classifyNotification 策略来源 | 必须 |
| MC-206 | 无 hooks 能力的 profile：事件通道对其不产生事件（无 reporter 即无信号文件），消费方遇未知 cliId 的事件时 console.warn + 跳过（不抛异常） | 必须 |

## 3. 需求条目 — 能力层（后端命令泛化，决策 D10）

| 编号 | 需求 | 优先级 |
|------|------|--------|
| MC-210 | 后端建立 `CliHooksProvider` trait：`inject() / uninstall() / injection_status() / context_usage(transcript_path) / config_read(layer, project_path) / config_write(layer, hooks, project_path)`；claude 为首个实现（现状 inject.rs/usage.rs/config.rs 整体下沉 `hooks/claude/`，行为零改动） | 必须 |
| MC-211 | 命令泛化（8 条中的 6 条 hooks 类）：`hooks_inject` → `agent_hooks_inject(cliId)`、`hooks_uninstall` → `agent_hooks_uninstall(cliId)`、`hooks_injection_status` → `agent_hooks_injection_status(cliId)`、`hooks_context_usage` → `agent_context_usage(cliId, transcriptPath)`、`hooks_config_read` → `agent_hooks_config_read(cliId, layer, projectPath?)`、`hooks_config_write` → `agent_hooks_config_write(cliId, layer, hooks, projectPath?)`；未知 cliId → `AppError::Validation`；无 hooks 能力的 cliId → `AppError::Validation`（消息含「不支持 hooks 能力」语义） | 必须 |
| MC-212 | `src/ipc/hooks.ts` → `src/ipc/agentHooks.ts`（wrapper 加 cliId 首参）；`src/types/hooks.ts` → `src/types/agent.ts`（AgentEventPayload/ContextUsage/InjectionStatus/HookInjectionStatus→AgentHookInjectionStatus）；`ipc-hooks-contract.test.ts`（22 用例）四维契约同步（命令名/参数结构含 cliId/返回/异常） | 必须 |
| MC-213 | 后端 claude provider 内部全部保留 claude 命名与 claude 知识：`HOOK_EVENTS` 10 事件、`~/.claude/settings.json` 路径、matcher 结构、SCRIPT_VERSION 检测、reporter 模板 `slterm-hook-reporter.js`、三层配置路径、`hooks/claude/` 目录——**provider 内部是 claude 的合法领地**（D11：profile 内部 claude 专属实现保留 claude 命名） | 必须 |
| MC-214 | `ContextUsage` DTO 保持四字段（input/output/cacheRead/cacheCreation，cache 字段 serde default 0）；用量口径 `(input+cacheRead+cacheCreation)/contextLimit` 中 **contextLimit 由 profile.hooks.contextLimit 提供**（claude=200_000），`agentStatus/consts.ts` 的 `CLAUDE_CONTEXT_LIMIT` 退役；其他 CLI provider 返回时可只填 input/output（cache 缺省 0） | 必须 |
| MC-215 | reporter 脚本（`slterm-hook-reporter.js`）归 claude provider 资产；`include_str!` 路径随 `hooks/claude/` 迁移；**C10 契约（任何路径 exit 0）与 E2E-06 端到端守卫不得削弱**；reporter 写 payload 时可不写 cliId（缺省 claude 兼容）或显式写 `"claude"`，二选一并在测试中锁死 | 必须 |

## 4. 需求条目 — 配置面板联动（与 05 域衔接）

| 编号 | 需求 | 优先级 |
|------|------|--------|
| MC-220 | `hooksConfig` 面板的 IPC 调用迁移到泛化命令（`readHooksConfig/writeHooksConfig` 加 cliId 首参，值 = hub 面板当前选中 CLI） | 必须 |
| MC-221 | 注入/卸载按钮（F2 并入现状）调用 `agent_hooks_inject/uninstall(selectedCliId)`；注入后自动重读 user 层（C13-8）语义不变 | 必须 |
| MC-222 | 保存成功提示「hooks 改动需重启 claude 会话生效」改由 `profile.hooks.restartHint` 驱动（claude 值同现状文案）；`data-e2e="hooks-restart-hint"` 选择器保留 | 必须 |
| MC-223 | claude hooks 协议知识（eventsCatalog 30 事件/matcherEngine/5 种 handler 字段矩阵/schema 内嵌/Draft07 校验）**不抽象**，作为 claude 配置编辑器的内部实现保留；文件物理位置可迁入 `profiles/claude/` 或保留 `panels/hooksConfig/` + `features/hooksConfig/` 现状（二选一，规格不强制——建议保留现状降低 churn，目录语义在模块 CLAUDE.md 注明「claude 专属」） | 必须 |

## 5. 测试要求

| 层级 | 用例 |
|------|------|
| L1 | claude provider 迁移后既有 133 用例（inject 34 / signal 14 / watcher 20 / usage 26 / config 27 / mod 12）全绿（文件物理迁移不丢用例）；provider 注册表（get/未知 cliId Validation/无能力 Validation）；命令泛化后 block_on 直测（cliId 透传） |
| L2 | `AgentEventPayload` 反序列化：无 cliId 字段的旧信号 → 缺省兼容；前端 profile 解析顺序（显式/反查/缺省）三分支；未知 cliId 事件 warn+跳过 |
| L2 | `ipc-agent-hooks-contract.test.ts`：6 命令 × 四维（命令名逐字/参数含 cliId camelCase/返回透传/异常传播） |
| L4 | `hooks.e2e.ts`：注入/卸载/状态三态（经泛化命令）、E2E-06 真实 reporter 链路（信号文件产生/消费/非法 JSON exit 0/文件数不变）全绿；**E2E-05 用户目录隔离备份路径参数化**——`run-wdio.cjs` 备份集合（`~/.claude/settings.json` + `~/.slterminal/hooks/`）由 profile 声明的配置路径驱动或保持 claude 硬编码并注释「随第二 CLI 接入扩展」（二选一，建议后者降范围） |
| AC-4 | mock profile（含 hooks 能力）：其 eventToStatus/classifyNotification 策略被消费方真实调用（L2 可证） |

## 6. 迁移点（本域）

| 现状 | 目标 |
|------|------|
| `hooks/signal.rs`（HookEventPayload） | 保留通道侧，类型更名 AgentEventPayload + cliId |
| `hooks/inject.rs`、`usage.rs`、`config.rs` | 下沉 `hooks/claude/`（行为零改动） |
| `hooks/mod.rs`（WATCHER/InjectionStatus 三态） | 通道侧保留；InjectionStatus/HookInjectionStatus 更名去 claude 化（AgentHookInjectionStatus）或保留（二选一，建议更名——三态语义通用） |
| 广播 `hook-event` / 前端 `onHookEvent` | `agent-event` / `onAgentEvent` |
| `agentStatus/consts.ts CLAUDE_CONTEXT_LIMIT` | 退役，profile.hooks.contextLimit |
