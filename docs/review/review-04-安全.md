# review-04 安全

> 维度：SEC 机制回归 + 注入面 + 用户文件写操作安全。只写问题。

## 问题条目

### AQ-1（P1）`buildResumeCommand` 单引号包裹 cwd 未转义，存在命令注入/解析失败
- 位置：`src/features/cliProfiles/profiles/claude/strategies.ts:109-115`
- 问题：函数用 `cd '${session.cwd}' && claude --resume ${session.sessionId}` 构造命令。`cwd` 直接来自 JSONL 内容解析（`AgentHistorySession.cwd`），未转义其中的单引号字符。
- 攻击/失误场景：攻击者若能在 `~/.claude/projects/` 下写入或篡改某会话 JSONL，将 `cwd` 字段设为 `C:\Users\attacker'; Start-Process notepad; '` 等含单引号内容。用户右键「复制恢复命令」并粘贴到 PowerShell 执行时，单引号提前闭合，后续字符串被解释为新的 PowerShell 语句，导致任意命令执行；正常用户若路径含单引号（如 `C:\Bob's Project`）也会导致命令解析失败。
- 修复建议：对 `cwd` 中的单引号按 PowerShell 规则替换为 `''`（两个单引号），或改用数组参数/ProcessStartInfo 方式构造命令；补充 cwd 含单引号的回归测试。
- 校注（汇总核实）：`strategies.ts:106-107` 注释已自述「单引号未转义，原实现遗留限制，原样保留」——非本次重构引入的回归，本条将其升级为注入面标注；修复即变更既定「行为零改动保留」，需同步更新 L2「输出与迁出源逐字一致」断言。
- 来源：独立发现

### AQ-2（P2）信号文件解析缺少大小限制，恶意超大 JSON 可造成内存压力
- 位置：`src-tauri/src/hooks/signal.rs:48-55`、`src-tauri/src/hooks/signal.rs:72-78`
- 问题：`parse_signal_file` 直接对 `fs::read_to_string(path)` 的完整内容调用 `serde_json::from_str`，未限制文件大小；`process_signal_file_with`  likewise 整文件读入内存。
- 攻击/失误场景：攻击者若能向 `~/.slterminal/hooks-events/` 写入一个 GB 级的 `.json` 文件（或利用 reporter 脚本异常生成超大文件），watcher 3 秒轮询或 notify 事件会触发读入与解析，消耗大量内存/CPU，造成 DoS。虽然需要文件系统访问权限，但信号目录本身由本应用创建且运行时存在。
- 修复建议：在 `process_signal_file_with` 读取前用 `metadata()` 限制文件大小（如 1MB），超限直接 warn 并删除；或改用流式/限量读取。
- 来源：独立发现

### AQ-3（P2）`agent_history_delete` 删除链可能跟随扫描根下符号链接
- 位置：`src-tauri/src/agent_history/claude/ops.rs:43-56`、`src-tauri/src/agent_history/claude/ops.rs:74-84`
- 问题：`locate_session_jsonl` 用 `dir_path.is_dir()` 与 `candidate.is_file()` 判定，未显式禁用符号链接跟随；`std::fs::remove_file`/`remove_dir_all` 会删除符号链接指向的真实目标。
- 攻击/失误场景：若扫描根 `~/.claude/projects/` 下某项目目录实际为指向外部目录的符号链接（如 `C--Users-bob-app` 指向 `C:\RealApp`），`delete_session` 会定位并删除外部目录中的 `<sessionId>.jsonl`，随后 `remove_dir_all` 还会删除外部 `<sessionId>/` 目录及其 subagents 数据，造成越界数据丢失。Windows 创建符号链接通常需管理员权限或开发者模式，属受限但存在的攻击面。
- 修复建议：在 `is_dir()`/`is_file()` 判定后增加 `!path.is_symlink()` 检查；对扫描根下子目录及命中文件均拒绝跟随符号链接。
- 来源：独立发现

### AQ-4（P2）E2E fixture 缺失时未覆盖 `SLTERM_CLAUDE_PROJECTS_DIR`，后端可能回落真实 home
- 位置：`e2e-tests/run-wdio.cjs:148-152`
- 问题：当 `fixtures/claude-projects` 不存在时，launcher 跳过设置 `SLTERM_CLAUDE_PROJECTS_DIR`，后端 `resolve_projects_root()` 会回落到 `~/.claude/projects`。同时 `~/.claude/settings.json` 与 `~/.slterminal/hooks/` 虽有备份还原，但扫描根无 env 覆盖。
- 攻击/失误场景：开发/CI 环境中若误删 fixture 目录或首次运行未生成，E2E 历史会话用例将直接读取、展示并可能执行删除操作（E2E 中若包含 delete 用例）用户真实历史会话数据，造成数据污染或丢失；`console.warn` 仅在日志中提示，不会阻止测试继续。
- 修复建议：fixture 缺失时直接 `throw` 或 `process.exit(1)` 终止 E2E；或在 launcher 中设置一个指向临时副本的扫描根，使异常路径也保持隔离。
- 来源：独立发现

## 已检查范围

1. SEC-05 等价强制：`validate_session_id` 为 delete 强制前置；UUID 形态校验拒绝路径分隔符与 `..`；定位限定扫描根一级子目录内精确匹配。无绕过。
2. hooks 注入面：非法 JSON 中止且不改原文件；用户 matcher/handler 保留；脚本与 settings 均使用 `NamedTempFile+persist` 原子写；reporter 路径由程序内构造。
3. 信号文件管道：`parse_signal_file` 对非法/空/缺 panelId 均降级为 None 不 panic；读→emit→删流程存在但已记录大小与符号链接风险。
4. 前端注入面：`buildRestoreInput` 仅使用 UUID 形态 sessionId，安全；`buildResumeCommand` 存在 cwd 单引号注入（AQ-1）。
5. E2E 隔离：L1 经 `HomeDirGuard`/`ScanRootGuard`、L2 经 mock、E2E 经 env+备份覆盖；fixture 缺失路径有风险（AQ-4）。
6. 既有安全机制回归：`agent_hooks_config_read/write` project/local 层正确接入 `validate_path_within_root`；`pty_write/resize/kill` 均保留 `validate_session_ownership`；`capabilities/default.json` 无新增通配权限。
