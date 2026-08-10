# slTerminal 针对 claude 的定制优化 — 汇总盘点

> 只读现状盘点（2026-08-08）。数据源：5 个领域分文件（见下表链接），本文件为汇总。
>
> **专属程度三档口径**：
> - **硬编码 claude**：代码/配置/文档中显式出现 claude 字样或假设其行为（hook 事件名、`~/.claude/` 路径、transcript 格式、`--resume` 参数、CLI 常量等）
> - **通用机制但 claude 触发**：机制本身通用（注册表、合帧管道、安全校验、编排框架），当前仅被 claude 的行为激活或为 claude 的场景设计
> - **完全通用**：与 claude 无关的底层能力（在 claude 优化链路的上下文中顺带标注）
>
> **口径说明（激活者 vs 受益者）**：「通用机制但 claude 触发」档内混合两类——**激活者**（机制被 claude 行为激活，claude 解耦后失效或需替换激活源：01-14 合帧、01-19 Kitty、01-20 OSC 133 规则、02 全档、04-16、05-7、05-15 通知链路等）与**受益者**（机制独立成立、claude 仅为当前主要受益场景，解耦后保留：01-26 键盘委托、05-6/9/12/17 等）。抽象设计引用分层结论时须区分「随 claude 解耦而失效」与「保留」两类，勿把受益者误判为随 claude 解耦而失效。

## 一、统计总览

| 分文件 | 领域 | 优化项数 | 硬编码 claude | 通用机制但 claude 触发 | 完全通用 |
|--------|------|---------|--------------|----------------------|---------|
| [01-pty-terminal.md](./01-pty-terminal.md) | PTY 与终端渲染 | 29 | 8 | 7 | 14 |
| [02-hooks.md](./02-hooks.md) | hooks 信号链路与注入 | 29 | 15 | 14 | 0 |
| [03-claude-history.md](./03-claude-history.md) | 历史会话查询与恢复 | 24 | 16 | 7 | 1 |
| [04-ui-status.md](./04-ui-status.md) | UI 状态指示（含品牌/展示） | 19 | 12 | 6 | 1 |
| [05-keyboard-window.md](./05-keyboard-window.md) | 键盘/快捷键/窗口 | 17 | 3 | 10 | 4 |
| **合计** | | **118** | **54** | **44** | **20** |

## 二、全量优化项

### 2.1 PTY 与终端渲染（01，29 项）

| # | 优化 | 位置 | 专属程度 |
|---|------|------|----------|
| 01-1 | DA1 查询模拟响应（注入 `ESC[?64;22c`，Ink 同步哨兵） | pty/reader.rs:104-120 | 硬编码 claude |
| 01-2 | 终端能力 env 注入（COLORTERM/TERM/TERM_PROGRAM，Chalk 检测） | pty/spawn.rs:931-936 | 硬编码 claude |
| 01-3 | SLTERM_PANEL_ID env 注入（hooks 信号路由键） | pty/spawn.rs:935 | 硬编码 claude |
| 01-4 | ConPTY flags 固定 0x7 守卫（PASSTHROUGH_MODE 吞 claude 全屏 TUI 滚轮） | pty/spawn.rs:58-73 | 硬编码 claude |
| 01-5 | ConPTY 启动序列剥离（strip_conpty_startup） | pty/reader.rs:179-250 | 完全通用 |
| 01-6 | CPR 注入（`\x1b[1;1R`） | pty/spawn.rs:1018-1027 | 完全通用 |
| 01-7 | cwd 反斜杠规范化 | pty/spawn.rs:128-130 | 完全通用 |
| 01-8 | reader 缓冲区 16KB | pty/reader.rs:20 | 完全通用 |
| 01-9 | E1 Channel 可替换 + ring buffer 回放（无条件缓存，无前端消费路径） | state.rs:201-218 | 完全通用 |
| 01-10 | shell-integration.ps1 OSC 7 + OSC 133 注入（Enter hook 携命令行） | assets/shell-integration.ps1:54-62 | 通用机制但 claude 触发 |
| 01-11 | -EncodedCommand 内联脚本注入（AMSI/ASR 规避） | pty/shell.rs:133-156 | 完全通用 |
| 01-12 | SPAWN_LOCK 串行化 | pty/spawn.rs:947-951 | 完全通用 |
| 01-13 | Job Object 孤儿防护 | pty/spawn.rs:1333-1425 | 完全通用 |
| 01-14 | Ink 输出合帧管道（64B 阈值分流 + Idle 2ms/Max 16ms + DEC 2026） | usePtyOutput.ts | 通用机制但 claude 触发 |
| 01-15 | 非焦点终端降频 + WebGL 按可见性释放 | usePtyOutput.ts:201-236 | 完全通用 |
| 01-16 | Resize X/Y 分离 debounce + NaN 守卫（Ink TUI 错位调查 #3） | usePtyResize.ts:63-111 | 通用机制但 claude 触发 |
| 01-17 | 交替缓冲 resize 必须 fit() 同步网格 | usePtyResize.ts:86-107 | 通用机制但 claude 触发 |
| 01-18 | OSC 52 剪贴板拦截（claude /copy 命令） | useClipboardHandler.ts:37-66 | 硬编码 claude |
| 01-19 | Kitty 键盘协议（CSI u）被动启用 | theme.ts:22 | 通用机制但 claude 触发 |
| 01-20 | OSC 133 命令边界检测 + 页签标题/图标/logo 联动 | useCommandDetection.ts:42-72 | 通用机制但 claude 触发 |
| 01-21 | F3 页签四态 emoji（hook-event 消费侧） | useXterm.ts:349-373 | 硬编码 claude |
| 01-22 | terminal.newline（Ctrl+Enter 写 `\n`，Ink 换行不提交） | keyboard.ts:40-46 | 硬编码 claude |
| 01-23 | Ctrl+C 保留为中断（不注册命令） | keyboard.ts:7,47-48 | 硬编码 claude |
| 01-24 | WebGL 渲染 + DOM 兜底 | webgl.ts:28-138 | 完全通用 |
| 01-25 | PTY spawn 等待布局就绪（rAF 轮询） | useXterm.ts:262-334 | 完全通用 |
| 01-26 | attachCustomKeyEventHandler 委托式 fallback | useXterm.ts:229-237 | 通用机制但 claude 触发 |
| 01-27 | OSC 8 超链接 | useXterm.ts:240-244 | 完全通用 |
| 01-28 | ConPTY buildNumber 设置（reflow 阈值） | useXterm.ts:221-226 | 完全通用 |
| 01-29 | pty_kill async + spawn_blocking（ClosePseudoConsole 永久阻塞防护） | pty/spawn.rs:1211-1255 | 完全通用 |

### 2.2 hooks 信号链路与注入（02，29 项）

| # | 优化 | 位置 | 专属程度 |
|---|------|------|----------|
| 02-1 | 10 事件 hooks 注入（C9） | hooks/inject.rs:16-27 | 硬编码 claude |
| 02-2 | ~/.claude/settings.json merge 注入 | hooks/inject.rs:39-41,190-263 | 硬编码 claude |
| 02-3 | matcher 条目结构（matcher:"" + node 命令 + timeout:5） | hooks/inject.rs:151-161 | 硬编码 claude |
| 02-4 | slterm matcher 识别 + handler 级剔除卸载 | hooks/inject.rs:98-148 | 通用机制但 claude 触发 |
| 02-5 | 注入状态三态 + SCRIPT_VERSION 版本检测 | hooks/inject.rs:50-78,341-394 | 通用机制但 claude 触发 |
| 02-6 | reporter 脚本（claude hook stdin JSON 解析） | assets/slterm-hook-reporter.js:44-53 | 硬编码 claude |
| 02-7 | SLTERM_PANEL_ID 环境变量路由 | reporter.js:27-32；spawn.rs:935,999 | 硬编码 claude |
| 02-8 | C10 契约（任何路径 exit 0） | reporter.js:1-81 | 硬编码 claude |
| 02-9 | 单事件单文件 + 原子 rename | reporter.js:62-65 | 通用机制但 claude 触发 |
| 02-10 | HookEventPayload 8 字段 DTO | hooks/signal.rs:19-38 | 硬编码 claude |
| 02-11 | 信号文件处理：读→emit→删 | hooks/signal.rs:55-93 | 通用机制但 claude 触发 |
| 02-12 | notify 50ms + 3s 轮询双通道 watcher（win10 实证） | hooks/watcher.rs:46-129,199-211 | 通用机制但 claude 触发 |
| 02-13 | WATCHER 全局静态实例 + 幂等启动 | hooks/mod.rs:64-101；lib.rs:71-72 | 通用机制但 claude 触发 |
| 02-14 | hook-event 广播 + 前端 onHookEvent 订阅 | signal.rs:56；ipc/hooks.ts:56-65 | 通用机制但 claude 触发 |
| 02-15 | 前端四态消费链（F3） | useXterm.ts:349-373；claudeStatus.ts:41-75 | 硬编码 claude |
| 02-16 | transcript 尾部 64KB 逆行扫描用量 | hooks/usage.rs:34-89 | 硬编码 claude |
| 02-17 | ContextUsage DTO + cache 字段（Anthropic 用量结构） | hooks/usage.rs:13-26 | 硬编码 claude |
| 02-18 | hooks 配置三层路径（~/.claude/settings*.json） | hooks/config.rs:22-51 | 硬编码 claude |
| 02-19 | 子树 read-modify-write merge | hooks/config.rs:111-152 | 通用机制但 claude 触发 |
| 02-20 | SchemaStore 官方 schema 内嵌 + hooks 子 schema 提取 | schema/index.ts:19-37 | 硬编码 claude |
| 02-21 | Draft07 双校验 validateHooksJson | schema/index.ts:61-79 | 硬编码 claude |
| 02-22 | 注入段保护 isSltermManaged | configModel.ts:195-199 | 通用机制但 claude 触发 |
| 02-23 | F2 注入入口并入面板 | HooksConfigPanel.tsx:192-210,224-252,331-369 | 通用机制但 claude 触发 |
| 02-24 | 注入后自动重读 user 层（C13-8） | HooksConfigPanel.tsx:215-221 | 通用机制但 claude 触发 |
| 02-25 | 保存提示「需重启 claude 会话生效」 | HooksConfigPanel.tsx:371-375 | 硬编码 claude |
| 02-26 | claude hooks 协议知识内嵌（30 事件目录/matcher 语义/handler 字段矩阵） | eventsCatalog.ts、matcherEngine.ts；types/hooksConfig.ts:23-49 | 硬编码 claude |
| 02-27 | 同页单例面板 + 侧栏右键入口（C13-7） | workspace/pageApis.ts:109-134 | 通用机制但 claude 触发 |
| 02-28 | 双模式编辑 + 双向转换同步（JSON/GUI） | HooksConfigPanel.tsx:290-310,392-397；configModel.ts:94,155 | 通用机制但 claude 触发 |
| 02-29 | 测试侧：E2E 用户目录隔离（E2E-05） | e2e-tests/run-wdio.cjs:47-94 | 通用机制但 claude 触发 |

### 2.3 历史会话（03，24 项）

| # | 优化 | 位置 | 专属程度 |
|---|------|------|----------|
| 03-1 | 扫描根单点 resolve_projects_root（`~/.claude/projects`） | claude_history/scan.rs:21-26 | 硬编码 claude |
| 03-2 | 存储布局假设（cwd 编码目录 + uuid.jsonl，禁反解码） | scan.rs:51-67 | 硬编码 claude |
| 03-3 | 排除规则（agent-* / 非 UUID / subagents 不递归） | scan.rs:72-80 | 硬编码 claude |
| 03-4 | is_uuid_filename UUID 形态校验 | mod.rs:59-74 | 通用机制但 claude 触发 |
| 03-5 | 头部 512KB + 尾部 64KB 双窗口轻量解析 | jsonl.rs:16-19,50-212 | 通用机制但 claude 触发 |
| 03-6 | 可见 prompt 判定（isMeta/tool_result 数组/< 占位符跳过） | jsonl.rs:134-148 | 硬编码 claude |
| 03-7 | 标题回退链 custom-title > ai-title > summary > prompt（决策 22） | jsonl.rs:217-237 | 硬编码 claude |
| 03-8 | 降级条目契约 + 扫描根缺失空数组（BE-02） | scan.rs:44-46,86-109 | 通用机制但 claude 触发 |
| 03-9 | cwd 内容解析 + cwd_exists 孤儿判定 + mtime 口径 | scan.rs:97-99；112-121；mod.rs:42,51-53 | 硬编码 claude |
| 03-10 | SEC-05 sessionId 严格校验 + 定位不信托前端 | ops.rs:22-56 | 通用机制但 claude 触发 |
| 03-11 | delete 范围：jsonl + 同名 `<id>/` 目录（subagents 数据） | ops.rs:78-86 | 硬编码 claude |
| 03-12 | HistorySession 七字段 DTO + TitleSource 五变体 | mod.rs:23-54 | 硬编码 claude |
| 03-13 | `claude --resume <id>` 命令注入 + 面板标题 "claude" | restoreSession.ts:122-140 | 硬编码 claude |
| 03-14 | 四步恢复编排框架（FE-06） | restoreSession.ts:71-141 | 通用机制但 claude 触发 |
| 03-15 | buildResumeCommand：`cd '<dir>' && claude --resume <id>` | historyContextMenu.ts:51-54 | 硬编码 claude |
| 03-16 | 操作矩阵禁用态 + 双击三分派 + 动作弹窗 | historyContextMenu.ts:63-78 | 硬编码 claude |
| 03-17 | 四态同源 deriveActiveSessionStatuses（问题 2） | historyModel.ts:123-137 | 硬编码 claude |
| 03-18 | 双行式行：四态 emoji + ✗ 孤儿标记 + CLI logo（`getSrc("claude")`） | HistorySessionRow.tsx:49-114 | 硬编码 claude |
| 03-19 | findPanelForSession 反查 + switchToPageAndFocus | HistorySessionList.tsx:192-204,281-296 | 硬编码 claude |
| 03-20 | 删除流程：ask → deleteHistorySession → removeLocal | HistorySessionList.tsx:334-351 | 通用机制但 claude 触发 |
| 03-21 | 三下拉框 + 懒加载 scan + 组默认收起 | ClaudeHistorySections.tsx:133-235,149-156；HistorySessionList.tsx:253-268 | 通用机制但 claude 触发 |
| 03-22 | 展示派生纯函数（isCurrentProject/groupByCwd/matchesSearch/formatRelativeTime） | historyModel.ts:21-111 | 完全通用 |
| 03-23 | 活跃区标题覆盖（对齐 /rename 写 custom-title） | AgentStatusView.tsx:126-141 | 硬编码 claude |
| 03-24 | IPC 命令名 claude_history_scan/delete + DTO 双边契约 | ipc/claudeHistory.ts:13-24；types/claudeHistory.ts:6-29 | 硬编码 claude |

### 2.4 UI 状态指示（04，19 项）

| # | 优化 | 位置 | 专属程度 |
|---|------|------|----------|
| 04-1 | F3 四态状态机 eventToStatus（hook 事件名→状态） | lib/claudeStatus.ts:41-75 | 硬编码 claude |
| 04-2 | ClaudeStatus 类型 + STATUS_EMOJI（⚡🟡✅❌） | lib/claudeStatus.ts:8-26 | 硬编码 claude |
| 04-3 | F9 CLI 品牌 logo 注册表机制（CliIconRegistry） | lib/cliIcons.ts:21-44 | 通用机制但 claude 触发 |
| 04-4 | F9 claude logo 内嵌注册 + claude.png 资源 | cliIcons.ts:48；public/cli-icons/claude.png | 硬编码 claude |
| 04-5 | 命令→页签标题注册表机制（TabTitleRegistry） | TabTitleRegistry.ts:31-53 | 通用机制但 claude 触发 |
| 04-6 | claude 页签标题规则 | tabRules.ts:11 | 硬编码 claude |
| 04-7 | OSC 133 消费侧页签联动（C/D 序列 → 标题/🟡/logo） | useCommandDetection.ts:42-77 | 通用机制但 claude 触发 |
| 04-8 | 页签状态应用 handleTabStateChange（title/icon/logo 条件更新与双清） | TerminalPanel.tsx:74-91 | 通用机制但 claude 触发 |
| 04-9 | hook-event 页签 emoji 直接通道 + claudeSession 写入 | useXterm.ts:349-373 | 硬编码 claude |
| 04-10 | TerminalRegistry.claudeSession 二态模型（存在即运行中） | TerminalRegistry.ts:12-21,80-98 | 硬编码 claude |
| 04-11 | DefaultTab 页签 emoji/logo 渲染（tabIcon && tabLogo 双条件） | PageDockviewHost.tsx:226-289 | 通用机制但 claude 触发 |
| 04-12 | F8 重命名 claude 运行中禁用 | PageDockviewHost.tsx:163-175 | 硬编码 claude |
| 04-13 | Agent 状态视图行建模（F5 建行双通道/删行三通道） | useAgentStatus.ts | 硬编码 claude |
| 04-14 | 上下文用量条（CLAUDE_CONTEXT_LIMIT=200_000 口径） | consts.ts:5；AgentStatusRow.tsx:37-43 | 硬编码 claude |
| 04-15 | 活跃区行双行式展示（`getSrc("claude")`） | AgentStatusRow.tsx | 硬编码 claude |
| 04-16 | 相对时间 60s ticker | useAgentStatus.ts:77-80 | 通用机制但 claude 触发 |
| 04-17 | F4 通知调度（classifyEvent permission/error/done + 失焦门控 + 任务栏闪烁） | useClaudeNotifications.ts | 硬编码 claude |
| 04-18 | 历史区四态同源与消费（getSrc("claude")） | historyModel.ts:123-137 | 硬编码 claude |
| 04-19 | formatRelativeTime 六档相对时间纯函数 | historyModel.ts | 完全通用 |

### 2.5 键盘/快捷键/窗口（05，17 项）

| # | 优化 | 位置 | 专属程度 |
|---|------|------|----------|
| 05-1 | 复制键约定 Ctrl+Shift+C/V（Ctrl+C 让位） | keyboard.ts:20-39；commandCatalog.ts:36-50 | 硬编码 claude |
| 05-2 | Ctrl+C 保留为中断（不注册命令 → 透传 \x03） | keyboard.ts:47；commandCatalog.ts | 硬编码 claude |
| 05-3 | 终端控制字符保留键（Ctrl+C/V/X/Z/A，防重绑） | shortcuts/reserved.ts:14-20 | 通用机制但 claude 触发 |
| 05-4 | CM 内部键保留 + Ctrl+F 排除出浏览器加速键拦截 | reserved.ts:23-30；lib.rs:59-61 | 完全通用 |
| 05-5 | terminal.newline（Ctrl+Enter 写 `\n`，Ink 换行不提交） | keyboard.ts:40-46 | 硬编码 claude |
| 05-6 | attachCustomKeyEventHandler 委托式 fallback | useXterm.ts:229-237 | 通用机制但 claude 触发 |
| 05-7 | Kitty 键盘协议（CSI u）被动启用 | theme.ts:20-22 | 通用机制但 claude 触发 |
| 05-8 | WebView2 三层按键控制（prevent-default + capture） | lib.rs:56-69；ShortcutRegistry.ts:63 | 通用机制但 claude 触发 |
| 05-9 | 快捷键框架（指纹索引 + 上下文栈 + active 指针一次注册） | ShortcutRegistry.ts | 通用机制但 claude 触发 |
| 05-10 | 用户自定义重绑定 + 校验静默降级 | wireKeybindings.ts；keybindings.ts | 通用机制但 claude 触发 |
| 05-11 | global.closeTab（Ctrl+W 关页签） | commandCatalog.ts:27-34 | 完全通用 |
| 05-12 | HTML iframe 键盘转发（postMessage 两层入站校验 + 预留信任标记） | HtmlPanel.tsx；ShortcutRegistry.ts:151-159 | 通用机制但 claude 触发 |
| 05-13 | 关窗杀子进程（P1-19：遍历 kill + 3s 超时） | App.tsx:102-162 | 通用机制但 claude 触发 |
| 05-14 | Job Object KILL_ON_JOB_CLOSE 孤儿防护兜底 | pty/spawn.rs:1324-1425 | 完全通用 |
| 05-15 | 窗口焦点监听 + 任务栏闪烁回窗引导 | App.tsx:181-191；ipc/window.ts | 通用机制但 claude 触发 |
| 05-16 | Ctrl+Wheel 缩放 / 编辑器与 explorer 快捷键 | useFontSizeWheel.ts；editor/keyboard.ts | 完全通用 |
| 05-17 | CSP 放行 HTML 预览内联脚本（HTML 面板执行前提） | tauri.conf.json:24-25 | 通用机制但 claude 触发 |

## 三、专属程度分层统计

### 3.1 硬编码 claude（54 项）——显式 claude 字样或假设其行为

claude 专属依赖集中在**三类协议知识**，散布前后端多处：

| 协议知识 | 分布项 |
|----------|--------|
| **hook 事件名**（10 事件清单/四态映射/通知分类/30 事件目录） | 02-1/6/10/15/26、04-1/2/9/17（+01-21 消费侧） |
| **transcript 格式**（JSONL 字段/custom-title/ai-title/message.usage） | 03-6/7/9/12、02-16/17 |
| **CLI 命令与配置路径**（`~/.claude/`、`claude --resume`/`--fork-session`、settings.json 结构） | 02-2/3/18/20/21/25、03-1/2/3/11/13/15/23/24 |
| **终端协议行为适配**（Ink 同步哨兵 DA1、Chalk/COLORTERM、OSC 52、Ctrl+Enter、flags 0x7、SLTERM_PANEL_ID） | 01-1/2/3/4/18/22/23、05-1/2/5、02-7/8 |
| **运行态模型与 UI**（claudeSession 二态、F8 禁用、`getSrc("claude")` 两处、CLAUDE_CONTEXT_LIMIT） | 04-4/6/10/12/13/14/15/18、03-16/17/18/19 |

### 3.2 通用机制但 claude 触发（44 项）——机制可复用，当前激活场景仅 claude

| 机制类型 | 分布项 |
|----------|--------|
| 注册表/策略模式（机制通用，注册内容仅 claude） | 04-3/5（CliIconRegistry/TabTitleRegistry 均仅 1 条 claude 规则）、04-7/8/11、01-20 |
| 输出/渲染/展示适配管道 | 01-14（合帧）、01-16/17（resize）、01-10/19（OSC 133 注入侧/Kitty）、04-16（60s ticker） |
| 安全与容错模式 | 01-26（键盘委托）、02-4/5/9/11/12/13/14/19/22/23/24/27/28/29（含 9 原子 rename、双模式编辑、E2E-05 用户目录隔离）、03-4/5/8/10/14/20/21、05-3/6/7/8/9/10/12/13/15/17 |

### 3.3 完全通用（20 项）——claude 优化链路的底层支撑

01-5/6/7/8/9/11/12/13/15/24/25/27/28/29（14 项，Windows ConPTY 平台基础设施 + xterm 渲染能力）+ 03-22（展示派生纯函数）+ 04-19（formatRelativeTime）+ 05-4/11/14/16（CM 内部键/Ctrl+W 关页签/Job Object 兜底/Ctrl+Wheel 与编辑器快捷键）。

## 四、交叉链路观察（现状事实）

以下为跨领域依赖关系的客观描述，不含优化建议。

1. **hooks 信号链路是全链路数据源头**：`SLTERM_PANEL_ID` env（01-3）→ claude hook 触发 → reporter.js 组装 payload（02-6/7/8）→ watcher 双通道（02-12）→ `hook-event` 广播（02-14）→ 前端消费三处：F3 页签 emoji（01-21/04-9）、Agent 状态行与用量（04-13/14）、F4 通知（04-17）。同一条链路支撑四个 UI 域。
2. **四态同源单点数据层**：`TerminalRegistry.claudeSession`（04-10）是活跃区/历史区/F8 禁用/03 领域导航反查四路消费的单一真值源；写入来自双源——hook 事件路径（04-9）与 OSC 133 路径（04-7，经 matchedCommand）。OSC 133 注入侧（01-10）→ 消费侧（04-7）构成独立于 hooks 的第二通道。
3. **Ctrl+C 三层保护 + WebView2 三层按键控制**：目录无条目透传（05-2）→ reserved 保留键防重绑（05-3）→ 文档红线；配合 prevent-default 插件排除 Ctrl+F（05-4/8）保证 claude 工作流全局键（Ctrl+W/复制/中断）不被浏览器层吞掉。复制键约定（05-1）与 OSC 52 拦截（01-18）共用同一 `writeText` 剪贴板通道。
4. **关窗双保险**：前端 P1-19 遍历 kill + 3s 超时（05-13）与后端 Job Object KILL_ON_JOB_CLOSE（05-14/01-13）独立实现、互为兜底。
5. **抽象难易观察**（现状标注，非优化建议）：机制层抽象点已存在——CliIconRegistry/TabTitleRegistry 注册表、eventToStatus 纯函数、restoreSession 编排框架、classifyEvent、安全校验，均为"机制通用、claude 数据驱动"形态；硬编码 claude 的点集中在三处：①hook 事件名常量（前后端多处，`HOOK_EVENTS`/`eventToStatus`/`classifyEvent`/`eventsCatalog`）；②transcript 数据格式解析（jsonl.rs/usage.rs 字段级假设）；③CLI 专属命令与常量（`claude --resume`、`~/.claude/` 路径、200K、`getSrc("claude")` 两处硬编码、DA1/Ink 行为假设）。其中 `getSrc("claude")` 与行内注释已预留"按 CLI 标识扩展"的扩展点。
