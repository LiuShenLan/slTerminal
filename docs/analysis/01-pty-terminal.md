# PTY 与终端渲染 — claude 定制优化盘点

> 只读现状盘点（2026-08-08）。覆盖范围：后端 `src-tauri/src/pty/`、`src-tauri/assets/shell-integration.ps1`、前端 `src/panels/terminal/`，及 `src-tauri/src/pty/CLAUDE.md`、`src/panels/CLAUDE.md` 文档记载的决策。
>
> 专属程度三档口径：
> - **硬编码 claude**：代码/配置/文档中显式出现 claude 字样或假设其行为
> - **通用机制但 claude 触发**：机制本身通用，当前仅被 claude 的行为激活或为 claude 的场景设计
> - **完全通用**：与 claude 无关的底层能力（在 claude 优化直接相关的上下文中顺带标注）

## 相关文件

**后端**
- `src-tauri/src/pty/spawn.rs` — PTY 生命周期命令 + 自定义 ConPTY 创建（`conpty_custom`）+ 能力环境变量注入 + CPR 注入 + Job Object
- `src-tauri/src/pty/reader.rs` — reader 线程 + `strip_conpty_startup` 启动序列剥离 + DA1 查询模拟响应 + 16KB 缓冲
- `src-tauri/src/pty/shell.rs` — shell 选择（pwsh→powershell→cmd）+ `-EncodedCommand` 集成脚本内联注入
- `src-tauri/src/pty/mod.rs` — 模块入口（仅模块声明）
- `src-tauri/src/pty/win_build.rs` — Windows build 号获取（`nt_version` crate RtlGetNtVersionNumbers 低 28 位，01-28 引用对象）
- `src-tauri/src/state.rs` — `PtySession`/`PtyState` + ring buffer（E1 回放缓存）
- `src-tauri/assets/shell-integration.ps1` — PowerShell 集成脚本（OSC 7 + OSC 133 A/B/C/D + UTF-8 修复）
- `src-tauri/src/pty/CLAUDE.md` — 模块决策文档（DA1/COLORTERM/PASSTHROUGH_MODE/strip/CPR 等段落）

**前端**
- `src/panels/terminal/usePtyOutput.ts` — 输出合帧（阈值分流 + Idle/Max 双定时器 + DEC 2026）+ 非焦点降频
- `src/panels/terminal/usePtyResize.ts` — ResizeObserver X/Y 分离 debounce + NaN 守卫
- `src/panels/terminal/useClipboardHandler.ts` — OSC 52 剪贴板拦截
- `src/panels/terminal/useCommandDetection.ts` — OSC 133 命令边界检测 + 页签标题/图标/logo 联动
- `src/panels/terminal/useXterm.ts` — 编排层（spawn 等待布局、onHookEvent 四态消费、OSC 8、键盘委托、windowsPty）
- `src/panels/terminal/useTerminalInstance.ts` — Terminal 实例 + WebGL/FitAddon 生命周期
- `src/panels/terminal/webgl.ts` — WebGL 检测 + 指数退避重试
- `src/panels/terminal/theme.ts` — xterm 选项（Kitty 键盘协议被动启用等）
- `src/panels/terminal/keyboard.ts` — 终端快捷键命令工厂（Ctrl+C 不注册 / Ctrl+Enter newline）
- `src/panels/terminal/TerminalPanel.tsx` — 面板组件（handleTabStateChange 消费侧/originalTitleRef/logoRef，F3/F9 消费链落点；可见性推导 :57-64）
- `src/panels/terminal/TerminalRegistry.ts` — claudeSession 二态模型 + setClaudeSession（04-10 完整盘点）
- `src/panels/terminal/tabRules.ts`、`TabTitleRegistry.ts` — 命令→标题映射（claude 规则）
- `src/lib/cliIcons.ts` — CLI 品牌 logo 注册表（claude 内嵌注册）
- `src/lib/claudeStatus.ts` — F3 四态映射（`eventToStatus`）
- `src/panels/CLAUDE.md`、`src/lib/CLAUDE.md` — 决策文档

## 优化项清单

### 后端（PTY 层）

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 1 | DA1 查询模拟响应 | `pty/reader.rs:104-120`（注入）、`reader.rs:274-298`（`should_inject_da1`/`mirror_da1_query` 纯函数） | 扫描输出中的 `ESC[c`/`ESC[0c`，向子进程 stdin 注入 `ESC[?64;22c`（VT420+ANSI 颜色）；`AtomicBool` 每会话仅注入一次 | Ink 渲染器启动时发 DA1 查询作为同步哨兵，ConPTY 拦截不返回响应 → claude `waitFor` 永不 resolve、阻塞约 60s | **硬编码 claude** |
| 2 | 终端能力环境变量注入（COLORTERM/TERM/TERM_PROGRAM） | `pty/spawn.rs:931-936`（extra_envs）、`spawn.rs:929`（注释）、`spawn.rs:103-124`（`build_env_block`） | spawn 阶段向子进程环境块注入 `COLORTERM=truecolor`/`TERM=xterm-256color`/`TERM_PROGRAM=slTerminal`；Windows 与非 Windows 双路径 | Claude Code 依赖 COLORTERM 启用 24-bit RGB（Chalk/supports-color 检测信号） | **硬编码 claude** |
| 3 | `SLTERM_PANEL_ID` 环境变量注入 | `pty/spawn.rs:935` | 与上项同批注入，子进程据此识别所属面板；hooks 信号文件（`slterm-hook-reporter.js`）靠它路由事件到对应页签 | claude 的 hooks 通道（信号文件 `panelId` 字段来源） | **硬编码 claude** |
| 4 | ConPTY flags 固定 0x7 守卫（PASSTHROUGH_MODE 0x8 移除） | `pty/spawn.rs:58-73`（`compute_conpty_flags` + 完整注释）、`spawn.rs:479-497`（4 条测试锁死 0x7） | 绕过 portable-pty 直接调 `CreatePseudoConsole` 以完全控制 flags；固定 `INHERIT_CURSOR|RESIZE_QUIRK|WIN32_INPUT_MODE`（0x7），禁用 0x8 | claude 全屏 TUI（v2.1.89+ 默认 alt buffer + mouse tracking）在 0x8 下鼠标滚轮完全失效（2026-07 Win11 build 26200 实测） | **硬编码 claude** |
| 5 | ConPTY 启动序列剥离（`strip_conpty_startup`） | `pty/reader.rs:179-250`、`reader.rs:257-268`（`apply_startup_strip`） | 首轮读取剥离 `VtIo::StartIfNeeded()` 注入序列：OSC 0/2 窗口标题（BEL→蜂鸣）、清屏 `2J/3J`、光标归位 `H`、光标显隐 `?25h/l`、DSR `6n`；仅剥 OSC 0/2，OSC 7/133 等保留 | 无特定 claude 行为（对任何子进程生效）；保留 OSC 7/133 为 shell-integration 链路服务 | **完全通用** |
| 6 | CPR 注入（`\x1b[1;1R`） | `pty/spawn.rs:1018-1027` | spawn 后（锁外）立即向 stdin 写 `\x1b[1;1R`，补偿 ConPTY DSR 握手 | 无（通用 ConPTY 兼容） | **完全通用** |
| 7 | cwd 反斜杠规范化 | `pty/spawn.rs:128-130`（`build_cwd_wide`） | `CreateProcessW` 对 `/` 行为异常，传 ConPTY 前将 cwd 中 `/` 替换为 `\` | 无（Windows 平台基础兼容） | **完全通用** |
| 8 | reader 缓冲区 16KB（`READER_BUF_SIZE`） | `pty/reader.rs:20` | 189KB/s 输出场景约 12 次/秒 read()（4KB 为 47 次/秒），减少约 75% 系统调用 | 无特定 claude 行为（高吞吐通用优化；189KB/s 量级与 claude 流式输出场景相符但未绑定） | **完全通用** |
| 9 | E1 Channel 可替换 + ring buffer 回放 | `state.rs:201-218`（`ring_buffer_append`）、`spawn.rs:1048-1050`、`spawn.rs:1264-1322`（`pty_reattach` drain 回放） | 无条件缓存 ring buffer + reattach drain 回放；无前端消费路径（详见详细节） | 无（不依赖任何 claude 行为；E1 为预留机制） | **完全通用** |
| 10 | shell-integration.ps1（OSC 7 + OSC 133 + UTF-8 修复） | `assets/shell-integration.ps1:5-9`（UTF-8）、`17-22`（OSC 7 cwd）、`24-29`（OSC 133 A/B/D）、`54-62`（Enter hook 发射 OSC 133 C 携命令行） | `prompt()` 包裹注入 OSC 133 A/B/D（提示符边界 + 退出码）；Enter 键 hook 在命令执行前发射 OSC 133 C 携带命令行文本；OSC 7 跟踪 cwd；UTF-8 编码修复中文 GBK 乱码 | Enter hook 设计动机即"供前端检测特定命令（如 claude）以切换页签标题/图标"（ps1:52-53 注释）；OSC 133 C 消费端当前仅匹配 claude 规则 | **通用机制但 claude 触发** |
| 11 | `-EncodedCommand` 内联脚本注入 | `pty/shell.rs:133-156`（`build_pwsh_command`/`build_pwsh_info`）、`shell.rs:161-167`（UTF-16LE Base64） | 集成脚本经 `include_str!` 编译期嵌入，UTF-16LE Base64 内联传参，不写 `%APPDATA%` 文件，避免 AMSI/ASR 误杀 | 无（shell 集成通用技术） | **完全通用** |
| 12 | SPAWN_LOCK 串行化 | `pty/spawn.rs:949-1006`（锁内 ConPTY 创建+子进程启动）、`spawn.rs:1008-1009`（锁外操作） | 并发 spawn 卡死 ConPTY 输出管道，锁仅保护 `create_conpty_pair` + `spawn_conpty_child` | 无（Windows ConPTY 平台约束） | **完全通用** |
| 13 | Job Object 孤儿防护 | `pty/spawn.rs:1333-1348`（`add_to_job_object`）、`1384-1425`（`create_and_assign_job` Win32 执行） | 每个子进程放入 Job Object，`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`（0x2000），父进程崩溃时 OS 杀全部子进程 | 无（进程生命周期通用）；claude 作为长驻子进程是主要受益对象 | **完全通用** |

### 前端（终端渲染层）

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 14 | 输出合帧管道（阈值分流 + Idle/Max 双定时器 + DEC 2026） | `usePtyOutput.ts:21`（直写阈值 64B）、`24-26`（Idle 2ms/Max 16ms）、`16-18`（DEC 2026 常量）、`79-84`（Uint8Array 缓冲）、`119-155`（`flushBuffer`）、`183-258`（`handlePtyOutput`）、`198-201`（按字节计阈值防 CJK 绕过） | <64B 直写终端（低延迟打字回显），≥64B 合帧缓冲；空闲 2ms flush、最多 16ms 强制 flush；`Uint8Array[]` 合并单次 `term.write` 减 GC 60-80%；DEC 2026 包裹原子渲染消除撕裂 | Claude Code 基于 Ink (React-in-terminal)，约 60fps 全帧刷写 ANSI，单次输出通常 64-200 字节（逐 token 级） | **通用机制但 claude 触发** |
| 15 | 非焦点终端降频 + WebGL 按可见性释放 | `usePtyOutput.ts:201-236`（visible 门控累积、64KB 上限）、`useXterm.ts:409-423`（P1-13：不可见 dispose WebGL、切回 flush 回放） | `visible=false` 仅累积不 flush（上限 64KB），切回可见时立即回放；隐藏面板同时释放 WebGL context | 无特定 claude 行为（多页面通用资源管理；claude 长时间任务运行中切页面的场景受益） | **完全通用** |
| 16 | Resize X/Y 分离 debounce + NaN 守卫 | `usePtyResize.ts:63-111`、`19`（100ms 列 debounce） | 行数变化（廉价）立即 fit+resize；列数变化（re-wrap 成本高）100ms debounce；`Number.isFinite` 防 `proposeDimensions` NaN（xtermjs#4338）；resize 前 `cancelPendingFlush` 丢弃旧尺寸数据 | 针对 Claude Code Ink TUI 在 resize 后画面错位问题（调查 #3）——Ink 在 SIGWINCH 后以新尺寸重绘 | **通用机制但 claude 触发** |
| 17 | 交替缓冲 resize 必须 `fit()` 同步网格 | `usePtyResize.ts:86-107`（fit 调用链）；决策记载于 `panels/CLAUDE.md`「交替缓冲 resize」段 | `pty.resize()` 只发 SIGWINCH 不改变 xterm 网格，交替缓冲中必须 `fitAddon.fit()` 同步 `term.rows/cols`，否则新尺寸输出渲染到旧网格造成永久撕裂 | Ink 全屏 TUI 在 alt buffer 中处理 SIGWINCH（claude v2.1.89+ 默认 alt buffer） | **通用机制但 claude 触发** |
| 18 | OSC 52 剪贴板拦截 | `useClipboardHandler.ts:37-66`、`15`（1MB 上限） | 注册 `registerOscHandler(52)`：仅写入（拒绝读请求 `?`）、仅系统剪贴板（c）、焦点门控（不可见面板忽略）、payload ≤1MB、`atob`+`TextDecoder` CJK 解码，写入路径与 Ctrl+Shift+C 共用 | Claude Code `/copy` 命令经 OSC 52（`ESC]52;c;<base64> BEL`）写系统剪贴板；xterm.js 核心有 handler 但无 addon 时静默丢弃（调查 #4） | **硬编码 claude** |
| 19 | Kitty 键盘协议（CSI u）被动启用 | `theme.ts:22`（`vtExtensions: { kittyKeyboard: true }`） | xterm 声明 CSI u 能力，子进程经 `CSI>1u`（Disambiguate 模式）主动激活差异化编码；未激活时回退传统 handler | claude 全屏 TUI 的 kitty 协议输入编码（PASSTHROUGH_MODE 回归注释亦提及 kitty 协议为阻断条件之一） | **通用机制但 claude 触发** |
| 20 | OSC 133 命令边界检测 + 页签标题/图标/logo 联动 | `useCommandDetection.ts:42-72`、`tabRules.ts:11`（claude 规则）、`TabTitleRegistry.ts:41-44`（首 token 匹配）、`cliIcons.ts:48`（claude logo） | 解析 OSC 133 C（携命令行）→ `tabTitleRegistry.match` 首 token 精确匹配 → `onTabStateChange({active, title: rule.title, icon:"🟡", logo})` + `TerminalRegistry.setClaudeSession`；OSC 133 D → 恢复原标题/清图标/清会话行；注册表可扩展但当前仅注册 claude 一条规则 | claude 命令启动/退出（Enter 提交时 shell 注入 OSC 133 C/D） | **通用机制但 claude 触发** |
| 21 | F3 页签四态 emoji（hook-event 消费侧） | `useXterm.ts:349-373`、`src/lib/claudeStatus.ts`（`eventToStatus` 映射）、`panels/CLAUDE.md`「F3 页签四态指示」段 | 订阅 `hook-event` 事件流按 panelId 过滤 → `eventToStatus`（working⚡/attention🟡/done✅/error❌）→ `onTabStateChange` + `setClaudeSession`（sessionId/transcriptPath/status 空串归一 `|| undefined`） | claude hooks 通道（SessionStart→attention、UserPromptSubmit/PreToolUse/PostToolUse→working、Notification（仅 permission_prompt/idle_prompt/agent_needs_input 子类型）→attention、PermissionRequest→attention、Stop→done、StopFailure/PostToolUseFailure→error） | **硬编码 claude** |
| 22 | `terminal.newline`（Ctrl+Enter 写 `\n`） | `keyboard.ts:40-46` | 命令 handler 经 active 指针派发，向 PTY 写 `0x0a`（Ctrl+J 等价），不触发提交 | Ink 据此插入换行不提交（claude 多行输入） | **硬编码 claude** |
| 23 | Ctrl+C 保留为中断（不注册命令） | `keyboard.ts:7,47-48`；`src/features/shortcuts` 保留键机制（`isReserved`） | `createTerminalShortcuts` 不注册 Ctrl+C——`ShortcutRegistry` 无匹配即透传，xterm 自然发送 `\x03` 到 PTY；保留键集合亦禁止用户重绑 | claude 用它取消操作（中断） | **硬编码 claude** |
| 24 | WebGL 渲染 + DOM 兜底 | `webgl.ts:28-40`（`detectWebgl`）、`60-138`（`setupWebglWithRetry` 指数退避）、`useTerminalInstance.ts:142-147`（加载链） | WebGL2 预检（`failIfMajorPerformanceCaveat`）→ WebglAddon 加载；context loss 后指数退避重建（1s→16s，5 次），耗尽回退 DOM 渲染器；仅可见终端持有 context（按面板可见性管理） | 无特定 claude 行为（项目定位"渲染 GPU 加速"，为 60fps Ink 帧刷写提供渲染性能） | **完全通用** |
| 25 | PTY spawn 等待布局就绪（rAF 轮询） | `useXterm.ts:262-334`（`pollFitAndSpawn`，30 帧/500ms 上限，超时回退 80×24） | 挂载后 rAF 轮询容器 `offsetWidth > 0`，就绪后 fit + `proposeDimensions` 以真实 `cols×rows` spawn | 无（布局就绪通用保障） | **完全通用** |
| 26 | attachCustomKeyEventHandler 委托式 fallback | `useXterm.ts:229-237` | xterm 内部 keydown 前拦截，`getShortcutRegistry().resolve(event,"terminal")` 命中即 preventDefault + 返回 false；未命中透传（Ctrl+C 等控制字符发往 PTY） | 机制通用（xterm 6.1 升级后 focusin 冒泡失效的兜底）；设计动机含保证 claude 工作流键（Ctrl+Enter/Ctrl+C）在 capture 失效时仍可用 | **通用机制但 claude 触发** |
| 27 | OSC 8 超链接 | `useXterm.ts:240-244` | xterm.js 原生 OSC 8 解析渲染 + `linkHandler.activate` → `openUrl` 系统默认浏览器 | 无特定 claude 行为（claude 输出中的 URL 受益，机制本身通用） | **完全通用** |
| 28 | ConPTY buildNumber 设置（reflow 阈值） | `useXterm.ts:221-226`、`398-406` | `term.options.windowsPty = { backend: "conpty", buildNumber }`，xterm.js 按 build 号决定 ConPTY reflow 行为 | 无（xterm.js ConPTY 兼容通用） | **完全通用** |
| 29 | `pty_kill` async + spawn_blocking（ClosePseudoConsole 永久阻塞防护） | `pty/spawn.rs:1211-1255` | 先提取 session 释放 RwLock 写锁（<1ms），再在 `spawn_blocking` 内执行 kill → join reader → drop master；`ClosePseudoConsole` 在 pre-Win11 24H2 上永久阻塞，避免持锁阻塞导致后续命令级联卡死 | 无（Windows ConPTY 平台约束） | **完全通用** |

## 详细机制描述

### 1. DA1 查询模拟响应（硬编码 claude）

Claude Code 的 Ink 渲染器启动时发送 DA1 终端查询（`ESC[c`）作为同步哨兵（`waitFor` 同步点）。Windows ConPTY 拦截 DA1 查询后内部处理、不向子进程 stdout 返回响应，导致 Ink 的 `waitFor` Promise 永不 resolve，阻塞约 60s。`reader.rs:104-120` 在 reader 线程**每轮读取剥离后**扫描输出字节流（不依赖 drained 状态——仅整轮全为启动序列被 `apply_startup_strip` 返回 None 时跳过该轮，其余每轮均检测直至注入一次）：`mirror_da1_query`（`reader.rs:282-298`，滑动窗口检测 `ESC[c`/`ESC[0c`，DA2 `ESC[>c` 与 XTVERSION `ESC[>0q` 不触发）命中后向子进程 stdin 注入 `ESC[?64;22c`（VT420 + ANSI 颜色），模拟 ConPTY + conhost 的一致行为；`da1_injected: AtomicBool`（spawn.rs:1068 创建）防重复注入（每会话一次）。检测决策抽为纯函数 `should_inject_da1`（`reader.rs:274-276`），注入动作依赖 writer 锁 + 管道 I/O 不可抽（豁免明细见 `pty/CLAUDE.md`「reader_loop I/O 编排残余豁免」）。出处：`pty/CLAUDE.md`「DA1 查询模拟响应」段。

### 2. 终端能力环境变量注入（硬编码 claude）

`pty_spawn` 在 spawn 阶段构造 `extra_envs`（`spawn.rs:931-936`）经 `spawn_conpty_child` 的 `build_env_block`（`spawn.rs:103-124`，Vec 保持插入顺序、extra 覆盖同名键）合并进子进程环境块；非 Windows 路径等价注入（`spawn.rs:996-999`）。代码注释（`spawn.rs:929`）明言"Claude Code 依赖此宣告启用 True Color"：`COLORTERM=truecolor` 是 Chalk/supports-color 的核心检测信号；`TERM=xterm-256color` 防御性能力宣告；`TERM_PROGRAM=slTerminal` 为品牌标识（非功能性）。注入在 spawn 阶段（非 shell rc），子进程一启动即可见；对 pwsh/powershell/cmd 统一注入。出处：`pty/CLAUDE.md`「终端能力环境变量注入」段。

### 3. `SLTERM_PANEL_ID` 环境变量注入（硬编码 claude）

与上项同批注入（`spawn.rs:935` Windows ConPTY 路径 extra_envs；`spawn.rs:999` 非 Windows `cmd.env`），值为 `request.panel_id`。claude hooks 通道的 `slterm-hook-reporter.js` 脚本读取 `process.env.SLTERM_PANEL_ID` 写入信号文件 `panelId` 字段，后端据此把 hook 事件路由到对应页签（F3 四态/Agent 状态的数据源头）。无此变量（非 slTerminal 启动的 claude）→ 脚本直接 `process.exit(0)`（C10 契约）。集成测试 `pty_env_injects_slterm_panel_id`（`tests/pty_integration_tests.rs:393-437`）验证子进程环境含该变量。出处：`src-tauri/src/hooks/CLAUDE.md`「SLTERM_PANEL_ID 环境变量路由」段。

### 4. ConPTY flags 固定 0x7 守卫（硬编码 claude）

`conpty_custom` 模块（`spawn.rs:30-801`；其中 30-471 为生产代码约 440 行、472-800 为 `#[cfg(test)] mod tests` 测试模块）绕过 `portable_pty::native_pty_system().openpty()`（portable-pty 0.9.0 硬编码 flags=0x7 不暴露 `CreatePseudoConsole` 的 `dwFlags`），直接调 `CreatePseudoConsole`/`CreateProcessW` 获得 flags 完全控制。`compute_conpty_flags`（`spawn.rs:71-73`）固定返回 `INHERIT_CURSOR|RESIZE_QUIRK|WIN32_INPUT_MODE`（0x7）。**PASSTHROUGH_MODE (0x8) 曾按 build 动态启用后被移除**：0x8 下 claude 全屏 TUI（v2.1.89+ 默认 alt buffer + mouse tracking）滚轮完全失效——诊断埋点显示 xterm 的 SGR wheel report（`\x1b[<64/65;x;yM`）完整写入 ConPTY stdin 但 claude 无反应；去掉 0x8 后恢复。疑似机制为 passthrough 下 conhost 不解析子进程输出、不跟踪 DECSET mouse mode。**无法用最小实验/自动化守卫**（node 直接子进程复现失败，阻断条件仅真实 claude 场景：pwsh→claude 进程树 + kitty 协议）；`compute_conpty_flags` 4 条测试（`spawn.rs:479-497`）锁死「任何 build 都返回 0x7」。**L4 侧守卫**：`e2e-tests/terminal.e2e.ts:241` describe（E2E-04）「全屏 TUI 大负载输出 + 切页签往返」用例（`it` 起于 :252；:255 注释为「// 1. 全屏 TUI 模拟负载：进入交替缓冲 → 满屏定位行（40 行 × 每行 80 字符）→ 退出交替缓冲」；「全屏 TUI（如 claude Ink 界面）」字样在 :248（describe 级注释的 M2 人工确认项）；用例内 :252-260 无 mouse tracking 字样（:244 注释仅提「交替缓冲」））是 flags 回归的视觉守卫（M2 人工验证点）——01-4 守卫链 = L1 配置层 4 条 + L4 视觉层一条。出处：`pty/CLAUDE.md`「自定义 ConPTY 创建」段引用的 PASSTHROUGH_MODE 警示块。

### 5. ConPTY 启动序列剥离（完全通用）

`strip_conpty_startup`（`reader.rs:179-220`）在首轮读取剥离 `VtIo::StartIfNeeded()` 注入的启动序列：OSC 0/2 窗口标题（含 BEL→xterm 蜂鸣）、清屏 `ESC[2J/3J`、光标归位 `ESC[H`、光标显隐 `ESC[?25h/l`、DSR `ESC[6n`；`find_osc_end`（`reader.rs:222-231`）以 BEL 或 ST 终结，`match_csi_startup`（`reader.rs:233-250`）逐 CSI 匹配。**仅剥 OSC 0/2 窗口标题**，OSC 1/3/4/9（图标/属性/调色板/桌面通知，`reader.rs:397-411` `strip_preserves_non_title_osc`）与 OSC 7 cwd（`reader.rs:392-395` `test_preserve_osc7_cwd`）原样透传——OSC 7 保留即 shell-integration cwd 跟踪链路不受影响。`apply_startup_strip`（`reader.rs:257-268`）返回 None 时不置 drained，支持跨 16KB 边界的残留启动序列（BE-13）。非 Windows 平台原样返回（`cfg!(windows)` 编译期常量）。无 claude 专属行为绑定。出处：`pty/CLAUDE.md`「ConPTY 启动序列剥离」段。

### 6. CPR 注入（完全通用）

spawn 后（SPAWN_LOCK 外）向 stdin 写 `\x1b[1;1R`（`spawn.rs:1020-1027`），补偿 ConPTY `VtIo::StartIfNeeded()` 的 DSR 握手，避免首次读取时 DSR 死锁。蜂鸣与首字符消失由 `strip_conpty_startup` 处理，与此机制无关。出处：`pty/CLAUDE.md`「CPR 注入（Windows）」段。

### 7. cwd 反斜杠规范化（完全通用）

`build_cwd_wide`（`spawn.rs:128-130`）将 cwd 中 `/` 替换为 `\` 后 UTF-16LE 编码——`CreateProcessW` 对 `/` 行为异常。出处：`pty/CLAUDE.md`「cwd 反斜杠规范化」段。

### 8. reader 缓冲区 16KB（完全通用）

`READER_BUF_SIZE = 16384`（`reader.rs:20`）：189KB/s 输出场景约 12 次/秒 read()（4KB 为 47 次/秒），减少约 75% 系统调用。出处：`pty/CLAUDE.md`「reader 缓冲区大小」段。

### 9. E1 Channel 可替换 + ring buffer 回放（完全通用）

`reader_loop` 经 `Arc<RwLock<Option<Channel>>>` 引用 Channel（`spawn.rs:1048-1049`）；**每轮读取无条件缓存**到 `output_ring`（`state.rs:201-218`，`ring_buffer_append`：256KB FIFO，超限从队首淘汰——`drain_target = 1024.min(len)` 窗口内找最后一个 `\n` 对齐行边界（丢弃量可变、可远小于 1KB，避免行内 UTF-8 序列截断），仅窗口内无换行的超长行按 1024 原量淘汰（P2-47，注释 `state.rs:200`）；P2-46：先缓存不 clone 再 send，成功路径零 clone——**不存在「断开→写 ring」分流决策**，无条件缓存恰是 reattach 回放完整性的前提）。`pty_reattach`（`spawn.rs:1264-1322`）替换 Channel → drain 回放 ring buffer → 检测已记录退出码补发 Exit 事件。**E1 机制本身真实存在但无前端消费路径**——`src/` 下无 `pty.reattach` 生产调用（仅 `ipc/pty.ts:74` wrapper 定义、契约测试与 `useXterm.ts:282` 注释「可供 reattach 查询」）；H6 跨页面存活由多 Dockview 实例 + CSS `display:none/block` 显隐实现，页面切换期间终端面板保持挂载、Channel 不断开，与 reattach 无关。出处：`pty/CLAUDE.md`「Channel 可替换 + ring buffer 回放（E1）」段（注：该段「服务于页面切换后恢复终端显示」为文档漂移表述，以代码为准）。

### 10. shell-integration.ps1（通用机制但 claude 触发）

`prompt()` 重写（`assets/shell-integration.ps1:34-50`）包裹注入：`Send-ExitCode`（OSC 133;D，退出码）、`Send-PrePrompt`（133 A）、`Update-Cwd`（OSC 7 cwd + OSC 9;9 ConEmu 兼容）、`Send-PostPrompt`（133 B），并保存/恢复 `$?` 与 `LASTEXITCODE` 防污染；文件头 UTF-8 修复（`ps1:5-9`，中文 Windows GBK/936 必须）。**Enter hook（`ps1:54-62`）**：`Set-PSReadLineKeyHandler -Chord Enter` 在命令执行前读取缓冲行发射 `OSC 133;C;<命令行>`——注释明言"供前端检测特定命令（如 claude）以切换页签标题/图标"；消费端 `useCommandDetection` 当前仅注册 claude 一条规则。OSC 133 协议本身通用（VS Code/Windows Terminal 同款——外部知识陈述，仓库内不可证；Enter hook 携命令行的设计动机已由 ps1:52-53 注释仓库内证实），Enter hook 携带命令行文本的设计为 claude 页签联动场景设计。**链路边界：仅限于 pwsh/powershell**——Enter hook 依赖 PowerShell PSReadLine API（`Set-PSReadLineKeyHandler`，ps1:54），cmd.exe 无等价机制（`panels/CLAUDE.md` 明示「shell integration 脚本仅在 PowerShell 注入」），抽象化须知晓此宿主依赖。出处：`panels/CLAUDE.md`「OSC 133 命令边界检测 + 页签标题/图标动态切换」段。

### 11. `-EncodedCommand` 内联脚本注入（完全通用）

PowerShell 以 `-NoProfile -NoLogo -NoExit -EncodedCommand <UTF-16LE Base64>` 启动（`shell.rs:133-156`），脚本经 `include_str!("../../assets/shell-integration.ps1")` 编译期嵌入（`shell.rs:193-195`），不写磁盘、消除 `%APPDATA%` 文件写入，避免 AMSI/ASR 误杀。`encode_utf16le_base64`（`shell.rs:161-167`）实现编码。shell 选择链 pwsh→powershell→cmd（`shell.rs:94-127`），`ShellInfo.program` 须为完整路径（`which_full_path` 解析，`shell.rs:173-183`）。无 claude 绑定。

### 12. SPAWN_LOCK 串行化（完全通用）

并发 spawn 卡死 ConPTY 输出管道——`pty_spawn` 中 SPAWN_LOCK（`state.rs` PtyState 字段，clone 移送 spawn_blocking）仅保护 `create_conpty_pair` + `spawn_conpty_child`（`spawn.rs:949-1006` 锁内区域——949-951 锁获取、955-963 `create_conpty_pair`、983-992 `spawn_conpty_child`，947 为 spawn_blocking 闭包起点），`take_writer`、CPR 注入、Job Object 分配、reader 线程启动在锁外（`spawn.rs:1008-1009`）。出处：`pty/CLAUDE.md`「SPAWN_LOCK 串行化」段。

### 13. Job Object 孤儿防护（完全通用）

`add_to_job_object`（`spawn.rs:1333-1348`）构建 Job Object 名称并委托 Win32 执行——1333-1425 区间覆盖四个函数：`add_to_job_object`（1333-1348）/ `job_name`（1352-1354，格式 `slTerminal_pty_{pid}`）/ `job_limits`（1361-1368，锁死 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`=0x2000）/ `create_and_assign_job`（1384-1425，`CreateJobObjectW` → `SetInformationJobObject` → `AssignProcessToJobObject`）。`JobHandle` RAII 在 `PtySession` 存活期间持有句柄防提前触发（`spawn.rs:817-852`）。父进程崩溃/退出时 OS 自动杀全部子进程（含 claude 进程树）。出处：`pty/CLAUDE.md`「Job Object 孤儿防护（Windows）」段。

### 14. 输出合帧管道（通用机制但 claude 触发）

针对 Claude Code Ink（React-in-terminal）约 60fps 全帧刷写 ANSI、单次输出通常 64-200 字节（逐 token 级）的输出特征设计。`handlePtyOutput`（`usePtyOutput.ts:183-258`）阈值分流：`rawBytes.length < 64` 且可见 → 直写终端（打字回显低延迟）；否则入 `pendingBufferRef: Uint8Array[]`（**缓冲原始字节、避免解码字符串拼接**——`handlePtyOutput` 仍逐块调 `decoderRef.decode` 供 E2E 文本缓冲记录，生产构建同样执行，「跳过」指合帧缓冲不拼接解码后字符串、合并单次 `term.write` 减 GC 60-80%）。**直写阈值按字节计数**（`usePtyOutput.ts:198-201`）：CJK 字符 UTF-8 每字符 3 字节，按 `text.length` 会低估数据量，使含中文的 Ink 输出绕过合帧路径造成撕裂。Idle+Max 双定时器（`usePtyOutput.ts:225-235`）：空闲 2ms 无新数据 flush；16ms 强制 flush 防饥饿（替代原纯 rAF 方案）。`flushBuffer`（`usePtyOutput.ts:119-155`）以 `\x1b[?2026h`/`\x1b[?2026l`（DEC 2026）包裹合并数据——xterm.js 6.0+ 原生支持，所有 grid 变更单帧内原子渲染消除撕裂。出处：`panels/CLAUDE.md`「输出合帧策略（针对 Claude Code Ink 流式输出优化）」段。

### 15. 非焦点终端降频（完全通用）

（注：本项标题合并两个独立 effect——非焦点降频（`usePtyOutput`）与 P1-13 WebGL 按可见性释放（`useXterm`），P1-13 为单一机制，WebGL 释放细节见 #24，抽象引用时勿当作同一机制。）`visible=false` 时输出仅累积不 flush（上限 64KB，`usePtyOutput.ts:201-236`），切回可见立即回放；`visibleRef` 避免 `handlePtyOutput` 依赖 `visible` 导致 PTY 回调重建。配套 P1-13（`useXterm.ts:409-423`）：面板不可见时 dispose WebGL addon 释放 context，切回时 `tryLoadWebgl` 重建 + `flushBuffer` 回放。无 claude 专属设计（多页面通用资源管理），claude 长时间任务运行中切页面的场景为主要受益场景。

### 16. Resize X/Y 分离 debounce + NaN 守卫（通用机制但 claude 触发）

针对 Claude Code Ink TUI 在 resize 后画面错位问题（调查 #3）。`ResizeObserver` 回调（`usePtyResize.ts:63-111`）：`proposeDimensions()` 结果经 `Number.isFinite` 守卫（WebGL 渲染器未就绪时可能返回 NaN，xtermjs#4338）；`prevDimsRef` 区分变化类型——仅行数变化（高度拖拽，廉价）立即 `fit()` + `pty.resize()`；列数变化（宽度拖拽，需 re-wrap）100ms debounce；resize 前 `cancelPendingFlush()` 清除定时器 + 丢弃积压缓冲（旧尺寸 PTY 数据渲染到新视口会错位；与 `flushBuffer` 的 DEC 2026 渲染路径区分，避免同帧几何变更撕裂）；无变化直接返回。出处：`panels/CLAUDE.md`「Resize X/Y 分离 debounce + NaN 防御」段。

### 17. 交替缓冲 resize 必须 `fit()` 同步网格（通用机制但 claude 触发）

`pty.resize()` 只发 SIGWINCH 给 ConPTY/子进程，不改变 xterm.js 网格尺寸；网格必须由客户端 `fitAddon.fit()` → `term.resize()` 更新。因此在交替缓冲（alt buffer）中**也必须调 `fit()`**——若跳过，Ink SIGWINCH 后新尺寸输出渲染到旧网格造成永久撕裂（`usePtyResize.ts:86-107` 的 fit 调用链承载）。交替缓冲 reflow 的短暂错位（≤1 帧）由 TUI 下一帧全量重绘覆盖。claude v2.1.89+ 默认 alt buffer + mouse tracking 全屏 TUI，是主要触发场景。出处：`panels/CLAUDE.md`「输出合帧策略」段末「交替缓冲 resize」子段。

### 18. OSC 52 剪贴板拦截（硬编码 claude）

Claude Code `/copy` 命令通过 OSC 52（`ESC]52;c;<base64> BEL`）写入系统剪贴板；xterm.js 6.0+ 核心解析器内建 OSC 52 handler 但无 addon 时静默丢弃（调查 #4）。`useClipboardHandler.ts:37-66` 在 `term.open()` 后注册自定义 handler：仅写入（payload 为 `?` 或空 → 忽略，不响应读请求，对齐 Windows Terminal/iTerm2/Alacritty 实践）；仅系统剪贴板选择器 `c`（忽略 primary/secondary）；焦点门控（`visibleRef.current === false` 忽略，防后台页签静默改剪贴板）；payload ≤1MB 防 DoS；`atob` → `Uint8Array` → `TextDecoder("utf-8")` 支持 CJK；写盘路径复用 `src/ipc/clipboard` 的 `writeText`（与 Ctrl+Shift+C 同一通道）。出处：`panels/CLAUDE.md`「OSC 52 剪贴板拦截（调查 #4）」段。

### 19. Kitty 键盘协议（CSI u）被动启用（通用机制但 claude 触发）

`theme.ts:22` 的 `vtExtensions: { kittyKeyboard: true }` 声明 xterm 的 CSI u 能力（Disambiguate 模式）。协议为被动模式：终端声明后应用需主动 push flags 激活；未激活时 `KeyboardService.useKitty` 返回 false 回退传统 handler。注释明言受益者"子进程（如 Claude Code）"；PASSTHROUGH_MODE 回归注释（spawn.rs:60-68）亦把"pwsh→claude 进程树 + kitty 协议"列为复现阻断条件。**外部假设标注**：触发点「claude 全屏 TUI 的 kitty 协议输入编码」隐含「claude 主动通过 `CSI>1u` 激活」这一外部行为——仓库内仅有间接证据（theme.ts:20-21 注释 + spawn.rs:60-68 复现条件），claude 是否真实激活 CSI u 在仓库内不可证（外部假设，仓库内不可证）。出处：`panels/CLAUDE.md`「Kitty 键盘协议（CSI u）被动启用」段。

### 20. OSC 133 命令边界检测 + 页签联动（通用机制但 claude 触发）

`useCommandDetection.ts:42-72` 注册 `registerOscHandler(133)`（xterm 解析器剥离 OSC number 前缀，data 形如 `"C;claude"`/`"D;0"`）：**C**（命令即将执行）→ `tabTitleRegistry.match(command)` 首 token 精确匹配（`TabTitleRegistry.ts:41-44`，覆盖 `claude --resume`/`claude -p` 带参变体）→ 命中则置 `isCommandRunningRef`、`onTabStateChange({active:true, title: rule.title, icon:"🟡", logo: cliIconRegistry.match(command)})`（**title 来自注册表规则 `rule.title`（claude 规则即 "claude"），🟡 是 `icon` 字段值**）、`TerminalRegistry.setClaudeSession(panelId,{matchedCommand})`；**D**（命令退出）→ 清运行态、`onTabStateChange({active:false})`、清会话行——**该分支仅在 `isCommandRunningRef.current === true` 时处理**（`useCommandDetection.ts:62` 前置门控，未运行注册命令时 D 事件不触发任何清态）。规则经 `tabRules.ts:11` 注册——**当前仅 claude 一条**（无 icon 字段，emoji 由 F3 系统接管）；CLI logo 经 `cliIcons.ts:48` 内嵌注册 `claude → /cli-icons/claude.png`（F9，32×32 透明底渲染 16×16，随 frontendDist 内嵌 exe 同源加载）。`resetCommandState` 在 spawn 成功后调用覆盖持久化残留。机制为注册表可扩展（新增命令仅追加 register），但当前激活场景只有 claude。**注入侧链路边界：OSC 133 C 携命令行是 pwsh/powershell 专属注入**（Enter hook 依赖 PSReadLine，cmd.exe 无此能力，见 #10）。出处：`panels/CLAUDE.md`「OSC 133 命令边界检测 + 页签标题/图标动态切换」段 + `src/lib/CLAUDE.md`「cliIcons.ts」。

### 21. F3 页签四态 emoji（硬编码 claude）

`useXterm.ts:349-373` 订阅 `hooks.onHookEvent` 按 panelId 过滤 → `eventToStatus(payload.event, payload.notificationType)`（`src/lib/claudeStatus.ts`，四态映射单点：working⚡/attention🟡/done✅/error❌）→ `onTabStateChange({active:true, icon: emoji})`；`SessionEnd`/`Exit` → `setClaudeSession(panelId, null)`（`useXterm.ts:354-355`），但 **`{active:false}` 仅 `SessionEnd` 触发**（`:368-370`）——hook 事件流按 claude 规范不存在 `Exit` 事件（该分支为防御代码）；PTY 通道的 Exit 页签复位在 `usePtyOutput.ts:243-246`（`PtyEvent::Exit` + `isCommandRunningRef`）。其余事件写 `setClaudeSession`（sessionId/transcriptPath/status，**payload 空串归一 `|| undefined`** 防 claude hook 输入缺字段时下游静默失效）。事件源为 claude hooks 通道（`slterm-hook-reporter.js` 上报的 Claude Code hook 事件名），`eventToStatus` 全映射（`claudeStatus.ts:41-75`）：SessionStart→attention、UserPromptSubmit/PreToolUse/PostToolUse→working、Notification（仅 `permission_prompt`/`idle_prompt`/`agent_needs_input` 子类型，`ATTENTION_NOTIFICATION_TYPES` `claudeStatus.ts:29-33`）→attention、PermissionRequest→attention、Stop→done、PostToolUseFailure/StopFailure→error、SessionEnd→null——事件名称与语义完全绑定 claude 规范。已知行为：用户主动 Ctrl+C 中断不发射任何 hook 事件，⚡ 滞留至下一事件覆盖（panels/CLAUDE.md「中断场景已知行为（Ctrl+C）」段）；**内置衰减**：回提示符约 60s 无操作 → `idle_prompt`（attention 子类型）→ 自动转 🟡，无需超时机制（claudeStatus.ts:5-6 文件头注释 + panels/CLAUDE.md 同载）。出处：`panels/CLAUDE.md`「F3 页签四态指示（hook-event + emoji）」段。

### 22. `terminal.newline`（硬编码 claude）

`keyboard.ts:40-46`：`Ctrl+Enter` 命令经 active 指针派发，`writeToPty(new Uint8Array([0x0a]))` 写 `\n`（Ctrl+J 等价）——注释明言"Ink 据此插入换行不提交"。命令为可重绑注册命令（`commandFromMeta`，目录 `commandCatalog.ts`），handler 经 `getActiveTerminal()` 派发到聚焦终端；与 xterm 委托层（`attachCustomKeyEventHandler`）共享同一绑定表。出处：`panels/CLAUDE.md`「attachCustomKeyEventHandler — 委托式 fallback」段。

### 23. Ctrl+C 保留为中断（硬编码 claude）

`keyboard.ts:7,47-48`：`createTerminalShortcuts` 不注册 Ctrl+C——`ShortcutRegistry` 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY，注释明言"供 claude 取消操作"。双保险：`src/features/shortcuts` 的 `isReserved` 将 `Ctrl+KeyC` 在 terminal/global 标记为保留键，用户 overrides 也无法重绑。出处：`panels/CLAUDE.md`「Ctrl+C 保留为中断」段 + `src/features/shortcuts/CLAUDE.md`「Ctrl+C 保留为中断」。

### 24. WebGL 渲染 + DOM 兜底（完全通用）

`detectWebgl`（`webgl.ts:28-40`）首次创建临时 canvas 检测 WebGL2（`failIfMajorPerformanceCaveat: true`）并模块级缓存；可用则 `useTerminalInstance`（`useTerminalInstance.ts:142-147`）加载 WebglAddon。`setupWebglWithRetry`（`webgl.ts:60-138`）：context loss 或加载失败后指数退避重建（1000/2000/4000/8000/16000ms，5 次），重试耗尽回退 DOM 渲染器；`onContextLoss` 自动 dispose 释放 GPU 资源；返回 `cancel` 供卸载清理（含重试定时器）。**WebGL 按可见性释放见 #15**（P1-13 同一机制，`useXterm.ts:409-423`）；补充一点：仅可见终端持有 WebGL context——可见性由 `TerminalPanel.tsx:57-64` 推导（panelId 正则解析 pageId + `useLayout` 订阅 activePageId → `visible` prop 传入），`useXterm.ts:413-417` 的 P1-13 effect 仅消费 `visible` 分支（`visible === false` → dispose），与「焦点」概念无关。无 claude 专属绑定（项目定位约束"渲染 GPU 加速"，为 Ink 高频帧刷写提供渲染性能）。出处：`panels/CLAUDE.md`「WebGL 优先 + DOM 兜底」段。

### 25. PTY spawn 等待布局就绪（完全通用）

`useXterm.ts:262-334`：挂载后 rAF 轮询容器 `offsetWidth > 0`（最多 30 帧/500ms），就绪后 `canFit` 多条件守卫（`useXterm.ts:90-104`——实际 6 个判空/守卫分支：:96 isDisposedRef 缺失、:97 terminal/fitAddon 缺失、:98 containerEl 缺失、:99-100 尺寸为 0、:101 terminal.element 缺失、:102 isDisposedRef.current 已销毁；「五条件」为 panels/CLAUDE.md 沿袭口径（null/undefined/0/isDisposed/no element 五类），计数口径差异注意）+ fit + `proposeDimensions` 获取真实字符尺寸以 `cols×rows` 调 `pty.spawn()`；NaN 或超时回退 80×24。`doSpawn` 暴露给 `setupRetry`（Enter 重连），spawn 失败/进程退出后按 Enter 触发重连（`usePtyOutput.ts:158-180`）。无 claude 绑定。

### 26. attachCustomKeyEventHandler 委托式 fallback（通用机制但 claude 触发）

`useXterm.ts:229-237`：xterm.js 6.1.0-beta 升级后窗口级 capture 路径可能因 focusin 未正确冒泡而失效，`term.attachCustomKeyEventHandler` 委托进 `getShortcutRegistry().resolve(event, "terminal")`——命中即 `preventDefault()` + 返回 false（不交 xterm），未命中返回 true 透传（Ctrl+C 等控制字符发往 PTY）。无双触发：capture 命中即 stopPropagation，事件到不了 xterm；委托层仅作 capture 失效兜底。机制本身通用（xterm 升级回归的兜底），但设计动机含保证 claude 工作流键——`Ctrl+Shift+C/V`、`Ctrl+Enter`（Ink 换行）可用且 `Ctrl+C` 透传不丢（`panels/CLAUDE.md`「委托式 fallback」段：双保险确保上述键位）。出处：`panels/CLAUDE.md`「attachCustomKeyEventHandler — 委托式 fallback」段。

### 27. OSC 8 超链接（完全通用）

`useXterm.ts:240-244`：xterm.js 6.0.0 原生 OSC 8 解析渲染，`term.options.linkHandler.activate` → `src/ipc/shell` 的 `openUrl()` 打开系统默认浏览器（Tauri plugin-opener）。零新依赖。hover/leave 回调一期不做。无 claude 专属设计（claude 输出中的 URL 受益）。出处：`panels/CLAUDE.md`「OSC 8 超链接」段。

### 28. ConPTY buildNumber 设置（完全通用）

`useXterm.ts:221-226` 与 `398-406`（独立 useEffect 监听异步更新的 F3 Bug 1 修复）：`term.options.windowsPty = { backend: "conpty", buildNumber }`，Windows build 号经 `pty/win_build.rs`（`nt_version` crate RtlGetNtVersionNumbers 低 28 位）获取，xterm.js 按 build 号决定 ConPTY reflow 等行为。**前端 fallback**：`getWindowsBuildNumber()` 对 invoke 失败 catch 后回退 21376（ConPTY reflow 阈值兜底，`src/ipc/pty.ts:92-98`，`win_build.rs:4` 注释同载）；非 Windows 平台后端返回 `Err(AppError::Unknown(...))`（`win_build.rs:14-20`）。无 claude 绑定。

### 29. `pty_kill` async + spawn_blocking（完全通用）

`ClosePseudoConsole` 在 pre-Win11 24H2 上永久阻塞（G1b 注释 `spawn.rs:1211-1213`，`pty/CLAUDE.md`「pty_kill：async + spawn_blocking」段同载）。`pty_kill`（`spawn.rs:1216-1255`）：先从 `PtyState` 提取 session 释放 `RwLock` 写锁（`spawn.rs:1221-1236`，锁在此 scope 结束时释放 <1ms），再在 `tokio::task::spawn_blocking` 内执行 `child.kill()` → `drop(child)` → `handle.join()`（reader 回收）→ session drop → master drop → `ClosePseudoConsole`（`spawn.rs:1239-1250`）——避免持锁阻塞导致后续命令级联卡死。与 #12 SPAWN_LOCK、#13 Job Object 同级的 Windows ConPTY 平台机制。无 claude 绑定。

## 三档分布

| 专属程度 | 数量 | 项目编号 |
|----------|------|----------|
| 硬编码 claude | 8 | #1（DA1 响应）、#2（能力环境变量）、#3（SLTERM_PANEL_ID）、#4（flags 0x7 守卫）、#18（OSC 52）、#21（F3 四态）、#22（Ctrl+Enter）、#23（Ctrl+C） |
| 通用机制但 claude 触发 | 7 | #10（shell-integration OSC 133）、#14（输出合帧）、#16（Resize X/Y 分离）、#17（交替缓冲 fit）、#19（Kitty 键盘）、#20（OSC 133 命令检测）、#26（键盘委托） |
| 完全通用 | 14 | #5（启动序列剥离）、#6（CPR）、#7（cwd 规范化）、#8（16KB 缓冲）、#9（ring buffer 回放）、#11（EncodedCommand）、#12（SPAWN_LOCK）、#13（Job Object）、#15（非焦点降频）、#24（WebGL）、#25（rAF 等待）、#27（OSC 8）、#28（windowsPty）、#29（pty_kill spawn_blocking） |

**跨文件重叠**（清单表格未逐行标注，汇总去重时以本表为准）：01-3↔02-7（SLTERM_PANEL_ID 注入，01 视角 env 注入机制、02 视角信号路由键）、01-21↔04-9（F3 页签四态消费侧）、01-10/01-20↔04-7（OSC 133 注入侧/消费侧）、01-22↔05-5（terminal.newline）、01-13↔05-14（Job Object）、01-19↔05-7（Kitty 键盘）、01-26↔05-6（键盘委托）。重叠项分类在两侧文件间无冲突。

**规律观察**（仅描述现状）：claude 定制集中在两个方向——①**协议握手层**（DA1 哨兵、COLORTERM、flags 0x7、OSC 52/133 均为对 claude 具体行为的直接适配或规避）；②**输出特征适配层**（合帧、resize、交替缓冲针对 Ink 60fps 全帧刷写）。通用项为 Windows ConPTY 平台基础设施与 xterm 渲染能力，多数为 claude 优化链路的底层支撑（如 strip/CPR 保证协议握手干净、WebGL 保证帧渲染性能）。
