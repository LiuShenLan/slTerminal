# Windows 平台

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有 Windows 平台边界：单实例与移交载荷、窗口几何/DPI/多显示器、系统通知与任务栏闪烁、托盘与开机启动、文件夹选择器、回收站、默认程序、进程树终止；仅 Windows 10/11，业务平台分支收敛在平台封装模块。

### 决策与红线

- **单窗口单实例是进程约束**：启动先抢生命周期锁；抢不到即把移交载荷发给既有实例并退出，绝不创建第二窗口。原因：多进程并发写项目数据与争抢焦点不可接受。边界：既有实例无响应时明确报错退出，不降级为第二实例。
- **移交载荷是结构化意图，不是命令通道**：第二次启动只能表达打开项目、打开文件、新建终端、仅聚焦；不执行任意命令行、不写任意路径。原因：命令行参数来自不可信上下文，必须收敛为产品动作。边界：Runtime v2 的完整命令面由认证端点承担，不走启动移交。
- **窗口几何按逻辑像素持久化（迁移里程碑）**：保存普通态边界、最大化态与显示器标识；启动时夹取到工作区，保证标题栏可达。原因：DPI 变化与显示器拔插会让绝对像素失效。边界：规格内只保留纯函数夹取规则，持久化与启动恢复属迁移里程碑，不创建 `window.json`；不保存最小化态，最大化恢复为普通态边界。
- **系统通知只在失焦时出现**：窗口聚焦时 CLI 状态事件不打扰用户；`desktop_enabled` 只控制系统 toast，`taskbar_flash` 独立控制任务栏闪烁；所有类别在 `taskbar_flash` 开启时触发闪烁，闪烁是唯一回窗引导。原因：任务栏与 toast 是两套注意力通道，聚焦时都多余。边界：应用内通知不受此门控。
- **托盘是回窗与注意力面，不是第二窗口**：托盘线程拥有隐藏消息窗口与原生菜单；退出走统一关闭链。原因：托盘回调需要 Win32 消息泵，不能挂在 GPUI 窗口上。边界：托盘不可用时仅托盘能力降级，不改变启动形态。
- **删除统一回收站**：文件与目录删除走 `SHFileOperationW` + `FOF_ALLOWUNDO`；回收站不可用时才回退永久删除并说明。原因：数据安全与用户预期。边界：不提供 `Shift+Delete` 或应用内永久删除入口。
- **平台代码只在平台模块出现 `#[cfg(windows)]`**：业务模块使用运行时能力探测与抽象接口；平台模块是 Windows 原生调用的唯一落点。原因：平台分支散落会让非 Windows 构建与测试路径腐化。边界：`slterm_terminal` 的 PTY 平台模块保留其既有边界。

### 单实例与移交载荷

`runtime.handover` 的线缆类型、参数 schema、单 op 形态、校验与接收语义归 [runtime-api-v2.md](integrations/runtime-api-v2.md)（`slterm_runtime_protocol::methods::handover`）。一次移交只含一个 `HandoffOp`，不定义操作序列；`request_id` 是幂等键，接收端对同一 `request_id` 返回首次结果。平台层只做 Windows 路径校验、结果映射与进程退出，不自定义载荷类型。

- 启动全序：完成进程准备与日志初始化后，抢 `<local>/state/instance.lock` 句柄锁（独占共享模式）→ 抢到则继续正常启动并登记控制端点；抢不到则读取既有端点，按 400ms 连接超时、700ms 读写超时发送 `runtime.handover` → 收到 `Accepted` 后退出。抢锁失败但端点暂不可达时按 40 次 × 25ms 重试探活；仍不可达则报告“已有实例未响应”并退出。
- 单实例锁统一为 `<local>/state/instance.lock` 句柄锁：基于句柄独占，进程崩溃由内核释放，不做超时回收；锁文件可残留，所有权只看句柄。
- 移交方法统一为 `runtime.handover`；传输复用 Runtime v2 的本地控制端点（回环 + token），线缆类型与 schema 归协议 crate，平台层只做校验与结果映射。壳启动最小控制端点，保证单实例语义。端点 token 不进日志、不进通知、不进诊断包；不另建命名管道。
- 接收侧映射：
  - 未知 `schema` 由协议层拒绝并记录一次；平台只映射拒绝结果，不尝试兼容旧载荷。
  - `OpenProject` 根必须是绝对路径、存在且为目录；已存在则激活，不存在则提示用户先在导航树添加或匹配现有项目，不自动创建。
  - `OpenFile` 的可选 `project_root` 若提供必须是已登记项目；未提供时按 `path` 在已登记项目根中取最深命中。文件路径经 `PathGuard` 校验并由统一文件打开路由分派；命中已有面板时聚焦，不新建。
  - `NewTerminal` 的目标项目：`project_root` 提供时必须是已登记项目；未提供时按 `cwd` 在已登记项目根中取最深命中，未命中则用活跃项目。`cwd` 经 `PathGuard` 校验，越界回退目标项目根并提示。目标组为该项目的聚焦面板组；无面板组时创建第一个。关闭链非 `Idle` 或 PTY 达到上限时拒绝，不排队重试。成功后聚焦新终端面板。
  - `Focus` 只还原、前置并聚焦唯一主窗。
- 前置失败（系统拒绝 `SetForegroundWindow`）时闪烁任务栏并保持窗口还原；不创建新窗口。
- 第二实例退出前不写任何项目数据，不触碰配置目录；只发送载荷并等待有界回复。
- 移交载荷与 Runtime v2 资源模型对齐：项目以根路径定位，文件以可选项目根 + 相对/绝对路径定位，终端以可选项目根 + 可选 cwd 定位；不传内部 panel id。

### 窗口几何、DPI 与多显示器

```rust
pub struct WindowGeometry {
    pub normal_bounds: Rect<Px>,      // 逻辑像素，普通态
    pub maximized: bool,
    pub monitor: Option<MonitorKey>,
}

pub struct MonitorKey {
    pub device_name: String,
    pub work_area: Rect<Px>,
}
```

- 持久化落点为 `<local>/state/window.json`；窗口几何保持范围外迁移能力，迁移里程碑才接入。规格内不创建该文件，关闭链不冲刷窗口几何。本节的夹取规则是目标态纯函数约束；持久化读写与启动恢复的接线属迁移里程碑验收。
- 默认窗口 1200×800 逻辑像素；窗口最小 760×540（权威约束）。内容最小宽度 406px 为派生约束（活动栏 46 + 侧栏最小 160 + 主区最小 200）；窗口最小尺寸与主区下限同时校验。
- 启动恢复顺序：读取保存几何 → 按 `device_name` 查找显示器 → 找不到用主显示器 → 将普通态边界夹取到工作区 → 若最大化则在该显示器最大化 → 否则按夹取后的普通态显示。
- 夹取规则：尺寸不超过工作区；位置保证标题栏至少 200×34px 可见；完全离屏时居中到目标工作区；工作区小于最小尺寸时使用工作区可容纳的最大尺寸并允许滚动/缩放内容。
- 不保存最小化态；从最小化恢复使用保存的普通态或最大化态。最大化状态由窗口状态事件回查，不依赖按钮本地翻转。
- DPI：持久化逻辑像素，不保存 scale factor；显示器 DPI 变化时由 GPUI 重新布局，应用按新工作区重新夹取。跨显示器拖动保持逻辑尺寸，不按像素尺寸缩放。`MonitorKey` 不含 DPI，DPI 不参与显示器恢复匹配。
- 多显示器：窗口归属按标题栏中心所在显示器判定；显示器移除时迁回主显示器并夹取。系统贴靠、双击最大化、拖动边框由操作系统窗口能力处理，应用只维护按钮状态与拖拽区命中。
- 自绘标题栏高 34px，左右内边距 12px，字号 12px；左段与中段为拖拽区，右段窗口控制钮不参与拖拽。窗口控制钮 38×26px、图标 12px、间距 2px、圆角 4px。
- 启动形态：正常启动显示窗口但不抢键盘焦点；首版不做静默启动，托盘缺失不改变启动形态。
- 关闭链：状态机以 [architecture.md](architecture.md) 为单点，`Guarded` 集合、冲刷范围与阶段时限见 [persistence.md](persistence.md)；平台只提供 PTY 终止与窗口移除。PTY 与项目数据保存失败只记录并继续关闭。

### 系统通知与任务栏闪烁

```rust
pub enum CliStatusCategory { TaskComplete, WaitingForInput, PermissionRequest, Error }

pub struct EventKey {
    pub panel_id: PanelId,
    pub kind: CliStatusCategory,
    pub session_id: String,
    pub turn_key: String,
}

pub struct CliStatusEvent {
    pub event_key: EventKey, // kind 即通知类别
    pub project: Option<ProjectId>,
    pub occurred_at: SystemTime,
}
```

- 门控：仅当窗口失焦且收到编码 CLI 状态事件时按设置分别处理；窗口聚焦时全部丢弃（应用内通知另走通知服务）。`desktop_enabled` 只控制系统 toast 投递，`taskbar_flash` 独立控制任务栏闪烁；两者互不代偿。
- 类别由 CLI 能力声明决定；未注册或没有状态事件能力的 CLI 不产生系统通知。
- 正文格式 `<项目名> · <类别> · <时间>`；项目名无法反查时留空。正文纯文本，控制字符过滤，XML 转义。
- 去重：事件键、去重窗口与缓存上限以 `cross-cutting.md` §通知类别与时长 为唯一 owner；本节只定义系统通知投递与投递失败行为。
- 系统通知无点击跳转；所有类别在 `taskbar_flash` 开启时触发任务栏闪烁，窗口重新获得焦点后停止。闪烁使用 `FlashWindowEx` 的 `FLASHW_ALL | FLASHW_TIMERNOFG`，对前台窗口幂等静默。
- 通知权限在首次需要时请求一次；AUMID 使用固定产品标识并在 HKCU 幂等注册。WinRT toast 只在常驻 MTA worker 投递，队列有界，满即丢；活动 toast 对象有界保留，防止回调提前释放。
- 投递失败只记录日志，不影响终端；通知是 best-effort。

### 托盘与开机启动

```rust
pub enum TrayCommand {
    Focus(Option<PanelRef>),
    Quit,
}

pub struct TrayAgentEntry {
    pub project: ProjectId,
    pub panel: PanelId,
    pub label: String,          // CLI 名 + 目录尾段
    pub attention: bool,
}

pub struct TrayMenuModel {
    pub agents: Vec<TrayAgentEntry>,
    pub language: UiLanguage,
    pub enabled: bool,
}
```

- 托盘线程拥有永不显示的 Win32 消息窗口与 `Shell_NotifyIconW` 图标；应用侧写入菜单快照并唤醒托盘线程，托盘线程只经有界回调发送 `TrayCommand`，不跨线程共享可变 UI 状态。
- 托盘菜单按注意力优先排序，列出正在运行的 Agent 面板；点击直达项目/面板/焦点落点；与 toast 回窗语义一致。托盘图标在等待输入/权限请求时显示注意力标记，标记同时提供提示文本，不仅靠颜色。
- 托盘启用状态来自应用设置；禁用时销毁图标与菜单。托盘创建失败时通知并保留设置值，运行期不自动重试，下次启动重试；失败重试策略以本节为单点，其他文档只引用。
- 开机启动：写入用户 Startup 已知文件夹的 `slTerminal.lnk`，使用 `IShellLinkW` 幂等重写；关闭删除快捷方式，`NotFound` 视为成功。安装器与设置页管理同一路径。
- 开机启动状态只读 `.lnk` 是否存在，不以内存开关为准；安装器创建/删除快捷方式后设置页重新查询。
- 托盘退出走统一关闭链，与标题栏关闭、系统关闭命令一致；关闭窗口即退出，托盘不改变退出语义。托盘线程在退出链中停止并移除图标，不留孤儿消息泵。

### 文件夹选择器、回收站与默认程序

- 文件夹选择器使用 `IFileOpenDialog` + `FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM`，所有者窗口为唯一主窗；取消返回 `None`，不产生错误；选择结果必须是绝对文件系统路径，经路径校验后使用。
- 回收站删除使用 `SHFileOperationW`：
  - 标志：`FO_DELETE | FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI`。
  - `pFrom` 双 NUL 结尾；返回码非 0 或 `fAnyOperationsAborted` 非 0 视为失败。
  - 回收站不可用或系统拒绝时，提示原因；用户确认后可回退永久删除并明确说明不可撤销。
- 默认程序打开使用 `ShellExecuteW` 的 `open` 动作，参数为单个结构化路径；返回值 ≤32 视为失败并通知。外部 URL 只允许 `http`/`https`/`mailto`/`tel`。
- 文件树重命名/新建/删除的名称规则归 [security.md](security.md) 的 `PathGuard`（`InvalidName` 决策）；平台文件操作模块只执行系统调用并返回类型化错误。已存在目标不得静默覆盖，唯一例外是同一文件仅大小写变化。
- 文件被占用、权限不足、路径过长、磁盘满等错误映射为可见错误与重试/系统程序打开入口，不静默覆盖。

### 进程树终止

- 终端 PTY 子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`；关闭面板或退出时先 `TerminateJobObject`，再 `Child::kill` 兜底。
- 一次性非交互子进程同样使用 Job Object 与隐藏窗口；超时后终止整棵树，防止后代持有管道导致读取挂起。
- 进程身份使用 PID + 创建时间双因子，避免 PID 回收后误认；进程快照按 250ms TTL 复用，父指针成环时按深度上限停止。
- 关闭链并发终止所有已注册 PTY，最多等待 3 秒；超时后仍继续关闭，由系统进程树终止兜底。失败项汇总记录，不阻止窗口关闭。
- 终端后端回退到系统控制台时提示 `终端已回退到系统控制台,鼠标滚轮转发不可用`；能力探测与回退细节由终端文档拥有。

### 平台能力探测

```rust
pub struct WindowsCapabilities {
    pub conpty: bool,
    pub mica: bool,
    pub acrylic: bool,
    pub toast: bool,
    pub tray: bool,
    pub dark_mode: bool,
}
```

- 能力探测在启动早期一次完成并缓存；不可用时对应功能进入降级态并只提示一次。
- `cmd` 无完整 Shell 集成；Windows 10 早期版本无 ConPTY 时回退系统控制台；两者都按平台边界显式降级，不伪造能力。
- GPU/渲染能力由 GPUI 处理；渲染不可用时静默降级软件渲染，功能不变，不向用户暴露渲染器。
- Git 操作依赖本机 Git 可用性与仓库状态；非仓库进入错误态并提示手动初始化，不自动执行 `git init`；不执行网络 Git 操作，不修改 Git 配置。平台层只负责可执行文件探测与结构化参数，仓库语义归 Git 服务。

### 验收与测试

- 单元层：移交请求的路径校验与结果映射、窗口几何夹取纯函数、通知去重裁剪与 `desktop_enabled`/`taskbar_flash` 门控、回收站标志、进程树终止状态机。
- 集成层：第二实例移交（真实进程）、`instance.lock` 互斥、托盘命令回调、文件夹选择器替身、默认程序与回收站替身。
- 虚拟窗口层：关闭链顺序；窗口几何恢复、最大化状态回查、DPI 变化重夹取属迁移里程碑验收。虚拟窗口测试必须带 `gpui-test-support` 进入提交门禁，见 [testing.md](testing.md) §门禁。
- 人工/条件跳过：真实系统通知与 AUMID、任务栏闪烁、托盘图标与菜单、真实回收站、真实默认程序、多显示器拔插、ConPTY 回退；登记根因与兜底。
- 安全：移交载荷不接受任意命令；路径在接收侧重新校验；token 不落日志。
