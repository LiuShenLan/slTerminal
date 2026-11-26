# 规格、设计与文档一致性审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`；审查 featureSpec、devPlan、.claude 文档、各级 CLAUDE.md、术语表与用户文档；只读，不运行构建或测试。

## 覆盖与局限

- 已通读 `docs/featureSpec/` 全部所有者文档与资产说明、`docs/devPlan/` 全部文档、`.claude/docs/` 全部文档、`.claude/CLAUDE.md`、`CONTEXT.md`、`README.md`、`README.zh-CN.md`、`CHANGELOG.md`、`release-notes/`。
- 模块 CLAUDE.md 按重点抽样：application 壳、AppRoot、持久化、本机控制面、文件树、终端视图、终端核心、工作区、Document、Runtime 协议、Agent、Hooks、侧栏、补全、hook 桥、路径、主题库、加密备份、脚本与发布说明。
- 为核实文档陈述，读取了备份、设置页、主题、导航、Runtime 协议、数据根、验收锚等实现与测试路径；未逐份通读全部模块 CLAUDE.md，也未运行门禁或实机验证。
- 未审历史 diff；只针对当前工作树。工作树内未被 git 跟踪的旧 worktree 文档不计入范围。

## 问题

### 阻断｜加密备份的类别、工作区载荷与恢复入口不符合 featureSpec 与设计

- 置信度：确认
- 位置：`docs/featureSpec/backup-and-recovery.md`「备份类别」「整包恢复」；`docs/devPlan/design/settings-recovery.md`「BackupPayload 与加密格式」；`slterm_app/src/encrypted_backup.rs` 的 `BackupCategory`、`BackupSelection`、`collect_from`、`restore`；`slterm_app/src/encrypted_backup/CLAUDE.md`「双根寻址」；`docs/devPlan/progress.md` 的备份／设置验收记述。
- 现象：规格要求备份类别为“应用设置与主题”和“工作区与界面状态”，工作区类别覆盖项目元数据、布局、`ViewKey` 状态、侧栏、终端编号／标题／cwd 与窗口几何。实现只有 `Settings`、`Session`、`Themes` 三类，没有工作区类别，工作区内容只能以遗留 `session.json` 文件形式出现；实现注释同时声明恢复与撤回 UI 尚未接线。进度记录仍把该范围记为已验收。
- 根因：旧栈的文件拷贝式备份被保留，规格与设计已按逻辑 `BackupPayload` 终态改写，但实现、用户入口与验收映射没有同步。
- 影响：用户无法导出或恢复工作区，跨机重定位后的工作区恢复不存在；恢复与撤回没有产品入口；全量验收声明不成立。
- 证据：`BackupCategory` 枚举不存在 `Workspace` 变体；`collect` 只按三个类别收集 `settings.json`、`session.json` 与主题文件；仓库内没有 `BackupPayload` 类型；`encrypted_backup.rs` 模块说明写明恢复／撤回 UI 未接线；运行期只有 `BackupRestoreRecoverer` 维护器被注册，没有设置页导出／恢复消费者；追溯矩阵把「仅选工作区类别时排除设置及永久排除内容」映射到 `backup_workspace_category_exclusions`，但该用例实际选择的是 `Settings`、`Session`、`Themes` 并只断言数据库与凭据不入包。
- 优化方向：以 featureSpec 为真值决定备份合同的最终形态，并让实现、用户入口、追溯矩阵与进度状态一致；在未存在工作区类别与恢复入口前，不得以全量验收完成表述覆盖该能力。

### 阻断｜加密备份的设置类别以敏感键黑名单导出，可能带出未登记字段

- 置信度：确认
- 位置：`docs/featureSpec/settings-and-persistence.md`「设置导入、导出与恢复默认」；`slterm_app/src/encrypted_backup.rs` 的 `filter_settings`、`filter_sensitive_document`、`strip_sensitive_keys`、`SENSITIVE_KEYS`；`slterm_app/src/settings_transfer.rs` 的 `export_settings`。
- 现象：规格要求设置导出与备份只收“已登记为非敏感的应用字段”，未知命名空间不得因此被当作安全字段导出。普通设置导出使用 `SettingsRegistry::export_keys()` 白名单，而加密备份直接导出整份 `settings.json`，仅按键名表剥除 `token`、`password` 等敏感键。
- 根因：备份路径没有复用设置注册表的导出资格与敏感性判定，改用了名称黑名单。
- 影响：任何未登记、键名不在黑名单内但含敏感内容的字段会被写入加密备份并随包迁移；这与凭据与未登记字段的排除边界冲突，属于真实的数据外带风险。
- 证据：`filter_sensitive_document` 只调用 `strip_sensitive_keys` 后序列化；`SENSITIVE_KEYS` 是固定字面量表；`backup_restore_does_not_restore_credentials` 只覆盖名为 `token`、`password` 的字段，没有覆盖未知命名空间中的非匹配敏感字段；普通导出 `export_settings` 已展示正确的注册表白名单语义。
- 优化方向：让备份设置类别复用注册表的“非敏感且可导出”投影，而不是名称黑名单；补充未知命名空间、未登记字段与非标准敏感键名的排除用例。

### 阻断｜设置中心的页面集合与用户面能力未按 featureSpec 交付

- 置信度：确认
- 位置：`docs/featureSpec/panels/settings.md`「视图模型与入口」「导航与页面状态」「快捷键页」「后台定时任务页」「终端页」「应用页」「Agent 基础配置页」「AgentRoot 管理」「导入、导出与恢复默认」；`slterm_app/src/settings_state.rs` 的 `SettingsPageId::ALL`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的字段页面归属；`slterm_app/src/app_shell/app_root/settings_agents.rs`；`slterm_app/src/app_shell/app_root/settings_overlay.rs`；`slterm_app/src/settings_transfer.rs`；`slterm_app/src/plan_balance/`。
- 现象：规格规定全局组依次为快捷键、后台定时任务、外观、终端、应用，并为每个 CLI 提供基础配置与 Hooks 分节；实现页面为 `Appearance`、`Scrolling`、`Interaction`、`Session`、`System`、`AgentHooks`、`Hooks`。快捷键页、后台定时任务页、终端页、应用页、Agent 基础配置与 AgentRoot 管理页均不存在；应用页要求的 Runtime API、诊断、备份与恢复、套餐来源配置也没有对应页面。设置导入／导出函数与套餐服务没有设置页消费者。
- 根因：旧设置中心的页面与字段结构被保留，规格已改为目标页面模型，但界面、字段目录与用户入口没有迁移。
- 影响：用户无法完成命令重绑、后台任务调度、终端与补全配置、AgentRoot 管理、套餐 token 配置、设置导入导出、备份恢复等核心操作；进度记录与用户文档却把这些能力写成已交付。
- 证据：`SettingsPageId::ALL` 与规格页面顺序不一致；`settings_controls` 没有快捷键、后台任务、AgentRoot、套餐来源或备份字段；`render_agent_hooks_page` 只渲染 hook 安装状态；`settings_transfer::export_settings`／`import_settings` 没有生产调用点；`plan_balance::service::PlanBalanceService` 只在自身测试中出现；`agent_root_service` 没有设置界面入口。
- 优化方向：以 featureSpec 为真值统一页面模型、字段目录、用户入口与追溯证据；若页面模型需要调整，应先更新规格与设计，而不是让实现与文档各自成立。

### 重要｜导航树、历史隔离区与套餐页脚只有服务与纯函数，没有生产界面

- 置信度：确认
- 位置：`docs/featureSpec/workspace/navigation.md`「视图与项目」「活跃会话」「历史会话」「删除隔离区与撤销」「套餐余量页脚」；`slterm_app/src/app_shell/app_root/nav.rs` 的 `NavRuntime`、`render_nav`；`slterm_app/src/history_quarantine.rs`；`slterm_app/src/plan_balance/`；`docs/devPlan/traceability.md` 的导航树映射。
- 现象：规格要求导航树包含项目行、活跃会话行、历史节点、未归属节点、删除隔离区入口与套餐页脚。生产导航视图只有项目行与移除入口；活跃会话、历史扫描结果、隔离撤销、套餐页脚没有渲染或交互入口；相关服务只在维护器或测试中被引用。
- 根因：领域服务与纯函数先落地，用户面接线未完成；追溯矩阵以服务或纯函数用例覆盖了界面级场景。
- 影响：导航规格的大部分用户行为不存在，用户无法恢复历史、撤销删除或查看套餐余量；以自动化测试全绿替代界面验收，验收结论不可信。
- 证据：`NavRuntime` 仅持有 `NavProjectRow` 列表，`render_nav` 只生成项目行与移除按钮；`history_quarantine` 只被维护期与导航测试引用；`PlanBalanceService` 没有 application 壳引用；追溯矩阵的导航树场景指向 `navigation_*` 纯逻辑用例，而不是可观察的界面行为。
- 优化方向：补齐用户面接线，或在规格与进度中明确这些入口仍属未交付；追溯矩阵应区分领域逻辑证据与用户面证据。

### 重要｜自定义主题承载排版、布局、效果、光标形状与背景图，违反“主题只负责颜色”

- 置信度：确认
- 位置：`docs/devPlan/design/settings-recovery.md`「主题与 ANSI」；`docs/devPlan/design/migration.md`「模块处置」；`slterm_settings/CLAUDE.md`「主题语义合同」；`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition`、`ThemeTypography`、`ThemeLayout`、`ThemeEffects`、`validate_typography`、`validate_layout`、`validate_effects`；`slterm_app/src/theme_library/document.rs` 的主题导入字段；`slterm_app/src/app_shell/config.rs` 的 `custom_theme_effects_fill_only_unset_material_values`；`docs/featureSpec/panels/settings.md` 与 `docs/featureSpec/panels/terminal.md`。
- 现象：设计与模块红线规定自定义主题只负责颜色，排版、密度、卡片几何、背景图片、媒体与效果不属于主题；规格还规定光标形状固定竖线。实现的 `ThemeDefinition` 直接包含 typography、layout、effects，导入时校验字体、行高、字重、卡片几何、透明度、模糊、光标形状、壁纸路径与适配方式，主题效果还会填充材质未设置的运行值。
- 根因：旧主题值模型被保留，目标设计的“纯颜色主题”没有落入实现；模块文档同时记录了两套相互冲突的边界。
- 影响：主题成为第二套外观配置通道，可改变字体、密度、材质、壁纸与光标形状；与“主题只负责颜色”“光标固定竖线”“材质、密度、字体不属于主题”的产品合同冲突，也使设置页与主题页的职责边界失效。
- 证据：`ThemeDefinition` 字段与校验函数；`ThemeEffects` 含 `blur`、`opacity`、`cursor_shape`、`background_image*`；`custom_theme_effects_fill_only_unset_material_values` 证明主题效果进入运行时外观；设计文档明确要求移除这些字段；`slterm_settings/CLAUDE.md` 的边界句与同类条目互相矛盾。
- 优化方向：以产品合同决定主题的最终职责；若维持纯颜色主题，应从值模型、导入校验、设置控件与测试中删除非颜色字段；若确需主题携带外观，应同步改写规格、设计与光标／材质边界，而不是保留两套事实。

### 重要｜WCAG 对比度校验与用户提示残留，违反“不做对比度校验”红线

- 置信度：确认（代码存在与文档冲突）；生产调用点未找到，实际拒绝路径疑似
- 位置：`.claude/CLAUDE.md`「配色单点」；`docs/featureSpec/acceptance.md`「最终排除项」；`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition::validate_wcag`、`ReviewedPalette::wcag_violations`、`foreground_recommendations`、`WCAG_TEXT_CONTRAST`；`slterm_settings/src/themes.rs` 的 `builtin_palettes_pass_semantic_slot_wcag_review`；`slterm_app/i18n/en-US.json` 与 `slterm_app/i18n/zh-CN.json` 的 `settings.theme_editor.foreground_description`；`slterm_settings/CLAUDE.md` 与 `slterm_app/src/theme_library/CLAUDE.md`；`docs/devPlan/design/migration.md` 的 WCAG 门禁删除条款。
- 现象：常驻红线规定不做对比度校验、不提示、不自动回退，最终排除项也排除对比度校验与专用无障碍适配。实现仍保留公开的 WCAG 校验 API，测试要求内置调色板满足 WCAG，翻译文案向用户宣称“冷/暖候选满足 WCAG AA”。模块文档同时在“不做 WCAG 校验”和“通过 WCAG 校验是必测面/新增主题操作”之间自相矛盾。
- 根因：旧主题 WCAG 门禁被保留，目标产品约束已取消该门禁；文档没有清理残留，而是同时收纳两种说法。
- 影响：主题编辑器一旦接线就会按对比度拒绝用户颜色或给出提示；当前用户文案也已承诺与产品红线相反的校验行为。审计上无法判断“不做对比度校验”是否仍然有效。
- 证据：`validate_wcag` 的文档注释写明主题编辑器保存路径会拒绝不达标组合；`foreground_recommendations` 以 WCAG AA 阈值生成候选；`builtin_palettes_pass_semantic_slot_wcag_review` 把 WCAG 作为内置主题的机械判据；i18n 的 `foreground_description` 直接对用户展示 WCAG 承诺；设计迁移表要求删除 WCAG 门禁。生产代码中未找到 `validate_wcag` 的调用点，因此当前运行时是否实际拒绝保存仍需确认。
- 优化方向：以根红线为真值删除对比度校验、推荐文案与对应测试，或明确修改红线并登记例外；模块文档必须只保留一种有效规则。

### 重要｜README、CHANGELOG 与发布说明的用户能力清单停留在旧栈

- 置信度：确认
- 位置：`README.md`「Features」、`README.zh-CN.md`「功能」、`CHANGELOG.md` 的 0.3.0 小节、`release-notes/v0.3.0.md`；对照 `docs/featureSpec/acceptance.md`「最终排除项」、`docs/featureSpec/panels/diff.md`、`docs/featureSpec/panels/settings.md`；`slterm_app/src/app_shell/doc_tabs.rs`；`slterm_app/i18n/*.json`。
- 现象：用户文档宣传“图片标签页”“只读双栏 diff”“窗格缩放”“WindowState 恢复”以及“应用/外观/配置/交互/高级/本地化/重置”设置页；规格明确排除 Image 面板，Diff 工作区侧可编辑，设置页为另一组页面，且不存在独立窗格缩放与 WindowState 术语。文档还宣传“屏幕检测”和“壁纸”，但 featureSpec 没有定义这两项能力。i18n 仍保留旧设置页、旧备份类别与主题编辑器的用户文案。
- 根因：用户文档与发布说明沿用旧栈能力描述，没有按新 featureSpec 重写；翻译目录先于消费点生成且允许未使用键，旧文案因此长期留存。
- 影响：用户看到不存在或与产品排除项冲突的能力；产品定位被旧栈描述稀释；发布说明不能作为可核验交付清单。
- 证据：README 与发布说明中的上述能力条目；`prepare_document_open` 只接受 Markdown 与可映射源码，图片被拒绝；Diff 规格允许工作区编辑；featureSpec 页面列表与旧页面名不一致；i18n 的 `settings.sidebar.profiles`、`advanced`、`localization`、`backup.category_command_history`、`theme_editor` 等没有对应生产消费点；featureSpec 全文没有壁纸与屏幕检测所有者。
- 优化方向：以 featureSpec 为唯一能力来源重写用户文档、CHANGELOG 与发布说明；未定义但已实现的能力要么补入规格，要么从产品与文案中移除；清理未消费的 i18n 键或为它们建立消费点。

### 重要｜被排除的图片面板实现仍编译存在，最终排除项与验收锚未覆盖

- 置信度：确认
- 位置：`docs/featureSpec/acceptance.md`「文档与 Git」最终排除项；`slterm_app/src/app_shell/doc_tabs.rs` 的 `ImageTabView`、`image_viewable`、`openable_in_app`；`slterm_app/src/app_shell/app_root/panel_host.rs` 的 `PanelContent`；`slterm_app/src/app_shell/app_root/documents.rs` 的 `prepare_document_open`；`slterm_app/src/acceptance.rs` 的 `acceptance_excluded_surface_absent`。
- 现象：规格明确不提供 Image 面板，图片按系统默认程序打开。实现仍保留完整的图片标签视图（缩放、平移、解码、渲染）与应用内图片路由判定；当前文档打开路径拒绝图片，因此该视图没有生产入口，但代码仍被编译。`acceptance_excluded_surface_absent` 只扫描少量文件中的旧通道字样，没有发现该残留。
- 根因：旧图片查看器未随排除项删除；最终排除项验收只做了有限源码字符串检查。
- 影响：排除项存在隐藏实现；一旦后续接线就会重新引入被排除的能力；死代码增加维护与审计成本，验收锚却给出“排除面不存在”的错误结论。
- 证据：`ImageTabView` 与图片几何状态机仍在；`prepare_document_open` 对非 Markdown／非源码文件返回不支持；`PanelContent` 没有 Image 变体；`acceptance_excluded_surface_absent` 只检查 `lib.rs`、`app_shell/mod.rs`、`persistence/mod.rs` 的若干字符串。
- 优化方向：删除被排除的图片查看器，或将其明确登记为待删除残留并调整验收锚；排除项检查应覆盖所有应用内打开路由与面板类型入口，而不是固定文件清单。

### 重要｜featureSpec 未覆盖已实现或已宣传的壁纸与屏幕检测能力

- 置信度：确认
- 位置：`docs/featureSpec/README.md`「产品范围」、`docs/featureSpec/system-integration.md` 所有者边界、`docs/featureSpec/panels/settings.md`「外观页」；`README.md` 与 `release-notes/v0.3.0.md`；`slterm_app/src/app_shell/wallpaper.rs`；`slterm_app/src/agent_host.rs` 的 `observe_screen`；`slterm_app/src/ai_agents.rs` 的屏幕规则。
- 现象：壁纸与屏幕检测都有实现与用户文档，但 featureSpec 没有对应所有者章节、行为判据或验收场景；系统集成篇只拥有窗口材质，明确没有把壁纸纳入；终端篇只承认命令边界、hooks 与状态信号，没有定义屏幕检测。
- 根因：产品规格声称覆盖完整产品能力，但实现沿旧栈保留了额外能力，规格没有同步或明确排除。
- 影响：这些能力没有产品合同与验收边界，后续无法判断其合法性；用户文档与 featureSpec 对同一产品描述不一致；屏幕规则与 hook 权威状态的关系缺乏规格约束。
- 证据：featureSpec 全文检索不到壁纸与屏幕检测；`wallpaper.rs` 与 `agent_host::observe_screen`／`ai_agents` 屏幕规则存在；README 与发布说明宣传壁纸和屏幕检测；终端规格只列命令边界、hooks 与状态信号。
- 优化方向：为已实现能力补充所有者规格与验收，或从实现和用户文案中删除；不得让未登记能力长期处于规格之外。

### 重要｜Mermaid 终态在 featureSpec、devPlan 与进度之间构成合同冲突

- 置信度：确认
- 位置：`docs/featureSpec/panels/markdown.md`「安全 HTML、公式与 Mermaid」；`docs/featureSpec/acceptance.md`「最终排除项」与「总验收判据」；`docs/devPlan/plan.md`「决策与红线」的 Mermaid 边界；`docs/devPlan/design/markdown.md`「解析与块投影」；`docs/devPlan/traceability.md` 的 Mermaid 行；`docs/devPlan/progress.md` 的状态锚。
- 现象：featureSpec 把 Mermaid 静态渲染列为最终能力，并明确它不得被误列为排除项；devPlan 又把 Mermaid 保持为普通代码块，称最终渲染由用户后续单独实现；追溯矩阵把相关场景标为延期；进度记录仍宣布全部阶段验收完成。
- 根因：产品终态、实施边界与验收状态三者没有统一的真值；延期裁决只存在于追溯矩阵的说明中。
- 影响：用户会按规格期待 Mermaid 静态渲染，而产品不提供；全量验收声明与最终排除项判据同时不成立。
- 证据：featureSpec 的 Mermaid 能力与总验收判据；实施计划的 Mermaid 边界；设计文档“不在本轮实现范围”；追溯矩阵的延期状态；进度记录的“全部阶段验收完成”。
- 优化方向：明确 Mermaid 是最终能力还是已裁决延期；若延期，进度与验收必须保留未完成状态并登记豁免；若不延期，按规格实现并替换延期条目。

### 重要｜全量验收锚与追溯映射不能证明所声明的场景

- 置信度：确认
- 位置：`slterm_app/src/acceptance.rs` 的 `acceptance_user_contract_observable`、`acceptance_excluded_surface_absent`、`integration_single_authority_channels`、`acceptance_exemptions_registered`、`sensitive_material_excluded_from_outputs`；`docs/devPlan/traceability.md` 的跨域验收、加密备份与实现回归映射；`docs/featureSpec/acceptance.md` 的跨域验收与总验收判据。
- 现象：多个最终验收锚实际只做单一事实检查或源码字符串断言：用户契约锚只检查工作区库的 `clean_shutdown`；单点权威锚检查若干源文件是否包含特定函数名与不包含特定字符串；排除面锚只扫描少量模块的固定禁用词；豁免锚只检查豁免表包含结论列与“保留”字样；敏感材料锚只覆盖一条诊断事件与一条审计事件。追溯矩阵把这些用例当作跨域场景的最终核销证据。备份工作区场景的映射还与测试实际类别不一致。
- 根因：全量验收锚被实现为轻量的元测试，未覆盖 featureSpec 的用户可观察行为；追溯矩阵只核对例名存在，没有核对前置、动作与可观察结果是否一致。
- 影响：全量验收的结论缺少行为证据，源码字符串或单点状态通过即可掩盖缺失的用户面、备份类别与排除项。
- 证据：上述测试的实际断言范围；`backup_workspace_category_exclusions` 选择的是 `Session` 而非工作区类别；`backup_cross_machine_requires_relocation` 是布局恢复用例而非备份用例；`acceptance_excluded_surface_absent` 没有扫描图片标签等排除残留。
- 优化方向：把这些锚降级为源码守卫或补充真实行为／集成证据；追溯矩阵应逐条核对测试前置、动作与可观察结果，而不是只核对例名。

### 重要｜测试豁免表含失效设计路径、目标态待实现项与超范围无障碍项

- 置信度：确认
- 位置：`.claude/docs/testing/exemptions.md`「目标态设计声明的人工验收面」「条件跳过用例」「既定豁免清单」；`docs/featureSpec/acceptance.md` 最终排除项；`slterm_app/src/encrypted_backup/CLAUDE.md`「验收与测试」；`slterm_terminal/src/event_loop.rs` 与 `slterm_app/src/background_tasks` 的引用。
- 现象：豁免表仍引用不存在的 `design/ux-framework.md`、`design/panels/*.md`、`design/platform-windows.md` 等目标态设计路径；目标态表标注“未落地前不视为已有覆盖”，但进度记录已宣布全部验收完成；其中侧栏分隔线 AccessKit 激活与屏幕阅读器数值播报属于 featureSpec 明确排除的专用无障碍能力。加密备份模块文档要求真实备份／恢复 UI 流程按豁免登记，但豁免表没有对应条目。个别被引用路径也不存在，例如 `slterm_terminal/src/event_loop.rs` 与 `slterm_app/src/background_tasks` 目录写法。
- 根因：豁免表混合了旧目标态草稿、现行豁免与未完成事项，复审规则没有与实际验收状态同步。
- 影响：豁免表不再是可信的单表真值源；未登记的用户面缺口与超范围能力被豁免掩盖；死路径降低可审计性。
- 证据：目标态表的路径在仓库中不存在；表头自述“未落地前不视为已有覆盖”；AccessKit 行与 featureSpec 排除项冲突；加密备份模块文档的 UI 人工验收声明在豁免表中找不到对应行；被引用文件实际为 `event_loop` 目录与 `background_tasks.rs`。
- 优化方向：删除目标态草稿表或迁移为当前有效登记；修复路径引用；对备份恢复 UI 与 AccessKit 行为重新裁决并登记或删除；豁免表只保留真实不可自动化边界。

### 重要｜架构总览与常驻规则仍使用旧栈数据名、类型名与已取消的校验

- 置信度：确认
- 位置：`.claude/docs/architecture/overview.md`「workspace 成员与 ownership」「数据根与命名」「slterm_settings 行」；`.claude/CLAUDE.md`「面板封闭」「布局单点」；`.claude/docs/architecture/provenance.md`「数据零迁移」；对照 `slterm_path/src/app_paths.rs`、`slterm_workspace/src/view_key.rs`、`slterm_app/src/app_shell/app_root/panel_host.rs`、`architecture/dependencies.toml`。
- 现象：架构总览把 `session.json`、`history.jsonl`、`directory_history.json`、`instance` 列为当前数据文件，并声称 `slterm_settings` 承担 WCAG 校验；实际 `AppPaths` 只定义 `settings.json`、`workspace.db`、`workspace.backup.db`、`learning.db`，学习数据在 `learning.db`，WCAG 是已取消能力。workspace 成员表只列部分 crate，却作为成员与 ownership 的入口。根常驻规则把面板封闭类型写成 `WorkspaceTab`，把布局单点写成“session schema 的 layout 转换对”，二者都没有对应现行类型；实际是 `PanelContent`／`ViewKey` 与 `ProjectLayout`。沿革文档称新版“只读写 `%APPDATA%\slterm`”，与双数据根模型不一致。
- 根因：常驻文档没有随终态数据根、类型锚点与主题约束更新，保留了旧 WebView 栈的命名和规则。
- 影响：新会话会按失效类型名或旧文件布局决策；文档与代码的事实基线不一致，门禁又只检查编号与链接，无法发现这类过期事实。
- 证据：`AppPaths` 的方法集合；`PanelContent` 与 `ViewKeyKind` 的封闭集合；`ProjectLayout` 的持久 DTO；`architecture/dependencies.toml` 的成员清单；overview 与 provenance 的原文。
- 优化方向：按现行类型、数据根与主题红线重写这些常驻段落；成员表应指向依赖策略而不是手抄部分清单；沿革文档的路径结论要同时写明配置根与本地根。

### 一般｜devPlan 的 core crate 依赖方向与实际策略相反

- 置信度：确认
- 位置：`docs/devPlan/design/architecture.md`「crate 边界」；`architecture/dependencies.toml`；`slterm_workspace/Cargo.toml`；`slterm_agents/Cargo.toml`。
- 现象：设计文档称 `slterm_agents` 可依赖 `slterm_workspace`，而实际依赖是 `slterm_workspace` 依赖 `slterm_agents`；`slterm_agents` 只依赖 path 与 settings。
- 根因：类型归属与依赖方向在文档中未按最终实现修正。
- 影响：按设计文档新增 workspace 类型或调整 agent 类型时可能制造循环依赖；架构审计会面对互相矛盾的两份合同。
- 证据：依赖策略与两个 crate 的 Cargo 依赖；设计文档同段的 ID 所有权描述也隐含 agents → workspace。
- 优化方向：以机械策略与实际依赖修正设计文档，并同步说明 `AgentKind`／`AgentSessionKey` 归属带来的方向。

### 一般｜devPlan 的 Runtime 方法表写错 `operation.get`／`operation.wait`

- 置信度：确认
- 位置：`docs/devPlan/design/runtime.md`「资源与方法面」「写操作与幂等」；对照 `assets/runtime-control-api.md` 与 `slterm_runtime_protocol/src/protocol.rs` 的 `#[serde(rename = "runtime.operation_get")]`、`runtime.operation_wait`，以及 `slterm_runtime_protocol/src/method.rs` 的注册表。
- 现象：设计文档把查询方法写成 `operation.get`、`operation.wait`，实际线缆方法名是 `runtime.operation_get`、`runtime.operation_wait`；随包参考文档与协议 crate 使用实际名称。
- 根因：方法表在实现改名后未同步。
- 影响：客户端或 conformance 实现会按错误方法名调用；设计文档与 schema 真值冲突。
- 证据：Serde 重命名、方法注册表与随包合同文档；设计文档表格与正文均使用短名。
- 优化方向：以协议 crate 与随包 schema 为唯一真值修正设计文档方法表，并增加方法名一致性检查。

### 一般｜进度与追溯文档保留测试通过计数、过期状态锚与计划期自述

- 置信度：确认
- 位置：`docs/devPlan/progress.md` 的 Agent 与 Hooks 验收行；`docs/devPlan/traceability.md`「决策与红线」状态锚与首段说明；`docs/devPlan/README.md` 的进度规则。
- 现象：进度记录的验收证据写入具体通过用例数与脚本测试数，属于代码可自证的快照计数；追溯矩阵自称“目标例名是实施计划，不代表测试已经存在或通过”，但全表状态已为已自动化；状态锚仍写过去某个日期，而进度记录的验收日期更晚。
- 根因：证据记录沿用过程期的统计与计划措辞，没有按最终文档规范收敛。
- 影响：计数迅速过期且无法审计；读者无法判断追溯矩阵是计划、证据还是当前状态；状态锚失去可信度。
- 证据：进度记录中的 passed／tests 数量表述；追溯矩阵的状态列与首段、状态锚原文；进度记录的状态锚日期。
- 优化方向：用可重跑命令、日志路径或例名替代计数；把追溯矩阵改为当前证据状态并更新锚点；删除“计划锚”与“已自动化”并存的歧义表述。

### 一般｜模块与设计文档存在第二份源码式复述，已出现与实现漂移

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/CLAUDE.md` 的关闭、Runtime、文档打开等长条目；`slterm_app/src/persistence/CLAUDE.md` 的 schema 与事务描述；`slterm_app/src/local_control/CLAUDE.md` 的完整方法面描述；`slterm_document/CLAUDE.md` 的编码与分页内部细节；`docs/devPlan/design/settings-recovery.md` 对 featureSpec 备份类别与参数的复述。
- 现象：这些文档逐项复述代码中的阶段顺序、函数名、字段、协议方法与事务边界，篇幅与实现同步增长；同时已经出现文档与代码不一致（备份类别、WCAG、主题字段、Settings 页面、Runtime 方法名）。
- 根因：文档被当作实现说明书维护，而不是只记录决策、原因、边界与外部限制。
- 影响：每次实现改动都需要同步大段叙述，实际同步失败即产生失真文档；读者难以区分稳定决策与易腐实现细节。
- 证据：上述文档中的方法级、字段级与状态机级复述；备份、主题与设置页面已出现事实冲突。
- 优化方向：把模块文档收敛为决策与红线、外部依赖的坑、操作边界与豁免；实现路径、字段与协议细节以代码、生成 schema 和测试为准。

### 一般｜CHANGELOG 版本状态与发布说明、打包证据不一致

- 置信度：确认
- 位置：`CHANGELOG.md` 的 0.3.0 小节标题；`release-notes/v0.3.0.md`；`release-notes/unreleased.md`；`.claude/docs/release/packaging.md` 的发布证据；`release-notes/CLAUDE.md` 的同步纪律。
- 现象：CHANGELOG 把 0.3.0 标为“未发布”，但发布说明已按版本归档，打包证据记录了首发安装器与便携包核验。同一版本资产在发布说明与后续打包证据中出现两组不同的 SHA256，文档没有说明哪一组是当前有效资产。发布说明的 SHA 块写“预期条目”，与“由 verify-release 生成并填入”的纪律关系不清。
- 根因：版本状态与发布证据没有单一锚点；重建同一版本后没有更新发布说明或明确版本状态。
- 影响：用户与发布审计无法判断 0.3.0 是否已发布、当前资产是哪一组；更新链的哈希预期可能指向旧产物。
- 证据：CHANGELOG 标题、发布说明归档、打包证据中的两组哈希与日期、发布说明的 SHA 说明文字。
- 优化方向：确定 0.3.0 的发布状态并同步 CHANGELOG、发布说明与打包证据；用日期／commit 锚区分历史重建与当前资产；SHA 块只保留当前有效生成结果。

## 跨域观察

- 安装器与发布包实机项在进度记录中按豁免留待发布机，但发布范围仍标记已验收，而实施计划的发布退出证据要求安装器与便携包验收通过；该证据核销与状态表述的冲突应由证据审查一并处理。
- `encrypted_backup` 模块文档写明恢复与撤回 UI 尚未接线，进度记录却把备份恢复能力记为通过；该矛盾与备份类别问题同源。
- `slterm_app/src/ai_sessions.rs`、`settings_transfer.rs`、`plan_balance` 服务等存在完整实现与测试，但在生产 UI 中没有找到消费者；这与“全部阶段验收完成”的表述形成系统性的接线缺口。
- `.claude/worktrees/` 下存在旧 WebView 栈文档，但未被 git 跟踪；本次审查未将其计入范围。若这些文件继续留在工作树，文档检索与人工阅读可能再次引入旧栈语义。

## 存疑未证

- `ThemeDefinition::validate_wcag` 未找到生产调用点；是否存在未检索到的 UI 或动态入口会实际执行对比度拒绝，需要运行时或调用图证据。
- `SettingsPageId` 是否只是过渡 UI、是否存在另一套目标设置界面未纳入抽样，需要完整 crate 入口证据。
- 备份导出／恢复是否有一条未接线的隐藏入口（例如命令面板或测试专用通道），需要 GUI 实机或入口检索确认。
- Mermaid 延期是否确有用户裁决记录；追溯矩阵声称按用户裁决延期，但未给出决策锚点。
- 窗格缩放、壁纸与屏幕检测是产品意图还是旧栈残留；featureSpec 未定义，需产品裁决后才能判断是补规格还是删除。
