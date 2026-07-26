# 阶段 1：状态可视化核心

> 最小闭环：注入 hooks → 状态上报 → 页签有状态。F1/F2/F3 三项不可分割，同属一期。
> 标注约定：**【已确认】** = 用户问答拍板；**【推导默认】** = 已确认决策的直接推论，开发前可复核。

## F1 信号文件通道（基础设施）

### 功能目标

让 slTerminal 实时得知每个 claude 会话发生的 hook 事件，并精确路由到对应终端页签。这是 F3/F4/F5 全部功能的数据基础。

### 调研依据

- 官方无 hook 运行时遥测（Issue #50287，D3 §5.3）——通道必须自建
- 第三方工具标准架构 `hooks → 信号文件(JSON) → 终端适配器`（D1 §3.2），tabby-claude-status 在 Windows 上已验证
- hook 子进程无 TTY（D4 §1.5），无法直接向终端写 OSC 序列；环境变量传递链 `终端→shell→claude→hook` 完整（D4 §6.1）

### 功能详述

**事件覆盖清单**（注入的 hook 监听以下 10 个事件）【推导默认——四态映射 + 启动/退出检测所需的最小集】：

| 分类 | 事件 |
|------|------|
| 会话生命周期 | SessionStart、SessionEnd |
| 用户交互 | UserPromptSubmit、Stop、StopFailure |
| 工具调用 | PreToolUse、PostToolUse、PostToolUseFailure |
| 注意信号 | Notification、PermissionRequest |

其余 20+ 事件（SubagentStart/Stop、PreCompact、ConfigChange 等）不注入【推导默认：与四态无关，避免每次工具调用多一份无谓的进程开销；未来扩展时追加】。

**信号文件内容**：每条事件一个 JSON 记录，字段集【推导默认】：

| 字段 | 来源 | 用途 |
|------|------|------|
| `panelId` | 环境变量 `SLTERM_PANEL_ID` | 页签路由（**必须**，缺失时该条事件丢弃并记录日志） |
| `event` | stdin JSON `hook_event_name` | 四态映射 |
| `timestamp` | 脚本生成 | 排序/去重 |
| `sessionId` | stdin JSON `session_id` | 会话标识 |
| `transcriptPath` | stdin JSON `transcript_path` | 阶段 2 定位 JSONL 解析上下文用量 |
| `cwd` | stdin JSON `cwd` | 辅助信息（项目归属校验） |
| `toolName` | stdin JSON `tool_name`（仅工具事件） | 预留 |
| `notificationType` | stdin JSON `notification_type`（仅 Notification） | 区分权限请求/空闲/其他 |

**信号文件位置**【推导默认】：`~/.slterminal/hooks-events/`（与 slTerminal 自身 settings 同目录体系，不进 `%TEMP%` 避免被系统清理）。

**pty_spawn 注入**：新增第 4 个环境变量 `SLTERM_PANEL_ID=<panelId>`，与现有 COLORTERM/TERM/TERM_PROGRAM 同一时机（spawn 阶段环境块）【已确认 Q3】。

**后端监听**：复用 notify 模块能力监听信号目录，解析后经 IPC（新 `ipc/hooks` 领域函数）推送前端【实现方式，实现阶段定】。

### 边界条件

1. 路由只用 `SLTERM_PANEL_ID`，不用 cwd 猜测【已确认 Q3】；信号中缺失 panelId 时丢弃该事件并写日志
2. hook 脚本**任何情况下 exit code 恒为 0**（exit 2 会阻断 claude 操作、stderr 会污染用户界面）——信号目录不存在/不可写/JSON 解析失败均静默退出【ADR-0001 后果】
3. 用户在其他终端（非 slTerminal）启动 claude 时脚本同样运行——无 `SLTERM_PANEL_ID` 环境变量时脚本直接静默退出，不产生信号文件
4. 未注入 hooks 的 claude 会话不产生信号文件，前端无状态显示——**不是错误**，走 OSC 133 C 降级路径（见 F3）
5. 注入的每个 hook 配置 `timeout: 5`（秒）【推导默认：脚本只写一个小文件，正常 <100ms；默认 600s 超时意味着脚本卡死时 claude 长时间挂起】
6. 信号文件并发：同页签高频事件（PreToolUse/PostToolUse 每秒多次）时写入不得相互损坏【推导默认：单事件单文件写入后由后端合并，或追加写 + 行级 JSONL，实现阶段定】

### 明确不做

- 不解析 PTY 输出推断状态（D1 §5.3 明确否定的路径）
- 不建 HTTP localhost 服务
- 不覆盖 10 个事件之外的 hook 事件
- 不在信号通道上传输 hook 的 stdout/stderr 内容（那是 A3 日志面板的范畴，已排除）

### 验收要点（可观察行为）

1. 注入后启动 claude 页签 → 提交任意 prompt → 信号目录出现 JSON 记录，含正确 `panelId`、`event=UserPromptSubmit`
2. 同项目开两个 claude 页签交替操作 → 各自事件的 panelId 互不串扰
3. 在非 slTerminal 终端（如 Windows Terminal）启动 claude → 不产生信号文件、claude 行为无任何异常
4. 删除信号目录后操作 claude → claude 无任何报错/阻断

---

## F2 状态上报 hooks 一键注入/卸载

### 功能目标

用户一键完成 F1 所需的全部配置（hook 脚本落盘 + settings.json 配置写入），且可无损回退。

### 功能详述

**注入**（手动按钮触发，无启动引导）【已确认 Q11】：

1. 把 hook 脚本写入 `~/.slterminal/hooks/`【推导默认：不放 exe 同级——slTerminal 是绿色软件可移动，exe 路径写进用户全局配置会因移动而悬空】
2. 读取 `~/.claude/settings.json` → 以 merge 方式追加 10 个事件的 matcher 组（command 指向上述脚本）→ 原子写回
3. **幂等**：已存在 slTerminal 配置段时，更新为当前版本（升级场景），不产生重复段【推导默认；CC 对完全相同 handler 自动去重，但版本升级时旧路径必须替换】

**卸载**：

1. 从 settings.json 移除 slTerminal 全部事件段（精确识别自身配置，不动用户其他任何配置）
2. 删除 `~/.slterminal/hooks/` 脚本目录
3. 清空信号目录
4. 前端四态回退到 OSC 133 C 降级行为

### 边界条件

1. 注入位置固定 user 层 `~/.claude/settings.json`【已确认 Q11】——状态上报与项目无关，所有会话生效
2. **merge 不覆盖**用户既有 hooks 配置【已确认 Q11】：用户已有的任何事件段原样保留，slTerminal 段与之并存
3. settings.json 本身非法（用户手写错误）时，注入操作**中止并提示用户先修复**，不强行改写【推导默认——与 Q10 保存安全原则一致】
4. 卸载后用户配置恢复到注入前状态（除了用户自己在注入期间手动改的部分）

### 明确不做

- 不做 claude 页签启动时的弹窗引导【已确认 Q11】
- 不做项目级/本地级注入（仅 user 层）
- 不修改 `disableAllHooks` 等用户其他设置项

### 验收要点

1. 空 settings.json 注入 → 文件合法、含 10 事件段；再次注入 → 无重复段
2. 已有用户自定义 hooks 的 settings.json 注入 → 用户段原样保留
3. 卸载 → settings.json 无 slTerminal 残留、脚本目录删除、页面状态回退降级行为
4. settings.json 非法时注入 → 中止 + 明确提示，文件未被改动

---

## F3 页签四态指示

### 功能目标

Dockview 页签 emoji 图标实时反映该页签内 claude 会话的状态，多页签时一眼定位"哪个在干活、哪个在等我、哪个跑完了、哪个出错了"。

### 调研依据

- 社区状态映射共识（D1 §3.2）：PreToolUse 等→工作态、Notification/PermissionRequest→注意态、Stop→完成态、失败→错误态
- 多 tab 管理是重度用户核心痛点（D1 §4.3）

### 状态机（完整定义）

| 触发 | 图标 | 语义 |
|------|------|------|
| 非 claude 页签 / claude 已退出 | （无图标） | 默认 |
| OSC 133 C 检测到 claude 命令启动 | 🟡 | 会话启动，等待用户输入 |
| 信号：SessionStart | 🟡 | 会话就绪，等待用户输入 |
| 信号：UserPromptSubmit | ⚡ | claude 处理中 |
| 信号：PreToolUse / PostToolUse | ⚡ | 工具执行中 |
| 信号：Notification（permission_prompt / idle_prompt / agent_needs_input） | 🟡 | 需要用户处理 |
| 信号：PermissionRequest | 🟡 | 等待授权 |
| 信号：Stop | ✅ | 本轮完成 |
| 信号：PostToolUseFailure / StopFailure | ❌ | 出错 |
| 信号：SessionEnd / OSC 133 D 命令退出 | （无图标） | 会话结束，恢复默认 |
| 信号：Notification（auth_success 等其他类型） | 不改变当前状态 | 【推导默认：与"需要用户处理"无关】 |

### 边界条件

1. 仅 emoji 图标，不动颜色系统（硬约束 #6）【已确认 Q4】
2. 生命周期 = **事件驱动覆盖**：状态为最后一个事件，无定时器、无聚焦清除【已确认 Q5】
3. **降级路径**：未注入 hooks 的 claude 页签，OSC 133 C 启动 → 🟡 并保持，直到 OSC 133 D 退出恢复默认【已确认 Q5b】
4. **删除旧功能**：TabTitleRegistry claude 规则的自定义**图标**切换删除（tabRules.ts 对应注册移除）；**标题切换保留**（claude 运行时页签标题仍变为规则标题）【已确认 Q5b】
5. OSC 133 C/D 命令边界检测机制（useCommandDetection）本身保留——它现在是四态的启动/退出触发器【已确认 Q5b】
6. 状态更新与页面显隐无关：后台页面/非聚焦页签的图标同样实时更新【推导默认——这正是多 tab 管理的价值】
7. 同一页签内 claude 退出后再次启动：图标走"默认 → 🟡"新周期【推导默认】

### 明确不做

- 不做页签颜色条/背景色【已确认 Q4】
- 不做状态定时消失、聚焦清除【已确认 Q5】
- 不做除四态外的更多状态（如"压缩中""子代理运行中"）【推导默认——保持四态语义简单】

### 验收要点

1. 全状态转换走查：启动(🟡) → 提交 prompt(⚡) → 权限请求(🟡) → 放行(⚡) → 完成(✅) → 新 prompt(⚡) → 退出(无图标)
2. 错误路径：工具失败 → ❌；下一个事件覆盖
3. 未注入会话：启动🟡 → 退出默认，全程无信号文件参与
4. 旧自定义图标不再出现；claude 运行时标题仍切换
5. 两个页签各自状态独立更新，切页面不影响

---

## 阶段级风险与注意事项（来自 D4 §7，开发前必读）

| 风险 | 出处 | 对策 |
|------|------|------|
| Windows 上 UserPromptSubmit hook stdin 为空 | Issue #48009（已关闭） | 社区方案：配置 `CLAUDE_CODE_GIT_BASH_PATH` 走 Git Bash 执行 hook。注入功能检测该环境并在缺失时提示用户【推导默认】 |
| SessionStart hook 导致终端输入冻结 | Issue #23554（已修复） | 官方已修复；阶段 1 验收时在真实 claude 最新版回归确认 |
| hook 脚本执行耗时阻塞 claude | CC 默认 600s 超时 | 注入配置 `timeout: 5`（F1 边界 5）；脚本只写小文件不做任何 I/O 等待 |
| 注入的 hook 与其他终端共存 | ADR-0001 后果 | 无 SLTERM_PANEL_ID 时脚本静默退出（F1 边界 3） |

## 阶段 1 验收（端到端）

1. F1/F2/F3 各自验收要点全部通过
2. 真实 claude（最新稳定版）完整走查状态机全表
3. L1/L2 测试按项目测试策略补齐；PTY 相关 L1 必须 `--test-threads=1`
