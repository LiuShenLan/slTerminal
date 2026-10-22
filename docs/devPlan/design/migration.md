# 迁移总表

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 职责：定义旧模型退役、旧能力删除、数据与单实例命名空间隔离、旧测试替换和迁移顺序；各域目标设计由对应设计文件拥有，本节只写迁移契约。

### 迁移原则

- **单一状态权威**：新项目/布局/面板模型是唯一工作区状态；过渡期不得让旧 `SltermWorkspace` 与新模型同时服务同一窗口。
- **引擎保留、状态重建**：`slterm_terminal`、文件编辑器、Markdown 渲染、Diff 引擎、Git、补全引擎、加密原语、更新下载/交接、通知漏斗等已验证引擎保留；旧 tab/pane/dock/session/workspace 状态模型整体废弃。
- **零数据迁移**：新代码只读写新状态根；旧 `settings.json`、`session.json` 等文件不读、不删、不视为损坏，也不提供导入通道。
- **无适配层**：不为旧模型、旧 Runtime API、旧设置键、旧备份格式建立双向适配；允许 breaking change。垂直切片前新旧壳同仓互不引用，主线始终可编译；切片通过后先退役旧状态模型与写路径，旧 UI 宿主按域在替代实现验收后逐个删除，不保留 `legacy-shell` feature。
- **删除即验收**：每个旧域删除前必须已有目标态实现与对应验收；共享旧类型随最后一个引用它的域删除；无替代的能力先删除并登记为明确不做，不能以“暂时保留”长期存在。
- **能力删除不可暗留**：Image、Merge、用户主题、Markdown 实时块编辑、提示符跳转、无障碍专用能力不得以隐藏设置、快捷键或恢复分支回流。

### 逐域处置表

| 域 | 旧代码/类型 | 处置 | 目标落点 | 隔离与删除时机 |
|---|---|---|---|---|
| 终端内核 | `slterm_terminal` 的 grid、VT、PTY、输入编码、选择、渲染快照 | 保留；删除提示符跳转 API 与提示符行标记存储，保留 OSC 133 命令生命周期与退出码解析 | `slterm_terminal` | 长期保留；提示符跳转随能力删除同步删除 |
| 终端视图 | `slterm_app/src/gpui_terminal` 的 pane 视图、事件泵、会话泵 | 改造为终端面板 host；保留数学覆盖层、补全、AI 修复条接线 | 终端面板 | 旧状态模型退役后删除旧 pane/tab 接线（终端面板已验收） |
| 布局树 | `slterm_split` 的 `SplitTree<PaneId>`、比例与导航纯函数 | 改造；叶类型改为 `PanelGroupId`，纯函数继续复用 | `slterm_workspace` + `slterm_split` | 保留 crate；仅删除旧 `PaneId` 叶类型绑定与旧 ID 测试 |
| 设置值模型 | `slterm_settings` 的 `RawSettings`/`RuntimeSettings`、键注册表、主题目录、对比度校验与回退 | 保留 JSON 值层、路径权威与内置主题；删除对比度校验代码；重建目标设置域与键注册表 | 新设置模型 | 新设置中心验收后删除旧键与旧转换 |
| 设置写通道 | `settings_store`、`atomic_file`、`audit` | 保留；升级为单一 flush coordinator + 串行写队列 + generation 防旧快照覆盖 | 持久化协调器 | 长期保留 |
| 设置 UI | `app_shell/settings_pane*`、主题编辑器、旧导航与页面注册 | 删除重建；新增项目内设置中心与应用页 | 设置中心/应用页 | 设置中心壳验收后删除旧设置 pane 与旧主题库 |
| 旧工作区 | `app_shell/workspace` 的 `SltermWorkspace`、`TabEntry`、`WorkspaceTab`、`TerminalPane`、旧分屏与焦点 | 删除；新壳按项目/面板组/面板重建 | 工作区壳 | 旧状态模型退役时停止读写并隔离旧状态类型；共享类型随最后一个引用域删除，旧 UI 宿主随各域替代验收删除 |
| 旧会话 | `session.rs`、`session_restore.rs`、`session.json`、`session.crashed.json` | 删除；项目列表、布局、侧栏、终端状态由新持久化承接 | 持久化协调器 | 旧状态模型退役时关闭写路径；崩溃恢复验收后删除旧读取代码与旧文件 |
| 侧栏 | `app_shell/workspace/sidebar.rs`、`side_panel` 视图 | 保留视图引擎；双槽固定模型改为封闭侧栏视图注册与状态矩阵 | 侧栏框架 | 旧状态模型退役后删除旧 `side_panel` 宿主 |
| 文件编辑器 | `app_shell/file_editor` 文档、搜索、折叠、Git 行标、保存 | 保留引擎；删除渲染态 live edit、inline edit 与相关命令 | 编辑器面板 | 编辑器面板验收后删除旧 tab 宿主与 live edit |
| Markdown | `markdown/` 渲染器、公式、资源加载 | 保留渲染与公式；Mermaid 首版不渲染，按源码显示；删除渲染态块编辑接线 | Markdown 面板 | Markdown 面板验收后删除旧双态宿主 |
| Diff | `app_shell/diff_tab.rs` | 保留双栏/行内、折叠、同步滚动；新增冲突快照只读模式；删除 Merge 路由 | Diff 面板 | Diff 面板验收后删除旧 CodeTab 模式 |
| Merge | `app_shell/merge_tab.rs`、三栏冲突采用流程 | 删除；冲突处理由终端 Git 工作流承接，冲突对比由 Diff 快照承接 | Diff 快照 | Diff 快照可用后删除 |
| Image | `app_shell/doc_tabs.rs` 的 `ImageTabView`、图片打开路由 | 删除；图片/二进制交系统默认程序或错误提示；不新增预览面板类型 | 文件打开路由 | 旧状态模型退役时删除 |
| GitShow | `app_shell/code_tab.rs` 的 GitShow 只读模式 | 保留只读读取与渲染；适配 GitShow 面板 | GitShow 面板 | 新面板验收后删除旧模式 |
| Git | `git/`、`git_completion.rs`、`git_worktree.rs` | 保留唯一执行入口；Commit/Diff/GitShow/文件树统一实际仓库根；worktree 供 agent fork | 各面板与 Git 服务 | 长期保留 |
| 文件树/Commit/Agent 文件 | `side_panel`、`agent_dirs.rs`、文件操作与监听 | 保留引擎；项目根与 Agent 根作为允许根；侧栏视图改为封闭注册集 | 侧栏框架 | 旧状态模型退役后删除旧 `side_panel` 视图宿主 |
| 主题库 | `theme_library` 的 document/store/package/preferences、自定义主题编辑器、`slterm_settings` 的 `custom_theme` 与主题覆盖键 | 删除；内置暗色主题目录并入 `slterm_settings`，Linear 默认，未知 id 回退 Linear | 主题设置域 | 新设置中心验收后删除；备份/导入导出不再含主题类别 |
| 无障碍专用实现 | 屏幕阅读器/ARIA/播报/高对比度/大字体适配入口，`theme_editor`、`custom_theme`、`themes` 的对比度校验与自动回退及相关测试 | 删除；不做对比度校验、不提示、不静默改写设置；保留键盘、焦点、IME/CJK、减少动效 | 无 | 新设置中心验收后随主题库删除 |
| hook 桥 | `slterm_hook` 独立小进程、hook 转发协议 | 保留；只随面板身份环境变量改名与事件字段适配 | `slterm_hook` | 长期保留；不被 app 依赖 |
| AI Providers/助手 | `ai_providers.rs`、`ai_assistant.rs`、`assistant_answer/`、`provider_test.rs`、`secrets.rs`、`agent_host.rs`、`ai_sessions.rs`、`ai_agents.rs`、`ai_hook*` | 保留引擎与凭据纪律；标识从 `PaneId` 改为 `PanelId`；Provider 元数据与助手配置迁入应用设置域；旧 `Slterm/AI/<id>` 凭据先写新 target、校验可读、再删除旧 target，任一步失败保留旧 target 并提示 | AI 服务 | 新应用页与模拟凭据测试通过后删除旧配置文件和旧设置入口 |
| 终端补全 | `completion.rs`、`completion/`、`suggest.rs`、`history.rs`、`directory_history.rs` | 保留引擎；来源收敛本地；缓存按面板；数据文件迁入 `persistence.md` 定义的本地路径 | 终端增强 | 终端增强回迁验收后删除旧缓存路径 |
| 数学覆盖层 | `terminal_math*`、`math/`、`gpui_terminal/math_overlay.rs` | 保留；只随面板 host 与设置开关适配 | 终端增强 | 长期保留 |
| 托盘/任务栏/开机启动/通知 | `tray.rs`、`taskbar.rs`、`platform/startup.rs`、`notify.rs`、`platform/notifications/` | 保留平台实现与线程模型；目标标识改为项目/面板；入口迁入应用页 | 应用服务 | 应用页范围外节与壳服务验收后删除旧 `PaneId`/旧 tab 路由 |
| 更新链 | `update_check.rs`、`update_download/`、`update_proxy.rs`、`platform/update_installation.rs`、`platform/distribution.rs` | 保留检查/下载/校验/两阶段交接；状态路径与工作区快照 schema 改为目标态 | 更新服务 | 最后迁移；与加密备份完成后接线并删除旧更新 UI 与旧状态路径 |
| 加密备份 | `encrypted_backup.rs`、`encrypted_backup/recovery.rs` | 保留 AES-GCM/Argon2id/恢复点/原子写；类别与归档名改为新状态文件；旧格式拒绝 | 备份服务 | 新持久化 schema 冻结后重写类别与 UI；验收后删除旧类别与旧 UI |
| Runtime API | `runtime_api*`、`cli.rs`、`assets/runtime-api-v1.schema.json`、`runtime-control-api.md`、随包 skill | 整体重写为 v2 资源模型；不保留 v1 兼容层 | Runtime API v2 | 旧 v1 实现随旧状态模型退役停止；v2 协议核心在切片后建立，完整面收敛门后交付，旧资产随后删除 |
| i18n | `i18n/` typed 键与中文/英文目录 | 保留构建期键合同；新增应用页/更新/备份键，删除旧能力键 | i18n | 长期保留 |
| 日志/panic | `logging.rs`、`panic.rs` | 保留 logger 与 panic 钩；数据路径改为新诊断目录 | 诊断服务 | 长期保留 |
| 后台任务 | `background_tasks.rs`、`plan_balance/` | 保留注册表、订阅、重入与调度；任务配置接入应用页 | 应用页/任务调度器 | 长期保留 |
| 窗口 | `windowing`、`app_shell/windowing` | 重建单窗口 shell；窗口几何持久化改为目标态 | 单实例与窗口服务 | 旧状态模型退役后删除旧窗口状态 |

旧状态模型与写路径在“旧状态模型退役”里程碑停止并被新壳替代；各域旧 UI 宿主在该域替代实现验收后删除，共享旧状态类型随最后一个引用它的域删除。上表“隔离与删除时机”列描述各域删除检查点。

新建域及其职责与边界见 `architecture.md` 的模块树与新 core crate 边界；依赖关系只在 `plan.md` 定义。本节只保留旧域→新域的迁移映射与删除时机，不再复述目标态职责。

### 新旧壳隔离与退役门禁

- 旧 UI 模块保留在 `slterm_app/src/app_shell/`，不使用 `legacy-shell` feature；新壳位于 `src/app/`，两者互不引用，只有新壳进入 `run()`。
- 新 core 领域 crate 不得依赖 `slterm_app`、`slterm_split` 的旧 `PaneId` 布局类型或旧 `Session`；依赖方向由 `architecture/dependencies.toml` 与 `scripts/check_architecture.py` 强制。
- 退役前以临时 `check_legacy_symbols` 门禁扫描 `SltermWorkspace`、`WorkspaceTab`、`TerminalPane`、`SplitTree<PaneId>`、`TabEntry`、`Session`、`TabSession`、`LayoutSession`、`ImageTabView`、`MergeTabView`、`ThemeLibraryStore` 等旧符号；旧状态模型退役时删除已替代宿主与不再被引用的旧状态代码，脚本与 allowlist 在旧符号全部退出后删除。
- 旧持久化写入在切换时关闭：新壳启动后不再创建或更新 `session.json`、旧 `settings.json`、旧 `providers.json`、旧 `runtime.port`；旧文件不读不删。
- 旧 Runtime v1 投影不桥接新模型；新 RuntimeHub 只发布 v2 快照。旧 v1 服务端在新壳接管时停止启动，不保留双服务端。
- 旧/新窗口可开发期并存，但发布安装覆盖旧二进制；应用内不存在并行模型选择开关。

### 逐域删除时机

删除触发条件：该域替代实现已验收、生产符号引用清零、旧测试映射已建立。按以下顺序推进：

1. 垂直切片前只做新域实现与保留引擎适配，不删除旧壳；旧壳与新壳互不引用。
2. 旧状态模型退役：关闭旧 `session`/设置/运行时写路径，新壳成为唯一状态权威；删除已被替代的旧终端、侧栏、导航 UI 宿主与对应旧测试；其余旧 UI 宿主与共享旧类型隔离为不可达模块并登记所属域。
3. 各域在替代实现验收后删除本域旧 UI 宿主：编辑器 tab 宿主、Markdown 双态宿主、Diff CodeTab 模式、GitShow 只读模式、设置 pane 与主题库、旧 pane 增强接线、旧 provider/assistant 入口、旧更新 UI、旧备份 UI。
4. 最后一个引用旧共享类型的域退出后，删除旧 workspace/tab/pane/dock/session 类型、`check_legacy_symbols` 脚本与 allowlist。
5. 备份与更新最后迁移；删除旧备份类别、旧更新状态路径与旧更新 UI 接线。

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
| `app_shell/ui_tests/theme_tests.rs` | 用户主题、主题包 | 仅内置暗色主题 | 内置主题与未知 id 回退；负向断言不做对比度校验、不静默改写用户设置；用户主题/包测试删除 |
| `theme_editor`/`custom_theme`/`themes` 对比度测试 | 对比度准入与自动回退 | 无对比度准入 | 删除；新增“不因对比度拒绝主题、不静默改写用户设置”负向断言 |
| `app_shell/ui_tests/wallpaper_tests.rs` | 壁纸与透明度 | 外观页 | 保留并适配新设置键 |
| `app_shell/ui_tests/tray_tests.rs`、`taskbar_tests.rs` | 旧 pane 托盘/任务栏 | 应用服务 | 新项目/面板路由、菜单、进度清除测试 |
| `app_shell/ui_tests/update_tests.rs` | 旧更新弹窗与交接 | 更新服务 | 保留更新引擎测试；新增应用页入口与 v2 快照恢复测试 |
| `app_shell/ui_tests/runtime_control_tests.rs`、`runtime_projection_tests.rs` | v1 命令与快照 | Runtime API v2 完整面 | v2 资源模型、稳定 ID、幂等与事件测试 |
| `slterm_app/tests/runtime_conformance.rs` | v1 外部契约 | Runtime API v2 完整面 | v2 conformance 与真实进程条件用例 |
| `session.rs` 内测试 | 旧 session.json 快照/恢复 | 持久化协调器 | 新项目索引、布局、去抖、损坏隔离测试 |
| `slterm_split` 测试 | 旧 `PaneId` 分屏 | 新布局树 | 迁移纯函数并改叶类型；删除旧 ID 绑定测试 |
| `app_shell/merge_tab.rs` 测试 | 三栏合并 | 能力删除 | 删除；无替换 |
| Image 相关测试 | 图片预览 | 能力删除 | 删除；文件路由改为系统程序/错误测试 |
| Markdown live edit 测试 | 渲染态块编辑 | 能力删除 | 删除；保留源码/渲染两态测试 |
| 提示符跳转测试 | `slterm_prompt_jump` | 能力删除 | 删除；新增“无跳转入口/无按键”缺失测试 |
| `ai_providers`/`ai_assistant`/`secrets` 测试 | 旧 PaneId/旧配置文件 | AI 服务 | 保留并改 ID/路径；新增应用页与凭据边界测试 |
| `terminal_math`/补全测试 | 数学覆盖层、补全引擎 | 终端增强 | 保留；新增面板 host 与开关测试 |

### 旧数据文件忽略

- 旧根目录为 `%APPDATA%\slterm`（或 `SLTERM_CONFIG_DIR` 指向的目录）。新应用设置位置由 `design/persistence.md` 定义，与旧根 `settings.json` 不碰撞；状态库与机器本地文件在本地根，与旧文件不重叠。
- 新代码启动、运行、关闭全程不读取、不重命名、不删除以下旧文件：`settings.json`、`settings.json.bak`、`session.json`、`session.json.bak`、`session.crashed.json`、`runtime.port`、`runtime.port.slterm-lock`、`providers.json`、`slterm_assistant.txt`、`slterm_assistant.txt.bak`、`slterm_assistant.corrupted.txt`、`history.jsonl`、`directory_history.json`、`update_state.json`、`updates/`、`backup-recovery/`、`themes/`、`ai_sessions.json`、`slterm-panic.log`、`slterm_debug.log` 及旧导入/导出文件。
- 旧文件存在不构成损坏、不触发隔离、不触发警告；新安装与升级从新状态默认值开始。
- 旧 `slterm-workspace.json` 导出文件不提供导入；设置导入只接受目标态应用设置 schema。
- 卸载与迁移均不清理旧目录；是否由用户手动删除不进入产品流程。

### 新数据目录与文件名

新数据目录与文件名由 `design/persistence.md` 单点定义；数据根划分与目录归属见 `design/architecture.md`。迁移只保证：

- 旧路径不读、不删、不重命名；新路径与旧路径不重叠，避免旧文件碰撞。
- 切换时机：新壳接管启动恢复后只写新路径；旧持久化写路径关闭，不提供双写。
- `SLTERM_CONFIG_DIR` 与 `SLTERM_LOCAL_DIR` 的覆盖语义由 `persistence.md` 定义；迁移测试用二者隔离新旧根。
- 范围外能力的数据位置随其所有者篇定义；迁移文档不复制路径清单。

### 旧单实例命名空间隔离

- 旧实例文件：`<基础目录>/runtime.port` 与 `runtime.port.slterm-lock`；新实例端点与实例锁位置由 `design/persistence.md` 定义。新启动链不探测旧端口文件，不读取旧锁。
- 端点记录格式与解析规则由 `design/integrations/runtime-api-v2.md` 定义；新解析器只接受 v2 格式，旧格式、缺失字段、非法端口一律视为不可达。
- 显式 `SLTERM_RUNTIME_ENDPOINT` 只接受 v2 格式；值非法时判定为“明确不可达”，不回落端口文件，避免连错实例。
- 第二实例启动只向 v2 端点发送 `runtime.handover`；旧实例不会收到移交，旧实例的第二启动也不会触发新实例。
- AUMID 保持产品身份，不作为单实例判定；发布安装覆盖旧二进制。开发期旧/新并存可能出现通知身份共用，不进入产品验收。

### 旧能力删除清单

| 能力 | 删除的代码与入口 | 替代/边界 | 验收 |
|---|---|---|---|
| Image 面板 | `ImageTabView`、`openable_in_app` 图片分支、图片页签恢复类型、相关测试 | 图片交系统默认程序打开或提示不支持；不新增预览类型 | 无 Image 面板类型、无图片打开路由、恢复丢弃未知类型 |
| Merge 三栏 | `merge_tab.rs`、冲突打开 Merge 路由、采用版本命令与测试 | Git 冲突处理由终端承接；冲突对比由 Diff 只读快照承接 | 无 Merge 入口与快捷键；冲突来源只打开 Diff 快照 |
| 用户自定义主题 | `theme_library` store/package/document/preferences、主题编辑器、导入导出、`themes/` | 仅内置暗色主题方案；默认与未知 id 回退由 `theme-and-i18n.md` 定义 | 无导入/导出/编辑器入口；主题列表只含内置方案并保持可读 |
| 无障碍专用实现 | 屏幕阅读器/ARIA/播报/高对比度/大字体适配入口，`theme_editor`/`custom_theme`/`themes` 的对比度校验与自动回退及相关测试 | 边界见 featureSpec 的无障碍与主题决策；保留键盘、焦点、IME/CJK、减少动效 | 无专用入口、快捷键或设置键；无对比度准入与自动回退 |
| Markdown 实时块编辑 | `file_editor/live_edit.rs`、`inline_edit.rs`、`live_commands.rs`、渲染态输入规则与订阅 | 源码/渲染两态；渲染态只读，编辑统一进源码态 | 渲染态无输入、无块编辑命令、无 live edit 状态 |
| 提示符跳转 | `slterm_prompt_jump`、提示符行标记 deque、相关按键与测试 | 终端不提供按提示符跳转；保留 OSC 133 命令边界、标题、状态与退出码 | 无跳转 API、无按键绑定、无文档入口 |

### 依赖里程碑

里程碑依赖只在 `plan.md` 的“里程碑”表定义；本文件不复制依赖列表。

### 迁移顺序

1. 确定新 core 领域 crate 与 `slterm_settings::paths` 双根访问器；建立临时旧符号门禁。
2. 上线状态库、壳与工作区、终端面板、侧栏框架与导航核心，打通垂直切片；新壳接管启动恢复与单实例移交。
3. 执行旧状态模型退役：关闭旧状态写路径，新壳成为唯一状态权威；删除已被替代的旧终端、侧栏、导航 UI 宿主与旧测试，旧 Runtime v1 停止启动；其余旧 UI 宿主隔离并登记待删域；保留已验证引擎。
4. 在新地基上推进编辑器、Markdown、Diff、GitShow、Explorer、Commit、设置中心与 Agent 域；每域验收后删除该域旧 UI 宿主；控制面协议核心同步交付。
5. 完成 Runtime API v2 完整面、应用页范围外节、AI Providers、终端增强。
6. 重写备份类别与 UI；重接更新链；完成发布链资产与预算。
7. 全量收敛门与发布核验。

### 测试与验收

- **旧数据不动**：预置全部旧文件后启动、操作、关闭新应用；旧文件逐字节不变，新状态库与设置按默认值创建，无损坏提示。
- **命名空间隔离**：旧 `runtime.port` 指向存活旧进程或死进程时，新应用仍按本地根新锁启动；旧格式显式端点被判定不可达。
- **符号清零**：退役前 `check_legacy_symbols` 引用非零即失败；全部域替换完成后旧符号、脚本与 allowlist 均不存在。
- **能力缺失**：Image、Merge、用户主题、Markdown 实时块编辑、提示符跳转、无障碍专用能力、对比度准入不存在入口、快捷键、设置键或恢复分支。
- **测试映射**：旧测试删除前对应新测试通过；无替换项在 `acceptance.md` 有明确不做登记。
- **启动兼容**：旧文件存在不阻断启动；新状态文件损坏时只按新文件规则隔离并提示。
- **数据路径单点**：源码扫描禁止在 `slterm_settings::paths` 之外拼接状态根或旧文件名。
- **文档切换**：实现落地时同步更新 `.claude/docs/architecture/overview.md`、各路径 `CLAUDE.md` 与 `CONTEXT.md`；文档阶段先更新 `CONTEXT.md` 与 `docs/devPlan`。

### 不可自动化面

- 旧/新两个 GUI 进程并存的桌面行为、托盘图标归属与通知身份只能人工冒烟。
- 安装覆盖后旧数据目录残留、旧快捷方式与开始菜单项状态依赖安装器与系统外壳，人工核验。
- 大规模删除旧模块后的编译期错误只由 CI 编译与门禁兜底，无法用行为测试替代。
