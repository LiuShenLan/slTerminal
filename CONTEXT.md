# slTerminal

面向 Windows 10/11、面向所有 AI CLI 调优的通用终端模拟器。单进程 GPUI 应用，单窗口单实例、仅暗色模式、GPU 加速渲染。

## 术语

### 产品与工作区

**项目**（Project）：
顶层容器，对应一个根目录，拥有稳定 `ProjectId`；根路径词法唯一并做真实路径查重。项目列表持久化。
_Avoid_: 工作区, 根目录条目

**项目布局**（ProjectLayout）：
项目对应的网格布局树，叶为面板组；每项目恰好一个，多项目布局同时存活，切换只改变可见性。

**面板组**（PanelGroup）：
布局树中的页签组，拥有独立页签栏与激活面板；空组自动移除。

**面板**（Panel）：
面板组内的页签条目，封闭类型为终端、编辑器、Markdown、Diff、GitShow、设置中心；有稳定 `PanelId`。

**活跃项目**（Active Project）：
主区当前显示的项目；项目集合非空时恰好一个。

**项目根**（Project Root）：
项目对应根目录；文件打开、文件浏览器、Commit 与新建终端的基准。

**设置中心**（Settings Center）：
项目内普通面板，每项目最多一个；无项目时由应用设置浮层承载应用页。

**应用设置浮层**（App Settings Overlay）：
无项目时打开的仅含应用页的模态覆盖层；不复用项目布局。

**WindowState**：
窗口大小/位置/最大化状态；语义是启动回放的行为输入，不是诊断字段。

**活动栏**（ActivityBar）：
最左侧常驻窄条，容纳侧栏视图按钮与设置入口。

**侧栏视图**（Side View）：
活动栏按钮对应的可开关内容视图，封闭集 `SideViewId`。未知 id 恢复时静默丢弃。

**双槽**（Top Zone / Bottom Zone）：
侧栏内容的上下两个半区；同一半区最多一个打开的视图，展示由状态纯推导。

> 旧 workspace / tab / pane / session.json 模型已退役；其术语与处置只出现在 `docs/devPlan/design/migration.md`。

### 终端与 PTY

**终端视图**（TerminalView）：
GPUI 中的终端呈现实体：接收 PTY 输出、驱动网格渲染、处理输入与选择。每个视图对应一个终端面板。

**PTY 会话**：
`slterm_terminal` 管理的伪终端进程实例（ConPTY、读写线程、子进程与生命周期）。与「面板组」「面板」是不同概念。

**Shell 解析**：
默认 shell 的探测链——PowerShell 7 → Windows PowerShell 5.1 → `cmd.exe`；用户可在设置中覆盖。

**Shell 集成脚本**：
随终端注入的 PowerShell 脚本，覆盖 `prompt` 函数发射 OSC 序列（cwd 跟踪、提示符边界与退出码），供宿主跟踪命令边界。

**ConPTY 捆绑**：
Win10（build < 21376）下嵌入并提取的新版 `conpty.dll` + `OpenConsole.exe`；Win11 走系统 ConPTY。提取失败静默回退系统路径。

**DA1 / DSR 接管**：
终端核心对设备属性查询（DA1）与光标位置查询（DSR）的按需代答/剥离策略；统一身份 `?64;22c`，禁止盲注。

**滚动回看**（Scrollback）：
终端历史行缓冲；容量是设置键与契约数值，默认值见代码。

**复制语义**：
`Ctrl+Shift+C` 有选区复制并反馈、无选区静默吞；`Ctrl+C` 恒中断并透传 PTY。

### 设置与外观

**设置中心**（Settings Center）：
项目内普通面板，左侧导航 + 右侧配置页；每项目最多一个；无项目时由应用设置浮层承载应用页。配置页经注册表分派，新增配置页 = 组件 + 注册一条。

**设置段**（Settings Segment）：
`settings.json` 的语义分组；「键 → 段」映射单源在 `slterm_settings::keys`，未知键原位保留。

**设置写通道**（settings_store）：
设置落盘的唯一硬机制：白名单校验、段级浅合并、保存锁、原子写、`.bak` 与损坏处理。写核心私有，所有写入经审计包装。

**前端/后端消费型配置**：
消费侧在 UI 的配置走通用段写；消费侧在后端的配置走域专用命令（校验 + 内存态 + 落盘一体）。禁止第二写通道。

**配色 token**：
UI 颜色的语义命名槽位。组件只引用 token，禁止硬编码颜色；仅暗色。

**内置主题**（ThemeName）：
内置暗色主题目录，`Linear` 为默认；主题色经 WCAG 语义槽校验后才可入目录。

**明度阶梯**（Lightness Ramp）：
UI 背景色的离散档位；层级靠相邻档差表达，不叠加阴影/边框。

**发丝线**（Hairline）：
暗色界面分隔线的唯一形态——半透明 1px 线，禁止实色粗边框。

**窗口材质**（BlurMode）：
材质档位枚举 `None/Mica/MicaAlt/Aero/Acrylic`，默认 `None`；`opacity` 为标量。WCA/DWMSBT 通道互斥，按 Windows build 门控。

**i18n 语言偏好**（LanguagePref）：
`System/English/Chinese`；System 按系统 locale 协商。翻译键集是构建期硬合同。

### Agent 集成

**编码 CLI / AI CLI**：
以命令行运行的 AI 编码代理（claude、codex、gemini 等）。slTerminal 面向所有 AI CLI 调优，`AgentKind` 是内置身份的封闭枚举。

**AgentKind**：
AI CLI 的封闭身份枚举；承载 slug、显示名、启动/恢复/分叉命令与屏幕规则匹配。持久化 `source` 宽松落盘，消费时经 `AgentKind::parse` 严格归一。

**Agent 状态**（AgentStatus）：
面板的 agent 语义状态：Idle / Working / Blocked / Done / Unknown；来源分为 Hook / Screen / Process / Unknown。`Unknown` 表示尚无权威判定（普通 shell 或未识别）。

**AgentHub**：
agent 事件归属与面板状态投影的宿主：消费 hook 事件与屏幕看门狗结果，维护 AgentActivity/AnswerInbox。

**AI hook 事件**（AiHookEvent）：
归一化的生命周期事实（会话开始、提示提交、工具完成、回合结束、需要关注、会话结束），附回合结果与能力位。

**hook 小进程**（`slterm-hook`）：
每个 AI 回合事件调用一次的独立小进程；经命名管道把原始载荷转发给宿主后退出。必须 exit 0、快速、隐形。

**命名管道通道**：
hook 事件唯一权威通道；宿主侧服务器解析 JSON，helper 不重编码。

**statusline 桥接**：
context 用量百分比的数据通道；桥接脚本节流后写 ContextUsage 信号文件，并透传用户原 statusline。与 hook 管道分通道。

**信号文件通道**：
statusline 用量等低频信号的介质；app 数据目录，与命名管道分通道。

**ContextUsage 信号**：
statusline 桥接产出的用量事件，payload 携带 `usedPercentage`。

**SLTERM_PANE_ID**：
PTY spawn 注入子进程的环境变量，经 shell → CLI → hook 继承链传递，用于会话→面板路由；v2 使用 `SLTERM_PANEL_ID`。

**历史会话**（AiSession）：
编码 CLI 在某目录运行产生的持久化会话记录（claude 为 `~/.claude/projects/` 下 `<uuid>.jsonl`），由各 CLI 的扫描/恢复命令处理。

**hook 失败边**：
hook 执行失败时的合成命令与证据；默认关（复用 `AssistantConfig.enabled`），只进闸门与 log，不直接弹 UI。

### Runtime 控制面

**Runtime API**：
本机 loopback 上的 JSON Lines 控制面，供 AI CLI 与脚本查询/驱动终端；协议名 `slterm.runtime`，目标版本 2；v1 已退役，不保留兼容层。

**信封**（Envelope）：
请求/响应的统一外层：`protocol`/`version`/`id`/`token`/`method`/`params`；响应以 `ok` + `result` 或 `error` 互斥表达。

**端点发现**：
进程内子端点 > `SLTERM_RUNTIME_ENDPOINT` 环境变量（`<port> <token>`）> 数据目录 `state/runtime.port`。

**CLI 双人格**：
同一 `slterm.exe`：带子命令 = CLI 客户端，无子命令 = GUI。`slterm ctl` 暴露完整协议，资源动词（`window`/`project`/`panel-group`/`panel`/`agent`）为同一信封上的薄别名，`slterm env` 离线报告身份与端点。

**RuntimeHub / RuntimeDispatch**：
Runtime API 的宿主侧枢纽：接收写命令、派发到工作区执行、经回执通道返回结果。

**单实例移交**：
第二个实例启动时不建窗，改为唤起既有实例；移交走版本化 JSON，载荷为仅聚焦、打开项目、打开文件或新建终端；目标不存在时提示，不自动创建项目。

**Runtime skill**：
随包分发的 `slterm-runtime` Skill（`assets/skills/`），提供给 AI CLI 自发现控制面；安装器按受管文件指纹投放。

### 文件与编辑器

**文件编辑器**（file_editor）：
内置源码/文本编辑内核与视图：文档模型、撤销、选择、输入规则、gutter、outline、预览/并排形态。

**编辑器面板 / Markdown 面板**：
`PanelKind::Editor` / `Markdown`；文件打开路由由单一路由判定；图片不进入面板封闭集，走系统默认程序。

**Diff 面板**：
HEAD ↔ 工作区双栏或行内对照，工作区侧可编辑；gutter 点击与重开命中走单入口。

**Gutter**：
编辑器行号区，按 git 状态显示增删改标记；点击打开对应 diff。

**文件浏览器**：
侧栏视图 `Explorer`：项目目录树、git 状态着色、创建/删除/重命名/打开，回收站删除走 `SHFileOperationW`。

**文件树模型**（side_panel）：
只持数据与状态转换的模型层；渲染按行号取行，模型内禁止出现 GPUI 类型。

### 后台任务与套餐余量

**后台定时任务**（Background Task）：
应用级任务单元；元数据注册于 `background_tasks`，执行体在后端或订阅方，配置落 `backgroundTasks.{taskId}` 段。

**TaskSchedule**：
任务触发形态——`Interval`（周期）/ `Once`（启动一次性，`interval_sec` 为启动延迟）/ `OnDemand`（事件驱动）。共用订阅生命周期与失败策略。

**扫描执行体**：
历史会话扫描的唯一执行路径；手动刷新与定时刷新共用。

**套餐余量**（Plan Balance）：
编码套餐当前剩余可用量（金额余额 / 时间窗用量）；查询走 user 层 settings 的 `env` 段，token 不出凭据域。

**余量推送**（Plan Balance Watch）：
模块级 `SNAPSHOT` + watch 语义订阅：只报「有新值」，订阅者读最新快照；有界 1 槽合并连续推送。

### 安全

**凭据域**：
真实凭据只存 Windows 凭据管理器；业务经 `secrets` 门面访问。`SecretBytes` 出域即用即焚，token/key 不出域。

**审计通道**（audit）：
`tracing` target `audit` 的安全事件通道；事件字段只含 operation/path/source/result 与 expected/actual 指纹，禁插值凭据值与文件内容。

**加密备份**：
口令保护的本地备份，格式 `TRMBK001 | salt | nonce | ciphertext`（AES-256-GCM，魔数作 AAD；Argon2id 派生）。类别与敏感键表以代码为准；敏感键导出剥除、导入复验。

**恢复点**（RestorePoint）：
一次备份/恢复操作的可撤回句柄；会话内撤回，跨重启清单归设置页安全分区。

**敏感键表**（SENSITIVE_KEYS）：
备份导出剥除与导入复验共用的第二道防线；手改文档塞入的凭据字段一律剥除/拒绝。

### 基础设施与治理

**GPUI 前台/后台**：
前台 = UI 线程（渲染与状态更新，禁阻塞）；后台 = background executor 或专用线程（阻塞 I/O、网络、子进程）。

**AppError**：
通用错误信封；变体与转换以类型定义为准。领域不得另立 envelope，错误消息用户可见且禁带凭据。

**会话元数据索引**（terminal_registry）：
模块级单例，按 `PanelId` 提供会话元数据查询；Workspace 是实体唯一属主，索引不裁决生命周期。

**原子写**：
持久化通用纪律——临时文件 + rename；损坏状态处理；设置写经审计包装。

**门禁**：
行数棘轮（`architecture/file-budgets.txt`）、依赖方向（`architecture/dependencies.toml`）、禁名回流（`scripts/check_brand_regress.py`）、文档策略（`scripts/check_doc_policy.py`）；git hooks 常挂。

**决策记录**：
当前有效决策写入目标路径最近的 `CLAUDE.md` 或 `.claude/docs/` 对应子域；不集中维护 ADR 或因果档案，完整历史从 git log 查。

**豁免登记**：
`.claude/docs/testing/exemptions.md` 是不可自动化测试面的单表真值源；未登记豁免即硬约束违规。
