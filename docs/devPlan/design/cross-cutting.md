# 跨域契约

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 跨域契约索引：错误与反馈、通知、焦点与键盘、IME/CJK、性能预算、后台任务生命周期、安全边界、i18n 与主题 token；每项只在所有者文档定义，其他域引用。

### 决策与红线

- **单一错误通道**：所有领域错误转换为统一 `ErrorReport`，由反馈路由器选择行内、横幅、通知或对话框。原因：面板、侧栏、设置各自弹错会产生重复、遗漏与不可关闭的噪声。边界：内部可恢复重试可静默，但最终失败必须可见。
- **单一通知服务**：应用内通知、可操作通知与系统通知共用一个服务；视图只能声明时长覆盖与去重键，不能自建浮层。原因：通知风暴、悬停暂停与去重必须集中。边界：系统通知仅在窗口失焦时发送。
- **单一焦点/命令契约**：命令优先级、焦点意图、覆盖层栈与 IME 门控由 `design/ux-framework.md` 单点定义，其他域只引用。原因：键盘行为跨面板重复实现必然漂移。
- **单一安全边界**：路径允许根、URL/OSC 白名单、Markdown 清洗、进程启动、凭据域由 `design/security.md` 单点定义。原因：安全判断分散会被局部例外侵蚀。
- **单一主题/语言入口**：颜色、字号、间距、动效与文案只从 `ThemeTokens`/`Message` 读取。原因：硬编码颜色与字符串会绕过主题与翻译合同。
- **资源上限单源**：PTY、回滚、编辑器、Diff、Markdown、GitShow、设置载荷的上限只在所有者文档定义，本文件只做索引，不复制数值。原因：跨文档复制数值会漂移。

### 契约索引

| 主题 | 所有者 | 本文件约束 |
|---|---|---|
| 命令、快捷键、焦点、覆盖层、IME、键盘/焦点原语 | `design/ux-framework.md` | 所有域只能经统一派发器与焦点协调器 |
| 错误分类、反馈层级、通知类别与时长 | 本文件 | 所有域返回统一 `ErrorReport`/`NotificationSpec` |
| 路径、URL/OSC、Markdown、进程、凭据 | `design/security.md` | 所有域经统一安全边界，不自建例外 |
| 主题 token、可读性、语言与翻译合同 | `design/theme-and-i18n.md` | 组件不得硬编码颜色或用户文案 |
| 单实例、窗口、系统通知、托盘、回收站、进程树 | `design/platform-windows.md` | OS 调用只在平台封装模块 |
| 日志、审计、panic、诊断包 | `design/diagnostics.md` | 诊断只经统一通道与脱敏 |
| 包名、安装器、更新、门禁与预算 | `design/build-and-release.md` | 发布只走单一入口与清单合同 |

### 错误分类与反馈层级

```rust
pub enum ErrorClass {
    PathNotFound,
    PermissionDenied,
    FileBusy,
    InvalidFormat,
    VersionIncompatible,
    ExternalConflict,
    ResourceLimit,
    PlatformUnavailable,
    ThirdPartyFailure,
    SecurityDenied,
    Internal,
}

pub struct ErrorReport {
    pub class: ErrorClass,
    pub code: &'static str,                 // 稳定诊断码，不进入用户文案
    pub message: MessageArgs,               // 用户可读原因与动作
    pub detail: Option<RedactedText>,       // 可折叠详情，禁止凭据/内容/终端输出
    pub actions: Vec<ErrorAction>,          // Retry / Reload / OpenWithSystem / Reveal / CopyDetails
    pub retryable: bool,
}

pub enum FeedbackLevel { Inline, Banner, Notification, Dialog }
```

| 层级 | 适用场景 | 生命周期 | 必须提供 |
|---|---|---|---|
| 行内 | 字段校验、保存状态、过滤输入 | 随控件状态 | 原因、正确范围/格式、输入保留 |
| 面板横幅 | 外部冲突、文件过期、Agent 错误、Hooks 外部修改 | 保留到解决、关闭或重试成功 | 明确动作（重载/覆盖/重试） |
| 通知 | 后台完成、短操作结果、非阻断失败 | 默认时长或视图覆盖 | 关闭按钮、可选动作、去重 |
| 对话框 | 危险操作、关闭守卫、导入预览、项目移除 | 模态，直到确认/取消 | 影响对象、取消、危险样式 |

- 分类到默认层级的映射：`PathNotFound`/`PermissionDenied`/`FileBusy` 按操作选行内或通知；`InvalidFormat` 输入场景行内、文件场景横幅；`VersionIncompatible` 通知并提供只读/重装入口；`ExternalConflict` 横幅；`ResourceLimit` 横幅或行内并说明上限；`PlatformUnavailable` 通知一次并进入降级态；`ThirdPartyFailure` 通知；`SecurityDenied` 行内或横幅且不提供“仍然执行”；`Internal` 通知并附可折叠诊断详情。
- 影响用户数据、终端状态、设置持久化或安全边界的失败必须至少达到通知层；可能丢失数据且不可撤销的操作必须使用对话框。
- 失败后保留可恢复状态：编辑缓冲、脏状态、未保存设置、重试入口；不得因失败清空用户输入。
- 错误文案不插值凭据、token、文件内容、终端输出或剪贴板内容；长错误截断并提供详情入口，详情同样经过脱敏。

### 通知类别与时长

```rust
pub enum NotificationKind { Info, Success, Warning, Error, Progress, Undo }

pub enum DurationPolicy {
    Default,                 // 成功 3s / 警告 4s / 错误 5s
    Fixed(Duration),
    Sticky,                  // 进度或必须手动关闭
}

pub struct NotificationSpec {
    pub key: NotificationKey,
    pub kind: NotificationKind,
    pub title: MessageArgs,
    pub body: Option<MessageArgs>,
    pub action: Option<NotificationAction>,
    pub duration: DurationPolicy,
    pub dedupe: Option<DedupeKey>,
}
```

| 来源 | 覆盖 | 说明 |
|---|---|---|
| 默认 | 信息 4s、成功 3s、警告 4s、错误 5s | 悬停或聚焦暂停倒计时，移出/失焦继续 |
| Explorer / Agent 文件 toast | 2s | 批量操作完成等短反馈 |
| 撤销 | 5s | 可操作通知，按钮可键盘聚焦，`Enter`/`Space` 触发 |
| 编辑器错误 | 8s | 保存/外部冲突等需阅读的错误 |
| 进度 | 粘性 | 完成后转为成功/错误，不自动消失直到终态 |

- 同一 `DedupeKey` 的通知合并；错误通知不得因去重被丢弃，只能合并正文并刷新时长。
- 通知正文为纯文本，不可点击、不可拖动；关闭按钮名称为“关闭通知”。通知不抢焦点，不因项目切换清空。
- 系统通知与任务栏闪烁不在本表：仅在窗口失焦且收到 CLI 状态事件时触发，类别为任务完成、等待输入、权限请求、错误；正文格式为 `<项目名> · <类别> · <时间>`，按事件键去重。系统通知无点击跳转，任务栏闪烁为唯一回窗引导。

### 焦点与键盘索引

- 命令层序、重绑、焦点意图、覆盖层栈、IME 门控与键盘/焦点原语见 `design/ux-framework.md`。
- 跨域硬约束：布局恢复不抢焦；显式打开/激活聚焦；项目切换恢复该项目运行期焦点；`Ctrl+Tab` 焦点逃逸；关闭焦点面板按同组后继规则转移；`Ctrl+W` 无激活面板时不消费。
- 侧栏视图快捷键全局生效并优先于终端输入；终端保留键优先于一切命令；快捷键录制期间全局命令暂停。
- 设置中心快捷键页只列出可重绑命令；面板固定键冲突在对应面板文档定义。

### 键盘、IME 与 CJK 索引

- 键盘可达、焦点环、角色/名称、实时播报、减少动效、200% 文本缩放与 24×24 命中区见 `design/ux-framework.md`。
- IME 组合期间零命令匹配；终端只在提交时写入 PTY；CJK 以字素簇处理；真实输入法候选窗为人工验收面。
- 所有对话框/菜单/通知关闭按钮可键盘到达；模态焦点陷阱关闭后回到触发控件。

### 性能预算

| 对象 | 预算 | 说明 |
|---|---|---|
| 前台帧 | 16.7ms（60Hz） | 单次输入派发与状态更新前台工作不超过 4ms；超时走后台 |
| 项目切换 | 不随面板数量线性增长 | 只切换可见性/激活态/焦点，不重建面板、不迁移 PTY |
| 布局落盘 | 2s 去抖 | 页签增删、拆分/合并、拖动、尺寸变化立即写内存 |
| 侧栏落盘 | 500ms 去抖 | 宽度、比例、槽位、视图状态、活动栏顺序 |
| 终端 cwd/编号 | 300ms 去抖 | 自定义标题立即写 |
| 设置落盘 | 2s 去抖 | 显式提交、导入、恢复默认、关闭冲刷不等待去抖 |
| 关闭链 | 每阶段 3s | 设置与布局冲刷 → 并发终止 PTY → 保存项目数据 |
| 终端 PTY | 上限 64 | 超出显示失败态，不无界创建 |
| 终端回滚 | 默认 10000 行，范围 1000–100000 | 只影响新终端 |
| 编辑器 | 10MB 提示、50MB 只读 | 大文件禁用重计算与自动保存 |
| Diff | 1MiB 警告、10MiB 只读 | 按字节计 |
| Markdown | 5MB/100k 行降级源码、50MB 不渲染 | 资源上限 |
| GitShow | 1,000,000/10,000,000 字节分级 | 只读 |
| 设置载荷 | 1MiB | 超过进入只读，拒绝写入 |
| 文件事件合并 | 约 300ms | 编辑器保存后通知侧栏/Commit 刷新 |
| Diff 重算 | 默认 300ms，范围 100–2000ms | 外观页配置 |

- 大文件、大树、多终端、多面板必须有明确降级：虚拟化、按需加载、只读、折叠或禁用重计算；不得无界占用内存或无限后台任务。
- 渲染不可用时静默降级软件渲染；性能持续不达标时只提示一次。
- 隐藏项目停止持续绘制；终端输出继续接收，界面更新可节流，切回时以当前终端状态重绘。

### 后台任务生命周期

```rust
pub enum TaskScope {
    App,
    Project(ProjectId),
    Panel(PanelId),
    SidebarView(SideViewId),
    Overlay(OverlayId),
}

pub struct TaskToken { pub id: TaskId, pub scope: TaskScope, pub generation: u64 }

pub trait TaskService {
    fn spawn(&self, scope: TaskScope, name: &'static str, task: TaskFuture) -> TaskHandle;
    fn cancel_scope(&self, scope: TaskScope);
    fn cancel_all(&self);
}
```

| 作用域 | 典型任务 | 取消/暂停条件 |
|---|---|---|
| App | 更新检查、托盘同步、Agent 全局监听 | 进程退出；托盘禁用时停止同步 |
| Project | 项目布局冲刷、项目级 Git 状态、根目录监听 | 项目移除；项目隐藏时暂停 UI 刷新，保留数据任务 |
| Panel | PTY 读取、编辑器文件监听、Diff 计算、GitShow 读取 | 面板关闭；项目隐藏不取消 PTY |
| SidebarView | Explorer 扫描、Commit 刷新、导航历史刷新、Agent 文件树监听 | 视图隐藏或项目隐藏时停止扫描；显示时重建并刷新 |
| Overlay | 通知倒计时、撤销窗口 | 覆盖层关闭 |

- Agent 全局目录的底层监听是 App 作用域，项目切换不重启；侧栏视图只订阅事件，隐藏时丢弃 UI 更新并在显示时重读快照，避免视图层重复监听。
- 同一 `(scope, task key)` 同时最多一个在飞任务；重复请求合并为一次或标记“需要重跑”，不做无界并发。
- 结果回前台必须携带 `generation`；作用域已取消或代数不匹配的结果丢弃，不得更新已销毁面板。
- 面板关闭、项目移除、视图隐藏后，相关监听、定时器和计算任务必须停止；不得留下孤儿任务。
- 阻塞 I/O、网络、子进程等待与重计算不在 GPUI 前台线程执行；走后台执行体或专用线程。
- 关闭链先取消 UI 任务，再冲刷、终止 PTY、保存项目数据；PTY 终止有系统进程树兜底。

### 安全边界索引

| 边界 | 所有者 | 契约要点 |
|---|---|---|
| 路径允许根与真实路径校验 | `design/security.md` | 读根：项目根、仓库实际根、已登记 Agent 目录；写根：项目根、已登记 Agent 目录；符号链接/联接按真实目标校验 |
| URL 与终端 OSC | `design/security.md` | 外部链接只允许 `http`/`https`/`mailto`/`tel`；OSC 8 只允许前三者；OSC 52 只写不读、默认仅聚焦终端、上限 1MB |
| Markdown 清洗 | `design/security.md` | 无脚本/事件/表单/内嵌框架/对象/嵌入；远程图片不自动加载；远程字体/样式/脚本永不加载 |
| 进程启动 | `design/security.md` | 结构化参数、无 shell 字符串拼接；自定义 Shell 只接受可执行文件路径 |
| 凭据域 | `design/security.md` | 只经 `SecretStore` 门面与 Windows 凭据管理器；DTO/日志/快照不出现凭据值 |
| 导入导出 | `design/security.md` | 只限应用设置；校验、预览、备份、原子写；不含 Hooks/CLI 配置/凭据/输出 |
| 删除 | `design/platform-windows.md` | 统一回收站；不可用回退永久删除并说明；无 `Shift+Delete` |
| 更新安装 | `design/build-and-release.md` | 资产四重校验、SHA-256、事务 handoff、commit 唯一授权 |

### i18n 与主题 token 索引

- 界面语言 `System`/`English`/`简体中文`，typed `Message` 编译期合同，格式化占位符字面替换；详见 `design/theme-and-i18n.md`。
- 语义 token 覆盖颜色、间距、字号、圆角、动效与终端 ANSI 调色板；组件不得硬编码颜色；详见 `design/theme-and-i18n.md`。
- 所有主题、透明度、壁纸、密度组合必须通过 主题可读性检查；不满足时回退最近可用值并提示。

### 跨组件不变量

- 面板类型与侧栏视图是封闭枚举，单一注册点；未知面板类型恢复时丢弃并聚合警告，未知侧栏视图静默丢弃。
- 每项目每路径只有一个编辑器视图；文件打开统一经路由；Commit 来源不产生后缀视图。
- Commit、Diff、GitShow、文件浏览器统一使用项目根所在 Git 仓库实际根；不自动初始化仓库。
- 焦点、脏状态、关闭守卫、通知、单实例与持久化去抖只在所有者文档定义，其他域引用。
- 错误通道、审计通道与凭据域是安全单点；领域不得自建第二套日志、错误信封或凭据存取通道。

### 验收与测试

- 跨域验收直接执行 `acceptance.md` 的“跨域验收”表；本设计提供实现落点与测试接缝，不另立判据。
- 错误分类到反馈层级的映射用表驱动用例覆盖；通知时长、悬停暂停、去重与风暴合并用假时钟覆盖。
- 后台任务作用域取消、代数失配、同键合并用可控执行体覆盖；关闭链用假时钟覆盖各阶段超时。
- 安全、主题、i18n 的测试矩阵分别见对应设计文档。
