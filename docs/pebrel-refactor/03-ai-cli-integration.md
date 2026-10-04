# pebrel 重构优化 spec · 分片 03：AI CLI 集成

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

本分片覆盖八件事：

1. **hook 三层拓扑**——`slterm-hook` 独立小进程（发送端）、命名管道服务器 + 信封解析（传输/协议层）、每 pane 仲裁器（生命周期层）的整体移植裁定，含串台双门、有界载荷、事件门重排等承重机制。
2. **安装策略**——共享安装权威（codex 版本分流、marker 归属、配置守护）+ 九家一等 provider 的本地安装器（claude settings.json、codex config.toml/hooks、cursor/kimi/opencode/pi 插件落盘）+ 设置页检视八字段。
3. **provider 识别**——`AgentKind` 27 家 CLI 枚举 + 9 家一等集成清单 + 可执行文件发现（PATH 扩展目录、别名归属判据）。
4. **屏幕观察**——`agent_detection/*.toml` 声明式规则族（21 厂商 + `_shared.toml` 兜底）、`screen_context.rs` 结构判据、用户 `%APPDATA%` 覆盖与 mtime 节流。
5. **会话发现/恢复**——`ai_sessions.rs` 有界扫描（jsonl 头部 64KB）、hook 会话注册表合并、`assistant_answer.rs` 回答提取三态、`conversation.rs` 有界 transcript 投影（头部 256KB）。
6. **per-pane 环境契约**——`agent_env.rs` 身份变量组（`TERM_PROGRAM`/`PANE_ID`/`CLI`/`BIN_DIR`/PATH 前置/`WSLENV`），改名 `SLTERM_*` 后的形态裁定。
7. **slTerminal 侧现状对照**——`src-tauri/src/hooks/`（信号文件通道）、`agent_dirs.rs`、`agent_history/`、`src/features/cliProfiles/`、`agentFiles/`、`agentHistory/` 在新世界的归宿原则。
8. **托盘 attention 与通知交界**——`notify.rs::Notification` 与生命周期事件的接口面（细节归分片 09）。

已定全局决策：AI CLI 集成整体照抄（三层拓扑、安装策略、27 家清单、9 家一等、声明式屏幕规则、per-pane 环境契约、会话发现/恢复全保留不裁剪）；SSH/远程与多平台整支砍；AI assistant 细节归分片 08；`nebula_*` → `slterm_*`、`PEBREL_*`/`NEBULA_*` 环境变量 → `SLTERM_*`。

## 采纳点

### hook 三层拓扑

1. **照抄：`slterm-hook` 独立小进程整体**。pebrel `nebula_hook/src/main.rs`（`main` / `run` / `read_payload` / `envelope` / `send_local`）+ `nebula_hook/Cargo.toml` 的「pure std on purpose」零依赖纪律（进程启动即延迟预算，冷启动 <15 ms）。核心约束逐一保留：任何路径含 panic 都 exit 0（`catch_unwind` + `recv_timeout` 2s 一次性等待 + `chain_notifier` 不等待）、1MB 载荷上限（`MAX_PAYLOAD_BYTES`，超限抽干 stdin 但不转发截断 JSON）、stdin 通道无条件抽干（claude/kimi 等把事件 JSON 写进 hook stdin，不读会在 CLI 侧变成 hook write error）、不在白名单的首参数视为误调用且 Nebula 之外恒无声 no-op（`known_source` / `payload_on_stdin`）。这是整个事件链的第一推动力，行为合同（exit 0、快、无声）一个字不能动。
2. **照抄：`PEBREL_NOTIFY_PIPE` 环境作用域闸门**（`nebula_app/src/ai_hook.rs` 的 `PIPE_ENV` / `windows.rs::spawn_pipe_server`）。hook 配置全局（settings.json / config.toml），效果必须 Nebula-only——闸门是环境变量，只在 Nebula 派生的进程树里存在；无闸门即 `Outcome::NotHosted` 直接退出。ConPTY 合并当前进程环境，进程级 set 早于首个 PTY spawn 即覆盖全部 pane。slTerminal 版改名 `SLTERM_NOTIFY_PIPE`。
3. **照抄：`FOREIGN_HOOK_RUNNERS` 串台门 + 载荷形状二次校验的双防线**。第一道门在 helper 侧（`main.rs` 的 `foreign_hook_runner`）：Grok Build 等别家 runner 也读 claude/cursor 的全局配置，会把别家会话误报成 claude——每个 runner 导出的独有环境变量是唯一可靠判据，且空值不算命中。这道门会静默失效（`GROK_SESSION_ID` 被上游改名、停导出的先例就写在注释里），所以第二道门在宿主侧独立拦：`protocol.rs::parse_envelope` 的 claude 分支拒收 camelCase `hookEventName`、session_id 键名按 source 收紧（claude 绝不读 camelCase，否则会把不存在的会话交给 `claude --resume`）。新增 provider 时两处同步加判据，钉死测试（`foreign_runner_payload_is_rejected_even_if_the_env_gate_fails`）一并迁移。
4. **照抄：codex notify `--chain` 链式调用**（`main.rs::chain_notifier`）。codex 的 notify 单槽可能已被占（如 OpenAI 自家 computer-use notifier）：包装不驱逐，转发后 spawn 原程序带同一载荷；管道不可用也绝不抑制原 notifier。伴随的 #38 指数包装防线在宿主侧（下条）。
5. **照抄：命名管道服务器 + 内核进程身份核验**（`ai_hook/windows.rs::serve` / `spawn_gpui_server`）。`CreateNamedPipeW` + `PIPE_ACCESS_INBOUND`、每次连接一个全新管道实例（客户端竞态见微秒级失败由 helper 侧重试 20×5ms 兜住）、`GetNamedPipeClientProcessId` 必须在断开连接前读取——这是内核对「谁在写这条管道」的回答，载荷里自报的任何 pid 都可伪造。`event.agent_pid` 沿祖先链查（`process_tree::nearest_agent_ancestor`），用于区分同 pane 里主 agent 与 spawn 的子代理流。GPUI 形态是标准 mpsc channel（`spawn_gpui_server`），workspace 在 foreground executor 排水、按稳定 pane id 契约路由——slTerminal 单进程直接采纳此形态。
6. **照抄：`protocol.rs::parse_envelope` 信封解析与 provider 归一化**。`nebula-hook/1 source=<s> pane=<n>` 头 + 原样 JSON 体（helper 不重编码，所有 JSON 工作都在宿主侧、离开回合热路径）；按 source 路由到各家解析分支，归一成 `AiHookKind` 七态（`event.rs`：SessionStart / PromptSubmit / ToolComplete / TurnDone / NeedsAttention / SessionEnd）+ `AiTurnOutcome` 显式分类（Succeeded/Failed/Cancelled/Incomplete，只信 provider 结果元数据，绝不扫 assistant 散文判成败）。codex 双形态并存：legacy notify（`type=agent-turn-complete`）与 native hooks（`hook_event_name` + `codex_hooks=turns|full` 头字段）；native codex 有 `agent_id` 的子代理事件直接丢（与主会话共享 session_id，不能替主回合收尾）；native 形态的会话身份以 transcript 文件名换 rollout id（fork/换载后 thread 名不可信）。`AttentionContext` 全字段有界提取（消息 300 字符、id 512、原文 16KB）。
7. **照抄：`payload.rs` 有界提取、脱敏与可操作性判定**。三层容器回退取值（payload / context / payload 嵌套 / properties）+ 多拼写字段名兼容；`sensitive_context_key` 词表（token/secret/password/authorization/cookie/credential/apikey/environment 等）命中即 `[redacted]`；JSON 深度 6、数组 24、对象 48 的有界清洗。`attention_is_actionable` 的显式阻塞类型词表（permission/permission_request/tool_permission/approval_request/input_request/awaiting_input/elicitation_dialog 等）——不可操作的 Notification（idle/auth/result 提醒）直接丢、无类型的 legacy Notification 保留可解析但在生命周期里降权。`background_task_summary` 不再按 type 过滤、只看 status 终态词表（`TERMINAL_TASK_STATUSES`）——这是「后台 bash 还在跑就先弹完成通知」那个毛病的修复核心；`in_process_teammate` 的 `isIdle` 特判（上游 issue 锚定）只对 teammate 生效。终态词表来自本机 claude 二进制实测字面量，注释即知识载体，迁移时注释一起搬。
8. **照抄：`ordering.rs` 有界重排事件门**（`AiHookEventGate` / `GateVerdict` / `accept_for_pane` / `reorder_batch`）。按流键（source + session_id + pane + agent_pid + remote_process，pi 以 bridge_instance 代 session_id）分流，全进程共一扇门（pane id 进程生命周期内不复用，不为每个 view 留互相矛盾的缓存）；512 流上限 LRU 驱逐；七判全带原因（DuplicateEventId / StaleSequence / StaleTime / DuplicateFingerprint / AfterSessionEnd / UnorderedAfterDone / Accepted）——事件被静默丢掉是这套链路最难查的故障，「通知没出现」唯一的线索就是门原因，调用方必须把它落日志。序元数据优先级 bridge_sequence > occurred_at_ms > 无序号时指纹 + 1.5s 窗口；`reorder_batch` 只在同 pump 批次且全带序号时组内重排，跨批次旧序号由门拒绝。
9. **照抄：`lifecycle.rs::AgentActivity` 每 pane 仲裁器**。hook 事件是事实、屏幕匹配是有限权威的观察；命令所有权止于 shell 边界、绝不在输出安静时转让。`HookCoverage` 四级（None / Completion / Turns / Lifecycle）声明当前 hook 组合的实际能力，`allows_screen` 在 Lifecycle 覆盖时剥夺屏幕发言权；`command_finished` 显式 shell 边界同时拒绝旧 CLI 的迟到事件；Completion-only hook 的 `screen_armed` 锁存——完成结果必须等到 idle 屏幕或显式提交才与新回合分离；`Owner`（source/session/pid/bridge_instance）+ turn_id 区分嵌套子代理。TurnDone 时有在飞后台任务则流保持 Active 不置 Done。
10. **照抄：`event.rs::capabilities_for` 能力不对称显式声明**。`AiHookCapabilities` 六字段（lifecycle / attention_events / attention_context / background_tasks / bridge_sequence / serialized_delivery）按 source 给出——没有任何 provider 提供原生顺序字段（opencode/pi 的序号是自家 plugin/extension 注入的 `bridge_sequence`，名字里 bridge 而非 provider 正是这个原因），上层不能把「有生命周期 hook」误当「有权限上下文或顺序保证」。`serialized_delivery`（Bun 串行交付）是 Done 后无序 ToolComplete 能否翻案的判据之一。
11. **参考：远端 OSC 回退通道**（`main.rs` 的 `REMOTE_HOOK_TOKEN` / `base64_encode` / OSC 777 分支、`protocol.rs::parse_remote_envelope`）。令牌形状校验（32 位 hex）、64KB 上限、写 /dev/tty 的形态设计可参考其防御性，但 SSH 整支不采纳（下详），本地形态不保留此分支。

### 安装策略

12. **照抄：`installation.rs` 共享安装权威**。平台适配器供文件与命令，归属、provider 事件、feature 兼容只有这一处权威。三条核心纪律：codex 版本分流（`codex_mode`——features 不含 hooks → 不装；≥0.154.0 Full 八事件否则 Turns 三事件，0.154.0 是本机实测验证点而非猜的历史引入版本）；`merge_groups` 的 marker 语义（marker 记录我们装过的精确组，只按 marker 认领，绝不因命令含产品名就认领；用户编辑过的组报「edited … hook preserved」错而不覆盖；卸载恢复 marker 之前的样子）；`enable_codex_feature`（toml_edit 保注释与无关内容、显式 `hooks = false` opt-out 优先于自动安装、返回是否本次引入 features 键以便卸载精准还原）。
13. **照抄：`local/codex_notify.rs` + `installation/notify.rs`——notify 槽包装与 #38 防线**。notify 单槽被占时 `--chain` 包装不驱逐；toml_edit 保注释；首次改动留 `*.pebrel-bak`；幂等且自愈 helper 移动后的路径。`desired_codex_notify` 是承重件：与其他 notify 包装器互相包装会指数膨胀（#38 实证 130MB 撑爆 config.toml），所以 helper 标记出现在任何位置（含别家 JSON 序列化的 `--previous-notify` 参数内部）都算已接线、只有最外层是自己时才自愈路径、嵌套愈合深度 8 上限、序列化字节预算 8KB 超限拒写。
14. **照抄：`local/codex_hooks.rs`——能力探针 + marker 归属**。真跑 `codex` 探测 hooks feature（3s 限时、隐藏窗口、输出落临时文件），不认广告信口；`hooks.pebrel-managed.json` marker 记录所装组；配置 1MB 上限；原子写。探针失败一律退回 legacy notify，宁缺毋滥。
15. **照抄：`local.rs::ensure_claude_hooks` 安装纪律**。目录不存在不 scaffold（无该 CLI 足迹就不往它的配置里写东西）；helper 路径拿不到不动手；原子文件锁；非法 JSON 绝不「修复」式覆盖（并发改写中的文件留给 watcher 下轮/boot pass 再试）；幂等合并只认领完整自有命令（`is_helper_executable` / `is_helper_shell_command`——echo、管道、追加命令仍属于用户）；`CLAUDE_EVENTS` 全事件注入。
16. **照抄：`local/kimi.rs` kimi 安装器**。kimi 官方硬约束：hook 条目只允许 event/matcher/command/timeout 四字段，多写一个整个 config.toml 加载失败——每条只写三键、省略 matcher；command 是 shell 命令字符串，helper 路径必须双引号包裹（安装路径含空格会被 shell 切开，#80 同源教训）；订阅八事件（SessionStart/UserPromptSubmit/Stop/Interrupt/StopFailure/PermissionRequest/PermissionResult/SessionEnd），心跳与后台 Notification 不订；timeout 与 claude 同值；与 claude 同一套纪律（幂等/自愈/备份/拒写/不 scaffold）。
17. **照抄：`local/cursor.rs` + `native_events.rs` + `local/extended.rs` 原生事件族**。cursor 落 `~/.cursor/hooks.json` 共享 JSON，五事件映射（sessionStart/sessionEnd/beforeSubmitPrompt/stop/afterShellExecution → 归一事件名）；自有命令识别兼容 PowerShell EncodedCommand 形态（base64 解码回 UTF-16 校验）；copilot/grok/omp 经 `extended.rs` 同一形态。cursor 提交前 hook 有响应合同——helper 在 `main.rs` 对 `cursor --event prompt` 无条件回写 `{"continue":true}`，传输失败也不能阻止用户提交。
18. **照抄：`bridges.rs` + `res/hooks/opencode.js` / `res/hooks/pi.ts` 插件落盘桥**。opencode 是 Bun 应用，自动加载插件目录 JS；插件订阅其事件总线、shell 出到 helper（路径取 `PEBREL_HOOK_EXE` 环境、管道继承），归一成 `{"kind":...}` 小载荷——宿主侧与 opencode 持续演进的 SDK schema 完全解耦。Bun 等上一个 helper 关闭再启下一个，天然串行交付（`serialized_delivery` 的来源）；3s 看门狗防挂死 helper 吞掉包括最终 idle 在内的全部后续事件。pi 官方扩展 API 的 agent_start/agent_end 提供稳定回合边界，fire-and-forget，无 `HOOK_EXE` 环境时完全静默——全局安装不影响从其他终端启动的 pi。
19. **照抄：`local/managed_files.rs` 归属指纹**。SHA-256 指纹 + `.pebrel-managed` marker + legacy hashes 三通道判归属；安装/检视/卸载共用同一判断，设置页不能把同名的用户文件显示成已接入；被用户编辑过的同名文件报 Conflict 保留不动。
20. **照抄：`local/config_guard.rs` 配置守护**。安装/修复调度线程（notify 监听 + 唤醒 channel + Drop 停止），`heal_all` 拿锁后重读授权——不能把菜单/卸载刚移除的 hook 按旧快照装回；`ai_hooks = false` 全局开关短路；runtime skills 安装结果分级（Installed/Current/Conflict）记日志。
21. **照抄：`integrations.rs::HookInspection` + `local/settings.rs::inspect` 检视八字段**。config_path / available / installed / needs_repair / enabled / helper_missing / error——installed 与 current 是两个事实分离（装了 ≠ 装的是当前版本）：`installed_at` 认 marker 语义，`current_at` 做写入验证比对，部分残留的旧配置保持可见为 installed 但过不了 current 判为 needs_repair；installed 强制 enabled 可见（上轮卸载可能失败，或别家工具写过）；helper_missing 单独暴露。
22. **照抄：`local.rs::announce` + `claim_setup_announcement` 一次性告知**。首次成功接线后发一次系统 toast（「AI hooks 已接入」语义），`ai-hooks-announced` 哨兵文件用 `create_new` 原子占位保证只报一次，失败仅 debug 不影响安装。
23. **参考：`local/runtime_skills.rs` 运行时 skills 落盘**。与 hook 安装伴生的 claude skills 投放，属于 AI assistant 能力面（分片 08），此处仅登记边界：安装调度已含其位（config_guard 的 heal_all），assistant 细节不归本分片。

### provider 识别

24. **照抄：`ai_agents.rs::AgentKind` 27 家枚举全家**。`ALL` / `slug` / `display_name` / `aliases` / `parse` 一套齐全；`aliases` 是进程识别与设置页可执行文件发现共用的同一组官方/兼容名称（cursor 保留历史标签 `agent`/`cursor`/`cursor-agent` 以归一旧进程快照）。27 家清单全保留不裁剪，改名映射：`nebula_*` → `slterm_*`，display 名里的产品名同步替换。
25. **照抄：`integrations.rs::executable_directories` + `find_executable` 可执行文件发现**。PATH + 家目录下 `~/.local/bin` / `.cargo/bin` / `.bun/bin` / `.grok/bin` + `%APPDATA%\npm` + `%LOCALAPPDATA%\cursor-agent` 的扩展目录集；Windows 扩展名序 `.exe/.cmd/.ps1/无`；「agent」这类通用文件名的按安装路径归属（canonicalize 后查路径分量：`.grok/bin/agent` 不能误报成 Cursor，反之亦然；grok 专属链 `agent` 别名仅 grok 追加）；桌面编辑器的 `cursor` 启动器不能证明 Cursor Agent CLI 已装（别名与路径双判据）。macOS 专属目录分支随多平台砍掉，只留 Windows 相关项。
26. **照抄：`integrations.rs::AGENTS` 9 家一等集成数组 + `hook_for` 映射**。claude / codex / opencode / cursor / kimi / pi / omp / copilot / grok 九家一等保留不裁剪；其余 18 家走屏幕观察 + 进程识别（无 hook 能力）。

### 屏幕观察

27. **照抄：`agent_detection/*.toml` 声明式规则族**。21 个厂商清单 + `_shared.toml` 兜底（无完整生命周期 hook 的客户端共享键盘 chrome 规则）；规则字段 `id` / `state`（idle/working/blocked）/ `priority` / `region`（`bottom_non_empty_lines(n)` / `after_last_horizontal_rule`）/ `contains` / `line_regex` / `any` / `not`，AND 语义、not 否定、`any` 析取。TOML 进资源文件随仓分发，改规则不动代码。
28. **照抄：claude.toml 注释承载的实测知识**。「esc to interrupt 是回合进行中唯一可靠证据」（Claude Code 只在真正可中断时打印，回合一结束整行消失）；「动画符号（·✢✳✶✻✽）不能当证据」——回合结束的总结行用同一组符号；「看见 ❯ 不等于空闲」——输入框在回合进行中同样可见（允许排队输入），需 not 条件与更高优先级规则共同仲裁。这些注释是反复踩坑的沉淀，迁移时逐字保留。
29. **照抄：`ai_agents.rs` 的用户覆盖机制**。`%APPDATA%\<产品>\agent-detection\<slug>.toml` 覆盖单个清单（或设置目录等价物）；mtime 检查最多 2s 一次——规则可以在客户端运行中调参，但文件系统工作不上每一帧；覆盖按 slug 一对一。缓存形态（`OnceLock`/`RwLock`）照结构迁。
30. **照抄：`ai_agents/screen_context.rs` 结构判据**。`has_live_input_controls`：assistant 正文里的字不是事件，只有最近的控制行说了算（ quoted 旧表单压不过新 footer）；`CONTROLS` / `BINARY` 两条正则给所有 bundled 与用户规则共享的结构边界；`attention_region` 对 codex 的 composer 特判（草稿行与编号选择行同字形，从最后的空提示符起算）。「blocked 候选必须先过 live-control/form 边界」是所有规则的公共闸。

### 会话发现 / 恢复

31. **照抄：`ai_sessions.rs` 有界扫描全家**。只读文件头部 64KB（`HEAD_BYTES`——会话 jsonl 动辄几十 MB，整读把面板卡成秒级；标题几乎总在最前：claude 压缩会话第一行是 summary、新会话首条 user 消息，codex 第一行 session_meta 带 cwd）；`scan_claude`（`~/.claude/projects/<编码目录>/<uuid>.jsonl`）+ `scan_codex`（`~/.codex/sessions` 年/月/日三层手写栈递归，深度封顶）；codex 会话 id 取 rollout 文件名 uuid 尾（时间戳里的数字段不能误当 id）；标题提取 `claude_title`（summary 行优先，否则首条真人 user 消息）与 `codex_details`（session_meta cwd + 首条 input_text）；`looks_injected` 形态筛（`<` 开头 / `Caveat:` / `# AGENTS.md` / `# CLAUDE.md` / `[Request interrupted`）+ `isMeta` 标记——注入块挂在 user 名下，用户截图实证的一排「AGENTS.md instructions」标题事故由它防。
32. **照抄：`ai_sessions.rs` hook 注册表合并**。`record_hook_session` upsert 到 `ai_sessions.json`（原子写 + 文件锁、容量上限截断、版本字段）——Nebula 只记「如何重新找到会话」，不复制对话正文；`scan` 把 CLI 原生档案扫描与 hook 注册表按 (source,id) 合并去重、取较新 modified、补齐缺失字段；id 校验复用 resume/fork 命令构造器做不可信 id 过滤器。`relative_label` 相对时间标签（刚刚/N 分钟前/N 小时前/N 天前）一并保留。
33. **照抄：`assistant_answer.rs::AssistantAnswer` 回答提取三态**。`Complete`（Arc 共享、128KB 上限内）/ `Missing` / `TooLarge { bytes }`；`from_hook` 按 source + 事件名取对应字段（claude Stop 的 `last_assistant_message`、codex legacy 的 kebab-case 同名、native Stop 同名字段）；`notice` 文案明示「不从屏幕猜测、请在终端查看」——回答原文缺失宁可留终端内容也不扫屏反推。`AnswerInbox` 按 (provider, session) 归属 + received_sequence 单调性去旧。
34. **照抄：`assistant_answer/conversation.rs` 有界 transcript 投影**。头部 256KB capture + 分页预算（页 1MB、单消息 48KB、输出 128KB、消息数 160）；`validate_source` 校验身份与路径一致后才投影；截断标记 `truncated`/`complete` 显式携带。原则一句话：无历史目录扫描、无屏幕到消息的猜测。
35. **参考：`ai_sessions.rs::AiSession::resume_command / fork_command` + `place_label`**。claude `--resume <id>` / `--resume <id> --fork-session`、codex `resume`/`fork` 的语法分散在 `AgentKind` 各实现里；claude 项目目录名是不可逆编码（分隔符与 `_` 都压成 `-`），位置词取末段尾缀。命令生成属于各 CLI 合法领地，形态参考。

### per-pane 环境契约

36. **照抄：`agent_env.rs::apply` 身份变量组**。`TERM_PROGRAM` / `TERM_PROGRAM_VERSION`（生态事实标准入口，第三方既有识别逻辑零改动）；`PANE_ID`（per-pane 身份）；`CLI` / `BIN_DIR` 绝对路径（便携版不一定在 PATH 上，Agent 要的是「现在就能执行」）；`BIN_DIR` 前置 PATH。`executable()` 优先 `current_exe()` 而非继承值——嵌套启动（套娃 Nebula / wsl / 测试隔离实例）时继承值指向外层旧副本，PTY 必须指向正在为自己提供控制面的那个进程。
37. **照抄：幂等判重三件套**。PATH 前置按值判重（先移除所有等值项再前置，已在首位不重写——否则每层嵌套环境块都涨一份；Windows 路径大小写不敏感归一）；`WSLENV` 合并按变量名判重、尊重宿主已有的标志位选择（`/p` 路径翻译让 WSL 把 `D:\…` 翻成 `/mnt/...`，`/l` 列表标志不覆盖）、保留两个来源（本次环境表现值与宿主进程值）的全部既有条目、`COLORTERM` 透传保真彩；变量组只注本地 PTY。WSLENV 字面量条目与变量名一致由编译测试（`wslenv_entries_match_variables`）钉死——改名即红。
38. **参考：`agent_env.rs::PROCESS_ENV`（宿主 PID + pane id 双因子防同机两实例 pane 混淆）与 `runtime_api::ENDPOINT_ENV`（控制面端点）**。机制本身合理，slTerminal 是否保留端点形态随 runtime API 设计定（方向性，不归本分片裁决）。
39. **参考→不抄：`PEBREL_*` / `NEBULA_*` 双名并写兼容层**（`main.rs::aliased_env`、各处 `LEGACY_*_ENV` 常量、managed_files 的 legacy hashes）。pebrel 有历史用户要迁移，slTerminal 是全新 fork——直接 `SLTERM_*` 单名，双写与旧名回读全部砍掉（进「不采纳点」）。

### 托盘 attention 态与通知交界（接口面，细节归分片 09）

40. **照抄：`notify.rs::Notification` 枚举形态**。「新来源 = 新变体、新输出 = deliver 里新一行」的加法扩展合同——AI-CLI 特有 hook 落地不需要重接通知骨架；投递纪律（一次性线程跑 toast RPC、panic 杀线程不杀终端、best-effort 每次失败降级为日志行、全局节流防 bell 狂魔后台任务）一并照抄为分片 09 的输入。生命周期侧的来源就是本分片的 `AgentActivity` 状态边：TurnDone（有界回答 + outcome）与 NeedsAttention（attention 上下文）是 toast/任务栏闪烁的两个触发源；`display/window.rs` 的 attention 闪烁接口由生命周期状态驱动。本分片只保证这两个状态边干净、带上下文、可解释（门原因落日志）。

### slTerminal 侧现状对照（归宿裁定）

41. **照抄关系裁定：`src-tauri/src/hooks/` 信号文件通道整体退役**。slTerminal 现状是 Node reporter 脚本（`slterm-hook-reporter.js` 落盘 + `~/.slterminal/hooks-events/` 信号文件 + notify + 3s 轮询双通道 watcher + `agent-event` Tauri 事件广播）。pebrel 方案里这一层整体不存在——hook 载荷走命名管道直进宿主，无中间文件、无 Node 依赖、无轮询兜底。裁定：三层拓扑落地后信号文件通道退役；随之退役的还有 watcher 双通道（notify 静默失效风险在管道直连形态下不复存在——管道读失败即 helper 侧 `PipeUnavailable`，宿主不存在即 NotHosted，失败语义显式）。reporter 的 C10 合同（任何路径 exit 0、不写 stderr）精神已由 helper 的 catch_unwind + 超时等待继承。
42. **参考：settings.json 注入/statusline 桥接的处置**。slTerminal 现有 claude 注入（10 事件 settings.json merge、`SLTERM_PANEL_ID` 路由、注入三态 Injected/Outdated/NotInjected）与 pebrel `ensure_claude_hooks` 高度同构，迁移时以 pebrel 安装纪律（marker 认领/编辑保留/不修复式覆盖/原子锁）为终态重设计，废弃自研 merge 逻辑；可疑 statusline 原命令审查（SEC-12 pendingConfirmation 二次确认）是 slTerminal 独有且合理的安全门，pebrel 无对应物——保留并接入新安装器（参考而非照抄）。statusline 桥接（`used_percentage` 官方口径、节流原子写信号文件）是 context 用量唯一来源、`ai_hook` 能力面不含此信息，属互补通道——信号文件介质是否保留待实现期裁定（方向：用量信号与 hook 事件分通道，不混入管道载荷）。
43. **参考：`agent_dirs.rs` / `agent_history/` / `agentFiles/` / `agentHistory` / `cliProfiles` 归宿**。`agent_dirs.rs` 的 cliId → home 目录静态表（ADR-0024 沙箱放行真值源）扩到 27 家清单，与 `agent_detection/*.toml` 的身份域分工：目录表管沙箱放行、AgentKind 管身份、TOML 管屏幕规则。`agent_history`（扫描缓存 + 指纹失效 + SEC-05 校验前置）被 `ai_sessions.rs` 形态吸收：头部有界读 + hook 注册表合并取代全量扫描缓存，resume/fork 命令构造统一进 `AgentKind`；标题回退链两侧同源。前端 `cliProfileRegistry` 的 OSC 133 首 token 命中（页签标题/logo）与 AgentKind 的 27 家清单是同一身份事实的两份枚举——新世界身份事实单源在 `AgentKind`，前端注册表退化为纯 UI 表现层（icon/tabTitle/编辑器挂点），逐家对齐。`agentHistory` 前端的恢复注入（restoreSession）消费 `resume_command` 产物，链路保留、数据源换。

## 不采纳点

1. **`ai_hook/remote/` 整支**（`remote.rs` 的 Snapshot / FILE_ADAPTER / remote_bridge.py / remote_shell.py / TOKEN_ENV、`remote/tests.rs`、`protocol.rs::parse_remote_envelope`、helper 侧 REMOTE_HOOK_TOKEN + OSC 777 分支、`agent_env.rs` 里 SSH pane 的 `NEBULA_PANE_REMOTE` 语义）。理由：已定不采纳 SSH/一切远程；本地单实例无远端宿主，所有「远端会话提交事件、本地 PTY 通道覆盖 pane 身份」的护栏无对象。helper 的远端分支、`base64_encode`、令牌校验随之整删。
2. **`ai_hook/unix.rs` + `send_local` 的 unix 分支 + `remote.rs::quote`**。理由：多平台砍掉，只保留 Windows 命名管道形态；unix socket 连接、ack 握手、POSIX 引号全删。
3. **`PEBREL_*` / `NEBULA_*` 双名并写兼容层**（`main.rs::aliased_env`、`ai_hook.rs` 的 `LEGACY_PIPE_ENV` / `LEGACY_PANE_ENV` / `LEGACY_HOOK_EXE_ENV`、`agent_env.rs` 的 `LEGACY_CLI_ENV` / `LEGACY_BIN_DIR_ENV`、managed_files 的 legacy hashes、`environment_migration_retains_scope_and_prefers_current_names` 迁移测试）。理由：pebrel 的双写是为已装历史用户过渡；slTerminal 全新 fork 无迁移对象，单名 `SLTERM_*` 直落，兼容层是纯债。
4. **legacy-shell feature 通道**（`windows.rs::spawn_server` 经 winit EventLoopProxy 的投递、`event.rs` 的 EventType::AiHook 变体）。理由：slTerminal 走 GPUI 单进程，事件循环只有一个；只保留 `spawn_gpui_server` 的 mpsc channel 形态。
5. **可执行文件发现里的 macOS 目录分支**（`/opt/homebrew/bin`、`/usr/local/bin`）。理由：Windows-only 定位，随多平台一并砍。
6. **`runtime_skills.rs` 与 `ai_assistant` 细节**。理由：已定归分片 08，本分片只保留安装调度的挂点位置（config_guard heal_all），不展开。
7. **slTerminal 自有的信号文件通道**（`hooks/watcher.rs` 双通道、`hooks/signal.rs`、`~/.slterminal/hooks-events/`、reporter/statusline 落盘形态）。理由：不是 pebrel 侧资产而是被替代项——管道直连后文件中介失去存在理由；其中 SEC-12 审查与 statusline 用量桥接按上列参考条目处置，其余退役。

## 优化方向

- **模块落位**：pebrel 侧整体平移为 `slterm_hook` crate（纯 std 单文件二进制）+ `slterm_app/src/ai_hook/` 目录树（`windows.rs` / `protocol.rs` / `payload.rs` / `ordering.rs` / `lifecycle.rs` / `event.rs` / `installation.rs` / `local/*` / `native_events.rs` / `bridges.rs` + `res/hooks/` 插件资产）+ `agent_detection/*.toml` + `ai_agents.rs` + `ai_agents/screen_context.rs` + `ai_sessions.rs` + `assistant_answer/` + `agent_env.rs`。标识改名一次做净：`nebula_*`/`pebrel-hook` → `slterm_*`/`slterm-hook`，信封头 `nebula-hook/1` → `slterm-hook/1`，环境变量全组 → `SLTERM_*`（含 `TERM_PROGRAM=slterm`、管道名 `\\.\pipe\slterm-notify-<pid>`、managed marker 后缀、备份文件后缀），27 家 display 名里的产品名同步替换。
- **slTerminal 现有 hooks 机制的终态**：三层拓扑落地后，hook 事件以命名管道为唯一权威通道，信号文件通道退役；claude 注入器按 pebrel 安装纪律重设计（marker 认领、编辑保留、不修复式覆盖），slTerminal 独有的 SEC-12 审查门与新安装器合流；statusline 用量桥接与 hook 事件分通道保留。设置中心 Agent 组 hooks 页（ADR-0023）的数据源从注入三态切换为 HookInspection 八字段（installed / needs_repair / helper_missing 三态检视 + config_path 直达）。
- **身份事实单源**：27 家清单落在 Rust `AgentKind` 为唯一真值；前端 `cliProfileRegistry` 退化为 UI 表现注册表（icon/tabTitle/编辑器挂点），现存「两侧各自枚举同一 agent 集」的刻意冗余收敛为单侧枚举 + 消费。OSC 133 首 token 命中与 `agent_detection` 屏幕规则并存：前者是命令行静态识别、后者是运行态屏幕观察，两者经 AgentActivity 仲裁，不互相推导。
- **会话数据的统一**：`agent_history` 全量扫描 + 指纹缓存被 `ai_sessions` 头部有界读 + hook 注册表合并形态取代；`AgentHistorySession` DTO 与 `AiSession` 字段对齐（cwd 从内容解析、标题回退链同源）；resume/fork 命令构造统一进 `AgentKind`，前端恢复注入链路消费统一产物。
- **pane 生命周期挂点**：`AgentActivity` 每 pane 一个，按稳定 pane id 键挂——与 slTerminal 现有「会话元数据单点（TerminalRegistry）」纪律的关系是：PTY 进程映射仍归终端注册表，agent 活动状态归 AgentActivity，两者以 pane id 关联；GPUI 侧由 workspace 在 foreground executor 排水管道事件并按 pane id 路由。屏幕观察采样走同一仲裁器，hook 覆盖高时自动降权。
- **可解释性纪律内化**：门原因（GateVerdict）落应用日志、helper 侧信道日志（`SLTERM_HOOK_LOG`，绝不记载荷）随移植重建，「通知没出现」类问题两侧都有取证面；事件被静默丢弃的每一处都必须带原因，这条作为模块约定写进 `ai_hook` 的 CLAUDE.md，不靠代码自觉。
- **测试迁移**：pebrel 侧 ai_hook / ai_agents / ai_sessions / agent_env 的既有测试用例全量随迁改名（串台双门、事件门七判、安装 marker、WSLENV 编译钉、标题注入筛、后台任务终态词表等承重用例一个不落），替换 slTerminal 侧信号通道测试；bugfix 防复发测试纪律（TQ 红线）按本仓既有四级金字塔归位。
