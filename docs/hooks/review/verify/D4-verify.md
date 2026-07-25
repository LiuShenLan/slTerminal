# D4 Review 验证报告

> 验证日期: 2026-07-25
> 验证范围: 1 个 review 文件, 25 个声称错误

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 23 |
| Review 不正确，未修改 | 1 |
| Review 部分正确，部分修正 | 1 |

## 验证方法

- 主要证据来源: Context7 查询 Claude Code 官方文档 (SKILL.md, code.claude.com/docs/en/hooks, code.claude.com/docs/en/plugins-reference)
- 内部交叉验证: D1-01-hooks-official-docs.md (同仓库)
- ADJUDICATION.md 裁决 (跨方向冲突)
- WebFetch 因企业网络策略不可用于 github.com / warp.dev / wezterm.org

> **注意**: 13 个错误因 Context7 配额耗尽 + WebFetch 受限，仅通过 ADJUDICATION 裁决 + 内部文档交叉验证，未独立外部验证。

## 逐条详情

### R4.1: CLAUDE_SESSION_ID 不是 hook 环境变量 (P0-1)
- **Review 声称**: CLAUDE_SESSION_ID 不在官方 env var 列表中，仅存在于 stdin JSON
- **验证结果**: 正确
- **验证证据**: Context7 SKILL.md 列出 hook 可用环境变量为 CLAUDE_PROJECT_DIR, CLAUDE_PLUGIN_ROOT, CLAUDE_ENV_FILE, CLAUDE_CODE_REMOTE——不含 CLAUDE_SESSION_ID。D1-01 §7.3 (line 537-544) 同样不含此变量。
- **行动**: 已删除 CLAUDE_SESSION_ID 行，添加说明：session_id 仅通过 stdin JSON 获取

### R4.2: 遗漏 CLAUDE_PLUGIN_ROOT 和 CLAUDE_PLUGIN_DATA
- **Review 声称**: 环境变量表缺失 CLAUDE_PLUGIN_ROOT 和 CLAUDE_PLUGIN_DATA
- **验证结果**: 正确
- **验证证据**: Context7 SKILL.md 明确列出 "`$CLAUDE_PLUGIN_ROOT` - Plugin directory (use for portable paths)"。D1-01 §7.3 列出 CLAUDE_PLUGIN_DATA。code.claude.com/docs/en/plugins-reference 列出三个路径变量。
- **行动**: 已补充两个变量到环境变量表

### R4.3: CLAUDE_ENV_FILE 可用范围未声明
- **Review 声称**: 文档未标注 CLAUDE_ENV_FILE 仅在特定事件中可用
- **验证结果**: 正确
- **验证证据**: Context7 官方 docs: "SessionStart, Setup, CwdChanged, and FileChanged hooks can use the CLAUDE_ENV_FILE environment variable"
- **行动**: 环境变量表新增"可用范围"列，CLAUDE_ENV_FILE 标注为 "SessionStart / Setup / CwdChanged / FileChanged"

### R4.4: CLAUDE_CODE_CHILD_SESSION 非官方变量
- **Review 声称**: 此变量来自社区 bug report，不应与官方变量并列
- **验证结果**: 正确
- **验证证据**: 不在 Context7 官方 env var 列表中。D1-01 §7.3 也不含此变量。D4 自身第 460 行引用 #72347 为社区 issue。
- **行动**: 从表格移除，放入注意事项段落，标注"来自社区 bug report，非官方文档变量"

### R4.5: SessionEnd reason 枚举缺漏
- **Review 声称**: 遗漏 "resume" 和 "bypass_permissions_disabled" 两个值
- **验证结果**: 部分正确
- **验证证据**: D1-01 line 23 列出的 5 个值中含 "resume" 但不含 "bypass_permissions_disabled"。ADJUDICATION 确认官方规范为 6 个值。
- **行动**: 已补充 "resume" 和 "bypass_permissions_disabled"

### R4.6: cwd 恒等于 CLAUDE_PROJECT_DIR 的说法错误
- **Review 声称**: cwd 和 CLAUDE_PROJECT_DIR 是独立字段，不一定相等
- **验证结果**: 正确
- **验证证据**: D1-01 line 103 定义 cwd 为"hook 触发时的当前工作目录"，line 540 定义 CLAUDE_PROJECT_DIR 为"项目根目录绝对路径"——两个独立概念
- **行动**: 修正为"不一定等于 CLAUDE_PROJECT_DIR"

### R4.7: permission_mode 示例值不正确
- **Review 声称**: 字段值应为 "ask" 或 "allow"，无 "default"
- **验证结果**: **不正确**
- **验证证据**: D1-01 line 104 列出 permission_mode 值为 `"default"` / `"plan"` / `"acceptEdits"` / `"bypassPermissions"`。"default" 是会话级权限模式的合法值。"ask"/"allow" 可能是个别权限决策的取值，非 session 级 permission_mode 字段。
- **行动**: 未修改

### R4.8: 未说明 PreCompact hook 可阻止压缩
- **Review 声称**: PreCompact 可以主动阻止压缩 (exit code 2)
- **验证结果**: 正确
- **验证证据**: D1-01 line 64: PreCompact 可阻塞列 = "是 (v2.1.105+)"
- **行动**: 已更新 PreCompact 角色描述，添加"可主动阻止压缩"说明

### R4.9: 未说明 CLAUDE_ENV_FILE 不传播到后续 hook 子进程
- **Review 声称**: CLAUDE_ENV_FILE 仅传播到 Bash Tool 子进程，不传播到其他 hook 子进程
- **验证结果**: 正确 (间接验证)
- **验证证据**: Context7 文档"persist environment variables for subsequent Bash commands"——明确范围为 Bash 命令。ADJUDICATION 未将此标注为冲突。
- **行动**: 已添加注意事项：仅传播到 Bash Tool 子进程，不传播到后续 hook 子进程

### R4.10: "追加式"表述不精确
- **Review 声称**: 应描述为"使用追加写入 (>>)"而非文件本身有追加式特性
- **验证结果**: 正确
- **验证证据**: Context7 官方示例使用 `>> "$CLAUDE_ENV_FILE"`——这是 shell 的普通追加重定向
- **行动**: 已修改措辞为"应使用追加写入"

### R4.11: Issue #2509 引用不当——Conda 非 venv
- **Review 声称**: #2509 讨论 Conda，在 Python venv 章节引用有误导
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证（WebFetch 不可达）。Review author 验证确认 #2509 标题为 "Conda environment workflow guidance needed"
- **行动**: 已将 #2509 从 venv 来源中分离，作为补充注释

### R4.12: claude-mem hooks 架构 URL 路径错误
- **Review 声称**: URL 应为 docs/hooks-architecture.mdx，非 docs/architecture/hooks.mdx
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证（WebFetch 不可达）。Review author 确认正确路径
- **行动**: 已修正 URL (references 表)

### R4.13: /dev/tty 并非"静默"失败
- **Review 声称**: bash 向 stderr 输出错误消息，被 Claude Code 解释为 hook 错误
- **验证结果**: 正确 (间接验证)
- **验证证据**: 技术逻辑合理——bash redirection 到不存在设备会输出错误到 stderr
- **行动**: 已修改为更精确的描述

### R4.14: commit 4f3092d 引用方式不精确
- **Review 声称**: commit 本身是代码变更，应引用项目 README
- **验证结果**: 正确
- **验证证据**: 内部逻辑——commit hash 指向代码变更，引用的信息实际在项目描述中
- **行动**: 已改为引用项目主页

### R4.15: WezTerm Shell Integration 注入机制虚构 (P0-2)
- **Review 声称**: ZDOTDIR/BASH_ENV/XDG_CONFIG_HOME 三个环境变量在 WezTerm 官方文档中零出现
- **验证结果**: 正确
- **验证证据**: ADJUDICATION P0-2 裁决。Review author 全文检索 wezterm.org/shell-integration.html 确认三个变量均零出现
- **行动**: 已删除虚构的注入机制描述，改为实际机制（用户手动 source wezterm.sh）

### R4.16: Windows Terminal WSL 示例无来源支撑
- **Review 声称**: SuperUser 页面不涉及 WSL
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证（WebFetch 不可达）
- **行动**: 已添加注释说明 WSL 示例不来自引用的 SuperUser 页面

### R4.17: iTerm2 评分机制描述不准确
- **Review 声称**: 16 分仅 hostname 单项分，非总上限；实际最高 23 分
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证
- **行动**: 已修正为 "hostname 16 + job 4 + user 2 + path 1 = 最高 23 分"

### R4.18: iTerm2 匹配机制描述错误——通配符非正则
- **Review 声称**: APS 主匹配是通配符 (*) 匹配，非正则表达式
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证
- **行动**: 已改为"通配符匹配"

### R4.19: iTerm2 Smart Selection 精度级别遗漏 "Very Low"
- **Review 声称**: 实际五级而非四级，遗漏 Very Low
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证
- **行动**: 已改为五级精度 (very_low/low/normal/high/very_high)

### R4.20: Warp 虚构术语 "SourcedRcFileForWarp" (P0-3)
- **Review 声称**: 该术语在引用来源中完全不存在
- **验证结果**: 正确
- **验证证据**: ADJUDICATION P0-3 裁决。Review author 全文检索 Warp blog 确认零出现
- **行动**: 已删除

### R4.21: Warp "OSC 777 事件" 无来源支撑 (P0-3)
- **Review 声称**: 三个引用来源均不提及 OSC 777
- **验证结果**: 正确
- **验证证据**: ADJUDICATION P0-3 裁决
- **行动**: 已删除

### R4.22: Warp 产品名称 + 对比表虚构成分 (P0-3)
- **Review 声称**: (a) 产品名为 "Agents 3.0" 非 "Agent Mode"; (b) MCP 非当前功能
- **验证结果**: 正确
- **验证证据**: ADJUDICATION P0-3 裁决
- **行动**: 对比表 Warp 列修正：生命周期 hook 标 "—"，程序化控制标 "—"，AI 保持 "Agents 3.0"

### R4.23: #14433 bug 机制描述错误
- **Review 声称**: 问题在于 sourcing 机制缺失，非指向旧文件
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证（WebFetch 不可达）。Review author 直接 WebFetch #14433 确认 body 原文
- **行动**: 已修正 bug 描述为"sourcing 机制缺失"

### R4.24: #38299 分类错误——功能请求非 bug
- **Review 声称**: #38299 是 Feature Request，非 Bug Report
- **验证结果**: 条件正确 (未独立外部验证)
- **验证证据**: 无独立验证。Review author 确认 issue 标签为 "Feature Request"
- **行动**: 已标注为"功能请求"，更新状态为"已关闭 2026-03-25"

### R4.25: 多数 Windows bug 已关闭但未注明 (P0-5)
- **Review 声称**: 7 个 Windows 问题中 6 个已关闭，仅 #69159 仍 OPEN
- **验证结果**: 正确
- **验证证据**: ADJUDICATION P0-5 裁决，列出逐条关闭状态和日期
- **行动**: 每个 issue 已标注状态和关闭日期

## 未独立验证项 (13 项)

以下错误因 Context7 配额耗尽 + WebFetch/WebSearch 不可用，仅依据 ADJUDICATION 裁决 + 内部文档交叉验证。保持 low-confidence 标记：

| 错误 | 说明 | 依赖 |
|------|------|------|
| R4.9 | CLAUDE_ENV_FILE 传播限制 | Context7 间接证据 + 技术逻辑 |
| R4.11 | #2509 Conda 确认 | Review author Context7 验证 |
| R4.12 | claude-mem URL | Review author WebFetch 验证 |
| R4.16 | WSL 来源 | Review author WebFetch 验证 |
| R4.17-19 | iTerm2 三项细节 | Review author 文档验证 |
| R4.23 | #14433 机制 | Review author WebFetch 验证 |
| R4.24 | #38299 分类 | Review author WebFetch 验证 |
| R4.5 (部分) | bypass_permissions_disabled | ADJUDICATION 裁决 (D1 无此值) |

## ADJUDICATION 专项处理

以下 ADJUDICATION.md 中明确指示的补充项已全部处理：

| 项目 | 状态 |
|------|------|
| CLAUDE_PLUGIN_ROOT 和 CLAUDE_PLUGIN_DATA 遗漏 → 补充 | 已补充 |
| CLAUDE_ENV_FILE 仅在 SessionStart 可用 → 标注限制 | 已标注 (含 Setup/CwdChanged/FileChanged) |
| CLAUDE_CODE_CHILD_SESSION 非官方变量 → 移到社区发现段落 | 已移到注意事项 |
| SessionEnd reason 枚举缺漏 "resume" 和 "bypass_permissions_disabled" | 已补充 |
