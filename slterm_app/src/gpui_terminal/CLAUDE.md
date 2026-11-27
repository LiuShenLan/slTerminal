# GPUI 终端视图

> 把核心终端 worker 接入 GPUI：固定代次命令绑定、不可变帧呈现与显式生命周期。

## 决策与红线

- **worker 单一所有权**：`AppRoot` 强持唯一 `TerminalRegistry`；Registry 独占逻辑终端到 `TerminalSession`、PTY 代次与进程身份的映射。原因：Panel、Runtime 与 View 分持会话会形成多个关闭源和 PID 复用错认。边界：View 只持固定 `TerminalTarget` 的弱 `TerminalBinding`，不得直持 `Term`、PTY 或子进程句柄；旧代次目标必须拒绝命令。
- **存活额度原子预检**：Registry 在调用启动或重连工厂前按 `process_live` 检查 64 个存活 PTY 上限；额度不足不得执行工厂、消耗新代次或改变既有目标，进程退出后同一记录可重试。原因：应用记录与真实进程必须共享同一容量事实，不能等创建成功后补救。边界：已退出和未启动记录不占额度；core 的全局创建锁仍作为最终并发闸门。
- **不可变帧呈现**：worker 在输出与显示状态边界发布带 revision 的 `TerminalFrame`；View 与绘制元素只读最新 `Arc` 帧，滚动、选区、resize 与配色经 worker 命令执行。原因：绘制阶段锁实时网格会把 PTY 解析与 GPU 呈现耦合，并让坐标映射读取过期状态。边界：paint 不锁 `Term`；显示命令只能在 worker 内修改网格。
- **退出不等于关闭**：进程退出只退休输入能力；显示 worker、事件订阅与最后画面继续服务滚动和选区，直到显式关闭。原因：用户退出 shell 后仍需阅读与复制输出。边界：事件泵不得因 `Exit` 自行终止；只有 View 销毁或 Registry 显式退休才结束对应生命周期。
- **关闭单点**：局部、批量、项目与应用关闭先按完整目标从 Registry 退休 worker，再在后台显式 `shutdown`／join；View Drop 只解除订阅，不杀 PTY。原因：隐藏项目与 View 析构不得绕过统一关闭协调。边界：GPUI 前台不得 join；重连换出的旧 worker 同样必须移交后台关闭。
- **重连代次切换**：只有已退出终端可经统一 `ReconnectTerminal` 命令重连；新 worker 共享原网格并使用新事件通道，View 原子替换固定绑定、最新帧与订阅任务，清除旧补全和输入镜像。旧目标按代次独立后台回收。原因：旧事件、搜索或 Runtime 读取不得落入新进程，失败重试不得替换 Panel。边界：启动失败保留旧绑定、Exited 状态和错误；成功后才提交 `RunIntent::Launch`。
- **有界事件传输**：每会话事件通道容量 256；帧与纯重绘可合并为最新帧，语义事件不得丢弃，满载由 worker 等待接收端释放容量。原因：输出洪流不能无界占用内存，也不能丢失命令边界或退出事实。边界：不得把语义事件降级为可覆盖状态。
- **隐藏只暂停绘制**：项目或 Panel 隐藏时继续解析、推进帧与保留输出，但不发出输出重绘；恢复可见只主动呈现最新帧，不补放中间帧。原因：后台进程不能因导航暂停，恢复也不能重放无效绘制。边界：隐藏不得关闭事件泵或丢弃字节。
- **补全编辑复核**：`ShellLineTracker` 的输入 revision 只随本地编辑与生命周期变化推进，PTY 回显不推进；请求键和后台回包同时携带该 revision。cwd 上报变化必须取消旧计算并刷新候选，不能只更新目录元数据。接受任何历史 ghost 或弹窗候选前必须先执行 `CompletionEdit::apply` 复核原文与 revision，失配不得写 PTY，也不得自动提交 Enter。原因：文本相同不代表仍是同一输入实例，接受动作必须先证明编辑身份。
- **OSC 52 授权**：只写不读；隐藏项目始终拒绝，窗口与 pane 均聚焦时才默认允许，非聚焦写入还必须开启 `osc52_unfocused`。单条 decoded UTF-8 上限 1 MiB；读取请求不得访问剪贴板或产生 PTY 应答。原因：终端输出是不可信输入，不能借后台进程窃取或覆盖系统剪贴板。
- **粘贴授权**：超过 10 MiB 直接拒绝；危险控制字符或超过 50 行在 `multiline_paste_confirm` 开启时必须经异步确认，提示不得包含正文。同一视图只允许一个在途确认，退出或 PTY 代次变化后不得补写；括号粘贴继续剥离内嵌结束哨兵，不自动追加 Enter。
- **OSC 8 打开边界**：链接目标只从最新不可变帧读取，仅 `http`／`https`／`mailto`、不超过 8 KiB 的 URI 可经 Ctrl+单击交系统打开；普通单击、普通文本和其他协议不得触发外部打开。
- **只读搜索**：`Ctrl+Shift+F` 打开当前终端的普通文本搜索；查询、匹配、位置与高亮只存 View 瞬态。搜索经 worker 的 `SearchLiteral` 命令只读当前画面与回滚，新帧先作废旧位置，同代次回包才可回填；前后导航只发本地滚动命令，永不写 PTY。关闭或重连不得恢复查询。
- **恢复与 CLI 信号**：恢复和重连仅在 `resume_ai` 允许时消费 TerminalRecord 的 AgentSession；显式 Runtime Agent 启动／恢复／分支经 `schedule_agent_command` 注入，不受 `resume_ai` 门控，但同样只在真实提示符或超时后执行一次；经 AgentCatalog 构造的命令在同代次真实提示符到达时注入，就绪等待按真实程序名经 `app_root::navigation` 的同一分类与上限（PowerShell 与未知程序最多 10 秒、cmd 约 0.5 秒），超时同样至多写入一次；本模块不得另设超时常量或第二套 Shell 分类，未知程序按上限等待以避免未就绪写入丢命令。显式 Runtime 调用携带完成回调，只在真实写入成功或明确失败时回执；退出、关闭、重连替换与 View 销毁取消旧任务并丢弃回调，迟到回执不得补写，丢弃由服务端按结果不明处理。BEL 与 OSC 9 分别以 `Bell`／`Text` 通知经 `TerminalViewEvent::Notification` 交给宿主，按 pane 进入唯一通知漏斗，视图不直接发系统通知；自定义标题锁定后不被 OSC/CLI 标题或重置覆盖；普通 Shell 命令保持 AgentStatus::Unknown，只有识别的 Agent 命令边界才建立工作状态，结束或退出不得伪造成完成。
- **可信输入单镜像**：`ShellLineTracker` 独占受支持 Shell 的本地行；本地编辑先等待同代次 PTY 回显对账，只有匹配才开放补全。Enter 只建立待决执行，必须依次收到 CommandStart 与 CommandDone 才采集；取消、新提示符未闭合、Readline、控制键、resize/reset、备用屏或前台接管均使镜像失效。原因：本地按键与 Shell 实际编辑可能分叉，不能以 Enter 或本地缓冲冒充真实执行。边界：未知 Shell、旧代次和失配回显不得复活补全或写历史；接受候选只编辑，不自动执行。

## 验收与测试

- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support gpui_terminal:: -- --test-threads=1`。
- 必测面：启动成功才分配代次、重复启动不执行工厂、重连使旧目标失效、代次耗尽、64 个存活上限下启动／重连零副作用及退出后重试、进程身份包含 PID 与创建时间、退出后拒绝输入但继续交付显示帧、真实重连保留回滚并插入分隔、View Drop 不停止 worker、Registry 退休后显式 join、绘制快照不锁实时网格、事件合并不丢语义事件。
- 测试视图经 `restore_error`／`restore_exited` 构造后安装注册到同一 Registry 的内存 worker；除明确的真实 ConPTY 用例外，禁止拉取真实 PTY 或访问用户数据。

## 操作

- 新增终端显示能力：先扩 core 命令或不可变帧，再经 Binding 与事件泵接线，不在 View 直接操作实时网格。
- 新增 worker 生命周期入口：复用 Registry 与既有关闭协调器，明确后台 shutdown 所有权。
