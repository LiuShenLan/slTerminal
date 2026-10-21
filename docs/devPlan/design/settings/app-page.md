# 应用页

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 应用页是设置中心全局组最后一页，承载应用级、跨项目的基础设施配置：界面语言、更新、托盘、开机启动、通知、Runtime API、AI Providers/助手、诊断与终端增强开关。页面模型与保存框架复用设置中心；本篇只定义该页的配置节、数据边界与无项目浮层。

### 注册与页面结构

```rust
pub const APP_PAGE_ID: SettingsPageId = SettingsPageId("app");
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
- 连续型配置自动保存，控件即时作用于运行期状态；失败显示保存失败与重试，修改仍在本轮生效。
- 凭据、快捷方式、检查更新、连通性测试、备份/恢复是显式动作，立即提交，不进入自动保存去抖。
- 页面重挂载时输入草稿与行内错误丢失；已生效配置保留。

### 节级交付矩阵

| 节 | 规格内状态 |
|---|---|
| 页壳、全局组顺序、节序、自动保存 | 可用 |
| 通知开关（桌面通知、任务栏闪烁） | 可用 |
| 诊断 | 可用；动作与规则见 `../diagnostics.md` |
| Runtime API | 只读状态；显示“未迁移/不可用”，不提供开关 |
| 界面语言、更新、托盘与开机启动、AI Providers/助手、备份与恢复 | 占位/禁用；只读说明 `该能力由迁移版本提供` |
| 终端增强 | 禁用态 `尚未启用` |

- 占位节保留迁移后契约描述；规格内以禁用态呈现，不产生运行期副作用。

### 数据模型

应用页使用独立的 `app` 设置域，键注册进 `slterm_settings::keys`；不与其他域共享裸键：

```rust
pub struct AppPageSettings {
    pub language: LanguagePreference,          // System | English | Chinese
    pub updates: UpdateSettings,
    pub tray: TraySettings,
    pub notifications: NotificationSettings,
    pub providers: ProviderCatalog,             // 非敏感元数据；凭据按 provider ID 查凭据管理器
    pub assistant: AssistantSettings,
    pub enhancements: TerminalEnhancementSettings,
}

pub struct UpdateSettings {
    pub auto_check: bool,        // 默认 true
    pub auto_download: bool,     // 默认 false
}
pub struct TraySettings { pub enabled: bool }          // 默认 true
pub struct NotificationSettings {
    pub desktop_enabled: bool,   // 默认 true；只控制系统 toast
    pub taskbar_flash: bool,     // 默认 true；独立控制任务栏闪烁
}
pub struct AssistantSettings {
    pub enabled: bool,                 // 默认 false（隐私闸门）
    pub provider: Option<ProviderId>,  // 只存 provider ID，不持久化 SecretId
    pub ignored_exit_codes: Vec<i32>,
}
pub struct TerminalEnhancementSettings {
    pub vi_mode: bool,                   // 默认 false
    pub completion: bool,                // 默认 true
    pub completion_style: CompletionStyle, // 默认 inline；取值 inline/popup/hybrid
    pub math_overlay: bool,              // 默认 true
    pub ai_fix: bool,                    // 默认 false
}
```

- 设置值只存行为开关与非敏感参数；不存 API key、token 或凭据明文。`ProviderCatalog` 与 `ProviderEntry` 由 AI Providers 设计定义，只含 provider ID、显示元数据与提示；不持久化 `SecretId` 或凭据引用值。AI 助手只引用 provider ID。
- `LanguagePreference::{System, English, Chinese}` 为唯一枚举；存储值 `system/en-US/zh-CN`，标签 `简体中文`。系统档按 locale 协商；切换后界面即时重渲染，不重建窗口。该节属于迁移能力；规格内应用页显示只读说明 `该能力由迁移版本提供`。
- 终端增强键：`terminal.enhancements.vi`、`terminal.enhancements.completion`、`terminal.completion.style`、`terminal.enhancements.math`、`terminal.enhancements.ai_fix`；默认 vi 关、completion 开、math 开、ai_fix 关；补全样式默认 inline。
- 更新、托盘与通知的默认值沿用现有产品默认；开机启动不保存布尔键，只读 `slTerminal.lnk` 实际存在性。若现有键与新域冲突，以新域为唯一写入目标，旧键不读。

### 界面语言

- 单选：系统 / English / 简体中文。
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

- `开机启动` 开关状态读取用户 Startup 文件夹中 `slTerminal.lnk` 的实际存在性；切换即幂等写入/删除，不写注册表、不需要管理员。
- 不持久化开机启动布尔键；打开/关闭失败时行内显示原因并回滚显示值。
- 与安装器管理同一快捷方式，不产生第二份启动项。

### 通知

- `桌面通知` 开关，默认开；`desktop_enabled` 只控制系统 toast 投递，不影响终端内状态、任务栏闪烁与导航树状态。
- `任务栏闪烁` 开关，默认开；`taskbar_flash` 独立控制闪烁，不受 `desktop_enabled` 影响；仅在窗口失焦且收到编码 CLI 状态事件时生效。
- 不提供每类事件的独立开关；类别、时长、去重与聚焦门控由 `../cross-cutting.md` 拥有，应用页只改两个开关。
- 开关即时作用于后续事件；已显示的通知不因关闭而撤回。

### Runtime API

- 规格内该节只读显示 `未迁移/不可用`，隐藏端点、协议版本与文档入口；迁移后恢复下述只读状态区。
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
- 删除 provider 时二次确认；删除只移除元数据与凭据管理器中的对应凭据，不触碰 CLI 自身配置。

助手（AI 修复建议）：

- `启用命令失败修复建议` 开关，默认关；隐私说明必须可见。
- 选择 provider、配置忽略的退出码列表；只持久化 provider ID，不保存 base URL、模型或 `SecretId`；凭据复用 provider。
- 生成建议前对终端输出尾部做脱敏与长度限制；建议只显示，不自动执行。
- 关闭后不发起请求、不渲染修复条；已显示的修复条立即清除。
- 请求失败显示可操作错误，不包含终端内容或凭据。

### 诊断

- 诊断动作、二次确认、清理范围与脱敏规则由 `../diagnostics.md` 独占；应用页只放入口，不定义第二套动作。
- 只读展示：应用版本、构建标识、数据目录、日志文件路径、设置加载状态（正常/损坏回退/只读）、项目数据加载状态、Runtime API 状态；路径经 `slterm_settings::paths` 的数据根/本地根访问器获取，页面不拼接品牌目录。
- 入口动作与 `../diagnostics.md` 一致：版本/构建、数据目录、日志目录、配置健康、复制诊断信息、打开数据目录、打开日志文件、通知自检、一键清理；可选导出诊断包与环境自检。
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
- AI 修复条的 provider 选择在“AI Providers/助手”节，不在增强节重复。

### 保存模式、状态与错误

- 保存模式：自动保存只覆盖连续型配置；凭据、快捷方式、检查更新、连通性测试、备份/恢复是显式动作，立即提交。
- 保存状态统一显示“保存中/已保存/保存失败 + 重试”；失败提示“设置保存失败，修改仅在本次运行中生效”。
- 输入校验：
  - Provider base URL 必须是 `http`/`https` 绝对地址；
  - Provider 模型名非空；
  - 忽略退出码必须是整数且去重；
  - 开机启动路径由平台适配器校验。
- 校验失败行内红字，不提交、不弹模态、保留输入。
- 页面关闭或应用退出前先冲刷；失败进入设置中心的统一关闭守卫。
- 设置文件损坏/只读时顶部警示条与写入禁用复用设置中心壳；应用页不另设提示通道。

### 凭据边界

- 凭据值只经 `secrets`/Windows 凭据管理器门面；provider 模块只读元数据文件，不读凭据库以外的位置。
- 类型层不向设置 DTO、页面状态、导出包、日志或错误消息暴露 token。
- 测试与文档只使用假值占位符（`sk-test` 形态）；真实凭据不得进入 git 追踪文件。
- 导出设置不包含凭据值或凭据引用；导入不写入凭据。
- 诊断信息不包含 key、hint 之外的凭据内容；hint 只用于识别，不可逆推。

### 无项目浮层复用

- 无活跃项目时，配置钮与 `Ctrl+,` 打开全局设置浮层；浮层只注册应用页，左导航可省略或仅显示单页，页头显示“应用”作用域。
- 浮层与项目内设置中心复用同一 `SettingsCenterView`、`SettingsPageCx`、页面组件与保存协调器；差异只在 `SettingsSurface::GlobalOverlay`：无项目上下文、无 Hooks、无项目级页内参数、无布局持久化。
- 打开浮层不创建项目、项目布局、面板组或面板；不改变主区与活跃项目。
- 浮层为固定定位覆盖层，遮罩阻止背景交互，焦点陷阱在浮层内；`Esc`、关闭按钮或取消关闭，关闭后焦点回到配置钮或发起控件；重复触发只聚焦已有浮层。
- 底部固定提供 `导入设置`、`导出设置`、`恢复所有设置默认`。
- 项目激活后自动关闭浮层。
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
  - 节级交付：规格内可用节可交互；占位节禁用并显示 `该能力由迁移版本提供`；终端增强显示 `尚未启用`。
  - 语言：`System/English/Chinese` 三档切换、`system/en-US/zh-CN` 存储、即时刷新。
  - 更新：开关默认值、手动检查后台执行与结果映射、失败不阻塞。
  - 托盘/开机启动：适配器假实现、失败回滚、开机启动只读 `.lnk` 实际存在性。
  - 通知：`desktop_enabled` 只影响后续 toast，`taskbar_flash` 独立；应用页不定义时长或去重。
  - 保存模式：连续型配置自动保存；凭据、快捷方式、检查更新、连通性测试、备份/恢复立即提交。
  - Runtime API：规格内只读显示 `未迁移/不可用`；迁移后状态展示不含 token。
  - Providers：元数据 CRUD、凭据设置/清除只调用凭据门面、测试连接失败不泄露 key、导出不含秘密。
  - 助手：默认关、只持久化 provider ID、关闭后不发请求、校验与脱敏。
  - 诊断：只放入口；动作集、二次确认与清理范围归 `../diagnostics.md`。
  - 备份与恢复：导出默认只选应用设置、导出不含凭据/Hooks、恢复前预览与确认、撤回需要口令。
  - 增强：默认值（vi=false、completion=true、math=true、ai_fix=false）、补全样式、禁用态（迁移前）、迁移后即时接线。
  - 无项目浮层：不创建项目/布局、只含应用页、底部提供导入/导出/恢复默认、项目激活后自动关闭、保存与关闭守卫复用。
