# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

hooks 模块——CLI 泛化 hook 宿主侧能力层（Stage 03，MC-201~215）：信号文件通道（接收 + 解析 + 广播）、`CliHooksProvider` trait + cliId 键注册表、claude provider 下沉（注入/卸载/状态查询/statusline 桥接/三层配置读写）、6 条泛化 Tauri 命令。

核心数据流：

```
claude 子进程 hook 触发
  → reporter 脚本（claude provider 资产）读 stdin → 组装 JSON（显式 cliId）→ 原子写 .tmp → rename .json
  → HookSignalWatcher（notify+轮询双通道：notify NonRecursive 50ms debounce 实时 + 3s 轮询补漏）
  → process_signal_file（读 → parse → emit("agent-event") → 删文件）
  → 前端 onAgentEvent 回调 → profile.hooks.eventToStatus → 页签 emoji 更新
```

context 官方用量百分比通道（statusline 桥接，原 transcript token 链路已退役）：

```
claude statusline 渲染（~300ms 一次）
  → slterm-statusline.js（桥接脚本）读 stdin statusline JSON → used_percentage 提取
  → 节流（取整无变化不写 + ≥1s，状态文件跨进程共享）→ 写 ContextUsage 信号文件
  → 现有 watcher 双通道 → agent-event（usedPercentage 可选字段）
  → 前端 useAgentStatus ContextUsage 分支 → 行 usage 更新 → profile.computeUsagePercent 渲染
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
    fn restore_statusline(&self) -> Result<(), AppError>; // 关闭清理：桥接 → 还原备份（备份保留）
    fn reinject_statusline(&self) -> Result<(), AppError>; // 启动重注入：备份+原配置 → 重新注入桥接
    fn config_read(&self, layer: &str, project_path: Option<&str>, project_root: &Option<PathBuf>) -> Result<Value, AppError>;
    fn config_write(&self, layer: &str, hooks: Value, project_path: Option<&str>, project_root: &Option<PathBuf>) -> Result<(), AppError>;
}
```

注册表 = cliId 键静态映射（`REGISTRY`，条目为 `Option<&dyn CliHooksProvider>`——`None` 表示已注册但无 hooks 能力，预留分支）；`resolve_provider`/`lookup_provider` 是命令层分发唯一入口。错误语义（MC-211）：未知 cliId → `Validation("未知 cliId: ...")`；已注册但无 hooks 能力 → `Validation`（消息含「不支持 hooks 能力」语义，本期注册表仅 claude，走不到第二分支，但分支与测试已建好）。实现均为同步阻塞（含 IO），命令层经 `spawn_blocking` 串行化（硬约束 #3）。原 `context_usage` 方法（transcript token 扫描）已随官方百分比口径退役删除（七方法现行）。

### claude provider 下沉（MC-213：provider 内部是 claude 合法领地）

`hooks/claude/` 承载 claude hooks 全部实现（`inject.rs`/`config.rs` 下沉 + `slterm-hook-reporter.js`/`slterm-statusline.js` 资产；原 `usage.rs` 随 transcript 链路退役删除）：provider 内部保留全部 claude 命名与 claude 知识——`HOOK_EVENTS` 10 事件、`~/.claude/settings.json`、matcher 结构、`SCRIPT_VERSION` 检测、reporter/桥接脚本模板、statusline 协议（`context_window.used_percentage`）、三层配置路径。`ClaudeHooksProvider`（单元结构，静态注册表条目）实现 trait 七方法，home 解析统一走 `home_dir()`（测试经 `HomeDirGuard` 注入覆盖，L1 隔离纪律）。

### reporter 归 claude provider 资产（决策 7，MC-215）

`slterm-hook-reporter.js` 由 `../../assets/` 迁入 `hooks/claude/`，随 claude provider 归属（`include_str!` 编译期嵌入）。**决策 7**：payload 显式写 `cliId: "claude"`；`SCRIPT_VERSION` 递增——**已注入用户升级后显示「版本过旧」（Outdated）需重新注入**，预期波及，测试锁死此形态。注入目标路径 `~/.slterminal/hooks/slterm-hook-reporter.js` 不变（E2E 零波及）。C10 契约（任何路径 exit 0、不写 stderr）不改，E2E-06 链路不削弱。`SCRIPT_VERSION=6`（B11：reporter 与桥接脚本同步升 5；B16：桥接脚本 bash 定位修复升 6——状态检测以 reporter 版本为准，桥接脚本行为修复随重注入一并落盘）。

### statusline 桥接（context 官方用量百分比通道，原 transcript 链路退役）

`slterm-statusline.js` 同驻 `hooks/claude/`（`include_str!` 嵌入，`STATUSLINE_SCRIPT_TEMPLATE`），随 inject 一并落盘 `~/.slterminal/hooks/slterm-statusline.js`。**数据源动机**：transcript 无官方窗口总量字段、hook stdin 无 context 字段（官方 issue #32406/#49226）——官方 `context_window.used_percentage` 仅存在于 statusline stdin JSON，故注入 `statusLine` 键桥接（官方建议做法）。

- **脚本行为**：读 stdin statusline JSON → `context_window.used_percentage` 数字校验 → 节流（`Math.round` 与上次相同不写 AND 距上次 ≥1s，节流状态文件 `~/.slterminal/hooks/statusline-state.json` 跨进程共享）→ 原子写 ContextUsage 信号文件（hooks-events 目录，payload = `{panelId, cliId:"claude", event:"ContextUsage", timestamp, sessionId, cwd, usedPercentage}`——复用瞬态信号通道，读→emit→删语义不变）→ **包裹透传**：argv[2] 为用户原 statusline 命令（**B11：先剥最外层成对引号 → ~ 展开 → .sh 经 bash、其余经系统 shell**），stdin/stdout 透传、**透传失败（bash 完全不可得/命令不存在/非零退出且无输出）→ stdout 写占位文本 `[slterm-statusline: 命令执行失败]`（不写 stderr、exit 0，C10 保持）**。C10 契约不变（任何路径 exit 0、不写 stderr）
- **bash 定位与正斜杠（B16）**：`.sh` 分支不依赖 bash 在 PATH——`bashCandidates()` 按序试错：PATH 的 `bash` → `where git` 推导（git.exe 可能在 `Git\cmd` 或 `Git\mingw64\bin` 布局，沿目录上溯 3 层每层探 `bin\bash.exe`/`usr\bin\bash.exe`）→ 固定路径 `C:\Program Files\Git\bin\bash.exe` 与 `(x86)`；**bash -c 参数反斜杠转正斜杠**（`C:/Users/...` 形态——bash 词法吃未加引号反斜杠致 127）。Windows 原生 PATH 常态 = 无 bash 仅 `Git\cmd`，git 推导即覆盖 Git for Windows 全部布局
- **注入**：`inject_impl` 写桥接脚本 + settings.json 写 `statusLine` 键（`{type:"command", command:"node \"<桥接脚本>\" \"<原命令>\""}`）；**B11 注入防御：原命令提取前递归解包自有脚本包裹形态（`parse_wrapped_command`/`unwrap_wrapped_statusline`，至多 5 层，仅认 slterm-statusline.js / slterm-hook-reporter.js 脚本路径——损坏中间态 `node reporter "原命令"` 会被解出最内层命令，防双重包裹/透传末端是 reporter）；解包命中时备份值 = 最内层命令（干净原配置）**；原 statusLine 备份到 `~/.slterminal/statusline-backup.json`（原配置缺失/已为桥接 → 不备份；无原配置 → 桥接仍注入、原命令空）
- **关闭恢复**（`restore_statusline_impl`，命令 `agent_hooks_restore_statusline`，前端 App.tsx 关闭序列调用）：当前为桥接 → 还原备份（备份**保留**供重开重注入）；无备份 → 移除键；非桥接/非法 JSON → 静默跳过（关闭链路尽力而为）
- **启动重注入**（`reinject_statusline_impl`，lib.rs `.setup()` 调 `reinject_statusline_on_startup` 遍历注册表）：备份存在 + 当前 statusLine 等于备份原配置 → 重新注入桥接；无备份 / 已是桥接 / 用户已改过 / 脚本缺失 → 跳过（尊重用户现状）。**B11：original_command 提取同样套 `unwrap_wrapped_statusline`——损坏态备份重注入不复刻双重包裹**。**B15：provider 层必须传 `statusline_script_path()`（桥接脚本）——误传 `hook_script_path()`（reporter）会把 statusLine 写成 reporter 包裹（透传末端 stdout 恒空 → TUI 状态行空白 + 状态检测 Outdated），provider 层 L1 用例锁死（mod.rs `reinject_statusline_provider_uses_statusline_script`）**
- **卸载**：桥接 → 还原备份（备份缺失 → 移除键），删备份文件；hooks matcher 关闭/卸载语义不变（C3 契约：其他终端 hook 触发 SLTERM_PANEL_ID 缺失 → 静默退出）
- **状态三态扩展**：matcher + 版本一致 + statusLine 为桥接 → Injected；matcher 在但 statusLine 非桥接（关闭还原后未重注入）→ Outdated

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

### 版本检测 + 脚本哈希比对（SEC-13，S17）

脚本模板通过 `include_str!("claude/slterm-hook-reporter.js")` 编译期嵌入。注入状态检测三态：

| 状态 | 判定条件 |
|------|---------|
| `Injected` | 脚本存在 + settings.json 含 slterm matcher + 磁盘 `SCRIPT_VERSION` === 编译期模板版本 **+ 磁盘脚本 SHA-256 === 模板哈希** |
| `Outdated` | 脚本存在 + settings.json 含 slterm matcher + 版本不匹配 **或哈希不匹配**（含决策 7 SCRIPT_VERSION 递增后的已注入用户） |
| `NotInjected` | 脚本不存在、或 matcher 缺失、或 settings.json 解析失败 |

版本从脚本首行 `const SCRIPT_VERSION = N;` 提取（纯文本解析，无需执行 JS），模板版本和磁盘版本各自提取后比较。

**SEC-13 内容哈希比对**：磁盘脚本可被替换为首行版本匹配的恶意文件（首行文本可伪造）——状态检测对磁盘脚本字节计算 SHA-256（`sha256_digest` 纯函数，`sha2` crate）与编译期模板哈希比对，不一致 → Outdated。L1 测试：已知向量 / 内容一致匹配 / 缺失文件 false / **篡改脚本（版本行保持）检测 Outdated**。

### statusline 原命令可疑模式审查（SEC-12，S17）

`slterm-statusline.js` 桥接脚本透传执行 `~/.claude/settings.json` 原 statusline 命令——原命令被篡改则形成命令注入面。**信任边界（登记）**：命令来自用户自身配置，审查 = 检测可疑模式（下载器 `curl`/`wget`、PowerShell 任意执行 `Invoke-Expression` 系）时 `tracing::warn!` 告警——**仅记录不阻断**（不破坏用户自定义 statusline）。`suspicious_statusline_pattern` 纯函数（词边界防变量名/路径子串误报，如 `$MYCURLPATH` 不命中、`curl.exe` 命中；大小写不敏感），注入/重注入时对原命令执行审查（`warn_if_suspicious_statusline`）。L1 测试：命中表驱动 / 正常命令忽略 / 词边界 / 大小写 + `inject_impl`/`reinject_impl` 命中仍注入。

### HookSignalWatcher 全局静态实例

watcher 使用 `static WATCHER: Mutex<Option<Box<dyn WatcherHandle>>>`（模块级，trait object 化——HUK-04 测试可注入桩），避免在 `state.rs` 的 `AppState` 中新增字段导致循环依赖。`start_signal_watcher` 幂等：已启动则跳过不报错。watcher 线程名 `hook-signal-watcher`，在 `lib.rs` 的 `.setup()` 中启动。信号目录 `~/.slterminal/hooks-events/` 单目录全 CLI 共用（路由靠 payload.panelId + cliId，不分目录，MC-203）。

### hooks 子树三层读写（claude/config.rs，P3-BE-01/02/03）

配置面板只编辑 settings.json 的 **hooks 子树**（C13-1 编辑范围），三层路径：

- `user` 层 → `~/.claude/settings.json`（`dirs::home_dir()` 解析，绕过 project_root 沙箱，照 `settings.rs`/`projects.rs` 先例）
- `project` 层 → `<projectPath>/.claude/settings.json`
- `local` 层 → `<projectPath>/.claude/settings.local.json`

**Layer 枚举（BE-18，S10）**：`parse_layer` 返回 `Layer::User/Project/Local` 枚举（serde `rename_all = "snake_case"`，与前端 `HooksLayer = "user"|"project"|"local"` 字面量联合双边对应——硬约束 #4/DOC-06 语义值集同步登记）。非法 layer / 非法 hooks / JSON 损坏统一走 `AppError::Validation`，IO 错误走 `AppError::IoKind`（P3-BE-08）。阻塞 I/O 全部在 `spawn_blocking` 内执行（硬约束 #3）。

**写入语义校验（SEC-05，S17，D9）**：`config_write_sync` 写盘前经 `validate_hooks_semantics` 三规则校验（基于 BE-18 所建 `HooksSubtree`/`MatcherGroup`/`HookHandler` 结构体反序列化校验形态）——事件名 ∈ `HOOK_EVENTS`（10 事件白名单，复用 inject.rs 单点）、handler `type == "command"`、`command` 为非空字符串（缺失/null/空串/纯空白全拒）；校验失败返回 `AppError::Validation` **且零副作用**（不写盘）。**user 层写入时前端 confirmDialog 二次确认**（D9，project/local 层不确认）。**威胁模型登记（SEC-17）**：user 层二次确认 = UX 层非安全边界（同进程信任模型，恶意前端代码本可绕过任何后端门控）；后端以 `tracing::warn!(target: "audit")` 记录 user 层写入供事后审计——**已由 L1 tracing-test 锁死（TQ-COV-05 翻案原「L1 不可断言」豁免理由）**：`user_layer_write_emits_audit_log`（正向 logs_contain）/`project_layer_write_no_audit_log`（对偶逆断言）两例，dev-dependencies 引入 `tracing-test`（`no-env-filter` feature——audit target 不以 crate 名开头，默认 EnvFilter 会丢弃致断言恒失败）。L1 测试：`semantics_rejects_*` 五条 + `config_write_sync_*` 四条（含无副作用断言）+ 审计日志两例。

**read 语义**：文件不存在或无 `hooks` 键 → `Ok(Null)`（面板首次创建场景）；**JSON 损坏 → `Err`**（不返回 Null——防止面板在损坏文件上编辑后 merge 丢其他字段，对齐 C9 注入的非法中止先例）。

**write 语义**：`hooks` 入参为 `null` 视作空对象 `{}` 进行 merge（ZQ-5 决策 3——与 read 返回 null 对称；语义 = 清空该层 hooks）；非 `null` 且非 JSON Object → `Validation`。后端 **read-modify-write**——读原文件 → 根对象 `hooks` 键替换为入参 → 原子写（`NamedTempFile` + `persist`，明确不做 `.bak`），`permissions`/`env`/`$schema` 等其他字段原样保留（P3-BE-03）。原文件损坏 → `Err` 拒绝覆盖；根元素为数组/标量无法安全 merge → `Err`；文件内容为 `null`（合法 JSON）视作空对象。父目录不存在时自动 `create_dir_all`（仅写入路径）。

### 信号文件瞬态特性 + dev 环境注入路径

**信号文件大小上限（AQ-2）**：信号文件读取前校验大小——超过 `MAX_SIGNAL_FILE_BYTES`（1MB，常量见 `signal.rs`）→ `tracing::warn!` 告警 + 删除文件不处理（不 emit，与「解析失败仍删」容错语义一致）。

**symlink 仅删除不读取（SEC-02，S02）**：`process_signal_file_with` 与 `collect_signal_files` 改用 `fs::symlink_metadata` + `is_symlink()` 检查——信号目录 `.json` 符号链接文件**仅删除不读取**（防 symlink 越界读取经 agent-event 泄露）。L1 测试：`process_symlink_signal_deletes_without_read` / `collect_excludes_symlink_files`（Windows symlink 需管理员/developer mode，创建失败 skip 并注释——`#[cfg(windows)]` 豁免登记，BE-17/D5）。

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
| `mod.rs` | 命令层 + 共享 DTO：6 条泛化 Tauri 命令（`agent_hooks_inject`/`agent_hooks_uninstall`/`agent_hooks_injection_status`/`agent_hooks_restore_statusline`/`agent_hooks_config_read`/`agent_hooks_config_write`，按 cliId 分发）+ `AgentInjectionStatus`/`AgentHookInjectionStatus` DTO + `start_signal_watcher` + `reinject_statusline_on_startup`（启动重注入，遍历注册表）+ 静态 `WATCHER`（trait object 化，HUK-04） |
| `provider.rs` | `CliHooksProvider` trait（七方法）+ cliId 键静态注册表 + `resolve_provider`/`lookup_provider` 分发入口（「无 hooks 能力」Validation 分支预留） |
| `signal.rs` | 信号文件解析与处理：`AgentEventPayload` DTO（10 字段 camelCase——可选 `cliId` + 可选 `usageSourcePath` + 可选 `usedPercentage`（ContextUsage 信号字段）；`#[serde(default)]` 不加 alias——旧键（transcriptPath）信号降级 None，仅丢该事件用量拉取）、`parse_signal_file()` 纯函数、`process_signal_file()` 文件处理流程（读 → emit("agent-event") → 删；**SEC-02 symlink 仅删除不读取**；**BE-20 模块级 `#![allow(dead_code)]` 已移除**——API 已被 watcher.rs 消费，clippy 零警告） |
| `watcher.rs` | 信号目录监听器 `HookSignalWatcher`：**notify+轮询双通道**——notify（NonRecursive，50ms debounce，失败降级 warn）+ **3s 轮询补漏**（`collect_signal_files`/`poll_once` 纯函数，目录删除自动重建，免疫事件丢失/句柄失效），线程名 `hook-signal-watcher`，`stop()` 幂等 + `Drop` 清理；**SEC-02 symlink 过滤（`is_symlink`，broken symlink 亦识别）** |
| `claude/mod.rs` | claude hooks provider：`ClaudeHooksProvider` trait 七方法实现（restore/reinject statusline 内核委托 inject.rs）+ `home_dir()` 统一 home 解析（测试经 `HomeDirGuard` 注入覆盖） |
| `claude/inject.rs` | 注入/卸载/状态/statusline 桥接内核：`inject_impl`/`uninstall_impl`/`injection_status_impl`/`restore_statusline_impl`/`reinject_statusline_impl`（路径可注入同步函数）+ `HOOK_EVENTS` 10 事件 + `remove_slterm_matchers`/`inject_matchers`/`build_matcher_entry`/`statusline_is_bridge`/`build_bridge_statusline`/`parse_wrapped_command`/`unwrap_wrapped_statusline`（B11 包裹形态递归解包）纯逻辑 |
| `claude/config.rs` | hooks 配置三层读写内核（P3-BE-01/02/03）：`config_read_sync`/`config_write_sync` + `parse_layer`/`resolve_config_path`/`read_hooks_subtree`/`write_hooks_subtree` 纯逻辑 |
| `claude/slterm-hook-reporter.js` | claude provider 资产（决策 7）：Node 单文件 hook 上报脚本（`include_str!` 嵌入），零依赖，C10 契约，payload 显式 `cliId:"claude"` + `usageSourcePath` 键（值 = stdin 协议 `data.transcript_path`，snake_case 不动）+ `SCRIPT_VERSION=6`（B11 与桥接脚本同步升 5、B16 升 6） |
| `claude/slterm-statusline.js` | claude provider 资产（statusline 桥接）：Node 单文件桥接脚本（`include_str!` 嵌入），零依赖，C10 契约——读 statusline stdin JSON 提取官方 `used_percentage` → 节流（取整无变化不写 + ≥1s）→ ContextUsage 信号文件 → 包裹透传用户原 statusline 命令（argv[2]，B11：引号容忍 + 剥引号后 ~ 展开 + 失败 stdout 占位） |

## 命令

六条泛化 Tauri 命令均在 `lib.rs` 的 `generate_handler!` 注册（旧命令名 `hooks_*` 零残留；原 `agent_context_usage` 已随 transcript 链路退役删除）。命令层经 `run_agent_hooks_*` 内核按 cliId 分发到 provider（`resolve_provider`），阻塞 I/O 在 `spawn_blocking` 内（硬约束 #3）。前四条读写 `~/.claude/settings.json`，绕过 project_root 路径沙箱（照 `settings.rs` 先例）；后两条（配置读写，P3-BE）仅 user 层绕过沙箱，project/local 层经 `validate_path_within_root` 校验。

### agent_hooks_inject

签名：`async fn agent_hooks_inject(cli_id: String) -> Result<AgentHookInjectionStatus, AppError>`

流程（claude provider）：确保 `~/.slterminal/hooks/` 目录存在 → 原子写 reporter + statusline 桥接脚本（NamedTempFile + persist）→ 读 `~/.claude/settings.json`（不存在或空则视为空 JSON 对象）→ 非法 JSON 返回 AppError（不改动文件）→ `remove_slterm_matchers` 清理旧段 → `inject_matchers` 追加 10 事件 matcher → statusLine 桥接（备份原配置 + 写桥接键）→ 原子写回 → 返回 `{ status: "injected", version: N }`。

返回值 `AgentHookInjectionStatus`：`{ status: "injected" | "notInjected" | "outdated", version: number | null }`（C6 契约，camelCase）。

### agent_hooks_uninstall

签名：`async fn agent_hooks_uninstall(cli_id: String) -> Result<(), AppError>`

流程（claude provider）：读 `~/.claude/settings.json` → 移除全部 slterm matcher → 清理空事件键 → 若 hooks 段全空则移除整个键 → statusLine 桥接还原备份（备份缺失 → 移除键）→ 原子写回 → 删 statusline 备份文件 → `remove_dir_all` 删 `~/.slterminal/hooks/` + `~/.slterminal/hooks-events/`。JSON 非法时静默跳过配置清理，仍删目录。

### agent_hooks_injection_status

签名：`async fn agent_hooks_injection_status(cli_id: String) -> Result<AgentHookInjectionStatus, AppError>`

流程（claude provider）：检查脚本文件是否存在且为普通文件 → 检查 settings.json 中是否有 slterm matcher → 版本比对（磁盘 `SCRIPT_VERSION` vs 模板 `SCRIPT_VERSION`）→ statusLine 桥接检查（非桥接 → Outdated）→ 返回三态之一。

### agent_hooks_restore_statusline

签名：`async fn agent_hooks_restore_statusline(cli_id: String) -> Result<(), AppError>`

流程（claude provider）：`restore_statusline_impl`——当前 statusLine 为桥接 → 还原备份原配置（**备份保留**供重开重注入）；无备份 → 移除键；非桥接/无 settings/非法 JSON → 静默跳过（客户端关闭序列调用，App.tsx，失败静默 catch 不阻断关闭）。

### agent_hooks_config_read / agent_hooks_config_write（P3-BE-02/03）

签名：

```rust
async fn agent_hooks_config_read(cli_id: String, layer: String, project_path: Option<String>, state: State<'_, AppState>) -> Result<Value, AppError>
async fn agent_hooks_config_write(cli_id: String, layer: String, hooks: Value, project_path: Option<String>, state: State<'_, AppState>) -> Result<(), AppError>
```

- **layer**：仅 `"user"` / `"project"` / `"local"`，非法 → `AppError::Validation`（P3-BE-02）。
- **路径解析**：锁内读取 `state.project_root` 后 `resolve_config_path` 解析（作用域块结束时即 drop 锁守卫，避免非 Send 的 `RwLockReadGuard` 跨 await 存活）。user 层不经过沙箱；project/local 层 `project_path` 缺失 → `Validation`、沙箱校验失败 → `PathNotAllowed`（P3-BE-06/07）。
- **read**：返回该层 `hooks` 子树（非整文件）；文件不存在或无 `hooks` 键 → `Ok(Value::Null)`；JSON 损坏 → `Err(Validation)`。
- **write**：`hooks` 为 `null` 视作空对象 `{}`（语义 = 清空该层 hooks）；非 `null` 且非 JSON Object → `Validation`（ZQ-5 决策 3）；read-modify-write merge 原样保留其他字段；原文件损坏 → `Err` 拒绝覆盖；父目录自动创建；`NamedTempFile` + `persist` 原子写，不做 `.bak`。
- 阻塞 I/O 均在 `spawn_blocking` 内（硬约束 #3）。前端 wrapper 见 `src/ipc/hooksConfig.ts`（C13-1）。

## ContextUsage 信号（官方 used_percentage 口径）

原 `ContextUsage` DTO（transcript token 四字段）与 `agent_context_usage` 命令已随官方百分比口径**整体退役删除**。现行数据通道：

- **信号字段**：`AgentEventPayload.used_percentage: Option<f64>`（serde default，旧信号兼容）——ContextUsage 信号由 statusline 桥接脚本写入（`usedPercentage` = claude 官方 `context_window.used_percentage`，0–100 float，未取整）
- **事件名**：`ContextUsage`（前端常量 `CONTEXT_USAGE_EVENT`，profiles/claude——claude 合法领地，AC-5）
- **前端消费**：`useAgentStatus` ContextUsage 分支 → 行 `usage: ContextUsageSignal { usedPercentage }` → `profile.hooks.computeUsagePercent`（claude = round + clamp 0–100）→ 渲染百分比/四档配色
- **口径正确性来源**：claude code 官方百分比（已知模型窗口/auto-compact 等内部口径）；transcript 单行 usage 是单次请求输入、且无窗口总量字段（官方 issue #32406/#49226）

## 性能实测（问题 5）

hook 脚本性能实测结论（2026-07-29，Win11 build 26200，Node v22）：

- **hook 脚本耗时**：36-44ms/次（5 次测量：44/37/36/37/36ms；裸 node 基线 35ms——`node -e "process.exit(0)"`）。
- **启动路径 hook 触发**：claude 启动生命周期仅 `SessionStart` 一个 hook 事件触发 → hooks 总贡献 ~0.1s 量级。
- **结论**：hooks **不是** claude 启动慢 1-3s 的主因——主因 = claude 自身 Windows node 模块加载 + Ink 渲染器初始化。**接受现状，不做 per-event node spawn 优化**（优化收益远低于架构复杂度成本）。

> statusline 桥接同型考量：claude statusline 每 ~300ms 渲染一次、每帧 spawn node——节流（取整无变化不写 + ≥1s）把信号文件写入与 watcher/前端事件洪泛挡在管道最前端；透传执行用户脚本的额外耗时与 claude 直接执行用户 statusline 脚本同量级（官方机制本身如此）。

## 关键约束

- **阻塞 I/O 必须 `spawn_blocking`**：六命令（三注入/卸载命令 + `agent_hooks_restore_statusline` + `agent_hooks_config_read`/`agent_hooks_config_write` 两条配置命令）均经 `tokio::task::spawn_blocking` 串行化（硬约束 #3）。
- **原子写**：settings.json、脚本文件、备份文件均使用 `tempfile::NamedTempFile` + `persist()`，确保写盘原子性。
- **路径规范化**：脚本绝对路径经 `dunce::simplified()` 处理（剥 Windows `\\?\` 前缀），matcher command 中反斜杠统一替换为 `/`。
- **模板内嵌**：`claude/slterm-hook-reporter.js` 与 `claude/slterm-statusline.js` 通过 `include_str!` 编译期嵌入，无需运行时读 assets 目录。
- **DTO 双边对应**：`AgentInjectionStatus` / `AgentHookInjectionStatus` / `AgentEventPayload`（含 `usedPercentage`）均 `snake_case` ↔ JS `camelCase`（硬约束 #4）。
- **前四条命令走绝对 home 路径**：不依赖 `project_root`（类似 `settings.rs`/`projects.rs`），故不经过路径沙箱 `validate_path_within_root`。
- **启动重注入**：`lib.rs` `.setup()` 调 `reinject_statusline_on_startup`（遍历 `provider::REGISTRY`，失败仅 warn 不阻断启动）；关闭恢复经前端 App.tsx 关闭序列调 `agent_hooks_restore_statusline`（备份保留 = 关闭恢复/重开重注入闭环）。

## 测试模式

Rust 测试分布 7 个位置（均为 `#[cfg(test)] mod tests` 嵌入源文件），共 180 用例（原 146——`usage.rs` 26 条随 transcript 链路退役删除，statusline 桥接新增 17 条，B11 解包新增 7 条，B15 provider 层 reinject 路径新增 1 条，**S02 SEC-02 symlink +2、S10 BE-18 Layer/子树形态 +11、S17 SEC-05 语义校验 +9 + SEC-12 审查 +5 + SEC-13 哈希 +4**）。

| 位置 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `mod.rs` `#[cfg(test)]` | 19 | `AgentInjectionStatus`/`AgentHookInjectionStatus` serde roundtrip + 键集合精确匹配（HUK-09）、`AgentEventPayload` 10 键含 cliId/usedPercentage serde 键集合（含无 cliId 旧信号兼容）、**start_signal_watcher（HUK-04：首次启动存实例/重复启动幂等跳过/启动失败不存/`reset_watcher_for_test` 重置钩子后重启）**、parse_signal_file 快速冒烟、**命令层 cliId 透传（六命令含 restore_statusline block_on 直测 HomeDirGuard 注入 tempdir + 未知 cliId 六命令全 Validation）** |
| `provider.rs` `#[cfg(test)]` | 3 | resolve_provider（已知 cliId 命中/未知 cliId Validation）、lookup_provider 注册表注入（已注册无 hooks 能力 Validation 分支） |
| `signal.rs` `#[cfg(test)]` | 19 | parse_signal_file 全分支（合法完整/optionals null/缺 panelId/空 panelId/非法 JSON/空串/仅空白）、camelCase 序列化+反序列化往返（10 键含 `usedPercentage`）、**ContextUsage 信号反序列化（usedPercentage float 保真）**、**旧信号兼容（无 cliId 键/无 usageSourcePath 键/无 usedPercentage 键 → 缺省 None——决策 1 降级语义）**、**process_signal_file_with（HUK-01：注入 emit 闭包——读→emit→删全流程/emit 失败仍删/非法 JSON 降级）**、**SEC-02 symlink 仅删除不读取（`process_symlink_signal_deletes_without_read`，Windows symlink 创建失败 skip 豁免 `#[cfg(windows)]`——BE-17/D5）** |
| `watcher.rs` `#[cfg(test)]` | 21 | is_signal_file、collect_signal_files（**SEC-02 排除 symlink：`collect_excludes_symlink_files`**）、poll_once（逐个处理注入闭包/幂等二次不处理/目录删除重建后恢复/非 json 忽略/无文件零调用）、**run_one_tick（HUK-03：轮询补漏消费残留/目录重建恢复/stop 信号返回 true）**、watcher 生命周期（stop 幂等、Drop join 线程） |
| `claude/mod.rs` `#[cfg(test)]` | 2 | HomeDirGuard 注入与 Drop 恢复原 home 解析 + **B15 防复发 `reinject_statusline_provider_uses_statusline_script`**（provider 层真实三件套 tempdir 驱动——reinject 后 statusLine 含 slterm-statusline 不含 slterm-hook-reporter；impl 层测试传参正确掩盖的 provider 路径 bug 由本用例锁死） |
| `claude/inject.rs` `#[cfg(test)]` | 67 | template_version 正值（=6）、HOOK_EVENTS 计数+唯一+关键事件、has_slterm_matchers、disk_script_version（解析/无版本/缺失/空格分号）、remove_slterm_matchers（清理 slterm 条目+保留用户 hook/清理空事件键/无 slterm 条目/**混组保用户 handler/全 slterm 组删除**——handler 级剔除）、inject_matchers（10 事件齐全/保留用户 matcher/二次注入幂等）、build_matcher_entry、模板内嵌校验（reporter 非空/含 SLTERM_PANEL_ID/含 SCRIPT_VERSION/含显式 `cliId:"claude"` + **桥接脚本非空/含 used_percentage/含 SLTERM_PANEL_ID/含 ContextUsage 事件/含 SCRIPT_VERSION/含失败占位文案（B11）**）、**三命令 impl 层（HUK-02：`inject_impl`/`uninstall_impl`/`injection_status_impl` tempdir 驱动——注入/幂等/非法 JSON 中止/保留其他字段/非 Object 根与 hooks 拒绝/卸载混组保用户 handler/状态三态含 statusLine 桥接检查）**、**statusline 桥接全表：注入写桥接+备份原配置/无原配置不备份仍注入/幂等不重建桥接/卸载还原备份+删备份/卸载无备份移除键/restore 三态（还原且备份保留/非桥接 no-op/无 settings no-op）/reinject 四态（备份+原配置重注入/用户已改过尊重/已是桥接或备份缺失 no-op/脚本缺失 no-op）/status 非桥接 Outdated**、**B11 解包 ×7（unwrap 单层/双层（含转义引号还原）/非包裹/外来 node 包裹 + inject 损坏态解包（备份干净 + restore/reinject 闭环）+ inject 双层损坏态 + reinject 损坏备份解包）**、**SEC-12 可疑模式审查 +5（`suspicious_pattern_detects_downloaders_and_iex`/`_ignores_normal_commands`/`_word_boundary_no_false_positive`/`_case_insensitive` + `inject_impl_suspicious_statusline_warns_but_injects`/`reinject_impl_suspicious_statusline_warns_but_reinjects`）**、**SEC-13 哈希比对 +4（`sha256_digest_known_vector`/`disk_script_matches_template_when_content_equal`/`_missing_file_false` + `tampered_script_with_matching_first_line_detected_outdated`/`_without_version_line_detected_outdated`）** |
| `claude/config.rs` `#[cfg(test)]` | 49 | parse_layer、resolve_config_path（user 层 home 路径/三层拼接/缺失 project_path Validation/子树外 PathNotAllowed）、read_hooks_subtree（文件不存在 Null/无 hooks 键 Null/子树提取/损坏 Err）、write_hooks_subtree（原子写/父目录自动创建/merge 保留其他字段/损坏拒绝覆盖/非 Object hooks 拒绝无副作用/非 Object 根拒绝/null 根视空对象）、**config_write_sync null 入参视空对象（ZQ-5：hooks=Null 写入 → hooks 键 = 空对象且保留其他字段）**——P3-BE 读写命令纯逻辑、**命令层（`run_config_read`/`run_config_write` + `block_on`——参数透传/非法 layer/路径校验）**、**BE-18 Layer 枚举 + 子树形态 +11（`layer_serde_serializes_snake_case`/`_deserializes_snake_case`/`_rejects_invalid`、`hooks_subtree_accepts_well_formed`/`_empty_object_accepted`/`_rejects_non_object_root`、`matcher_group_shape_validated`、`handler_must_be_object`/`_missing_type_and_command_accepted_by_shape`、`unknown_event_and_handler_fields_tolerated`、`hooks_subtree_serialize_matches_frontend_dto_shape`）**、**SEC-05 语义校验 +9（`semantics_rejects_unknown_event`/`_invalid_handler_type`/`_empty_or_missing_command`/`_shape_invalid_subtree`、`semantics_accepts_legal_hooks`/`_empty_subtree` + `config_write_sync_rejects_illegal_event_name_no_side_effect`/`_invalid_handler_type`/`_empty_command` + `config_write_sync_accepts_legal_hooks_and_writes`）** |

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

### statusline 桥接内核测试模式

`restore_statusline_impl` / `reinject_statusline_impl` 为路径可注入同步函数（settings/备份/脚本三路径入参），tempdir 驱动——注入（含备份）→ 关闭恢复（备份保留）→ 重开重注入 闭环全链路在单用例内断言；用户已改过/无备份/已是桥接/脚本缺失各 no-op 分支独立用例。statusline 桥接判定 `statusline_is_bridge`（command 含 `slterm-statusline`）与桥接构造 `build_bridge_statusline`（原命令 argv 内嵌）为纯函数，直接断言输出形态。

### 运行

```bash
# 运行全部 hooks 测试
cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1

# 运行单个文件测试
cargo test --manifest-path src-tauri/Cargo.toml hooks::signal -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::watcher -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::claude::inject -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::claude::config -- --test-threads=1
```

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册（旧命令名 `hooks_*` 不保留兼容）。
2. 修改 `HOOK_EVENTS` 常量后跑 `hook_events_count_and_unique` + `hook_events_contains_key_events` 测试。
3. 修改 `claude/slterm-hook-reporter.js` 后：确保 `SCRIPT_VERSION` 递增（决策 7——已注入用户会显示「版本过旧」需重新注入，测试锁死此形态）+ 跑 `template_version_positive` / `template_is_non_empty` 测试确认内嵌内容正确。
4. 修改注入逻辑（`remove_slterm_matchers`/`inject_matchers`/`build_matcher_entry`/`statusline_is_bridge`/`build_bridge_statusline`）或五命令 impl 层（`inject_impl`/`uninstall_impl`/`injection_status_impl`/`restore_statusline_impl`/`reinject_statusline_impl`）后跑 `claude/inject.rs` 全部 56 条测试，尤其幂等测试（`inject_impl_idempotent`/`inject_idempotent_keeps_existing_bridge`）与混组保用户 handler 用例。**修改 `claude/mod.rs` 的 `reinject_statusline` 时，script_path 必须传 `statusline_script_path()`——B15：误传 `hook_script_path()`（reporter）会把 statusLine 写成 reporter 包裹（透传末端 stdout 恒空 + 状态检测 Outdated），跑 mod.rs 的 `reinject_statusline_provider_uses_statusline_script` 防复发用例。**
5. 修改 `parse_signal_file` / `process_signal_file` / `process_signal_file_with` 或 `AgentEventPayload`（含 `used_percentage` 字段）后跑 `signal.rs` 全部 18 条测试 + `mod.rs` 的 4 条快速冒烟测试。
6. **绝对不要修改 C10 契约**——`slterm-hook-reporter.js` 与 `slterm-statusline.js` 任何代码路径必须 `process.exit(0)`。新增 catch 分支时确认静默退出、不写 stderr。
7. 修改注入命令的 settings.json 读写逻辑后，务必跑 L4 E2E（`npm run e2e`）验证真实 settings.json merge/卸载/非法 JSON 中止行为（P1-TE-03）。
8. 修改 `slterm-statusline.js` 后：桥接脚本行为变更须同步 `STATUSLINE_SCRIPT_TEMPLATE` 内嵌校验测试（`statusline_template_is_non_empty_and_contains_contract`）；影响节流/信号契约时同步更新 `src/types/agent.ts` 与 `ipc-agent-hooks-contract.test.ts`；`SCRIPT_VERSION` 递增（已注入用户 Outdated 重注入，reporter 同步升）。**B16：修改 bash 分支（bashCandidates 定位/正斜杠转换）后跑 `statusline-bridge-behavior.test.ts` 全部 10 用例**（本机 PATH 无 bash 时用例 1/8 经 git 推导真实执行，非 skip）。
9. 修改 `used_percentage` 信号字段语义后同步更新 `src/types/agent.ts`（`ContextUsageSignal` / `AgentEventPayload.usedPercentage`）与前端 `profiles/claude/strategies.ts` 的 `computeUsagePercent`，跑 `cli-profile-claude.test.ts` + `ipc-agent-hooks-contract.test.ts`。
10. 修改 `claude/config.rs`（`parse_layer` / `resolve_config_path` / `read_hooks_subtree` / `write_hooks_subtree` / `run_config_read` / `run_config_write` / SEC-05 语义校验 `validate_hooks_semantics`）后跑 `hooks::claude::config` 全部 49 条测试。改 read 的「损坏 → Err」或 write 的 merge 语义时，同步核对 `src/ipc/hooksConfig.ts` 与契约 C13-1（损坏文件上编辑后 merge 丢字段是设计红线）。**SEC-05 三规则（事件名白名单/type==command/command 非空）不可削弱**——校验失败零副作用契约由 `config_write_sync_rejects_*_no_side_effect` 锁死；改规则同步前端 `src/panels/hooksConfig/useHooksConfig.ts` 的 user 层确认弹窗（D9）。新增配置层（如 org 层）需同步更新 `parse_layer`、`layer_file_name` 与 `src/types/hooksConfig.ts` 的 `HooksLayer`（BE-18 双边语义值集同步）。
11. 修改 `claude/inject.rs` 的 SEC-13 哈希比对（`sha256_digest`/`disk_script_sha256_matches_template`）后跑 `sha256_digest_known_vector` + `tampered_script_with_matching_first_line_detected_outdated` 等哈希用例；修改 SEC-12 可疑模式表（`suspicious_statusline_pattern`）后跑 `suspicious_pattern_*` 四条（词边界防误报）。
11. 修改 `watcher.rs`（`POLL_INTERVAL` / `LOOP_TICK` / `collect_signal_files` / `poll_once` / `run_one_tick` / notify 降级逻辑）后跑 `hooks::watcher` 全部 20 条测试。**勿削弱轮询补漏**——它是 win10 实证 watcher 静默失效的兜底（notify 事件丢失/目录重建句柄失效）。
12. 新增 provider：在 `provider.rs` 的 `REGISTRY` 注册 cliId 条目；trait 七方法签名与错误语义（未知 cliId/无 hooks 能力）勿改。
