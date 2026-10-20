# 迁移总表

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 职责：定义旧模型退役、旧能力删除、数据与单实例命名空间隔离、旧测试替换和迁移顺序；各域目标设计由对应设计文件拥有，本节只写迁移契约。

### 迁移原则

- **单一状态权威**：新项目/布局/面板模型是唯一工作区状态；过渡期不得让旧 `SltermWorkspace` 与新模型同时服务同一窗口。
- **引擎保留、状态重建**：`slterm_terminal`、文件编辑器、Markdown 渲染、Diff 引擎、Git、补全引擎、加密原语、更新下载/交接、通知漏斗等已验证引擎保留；旧 tab/pane/dock/session/workspace 状态模型整体废弃。
- **零数据迁移**：新代码只读写新状态根；旧 `settings.json`、`session.json` 等文件不读、不删、不视为损坏，也不提供导入通道。
- **无适配层**：不为旧模型、旧 Runtime API、旧设置键、旧备份格式建立双向适配；允许 breaking change，过渡期允许 app crate 不可编译。
- **删除即验收**：每个旧域删除前必须已有目标态实现与对应验收；无替代的能力先删除并登记为明确不做，不能以“暂时保留”长期存在。
- **能力删除不可暗留**：Image、Merge、用户主题、Markdown 实时块编辑、提示符跳转不得以隐藏设置、快捷键或恢复分支回流。

### 逐域处置表

| 域 | 旧代码/类型 | 处置 | 目标落点 | 隔离与删除时机 |
|---|---|---|---|---|
| 终端内核 | `slterm_terminal` 的 grid、VT、PTY、输入编码、选择、渲染快照 | 保留；删除提示符跳转 API 与提示符行标记存储，保留 OSC 133 命令生命周期与退出码解析 | `slterm_terminal` | 长期保留；提示符跳转随能力删除同步删除 |
| 终端视图 | `slterm_app/src/gpui_terminal` 的 pane 视图、事件泵、会话泵 | 改造为终端面板 host；保留数学覆盖层、补全、AI 修复条接线 | 终端面板 | 新终端面板验收后删除旧 pane/tab 接线 |
| 布局树 | `slterm_split` 的 `SplitTree<PaneId>`、比例与导航纯函数 | 改造上移为项目布局几何树，叶类型改为 `PanelGroupId` | `slterm_workspace` + `slterm_split` | 新布局持久化与拖拽验收后删除旧 crate |
| 设置值模型 | `slterm_settings` 的 `RawSettings`/`RuntimeSettings`、键注册表、主题目录、主题可读性检查 | 保留 JSON 值层、路径权威、主题与校验；重建目标设置域与键注册表 | 新设置模型 | 新设置页全量接线后删除旧键与旧转换 |
| 设置写通道 | `settings_store`、`atomic_file`、`audit` | 保留；升级为单一 flush coordinator + 串行写队列 + generation 防旧快照覆盖 | 持久化协调器 | 长期保留 |
| 设置 UI | `app_shell/settings_pane*`、主题编辑器、旧导航与页面注册 | 删除重建；新增项目内设置中心与应用页 | 设置中心/应用页 | 新页面注册表与保存模型验收后删除 |
| 旧工作区 | `app_shell/workspace` 的 `SltermWorkspace`、`TabEntry`、`WorkspaceTab`、`TerminalPane`、旧分屏与焦点 | 删除；新壳按项目/面板组/面板重建 | 工作区壳 | 新壳、全部面板类型、侧栏迁移验收后删除 |
| 旧会话 | `session.rs`、`session_restore.rs`、`session.json`、`session.crashed.json` | 删除；项目列表、布局、侧栏、终端状态由新持久化承接 | 持久化协调器 | 新持久化切换并通过崩溃恢复验收后删除 |
| 侧栏 | `app_shell/workspace/sidebar.rs`、`side_panel` 视图 | 保留视图引擎；双槽固定模型改为封闭侧栏视图注册与状态矩阵 | 侧栏框架 | 新侧栏视图全部迁移后删除旧状态 |
| 文件编辑器 | `app_shell/file_editor` 文档、搜索、折叠、Git 行标、保存 | 保留引擎；删除渲染态 live edit、inline edit 与相关命令 | 编辑器面板 | 编辑器面板验收后删除旧 tab 宿主与 live edit |
| Markdown | `markdown/` 渲染器、公式、资源加载 | 保留渲染与公式；Mermaid 首版不渲染，按源码显示；删除渲染态块编辑接线 | Markdown 面板 | 编辑器面板验收后删除旧双态宿主 |
| Diff | `app_shell/diff_tab.rs` | 保留双栏/行内、折叠、同步滚动；新增冲突快照只读模式；删除 Merge 路由 | Diff 面板 | Diff 面板验收后删除旧 CodeTab 模式 |
| Merge | `app_shell/merge_tab.rs`、三栏冲突采用流程 | 删除；冲突处理由终端 Git 工作流承接，冲突对比由 Diff 快照承接 | 无 | 随旧 CodeTab 删除 |
| Image | `app_shell/doc_tabs.rs` 的 `ImageTabView`、图片打开路由 | 删除；图片/二进制交系统默认程序或错误提示；不新增预览面板类型 | 文件打开路由 | 立即删除 |
| GitShow | `app_shell/code_tab.rs` 的 GitShow 只读模式 | 保留只读读取与渲染；适配 GitShow 面板 | GitShow 面板 | 新面板验收后删除旧模式 |
| Git | `git/`、`git_completion.rs`、`git_worktree.rs` | 保留唯一执行入口；Commit/Diff/GitShow/文件树统一实际仓库根；worktree 供 agent fork | 各面板与 Git 服务 | 长期保留 |
| 文件树/Commit/Agent 文件 | `side_panel`、`agent_dirs.rs`、文件操作与监听 | 保留引擎；项目根与 Agent 根作为允许根；侧栏视图改为封闭注册集 | 侧栏框架 | 新侧栏验收后删除旧视图宿主 |
| 主题库 | `theme_library` 的 document/store/package/preferences、自定义主题编辑器、`slterm_settings` 的 `custom_theme` 与主题覆盖键 | 删除；内置暗色主题目录并入 `slterm_settings`，Linear 默认，未知 id 回退 Linear | 主题设置域 | 立即删除；备份/导入导出不再含主题类别 |
| hook 桥 | `slterm_hook` 独立小进程、hook 转发协议 | 保留；只随面板身份环境变量改名与事件字段适配 | `slterm_hook` | 长期保留；不被 app 依赖 |
| AI Providers/助手 | `ai_providers.rs`、`ai_assistant.rs`、`assistant_answer/`、`provider_test.rs`、`secrets.rs`、`agent_host.rs`、`ai_sessions.rs`、`ai_agents.rs`、`ai_hook*` | 保留引擎与凭据纪律；标识从 `PaneId` 改为 `PanelId`；Provider 元数据与助手配置迁入应用设置域 | AI 服务 | 新应用页与模拟凭据测试通过后删除旧配置文件和旧设置入口 |
| 终端补全 | `completion.rs`、`completion/`、`suggest.rs`、`history.rs`、`directory_history.rs` | 保留引擎；来源收敛本地；缓存按面板；数据文件迁入新状态根 | 终端增强 | 终端面板验收后接线，随后删除旧缓存路径 |
| 数学覆盖层 | `terminal_math*`、`math/`、`gpui_terminal/math_overlay.rs` | 保留；只随面板 host 与设置开关适配 | 终端增强 | 长期保留 |
| 托盘/任务栏/开机启动/通知 | `tray.rs`、`taskbar.rs`、`platform/startup.rs`、`notify.rs`、`platform/notifications/` | 保留平台实现与线程模型；目标标识改为项目/面板；入口迁入应用页 | 应用服务 | 新壳事件模型验收后删除旧 `PaneId`/旧 tab 路由 |
| 更新链 | `update_check.rs`、`update_download/`、`update_proxy.rs`、`platform/update_installation.rs`、`platform/distribution.rs` | 保留检查/下载/校验/两阶段交接；状态路径与工作区快照 schema 改为目标态 | 更新服务 | 最后迁移；其他域稳定后接线并删除旧更新 UI |
| 加密备份 | `encrypted_backup.rs`、`encrypted_backup/recovery.rs` | 保留 AES-GCM/Argon2id/恢复点/原子写；类别与归档名改为新状态文件；旧格式拒绝 | 备份服务 | 新持久化 schema 冻结后重写类别与 UI |
| Runtime API | `runtime_api*`、`cli.rs`、`assets/runtime-api-v1.schema.json`、`runtime-control-api.md`、随包 skill | 整体重写为 v2 资源模型；不保留 v1 兼容层 | Runtime API v2 | 新模型与壳稳定后重写，v2 conformance 通过后删除 v1 |
| i18n | `i18n/` typed 键与中文/英文目录 | 保留构建期键合同；新增应用页/更新/备份键，删除旧能力键 | i18n | 长期保留 |
| 日志/panic | `logging.rs`、`panic.rs` | 保留 logger 与 panic 钩；数据路径改为新诊断目录 | 诊断服务 | 长期保留 |
| 后台任务 | `background_tasks.rs`、`plan_balance/` | 保留注册表、订阅、重入与调度；任务配置接入应用页 | 应用页/任务调度器 | 长期保留 |
| 窗口 | `windowing`、`app_shell/windowing` | 重建单窗口 shell；窗口几何持久化改为目标态 | 单实例与窗口服务 | 新壳接管后删除旧窗口状态 |

新建域与依赖：

| 新建域 | 职责 | 依赖 |
|---|---|---|
| 新 core crates | `slterm_workspace`（项目/布局/面板）、`slterm_sidebar`（侧栏/树/Commit 模型）、`slterm_agents`（AgentKind/会话元数据）、`slterm_hooks`（Hooks schema）、`slterm_runtime_protocol`（协议类型） | 无 app 依赖；可复用 `slterm_split` 纯函数后再删除旧 crate |
| 面板宿主注册点 | 面板类型封闭枚举、`Panel` 生命周期、焦点落点、关闭守卫接缝、扩展点 | 新 core 领域 crate |
| 持久化协调器 | 设置/布局/侧栏/窗口/项目数据的 flush、串行写、generation、崩溃快照 | `slterm_settings` 路径与值层、`atomic_file` |
| 设置中心与应用页注册表 | 页面描述、作用域、保存模式、无项目浮层 | 设置模型、持久化协调器 |
| 应用服务 | 托盘、任务栏、开机启动、通知、诊断、更新入口 | 工作区壳、通知平台、更新引擎 |
| Runtime API v2 | 资源快照、稳定 ID、传输/鉴权/发现、CLI/skill/audit/conformance | 新 core 领域 crate、工作区壳、单实例服务 |
| 备份服务 | 新类别归档、敏感键闸、恢复点与撤回 | 持久化协调器、AI 服务 |
| 更新服务 | 新状态路径、两阶段交接、v2 工作区快照 | 持久化协调器、应用页、发布链 |

### 旧模型隔离

- 旧 UI 模块统一移入 `slterm_app/src/legacy/`，以 `legacy-shell` feature 门控；默认关闭后旧代码不参与编译，避免新代码误依赖。
- 新 core 领域 crate 不得依赖 `slterm_app`、`slterm_split` 旧布局类型或旧 `Session`；依赖方向由 `architecture/dependencies.toml` 与 `scripts/check_architecture.py` 强制。
- 新增源码门禁 `check_legacy_symbols`：禁止 `legacy` 之外的模块引用 `SltermWorkspace`、`WorkspaceTab`、`TerminalPane`、`SplitTree<PaneId>`、`TabEntry`、`Session`、`TabSession`、`LayoutSession`、`ImageTabView`、`MergeTabView`、`ThemeLibraryStore` 等旧符号；allowlist 随删除逐步清空。
- 旧持久化写入在切换时关闭：新壳启动后不再创建或更新 `session.json`、旧 `settings.json`、旧 `providers.json`、旧 `runtime.port`；只读旧代码不参与运行。
- 旧 Runtime v1 投影不桥接新模型；新 RuntimeHub 只发布 v2 快照。旧 v1 服务端在新壳接管时停止启动，不保留双服务端。
- 旧/新窗口可开发期并存，但发布安装覆盖旧二进制；应用内不存在并行模型选择开关。

### 逐域删除时机

删除触发条件统一为：目标态实现通过验收、生产符号引用清零、数据切换完成、旧测试映射已执行。按以下顺序推进：

1. 无替代能力先删：Image、Merge、用户主题、Markdown 实时块编辑、提示符跳转。
2. 新项目/布局/面板模型与持久化上线，删除旧 `session`、旧 `SltermWorkspace`、旧 `TabEntry`/`TerminalPane` 与旧布局树。
3. 新设置中心与应用页上线，删除旧 `settings_pane`、旧设置键转换与旧主题编辑器。
4. 新侧栏、导航、文件浏览、Commit、Agent 文件上线，删除旧 `side_panel` 与旧双槽状态。
5. 新 Runtime v2 通过 conformance，删除 `runtime_api` v1、旧 CLI、旧 schema/docs/skill。
6. 备份与更新最后迁移；删除旧备份类别、旧更新状态路径与旧更新 UI 接线。
7. 删除 `legacy-shell` feature 与空 allowlist；此后仓库不再存在旧模型符号。

### 旧测试删除映射

删除旧测试前按表建立替换；无替换项必须在 `acceptance.md` 的“明确不做”中有对应边界，不得无登记删除。

| 旧测试面 | 覆盖的旧行为 | 规格/设计承接 | 新测试落点 |
|---|---|---|---|
| `app_shell/ui_tests/focus_tests.rs` | 旧 tab/pane 焦点与关闭后转移 | 工作区壳焦点模型 | 新工作区壳焦点测试：恢复不抢焦、`Ctrl+Tab` 循环、关闭后落点 |
| `app_shell/ui_tests/sidebar_views_tests.rs` | 固定双槽、视图替换 | 侧栏框架 | 新侧栏显隐、槽位、视图快捷键与状态矩阵测试 |
| `app_shell/ui_tests/file_tree_tests.rs` | 文件树选中、批量、剪贴板 | 文件浏览器/Agent 文件 | 保留并适配项目根与 Agent 根；新增写边界测试 |
| `app_shell/ui_tests/file_editor_tests.rs`、`file_editor_chrome_tests.rs` | 编辑器宿主、脏关闭、保存 | 编辑器面板 | 保留文档引擎测试；新增面板关闭守卫与去重测试 |
| `app_shell/ui_tests/workspace_export_tests.rs` | 旧工作区导出文件 | 不恢复该能力 | 删除；设置导入导出测试改由应用页承接 |
| `app_shell/ui_tests/keybind_editor_tests.rs` | 旧快捷键页 | 设置中心快捷键页 | 新命令目录、保留键、冲突、录制暂停测试 |
| `app_shell/ui_tests/hooks_tests.rs` | 旧 Hooks 页 | 设置中心 Hooks 页 | 保留 Hooks 引擎测试；新增项目作用域测试 |
| `app_shell/ui_tests/backup_tests.rs` | 旧备份类别与导出 UI | 加密备份 | 新类别、旧格式拒绝、恢复点与撤回测试 |
| `app_shell/ui_tests/theme_tests.rs` | 用户主题、主题包 | 仅内置暗色主题 | 内置主题与 主题可读性测试；用户主题/包测试删除 |
| `app_shell/ui_tests/wallpaper_tests.rs` | 壁纸与透明度 | 外观页 | 保留并适配新设置键 |
| `app_shell/ui_tests/tray_tests.rs`、`taskbar_tests.rs` | 旧 pane 托盘/任务栏 | 应用服务 | 新项目/面板路由、菜单、进度清除测试 |
| `app_shell/ui_tests/update_tests.rs` | 旧更新弹窗与交接 | 更新服务 | 保留更新引擎测试；新增应用页入口与 v2 快照恢复测试 |
| `app_shell/ui_tests/runtime_control_tests.rs`、`runtime_projection_tests.rs` | v1 命令与快照 | Runtime API v2 | v2 资源模型、稳定 ID、幂等与事件测试 |
| `slterm_app/tests/runtime_conformance.rs` | v1 外部契约 | Runtime API v2 | v2 conformance 与真实进程条件用例 |
| `session.rs` 内测试 | 旧 session.json 快照/恢复 | 持久化协调器 | 新项目索引、布局、去抖、损坏隔离测试 |
| `slterm_split` 测试 | 旧 `PaneId` 分屏 | 新布局树 | 迁移纯函数并改叶类型；删除旧 ID 绑定测试 |
| `app_shell/merge_tab.rs` 测试 | 三栏合并 | 能力删除 | 删除；无替换 |
| Image 相关测试 | 图片预览 | 能力删除 | 删除；文件路由改为系统程序/错误测试 |
| Markdown live edit 测试 | 渲染态块编辑 | 能力删除 | 删除；保留源码/渲染两态测试 |
| 提示符跳转测试 | `slterm_prompt_jump` | 能力删除 | 删除；新增“无跳转入口/无按键”缺失测试 |
| `ai_providers`/`ai_assistant`/`secrets` 测试 | 旧 PaneId/旧配置文件 | AI 服务 | 保留并改 ID/路径；新增应用页与凭据边界测试 |
| `terminal_math`/补全测试 | 数学覆盖层、补全引擎 | 终端增强 | 保留；新增面板 host 与开关测试 |

### 旧数据文件忽略

- 旧根目录为 `%APPDATA%\slterm`（或 `SLTERM_CONFIG_DIR` 指向的目录）。新状态根固定为其下 `state/` 子目录，因此旧文件与同名新文件不会碰撞。
- 新代码启动、运行、关闭全程不读取、不重命名、不删除以下旧文件：`settings.json`、`settings.json.bak`、`session.json`、`session.json.bak`、`session.crashed.json`、`runtime.port`、`runtime.port.slterm-lock`、`providers.json`、`slterm_assistant.txt`、`slterm_assistant.txt.bak`、`slterm_assistant.corrupted.txt`、`history.jsonl`、`directory_history.json`、`update_state.json`、`updates/`、`backup-recovery/`、`themes/`、`ai_sessions.json`、`slterm-panic.log`、`slterm_debug.log` 及旧导入/导出文件。
- 旧文件存在不构成损坏、不触发隔离、不触发警告；新安装与升级从新状态默认值开始。
- 旧 `slterm-workspace.json` 导出文件不提供导入；设置导入只接受目标态应用设置 schema。
- 卸载与迁移均不清理旧目录；是否由用户手动删除不进入产品流程。

### 新数据目录与文件名

新数据目录与文件名由 `design/persistence.md` 单点定义；范围外能力的数据位置由 `design/out-of-scope/` 各自定义。迁移只保证：

- 目标态使用 `<数据根>/state/`；设置、项目索引、项目布局、终端簿、侧栏/视图状态、恢复快照与隔离区均在 `state` 根下。
- 日志、审计、更新下载与崩溃现场在 `%LOCALAPPDATA%\slterm`。
- Provider 元数据与助手配置并入应用设置域；凭据只存 Windows 凭据管理器。
- 旧目录不清理；新代码不读旧文件。

- 所有文件名在 `slterm_settings::paths` 单点定义；其他模块不得拼接状态根。
- `SLTERM_CONFIG_DIR` 语义保持“基础目录覆盖”；状态根恒为 `<基础目录>/state`，避免旧文件碰撞。
- `%TEMP%` 日志文件名改为 `slterm-app-<pid>.log`，并继续通过 `SLTERM_LOG` 发布。

### 旧单实例命名空间隔离

- 旧实例身份：`<基础目录>/runtime.port` 与 `runtime.port.slterm-lock`；新实例身份：`<状态根>/runtime.port` 与 `<状态根>/instance.lock`。新启动链不探测旧端口文件，不读取旧锁。
- 端点记录格式带协议前缀；新解析器只接受 `slterm.runtime/2`，旧格式、缺失字段、非法端口一律视为不可达。
- 显式 `SLTERM_RUNTIME_ENDPOINT` 仅接受 v2 格式；值非法时判定为“明确不可达”，不回落端口文件，避免连错实例。
- 第二实例启动只向 v2 端点发送 `app.handover`；旧实例不会收到移交，旧实例的第二启动也不会触发新实例。
- AUMID 保持产品身份，不作为单实例判定；发布安装覆盖旧二进制。开发期旧/新并存可能出现通知身份共用，不进入产品验收。

### 旧能力删除清单

| 能力 | 删除的代码与入口 | 替代/边界 | 验收 |
|---|---|---|---|
| Image 面板 | `ImageTabView`、`openable_in_app` 图片分支、图片页签恢复类型、相关测试 | 图片交系统默认程序打开或提示不支持；不新增预览类型 | 无 Image 面板类型、无图片打开路由、恢复丢弃未知类型 |
| Merge 三栏 | `merge_tab.rs`、冲突打开 Merge 路由、采用版本命令与测试 | Git 冲突处理由终端承接；冲突对比由 Diff 只读快照承接 | 无 Merge 入口与快捷键；冲突来源只打开 Diff 快照 |
| 用户自定义主题 | `theme_library` store/package/document/preferences、主题编辑器、导入导出、`themes/` | 仅内置暗色主题方案；Linear 默认；未知 id 回退 Linear | 无导入/导出/编辑器入口；主题列表只含内置方案并保持可读 |
| Markdown 实时块编辑 | `file_editor/live_edit.rs`、`inline_edit.rs`、`live_commands.rs`、渲染态输入规则与订阅 | 源码/渲染两态；渲染态只读，编辑统一进源码态 | 渲染态无输入、无块编辑命令、无 live edit 状态 |
| 提示符跳转 | `slterm_prompt_jump`、提示符行标记 deque、相关按键与测试 | 终端不提供按提示符跳转；保留 OSC 133 命令边界、标题、状态与退出码 | 无跳转 API、无按键绑定、无文档入口 |

### 依赖里程碑

- 项目与布局模型（ProjectId、PanelGroupId、PanelId、LayoutNode）。
- 持久化协调器（新状态根、串行写、flush、崩溃快照）。
- 工作区壳与面板宿主注册点。
- 设置模型与写通道、设置中心应用页。
- Runtime API v2（稳定 ID 与 v2 单实例移交）。
- 更新与备份依赖以上全部完成后迁移。

### 迁移顺序

1. 确定新 core 领域 crate 与 `slterm_settings::paths` 新状态根；建立 `legacy-shell` 门控与符号门禁。
2. 删除无替代能力并同步删除测试；清理旧设置键、旧主题库与旧打开路由。
3. 上线项目/布局/面板模型与持久化，接管启动恢复；切换旧 `session` 写入。
4. 上线终端、编辑器、Markdown、Diff、GitShow 面板与侧栏；删除旧 `SltermWorkspace`、`TabEntry`、`TerminalPane`、旧布局树。
5. 上线设置中心与应用页，删除旧设置 UI 与旧配置转换。
6. 上线 Runtime v2、CLI、skill、schema 与 conformance，删除 v1。
7. 重写备份类别与 UI；重接更新链；完成发布链资产与预算。
8. 删除 `legacy-shell`、空 allowlist 与全部旧测试残留。

### 测试与验收

- **旧数据不动**：预置全部旧文件后启动、操作、关闭新应用；旧文件逐字节不变，新状态根按默认值创建，无损坏提示。
- **命名空间隔离**：旧 `runtime.port` 指向存活旧进程或死进程时，新应用仍按新 owner 锁启动；旧格式显式端点被判定不可达。
- **符号门禁**：`check_legacy_symbols` 在旧符号引用非零时失败；删除完成后 allowlist 为空。
- **能力缺失**：Image、Merge、用户主题、Markdown 实时块编辑、提示符跳转不存在入口、快捷键、设置键或恢复分支。
- **测试映射**：旧测试删除前对应新测试通过；无替换项在 `acceptance.md` 有明确不做登记。
- **启动兼容**：旧文件存在不阻断启动；新状态文件损坏时只按新文件规则隔离并提示。
- **数据路径单点**：源码扫描禁止在 `slterm_settings::paths` 之外拼接状态根或旧文件名。
- **文档切换**：实现落地时同步更新 `.claude/docs/architecture/overview.md`、各路径 `CLAUDE.md` 与 `CONTEXT.md`；本轮只更新 `CONTEXT.md` 与 `docs/devPlan`。

### 不可自动化面

- 旧/新两个 GUI 进程并存的桌面行为、托盘图标归属与通知身份只能人工冒烟。
- 安装覆盖后旧数据目录残留、旧快捷方式与开始菜单项状态依赖安装器与系统外壳，人工核验。
- 大规模删除旧模块后的编译期错误只由 CI 编译与门禁兜底，无法用行为测试替代。
