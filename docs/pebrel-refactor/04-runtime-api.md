# 分片 04:Runtime 控制 API + 单实例移交

## 优化面

slTerminal 目前没有任何外部控制面。pane 里的 claude 进程无法回控终端:不能开 tab、不能分屏、不能读别的 pane 输出、不能派任务、不能等状态。现有对外通道只有两层,都服务于前端自身:

- **Tauri 命令面**(`src-tauri/src/lib.rs` 的 `generate_handler!`):pty_spawn/pty_write/pty_resize/pty_kill、git_status/git_diff/git_rollback/git_unstage、hooks 注入族、agent_history 族、background_tasks、notify、settings/projects/plan_balance。这批命令从 WebView 发起,只能被自家前端 `invoke`,pane 里的 AI CLI 够不到,也没有版本化协议、没有快照/revision、没有等待语义。
- **env 注入**(`src-tauri/src/pty/spawn.rs` 的 PTY spawn 注入点):只有 `TERM_PROGRAM=slTerminal` 和 `SLTERM_PANEL_ID`(供 hooks 信号文件标记事件来源)。没有 CLI 路径注入、没有控制面端点注入,pane 里的进程连"控制面在哪"都无从得知。

与 pebrel 对照,缺的东西成体系:无版本化 JSON Lines 协议、无单一状态权威(snapshot + 单调 revision)、无 `state_change_seq`/`after_seq` 等待基线、无 generation 绑定、无资源+动词 CLI、无 SKILL.md 自发现、无单实例机制(全仓搜不到 single instance 任何痕迹)、无事务型 worktree 准备。

本片的裁定背景:全局决策已定为 Runtime 控制 API 整体照抄,mux 驻留保活语义砍掉、仅保留单实例移交,SSH/mobile/Lua/多平台不采纳,`nebula_*` 一律改名 `slterm_*`。本节列出的是在此框架下的功能点粒度裁定。

## 采纳点

### 传输与发现(照抄)

1. **loopback TCP + 端口文件发现,照抄。** 服务端只监听 `127.0.0.1`,发现文件 `runtime.port` 内容为 `<port> <128bit-token> <protocol_version>`,token 由两个 OS 种子 RandomState 生成,只挡其他本机用户、不承诺抵御同用户攻击者。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `RuntimeServer::spawn_at`、`fresh_token`、`port_file`、`Endpoint`、`parse_endpoint`;token 生成同款另见 `nebula_app/src/mux.rs` 的 `fresh_token`。
2. **owner 锁 + 原子写端口文件,照抄。** 启动时先拿 `LifetimeFileLock` 再 PING 既有 endpoint,两进程不抢文件;写文件走原子写,Drop 时仅当文件内容仍等于自己的 endpoint 才删除(不删别进程的新记录)。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `RuntimeServer::spawn_at`、`Drop`、`nebula_app/src/atomic_file.rs`。
3. **JSON Lines 带版本信封,照抄。** 请求 `{protocol, version, id, token, method, params}`、响应 `{protocol, version, id, ok, result|error}`、事件 `{protocol, version, event, revision, data}`,全部 `deny_unknown_fields`;协议名/版本不匹配返回 `protocol_version_mismatch` 并带 `supported_versions`。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `ApiRequest`、`ApiResponse`、`ApiEvent`、`ApiError`、`PROTOCOL_NAME`、`PROTOCOL_VERSION`、`SUPPORTED_VERSIONS`;`docs/runtime-api-v1.schema.json` 全文作协议 Schema 蓝本(protocol 常量改名 `slterm.runtime`)。
4. **单连接单请求 + 读超时 + 请求体上限,照抄。** 每连接一个 JSON 行请求,读超时不限大小直接拒(`request_too_large`);客户端侧 connect/read/write 全部带超时,`shutdown(Write)` 后读完整响应行,响应身份(id/protocol/version/ok/result 互斥)逐字段校验。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `handle_connection`;`nebula_app/src/runtime_api/cli.rs` 的 `client_stream`、`decode_plugin_response`。
5. **客户端上限与线程模型,照抄。** accept 循环按 `MAX_CLIENTS` 计数、每连接一线程、线程名可辨。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `serve`、`ActiveClient`。

### 状态权威与等待语义(照抄)

6. **RuntimeHub 单一状态权威 + 单调 revision,照抄。** GUI、CLI、外部客户端读同一份 snapshot;语义内容不变不加 revision、不重发事件。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimeHub`、`RuntimeSnapshot`、`RuntimeWindow`、`RuntimeTab`、`RuntimePane`、`RuntimeLayout`。
7. **`state_change_seq` 单调计数 + `after_seq` 等待基线,照抄。** 每 pane 只在 task_state 真正跃迁时 +1;先取基线再等待跃迁,服务端只承认 `seq > after_seq`,堵死"提交前的 idle 被当成已完成"的经典竞态。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimePane::state_change_seq`;`nebula_app/src/runtime_api/command.rs` 的 `WaitParams`、`wait_matches`;`nebula_app/src/runtime_api/transport.rs` 的 `wait_connection`(超时 details 带 `after_seq` 与 `observed_state_change_seq`,区分"一直没动"与"跃迁了但没到目标态")。
8. **提交时同步建立 running 边沿,照抄语义。** 提交 Enter 的同步动作里先建立 `running` 边沿、再由真实 shell/hook 结束事件归位,使 `--wait` 即使落在 runtime pump 两拍之间也能观察到提交后的新 `state_change_seq`。pebrel 侧:设计说明在 `docs/runtime-control-api.md` 的「等待语义与 state_change_seq」一节,落点对应 GPUI 壳的 prompt 提交通路。
9. **generation 绑定,照抄。** `agent.*` 路径全部带 `generation`,Codex/claude 退出重开后不会把任务投给新会话;等待遇 `agent_exited`/`agent_replaced`/`agent_identity_mismatch` 明确失败,不允许静默换目标。pane 路径无此保护,用两个资源名把这条边界摆在命令行上。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimeAgent`、`RuntimeManagedAgent`、`RuntimePaneLifecycleKind`;`nebula_app/src/runtime_api/agent_api.rs`。
10. **`--wait` 基线取提交后快照,照抄。** 发送/粘贴命令在写入前校验等待超时;提交响应缺有效非零基线时明确报错,不降级为无基线等待;`--no-submit` 与 `--wait` 参数解析直接拒绝。超时只代表未确认结果,不代表输入未送达,错误语义走 `submission_outcome_unknown`。pebrel 侧:`docs/runtime-control-api.md` 的 CLI 一节;`nebula_app/src/runtime_api/cli.rs` 的 `request_once`。
11. **wait 状态机投影单一化,照抄。** 侧栏、托盘、snapshot、`agents.list`、`pane.wait` 共用同一个 `RuntimeTaskState` 投影,优先级 失败>attention>等待输入>运行>完成>空闲,外部客户端不解析标题猜生命周期。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimeTaskState`;`docs/runtime-control-api.md` 的「Agent 状态与终端读取」一节。

### 执行模型(照抄)

12. **所有写操作进 UI owner 线程,照抄。** transport worker 只校验、等待、序列化;窗口/Tab/PTY 写操作经 `RuntimeDispatch`/`RuntimeCallback` 派发进 owner 线程,事件循环不可用时返回 `runtime_unavailable`,不伪造成功。普通请求与编排步骤共享同一派发原语,避免 worktree 事务、run 等待、UI 超时的语义分叉。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimeDispatch`、`RuntimeCallback`、`EventSink`;`nebula_app/src/runtime_api/transport.rs` 的 `dispatch_runtime_command`。
13. **派发超时与 worktree 未知态保留,照抄。** UI 派发超时返回 `runtime_timeout`;worktree 已创建但结果未知时 `cleanup_deferred=true` 并保留 checkout,避免晚到 PTY 用刚被删的 cwd。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `dispatch_runtime_command`;`nebula_app/src/runtime_api/agent_api.rs` 的 `rollback_prepared_worktree`。

### 能力面(照抄)

14. **`runtime.describe`/`runtime.snapshot`/`events.subscribe`,照抄。** describe 回报应用版本、协议版本、capabilities、features、env 契约、命令清单;subscribe 第一行确认、随后每行一个 snapshot 事件,支持 `since_revision` 断点续传。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `runtime_description`、`subscribe_connection`。
15. **features 探测串,照抄。** 附加参数能力(如 `pane.wait.after_seq`)不可从 capabilities 探测——旧版静默忽略未知参数仍带竞态,严格客户端应检查 feature 串;`runtime.describe` 的 env 段让客户端探测环境契约而不硬编码变量名。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `runtime_description`。
16. **pane 方法族,照抄。** `pane.list/read/send/paste/wait/exec/zoom/resize/prompt/send_key/close/split`。prompt 拒换行与控制字符、限 32 KiB;paste 只收 UTF-8、拒 ESC/NUL、要求 bracketed paste;send_key 只开放命名键白名单、字母必须配 control、repeat 上限;zoom 幂等显式值不用 toggle;resize 设直接父分屏占比(0.05–0.95)。pebrel 侧:`nebula_app/src/runtime_api/command.rs` 的 `validate_prompt`、`validate_paste_text`、`validate_command_line`;`nebula_app/src/runtime_api.rs` 的 `RuntimeKey`、`MAX_KEY_REPEAT`、`MIN_PANE_RATIO`/`MAX_PANE_RATIO`。
17. **`pane.read` 直读真实 Grid 尾部,照抄。** 锁定 pane 的 Term 经 `bounds_to_string` 读 scrollback,不截图、不动用户滚动位置/选区/光标;范围锚定 buffer 底部;返回 requested/returned/history_available/truncated/task_state/exited。pebrel 侧:`nebula_app/src/runtime_api/terminal_read.rs` 的 `RuntimePaneRead`、`capture_terminal_tail`。
18. **`pane.run` 依赖 OSC 133 真实 exit code,照抄。** 只认 `CommandDone` 携带的 exit code;无集成时返回 `exit_code_unavailable`/`run_start_timeout`/`run_aborted`,绝不把未知结果伪造成 0。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `RuntimeRunOutcome`、`ExitCodeCapability`、`RUN_START_GRACE` 相关等待逻辑。
19. **`pane.exec` 独立非 TTY 子进程,照抄。** 直接收 argv 无 shell 展开,在 pane 冻结的 cwd/环境上下文里起 child,不进 Grid/history;stdout/stderr 并行排水、上限可配、超时回收整个进程树并保留已捕获输出与 `timed_out`。pebrel 侧:`nebula_app/src/runtime_exec.rs` 的 `PaneExecContext`、`ExecLocation` 的 Host 分支;命令派发点 `nebula_app/src/runtime_api/transport.rs` 的 `dispatch_connection`(Exec 分支)。
20. **`runtime.orchestrate` 单请求编排,照抄。** 强类型 op 步骤面(new_tab/focus/split/prompt/run/agent_launch/wait),步骤引用只允许 `{step, field: "pane_id"}` 的结构化回指,未来引用/重复 ID/未知字段在任何 UI 动作前被拒;先并行启动全部 Agent 再等 ready 后投首任务,receipt 只带必要字段,中途失败保留已完成动作、从失败步续作。pebrel 侧:`nebula_app/src/runtime_api/orchestrate.rs`;`docs/runtime-api-v1.schema.json` 的 `orchestrate_*` 定义族。
21. **agent 方法族,照抄。** `agents.list/agent.start/agent.fork/agent.get/agent.delegate/agent.prompt/agent.paste/agent.read/agent.wait`;冷启动 kind 白名单只放经官方文档核实的 CLI;`agent.fork` 事务顺序为 解析 cwd→校验 dirty/base/branch/path→建 branch+worktree→交真实 Tab/PTY 链;`agent.delegate` 回传只认同 pane 同 generation、每目标单在途委派、`worker_output` 按不可信数据处理。pebrel 侧:`nebula_app/src/runtime_api/agent_api.rs`;`nebula_app/src/runtime_api.rs` 的 `RuntimeDelegationEndpoint`、`RuntimeDelegationReceipt`。
22. **git 方法族的 revision 防陈旧校验形态,参考。** `git.status/diff/history/stage/unstage/commit/fetch/pull/push` 写操作要求 `expected_cwd` + 服务端重查 repo root + 写前 revision 校验(outdated → `git_stale`),路径参数永不解释为 shell 命令或 glob。slTerminal 已有 git_status/git_diff 等 Tauri 命令(`src-tauri/src/git/`),控制面暴露时按此校验形态收口,不整族照搬为独立协议面。pebrel 侧:`nebula_app/src/runtime_api/git.rs`;`docs/runtime-api-v1.schema.json` 的 `git_params`。
23. **tab 方法族,照抄(窗口语义收编)。** `tab.new/close/rename/move`;rename 空值恢复生成标题、控制字符模式拒;tab 定位同时支持窗口内零基索引与稳定 `tab_id` 身份。pebrel 侧:`nebula_app/src/runtime_api/tabs.rs`;`docs/runtime-api-v1.schema.json` 的 `tab_*` 定义族。
24. **window.close 语义,照抄(单窗口收编)。** 关空闲窗口、忙碌 pane 返回显式确认错误,不静默强杀。pebrel 侧:`docs/runtime-control-api.md` 方法表 `window.close`;单窗口定位下收编为"关当前唯一窗口"动作。pebrel 侧:`nebula_app/src/runtime_api/command.rs`。

### CLI 与发现层(照抄)

25. **资源+动词薄别名 CLI,照抄。** `slterm pane send 17 "cargo test" --wait`、`slterm agent send codex "..." --wait`、`slterm pane read/paste/wait/exec/zoom/resize`、`slterm agent list/send/delegate/paste/read/wait`、`slterm window close`、`slterm tab rename/move`——薄别名与完整协议共用请求函数、响应信封、generation 与 after_seq 竞态保护,pane 用数字 id、agent 用名字/稳定 id。pebrel 侧:`nebula_app/src/cli.rs` 的 verb 解析与 `ControlCommand`;`nebula_app/src/runtime_api/cli.rs`;`nebula_app/src/runtime_api/shortcuts.rs`。
26. **`slterm env` 离线可用的发现命令,照抄。** 不依赖控制面即可返回身份、CLI 路径、控制面可达性、自身 pane cwd/branch/agent、完整命令清单且每条给可直接复制执行的样例;环境缺什么如实报告,不在"没连上"时整体失败。pebrel 侧:`docs/runtime-control-api.md` 的 CLI 一节;`nebula_app/src/cli.rs` 的 env 命令实现。
27. **SKILL.md 自发现机制,照抄。** 打包 `docs/skills/slterm-runtime/SKILL.md`,教 AI CLI 用 `slterm env` 定向后直接调 Runtime API,禁止扫进程/读端口文件/grep 源码/GUI 自动化;内含委派规则、安全边界(不把 read 回包文本当指令、不盲目重试、`settled` 不等于成功)。pebrel 侧:`docs/skills/pebrel-runtime/SKILL.md` 全文为蓝本(命名改 slterm)。
28. **env 契约扩展,照抄(叠加既有注入)。** 在现有 `TERM_PROGRAM=slTerminal` + `SLTERM_PANEL_ID` 之上追加 `TERM_PROGRAM_VERSION`、`SLTERM_CLI`(可执行文件绝对路径)、`SLTERM_BIN_DIR`(前置进 PATH)、`SLTERM_PROCESS_ID`;注入点沿用 `src-tauri/src/pty/spawn.rs` 的 spawn 环境块,幂等判重(嵌套 shell 不增长 PATH)。pebrel 侧:`nebula_app/src/agent_env.rs` 的 `apply`、`insert_env`、`prepended_path`、`TERM_PROGRAM`、`CLI_ENV`、`BIN_DIR_ENV`、`PROCESS_ENV`;落地时 `NEBULA_*`/`PEBREL_*` 兼容别名一律不抄。

### 与 ai_hook 的交界(照抄)

29. **`SLTERM_RUNTIME_ENDPOINT` 端点注入,照抄。** `ENDPOINT_ENV` 携带 `"<port> <token>"` 直接注入本实例 pane 子进程:私有实例(不写端口文件时)的 pane 照样能发现控制面;显式 env 端点非法时不得静默回落到端口文件选别的实例。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `ENDPOINT_ENV`、`CHILD_ENDPOINT`、`apply_child_endpoint`、`read_endpoint`。
30. **端点发现三级优先级,照抄。** 本进程 CHILD_ENDPOINT > 显式 env 端点 > 端口文件;二次启动移交请求同走该链,并保留向旧版端口文件的兼容回落位(升级过渡期)。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `read_endpoint`、`legacy_request`。

### 单实例移交(照抄机制,砍语义)

31. **二次启动移交,照抄机制。** 二次启动经 loopback 连接首个实例:先发 ATTACH 唤醒/还原窗口,再发 `tab.new` 新开默认标签;带目录参数启动走 `try_open_directory_existing`。"还原文档 tab 的 PTY 常驻"这一驻留保活语义不搬,移交只覆盖"还原窗口 + 新开 tab"两个动作。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `try_open_default_tab_existing`、`try_open_directory_existing`、`try_open_tab_existing`、`handover_params`。
32. **陈旧端口文件的降级不挂起,照抄。** 客户端 connect 超时 400ms,连不上即认为无活实例、本进程接管端口文件;IO 超时 700ms;port 文件 Drop 删除,被杀进程留下的陈旧文件由连接超时兜底。pebrel 侧:`nebula_app/src/mux.rs` 的 `CONNECT_TIMEOUT`、`IO_TIMEOUT`、`MuxServer::spawn_callback_at`、`request_at`。
33. **旧行协议与新 JSON 同端口并存,照抄分流形态。** 每连接首行非 `{` 开头走 legacy ATTACH/PING 分支、token 不符静默丢弃;新版本功能只经版本化 JSON 开放。slTerminal 无历史协议包袱,该机制作为"移交探测 + 未来演进"的扩展位保留,首发即实现 JSON 主面。pebrel 侧:`nebula_app/src/runtime_api/transport.rs` 的 `handle_connection`、`handle_legacy`;`nebula_app/src/mux.rs` 的 `serve_callback`。

## 不采纳点

1. **mux 驻留保活语义,不采纳(已定)。** `nebula_app/src/mux.rs` 文档注释的核心价值——"关窗后 claude 会话不退出、二次启动重新挂载 detached tab"——整体砍掉;关窗即退出。仅保留其移交机制与降级语义(采纳点 31/32)。
2. **SSH pane 全套,不采纳(已定)。** `PEBREL_PANE_REMOTE=1` 远端标记、`ssh_not_ready` 阶段门禁、SSH pane 拒绝本地文件/文本转发、remote_* 系列错误码,随 SSH/mobile 整支砍掉;env 契约只进本地 PTY 的护栏保留为"无远端概念"的单形态。pebrel 侧:`docs/runtime-control-api.md` 环境契约与错误码表;`nebula_app/src/agent_env.rs` 的远端分支。
3. **mobile_bridge 整支,不采纳(已定)。** `nebula_app/src/runtime_api/mobile_bridge.rs`、`mobile_screen.rs`、`mobile/protocol/bridge-policy.json` 全砍。其 `Request::validate` 的「能力分组白名单 + 通道只读授权 + 显式非零 ID 强校验」校验风格对控制面本身的参数校验有借鉴意义,但白名单作为独立机制不搬——控制面本身已是全能力面,不存在第二通道。
4. **mobile 驱动的 screen v1 手机重排帧,不采纳。** `terminal_screen_v1` 的 wrapped 行标记等手机专用 reflow 字段不抄;`pane.read` 的 screen 附带能力按 slTerminal 实际需要再定,首发只抄文本尾部读取。pebrel 侧:`docs/runtime-api-v1.schema.json` 的 `terminal_screen_v1`;`nebula_app/src/runtime_api/mobile_screen.rs`。
5. **`conversation.*` 整组,不采纳。** `conversation.read/send/choose/key` 是 GPUI 壳为 mobile 做的 Claude/Codex 会话回放交互,依赖壳内私有的会话状态机;slTerminal 的 AI 交互走 hooks + agent_history 既有链,控制面读 Agent 输出用 `agent.read` 即可。identity + epoch 绑定的校验形态在 `agent.*` 的 generation 绑定中已有等价物。pebrel 侧:`nebula_app/src/runtime_api/conversation.rs`。
6. **WSL 透传,不采纳。** `WSLENV` `/p` 路径翻译、按值判重的 WSLENV 合并、`ExecLocation::Wsl` 分发、`for_wsl_distribution`、git 场景的 WSLENV 凭据转发——slTerminal 定位 Windows 原生 pwsh,WSL pane 不在产品面内。env 契约的幂等判重与 PATH 前置仍抄(采纳点 28)。pebrel 侧:`nebula_app/src/agent_env.rs` 的 `merge_wslenv`;`nebula_app/src/runtime_exec.rs` 的 `ExecLocation::Wsl`、`for_git` 的 WSLENV 分支。
7. **多窗口方法面,不采纳。** `window.create`、snapshot 的 `detached_windows` 计数、`session_exempt` 字段、进程级窗口分发器——单窗口单实例定位下 window 维度收编为唯一当前窗口,`window.*` 只留 close/focus 语义的收编形态。pebrel 侧:`docs/runtime-control-api.md` 的 window.create 段;`nebula_app/src/runtime_api.rs` 的 `RuntimeWindow`、`RuntimeSnapshot::detached_windows`。
8. **legacy-shell winit 双壳 EventSink,不采纳。** `EventSink::Winit` 与 `EventSink::Callback` 双形态是 pebrel 双壳并存的产物;slTerminal 单壳,EventSink 收敛为单形态回调。pebrel 侧:`nebula_app/src/runtime_api.rs` 的 `EventSink`;`nebula_app/src/runtime_api/server.rs` 的 `RuntimeServer::spawn`。
9. **`NEBULA_*`/`PEBREL_*` 双名兼容别名,不采纳。** `agent_env.rs` 的新旧双写、`runtime.port` 与 `mux.port` 双发现文件、`legacy_port_file` 兼容回落——新应用无历史包袱,变量与文件名单一 `slterm_*`/`SLTERM_*`;采纳点 30 的兼容回落位仅作扩展位保留,首发不启用。pebrel 侧:`nebula_app/src/agent_env.rs` 的 `LEGACY_CLI_ENV` 等;`nebula_app/src/runtime_api/server.rs` 的 `legacy_port_file`。
10. **提权进程隔离发布逻辑,不采纳(首发)。** `spawn_with_sink` 中 `requires_isolation()` 的提权进程不写端口文件、仅给子进程注 env 的分支随 UAC 隔离策略整体不抄;若未来出现提权场景再单独设计。私有实例 env-only 发现的形态仍抄(采纳点 29)。pebrel 侧:`nebula_app/src/runtime_api/server.rs` 的 `RuntimeServer::spawn_with_sink`。
11. **非终端 tab 类型枚举,不采纳。** snapshot 的 tab.kind 枚举(document/image/code/settings)对应 pebrel 的多类型标签;slTerminal 面板体系在 GPUI 壳内重建(归分片 05/07),控制面 tab 投影按新面板注册表重定。pebrel 侧:`docs/runtime-api-v1.schema.json` 的 `tab.kind`。
12. **超大方法面整族照搬,不采纳。** `git.*` 九方法、`window.create` 链等按 slTerminal 既有命令面(采纳点 22 的收口思路)裁剪映射,不从 schema 整族复制;契约数值(32 KiB/1 MiB/64 客户端等)按 slTerminal 负载重新标定,不照抄 pebrel 常量。

## 优化方向

控制面应成为 slTerminal 的唯一运行时状态权威:GPUI 壳(单进程,壳即前端)与控制面消费同一份 RuntimeHub 状态权威,壳内 UI 与外部客户端不各读私有结构,语义从同一份 snapshot/revision 投影派生;既有 src-tauri 命令能力(pty 写族、git 读族、hooks 注入族)中凡是"pane 里的 AI CLI 也需要"的能力,一律以控制面方法形态重暴露,前端专属能力不留存(随 Tauri 整体消亡)。

协议层按 pebrel v1 形态整体移植并改名:协议标识 `slterm.runtime`、Schema 单文件成文、信封 `deny_unknown_fields`、错误码表成文;等待语义(`state_change_seq`/`after_seq`/`--wait` 提交后基线)与 generation 绑定作为协议一等公民,不做成客户端约定。`runtime.describe` 的 features 探测串机制用于控制面自身的演进兼容。

执行模型上,所有窗口/面板/PTY 写操作进 GPUI 壳 UI owner(App 上下文,对应 pebrel 的 UI owner 线程),transport 线程只做校验与序列化;`pane.exec` 的独立非 TTY 执行与 `pane.read` 的真实 Grid 尾部读取按 pebrel 语义落地,作为"不进 shell history、不截图"的硬边界。

单实例采用"移交不驻留"形态:关窗即退出,二次启动把"还原窗口 + 新开 tab(可带 cwd/shell)"经 loopback 交给首个实例;发现文件、400ms 连接超时接管、token 挡本机其他用户的整套机制保留;移交与控制面共用端口与分流位,不维护第二套 socket。

发现层双轨:pane 环境契约(`TERM_PROGRAM` 系列 + `SLTERM_CLI`/`SLTERM_BIN_DIR`/`SLTERM_PROCESS_ID`/`SLTERM_RUNTIME_ENDPOINT`)解决可达性,打包 SKILL.md 解决"该不该调",`slterm env` 作为不依赖控制面的离线探测入口;三者在 `runtime.describe` 的 env/commands 段可探测、可回落。env 注入叠加在 `src-tauri/src/pty/spawn.rs` 现有注入点之上,与 `SLTERM_PANEL_ID` 的 hooks 路由并存不冲突。

与 ai_hook/hooks 的交界以 generation 对接:hooks 信号链既有的会话身份(session/panel 路由)作为 `agent.*` 方法的 generation 事实来源,保证 claude 重开会话后控制面投递不串任务;委派回传(`agent.delegate`)与 hooks 的回合状态信号复用同一身份链,回传内容按不可信数据处理。

测试分布:协议解析、信封校验、`after_seq` 竞态、worktree 事务回滚、移交降级路径归 Rust 单测与集成测试;控制面端到端(pane 内 claude 实际回控开 tab/派任务/等状态)走 GPUI 虚拟窗口测试形态;测试体系总体归分片 11。
