# 开发计划完成情况真实性审查

> 基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`；只核验 docs/devPlan、featureSpec、测试豁免、发布说明与对应代码证据链，未运行构建、测试或网络请求。

## 覆盖与局限

- 已覆盖：`docs/devPlan/` 全部计划、进度、追溯与设计文档；`docs/featureSpec/` 全部所有者与验收文档；`.claude/docs/testing/exemptions.md`；`release-notes/` 与 `CHANGELOG.md`；根 `CLAUDE.md`；`slterm_app` 的导航、设置、历史隔离、套餐、诊断、清理、更新与验收锚代码；`slterm_settings` 的主题模型；`git log`、提交内容与短 hash 解析。
- 核对方法：对进度表 hash 做祖先与短 hash 唯一性解析；对追溯矩阵例名做精确函数名检索；对 featureSpec 所有者验收段与追溯表行做逐段对照；对关键产品入口做静态调用方检索。
- 未覆盖：未运行 cargo、测试、clippy、门禁脚本与真实 GUI；未实测安装器、更新移交、ConPTY 与发布包；未逐行业务逻辑审查，只核验计划证据链与代表性产品入口。测试“全绿”与通过数量无法由本审查复现。

## 问题

### 阻断｜导航树所有者未落地，进度与追溯把它写成已自动化

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/nav.rs` 的 `NavRuntime` 与 `render_nav`；`slterm_app/src/app_shell/app_root/navigation.rs`；`slterm_app/src/ai_sessions.rs`；`slterm_app/src/plan_balance/service.rs`；`slterm_app/src/history_quarantine.rs`；`docs/devPlan/progress.md` 的 `Agent 与 Hooks` 行；`docs/devPlan/traceability.md` 的 `导航树` 全部行
- 现象：导航视图的运行时只有 `projects` 字段，渲染只有项目行、激活与移除入口；没有活跃会话行、历史节点、未归属节点、搜索、状态列、套餐页脚与“已删除会话”入口。历史归属、恢复 cwd、恢复项目确认、注入超时全部是带“消费点归导航历史视图”标记的未消费函数；`ai_sessions` 模块整体标记为未消费；`PlanBalanceService` 只被自身测试构造；历史隔离服务只被自身测试调用。
- 根因：计划把“历史扫描／恢复／隔离／套餐”写成行为切片，但实现停在纯函数、状态机与单元测试层；进度把“锚全量自动化”直接当成产品行为验收，追溯矩阵沿用同名单元测试把状态改为已自动化。
- 影响：featureSpec 的导航树所有者整体不可用，用户无法查看活跃会话与历史会话、执行恢复／分支、撤销隔离删除或查看套餐页脚；全量验收的核心场景不成立，进度状态失真。
- 证据：`NavRuntime` 只含 `projects`；`render_nav` 只生成项目列表；`navigation.rs` 的 `deepest_project_for_cwd`、`resolve_restore_cwd`、`resolve_restore_project`、`restore_injection_timeout` 均带未消费标记；`lib.rs` 对 `ai_sessions` 注明“手动恢复面板的消费点归后续”；`slterm_sidebar` 的导航持久状态只有 `projects`、`unassigned_expanded`、`scroll`，没有历史或活跃会话数据；`history_quarantine.rs` 与 `plan_balance/service.rs` 的生产调用方检索为空。
- 优化方向：先明确导航树所有者是延期还是实现；若延期，按 featureSpec 所有者登记延期与理由并回退相关追溯状态；若实现，需以真实导航视图和持久窗口为验收对象，而不是继续用纯函数用例充当产品验收。

### 阻断｜设置中心未实现 featureSpec 页面与操作，旧设置与排除项仍在线

- 置信度：确认
- 位置：`slterm_app/src/settings_state.rs` 的 `SettingsPageId`；`slterm_app/src/app_shell/app_root/settings_controls.rs` 的 `FIELDS`、`SettingsControl`、`CURSOR_SHAPES`、`WALLPAPER_FITS`；`slterm_app/src/app_shell/app_root/settings_overlay.rs` 的 `render_page`；`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition`、`ThemeTypography`、`ThemeLayout`、`ThemeEffects`、`validate_wcag`；`docs/featureSpec/panels/settings.md`；`docs/featureSpec/acceptance.md` 最终排除项
- 现象：设置页集合是 `Appearance`、`Scrolling`、`Interaction`、`Session`、`System`、`AgentHooks`、`Hooks`，而 featureSpec 要求快捷键、后台定时任务、外观、终端、应用、Agent 基础配置与 Hooks User；当前没有快捷键页、后台任务页、终端的完整功能页、应用页的更新／诊断／备份／Runtime 状态入口，也没有导入设置、导出设置、恢复所有默认、主题导入／重命名／删除、AgentRoot 管理入口。字段表仍暴露 `cursor_shape`（featureSpec 明确不提供光标形状设置）、`background_image_*`、`pane_card_*`、`density` 等旧设置；主题模型仍携带排版、布局与效果字段，且保留WCAG对比度校验函数；`theme_library/CLAUDE.md` 还要求新增主题通过WCAG校验。
- 根因：旧产品设置界面被保留，新增的设置存储、主题库、备份、诊断、套餐等只有引擎与单元测试，没有按 featureSpec 重建页面和动作；追溯矩阵把引擎测试当作页面场景。
- 影响：设置中心这一核心所有者不成立；用户无法执行备份恢复、设置导入导出、主题导入、AgentRoot 管理、诊断清理与 Runtime 状态查看；最终排除项中的光标形状设置与对比度校验仍以代码和设置项存在。
- 证据：`SettingsPageId::ALL` 的变体集合与 featureSpec 页面顺序不一致；`FIELDS` 含 `cursor_shape`、`background_image`、`background_image_fit`、`background_image_alignment`、`pane_card_radius`、`pane_card_gutter`、`pane_card_shadow`；`SettingsView::render_page` 只按字段表渲染，未注册导入／导出／备份／诊断／更新／Runtime／AgentRoot 动作；`ThemeDefinition` 含 `typography`、`layout`、`effects`；`validate_wcag` 与 `WCAG_TEXT_CONTRAST` 仍在；`theme_library/CLAUDE.md` 的验收要求与 featureSpec“不做对比度校验”冲突。
- 优化方向：按 featureSpec 重建页面与动作，或将该所有者整体登记延期；删除旧设置项与旧主题字段，统一主题模型与对比度纪律；追溯状态在真实页面验收前不得标为已自动化。

### 重要｜隔离到期清理、导出与永久删除没有实现，计划切片无落点

- 置信度：确认
- 位置：`docs/devPlan/plan.md` 的 `隔离到期清理与 ZIP 导出`、`隔离永久删除确认`；`slterm_app/src/history_quarantine.rs`；`docs/featureSpec/workspace/navigation.md` 的删除隔离区与撤销规则
- 现象：历史隔离模块只有批次登记、完成、撤销窗口、恢复预检与对账纯函数；`expired` 只返回过期批次标识，没有到期清理、ZIP 导出、永久删除或用户入口。相关计划行既无追溯行，也无豁免登记，进度记录只写“隔离批次与撤销”。
- 根因：只实现了撤销窗口状态机，没有接维护期清理、导出与永久删除；计划验收锚没有落到例名或人工记录。
- 影响：已删除历史永远不会被永久清除，隔离材料无界累积；featureSpec 要求的“导出隔离记录”和“明确永久删除”不可用；计划切片被静默跳过，进度却标记相关阶段已验收。
- 证据：`history_quarantine.rs` 的公开函数集合不含导出、清理或永久删除；全仓检索 `quarantine` 与 `export`/`permanent` 组合无实现；`expired` 的调用方检索为空；相关计划行没有反引号例名，追溯矩阵没有对应章节。
- 优化方向：补实现并接入导航或维护期，或将其登记为延期并回退阶段状态；无论哪种都必须让计划行、追溯行与豁免记录一致。

### 重要｜Runtime v2 的验收 hash 指向 Windows 阶段提交，阶段证据不成立

- 置信度：确认
- 位置：`docs/devPlan/progress.md` 的 `Runtime v2` 行；提交 `4c77058549cc1ea3156ce373f6e33cfafc1dc312`；`docs/devPlan/traceability.md` 的 Runtime 行
- 现象：进度把 Runtime v2 的验收基线写成 `4c770585...`，该提交消息为“诊断白名单与脱敏导出”，属于 Windows 与发布阶段；Runtime 自身的最后实现与文档提交是 `b47e9b92`（单实例同构验收）和 `e4af2a19`（Runtime 控制面合同），均早于 Windows 阶段开始的提交。该提交同时修改了 Runtime 的 token 追溯行，与“阶段验收基线”语义混在一起。
- 根因：阶段验收记录延迟，hash 取当时 HEAD 而不是 Runtime 完成或验收提交；没有独立的 Runtime 验收文档提交。
- 影响：Runtime 阶段声称的全量方法与单实例同构无法在该 hash 对应的工作树上被定位；进度表阶段顺序与证据锚不一致；最终“全部阶段验收完成”的审计链出现断点。
- 证据：`git log` 显示 `4c77058` 的修改面为诊断、`slterm_app/CLAUDE.md` 与追溯单行；Runtime 方法族提交在更早时间完成；进度表 Windows 行使用更晚的 `9c76616b`。
- 优化方向：为 Runtime 阶段指定与其自身实现/验收对应的提交，或记录独立验收文档；在进度说明中明确 hash 的语义，不能用后续阶段提交充当基线。

### 重要｜全量验收锚以源码字符串和局部状态代替跨域行为

- 置信度：确认
- 位置：`slterm_app/src/acceptance.rs` 的 `acceptance_user_contract_observable`、`integration_single_authority_channels`、`acceptance_excluded_surface_absent`、`acceptance_exemptions_registered`、`performance_claims_have_conditions`、`integration_platform_boundary_failure`；`docs/devPlan/traceability.md` 的跨域验收与跨域集成行
- 现象：`acceptance_user_contract_observable` 只验证工作区库的 `clean_shutdown` 标志；`integration_single_authority_channels` 只检索源码是否包含若干函数名与字符串；`acceptance_excluded_surface_absent` 只扫描少数模块文件中的若干旧标识，未覆盖旧设置、旧图片路由、旧 Diff 等实际排除面；`acceptance_exemptions_registered` 只统计豁免表行数；`performance_claims_have_conditions` 在文件内首次出现日期后把后续所有行都视为有锚点；`integration_platform_boundary_failure` 只使用假平台端口验证模糊材质、托盘不可用与缺哈希资产。
- 根因：把跨域验收简化为可静态通过的源码检索、单点状态或文档计数，未构造真实双进程、跨服务、崩溃或平台边界场景。
- 影响：全量验收的“锚自动化核销”不能证明单一权威、崩溃恢复、平台降级、排除面与敏感材料；隐藏的第二通道或旧入口可以通过这些断言；进度中的全量验收结论缺乏行为证据。
- 证据：`production_source` 只读取首个 `#[cfg(test)]` 前的文本；`integration_single_authority_channels` 断言 `contains`；`acceptance_excluded_surface_absent` 只遍历少数入口文件与有限字符串；`performance_claims_have_conditions` 的 `dated_section` 一旦置真不再复位；`acceptance_exemptions_registered` 只匹配表格行文本。
- 优化方向：以真实集成场景替换静态检索式验收；每个跨域锚应包含可观察副作用、失败路径与恢复路径，而不是源码字符串或文档行数。

### 重要｜追溯矩阵有行引用不存在的测试例名

- 置信度：确认
- 位置：`docs/devPlan/traceability.md` 的 `PathPolicy` 行 `path_file_identity_deduplicates_document`、`Commit` 行 `commit_rollback_preserves_staged_changes`、`Agent 全局文件` 行 `agent_files_default_capabilities`
- 现象：上述例名在全仓没有任何 `fn` 定义，也没有同名模块或宏生成；它们只出现在计划、进度和追溯文本中。对应的实际测试是别处已有的 `integration_document_global_identity`、`git_restore_from_index_keeps_staged_changes`、`agent_root_default_capabilities`，但矩阵没有指向它们。
- 根因：例名改名或跨层迁移后未同步矩阵，状态却已改成已自动化；没有执行“复审时核验引用存在”的检查。
- 影响：这些 featureSpec 验收场景无法按矩阵定位到真实测试，最终“每个场景都有实际测试或豁免”的结论不成立。
- 证据：精确函数名检索仅命中文档；上述替代测试的实际行为分别覆盖文件身份去重、索引恢复保留 staged、AgentRoot 默认能力，但与矩阵引用的场景名不一致。
- 优化方向：把矩阵指向真实例名，或补充缺失测试；在例名替换前回退状态，不要保留已自动化。

### 重要｜部分矩阵例名存在，但覆盖的不是声明场景

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/navigation.rs` 的 `navigation_transient_and_quarantine_restore`、`navigation_restore_fallback_and_injection_retry`、`navigation_unowned_restore_requires_project_confirmation`；`slterm_app/src/plan_balance/mod.rs` 的 `plan_balance_stale_after_full_period`；`slterm_app/src/settings_state.rs` 的 `settings_page_view_state_restore`
- 现象：导航相关用例只验证持久状态的键集合与纯函数返回值，不验证搜索、多选、恢复对话框、Shell 就绪等待或真实注入；`plan_balance_stale_after_full_period` 只测 `balance_stale` 辅助函数，没有覆盖过期状态下的旧值保留、最后成功时间与页面呈现；`settings_page_view_state_restore` 使用的是旧 `Session` 页，不是 featureSpec 页面。
- 根因：用例名对齐了验收场景，内容停留在底层纯函数或旧页面；追溯矩阵以名字为准，不检查前置条件、动作与可观察结果。
- 影响：这些行给出的“已自动化”是语义漂移的假阳性；导航、套餐与设置的产品行为仍然没有覆盖。
- 证据：上述测试体只构造纯状态或查询常量；设置测试断言 `SettingsPageId::Session` 与旧页参数；导航测试没有构造视图或对话框。
- 优化方向：按场景重建集成用例，或将矩阵行改成实际覆盖的底层范围并明确降级；不能用名字相同替代行为相同。

### 重要｜Windows 与发布阶段的服务没有产品入口，进度却按切片验收

- 置信度：确认
- 位置：`slterm_app/src/app_shell/app_root/update_install.rs` 的 `begin_update_install`、`cancel_update_install`；`slterm_app/src/diagnostics.rs`；`slterm_app/src/cleanup.rs`；`slterm_app/src/plan_balance/service.rs`；`slterm_app/src/app_shell/app_root/settings_controls.rs`；`docs/devPlan/progress.md` 的 `Windows 与发布` 行
- 现象：`begin_update_install` 与 `cancel_update_install` 均带“消费点 = 更新提示 UI”的未消费标记；诊断、清理与套餐服务没有设置页或工作区调用方；设置字段表没有诊断、清理、备份、更新状态或 Runtime 状态页面。进度仍写“平台接入、套餐调度、诊断白名单、清理切片全绿”。
- 根因：Windows 阶段按服务层与单元测试验收，未接产品入口；设置中心页面缺失使这些服务无法被用户触发。
- 影响：更新安装、诊断导出、清理、套餐页脚与 Runtime 状态在真实产品中不可达；阶段“已验收”只代表引擎测试通过。
- 证据：`update_install` 的安装与取消入口带未消费标记；`diagnostics`、`cleanup`、`plan_balance::service::PlanBalanceService` 的生产调用方检索为空；设置页没有对应动作或页面。
- 优化方向：接入真实入口并补产品级测试，或在进度与追溯中明确这些能力仍待接线，不能按已验收登记。

### 重要｜“全部阶段验收完成”与安装器、发布包实机待办直接矛盾

- 置信度：确认
- 位置：`docs/devPlan/progress.md` 的 `Windows 与发布` 与 `全量验收` 行；`.claude/docs/release/packaging.md` 的阶段回归证据；`.claude/docs/testing/exemptions.md` 的真实 setup.exe 安装条目；`scripts/release-checklist.md`
- 现象：进度把 Windows 与发布、全量验收都标为已验收，但 Windows 行同时写“安装器与发布包实机项按豁免留发布机”；打包文档写明本机未安装 Inno Setup，安装器重建与真实安装／更新链待发布机执行，并明确“不得以本次钉测试结果冒充”；豁免表只登记人工兜底，没有完成记录。
- 根因：把自动化钉测试和豁免登记当成阶段验收完成；人工核验项没有执行记录，也没有在进度中作为未决项阻塞。
- 影响：全量验收的发布包、安装器与更新链结论没有真实证据；对外发布状态可能早于实际验证。
- 证据：`packaging.md` 的“本机未安装 Inno Setup”“不得以本次钉测试结果冒充”；进度行的“按豁免留发布机”；豁免表条目的兜底为人工验收而非已完成记录。
- 优化方向：在人工项完成前保持阶段为进行中或阻塞，或在发布机执行后补充带日期的核验记录；追溯矩阵应为这些人工面建立人工豁免行。

### 重要｜发布说明与 CHANGELOG 仍在描述旧产品能力，与 featureSpec 排除项冲突

- 置信度：确认
- 位置：`release-notes/v0.3.0.md`；`CHANGELOG.md` 的 `0.3.0` 小节；`docs/featureSpec/acceptance.md` 的最终排除项；`docs/featureSpec/panels/settings.md`
- 现象：发布说明宣称图片标签页、窗格缩放与拖拽停靠、配置／本地化／高级／重置页、窗口壁纸、历史扫描等能力；featureSpec 明确排除 Image 面板，设置页面清单也不含这些旧页面，导航历史在产品中不可用。发布说明的“只写可核验结果”与当前交付面不一致。
- 根因：发布说明沿用旧产品文本，未随目标规格与实现重写。
- 影响：用户看到的能力承诺与产品契约冲突；发布说明不是可核验事实，最终验收的文档边界不成立。
- 证据：`v0.3.0.md` 与 `CHANGELOG.md` 的 Added 条目；`doc_tabs::openable_in_app` 仍接受图片的旧测试；设置页枚举与 featureSpec 页面清单的差异；导航历史视图缺失。
- 优化方向：按当前 featureSpec 与真实交付重写发布说明，或明确标注未交付能力；旧能力不得以已发布事实出现。

### 合同冲突｜Mermaid 最终静态渲染：featureSpec 要求 vs devPlan 延期

- 置信度：确认（文档冲突）；用户裁决本身疑似
- 位置：`docs/featureSpec/panels/markdown.md` 的 Mermaid 验收；`docs/featureSpec/acceptance.md` 的总验收判据；`docs/devPlan/README.md`；`docs/devPlan/design/markdown.md`；`docs/devPlan/traceability.md` 的 `延期` 行
- 现象：featureSpec 把 Mermaid 静态渲染列为最终能力，并要求不得被误列为排除项；devPlan 设计与 README 明确本轮不实现，追溯标为延期；进度仍称全部阶段验收完成。追溯中的“按用户裁决”没有日期、范围或决策记录。
- 根因：需求真值与实现计划对同一能力采用了不同状态；用户裁决只以叙述存在，没有可核验锚点。
- 影响：按 featureSpec 判据，全量验收不可能同时成立；按 devPlan 判据，功能被有意延期但未反映在最终进度。两种口径不可同时为真。
- 证据：featureSpec 的 Mermaid 验收段与总判据；devPlan README 的“最终静态渲染由用户后续单独实现”；design/markdown 的“不在本轮实现”；traceability 的延期行；progress 的“全部阶段验收完成”。
- 优化方向：补充带日期和范围的用户裁决记录，并据此更新 featureSpec 或最终验收口径；在冲突解决前不要把全量验收标为完成。

### 重要｜旧实现与最终排除面未清理，仍有隐藏入口或在线能力

- 置信度：确认
- 位置：`slterm_app/src/app_shell/mod.rs` 的 `doc_tabs`、`code_tab`、`diff_tab`、`wallpaper` 模块声明；`slterm_app/src/app_shell/doc_tabs.rs` 的 `openable_in_app`、`image_viewable`；`slterm_app/src/app_shell/diff_tab.rs` 的 `DiffTabView`；`slterm_app/src/app_shell/windowing.rs` 与 `theme.rs` 的壁纸调用；`slterm_settings/src/custom_theme.rs` 的 `ThemeDefinition`
- 现象：旧图片路由仍把图片视为可在应用内打开，并有测试断言该行为；旧双栏只读 `InputState` Diff 仍存在；旧模块以未消费标记保留；壁纸仍在窗口初始化与主题刷新链中运行；主题模型仍含排版、布局与效果字段。设计文档要求删除旧 Diff 双缓冲、背景图片、排版与效果字段，featureSpec 排除 Image 面板与对比度校验。
- 根因：迁移处置表没有执行完毕，`dead_code` 标记被当作保留手段；最终排除项验收只做有限字符串检索，未覆盖这些模块。
- 影响：最终排除面存在隐藏旧实现，产品仍暴露被排除的外观能力；迁移合同与“单一权威、无第二通道”目标被削弱。
- 证据：`doc_tabs::openable_in_app` 的图片分支与 `openable_in_app_accepts_images_documents_and_source`；`diff_tab` 模块注释声明两栏只读 `InputState`；`windowing.rs` 与 `theme.rs` 对 `wallpaper` 的生产调用；`ThemeDefinition` 的 `typography`、`layout`、`effects` 字段。
- 优化方向：删除不可达旧实现与其测试，或将其登记为明确延期并同步排除项；壁纸与主题字段必须按设计收敛，最终排除检查需覆盖真实模块而不是少量入口文件。

### 一般｜进度表用同一提交充当多个阶段锚，且提交内容与阶段不符

- 置信度：确认
- 位置：`docs/devPlan/progress.md` 的 `工作区与生命周期` 与 `Document 与文本视图` 行、`Agent 与 Hooks` 与 `设置、主题与恢复` 行、`Windows 与发布` 与 `全量验收` 行
- 现象：工作区与 Document 共用 `59fe311f...`，该提交只是 `test: 兼容源码扫描的 CRLF 行尾` 的一行测试调整；Agent 与设置共用 `6e1fea36`，提交为 `fix: 套餐退役扫描与测试构建告警`；Windows 与全量验收共用 `9c76616b`，提交为许可清单补齐。阶段验收文档提交在这些 hash 之后，阶段专属证据无法从 hash 内容区分。
- 根因：hash 取“验收前最后一次提交”而不是阶段实现或验收提交；多个阶段在同一工作树上批量验收。
- 影响：无法按 hash 复核每个阶段的实际变更与测试证据；阶段证据链的粒度不足。
- 证据：提交消息与修改面；验收文档提交分别晚于这些 hash；多个阶段行使用同一 hash。
- 优化方向：为每个阶段指定与其行为切片对应的提交，或指定独立验收文档提交；共用锚时应说明共同基线并保留阶段专属证据。

### 一般｜追溯矩阵存在同名测试例与重复行，例名无法唯一定位

- 置信度：确认
- 位置：`docs/devPlan/traceability.md`；`slterm_app/src/platform/close_dialog.rs` 与 `slterm_workspace/src/close_tests.rs` 的 `close_abandon_warns_about_old_state`；`slterm_app/src/app_shell/document_external.rs` 与 `slterm_document/src/edit.rs` 的 `editor_large_file_dirty_preserves_buffer`；`slterm_app/src/app_shell/app_root/tests/documents.rs` 与 `slterm_document/src/registry.rs` 的 `integration_document_global_identity`；`slterm_app/src/app_shell/app_root/tests/documents.rs` 与 `slterm_app/src/persistence/document_snapshot/tests.rs` 的 `outside_document_restart_requires_authorization` 与 `startup_reconciles_layout_before_snapshot`
- 现象：同一例名在多个 crate 或层中各有实现；`document_batch_close_partial_save` 同时出现在工作区壳与 Document 验收行。矩阵只写名字，无法区分层级与实际覆盖。
- 根因：例名复用以表达同类场景，但矩阵没有作用域限定；行复用让一个例名承担多个所有者场景。
- 影响：按例名执行的过滤会同时命中多个测试，复核时无法判断哪一份是声明证据；覆盖统计与责任归属模糊。
- 证据：上述文件中的同名函数；矩阵章节中的重复行。
- 优化方向：为例名增加层级别名或在矩阵中写明 crate/模块范围；重复行应拆分为各自证据或明确共享理由。

### 一般｜豁免表存在失效设计路径与失效例名，追溯矩阵没有人工豁免行

- 置信度：确认
- 位置：`.claude/docs/testing/exemptions.md` 的“目标态设计声明的人工验收面”与既定清单；`docs/devPlan/traceability.md` 的状态定义
- 现象：豁免表引用 `design/ux-framework.md`、`design/panels/terminal.md`、`design/panels/markdown.md`、`design/panels/editor.md`、`design/panels/diff.md`、`design/platform-windows.md` 等不存在于当前 `.claude/docs` 或 `docs/devPlan/design` 的路径；`spawn_lock_serializes_and_capacity_32_enforced`、`code_and_math_edit_bodies_without_touching_authored_fences`、`mirror_da1_query` 等引用例名在仓内无定义；多条目标态人工面仍写“目标态待实现登记”，但进度已称全部验收完成；追溯矩阵没有任何一行使用“人工豁免”状态。
- 根因：豁免复审没有同步路径与例名，人工面没有回写矩阵；验收锚只检查豁免表存在与行数。
- 影响：豁免兜底无法按引用复核；人工验收面游离于追溯矩阵之外；最终“每个场景有测试或豁免”的结论缺少可定位证据。
- 证据：文件系统路径检查；全仓符号检索；追溯矩阵状态词检索；`acceptance_exemptions_registered` 只匹配表格文本。
- 优化方向：删除或更新失效路径与例名；为仍依赖人工的 featureSpec 场景建立人工豁免行并写明最后核验日期；把“待实现登记”改成可核验状态。

### 一般｜文档策略：进度写入测试计数，追溯状态锚过期，门禁未覆盖

- 置信度：确认
- 位置：`docs/devPlan/progress.md` 的 `Agent 与 Hooks` 行与 `全量验收` 行；`docs/devPlan/traceability.md` 的状态锚说明；`scripts/check_doc_policy.py`
- 现象：进度行写入具体 passed 数量、脚本测试数量和固定数量的跨域锚表述，属于代码可自证的快照计数；追溯矩阵仍声明状态锚为 2026-11-16 的最近核销提交，但该文件在之后多次更新；文档策略脚本只覆盖过程编号、行号、失效文档名与少量旧路径，不检查快照计数，也不识别反引号中的失效设计路径。
- 根因：进度记录沿用了运行输出数字；追溯状态锚没有随最终核销更新；门禁实现落后于根文档纪律。
- 影响：文档违反根 CLAUDE 的快照计数禁令；追溯状态时间线与实际证据不一致；门禁全绿不能证明文档策略成立。
- 证据：进度行的通过数量与固定锚数量；追溯状态锚文本；`check_doc_policy.py` 的规则集合与 `dead-doc` 正则；该脚本对 backticked 路径无检查。
- 优化方向：删除可被代码或运行复算的计数，只保留日期、hash 与契约数值；更新或移除过期状态锚；把快照计数与失效路径纳入文档门禁。

### 一般｜多个计划行为切片没有可核销例名，也没有追溯行或豁免

- 置信度：确认
- 位置：`docs/devPlan/plan.md` 的语言注册与增量语法、TerminalRegistry 与单 worker 所有权、resize 水位与重连代次、ShellLineTracker 与执行确认、纯补全方言与编辑计算、共享 FileTreeModel、GitFacade 状态与仓库绑定、活跃会话事件关联、Hooks 子树原子保存、hook 小进程事件桥、describe 与不可变快照、有界读取与分页游标、Terminal 与 Agent 方法族、WindowsIntegration 统一接入、发布包与实机核验等行为行
- 现象：这些行的“交付与验收锚”只有描述性要求，没有可执行例名；追溯矩阵没有对应行，部分有单元测试但未登记，部分只有服务层假端口测试，部分完全依赖人工发布清单。
- 根因：计划把验收锚写成交付描述，追溯矩阵只接 featureSpec 场景，二者没有闭合；阶段验收按“代码已存在”补齐。
- 影响：这些切片无法区分已交付、部分交付与静默跳过；计划承诺“每个行为行都有回归锚与最终核销锚”不成立。
- 证据：上述计划行的交付列没有反引号例名；追溯矩阵各相关章节没有对应行；相关实现有的存在测试但未被矩阵引用。
- 优化方向：为每个行为行登记真实例名或明确的人工/延期记录，并同步追溯矩阵；无法给出证据的行不得计入阶段完成。

## 跨域观察

- `scripts/check_doc_policy.py` 的规则集合没有实现根 CLAUDE 的“快照计数禁令”，也没有覆盖豁免表中的反引号失效路径；因此文档门禁全绿不能证明上述文档问题不存在。这属于工具链观察，不属于 devPlan 文档本身。
- `release-notes/CLAUDE.md` 要求发布说明只写可核验结果，但 `release-notes/v0.3.0.md` 与 `CHANGELOG.md` 仍包含旧产品能力；该冲突同时落在发布文档与 featureSpec 排除项两侧。

## 存疑未证

- 进度表中的测试通过数与 GPUI 全特性、core release、脚本门禁结果无法在本审查中复现；合同禁止运行 cargo 与测试，无法确认这些数字是否来自当前 HEAD。
- 安装器、更新移交、真实安装与发布包实机项没有执行记录；`packaging.md` 自述待发布机执行，进度却写已验收，需要发布机证据才能闭合。
- Mermaid 的“用户裁决”没有日期、范围或决策记录；无法确认该延期是否仍有效，也无法确认 featureSpec 是否应同步降级。
- 导航树与设置中心的缺失可能是未记录的产品缩减；当前文档没有任何延期或范围调整说明，无法从文档确认这是有意为之。
- 旧图片路由与旧 Diff 模块的静态检索没有生产调用方，但未做动态可达性证明；若存在未见的调用路径，最终排除项违反会更严重。
- `runtime_token_absent_from_materials` 只直接覆盖诊断报告；端点、设置与备份的 token 排除可能由其他用例部分覆盖，但没有在本矩阵行中形成闭合证据。
