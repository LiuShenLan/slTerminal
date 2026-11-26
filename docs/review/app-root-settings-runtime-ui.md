# 应用壳设置中心、Runtime 快照 UI、项目/导航/语言/主题库与 i18n 审查

> 审查基线为工作树 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`；只读审查源码、文档与测试，未运行 cargo、测试或网络。

## 覆盖与局限

- 已覆盖：`slterm_app/src/app_shell/app_root/` 的 `settings_overlay`、`settings_controls`、`settings_agents`、`hooks_page`、`runtime_snapshot`、`language_selector`、`projects`、`navigation`、`nav`、`markdown_links`；`slterm_app/src/theme_library/` 的 `store`、`document`、`preferences`；`slterm_app/i18n/` 的生成器、语言查询与目录；`slterm_app/src/settings_state.rs`、`slterm_app/src/app_shell/config.rs`、`slterm_app/src/app_shell/theme.rs`。
- 已读测试：`tests/settings_overlay.rs`、`tests/language.rs`、`tests/navigation.rs`、`tests/nav.rs`、`tests/projects.rs`、`tests/relocation.rs`、`tests/runtime_snapshot.rs`，以及 `settings_state.rs`、`theme_library/store.rs`、`theme_library/preferences.rs`、`settings_transfer.rs`、`acceptance.rs` 内的相关测试。
- 已读文档：`docs/featureSpec/panels/settings.md`、`docs/featureSpec/workspace/shell.md`、`docs/featureSpec/workspace/navigation.md`、`docs/featureSpec/acceptance.md`、`docs/devPlan/design/settings-recovery.md`、`docs/devPlan/design/workspace.md`、`docs/devPlan/design/windows.md`、`docs/devPlan/traceability.md` 的设置、工作区、导航行，以及相关 `CLAUDE.md`。
- 未覆盖：未运行真实 GPUI 窗口、真实 Windows 桌面、真实文件选择器、真实 Hooks 文件或真实主题目录；`theme_library/document.rs` 与 `slterm_settings/src/custom_theme.rs` 只读了校验与值模型相关区段，未逐行审阅全部内置主题数据；`app_root/relocation.rs` 不存在，根重定位实现位于 `projects.rs`，测试位于 `tests/relocation.rs`。
- 未审阅构建产物中的 `translations.rs`；i18n 消费情况以目录键、生成变体名与源码 grep 核对。

## 问题

### 阻断｜设置中心页面集合与导航结构未按规格实现

- 置信度：确认
- 位置：`slterm_app/src/settings_state.rs` 的 `SettingsPageId` 与 `SettingsPageId::ALL`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `FIELDS`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `SettingsView::render`、`SettingsView::select_page`。
- 现象：页面集合固定为 Appearance、Scrolling、Interaction、Session、System、AgentHooks、Hooks，没有规格要求的快捷键页、后台定时任务页、终端页、应用页和 Agent 基础配置页。快捷键录制、冲突呈现、后台任务周期、默认 Shell、回滚容量、编辑器字体、Diff 重算间隔、Document 崩溃恢复、侧栏默认布局等规格字段没有对应页面或入口；左导航是平铺枚举，没有全局组与 Agent 组、没有按标题/CLI/分组过滤，也没有底部导入、导出、恢复所有设置默认、每页恢复默认。
- 根因：页面模型仍是旧上游的 Appearance/Scrolling/Interaction/Session/System 分类，字段表围绕 `SettingsRegistry` 的五个域直接生成；没有按 featureSpec 的负责人页面、宿主可用页集合和页面级操作建模，导航搜索、分组与恢复默认从未进入 `SettingsPageId` 或字段表。
- 影响：设置中心的核心验收不成立；用户无法配置快捷键、后台任务、终端默认 Shell、应用集成与 Agent 基础配置；按规格应存在的页面和操作在界面中不可达。
- 证据：`SettingsPageId::ALL` 只包含上述旧页面；`FIELDS` 把 `shell`、`startup_directory` 放在 Session，把 `scrollback_lines`、`scroll_speed` 放在 Scrolling，把 `copy_on_select`、`cursor_shape`、`completion_style` 等放在 Interaction；`SettingsView::render` 直接遍历 `SettingsPageId::ALL` 生成平铺按钮，没有分组、搜索或底部操作区；`docs/featureSpec/panels/settings.md` 的“导航与页面状态”明确要求全局组/Agent 组、搜索和底部导入导出/恢复默认，`docs/featureSpec/workspace/shell.md` 的“设置入口”要求浮层提供快捷键、后台任务、外观、终端、应用、Agent 基础与 Hooks User；当前 `settings.nav.search`、`settings.interaction.keybinds`、`settings.advanced.*` 等译文没有消费点。
- 优化方向：以 featureSpec 页面清单重建页面注册与宿主可用页集合，把字段按负责人页面归位，并让页面级操作、分组和搜索成为页面模型的一部分，而不是继续在旧注册表域上追加控件。

### 阻断｜设置中心缺少导入、导出、备份、诊断、更新、Runtime、AgentRoot 与套餐凭据入口

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs::render_page`；`slterm_app/src/app_shell/app_root/settings_controls.rs::FIELDS`；`slterm_app/src/settings_transfer.rs`；`slterm_app/src/theme_library/preferences.rs`；`slterm_app/src/plan_balance/`；`slterm_app/src/agent_root_service.rs`；`slterm_app/src/diagnostics.rs`；`slterm_app/src/update_check.rs`。
- 现象：设置界面没有普通设置导入导出、加密备份与恢复、诊断包、版本与更新、Runtime API 状态、AgentRoot 管理、套餐来源 token 配置等入口。`settings_transfer::export_settings`/`import_settings`、加密备份、诊断、更新、套餐服务、AgentRoot 服务均存在，但没有被 `SettingsView` 或 AppRoot 的设置页接线；应用页只剩 System 页的若干开关。
- 根因：应用级功能按领域各自实现后没有汇总到设置页；页面集合里没有应用页，字段表只覆盖标量键，无法承载文件选择、口令、预览、确认、重试等动作型交互。
- 影响：用户无法完成规格要求的设置导入导出、备份恢复、诊断导出、更新、Runtime 端点查看和 AgentRoot 管理；这些能力的引擎代码没有真实用户入口，设置中心验收不成立。
- 证据：`settings_transfer.rs` 的导出/导入函数只出现在自身测试；`encrypted_backup`、`diagnostics`、`update_check`、`plan_balance` 在 `app_shell/app_root` 中没有设置页消费点；`settings_overlay.rs` 的 `render_page` 只渲染字段表、Hooks 和 AgentHooks；`docs/featureSpec/panels/settings.md` 的“应用页”“导入、导出与恢复默认”“AgentRoot 管理”分别要求这些入口；`docs/featureSpec/runtime.md` 要求设置界面显示运行状态和非敏感端点；`settings.advanced.export_workspace`、`settings.backup.*`、`settings.agents.integration_group`、`update.*` 等译文没有消费点。
- 优化方向：把应用页及动作型入口作为一等页面内容接入设置中心，动作结果、取消、失败重试和敏感值不回显由页面状态承载；不要仅把领域服务暴露为不可达 API。

### 阻断｜自定义主题库没有任何设置界面，主题选择只列内置主题

- 置信度：确认
- 位置：`slterm_app/src/theme_library/store.rs` 的 `import`、`import_json_file`、`delete`、`fork_builtin`、`resolve_theme_deletion`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `THEMES` 与 `"theme"` 字段；`slterm_app/src/app_shell/app_root/settings_overlay.rs::render_field`；`slterm_app/src/app_shell/config.rs::resolve_theme`。
- 现象：主题字段只提供内置主题选择，没有导入文件选择器、自定义主题列表、选择、重命名、删除或“删除当前主题需先另选”的界面。`custom_theme` 被字段表排除为“宿主管理”，但不存在任何宿主管理界面。自定义主题库的导入、删除、fork 与删除前置判定只在 store 测试或 `config.rs` 解析中使用。
- 根因：主题库被实现为底层文件 API，但设置页没有对应的主题管理页面；`custom_theme` 从字段表排除后没有补上主题编辑器/管理宿主，主题选择也只由静态内置目录生成。
- 影响：规格要求的自定义暗色主题导入、选择、重命名和删除全部不可达；当前主题为自定义主题时，设置页无法显示或切换它，主题库能力对用户不可见。
- 证据：`THEMES` 是静态内置目录；`SettingsControl::Choice` 只渲染该目录；`theme_library/store.rs` 的 `import_json_file`、`delete`、`fork_builtin` 在 `app_shell` 中没有调用点；`theme_library/preferences.rs` 整体带 `#![allow(dead_code)]`；`docs/featureSpec/panels/settings.md` 的“外观页”要求系统文件选择器导入、选择、重命名和删除；`theme_library/CLAUDE.md` 的“GPUI 用例覆盖文件选择器”没有对应实现或页面测试。
- 优化方向：为自定义主题建立设置页管理面，复用唯一 store 与原子写规则；主题选择必须同时覆盖内置与自定义身份，并显式处理当前主题删除前置条件和导入失败零写入。

### 阻断｜主题文档仍强制承载排版、卡片几何、背景图与效果字段

- 置信度：确认
- 位置：`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition`、`ThemeEffects`、`ThemeTypography`、`ThemeLayout`；`slterm_app/src/theme_library/document.rs::validate_and_normalize`、`validate_typography`、`validate_layout`、`validate_effects`；`slterm_app/src/theme_library/preferences.rs::custom_theme_updates`。
- 现象：主题 JSON 仍要求 `typography`、`layout`、`effects` 三个对象，`ThemeEffects` 还包含 `cursor_shape`、`background_image`、壁纸透明度/适配/对齐/覆盖 chrome 等字段；`custom_theme_updates` 应用主题时会把卡片几何、光标形状和壁纸键写回应用设置。仅包含颜色 token 的合规主题会因缺少非颜色对象被拒绝。
- 根因：主题值模型保留了旧上游的排版、布局和视效字段，主题文档信封围绕这些字段定义了必填结构；设计文档要求移除这些字段的裁决没有落到 `ThemeDefinition` 与文档 schema。
- 影响：主题不再只负责颜色，违反“配色单点”和“主题只负责颜色”的设计裁决；合规的纯颜色自定义主题无法导入，导入的主题又可能修改被最终排除的外观项，主题验收不成立。
- 证据：`docs/devPlan/design/settings-recovery.md` 的“主题与 ANSI”明确写“主题只负责颜色，移除排版、密度、卡片几何、背景图片、媒体、ZIP 和效果字段”；`docs/featureSpec/panels/settings.md` 的“外观页”只要求完整语义 token 与 ANSI 映射；`ThemeDefinition` 与 `ThemeEffects` 和 `validate_and_normalize` 的必填校验仍是旧结构；`custom_theme_updates` 写入 `pane_card_radius`、`pane_card_gutter`、`pane_card_shadow`、`pane_card_divider`、`cursor_shape`、`background_image*`。
- 优化方向：按设计终态把主题文档收敛到颜色 token 与 ANSI 映射，删除排版/布局/效果字段及其应用逻辑；主题文档校验只拒绝缺色、未知字段和非法色值，不要求非颜色对象。

### 阻断｜最终排除项仍在设置页暴露：光标形状、壁纸、卡片几何与旧密度项

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `CURSOR_SHAPES`、`WALLPAPER_FITS`、`WALLPAPER_ALIGNMENTS`、`DENSITIES`、`pane_card_*` 字段与 `"cursor_shape"`、`"background_image"`、`"density"` 字段；`slterm_app/src/acceptance.rs::acceptance_excluded_surface_absent`。
- 现象：设置页仍提供光标形状的 block/beam/underline/hollow 选项、壁纸路径/适配/对齐/透明度/覆盖 chrome、卡片圆角/间距/阴影/分隔线，以及只有标准/紧凑两档的旧密度字段。规格要求光标形状固定竖线，最终排除项明确不提供光标形状设置；外观页要求密度为紧凑、标准、宽松，缺“宽松”档。
- 根因：旧上游外观与交互字段没有按最终排除项和外观页规格裁剪；`every_setting_field_in_displayed_domains_has_exactly_one_control` 以注册表键集为强等目标，反而强制这些旧键必须保留控件；验收测试只检查旧通道字符串，没有检查设置字段。
- 影响：产品仍向用户暴露被排除的光标形状设置，且该设置当前不产生实际光标行为；壁纸、卡片几何和缺失档位的密度与规格不一致，最终排除项验收不成立。
- 证据：`docs/featureSpec/acceptance.md` 的“最终排除项”写明“不提供 vi／输入模式页、数学覆盖层、AI 修复条、独立终端配色或光标形状设置”；`docs/featureSpec/panels/settings.md` 的“外观页”要求“界面密度为紧凑、标准、宽松，默认标准”，而 `DENSITIES` 只有 `standard` 与 `compact`；`acceptance_excluded_surface_absent` 只搜索 `legacy_session`、`session_v1` 等旧通道字符串，没有遍历 `FIELDS` 或主题 effects。
- 优化方向：删除光标形状、壁纸、卡片几何与旧密度项，按规格提供密度三档；同步删除主题 effects 中的对应字段，并让最终排除项测试检查真实设置页字段与命令目录，而不是只检查少数源码字符串。

### 阻断｜导航树未实现规格的会话、历史、搜索、套餐与隔离区界面，仅项目列表

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/nav.rs` 的 `NavRuntime`、`render_nav`、`sync_nav_projects`；`slterm_app/src/app_shell/app_root/navigation.rs` 的 `deepest_project_for_cwd`、`resolve_restore_cwd`、`resolve_restore_project`；`slterm_app/src/plan_balance/`；`slterm_app/src/history_quarantine`。
- 现象：导航视图只有项目行、激活按钮和移除按钮，没有顶部标题/刷新/搜索、添加项目、项目展开箭头、活跃会话行、历史会话节点与行、未归属节点、项目根不可访问警示、重命名、拖拽排序、Alt+Up/Down、右键/Shift+F10 菜单、会话恢复与分支、套餐余量页脚、删除隔离区与撤销入口。导航状态只保存项目名与活跃项，未使用 `SideViewState` 的 `scroll`、`unassigned_expanded` 等字段。
- 根因：导航被压缩为项目列表选点；历史扫描、会话展示、套餐服务和隔离区没有接入 `SideView` 渲染与交互，恢复/归属判定只实现为未消费的纯函数。
- 影响：导航树作为规格所有者的大部分能力不可用；用户无法浏览历史会话、搜索、恢复、撤销删除或查看套餐余量，核心验收不成立。
- 证据：`NavRuntime` 只有 `projects: Vec<NavProjectRow>`，`NavProjectRow` 只有 `id`、`display_name`、`active`；`render_nav` 只生成项目行与 `nav-remove`；`navigation.rs` 的归属与恢复辅助函数均带 `#[allow(dead_code)]`，没有生产调用点；`plan_balance::PlanBalanceService` 在 `app_shell` 中没有实例化或消费点；`history_quarantine` 只在测试与模块声明中出现；`docs/featureSpec/workspace/navigation.md` 要求完整树、活跃/历史会话、搜索、套餐页脚和隔离区撤销。
- 优化方向：按导航规格建立树模型、会话/历史索引、搜索与选择、恢复与隔离区、套餐页脚及菜单的单一执行链，并复用统一命令与 PathPolicy；不要保留只有项目行的占位视图。

### 阻断｜关闭设置浮层、设置面板或切换 Hooks 层时没有草稿守卫

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs::close_settings_overlay`、`SettingsView::select_page`；`slterm_app/src/app_shell/app_root/hooks_page.rs::set_hooks_layer`；`slterm_app/src/app_shell/app_root/batch_close.rs`；`slterm_app/src/app_shell/app_root/project_close.rs`；`slterm_app/src/ai_hook/draft_host.rs::project_has_drafts`。
- 现象：关闭无项目浮层只还焦并释放视图，不检查 Hooks 草稿；批量关闭/页签关闭不识别 Settings 面板中的 Hooks 草稿；项目移除和根重定位只以 `HooksDraftPending` 拒绝，不提供保存/丢弃/取消选择；切换 Hooks 层直接切换并请求另一层草稿，不经过保存/丢弃/取消守卫。带脏草稿的 Settings 面板关闭后，草稿留在内存但失去可寻址面板。
- 根因：Hooks 草稿被登记为独立内存宿主，但关闭与切层路径只覆盖 Document/终端，没有把 Hooks 草稿纳入统一关闭守卫；项目关闭路径用了“存在草稿就拒绝”代替显式决定。
- 影响：用户可能在关闭设置面板时无声丢失未保存 Hooks 编辑；关闭失败或取消时没有可见选择；草稿还可能成为无法再次打开的残留，违背规格的“保存／丢弃／取消守卫”。
- 证据：`close_settings_overlay` 只执行焦点恢复与注销；`batch_close` 的对象收集只处理终端与 Document，没有 `hook_drafts` 检查；`project_close` 在 `project_has_drafts` 时直接返回错误；`set_hooks_layer` 只调用 `reset_hooks_sync` 与 `request_hooks_draft`；`docs/featureSpec/panels/settings.md` 的“Hooks 草稿例外”要求切页、切层、关闭或根重定位必须经保存／丢弃／取消守卫。
- 优化方向：把 Hooks 草稿接入统一关闭决策，提供显式的保存、丢弃、取消结果；切层与关闭都必须先解决当前层或全部层草稿，不能只以错误码拒绝。

### 重要｜大量设置控件没有生产消费方，修改不产生行为

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_controls.rs::FIELDS`；`slterm_app/src/app_shell/config.rs::Settings::from_runtime`、`Settings::term_config`；`slterm_app/src/gpui_terminal/view/typography.rs::CELL_WIDTH_MODE`；`slterm_app/src/gpui_terminal/view/pointer.rs`。
- 现象：多个设置键只出现在字段表/注册表与写队列中，没有生产消费方。至少包括 `density`、`cell_width_mode`、`tab_reveal`、`tab_close_visible`、`cjk_bold_regular`、`vcs_display`、`font_family_cjk`、`panel_resize`、`ctrl_wheel_font_zoom`、`cursor_blink`、`cursor_motion`、`new_tab_position`、`focus_follows_mouse`、`dim_inactive_panes`、`startup_directory`、`keep_session`、`refresh_environment`、`ai_toasts`、`background`、`pane_card_*`。`shell` 设置没有进入终端启动器；`scrollback_lines` 不进入 `Settings::term_config`，后者固定使用 `SCROLLING_HISTORY`；`cursor_shape` 不改变光标绘制。
- 根因：字段表以注册表键集为唯一来源，缺少“设置键到真实消费点”的映射与行为测试；旧上游字段被保留为可写键，但对应渲染或启动逻辑已被裁剪或从未迁移。
- 影响：用户修改设置后界面显示已保存，实际行为不变；保存失败或重启后无法从行为上验证设置，形成静默失效面，违背设置中心的可观察契约。
- 证据：`Settings::term_config` 固定返回 `scrolling_history: SCROLLING_HISTORY`；`typography.rs` 以 `CELL_WIDTH_MODE` 常量固定 Compact；`pointer.rs` 注释写明 `focus_follows_mouse` 属于“裁剪”项；对上述键的源码检索只命中字段表、注册表或测试；`settings_controls.rs` 仍为这些键生成 Toggle/Choice/Stepper。
- 优化方向：以消费点驱动字段表，删除无实际行为的旧键；对保留键补真实行为接线与页面级测试，至少覆盖默认 Shell、回滚容量、密度、编辑器字号和光标相关项。

### 重要｜Hooks 页只实现 Claude JSON 子集，缺少 GUI、事件目录、外部监听、托管只读与保存提示

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/hooks_page.rs` 的 `render_hooks_page`、`available_hooks_layers`、`save_hooks_draft_for`；`slterm_app/src/app_shell/app_root/settings_overlay.rs::with_state` 的 `hooks_layer` 初始化；`slterm_app/src/ai_hook/editor.rs`。
- 现象：Hooks 页只提供 Claude 的 JSON 文本编辑与层选择，没有 JSON/GUI 双模式、事件目录导航、matcher/handler 表单、schema 校验与完整诊断入口、外部修改监听与“已外部修改”提示、User 保存前可执行命令警告、保存后重启 CLI 提示、项目根不可访问时禁用 Project/Local 的理由、托管 handler 的只读与不可删除保护。项目 Settings 新建时默认层是 `Project(project_id)`，而规格要求重建后回到 User 与 JSON。
- 根因：Hooks 页只迁移了最小草稿编辑路径，未接入能力目录、GUI 表单、schema 校验与外部 watcher；默认层按项目身份直接初始化，未按规格的运行期界面状态复位。
- 影响：Hooks 管理能力与规格差距大，用户无法以安全方式编辑事件/处理器，也无法在外部修改时得到提示；默认层行为与规格相反，测试还把这个偏离固化为预期。
- 证据：`render_hooks_page` 只有层 pill、文本编辑框、保存/丢弃；`DraftParseError` 只区分非法 UTF-8、非法 JSON 和重复键，没有 schema/事件目录；`hooks_page.rs` 没有 watcher 订阅或外部修改状态；`with_state` 对 `ViewKey::Settings` 设置 `HooksDraftLayer::Project`，而 `docs/featureSpec/panels/settings.md` 的“状态与验收”要求 Hooks 当前层级与模式重建后回到 User／JSON；`hooks_page_loads_saves_external_guard_and_discards` 断言打开即载入 Project 层草稿。
- 优化方向：按 Hooks 规格补齐 JSON/GUI 双模式、能力目录、schema 诊断、外部监听与托管内容保护；默认层与模式在重建时复位为 User／JSON，并通过页面场景测试固定。

### 重要｜无项目浮层不持久化 ViewState，失效页回退与提示未接线；浮层中的 Hooks 页没有面板身份

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs::new_overlay`、`SettingsView::with_state`、`SettingsView::select_page`、`open_settings_overlay_from_request`；`slterm_app/src/settings_state.rs::SettingsViewState::restore`；`slterm_app/src/app_shell/app_root/side_view.rs` 的 Settings ViewState 预载。
- 现象：无项目浮层创建时传入 `None` 作为 `SettingsViewStateStore`，`select_page` 的 checkpoint 分支不会执行；`AppSettingsOverlay` 的选中页在关闭重开或重启后不恢复，总是回到构造时的默认页。`SettingsViewState::restore` 没有生产调用点，未知/失效页不会回退并提示；浮层页面列表仍显示 Hooks，但浮层没有 `PanelId`，`hooks_key()` 返回 `None`，该页的保存/丢弃实际无效。
- 根因：浮层只用 `ViewKey::AppSettingsOverlay` 做身份，但未把应用级 ViewStateStore 注入；页面可用性没有按宿主（项目/浮层）过滤，页面恢复的纯函数也没有接入视图构造。
- 影响：规格要求的“浮层独立应用级逻辑视图键”与“选中页关闭重开、重启恢复”不成立；用户可能看到可点击但无作用的 Hooks 页面；失效页回退提示缺失。
- 证据：`new_overlay` 调用 `with_state(..., ViewKey::AppSettingsOverlay, None, true, ...)`；`side_view.rs` 只从项目快照中收集 `ViewKey::Settings` 预载设置状态；`SettingsViewState::restore` 只在 `settings_state.rs` 测试中出现；`render` 对浮层与项目面板使用同一 `SettingsPageId::ALL`；`available_hooks_layers` 在浮层给 User，但 `hooks_key` 依赖 `panel_id_value()` 为 `Some`。
- 优化方向：为浮层注入独立的应用级 ViewStateStore，并在构造时调用可用页回退；页面列表按宿主能力过滤，浮层 Hooks 页要么提供可用的 User 草稿身份，要么不显示。

### 重要｜设置保存状态与失败不可见；文本/颜色控件跨视图不同步且缺少路径与范围校验

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs::poll_settings_save`、`apply_settings_field`、`render_field`、`on_input_change`；`slterm_app/src/settings_state.rs::SettingsStore::persist_failed`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 Text/Color 字段。
- 现象：设置页没有“保存中/已保存/保存失败与重试”的统一状态，后台写盘失败只保留 `persist_failed` 与待写集合并记日志，界面仍显示正常；`SettingsStatusSaveFailed` 等译文没有消费点。Text/Color 输入实体只在视图构造时读取一次值，其他 Settings 视图修改同一键时不会刷新；输入每次 `InputEvent::Change` 都提交，非法值只被拒绝并留下瞬态标记，但文本不会回滚，且跨页切换仍保留缓存输入。`shell`、`startup_directory`、`background_image` 等文本没有存在性、绝对路径或目录规则校验。
- 根因：字段控件按视图本地缓存文本实体，未订阅全局运行值变化；保存回执只更新存储层，没有进入视图状态；文本字段只依赖注册表的类型/格式校验，缺少规格要求的路径与语义校验。
- 影响：两个设置视图可能显示不同生效值；保存失败被静默；非法路径可写入运行值；用户在切页后仍看到上一页的非法文本，违反瞬态与同步规则。
- 证据：`SettingsView::with_state` 为每个 Text/Color 字段创建一次 `InputState` 并注入初始值；`render_field` 直接复用 `self.inputs`，没有从 `field_values` 回填；`on_input_change` 以每次按键调用 `commit_field`；`poll_settings_save` 只调用 `finish_save` 与 `tasks.complete`，没有通知或字段级错误；`SettingsStore::persist_failed` 只被测试读取；`SettingsControl::Text` 的 `shell`、`startup_directory` 没有路径校验器；`docs/featureSpec/panels/settings.md` 要求所有页面统一显示保存中/已保存/保存失败与重试，且自定义 Shell 路径必须是存在的可执行文件绝对路径。
- 优化方向：把保存状态与失败回执纳入设置视图状态，文本控件由全局运行值驱动并区分“编辑中/已提交/非法”；路径类字段使用真实校验与行内错误，不把非法值写入运行值或全局队列。

### 重要｜设置悬浮窗无活动栏配置按钮

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/sidebar.rs::render_activity_bar`、`render_side_button_group`；`slterm_app/src/app_shell/app_root/command_registry.rs` 的 `OpenSettings`。
- 现象：活动栏只渲染 `SideViewId` 的上/下槽按钮，没有设置入口；`OpenSettings` 命令只通过 `Ctrl+,` 键位和根动作可达。规格要求活动栏配置按钮与 `Ctrl+,` 使用同一命令。
- 根因：设置入口只接到键盘命令，未在活动栏的固定按钮区登记独立焦点节点与按钮。
- 影响：鼠标用户无法从活动栏打开设置中心；规格的“同一命令”入口不完整，配置按钮的激活态语义也无从实现。
- 证据：`render_activity_bar` 只包含 `render_side_button_group` 与空白弹性区；`SideViewId` 封闭集合不含设置；`CommandId::OpenSettings` 的按键定义存在，但活动栏渲染没有对应按钮或 action。
- 优化方向：在活动栏固定位置登记设置按钮，复用 `OpenSettings` 命令与同一焦点/激活态协调，不复制第二套打开逻辑。

### 重要｜根重定位实现为 dead code，缺少真实路径查重与保存/丢弃/取消守卫

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/projects.rs::relocate_project`；`slterm_app/src/app_shell/app_root/navigation.rs` 的恢复辅助函数；`slterm_app/src/app_shell/app_root/tests/relocation.rs`。
- 现象：`relocate_project` 带 `#[allow(dead_code)]`，没有 UI 或命令入口；它只调用 `ProjectRegistry::relocate_project` 做词法冲突检查并检查 `hook_drafts.project_has_drafts`，没有真实路径查重（同一真实目录的别名），没有保存/丢弃/取消交互入口，失败时只返回 `HooksDraftPending`。测试主要使用 `C:/work/old` 等虚拟路径，真实文件系统用例也只验证文件打开，不验证真实路径别名重定位。
- 根因：根重定位被实现为内部候选提交方法，未接入导航或命令目录；真实路径解析被假定由调用方完成，但调用方不存在；Hooks 草稿守卫只做成布尔拒绝，没有决定流程。
- 影响：规格要求的根重定位对用户不可达；若未来接线，别名指向同一目录的项目仍可能形成重复根；Hooks 草稿只能被拒绝，无法完成保存或丢弃。
- 证据：`relocate_project` 的 `#[allow(dead_code)]` 注释写明“消费点归项目根重定位命令与导航视图入口”，但两者均未实现；`ProjectRegistry::relocate_project` 只处理词法根；`navigation_relocation_guards_and_reassigns_history` 直接调用 `relocate_project`，没有 UI 场景；`navigation.rs` 的 `resolve_restore_cwd`、`resolve_restore_project` 等同样为 `#[allow(dead_code)]`。
- 优化方向：把根重定位接入真实导航命令，使用真实路径解析与身份比较完成查重，并把 Hooks 草稿纳入保存/丢弃/取消决定；删除无消费的辅助路径或使其成为唯一生产入口。

### 重要｜重复 Settings 恢复不按规格保留首个并提示，布局解码直接失败

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/commands.rs` 的 `ViewKey::Settings` 恢复分支；`slterm_workspace/src/project_layout.rs` 的 `LayoutError::DuplicateView` 与 `ProjectLayout::validate`；`slterm_app/src/persistence/project.rs::decode_project_snapshot_from`；`slterm_app/src/settings_state.rs::SettingsViewState::restore`。
- 现象：恢复时通过 `panel_for_view(project_id, &view, cx)` 跳过已存在的 Settings 面板，看似保留首个；但持久布局在解码阶段就会因重复 `ViewKey` 触发 `LayoutError::DuplicateView`，整个项目布局解码失败，根本到不了逐面板跳过；也没有聚合提示或“移除其余”的状态归一。失效选中页的回退函数 `SettingsViewState::restore` 同样未接入恢复。
- 根因：重复 Settings 的归一被放在视图安装阶段，而 core 布局 DTO 在更早的解码阶段就拒绝重复；没有恢复前的专用归一器。
- 影响：损坏或旧布局中的重复 Settings 会使项目无法按规格恢复；用户得不到“保留首个、移除其余”的结果或提示；失效页无回退。
- 证据：`PanelGroup::new` 与 `ProjectLayout::validate` 对重复视图返回 `LayoutError::DuplicateView`；`decode_project_snapshot_from` 直接 `ProjectLayout::from_canonical_json` 并上抛错误；`commands.rs` 的 `panel_for_view` 判断位于解码之后；`SettingsViewState::restore` 只在 `settings_state.rs` 测试中使用。
- 优化方向：在布局恢复进入 core DTO 校验前提供显式的 Settings 重复归一与警告聚合；把失效页回退接入同一恢复链，确保首个保留、其余移除且用户可见。

### 重要｜主题下拉选择不清除 custom_theme，切换内置主题可能仍显示自定义主题

- 置信度：确认
- 位置：`slterm_app/src/app_shell/config.rs::resolve_theme`；`slterm_app/src/app_shell/app_root/settings_overlay.rs::apply_settings_field`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `"theme"` 字段；`slterm_app/src/theme_library/preferences.rs::custom_theme_updates`。
- 现象：`resolve_theme` 先检查 `custom_theme`，存在即加载自定义主题并忽略 `theme`；设置页选择内置主题只写 `theme`，不会清除 `custom_theme`。因此当前存在自定义主题时，用户选择内置主题后界面选中项改变，但实际配色仍来自自定义主题。设置页也没有清除自定义主题的入口。
- 根因：主题选择被拆成 `theme` 与 `custom_theme` 两个键，但没有统一的“当前主题”选择状态；自定义主题应用路径只由 `custom_theme_updates` 写入，内置主题选择路径不重置它。
- 影响：主题选择与实际渲染不一致，用户无法通过设置页回到内置主题；主题切换验收不成立。
- 证据：`resolve_theme` 的 `if let Some(id) = runtime.custom_theme.as_deref()` 分支先于内置主题分支返回；`apply_settings_field` 对 `theme` 只调用 `settings.set_auto`/`queue_explicit`，没有联动 `custom_theme`；`custom_theme_updates` 是唯一写入 `custom_theme` 的路径且整体 `#![allow(dead_code)]`；`integration_custom_theme` 验证了 `custom_theme` 优先于 `theme`。
- 优化方向：把当前主题收敛为单一选择（内置名或自定义身份），选择内置主题时显式清除自定义身份；设置页显示当前主题来源并提供回到内置主题的动作。

### 重要｜Runtime v2 快照没有设置界面，快照测试未覆盖资源边界

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/runtime_snapshot.rs::runtime_snapshot_resources`、`runtime_agents`、`handle_runtime_read`；`tests/runtime_snapshot.rs`；`docs/featureSpec/panels/settings.md` 的“应用页”、`docs/featureSpec/runtime.md`。
- 现象：没有任何设置视图消费 `SnapshotResources` 或显示 Runtime v2 运行状态、非敏感端点、随包文档入口；`runtime_snapshot_resources` 只服务控制面事件。设置中心因此没有 Runtime 快照 UI，也没有凭据读取/替换/删除失败的可见反馈。快照测试只覆盖单项目单终端的成功快照与关闭中/关闭完成拒绝，没有覆盖空工作区、多项目/多组、隐藏项目、Document 资源、已退出终端代次为零、非 Unicode 路径编码失败等资源边界。
- 根因：Runtime 快照被当作后端协议端口实现，未接入应用页 UI；测试只验证事件回执的基本连通性。
- 影响：规格要求的 Runtime 状态只读展示与端点入口缺失；快照编码失败和资源模型边界没有证据，无法证明设置中心或 Runtime UI 的验收。
- 证据：`runtime_snapshot_resources` 没有非测试调用方；`settings_overlay.rs`、`settings_controls.rs` 没有任何 Runtime 资源或端点字段；`runtime_snapshot_returns_workspace_resources_and_identity` 只构造一个项目和终端的断言；`docs/featureSpec/runtime.md` 要求设置界面显示运行状态与非敏感端点，且不回显 token。
- 优化方向：在应用页提供只读 Runtime 状态/端点/文档入口，所有显示走同一快照资源；补齐空、多项目、隐藏、退出和非 Unicode 编码边界的快照测试。

### 重要｜测试主要覆盖引擎函数与 DTO，未覆盖页面场景；追溯矩阵锚点名不副实

- 置信度：确认
- 位置：`slterm_app/src/settings_state.rs` 的 `settings_single_queue_across_views`、`settings_views_share_global_queue`、`settings_page_view_state_restore`；`slterm_app/src/app_shell/app_root/navigation.rs` 的导航测试；`slterm_app/src/theme_library/store.rs` 的 `theme_import_complete_or_zero_write`；`slterm_app/src/app_shell/config.rs` 的 `integration_custom_theme`；`slterm_app/src/app_shell/text_view/mod.rs` 的 `keybinding_reserved_contexts`；`tests/settings_overlay.rs` 与 `tests/language.rs`；`acceptance.rs::acceptance_excluded_surface_absent`。
- 现象：追溯矩阵标为设置/工作区/导航“已自动化”的多个锚点只验证引擎或纯 DTO：`settings_single_queue_across_views` 与 `settings_views_share_global_queue` 只操作 `SettingsStore`，没有创建两个 SettingsView；`settings_page_view_state_restore` 只做 DTO 往返与纯 `SettingsViewState::restore`，没有视图重开、重启或失效页提示；`navigation_history_lexical_deepest_match`、`navigation_unowned_restore_requires_project_confirmation`、`navigation_restore_fallback_and_injection_retry`、`navigation_transient_and_quarantine_restore` 只测纯函数或序列化；`theme_import_complete_or_zero_write` 只测 store，不测文件选择器或设置页；`integration_custom_theme` 只测配色解析；`keybinding_reserved_contexts` 测文本视图复制，不是设置页快捷键录制与冲突。`settings_panel_lists_pages_and_switches` 只断言旧页面顺序和页选择器存在，不验证字段内容、标签、保存或错误状态；`settings_overlay_keeps_draft_and_queue` 直接向 `hook_drafts` 插入草稿，不经过 Hooks 页。`acceptance_excluded_surface_absent` 不检查设置字段，无法发现光标形状等最终排除项。
- 根因：测试以可单测的存储层/核心函数为中心，页面级场景需要 GPUI 交互与真实文件/窗口，现有用例没有补齐；追溯矩阵把引擎测试当作页面验收证据。
- 影响：页面行为回归不可见，追溯矩阵的状态不可信；用户可见的设置、导航、主题与语言问题无法被现有测试发现。
- 证据：上述测试的断言对象分别是 `SettingsStore`、`SettingsViewState`、`deepest_project_for_cwd`、`ThemeLibraryStore`、`CustomTheme` 解析或 `Keymap`，没有真实点击、输入、关闭或重启的页面动作；`settings_overlay_keeps_draft_and_queue` 与 `app_settings_overlay_keeps_hooks_draft` 都手工插入 `HooksDraft`；`hooks_page_loads_saves_external_guard_and_discards` 未覆盖切层/关闭守卫、外部监听或 GUI 模式，并把 Project 默认层写成预期。
- 优化方向：按 featureSpec 的 Given/When/Then 编写页面级 GPUI 测试，真实操作控件、关闭重开、切项目、重启恢复、非法输入、失败回执与草稿守卫；追溯矩阵只引用能观察用户结果的例名。

### 一般｜应用语言选项直接显示 `zh-CN`/`en-US`，未使用规格要求的语言专名

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `LANGUAGES`；`slterm_app/src/app_shell/app_root/settings_overlay.rs::render_field` 的 Choice 分支；`tests/language.rs`。
- 现象：语言选项用 `literal("zh-CN")` 与 `literal("en-US")`，`ChoiceOption.label` 为 `None` 时界面回退显示存储值，因此用户看到的是语言代码，不是规格要求的 `English` / `简体中文`。语言切换测试直接调用 `apply_settings_field("language", ...)`，没有点击设置页选项或验证标签。
- 根因：语言选项没有提供 typed 显示文案，渲染层按存储值兜底；测试绕过了页面控件。
- 影响：应用页语言选项不符合用户体验规格，且“System、English、简体中文”的选择项无法通过页面验证。
- 证据：`LANGUAGES` 的后两项使用 `literal`；`render_field` 对 Choice 使用 `option.label.map(...).unwrap_or(option.value)`；`integration_language` 与 `i18n_switch_preserves_workspace` 只验证全局快照与通知文案，不验证 Choice 标签。
- 优化方向：为语言专名提供独立 typed 文案或稳定的显示名映射，并用设置页点击测试覆盖 System/English/简体中文三档。

### 一般｜Markdown 链接分派失败被静默吞掉，未显示非阻塞原因

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/markdown_links.rs::open_markdown_link`、`open_markdown_resolved`；`slterm_app/src/app_shell/markdown/link.rs::MarkdownLinkDispatcher::dispatch`、`MarkdownLinkOutcome`。
- 现象：`MarkdownLinkOutcome::System` 与 `MarkdownLinkOutcome::Rejected` 在 AppRoot 中直接空分支处理；本地链接打开失败、目录/不存在/不可读、危险协议拒绝或系统处理程序失败只写日志，不在 Markdown 视图或通知中显示非阻塞原因。`MarkdownLinkDispatcher::dispatch` 的 `opener.open(url)?` 错误在后台任务中只被记录。
- 根因：链接分派只实现了结果分类，没有把拒绝/失败结果接到视图的可见提示；文档要求“显示非阻塞原因”“协议拒绝提示”未被实现。
- 影响：用户点击链接没有反应时无法知道是协议拒绝、路径越权、目标不存在还是系统打开失败；违反 Markdown 链接的反馈契约。
- 证据：`open_markdown_link` 的 `Ok(MarkdownLinkOutcome::System | MarkdownLinkOutcome::Rejected) => {}`；`Err(error)` 只 `log::warn`；`open_markdown_resolved` 的 `prepare_document_open` 失败同样只记录；`docs/featureSpec/panels/markdown.md` 的“链接分派”要求目录、不存在或不可读目标显示非阻塞原因，危险协议显示策略拒绝提示。
- 优化方向：把链接拒绝与打开失败作为可见的临时通知/行内状态回投到对应 Markdown 视图，区分协议拒绝、授权拒绝、目标不存在和系统打开失败，不把日志作为唯一反馈。

### 一般｜合同冲突：主题文档仍要求 WCAG 校验，与 featureSpec 和设计文档冲突

- 置信度：确认
- 位置：`slterm_settings/CLAUDE.md` 的“主题语义合同”与“验收与测试”“操作”；`slterm_app/src/theme_library/CLAUDE.md` 的“操作”；`slterm_settings/src/custom_theme.rs::validate_wcag`、`WCAG_TEXT_CONTRAST`、`WCAG_GRAPHICS_CONTRAST`；`docs/featureSpec/panels/settings.md` 的“外观页”；`docs/featureSpec/acceptance.md` 的“最终排除项”；`docs/devPlan/design/settings-recovery.md` 的“主题与 ANSI”。
- 现象：`slterm_settings/CLAUDE.md` 同一文件内既写“不做 WCAG 校验、对比度提示”，又在验收与操作里写“WCAG 边界是必测面”“新增主题……通过语义槽 WCAG 校验”；`theme_library/CLAUDE.md` 也要求新增主题通过 WCAG 校验。`custom_theme.rs` 仍保留 `validate_wcag` 与 WCAG 常量，虽未被主题导入路径调用。featureSpec 与设置恢复设计明确要求不做对比度校验、提示或自动回退。
- 根因：旧上游的 WCAG 主题编辑体系在裁剪时保留了辅助类型与文档指令，但没有同步删除或改写纪律文档，形成相互矛盾的合同。
- 影响：后续实现者可能据此恢复 WCAG 拒绝或提示，直接违反产品边界；当前代码中未被调用的 WCAG 代码也会误导审查与测试。
- 证据：上述两个 `CLAUDE.md` 的 WCAG 条目与同一文件/featureSpec 的“不做对比度校验”并存；`custom_theme.rs` 的 `validate_wcag` 存在但主题导入的 `ThemeDocument::from_value` 只调用 `ThemeDefinition::validate`；`docs/featureSpec/acceptance.md` 明确不提供对比度校验或提示。
- 优化方向：以产品边界为准统一文档，删除 WCAG 必测/通过表述；若 WCAG 辅助代码不再使用则移除，避免形成第二套主题校验通道。

### 提示｜设置左导航宽度使用 160 逻辑像素，规格为 180

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/settings_overlay.rs::SettingsView::render`。
- 现象：左导航容器宽度写为 `px(160.0)`，featureSpec 规定 180 逻辑像素。
- 根因：布局常量沿用旧实现，未按设置规格更新。
- 影响：导航宽度与规格视觉不一致，长页面标题更容易截断。
- 证据：`SettingsView::render` 中的宽度设置；`docs/featureSpec/panels/settings.md` 的“导航与页面状态”规定左导航宽 180 逻辑像素。
- 优化方向：把宽度收敛为设置中心布局常量并按规格固定，避免散落在渲染代码。

### 提示｜i18n 目录保留大量无消费译文，掩盖未实现页面

- 置信度：确认
- 位置：`slterm_app/i18n/en-US.json`、`slterm_app/i18n/zh-CN.json`；`slterm_app/build/i18n.rs`；`slterm_app/src/app_shell/app_root/settings_overlay.rs`。
- 现象：目录中保留快捷键录制、设置备份、导入导出、更新对话框、Agent 表单、主题编辑器等译文，但对应 `Message` 变体在当前 `slterm_app/src` 中没有消费方；生成器以 `allow(dead_code)` 放行。构建期只保证两语键集与占位符一致，不保证键被页面使用。
- 根因：目录来自旧上游且允许先于消费点存在；设置页迁移只接入了部分键，未清理未实现功能的文案。
- 影响：审查与交付时容易被完整译文目录误导为功能已实现；键集合同无法表达“页面未接线”。
- 证据：`settings.nav.search`、`settings.interaction.keybinds*`、`settings.backup.*`、`settings.advanced.export_workspace*`、`settings.agents.mode_form/mode_json`、`settings.theme_editor.*`、`update.*` 等生成变体没有源码消费点；`build/i18n.rs` 的注释明确允许未消费 ID。
- 优化方向：让目录随页面接线收敛，至少对声明已验收的页面建立“页面消费键集合”与目录键集合的门禁；未实现功能的文案不应留在已验收产品目录中。

## 跨域观察

- `Settings::term_config` 以 `SCROLLING_HISTORY` 固定回滚容量，`gpui_terminal/view/typography.rs` 以 `CELL_WIDTH_MODE` 固定 Compact；设置页写入的 `scrollback_lines`、`cell_width_mode` 与终端真实行为脱节，说明设置字段目录与消费点之间没有机械对账，不只是设置页局部问题。
- `slterm_settings/src/custom_theme.rs` 的非颜色主题字段与 `theme_library/document.rs` 的必填校验，会让主题库、备份、内置主题目录和设置页主题选择共享同一违反设计裁决的数据模型；修复应落在值模型而不是只改一个页面。
- `settings_transfer`、`encrypted_backup`、`diagnostics`、`update_check`、`plan_balance`、`agent_root_service` 都存在领域实现但没有设置 UI，跨设置、恢复、系统集成三个所有者；缺少入口意味着这些领域的验收证据不能从产品界面观察。

## 存疑未证

- `hooks_page.rs` 的保存任务使用 `TaskId::HooksDraftSave(key.panel)`，同一面板不同层并发保存时第二次登记可能被任务服务拒绝且只记日志。需要实际快速切层并连续保存，观察是否有可见失败或草稿停滞。
- `HooksDraftHost::project_has_drafts` 对“已载入但干净”的 Project/Local 草稿也返回真，导致项目移除/根重定位直接拒绝；规格是否允许干净草稿静默丢弃或要求显式决定需要运行时或产品裁决确认。
- `AgentHooks` 页的安装/卸载、注入状态、托管 handler 清理与提交后清理失败，当前只通过注入端口测试；真实 Hooks 文件路径、权限、锁与外部修改场景未运行，无法确认是否达到 Hooks 注入规格。
- 无项目浮层的 ViewState 缺失在源码路径上是确认的，但真实重启恢复是否被其他启动链补写未运行验证；需要真实窗口重启观察。
- 最终排除项与旧设置字段的冲突以源码和文档为准；实际发布产物是否通过其他入口隐藏这些控件未在真实窗口核对。
