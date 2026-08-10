# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

hooks 模块——CLI 泛化 hook 宿主侧能力层（Stage 03，MC-201~215）：信号文件通道（接收 + 解析 + 广播）、`CliHooksProvider` trait + cliId 键注册表、claude provider 下沉（注入/卸载/状态查询/transcript token 用量/三层配置读写）、6 条泛化 Tauri 命令。

核心数据流：

```
claude 子进程 hook 触发
  → reporter 脚本（claude provider 资产）读 stdin → 组装 JSON（显式 cliId）→ 原子写 .tmp → rename .json
  → HookSignalWatcher（notify+轮询双通道：notify NonRecursive 50ms debounce 实时 + 3s 轮询补漏）
  → process_signal_file（读 → parse → emit("agent-event") → 删文件）
  → 前端 onAgentEvent 回调 → profile.hooks.eventToStatus → 页签 emoji 更新
```

### notify+轮询双通道（win10 实证修复）

notify 实时通道存在静默失效风险（win10 另一台 PC 实证：33 个信号文件残留——notify 事件丢失/目录删除重建后句柄失效），故 watcher 采用 **notify 实时 + 轮询补漏** 双通道：

- **notify 实时通道**：50ms debounce，初始化/监听失败**仅降级 warn**（不再返回 Err 终止）——实时性通道，失败不致命
- **轮询补漏通道**：每 3s（`POLL_INTERVAL`）扫描目录处理残留 `.json`（`poll_once` + `collect_signal_files` 纯函数），幂等（处理后删除）；目录被删除（卸载 hooks 的 `remove_dir_all` 等）后自动 `create_dir_all` 重建——彻底免疫事件丢失/目录重建/启动失败，积压残留被补送恢复前端状态
- 两通道同线程串行执行，无并发竞态；notify 降级时线程走 `LOOP_TICK`(250ms) sleep 节奏防忙循环

## 架构决策

### CliHooksProvider trait + 注册表（provider.rs，MC-210）

```rust
pub trait CliHooksProvider: Send + Sync + std::fmt::Debug {
    fn inject(&self) -> Result<AgentHookInjectionStatus, AppError>;
    fn uninstall(&self) -> Result<(), AppError>;
    fn injection_status(&self) -> Result<AgentHookInjectionStatus, AppError>;
    fn context_usage(&self, transcript_path: &str) -> Result<Option<ContextUsage>, AppError>;
    fn config_read(&self, layer: &str, project_path: Option<&str>, project_root: &Option<PathBuf>) -> Result<Value, AppError>;
    fn config_write(&self, layer: &str, hooks: Value, project_path: Option<&str>, project_root: &Option<PathBuf>) -> Result<(), AppError>;
}
```

注册表 = cliId 键静态映射（`REGISTRY`，条目为 `Option<&dyn CliHooksProvider>`——`None` 表示已注册但无 hooks 能力，预留分支）；`resolve_provider`/`lookup_provider` 是命令层分发唯一入口。错误语义（MC-211）：未知 cliId → `Validation("未知 cliId: ...")`；已注册但无 hooks 能力 → `Validation`（消息含「不支持 hooks 能力」语义，本期注册表仅 claude，走不到第二分支，但分支与测试已建好）。实现均为同步阻塞（含 IO），命令层经 `spawn_blocking` 串行化（硬约束 #3）。

### claude provider 下沉（MC-213：provider 内部是 claude 合法领地）

`hooks/claude/` 承载 claude hooks 全部实现（`inject.rs`/`usage.rs`/`config.rs` 整体下沉，行为零改动）：provider 内部保留全部 claude 命名与 claude 知识——`HOOK_EVENTS` 10 事件、`~/.claude/settings.json`、matcher 结构、`SCRIPT_VERSION` 检测、reporter 模板、三层配置路径。`ClaudeHooksProvider`（单元结构，静态注册表条目）实现 trait 六方法，home 解析统一走 `home_dir()`（测试经 `HomeDirGuard` 注入覆盖，L1 隔离纪律）。

### reporter 归 claude provider 资产（决策 7，MC-215）

`slterm-hook-reporter.js` 由 `../../assets/` 迁入 `hooks/claude/`，随 claude provider 归属（`include_str!` 编译期嵌入）。**决策 7**：payload 显式写 `cliId: "claude"`；`SCRIPT_VERSION` 递增——**已注入用户升级后显示「版本过旧」（Outdated）需重新注入**，预期波及，测试锁死此形态。注入目标路径 `~/.slterminal/hooks/slterm-hook-reporter.js` 不变（E2E 零波及）。C10 契约（任何路径 exit 0、不写 stderr）不改，E2E-06 链路不削弱。

### SLTERM_PANEL_ID 环境变量路由

PTY spawn 时注入 `SLTERM_PANEL_ID` 环境变量（注入点在 pty 层，见 @../pty/CLAUDE.md，与 `COLORTERM`/`TERM`/`TERM_PROGRAM` 同一时机），值等于 `request.panel_id`。reporter 脚本通过 `process.env.SLTERM_PANEL_ID` 读取，写入信号文件 `panelId` 字段。**「无此变量 exit(0)」门控语义归 reporter 实现**（C3 契约：非 slTerminal 终端启动的 claude → 脚本直接 `process.exit(0)`）。

### 脚本任何路径 exit 0（C10 契约）

`slterm-hook-reporter.js` 零依赖（仅 Node.js >= 18 内置 API），任何代码路径恒 `process.exit(0)`：stdin 为空、JSON 解析失败、目录不可写、无 `SLTERM_PANEL_ID`、顶层异常——全部静默退出。不向 stderr 输出，确保 Claude Code hook 机制不受干扰。

### settings.json 注入/卸载规则

注入时操作 `~/.claude/settings.json`：

- **merge 策略**：读现有 settings → 移除旧 slterm matcher（幂等升级）→ 10 事件每事件键下追加 `{matcher: "", hooks: [{type: "command", command: "node \"<脚本绝对路径>\"", timeout: 5}]}` → 原子写回（NamedTempFile + persist）。用户已有自定义 matcher 保留。
- **非法 JSON 中止**：settings.json 格式错误时返回 AppError，不改动文件。用户需先手动修复。
- **matcher 识别**：通过 command 字符串含 `"slterm-hook-reporter"` 判定。不含此子串的 matcher 视为用户条目，保留不动。
- **10 事件**：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`Stop`、`StopFailure`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`PermissionRequest`（C9 规定）。

卸载时：

- **安全策略**：settings.json 非法 JSON → 静默跳过配置清理（不损坏用户文件），但仍删除 `~/.slterminal/hooks/` 和 `~/.slterminal/hooks-events/` 目录。
- **清理范围（handler 级剔除）**：组内 `hooks` 数组剔除全部含 slterm 子串的 handler（**用户自定义 handler 与注入 handler 混入同一 matcher 组时仅删 slterm 条目、组保留**——与前端 `isSltermManaged` 粒度一致，验收修复：旧实现按 matcher 组级删除会连带删除组内用户条目）→ 组内 hooks 全空才删组 → 删除空事件键 → 若 hooks 段全空则移除整个 `"hooks"` 键 → 原子写回。组内任一 handler 被剔除即标记 changed 触发写盘。
- **目录删除**：`remove_dir_all` 删 `~/.slterminal/hooks/`（脚本）+ `~/.slterminal/hooks-events/`（信号文件），忽略不存在的目录。

### 版本检测

脚本模板通过 `include_str!("claude/slterm-hook-reporter.js")` 编译期嵌入。注入状态检测三态：

| 状态 | 判定条件 |
|------|---------|
| `Injected` | 脚本存在 + settings.json 含 slterm matcher + 磁盘 `SCRIPT_VERSION` === 编译期模板版本 |
| `Outdated` | 脚本存在 + settings.json 含 slterm matcher + 版本不匹配（含决策 7 SCRIPT_VERSION 递增后的已注入用户） |
| `NotInjected` | 脚本不存在、或 matcher 缺失、或 settings.json 解析失败 |

版本从脚本首行 `const SCRIPT_VERSION = N;` 提取（纯文本解析，无需执行 JS），模板版本和磁盘版本各自提取后比较。

### HookSignalWatcher 全局静态实例

watcher 使用 `static WATCHER: Mutex<Option<Box<dyn WatcherHandle>>>`（模块级，trait object 化——HUK-04 测试可注入桩），避免在 `state.rs` 的 `AppState` 中新增字段导致循环依赖。`start_signal_watcher` 幂等：已启动则跳过不报错。watcher 线程名 `hook-signal-watcher`，在 `lib.rs` 的 `.setup()` 中启动。信号目录 `~/.slterminal/hooks-events/` 单目录全 CLI 共用（路由靠 payload.panelId + cliId，不分目录，MC-203）。

### hooks 子树三层读写（claude/config.rs，P3-BE-01/02/03）

配置面板只编辑 settings.json 的 **hooks 子树**（C13-1 编辑范围），三层路径：

- `user` 层 → `~/.claude/settings.json`（`dirs::home_dir()` 解析，绕过 project_root 沙箱，照 `settings.rs`/`projects.rs` 先例）
- `project` 层 → `<projectPath>/.claude/settings.json`
- `local` 层 → `<projectPath>/.claude/settings.local.json`

project/local 层入参经 `validate_path_within_root` 沙箱校验：project_path 缺失返回 `Validation`，校验失败返回 `PathNotAllowed`（P3-BE-06/07）。非法 layer / 非法 hooks / JSON 损坏统一走 `AppError::Validation`，IO 错误走 `AppError::IoKind`（P3-BE-08）。阻塞 I/O 全部在 `spawn_blocking` 内执行（硬约束 #3）。

**read 语义**：文件不存在或无 `hooks` 键 → `Ok(Null)`（面板首次创建场景）；**JSON 损坏 → `Err`**（不返回 Null——防止面板在损坏文件上编辑后 merge 丢其他字段，对齐 C9 注入的非法中止先例）。

**write 语义**：`hooks` 必须为 JSON Object（否则 `Validation`）；后端 **read-modify-write**——读原文件 → 根对象 `hooks` 键替换为入参 → 原子写（`NamedTempFile` + `persist`，明确不做 `.bak`），`permissions`/`env`/`$schema` 等其他字段原样保留（P3-BE-03）。原文件损坏 → `Err` 拒绝覆盖；根元素为数组/标量无法安全 merge → `Err`；文件内容为 `null`（合法 JSON）视作空对象。父目录不存在时自动 `create_dir_all`（仅写入路径）。

### 信号文件瞬态特性 + dev 环境注入路径

**目录常态为空是设计行为**：`process_signal_file`（`signal.rs`）处理后无论 emit 成败均立即 `fs::remove_file` 删除文件，watcher 实时通道 debounce 仅 50ms（`watcher.rs`）。信号文件从产生到删除存活亚秒级（实时通道）或 ≤3s（轮询补漏兜底），任何时刻 `ls` 几乎都看不到文件——目录为空恰是管道正常工作的表现。如需观察信号文件，应使用文件系统监视工具（如 `watchexec`）或临时停 watcher。

**残留文件 = watcher 失效的诊断信号**：若目录持续堆积 `.json` 残留（win10 实证 33 个），说明 notify 实时通道事件丢失/目录重建后句柄失效——轮询补漏（3s）会自动清理并补送积压事件恢复前端状态；**残留持续不消则 watcher 未启动或目录重建后未恢复**，需排查 `start_signal_watcher` 启动日志与目录句柄。

**注入入口（面板 GUI 为主 + dev/E2E helper 补充）**：生产环境主入口为 hooksConfig 面板（hub 容器）claude 编辑器工具栏的「注入 Hooks」/「卸载 Hooks」按钮（F2 并入，见 `src/panels/CLAUDE.md` hooksConfig 节）；dev/E2E 构建下另有 E2E helper 可用（`E2E_ENABLED` 门控，`e2e-tests/helpers.ts`）：
- `npm run tauri dev` 启动后，devtools 控制台执行 `await window.__slterm_e2e_injectHooks()`（= `agentHooks.inject("claude")`）
- 状态查询：`await window.__slterm_e2e_getHookInjectionStatus()`（= `agentHooks.getInjectionStatus("claude")`）
- 卸载：`await window.__slterm_e2e_uninstallHooks()`（= `agentHooks.uninstall("claude")`）

生产构建（`npx tauri build`）这些 helper 被 tree-shake 排除，注入功能仍经面板 GUI 可用。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | 命令层 + 共享 DTO：6 条泛化 Tauri 命令（`agent_hooks_inject`/`agent_hooks_uninstall`/`agent_hooks_injection_status`/`agent_context_usage`/`agent_hooks_config_read`/`agent_hooks_config_write`，按 cliId 分发）+ `AgentInjectionStatus`/`AgentHookInjectionStatus`/`ContextUsage` DTO + `start_signal_watcher` + 静态 `WATCHER`（trait object 化，HUK-04） |
| `provider.rs` | `CliHooksProvider` trait（六方法）+ cliId 键静态注册表 + `resolve_provider`/`lookup_provider` 分发入口（「无 hooks 能力」Validation 分支预留） |
| `signal.rs` | 信号文件解析与处理：`AgentEventPayload` DTO（9 字段含可选 `cliId`，camelCase）、`parse_signal_file()` 纯函数、`process_signal_file()` 文件处理流程（读 → emit("agent-event") → 删） |
| `watcher.rs` | 信号目录监听器 `HookSignalWatcher`：**notify+轮询双通道**——notify（NonRecursive，50ms debounce，失败降级 warn）+ **3s 轮询补漏**（`collect_signal_files`/`poll_once` 纯函数，目录删除自动重建，免疫事件丢失/句柄失效），线程名 `hook-signal-watcher`，`stop()` 幂等 + `Drop` 清理 |
| `claude/mod.rs` | claude hooks provider：`ClaudeHooksProvider` trait 六方法实现 + `home_dir()` 统一 home 解析（测试经 `HomeDirGuard` 注入覆盖） |
| `claude/inject.rs` | 注入/卸载/状态三命令内核：`inject_impl`/`uninstall_impl`/`injection_status_impl`（路径可注入同步函数）+ `HOOK_EVENTS` 10 事件 + `remove_slterm_matchers`/`inject_matchers`/`build_matcher_entry` 纯逻辑 |
| `claude/usage.rs` | transcript token 用量查询内核：`scan_transcript_usage` + `parse_usage_line` 纯 I/O 逻辑 |
| `claude/config.rs` | hooks 配置三层读写内核（P3-BE-01/02/03）：`config_read_sync`/`config_write_sync` + `parse_layer`/`resolve_config_path`/`read_hooks_subtree`/`write_hooks_subtree` 纯逻辑 |
| `claude/slterm-hook-reporter.js` | claude provider 资产（决策 7）：Node 单文件 hook 上报脚本（`include_str!` 嵌入），零依赖，C10 契约，payload 显式 `cliId:"claude"` + `SCRIPT_VERSION=2` |

## 命令

六条泛化 Tauri 命令均在 `lib.rs` 的 `generate_handler!` 注册（旧命令名 `hooks_*` 零残留）。命令层经 `run_agent_hooks_*` 内核按 cliId 分发到 provider（`resolve_provider`），阻塞 I/O 在 `spawn_blocking` 内（硬约束 #3）。前四条读写 `~/.claude/settings.json`，绕过 project_root 路径沙箱（照 `settings.rs` 先例）；后两条（配置读写，P3-BE）仅 user 层绕过沙箱，project/local 层经 `validate_path_within_root` 校验。

### agent_hooks_inject

签名：`async fn agent_hooks_inject(cli_id: String) -> Result<AgentHookInjectionStatus, AppError>`

流程（claude provider）：确保 `~/.slterminal/hooks/` 目录存在 → 原子写脚本（NamedTempFile + persist）→ 读 `~/.claude/settings.json`（不存在或空则视为空 JSON 对象）→ 非法 JSON 返回 AppError（不改动文件）→ `remove_slterm_matchers` 清理旧段 → `inject_matchers` 追加 10 事件 matcher → 原子写回 → 返回 `{ status: "injected", version: N }`。

返回值 `AgentHookInjectionStatus`：`{ status: "injected" | "notInjected" | "outdated", version: number | null }`（C6 契约，camelCase）。

### agent_hooks_uninstall

签名：`async fn agent_hooks_uninstall(cli_id: String) -> Result<(), AppError>`

流程（claude provider）：读 `~/.claude/settings.json` → 移除全部 slterm matcher → 清理空事件键 → 若 hooks 段全空则移除整个键 → 原子写回 → `remove_dir_all` 删 `~/.slterminal/hooks/` + `~/.slterminal/hooks-events/`。JSON 非法时静默跳过配置清理，仍删目录。

### agent_hooks_injection_status

签名：`async fn agent_hooks_injection_status(cli_id: String) -> Result<AgentHookInjectionStatus, AppError>`

流程（claude provider）：检查脚本文件是否存在且为普通文件 → 检查 settings.json 中是否有 slterm matcher → 版本比对（磁盘 `SCRIPT_VERSION` vs 模板 `SCRIPT_VERSION`）→ 返回三态之一。

### agent_context_usage

签名：`async fn agent_context_usage(cli_id: String, transcript_path: String) -> Result<Option<ContextUsage>, AppError>`

参数：`transcript_path` — transcript JSONL 文件路径（来自 `AgentEventPayload.transcript_path`）。

返回：`Option<ContextUsage>`——最后一条含 `message.usage` 的行的 token 数据；无 usage 记录或文件异常返回 `null`（非 error）。

流程：`spawn_blocking` 内打开文件 → 读取尾部最多 64KB（`TRANSCRIPT_TAIL_BYTES`）→ UTF-8 解码 → 按行分割 → 从中途起始则跳过首行（截断行）→ 逆行扫描 → 对每行调用 `parse_usage_line` → 返回首个匹配的 usage。

### agent_hooks_config_read / agent_hooks_config_write（P3-BE-02/03）

签名：

```rust
async fn agent_hooks_config_read(cli_id: String, layer: String, project_path: Option<String>, state: State<'_, AppState>) -> Result<Value, AppError>
async fn agent_hooks_config_write(cli_id: String, layer: String, hooks: Value, project_path: Option<String>, state: State<'_, AppState>) -> Result<(), AppError>
```

- **layer**：仅 `"user"` / `"project"` / `"local"`，非法 → `AppError::Validation`（P3-BE-02）。
- **路径解析**：锁内读取 `state.project_root` 后 `resolve_config_path` 解析（作用域块结束时即 drop 锁守卫，避免非 Send 的 `RwLockReadGuard` 跨 await 存活）。user 层不经过沙箱；project/local 层 `project_path` 缺失 → `Validation`、沙箱校验失败 → `PathNotAllowed`（P3-BE-06/07）。
- **read**：返回该层 `hooks` 子树（非整文件）；文件不存在或无 `hooks` 键 → `Ok(Value::Null)`；JSON 损坏 → `Err(Validation)`。
- **write**：`hooks` 必须为 JSON Object；read-modify-write merge 原样保留其他字段；原文件损坏 → `Err` 拒绝覆盖；父目录自动创建；`NamedTempFile` + `persist` 原子写，不做 `.bak`。
- 阻塞 I/O 均在 `spawn_blocking` 内（硬约束 #3）。前端 wrapper 见 `src/ipc/hooksConfig.ts`（C13-1）。

## ContextUsage DTO

`mod.rs` 定义的 token 用量 DTO（C5 契约，camelCase 序列化；MC-214 四字段保留，cache serde default 0）：

```rust
pub struct ContextUsage {
    pub input_tokens: u64,             // 输入 token 数，序列化为 "inputTokens"
    pub output_tokens: u64,            // 输出 token 数，序列化为 "outputTokens"（信息字段，不计占用）
    #[serde(default)]                  // 兼容旧 transcript 缺失
    pub cache_read_input_tokens: u64,  // 缓存读取输入 token，序列化为 "cacheReadInputTokens"
    #[serde(default)]                  // 兼容旧 transcript 缺失
    pub cache_creation_input_tokens: u64, // 缓存创建输入 token，序列化为 "cacheCreationInputTokens"
}
```

**用量口径**：总占用 = `inputTokens + cacheReadInputTokens + cacheCreationInputTokens`；**contextLimit 由前端 `profile.hooks.contextLimit` 提供（claude=200_000，MC-214）**，后端 DTO 不含上限常量；`outputTokens` 不计占用保留为信息字段。缺 cache 字段默认 0（serde `default`，兼容旧 transcript）；`input_tokens` 缺失仍整行 None（沿用现状）。

前端对应 `src/types/agent.ts` 的 `ContextUsage` 接口（四字段同名，`cacheReadInputTokens` / `cacheCreationInputTokens`），IPC 封装见 `src/ipc/agentHooks.ts` 的 `contextUsage(cliId, transcriptPath)`。

## 实现要点

### 尾部读取 + 逆行扫描

`scan_transcript_usage` 从文件尾部读取最多 64KB（`TRANSCRIPT_TAIL_BYTES = 64 * 1024`），不足则全读。从中途起始时跳过首行（可能为截断行，JSON 不完整），其余行自末行逆向扫描，遇含 `message.usage` 的完整 JSON 行即解析返回。此策略确保仅读最小必要数据量，不加载全文件。cache 字段（`cache_read_input_tokens`/`cache_creation_input_tokens`）从同一条 `message.usage` 对象中提取，缺失默认 0。

### parse_usage_line 纯函数

`parse_usage_line(line: &str) -> Option<ContextUsage>` 解析单行 JSON：提取 `message.usage.input_tokens`、`message.usage.output_tokens`、`message.usage.cache_read_input_tokens`、`message.usage.cache_creation_input_tokens`（均为 `u64`，cache 字段缺失默认 0）。JSON 非法、字段缺失（除 cache 外）、类型不匹配均返回 `None`，不 panic。仅 `input_tokens` 缺失时整行 None（沿用现状）。

### 解析失败返回 None

任何异常路径均返回 `Ok(None)`（非 `Err`）：文件不存在、权限不足、UTF-8 无效字节、JSONL 全无 usage 行——调用方统一按"无 token 数据"处理，不区分具体错误类型。

## 性能实测（问题 5）

hook 脚本性能实测结论（2026-07-29，Win11 build 26200，Node v22）：

- **hook 脚本耗时**：36-44ms/次（5 次测量：44/37/36/37/36ms；裸 node 基线 35ms——`node -e "process.exit(0)"`）。
- **启动路径 hook 触发**：claude 启动生命周期仅 `SessionStart` 一个 hook 事件触发 → hooks 总贡献 ~0.1s 量级。
- **结论**：hooks **不是** claude 启动慢 1-3s 的主因——主因 = claude 自身 Windows node 模块加载 + Ink 渲染器初始化。**接受现状，不做 per-event node spawn 优化**（优化收益远低于架构复杂度成本）。

## 关键约束

- **阻塞 I/O 必须 `spawn_blocking`**：六命令（三注入/卸载命令 + `agent_context_usage` + `agent_hooks_config_read`/`agent_hooks_config_write` 两条配置命令）均经 `tokio::task::spawn_blocking` 串行化（硬约束 #3）。
- **原子写**：settings.json 和脚本文件均使用 `tempfile::NamedTempFile` + `persist()`，确保写盘原子性。
- **路径规范化**：脚本绝对路径经 `dunce::simplified()` 处理（剥 Windows `\\?\` 前缀），matcher command 中反斜杠统一替换为 `/`。
- **模板内嵌**：`claude/slterm-hook-reporter.js` 通过 `include_str!` 编译期嵌入，无需运行时读 assets 目录。
- **DTO 双边对应**：`AgentInjectionStatus` / `AgentHookInjectionStatus` / `AgentEventPayload` / `ContextUsage` 均 `snake_case` ↔ JS `camelCase`（硬约束 #4）。
- **前四条命令走绝对 home 路径**：不依赖 `project_root`（类似 `settings.rs`/`projects.rs`），故不经过路径沙箱 `validate_path_within_root`。

## 测试模式

Rust 测试分布 8 个位置（均为 `#[cfg(test)] mod tests` 嵌入源文件），共 147 用例（Stage 03 前 133 条全保留迁移 + provider 注册表/命令层 cliId 透传新增）。

| 位置 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `mod.rs` `#[cfg(test)]` | 19 | `AgentInjectionStatus`/`AgentHookInjectionStatus` serde roundtrip + 键集合精确匹配（HUK-09）、`AgentEventPayload` 9 键含 cliId serde 键集合（含无 cliId 旧信号兼容）、**start_signal_watcher（HUK-04：首次启动存实例/重复启动幂等跳过/启动失败不存/`reset_watcher_for_test` 重置钩子后重启）**、parse_signal_file 快速冒烟、**命令层 cliId 透传（HUK-02/05 式：六命令 block_on 直测 HomeDirGuard 注入 tempdir + 未知 cliId 六命令全 Validation）** |
| `provider.rs` `#[cfg(test)]` | 3 | resolve_provider（已知 cliId 命中/未知 cliId Validation）、lookup_provider 注册表注入（已注册无 hooks 能力 Validation 分支） |
| `signal.rs` `#[cfg(test)]` | 16 | parse_signal_file 全分支（合法完整/optionals null/缺 panelId/空 panelId/非法 JSON/空串/仅空白）、camelCase 序列化+反序列化往返、**process_signal_file_with（HUK-01：注入 emit 闭包——读→emit→删全流程/emit 失败仍删/非法 JSON 降级）** |
| `watcher.rs` `#[cfg(test)]` | 20 | is_signal_file、collect_signal_files、poll_once（逐个处理注入闭包/幂等二次不处理/目录删除重建后恢复/非 json 忽略/无文件零调用）、**run_one_tick（HUK-03：轮询补漏消费残留/目录重建恢复/stop 信号返回 true）**、watcher 生命周期（stop 幂等、Drop join 线程） |
| `claude/mod.rs` `#[cfg(test)]` | 1 | HomeDirGuard 注入与 Drop 恢复原 home 解析 |
| `claude/inject.rs` `#[cfg(test)]` | 35 | template_version 正值、HOOK_EVENTS 计数+唯一+关键事件、has_slterm_matchers、disk_script_version（解析/无版本/缺失/空格分号）、remove_slterm_matchers（清理 slterm 条目+保留用户 hook/清理空事件键/无 slterm 条目/**混组保用户 handler/全 slterm 组删除**——handler 级剔除）、inject_matchers（10 事件齐全/保留用户 matcher/二次注入幂等）、build_matcher_entry、模板内嵌校验（非空/含 SLTERM_PANEL_ID/含 SCRIPT_VERSION/含显式 `cliId:"claude"`）、**三命令 impl 层（HUK-02：`inject_impl`/`uninstall_impl`/`injection_status_impl` tempdir 驱动——注入/幂等/非法 JSON 中止/保留其他字段/非 Object 根与 hooks 拒绝/卸载混组保用户 handler/状态三态）** |
| `claude/usage.rs` `#[cfg(test)]` | 26 | parse_usage_line 全分支、scan_transcript_usage 集成（末行命中/中间行回溯/无 usage/文件不存在/空文件/损坏行跳过）、ContextUsage serde camelCase、**命令层（HUK-05：`run_context_usage` + `block_on`——参数透传 transcriptPath/None 与 Some 返回映射/大文件 >128KB 仅读尾部 64KB）** |
| `claude/config.rs` `#[cfg(test)]` | 27 | parse_layer、resolve_config_path（user 层 home 路径/三层拼接/缺失 project_path Validation/子树外 PathNotAllowed）、read_hooks_subtree（文件不存在 Null/无 hooks 键 Null/子树提取/损坏 Err）、write_hooks_subtree（原子写/父目录自动创建/merge 保留其他字段/损坏拒绝覆盖/非 Object hooks 拒绝无副作用/非 Object 根拒绝/null 根视空对象）——P3-BE 读写命令纯逻辑、**命令层（`run_config_read`/`run_config_write` + `block_on`——参数透传/非法 layer/路径校验）** |

### 单元测试组织

所有单元测试遵循标准 Rust 模式——`#[cfg(test)] mod tests` 嵌入源文件底部，`use super::*` 导入父模块全部项。

### 信号解析测试模式

`parse_signal_file` 是纯函数（`&str → Option<AgentEventPayload>`），无依赖，直接构造 JSON 字符串断言：

```rust
// 合法完整 JSON
let json = r#"{"panelId":"p1","event":"PreToolUse",...}"#;
let p = parse_signal_file(json).unwrap();
assert_eq!(p.panel_id, "p1");
assert_eq!(p.event, "PreToolUse");

// panelId 为空串 → None
assert!(parse_signal_file(r#"{"panelId":"","event":"SessionStart",...}"#).is_none());

// 非法 JSON → None
assert!(parse_signal_file("not json").is_none());
```

### 注入/卸载测试模式

使用 `tempfile::tempdir()` 隔离文件系统，不依赖真实 `~/.claude/settings.json`：

- **has_slterm_matchers**：直接传入 `serde_json::json!({...})` 构造 settings，验证检测逻辑。
- **disk_script_version**：`tempdir` → `std::fs::write` 写含 `SCRIPT_VERSION` 的 .js 文件 → 调用 `disk_script_version` 验证解析。
- **remove_slterm_matchers / inject_matchers**：直接操作 `serde_json::Map`，验证清理/追加/幂等行为。
- **模板内嵌校验**：`HOOK_SCRIPT_TEMPLATE` 是编译期常量，直接断言其长度和关键字符串（含显式 `cliId:"claude"`，决策 7）。

三命令内核（`inject_impl`/`uninstall_impl`/`injection_status_impl`）已抽为路径可注入的同步函数，tempdir 驱动 L1 全覆盖（HUK-02）；async 命令包装层（`spawn_blocking` 编排）仍由 L4 E2E 关键路径（P1-TE-03）验收真实 settings.json 读写。

### watcher 生命周期测试模式

`HookSignalWatcher::start()` 需要 `AppHandle`，单元测试无法构造。测试使用**手动构造模式**（同 `FileWatcher` 测试，见 `notify/CLAUDE.md`）：

```rust
let (stop_tx, stop_rx) = mpsc::channel();
let handle = std::thread::spawn(move || { let _ = stop_rx.recv(); });
let mut w = HookSignalWatcher {
    stop_tx: Some(stop_tx),
    thread_handle: Some(handle),
};
w.stop();  // 第一次停止
w.stop();  // 第二次应不 panic（幂等）
```

### usage token 用量测试模式

`usage.rs` 测试分三层：

**parse_usage_line 纯函数测试**（13 条）：无依赖，直接构造 JSON 字符串断言：
```rust
assert_eq!(parse_usage_line(r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#).unwrap(),
           ContextUsage { input_tokens: 100, output_tokens: 50 });
assert!(parse_usage_line("not json").is_none());
assert!(parse_usage_line("").is_none());
```

**scan_transcript_usage 集成测试**（6 条）：使用 `NamedTempFile` 构造 JSONL 文件，验证尾部扫描 + 逆行逻辑：
```rust
let (_dir, path) = make_temp_transcript(&[
    r#"{"message":{"usage":{"input_tokens":10,"output_tokens":5}}}"#,
    r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#,
]);
let r = scan_transcript_usage(path.to_str().unwrap()).unwrap();
assert_eq!(r.input_tokens, 100);  // 末行优先
```

**命令层测试**（3 条，HUK-05）：`run_context_usage`（命令内核）经 `block_on` await——参数透传 transcriptPath（两条不同路径各取各自 usage）、None 与 Some 返回映射、大文件（>128KB padding + usage 在最后 1KB → 仅加载尾部 64KB 仍命中，P2-TE-05 兜底）。

`scan_transcript_usage` 是纯 I/O 逻辑函数（非 async），测试直接调用不依赖 `spawn_blocking`。命令层经 `block_on` 直测 `run_context_usage`；真实 L4 E2E 仍验收完整命令链路。

### 运行

```bash
# 运行全部 hooks 测试
cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1

# 运行单个文件测试
cargo test --manifest-path src-tauri/Cargo.toml hooks::signal -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::watcher -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::claude::inject -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::claude::usage -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::claude::config -- --test-threads=1
```

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册（旧命令名 `hooks_*` 不保留兼容）。
2. 修改 `HOOK_EVENTS` 常量后跑 `hook_events_count_and_unique` + `hook_events_contains_key_events` 测试。
3. 修改 `claude/slterm-hook-reporter.js` 后：确保 `SCRIPT_VERSION` 递增（决策 7——已注入用户会显示「版本过旧」需重新注入，测试锁死此形态）+ 跑 `template_version_positive` / `template_is_non_empty` 测试确认内嵌内容正确。
4. 修改注入逻辑（`remove_slterm_matchers`/`inject_matchers`/`build_matcher_entry`）或三命令 impl 层（`inject_impl`/`uninstall_impl`/`injection_status_impl`）后跑 `claude/inject.rs` 全部 35 条测试，尤其幂等测试（`inject_impl_idempotent`）与混组保用户 handler 用例。
5. 修改 `parse_signal_file` / `process_signal_file` / `process_signal_file_with` 后跑 `signal.rs` 全部 16 条测试 + `mod.rs` 的 4 条快速冒烟测试。
6. **绝对不要修改 C10 契约**——`slterm-hook-reporter.js` 任何代码路径必须 `process.exit(0)`。新增 catch 分支时确认静默退出、不写 stderr。
7. 修改注入命令的 settings.json 读写逻辑后，务必跑 L4 E2E（`npm run e2e`）验证真实 settings.json merge/卸载/非法 JSON 中止行为（P1-TE-03）。
8. 修改 `parse_usage_line` / `scan_transcript_usage` / `TRANSCRIPT_TAIL_BYTES` / `run_context_usage` 后跑 `claude/usage.rs` 全部 26 条测试，尤其命令层三用例（参数透传/None 与 Some 映射/大文件尾部扫描）与 cache 字段用例。
9. 修改 `ContextUsage` DTO 字段后同步更新 `src/types/agent.ts`（前端 `ContextUsage` 接口）和 `src/ipc/agentHooks.ts`（IPC wrapper，cliId 首参），跑 `ipc-agent-hooks-contract.test.ts` + `context_usage_serialize_camelcase` / `context_usage_deserialize_camelcase` 测试。
10. 修改 `claude/config.rs`（`parse_layer` / `resolve_config_path` / `read_hooks_subtree` / `write_hooks_subtree` / `run_config_read` / `run_config_write`）后跑 `hooks::claude::config` 全部 27 条测试。改 read 的「损坏 → Err」或 write 的 merge 语义时，同步核对 `src/ipc/hooksConfig.ts` 与契约 C13-1（损坏文件上编辑后 merge 丢字段是设计红线）。新增配置层（如 org 层）需同步更新 `parse_layer`、`layer_file_name` 与 `src/types/hooksConfig.ts` 的 `HooksLayer`。
11. 修改 `watcher.rs`（`POLL_INTERVAL` / `LOOP_TICK` / `collect_signal_files` / `poll_once` / `run_one_tick` / notify 降级逻辑）后跑 `hooks::watcher` 全部 20 条测试。**勿削弱轮询补漏**——它是 win10 实证 watcher 静默失效的兜底（notify 事件丢失/目录重建句柄失效）。
12. 新增 provider：在 `provider.rs` 的 `REGISTRY` 注册 cliId 条目；trait 六方法签名与错误语义（未知 cliId/无 hooks 能力）勿改。
