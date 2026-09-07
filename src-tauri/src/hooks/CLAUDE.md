# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/hooks` 是 CLI hook 能力的宿主侧：把子进程（当前仅 claude）通过信号文件发回的事件转译为前端可消费的 `agent-event`。这里集中保管 claude 官方机制、statusline 桥接、settings.json 注入/卸载规则、信号目录 watcher 双通道、三层配置读写等——这些依赖 claude/Node/Windows 的外部坑与契约无法从代码本身读出，必须文档化。

## 关键约束与决策

### `CliHooksProvider` trait + cliId 注册表（MC-210）

能力按 cliId 静态注册（`provider.rs`）。未知 cliId → `Validation`；已注册但无 hooks 能力 → `Validation`（预留分支）。trait 七方法签名是跨边界契约（另有 `ensure_hooks_scripts`/`confirm_inject` 两带默认实现方法，见外部坑红线），命令层经 `spawn_blocking` 串行化（硬约束 #3）。

### claude provider 内部是 claude 合法领地（MC-213）

`hooks/claude/` 保留全部 claude 命名、官方事件、settings.json 结构、statusline 协议、`SCRIPT_VERSION` 检测与 reporter/桥接脚本模板。`ClaudeHooksProvider` 实现 trait 签名方法 + override 两带默认实现方法（`ensure_hooks_scripts`、`confirm_inject`）；home 解析统一经 `crate::home::home_dir()`（顶层共享件，ADR-0016——守卫已收编顶层，本模块不自建），测试经 `crate::home::HomeDirGuard` 注入覆盖。

### reporter 归 claude provider 资产（MC-215）

`slterm-hook-reporter.js` 通过 `include_str!` 编译期嵌入，落盘 `~/.slterminal/hooks/slterm-hook-reporter.js`，payload 显式写 `cliId: "claude"`。`SCRIPT_VERSION=6`，版本不匹配的状态检测显示 `Outdated`，需用户重新注入。

### statusline 桥接（context 官方 used_percentage 口径）

官方 `context_window.used_percentage` 只存在于 statusline stdin JSON，transcript 链路无法取得。`slterm-statusline.js` 读 stdin → 提取 `used_percentage` → 节流（取整无变化不写 + ≥1s）→ 原子写 ContextUsage 信号文件 → 透传用户原 statusline 命令。

- **包裹形态**：statusLine 被改写成 `node "<桥接脚本>" "<原命令>"`。
- **B11 注入防御**：注入/重注入前递归解包自有脚本包裹（最多 5 层），备份最内层原命令，防双重包裹。
- **B15 启动重注入**：`ClaudeHooksProvider::reinject_statusline` 必须传 `statusline_script_path()`；误传 reporter 会把 statusLine 写成 reporter 包裹，导致状态行空白 + Outdated。
- **B16 bash 分支**：`.sh` 原命令不依赖 PATH 有 bash，用 `bashCandidates` 试错定位（PATH bash → git 目录上溯 → 固定路径），且反斜杠路径转正斜杠后再传给 bash。
- **C10 契约**：reporter 与桥接脚本任何路径必须 `process.exit(0)`，不写 stderr。
- **关闭恢复**：App.tsx 关闭序列调 `agent_hooks_restore_statusline`——当前为桥接则还原备份（备份保留供下次重注入）。
- **启动对账 reconcile**（9-6 起，替代纯重注入）：`lib.rs` `.setup()` 调 `reconcile_hooks_on_startup` 遍历注册表——先按意图信号补写缺失脚本（settings 含 slterm matcher，或备份在 + statusLine == 备份原值；仅补缺失、已存在不覆盖——保 SEC-13 Outdated 审计路径），再仅当「备份存在 + 当前 statusLine 等于备份原配置」时重注入桥接。

### notify + 3s 轮询双通道（win10 实证修复）

notify 实时通道存在静默失效风险（win10 实测 33 个残留文件），故 `HookSignalWatcher` 用：
- notify NonRecursive + 50ms debounce（初始化失败仅 warn 降级）；
- 每 3s 轮询扫描目录，幂等处理残留 `.json`，目录被删后自动重建。

两通道同线程串行，无并发竞态。

### SLTERM_PANEL_ID 路由

PTY spawn 时注入 `SLTERM_PANEL_ID`（见 @../pty/CLAUDE.md）。reporter 读该变量写信号文件 `panelId`；无此变量时 reporter 直接 `process.exit(0)`。

### 信号文件瞬态与大小上限（AQ-2 / SEC-02）

- 处理完后立即删除，目录常态为空是正常表现；持续残留说明 watcher 未启动或句柄失效。
- 单文件大小上限 1MB，超限删除不处理。
- symlink 文件仅删除不读取。

### settings.json 注入/卸载规则

- **merge 策略**：读现有 settings → 移除旧 slterm matcher → 10 事件每事件追加 slterm handler → 原子写回；用户其他字段保留。
- **非法 JSON 中止**：注入时格式错误返回 `AppError`，不改动文件；卸载时（9-6 翻案，废止「静默跳过配置清理但仍删目录」）read/parse 失败返回 `AppError` 且目录全保留——「要么全清、要么全不清」，防「matcher 残留 + 脚本已删」dangling 态刷 claude 错误；修复 settings.json 后重试卸载。
- **卸载粒度**：handler 级剔除含 slterm 子串的条目，不连带删除同 matcher 组内的用户 handler。
- **10 事件**：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`Stop`、`StopFailure`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`PermissionRequest`。

### hooks 配置三层读写（P3-BE，BE-18）

- `user` → `~/.claude/settings.json`
- `project` → `<projectPath>/.claude/settings.json`
- `local` → `<projectPath>/.claude/settings.local.json`

`Layer` 枚举（`rename_all = "snake_case"`）经 ts-rs 生成前端 `HooksLayer` 字面量联合（CP-024 单源，`config.rs` 内 `#[ts(rename = "HooksLayer")]`；值集锁死用例 `layer_serde_*`）。

### 写入语义校验（SEC-05 / SEC-17）

`config_write_sync` 写盘前校验：事件名 ∈ `HOOK_EVENTS`、handler `type == "command"`、`command` 非空。校验失败返回 `Validation` 且零副作用。user 层写入前端二次确认（D9），后端以 `tracing::warn!(target: "audit")` 记录 user 层写入供审计。

### 注入状态三态（SEC-13）

`Injected`：脚本存在 + settings.json 含 slterm matcher + 磁盘 `SCRIPT_VERSION` 与模板一致 + 磁盘脚本 SHA-256 与模板一致。
`Outdated`：含 matcher 但版本或哈希不匹配。
`NotInjected`：脚本不存在、matcher 缺失或 settings 解析失败。
（CP-043 起 `AgentInjectionStatus` 枚举另含 `pendingConfirmation`——仅 inject/confirm 返回值，非本查询路径结果；序列化契约见 hooks/mod.rs DTO。）

### statusline 原命令审查（SEC-12，CP-043 起命中即暂停）

原命令来自用户配置，后端检测可疑模式（`curl`/`wget`/`Invoke-Expression` 等）——**命中即暂停注入**：inject 返回 `pendingConfirmation` + 可疑命令原文（`suspiciousCommand`），settings.json 零写盘；审计进 `target: "audit"` 通道。前端展示命令原文，用户确认后经 `agent_hooks_confirm_inject` 二次调用（跳过审查）完成注入；拒绝/不处理则保持未注入。启动重注入（reinject）路径命中 → 跳过 + 审计，不改写用户配置（无用户交互可用）。

## 外部坑/红线

- **C10 契约不可改**：reporter 和桥接脚本任何路径 exit 0、不写 stderr。
- **不要改 notify + 轮询双通道**：notify 单通道在 Win10 会丢事件，轮询补漏是兜底。
- **不要重新启用 transcript 用量链路**：官方口径只剩 statusline 的 `used_percentage`。
- **statusline 桥接脚本路径不能传错**：`reinject_statusline` 必须用 `statusline_script_path()`，不能用 `hook_script_path()`（B15）。
- **bash 路径转正斜杠**：Windows 原生 PATH 通常无 bash，定位逻辑和斜杠转换缺一不可（B16）。
- **信号目录为空是正常**：持续残留才说明 watcher 失效。
- **symlink 信号文件只删不读**（SEC-02）。
- **新增 provider 须静态注册**：`provider.rs` 的 `REGISTRY`，trait 七方法签名勿改；新增能力走带默认实现的方法——9-6 `ensure_hooks_scripts` 先例（脚本落盘型 provider override）后，`confirm_inject`（CP-043，默认 Validation「该 CLI 不支持确认注入」）为第二个带默认实现的方法，claude override = `inject_impl(..., true)`，既有实现零改动。
- **claude 会话启动时加载 hooks 配置**：注入/修复/卸载后须重启 claude 会话才生效（9-6 事故：hooks 目录被外部删除后，运行中会话仍按内存 matcher 跑缺失脚本刷 MODULE_NOT_FOUND——启动对账只保证此后新会话不再触发）。

## 测试模式

- **HomeDirGuard 注入**：守卫收编于 `crate::home::HomeDirGuard`（顶层共享，home.rs）——L1 测试经它把 home 指向 tempdir，不碰真实 `~/.claude/settings.json`（claude/mod.rs、hooks/mod.rs、watcher 信号目录用例共用）。
- **watcher 手动构造**：`HookSignalWatcher` 需要 `AppHandle`，L1 通过直接构造结构体 + 手动线程桩测试 stop 幂等/Drop 清理。
- **信号处理注入 emit 闭包**：`process_signal_file_with` 把 `emit("agent-event")` 抽为参数，L1 无需真实 AppHandle。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| Windows symlink 创建测试 | 需要管理员/developer mode | 创建失败时 skip，逻辑分支由非 symlink 用例覆盖 |
| async 命令包装层 spawn_blocking | 依赖 tokio/tauri 运行时 | L4 E2E 真实 settings.json 路径验收 |
| reporter/桥接脚本真实 Node 执行 | 依赖 claude 子进程与 Node 运行时 | L4 E2E / 人工实测 |
