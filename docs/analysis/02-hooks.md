# hooks 信号链路 — claude 定制优化盘点

## 相关文件

**后端（src-tauri/src/hooks/）**：
- `mod.rs` — InjectionStatus 枚举 / HookInjectionStatus DTO / WATCHER 全局静态实例 / start_signal_watcher 幂等启动
- `inject.rs` — 注入/卸载/状态三命令（10 事件注入、settings.json merge、handler 级剔除、版本检测）
- `signal.rs` — HookEventPayload DTO（8 字段）/ parse_signal_file / process_signal_file（读→emit→删）
- `watcher.rs` — HookSignalWatcher（notify 50ms debounce + 3s 轮询补漏双通道、目录自动重建）
- `usage.rs` — hooks_context_usage（transcript 尾部 64KB 逆行扫描）+ ContextUsage DTO
- `config.rs` — hooks_config_read/write（user/project/local 三层子树读写、read-modify-write merge）

**脚本与 PTY**：
- `src-tauri/assets/slterm-hook-reporter.js` — Node 单文件 hook 上报脚本（include_str! 内嵌）
- `src-tauri/src/pty/spawn.rs:931-936,999` — SLTERM_PANEL_ID 环境变量注入点（931-936 = Windows extra_envs 区域；999 = 非 Windows `cmd.env`）

**前端**：
- `src/ipc/hooks.ts` — inject/uninstall/getInjectionStatus/contextUsage + onHookEvent 订阅
- `src/ipc/hooksConfig.ts` — readHooksConfig/writeHooksConfig
- `src/features/hooksConfig/schema/index.ts` + `schema/claude-code-settings.json` — SchemaStore 官方 schema 内嵌 + hooks 子 schema + Draft07 校验
- `src/panels/hooksConfig/` — HooksConfigPanel（F2 注入入口）、useHooksConfig、configModel（isSltermManaged）、eventsCatalog、matcherEngine、JsonMode/GuiMode/EventTree/HandlerForm/MatcherTester
- `src/types/hooks.ts`、`src/types/hooksConfig.ts` — DTO 类型
- `src/panels/terminal/useXterm.ts:349-373` — hook-event 下游消费（F3 四态，另属 F3 领域，仅标注链路位置）

**文档**：`src-tauri/src/hooks/CLAUDE.md`（核心数据流/双通道/注入规则/版本检测/三层读写等）、`src/panels/CLAUDE.md` hooksConfig 节、`src/features/hooksConfig/CLAUDE.md`

**测试侧（L4 E2E）**：
- `e2e-tests/run-wdio.cjs` — 用户目录隔离（E2E-05：`~/.claude/settings.json` + `~/.slterminal/hooks/` 备份/还原 + `hooks-events/` exit 清理——hooks 注入链路对用户 claude 配置的专属测试防护，`run-wdio.cjs:47-94`）
- `e2e-tests/hooks.e2e.ts` — hooks spec（4 条 active，E2E-06 真实 reporter 链路见 #8）

## 优化项清单

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 1 | 10 事件 hooks 注入（C9） | inject.rs:16-27 | 按 claude hooks 事件清单注入 matcher | claude 的 10 个 hook 生命周期事件 | 硬编码 claude |
| 2 | ~/.claude/settings.json merge 注入 | inject.rs:39-41,190-263 | 读现有 settings → 去旧 slterm 段 → 追加 matcher → 原子写回 | claude 的 hooks 配置载体（`~/.claude/settings.json`） | 硬编码 claude |
| 3 | matcher 条目结构（matcher:"" + node 命令 + timeout:5） | inject.rs:151-161 | 按 claude hooks 配置 schema 构造 matcher 组 | claude hooks 的 matcher/hook 配置格式 | 硬编码 claude |
| 4 | slterm matcher 识别 + handler 级剔除卸载 | inject.rs:98-148 | command 含 "slterm-hook-reporter" 子串识别注入段；卸载时仅剔 slterm handler、保用户 handler | claude 每次注入/卸载 hooks 配置 | 通用机制但 claude 触发 |
| 5 | 注入状态三态 + 版本检测 | inject.rs:50-78,341-394 | SCRIPT_VERSION 纯文本提取比对 → Injected/Outdated/NotInjected | 查询 claude hooks 注入状态 | 通用机制但 claude 触发 |
| 6 | reporter 脚本（claude hook stdin 解析） | assets/slterm-hook-reporter.js:44-53 | 解析 claude hook 事件 JSON（hook_event_name/session_id/transcript_path/cwd/tool_name/notification_type）→ 组装 8 字段 payload | claude 每次 hook 触发时以 stdin JSON 调用 | 硬编码 claude |
| 7 | SLTERM_PANEL_ID 环境变量路由 | reporter.js:27-32；spawn.rs:931-936,999 | PTY spawn 注入 panelId 环境变量，脚本无此变量即 exit(0) | claude 在 slTerminal 终端内启动 | 硬编码 claude（**消费专属**——注入机制本身通用，claude 专属仅为 reporter 消费侧 exit(0) 门控，见详述） |
| 8 | C10 契约（任何路径 exit 0） | reporter.js:1-81 | 全部代码路径恒 exit(0)、不写 stderr | claude hook 机制不容许失败信号干扰 | 硬编码 claude |
| 9 | 单事件单文件 + 原子 rename（备选 A） | reporter.js:62-65 | .tmp 写完再 renameSync 成 .json，天然避免半写文件 | claude 高频 hook 触发（信号文件写入） | 通用机制但 claude 触发 |
| 10 | HookEventPayload 8 字段 DTO | signal.rs:19-38 | camelCase 序列化，字段语义对应 claude hook 事件 | claude hook 事件携带的会话/transcript 元数据 | 硬编码 claude |
| 11 | 信号文件处理：读→emit→删 | signal.rs:55-93 | process_signal_file 无论 emit 成败均删文件 | claude hook 信号文件到达 | 通用机制但 claude 触发 |
| 12 | notify+轮询双通道 watcher（win10 实证） | watcher.rs:46-129,199-211 | notify NonRecursive 50ms debounce 实时 + 3s 轮询补漏 + 目录删除自动重建 | claude hook 信号文件持续产生（33 残留实证场景） | 通用机制但 claude 触发 |
| 13 | WATCHER 全局静态实例 + 幂等启动 | mod.rs:64-101；lib.rs:71-72 | 静态 Mutex 存储（避免 state.rs 循环依赖），setup 启动，已启动跳过 | 应用启动即监听 claude hook 信号 | 通用机制但 claude 触发 |
| 14 | hook-event 广播 + 前端订阅 | signal.rs:56；ipc/hooks.ts:56-65 | Tauri event 广播，onHookEvent listen 封装（照 onFsEvent 模式） | claude hook 事件上抛前端 | 通用机制但 claude 触发 |
| 15 | 前端四态消费链（F3） | useXterm.ts:349-373；src/lib/claudeStatus.ts:41-75 | onHookEvent 按 panelId 过滤 → eventToStatus 映射事件名→emoji → 页签状态 | claude 的 SessionStart/UserPromptSubmit/PreToolUse/PostToolUse/Notification/PermissionRequest/Stop/SessionEnd/StopFailure/PostToolUseFailure 等 hook 事件 | 硬编码 claude |
| 16 | transcript 尾部 64KB 逆行扫描用量 | usage.rs:34-89 | 尾部 64KB 窗口 + 跳首行 + 逆行扫 `message.usage` | claude transcript JSONL 的 usage 记录格式 | 硬编码 claude |
| 17 | ContextUsage DTO + cache 字段 | usage.rs:13-26 | input/output/cacheRead/cacheCreation token 四字段，缺省 0 | Anthropic API 用量结构（claude 上下文占用口径） | 硬编码 claude |
| 18 | hooks 配置三层路径 | config.rs:22-51,60-85 | user=`~/.claude/settings.json`、project=`<root>/.claude/settings.json`、local=`<root>/.claude/settings.local.json` | claude 三层配置文件的路径约定 | 硬编码 claude |
| 19 | 子树 read-modify-write merge | config.rs:111-152 | hooks 键替换、permissions/env/$schema 保留、损坏拒绝覆盖、原子写 | 面板编辑 claude 配置 | 通用机制但 claude 触发 |
| 20 | SchemaStore 官方 schema 内嵌 + hooks 子 schema 提取 | schema/index.ts:19-37 | 内嵌 claude-code-settings.json（无远程 $ref 已核实），提取 properties.hooks + hookMatcher/hookCommand $defs | claude hooks 配置 schema（官方 SchemaStore） | 硬编码 claude |
| 21 | Draft07 双校验 validateHooksJson | schema/index.ts:61-79 | json-schema-library Draft07 单例校验 hooks 子树 | 保存前校验 claude hooks 配置合法性 | 硬编码 claude |
| 22 | 注入段保护 isSltermManaged | configModel.ts:195-199 | command 含 slterm-hook-reporter → GUI 标记「slTerminal 托管」+ 禁删/只读 | 面板编辑含注入段（C9）的 claude 配置 | 通用机制但 claude 触发 |
| 23 | F2 注入入口并入面板 | HooksConfigPanel.tsx:192-210,224-252,331-369 | 「注入 Hooks」/「卸载 Hooks」按钮 + 注入状态条三态 | 面板内管理 claude hooks 注入 | 通用机制但 claude 触发 |
| 24 | 注入后自动重读 user 层（C13-8） | HooksConfigPanel.tsx:215-221 | 注入改写 `~/.claude/settings.json` 后切 user 层重读 | 注入/卸载操作改写了 claude 配置 | 通用机制但 claude 触发 |
| 25 | 保存成功提示「需重启 claude 会话生效」 | HooksConfigPanel.tsx:371-375 | 保存后提示文案 | 配置改动需重启 claude 会话生效 | 硬编码 claude |
| 26 | claude hooks 协议知识内嵌（事件目录/matcher 语义/handler 字段矩阵） | panels/hooksConfig/eventsCatalog.ts、matcherEngine.ts、types/hooksConfig.ts:23-49 | 30 事件×10 组全表、matcher 语义（exact-or/regex/all + 窄字符集）、5 种 handler 类型字段矩阵（C13-3 官方版） | claude hooks 官方协议（事件名/matcher 语法/handler 类型） | 硬编码 claude |
| 27 | 同页单例面板 + 侧栏右键入口（C13-7） | workspace/pageApis.ts:109-134（openHooksConfigPanel） | 面板 id=hooksConfig-{pageId}，命中聚焦/未命中 addPanel | 打开 claude hooks 配置面板 | 通用机制但 claude 触发 |
| 28 | 双模式编辑 + 双向转换同步（JSON/GUI） | HooksConfigPanel.tsx:290-310,392-397；configModel.ts:94,155 | JSON 优先/GUI Master-Detail 双模式切换（非法 JSON 禁切 GUI），`jsonToGui`/`guiToJson` 双向同步（round-trip 不丢数据、未知字段归 extraFields、未知事件归组），MatcherTester 内联试测 | 编辑 claude hooks 配置（与 #26 同面板：知识层 #26 + 编辑机制层本条；多 CLI 抽象时该面板作为 claude 专属配置编辑器整体保留） | 通用机制但 claude 触发 |
| 29 | 测试侧：E2E 用户目录隔离（E2E-05） | `e2e-tests/run-wdio.cjs:47-94` | wdio 启动时备份 `~/.claude/settings.json` + `~/.slterminal/hooks/` 整目录（.e2e-bak），exit 时还原 + 清理 `hooks-events/` 运行产物 | hooks 注入链路对用户 claude 配置的专属测试防护（备份路径随 CLI 配置路径替换） | 通用机制但 claude 触发 |

---

## 各项详细机制描述

### A. 注入链路（inject.rs + reporter 脚本 + PTY env）

**1. 10 事件 hooks 注入（C9）** — `inject.rs:16-27` 的 `HOOK_EVENTS` 常量：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`Stop`、`StopFailure`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`PermissionRequest`。注释明示「C9 规定的 10 个注入事件（与四态映射相关的最小集）」。事件名是 claude hooks 协议专有名词；注入目的正是为 claude 的四个状态（F3 四态）供数据。**注入事件集与 04-1 四态映射表消费事件为全集一致关系（10 = 10）**——`eventToStatus`（`claudeStatus.ts:45-74`）switch 含全部 10 个注入事件的显式分支（SessionStart/UserPromptSubmit/PreToolUse/PostToolUse/Notification/PermissionRequest/Stop/StopFailure/PostToolUseFailure/SessionEnd），SessionEnd 亦有显式 case（`claudeStatus.ts:69-70`，返回 null 清状态）。专属程度：硬编码 claude。

**2. ~/.claude/settings.json merge 注入** — `inject.rs:39-41` 的 `claude_settings_path()` 硬编码 `~/.claude/settings.json`（claude 用户配置路径）。`inject_impl`（inject.rs:190-263）流程：确保 `~/.slterminal/hooks/` 目录 → NamedTempFile 原子写脚本 → 读 settings.json（不存在/空视为空对象，**非法 JSON 返回 AppError 且不改动文件**）→ `remove_slterm_matchers` 去旧段（幂等升级）→ `inject_matchers` 追加 → 原子写回。merge 保留用户已有自定义 matcher（`inject_preserves_existing_user_matchers` 测试）。文档出处：hooks/CLAUDE.md「settings.json 注入/卸载规则」。专属程度：硬编码 claude（操作对象是 claude 的配置文件与 hooks 结构）。

**3. matcher 条目结构** — `build_matcher_entry`（inject.rs:151-161）按 claude hooks 配置格式构造：`{"matcher": "", "hooks": [{"type": "command", "command": "node \"<绝对路径>\"", "timeout": 5}]}`。`matcher: ""` 匹配全部事件；脚本路径反斜杠统一替换为 `/`（Windows 路径规范化）；`timeout: 5` 限制 claude hook 执行超时。专属程度：硬编码 claude（写入 claude 的 hooks 配置 schema）。

**4. slterm matcher 识别 + handler 级剔除卸载** — `handler_contains_slterm`（inject.rs:98-102）以 command 字符串含 `"slterm-hook-reporter"` 判定注入段（与前端 `isSltermManaged` 粒度一致）。`remove_slterm_matchers`（inject.rs:119-148）做 **handler 级剔除**：组内仅删命中 handler、用户 handler 保留；组内全空才删组；事件键空则清键；hooks 段全空则移除整个 `"hooks"` 键（inject.rs:289-294，uninstall 路径）。卸载安全策略：settings.json 非法 JSON 时静默跳过配置清理但仍删 `~/.slterminal/hooks/` + `hooks-events/` 目录（inject.rs:268-335）。识别/剔除机制本身与 claude 无关（识别自家标记），但它只作用于 claude 的 settings.json hooks 结构。专属程度：通用机制但 claude 触发。

**5. 注入状态三态 + 版本检测** — `template_version`（inject.rs:50-63）与 `disk_script_version`（inject.rs:66-78）均以纯文本解析 `const SCRIPT_VERSION = N;`（无需执行 JS）。`injection_status_impl`（inject.rs:341-394）三态判定：脚本存在 + settings 含 slterm matcher + 版本一致 → `Injected`；版本不匹配 → `Outdated`（version 报磁盘版本）；其余 → `NotInjected`。脚本模板经 `include_str!("../../assets/slterm-hook-reporter.js")`（inject.rs:13）编译期嵌入，无运行时 assets 依赖。专属程度：通用机制但 claude 触发（脚本与配置都是 claude 链路的）。

**6. reporter 脚本（claude hook stdin 解析）** — `src-tauri/assets/slterm-hook-reporter.js`：零依赖 Node 脚本（Node >= 18 内置 API）。从 claude hook 机制传入的 stdin JSON 中提取 `hook_event_name`/`session_id`/`transcript_path`/`cwd`/`tool_name`/`notification_type`（reporter.js:44-53），组装 8 字段 payload（panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType），写入 `~/.slterminal/hooks-events/`。字段名与 `session_id`、`transcript_path` 等完全对应 claude hook 事件负载结构。专属程度：硬编码 claude（解析 claude 专有的 hook 事件 JSON 格式）。

**7. SLTERM_PANEL_ID 环境变量路由** — PTY spawn 时与 `COLORTERM`/`TERM`/`TERM_PROGRAM` 同一时机注入 `SLTERM_PANEL_ID`（spawn.rs:931-936，Windows ConPTY 路径 extra_envs；spawn.rs:999 非 Windows `cmd.env`；**与 01-3 重叠**——01 领域视角为 env 注入机制，本领域视角为信号路由键），值 = `request.panel_id`。reporter 脚本 `process.env.SLTERM_PANEL_ID` 读取（reporter.js:27），**无此变量（非 slTerminal 启动的 claude）→ 直接 exit(0)**（reporter.js:30-32）。集成测试 `pty_env_injects_slterm_panel_id`（tests/pty_integration_tests.rs:393-437，387-392 为 doc/属性行）验证子进程环境含该变量。**口径：注入机制通用、消费专属**——注入点对全部 PTY spawn 生效（pwsh/cmd/claude 一律注入），claude 专属仅为 reporter 脚本的消费侧（无变量即 exit(0) 门控，reporter.js:30-32）；抽象多 CLI 时该注入点应**保留为通用每终端路由键**（任何 CLI 的 hook 链路均可消费 panelId），仅 exit(0) 门控语义随 CLI 替换（与 01-3 重叠合并项）。文档出处：hooks/CLAUDE.md「SLTERM_PANEL_ID 环境变量路由」。专属程度：硬编码 claude（注入点存在理由为 claude hook 链路路由）。

**8. C10 契约（任何路径 exit 0）** — reporter.js 全部代码路径恒 `process.exit(0)`：stdin 为空（:22-24）、无 SLTERM_PANEL_ID（:30-32）、`os.homedir()` 为空（:34-37）、JSON 解析失败/目录不可写（:68-71）、stdin error（:73-75）、顶层异常（:77-80），以及正常完成路径（:67）——异常路径与正常路径全部 exit(0)。不向 stderr 输出。**L4 守卫（E2E-06 ↔ 02-8）**：`e2e-tests/hooks.e2e.ts:147-217` 真实 node 执行 reporter 脚本（用例 3，`it(...)` 起于 :157：stdin 契约 JSON + SLTERM_PANEL_ID env → 信号文件产生/消费 :175-200 + emoji 断言 :204 + **非法 JSON exit 0 断言 :214、文件数不变断言 :216-217**）——C10 契约与整条信号链路的**唯一端到端守卫**（L1/L2 均为注入闭包/mock 层），多 CLI 抽象改造信号链路时是回归基线。文档出处：hooks/CLAUDE.md「脚本任何路径 exit 0（C10 契约）」，原话「确保 Claude Code hook 机制不受干扰」。专属程度：硬编码 claude（行为约束为适配 claude hook 机制而定）。

**9. 单事件单文件 + 原子 rename（备选 A）** — reporter.js:62-65 先写 `.tmp` 再 `fs.renameSync` 成 `.json`；完整文件名规则 `{timestamp}_{safePanelId}_{event}_{rnd6}.json`（reporter.js:55-59——时间戳/净化 panelId/事件名/6 位随机后缀，唯一性与可诊断性目的；被 `hooks.e2e.ts:187`（safeId 计算）按 panelId 匹配断言依赖，waitUntil 断言在 :188-196）；文件名含**安全净化**（reporter.js:56 `panelId.replace(/[^a-zA-Z0-9_-]/g, "_")` 白名单替换——panelId 来自环境变量，写文件名前防路径注入）；后端 watcher 消费时**不区分事件类型**（`handle_notify_events`，watcher.rs:186-194），仅按「路径为 `.json` 文件」过滤（`.tmp` 被扩展名过滤忽略），rename 完成即代表文件完整落盘，天然避免半写。文档出处：hooks/CLAUDE.md「单事件单文件 + 原子 rename（备选 A）」——对比备选 B（JSONL 追加）在 Windows 多进程并发 `appendFile` 无法保证行级原子性。专属程度：通用机制但 claude 触发（文件通道机制通用，当前仅 claude hook 信号使用）。

### B. 信号接收链路（signal.rs + watcher.rs + mod.rs）

**10. HookEventPayload 8 字段 DTO** — `signal.rs:19-38`，camelCase 序列化（C1 契约）。`panel_id` 注释「页签路由标识（环境变量 SLTERM_PANEL_ID）」；`event` 注释「C9 10 事件之一」；`session_id`/`transcript_path`/`cwd`/`tool_name`/`notification_type` 均取自 claude hook 事件。serde 键集合精确匹配测试（signal.rs:156-174）锁死 8 键。专属程度：硬编码 claude（字段语义是 claude 会话/transcript 元数据）。

**11. 信号文件处理：读→emit→删** — `process_signal_file`（signal.rs:55-57）经 AppHandle `emit("hook-event", payload)`；可测试核心 `process_signal_file_with`（signal.rs:63-93）：读失败/解析失败/缺 panelId 均 warn 并**仍尝试删除文件**，绝不 panic；emit 失败仅 warn 同样删除。信号文件从产生到删除存活亚秒级（实时通道）或 ≤3s（轮询兜底），目录常态为空是设计行为（hooks/CLAUDE.md「信号文件瞬态特性」）。专属程度：通用机制但 claude 触发。

**12. notify+轮询双通道 watcher（win10 实证）** — `HookSignalWatcher::start`（watcher.rs:46-129）：notify NonRecursive + 50ms debounce 实时通道（初始化/监听失败**仅降级 warn** 不致命，watcher.rs:58-74）；3s `POLL_INTERVAL` 轮询补漏通道（watcher.rs:27,111-117）：`run_one_tick`（watcher.rs:199-211）每 tick 先 `create_dir_all` 自动重建被删目录（卸载 `remove_dir_all` 场景）→ `poll_once` 消费残留 `.json` → 查停止信号。notify 降级时线程走 `LOOP_TICK`（250ms）sleep 节奏防忙循环（watcher.rs:30,106）。两通道同线程串行执行无竞态；线程名 `hook-signal-watcher`。动机是 win10 另一台 PC 实证 33 个信号文件残留（notify 事件丢失/目录重建句柄失效）。文档出处：hooks/CLAUDE.md「notify+轮询双通道（win10 实证修复）」。专属程度：通用机制但 claude 触发（监听的是 claude hook 产生的信号文件）。

**13. WATCHER 全局静态实例 + 幂等启动** — `static WATCHER: Mutex<Option<Box<dyn WatcherHandle>>>`（mod.rs:64），避免在 AppState 新增字段导致 state.rs↔hooks 循环依赖。`start_signal_watcher`（mod.rs:71-101）幂等：已启动跳过不报错；启动失败不存实例可重试。在 `lib.rs:71-72` 的 `.setup()` 中启动。专属程度：通用机制但 claude 触发。

**14. hook-event 广播 + 前端订阅** — 后端 `emit("hook-event")`（signal.rs:56）→ 前端 `onHookEvent`（ipc/hooks.ts:56-65）照 `onFsEvent` 模式封装 `listen<HookEventPayload>("hook-event")`，返回 unsubscribe。**前端消费端三个**：F3 页签四态（#15）、F4 通知调度（`src/features/notifications/useClaudeNotifications.ts` 订阅 onHookEvent → `classifyEvent` 分类 permission/error/done → 失焦门控 toast + 任务栏闪烁，机制本体已由 04-17 盘点）、**Agent 状态视图行建模**（`src/features/agentStatus/useAgentStatus.ts:196-202` 直接订阅 `onHookEvent`，`handleHookEvent` :109-194——SessionEnd/Exit 删行 :126-135、建行/更新 :139-176、`contextUsage(payload.transcriptPath)` 用量拉取 :178-191（F5）——恰是 02-16/17 的前端消费方，机制本体已由 04-13 盘点），本条补链路完整性。专属程度：通用机制但 claude 触发（事件内容为 claude hook 信号）。

**15. 前端四态消费链（F3）** — `useXterm.ts:349-373` 订阅 `onHookEvent`：按 panelId 过滤 → `eventToStatus(payload.event, payload.notificationType)`（`src/lib/claudeStatus.ts:41-75` 纯函数，事件名→四态映射）→ 页签 emoji；`SessionEnd`/`Exit` → `setClaudeSession(panelId, null)`（`{active:false}` 仅 SessionEnd 触发，Exit 分支为防御代码，详 01-21）；`setClaudeSession` 写入时 **payload 空串归一 `|| undefined`**（`useXterm.ts:361-364`——claude hook 输入缺字段时下游 derive/标题覆盖/usage 拉取全部静默失效的专属防御）。`eventToStatus` 的映射表直接消费 claude hook 事件名：`SessionStart`→attention、`UserPromptSubmit`/`PreToolUse`/`PostToolUse`→working、`Notification`（**仅 `notificationType` ∈ {permission_prompt, idle_prompt, agent_needs_input} 子类型，`ATTENTION_NOTIFICATION_TYPES` `claudeStatus.ts:29-33`**）→attention、其余 notification 类型→null（不改变状态）、`PermissionRequest`→attention、`Stop`→✅、`PostToolUseFailure`/`StopFailure`→❌、**`SessionEnd`→null（显式分支 `claudeStatus.ts:69-70`，清状态的关键事件）**。本领域仅标注链路位置与消费入口，四态系统细节属 F3 领域。**重叠标注**：与 01-21（消费侧链路）、04-9（页签 emoji 直接通道 + 会话写入）为同一代码区间 `useXterm.ts:349-373` + `setClaudeSession` + 空串归一 `|| undefined` 的三处描述，内容一致无冲突。专属程度：硬编码 claude（事件名映射是 claude 专有协议）。

### C. 用量查询（usage.rs）

**16. transcript 尾部 64KB 逆行扫描** — `hooks_context_usage`（usage.rs:34-40）+ `scan_transcript_usage`（usage.rs:55-86）：从尾部最多读 `TRANSCRIPT_TAIL_BYTES = 64KB`（usage.rs:89），中途起始跳首行（截断行），自末行逆行扫描含 `message.usage` 的 JSON 行。任何异常（文件不存在/权限/UTF-8 无效/无 usage 行）→ `Ok(None)` 不报错。I/O 在 spawn_blocking 内（usage.rs:49）。大文件用例锁死「>128KB 仅读尾部窗口仍命中」（usage.rs:429-464）。transcript 是 claude 的会话记录文件（路径来自 hook 事件 `transcript_path`），`message.usage` 是 claude transcript 行结构。专属程度：硬编码 claude。

**17. ContextUsage DTO + cache 字段** — `usage.rs:13-26`：`input_tokens`/`output_tokens`/`cache_read_input_tokens`/`cache_creation_input_tokens`（camelCase，cache 字段 `#[serde(default)]` 兼容旧 transcript 缺失）。cache 字段是 Anthropic API 的 prompt caching 用量结构；前端 `src/types/hooks.ts:1-11` 四字段镜像；用量口径（input+cacheRead+cacheCreation，上限 200_000）由 `src/features/agentStatus/consts.ts` 消费（agentStatus 领域）。专属程度：硬编码 claude。

### D. 配置编辑（config.rs + hooksConfig 面板）

**18. hooks 配置三层路径** — `HooksLayer`（config.rs:22-31）+ `layer_file_name`（config.rs:46-51）：user = `~/.claude/settings.json`（dirs::home_dir 解析，绕过 project_root 沙箱）、project = `<projectPath>/.claude/settings.json`、local = `<projectPath>/.claude/settings.local.json`。project/local 入参经 `validate_path_within_root` 沙箱校验（config.rs:74-83）。`.claude` 目录与 settings.local.json 是 claude 配置体系的路径约定。专属程度：硬编码 claude。

**19. 子树 read-modify-write merge** — `read_hooks_subtree`（config.rs:93-102）：文件不存在/无 hooks 键 → `Null`（面板首次创建场景），**JSON 损坏 → Err**（防损坏文件上编辑后 merge 丢其他字段）。`write_hooks_subtree`（config.rs:111-152）：hooks 必须为 Object；读原文件 → 根对象 hooks 键替换为入参 → NamedTempFile 原子写（明确不做 .bak），`permissions`/`env`/`$schema` 原样保留；损坏 → Err 拒绝覆盖；根为数组/标量 → Err；null 根视空对象；父目录自动创建。**merge 原语关联**：与 02-2（inject.rs matcher 组级 merge）为同一「读→改→原子写 NamedTempFile+persist」原语的两套平行实现——抽象时两处可统一为同一配置编辑原语（注入侧按 matcher 组粒度、面板侧按整键粒度，语义差异需保留）。专属程度：通用机制但 claude 触发（读写的是 claude 配置文件的 hooks 子树）。

**20. SchemaStore 官方 schema 内嵌 + hooks 子 schema 提取** — `schema/index.ts:19-37`：`claude-code-settings.json` 为 SchemaStore `https://json.schemastore.org/claude-code-settings.json` 快照（2026-08-01 下载，整文件替换即升级）；自包含性已核实（35 个本地 `$ref` 全指 `#/$defs/*`）。`hooksSubSchema` = `properties.hooks` + `$defs` 子集（hookMatcher + hookCommand，不含 permissions 专用 permissionRule），保证本地 `$ref` 在独立 schema 中可解析。文档出处：features/hooksConfig/CLAUDE.md「SchemaStore 官方 schema 内嵌（P3-FE-07）」。专属程度：硬编码 claude（schema 是 claude-code-settings 的官方 schema）。

**21. Draft07 双校验 validateHooksJson** — `schema/index.ts:37,61-79`：`hooksDraft` 为 json-schema-library Draft07 单例（非 ajv）；`validateHooksJson` 先 JSON.parse 语法校验、再 hooks 子 schema 校验（additionalProperties 拦未知事件）。供 JsonMode 波浪线（onValidationChange 上报）与保存路径（P3-FE-17 双校验：JSON.parse + validateHooksJson 任一失败弹窗拒绝写盘）共用。专属程度：硬编码 claude（校验规则来自 claude schema）。

**22. 注入段保护 isSltermManaged** — `configModel.ts:195-199`：`command` 含 `slterm-hook-reporter` 子串（照 C9 识别规则，与后端 `handler_contains_slterm` 同款）→ GUI 标记「slTerminal 托管」+ 禁删/表单只读（HandlerForm 只读、EventTree 标记、GuiMode 三层删除按钮禁用——handler/含托管 handler 的 matcher 组/事件）。**JSON 模式不限制**（用户对自己文件有最终权利）。文档出处：panels/CLAUDE.md「注入段保护（C13-8）」。专属程度：通用机制但 claude 触发（保护的是自家注入进 claude 配置的段）。

**23. F2 注入入口并入面板** — `HooksConfigPanel.tsx:192-210`（挂载查询注入状态）、`:224-252`（`handleInject`/`handleUninstall` 直接调用 `src/ipc/hooks` 的 `inject()`/`uninstall()`，不改其实现）、`:331-369`（注入状态条三态：已注入/未注入/版本过旧 + 注入/卸载按钮，busy 期间禁用防重复点击）。生产环境主入口即面板 GUI（dev/E2E 另有 `__slterm_e2e_injectHooks` 等 helper，E2E_ENABLED 门控）。文档出处：hooks/CLAUDE.md「注入入口（面板 GUI 为主 + dev/E2E helper 补充）」、panels/CLAUDE.md「F2 并入（P3-FE-21/22）」。专属程度：通用机制但 claude 触发。

**24. 注入后自动重读 user 层（C13-8）** — `HooksConfigPanel.tsx:215-221` `reloadUserConfig`：注入/卸载改写 `~/.claude/settings.json` 后，当前层为 user 直接 reload，非 user 切到 user 层（dirty 守卫由 useHooksConfig 内部 ask 处理）。专属程度：通用机制但 claude 触发。

**25. 保存成功提示「需重启 claude 会话生效」** — `HooksConfigPanel.tsx:371-375`：保存成功后状态条显示 `hooks 改动需重启 claude 会话生效`（data-e2e="hooks-restart-hint"），下次编辑/重载后隐藏。文案假设 claude 会话重启后 hooks 配置才生效。专属程度：硬编码 claude。

**26. claude hooks 协议知识内嵌（事件目录/matcher 语义/handler 字段矩阵）** — `panels/hooksConfig/eventsCatalog.ts`：30 事件 × 10 组全表（**官方 hooks 协议全集；与 #1 注入的 10 事件为全集-子集关系：10 ⊂ 30**）+ handler 支持档（A/B/C）+ 窄字符集受限事件（FileChanged/StopFailure）——事件名全部为 claude hooks 官方事件；`matcherEngine.ts` `matchHook`（C13-5）：exact-or / regex / all matcher 语义 + 受限窄字符集——claude hooks matcher 语法；**matcher 版本前提**（matcherEngine.ts:6-10 模块注释）：逗号/空格作为 OR 分隔符需 claude v2.1.191+、连字符参与匹配值需 v2.1.195+，低版本 claude 对受限事件（FileChanged/StopFailure）中的连字符/空格/逗号 matcher 行为不可预期——语义随 CLI 版本演进需跟踪；`src/types/hooksConfig.ts:23-49` 的 5 种 handler 类型（command/http/mcp_tool/prompt/agent）字段矩阵——claude 官方 hooks handler 类型（C13-3 官方版）。文档出处：panels/CLAUDE.md hooksConfig 文件表。专属程度：硬编码 claude。

**27. 同页单例面板 + 侧栏右键入口（C13-7）** — 面板 id = `hooksConfig-{activePageId}`；入口为侧栏右键菜单「打开 Hooks 配置」（SidebarTree 先 `switchToPage` 切到目标页 → `openHooksConfigPanel(pageId)`，workspace/pageApis.ts:109-134：轮询 `getPageApi` 就绪 → `getPanel(id)` 命中聚焦、未命中 addPanel）。面板 props 兼容 Dockview 不依赖 panelId 的单例语义。单例机制本身与 claude 无关，但入口与面板内容全部服务 claude hooks 配置。专属程度：通用机制但 claude 触发。

**28. 双模式编辑 + 双向转换同步（JSON/GUI）** — `HooksConfigPanel.tsx:290-310` 模式切换（默认 JSON，非法 JSON 时 GUI 按钮禁用，`:290-297` title 提示「JSON 存在错误，无法切换到 GUI 模式」）+ `:392-397` 模式渲染容器（`mode === "gui" ? <GuiMode> : <JsonMode>`）。`configModel.ts:94` `jsonToGui` / `:155` `guiToJson` 双向转换纯函数：round-trip 不丢数据（未知字段归 `extraFields`）、未知事件归组。双模式同步（P3-FE-16，`HooksConfigPanel.tsx:7-8` 文件头注释）：JsonMode.onChange → updateConfigJson（JSON.parse 门控，非法保留最后合法快照仅校验上报）、GuiMode.onChange → updateGui（guiToJson 回写 JSON），`configJson`/`guiModel`/`dirty` 共享于 `useHooksConfig`。JsonMode 另含事件导航侧栏 + MatcherTester 内联试测（试测与保存校验共用 `matchHook`）。机制本身通用（JSON 表单 ↔ GUI 表单双向同步是通用编辑模式），存在理由为编辑 claude hooks 配置——与 #26 同面板（#26 = 协议知识层，本条 = 承载知识的编辑机制层）；多 CLI 抽象时该面板作为 claude 专属配置编辑器整体保留。文档出处：panels/CLAUDE.md「hooksConfig：双模式编辑（JSON/GUI）」节。专属程度：通用机制但 claude 触发。

### E. 测试侧（run-wdio.cjs）

**29. E2E 用户目录隔离（E2E-05）** — `run-wdio.cjs:47-94`（详细机制见 `e2e-tests/CLAUDE.md`「用户目录隔离机制（FIX-TE-04 + E2E-05 扩展）」节）：wdio 启动时备份 `~/.claude/settings.json`（复制为 `.e2e-bak`，:51-52）+ `~/.slterminal/hooks/` 整目录（`fs.cpSync` 递归复制，:57-70），`process.on('exit')` 中同步还原（先 `rmSync` 删产物再 rename 备份，:78-94）+ `hooks-events/` 运行产物清理（:81）。防护对象是 claude 的 `~/.claude/settings.json` 与注入脚本目录——多 CLI 抽象信号链路时，该测试防护的备份路径需随 CLI 配置路径替换。**专属程度：通用机制但 claude 触发**（测试侧，非产品机制；防护对象为 claude 配置）。

---

## 补充：hook 脚本性能实测决策（问题 5）

claude 专属的 hook 脚本性能调查结论（记载于 `src-tauri/src/hooks/CLAUDE.md`「性能实测（问题 5）」段，2026-07-29 Win11 build 26200、Node v22 实测）：**hook 脚本 36-44ms/次**（44/37/36/37/36ms 五次测量；裸 node 基线 35ms）；**启动路径仅 `SessionStart` 一个 hook 事件触发**（claude 启动生命周期 hooks 总贡献 ~0.1s 量级）；结论「**hooks 不是 claude 启动慢 1-3s 主因**（主因 = claude 自身 Windows node 模块加载 + Ink 渲染器初始化），接受现状，不做 per-event node spawn 优化」。对后续抽象通用 CLI 机制时的性能预算有参考价值。

---

## 三档分布统计

- **硬编码 claude**：15 项（#1,2,3,6,7,8,10,15,16,17,18,20,21,25,26）
- **通用机制但 claude 触发**：14 项（#4,5,9,11,12,13,14,19,22,23,24,27,28,29）
- **完全通用**：0 项

> 统计口径：表格 29 项逐条归类。本领域不存在与 claude 无关的优化——信号文件通道、watcher 双通道、配置读写等机制虽为通用实现，但全部仅被 claude hooks 激活（专属程度口径：「通用机制但 claude 触发」）。
