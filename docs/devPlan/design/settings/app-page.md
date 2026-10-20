# 应用页

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 应用页是设置中心全局组最后一页，承载应用级、跨项目的基础设施配置：界面语言、更新、托盘、开机启动、通知、Runtime API、AI Providers/助手、诊断与终端增强开关。页面模型与保存框架复用设置中心；本篇只定义该页的配置节、数据边界与无项目浮层。

### 注册与页面结构

```rust
pub const APP_PAGE_ID: PageId = PageId("app");
pub const APP_PAGE: SettingsPageDef = SettingsPageDef {
    id: "app",
    group: SettingsGroup::Global,
    order: APP_ORDER,            // 排在终端页之后
    title: Message::SettingsSidebarApplication,
    search_terms: "application app 应用 language 语言 update 更新 tray 托盘 startup 开机启动 notification 通知 runtime api providers ai assistant 助手 diagnostics 诊断 terminal enhancements 终端增强",
    save_mode: SaveMode::Auto,
    scope: SettingsScope::Application,
    factory: app_page,
};
```

- 全局组顺序：快捷键 → 后台定时任务 → 外观 → 终端 → 应用。
- 页面分节顺序固定：界面语言、更新、托盘与开机启动、通知、Runtime API、AI Providers/助手、诊断、备份与恢复、终端增强。
- 页头显示“应用”作用域与保存模式“自动保存”；右侧“恢复本页默认”。
- 所有控件即时作用于运行期状态；停止操作 2 秒后按应用域去抖写盘；失败显示保存失败与重试，修改仍在本轮生效。
- 页面重挂载时输入草稿与行内错误丢失；已生效配置保留。

### 数据模型

应用页使用独立的 `app` 设置域，键注册进 `slterm_settings::keys`；不与其他域共享裸键：

```rust
pub struct AppPageSettings {
    pub language: LanguagePreference,          // System | EnUs | ZhCn
    pub updates: UpdateSettings,
    pub tray: TraySettings,
    pub startup: StartupSettings,
    pub notifications: NotificationSettings,
    pub runtime_api: RuntimeApiViewSettings,   // 只读展示与入口
    pub providers: ProviderCatalog,             // 非敏感元数据；凭据只存 SecretId
    pub assistant: AssistantSettings,
    pub enhancements: TerminalEnhancementSettings,
}

pub struct UpdateSettings {
    pub auto_check: bool,        // 默认 true
    pub auto_download: bool,     // 默认 false
}
pub struct TraySettings { pub enabled: bool }          // 默认 true
pub struct StartupSettings { pub launch_at_login: bool } // 默认 false
pub struct NotificationSettings {
    pub desktop_enabled: bool,   // 默认 true
    pub taskbar_flash: bool,     // 默认 true
}
pub struct AssistantSettings {
    pub enabled: bool,           // 默认 false（隐私闸门）
    pub base_url: String,
    pub model: String,
    pub ignored_exit_codes: Vec<i32>,
}
pub struct TerminalEnhancementSettings {
    pub vi_mode: bool,           // 默认 false
    pub completion: bool,        // 默认 true
    pub math_overlay: bool,      // 默认 true
    pub ai_fix: bool,            // 默认 false
}
```

- 设置值只存行为开关与非敏感参数；不存 API key、token、凭据引用值。`ProviderCatalog` 与 `ProviderEntry` 由 AI Providers 设计定义，只含显示元数据与 `SecretId` 引用。
- 语言值复用现有 `LanguagePreference`，系统档按 locale 协商；切换后界面即时重渲染，不重建窗口。该节属于迁移能力；迁移前应用页按规格显示只读说明 `该能力由迁移版本提供`。
- 更新、托盘、开机启动与通知的默认值沿用现有产品默认；若现有键与新域冲突，以新域为唯一写入目标，旧键不读。

### 界面语言

- 单选：系统 / English / 中文（简体）。
- 即时切换界面文案；不重启应用；打开中的页面、对话框与通知按新语言刷新。
- 语言不影响文件内容、Hooks 配置或 CLI 输出。
- 保存失败时提示“设置保存失败，修改仅在本次运行中生效”，保留本轮语言。

### 更新

- 显示当前版本与构建信息（只读）。
- `自动检查更新` 开关，默认开；关闭后仍可手动“检查更新”。
- `后台下载更新包` 开关，默认关；不授予安装权限；无可用资产或校验不足时不下载。
- “检查更新”在后台执行，按钮显示进行中并禁用；结果非阻塞展示：已是最新、发现新版本、检查失败（断网/限流/坏响应）。
- 更新下载与安装入口沿用更新链；应用页只提供开关与手动检查，不内嵌第二套更新协议。
- 网络跟随系统代理；提供 `仅手动检查` 开关；不提供应用内代理地址或代理凭据配置。
- 页面不显示 release token 或私有 URL；错误不含凭据。

### 托盘与开机启动

托盘：

- `常驻系统托盘` 开关，默认开；即时增删图标，托盘线程留驻。
- 关闭托盘不影响已有终端与 AI CLI 运行；托盘菜单的聚焦/退出动作归平台托盘服务。
- 无项目浮层中同样可用；配置为应用级。

开机启动：

- `开机启动` 开关，默认关；写用户 Startup 文件夹的 `slTerminal.lnk`，幂等重写，不写注册表、不需要管理员。
- 开关状态以实际快捷方式存在性为准；打开/关闭失败时行内显示原因并回滚显示值。
- 与安装器管理同一快捷方式，不产生第二份启动项。

### 通知

- `桌面通知` 总开关，默认开；关闭后不投递系统 toast，但终端内状态、任务栏闪烁与导航树状态不受影响。
- `任务栏闪烁` 开关，默认开；仅在窗口失焦且收到编码 CLI 状态事件时生效。
- 不提供每类事件的独立开关；任务完成、等待输入、权限请求、错误四类均按工作区通知契约处理。
- 通知时长使用统一契约：成功 3 秒、警告 4 秒、错误 5 秒；视图显式覆盖（Explorer/Agent toast 2 秒、撤销 5 秒、编辑器错误 8 秒）作为属性，不在应用页暴露。
- 通知去重、窗口聚焦门控与任务栏闪烁由唯一通知服务拥有；应用页只改总开关。
- 开关即时作用于后续事件；已显示的通知不因关闭而撤回。

### Runtime API

- 只读状态区：服务状态（运行中/不可用）、协议版本、监听地址与端口、随包文档入口。
- 不显示、不复制、不记录 128 位 token；不提供 token 查看或重置按钮。
- 提供“复制环境命令”（`slterm env`）与“打开 Runtime API 文档”；命令文本不含 token。
- 显示 agent 子进程端点注入状态（已注入/未注入/未知），用于诊断；不提供手动注入任意端点。
- 不提供关闭 Runtime API 的开关：它是子进程与 CLI 的控制面基础设施。若后续要求可关闭，需先定义关闭后 CLI/skill 的降级语义。
- Runtime v2 的资源模型（project / panel-group / panel）由 Runtime 设计拥有；应用页不展示或编辑资源 ID。

### AI Providers 与助手

Providers：

- 列出已配置 provider（种类、显示名、base URL、模型、`api_key_set`/`api_key_hint`）；支持添加、编辑、删除。
- 凭据只经 Windows 凭据管理器：设置/替换/清除 key 是独立动作；界面只显示“已设置”与提示串，永不读取或回显明文。
- Provider 元数据并入应用设置域，不新增独立 `providers.json`；应用设置、日志、错误、导出包均不含 key。
- “测试连接”在后台执行，只显示成功/失败与可操作原因；请求与日志不插值 key。
- 删除 provider 时二次确认；删除只移除元数据与凭据引用，不触碰 CLI 自身配置。

助手（AI 修复建议）：

- `启用命令失败修复建议` 开关，默认关；隐私说明必须可见。
- 配置 base URL、模型、忽略的退出码列表；不配置 key（复用 provider 凭据）。
- 生成建议前对终端输出尾部做脱敏与长度限制；建议只显示，不自动执行。
- 关闭后不发起请求、不渲染修复条；已显示的修复条立即清除。
- 请求失败显示可操作错误，不包含终端内容或凭据。

### 诊断

- 只读展示：应用版本、构建标识、数据目录、日志文件路径（`%LOCALAPPDATA%\slterm\logs`）、设置加载状态（正常/损坏回退/只读）、项目数据加载状态、Runtime API 状态。
- 动作：
  - `复制诊断信息`：版本、路径、状态与最近错误类别；不含凭据、文件内容、终端输出、剪贴板或项目路径以外的用户数据。
  - `打开数据目录`、`打开日志文件`：交给系统文件管理器/默认程序。
  - `运行通知自检`：仅在用户显式触发时投递一条测试通知，绕过前台门控与节流。
  - `一键清理日志/审计/更新下载/崩溃现场`：二次确认并列出将删除的范围；清理失败逐项可见；不删除设置、项目数据、布局或恢复快照。
- 诊断命令在后台执行；失败显示原因，不阻塞页面。
- 不提供任意命令执行、环境变量查看或注册表编辑入口。

### 备份与恢复

- 分节只承载入口与状态；加密格式、类别、敏感键与恢复点归 `design/out-of-scope/encrypted-backup.md`。
- `导出备份…`：类别复选、口令输入/确认（掩码）、目标路径选择、进度与结果；默认只选应用设置。
- `恢复备份…`：文件选择、口令、解密预览（类别、文件数、将覆盖路径）、破坏性确认、执行与重启提示。
- 恢复点撤回：显示路径与撤回入口，撤回需要口令；恢复点保留到下一次成功恢复或用户手动清理。
- 凭据、Hooks、CLI 配置、终端内容、剪贴板、崩溃快照不进入备份；口令不持久化，不写入日志或诊断。

### 终端增强开关

- 四个开关：vi 模式、终端补全、数学覆盖层、AI 修复条；全部应用级、即时生效。
- 关闭时对应增强不拦截按键、不扫描、不渲染覆盖层；打开立即按当前会话状态接线（vi 进入普通模式、补全下一键生效、数学覆盖下一帧、修复条下一次命令失败生效）。
- 这些能力属于范围外迁移，在规格范围完成后统一接入；应用页在迁移前仍显示开关，但以“尚未启用”禁用态呈现，避免用户误以为已生效。迁移完成后去掉禁用态。
- 完整键盘语义、性能与安全约束由终端增强设计拥有；应用页只提供开关，不提供命令、模型或 prompt 配置。
- AI 修复条的 provider/model 配置在“AI Providers/助手”节，不在增强节重复。

### 保存模式、状态与错误

- 保存模式：自动保存；所有开关与文本配置立即作用于运行期，2 秒去抖写盘。
- 保存状态统一显示“保存中/已保存/保存失败 + 重试”；失败提示“设置保存失败，修改仅在本次运行中生效”。
- 输入校验：
  - base URL 必须是 `http`/`https` 绝对地址；
  - 模型名非空；
  - 忽略退出码必须是整数且去重；
  - 开机启动路径由平台适配器校验。
- 校验失败行内红字，不提交、不弹模态、保留输入。
- 页面关闭或应用退出前先冲刷；失败进入设置中心的统一关闭守卫。
- 设置文件损坏/只读时顶部警示条与写入禁用复用设置中心壳；应用页不另设提示通道。

### 凭据边界

- 凭据值只经 `secrets`/Windows 凭据管理器门面；provider 模块只读元数据文件，不读凭据库以外的位置。
- 类型层不向设置 DTO、页面状态、导出包、日志或错误消息暴露 token。
- 测试与文档只使用假值占位符（`sk-test` 形态）；真实凭据不得进入 git 追踪文件。
- 导出设置不包含 provider 元数据文件中的凭据引用值之外的任何秘密；导入不写入凭据。
- 诊断信息不包含 key、hint 之外的凭据内容；hint 只用于识别，不可逆推。

### 无项目浮层复用

- 无活跃项目时，配置钮与 `Ctrl+,` 打开全局设置浮层；浮层只注册应用页，左导航可省略或仅显示单页，页头显示“应用”作用域。
- 浮层与项目内设置中心复用同一 `SettingsCenterView`、`SettingsPageCx`、页面组件与保存协调器；差异只在 `SettingsSurface::GlobalOverlay`：无项目上下文、无 Hooks、无项目级页内参数、无布局持久化。
- 打开浮层不创建项目、项目布局、面板组或面板；不改变主区与活跃项目。
- 浮层为固定定位覆盖层，焦点陷阱在浮层内；`Esc` 关闭，关闭后焦点回到配置钮或发起控件。
- 浮层中保存失败仍显示失败状态与重试；关闭前冲刷待写配置，失败时提示并允许取消关闭。
- 浮层不显示“所属项目”；搜索只覆盖应用页；页面目录不因无项目而变化。
- 项目内设置中心与浮层同时存在时，应用域配置共享同一设置模型；任一处保存成功后另一处读取最新值。

### 模块树与测试接缝

```text
slterm_app/src/panels/settings/pages/app/
  mod.rs
  language.rs
  updates.rs
  tray_startup.rs
  notifications.rs
  runtime_api.rs
  providers.rs
  assistant.rs
  diagnostics.rs
  enhancements.rs
```

- 页面壳只负责布局与控件；更新/托盘/开机启动/通知/Runtime/Providers/诊断全部经既有服务门面调用，页面不 spawn 子进程、不直接读凭据、不直接写文件。
- 测试接缝：
  - 注册与顺序：应用页排在终端之后，无项目浮层只含应用页。
  - 语言：三档切换、即时刷新、持久化。
  - 更新：开关默认值、手动检查后台执行与结果映射、失败不阻塞。
  - 托盘/开机启动：适配器假实现、失败回滚、以真实快捷方式为准。
  - 通知：总开关/闪烁开关只影响后续事件，不去重、不改固定时长。
  - Runtime API：状态展示不含 token；文档/环境命令动作可触发。
  - Providers：元数据 CRUD、凭据设置/清除只调用凭据门面、测试连接失败不泄露 key、导出不含秘密。
  - 助手：默认关、关闭后不发请求、校验与脱敏。
  - 诊断：复制内容脱敏、打开路径失败可感知、通知自检仅显式触发、一键清理需二次确认且不触碰设置/项目数据/快照。
  - 备份与恢复：导出默认只选应用设置、导出不含凭据/Hooks、恢复前预览与确认、撤回需要口令。
  - 增强：默认值、禁用态（迁移前）、迁移后即时接线。
  - 无项目浮层：不创建项目/布局、页面集合正确、保存与关闭守卫复用。
