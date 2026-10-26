# AI CLI hook 集成

> hook 事件接收、安装、审查与状态投影的宿主侧；hook 小进程桥归 `slterm_hook`。

## 决策与红线

- **命名管道 = hook 事件唯一权威通道**：`slterm-hook` 把原始载荷经命名管道转发给宿主；旧信号文件事件通道退役。原因：轮询延迟与丢失窗口不可接受。边界：宿主解析 JSON，helper 不重编码。
- **statusline 用量信号分通道保留**：信号文件介质保留（app 本地数据根），与 hook 管道分通道；桥接脚本节流后写 ContextUsage，并透传用户原 statusline。
- **流键收窄**：`AiHookStreamKey` = source + session + pane + agent_pid + bridge_instance；`remote_process` 维度随 SSH/远程砍除。
- **`AgentSession.source` 宽松落盘、严格消费**：持久化保留 hook 原报字符串；恢复注入时经 `AgentKind::parse` 归一，不归一即不可恢复。原因：未知 CLI 的事件不应被静默丢弃，但恢复必须只作用于已知 agent。
- **`AgentKind` 封闭集**：内置 agent 身份是封闭枚举，屏幕规则随仓；新增 agent = 枚举 + 规则 + 必要的安装器/目录表登记，不裁剪既有集。
- **安装权威**：claude/kimi/codex/pi/opencode 按各自配置形态安装；codex 支持 `--chain` 串联原 notifier；managed 文件带归属指纹，`config_guard` 监视防外部改写。
- **审查门**：statusline 键在安装器审查门落地；审查词表演化登记到安全审计域。审查门只覆盖 statusline 键，hooks 数组的 command 字段由 marker/归属指纹兜底。
- **自愈授权与失败提示**：授权只经 `AgentHook`/SettingsRegistry；失败提示只含 `Install`/`Settings` 类别与 agent，进程内冷却 5 秒，空失败不占冷却，不携带命令或配置正文。
- **补全方言**：保留 Posix 方言（Git Bash/MSYS 场景）；SSH/WSL 语义源、ANSI/LS_COLORS 样式分支与 Unix 小分支整面不迁。
- **WSL/SSH/远程整面不迁**：`WSLENV` 合并、远端进程树、remote_* 错误码与移动端桥接均无对象；只保留本地 PTY 与 Hooks 命名管道。
- **隔离面**：hook 通道与 statusline 通道各自有界；安全审查词表与审计事件是后续演化单点，新增命令形态不得绕过。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：管道事件解析、statusline 分通道、流键生成、source 归一、安装器各配置形态、managed 文件指纹、config_guard 失败边与冷却、审查门词表。
- 真实 hook 安装/卸载与外部 CLI 交互由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增 agent：扩 `AgentKind` + 屏幕规则 + 安装器/目录表登记，并补安装与消费用例。
- 新增 hook 形态：先确定配置形态与归属指纹，再落安装器；不得绕过审查门。
- 修改 managed 文件：保持归属指纹可重算，避免与用户手改冲突。
