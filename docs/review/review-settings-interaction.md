# 设置与交互框架评审

## 覆盖矩阵

### docs/featureSpec/panels/settings.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 面板归属与唯一性 | multi-instance-restore-warning |
| 入口与激活态 | 无问题 |
| 项目作用域 | 无问题 |
| 左导航与配置页目录 | 无问题 |
| 选中页与面板参数 | settings-panel-model-gaps |
| 保存模式与未完成状态 | save-mode-taxonomy; app-page-save-mode-coverage |
| 快捷键页 | dispatch-layer-order-conflict; dispatch-conflict-resolution-conflict; key-name-format-drift; focus-cycle-rebind-reservation; system-key-layer-undefined; shortcut-contract-duplication |
| 后台定时任务页 | background-task-restore-default |
| 外观页 | sidebar-layout-persistence-conflict; sidebar-restore-default-wording; theme-token-validation-ambiguity |
| 终端页 | osc52-nonfocused-scope |
| 应用页 | app-update-switch-conflict; app-language-failure-conflict; app-language-label-drift; app-page-save-mode-coverage; diagnostics-actions-scope; diagnostics-path-placeholder |
| Agent 基础配置页 | agent-config-scope-conflict; agent-node-reserved-dir-validation |
| Hooks 页 | hooks-contract-duplication |
| 导入导出与恢复默认 | sidebar-layout-persistence-conflict; import-version-validation |
| 状态保留 | app-page-save-mode-coverage |
| 交互规则 | 无问题 |
| 打开与聚焦 | 无问题 |
| 导航与搜索 | 无问题 |
| 配置页操作与保存反馈 | save-mode-taxonomy; app-page-save-mode-coverage |
| 关闭、切换与确认 | close-flush-scope-conflict |
| 键盘与焦点 | dispatch-layer-order-conflict; dispatch-conflict-resolution-conflict; key-name-format-drift |
| 拖放与右键 | 无问题 |
| 状态迁移 | 无问题 |
| 边界与错误 | 无问题 |
| 空态 | 无问题 |
| 加载与异步 | 无问题 |
| 错误与恢复 | 无问题 |
| 并发与冲突 | 无问题 |
| 数据完整性与备份 | 无问题 |
| 性能 | 无问题 |
| 键盘与焦点（边界与错误） | key-name-format-drift |
| 平台边界 | 无问题 |
| 安全与隐私 | 无问题 |
| 验收判据 | 无问题 |
| 面板唯一性与入口 | multi-instance-restore-warning |
| 导航与配置页 | 无问题 |
| 快捷键页（验收） | dispatch-layer-order-conflict; dispatch-conflict-resolution-conflict; key-name-format-drift; focus-cycle-rebind-reservation |
| 后台定时任务页（验收） | background-task-restore-default |
| 外观页（验收） | sidebar-layout-persistence-conflict; sidebar-restore-default-wording; theme-token-validation-ambiguity |
| 终端页（验收） | osc52-nonfocused-scope |
| Agent 基础配置页（验收） | agent-config-scope-conflict; agent-node-reserved-dir-validation |
| Hooks 页（验收） | hooks-contract-duplication |
| 应用页（验收） | app-update-switch-conflict; app-language-failure-conflict; diagnostics-actions-scope |
| 导入导出与恢复默认（验收） | sidebar-layout-persistence-conflict; import-version-validation |
| 保存、失败与关闭（验收） | close-flush-scope-conflict; save-mode-taxonomy |
| 关键决策与依据 | save-mode-taxonomy; agent-config-scope-conflict; sidebar-layout-persistence-conflict; theme-token-validation-ambiguity |

### docs/featureSpec/settings-and-persistence.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 设置归属与作用域 | settings-and-persistence-app-scope; save-mode-taxonomy |
| 保存模型 | save-mode-taxonomy |
| 持久化对象与去抖 | sidebar-layout-persistence-conflict; save-mode-taxonomy |
| 原子写、备份与损坏恢复 | 无问题 |
| 导入、导出与恢复默认 | sidebar-layout-persistence-conflict; import-version-validation |
| 关闭冲刷与崩溃恢复 | close-flush-scope-conflict |
| 数据形态边界 | window-geometry-scope; state-db-resource-caps |
| 交互规则 | 无问题 |
| 保存状态与失败重试 | save-mode-taxonomy |
| 导入导出交互 | import-version-validation |
| 关闭与冲刷 | close-flush-scope-conflict |
| 边界与错误 | close-flush-scope-conflict; window-geometry-scope |
| 验收判据 | close-flush-scope-conflict; sidebar-layout-persistence-conflict |
| 关键决策与依据 | save-mode-taxonomy; close-flush-scope-conflict; sidebar-layout-persistence-conflict |

### docs/featureSpec/cross-cutting.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 错误与失败 | 无问题 |
| 键盘与输入 | dispatch-conflict-resolution-conflict; ime-gating-drift |
| 性能与资源 | state-db-resource-caps |
| 安全与隐私 | app-managed-write-boundary |
| 平台与第三方边界 | 无问题 |
| 跨组件不变量 | 无问题 |
| 交互规则 | 无问题 |
| 反馈层级选择 | 无问题 |
| 键盘与焦点 | dispatch-conflict-resolution-conflict |
| 危险操作 | 无问题 |
| 边界与错误 | 无问题 |
| 验收判据 | cross-cutting-font-scaling-leftover |
| 关键决策与依据 | 无问题 |

### docs/devPlan/design/panels/settings.md
| 章节 | 结论 |
|---|---|
| 面板归属与入口 | multi-instance-restore-warning |
| 页面注册表与目录 | settings-registry-layer-boundary; settings-panel-model-gaps |
| 导航、搜索与状态标识 | 无问题 |
| 作用域与保存模式 | save-mode-taxonomy; sidebar-layout-persistence-conflict; settings-panel-model-gaps |
| 页面契约 | dispatch-layer-order-conflict; shortcut-contract-duplication; key-name-format-drift; focus-cycle-rebind-reservation; background-task-restore-default; sidebar-layout-persistence-conflict; sidebar-restore-default-wording; theme-material-home; osc52-nonfocused-scope; agent-config-scope-conflict; agent-filter-semantics-duplication; agent-node-reserved-dir-validation; hooks-contract-duplication; app-update-switch-conflict; app-language-failure-conflict |
| 导入导出与恢复默认 | import-version-validation; sidebar-layout-persistence-conflict |
| 状态保留 | appearance-font-draft-retention; app-page-save-mode-coverage |
| 模块树与复用/重建 | settings-registry-layer-boundary |
| 错误映射 | 无问题 |
| 测试接缝 | 无问题 |

### docs/devPlan/design/settings/app-page.md
| 章节 | 结论 |
|---|---|
| 注册与页面结构 | 无问题 |
| 节级交付矩阵 | 无问题 |
| 数据模型 | app-update-switch-conflict; app-language-label-drift |
| 界面语言 | app-language-failure-conflict; app-language-label-drift |
| 更新 | app-update-switch-conflict |
| 托盘与开机启动 | 无问题 |
| 通知 | 无问题 |
| Runtime API | 无问题 |
| AI Providers 与助手 | 无问题 |
| 诊断 | diagnostics-actions-scope |
| 备份与恢复 | 无问题 |
| 终端增强开关 | 无问题 |
| 保存模式、状态与错误 | app-page-save-mode-coverage; save-mode-taxonomy |
| 凭据边界 | 无问题 |
| 无项目浮层复用 | 无问题 |
| 模块树与测试接缝 | 无问题 |

### docs/devPlan/design/persistence.md
| 章节 | 结论 |
|---|---|
| 数据根与文件 | window-geometry-scope |
| 状态库 schema | state-db-resource-caps |
| 写入模型与事务 | 无问题 |
| 跨 facet 一致性 | 无问题 |
| 启动恢复 | state-db-backup-recovery-entry |
| 崩溃快照与隔离 | 无问题 |
| 设置 JSON | 无问题 |
| 损坏、备份与只读 | state-db-backup-recovery-entry; state-db-resource-caps |
| 线程与生命周期 | design-contract-duplication |
| 可注入时钟与故障注入 | 无问题 |
| 无迁移策略 | 无问题 |
| 持久化测试接缝 | 无问题 |

### docs/devPlan/design/security.md
| 章节 | 结论 |
|---|---|
| 决策与红线 | app-managed-write-boundary |
| 路径允许根与真实路径校验 | app-managed-write-boundary; reparse-point-write-rule |
| URL、终端 OSC 与剪贴板 | 无问题 |
| Markdown 清洗与资源加载 | 无问题 |
| 进程启动与终端边界 | 无问题 |
| 凭据域 | 无问题 |
| 威胁模型 | 无问题 |
| 测试矩阵 | 无问题 |

### docs/devPlan/design/theme-and-i18n.md
| 章节 | 结论 |
|---|---|
| 决策与红线 | app-language-label-drift |
| 语义 token 目录 | design-contract-duplication |
| 内置暗色主题与 Linear 默认 | 无问题 |
| 主题可读性边界 | 无问题 |
| 密度、透明度、壁纸与窗口材质 | theme-material-home |
| 界面语言与 typed i18n 合同 | app-language-label-drift |
| CJK 与字体 | 无问题 |
| 验收与测试 | 无问题 |

### docs/devPlan/design/diagnostics.md
| 章节 | 结论 |
|---|---|
| 决策与红线 | 无问题 |
| 日志 | 无问题 |
| 审计 target | 无问题 |
| panic 与错误边界 | 无问题 |
| 诊断包 | diagnostics-actions-scope |
| 应用页入口与动作 | diagnostics-actions-scope; diagnostics-path-placeholder |
| 验收与测试 | 无问题 |

### docs/devPlan/design/cross-cutting.md
| 章节 | 结论 |
|---|---|
| 决策与红线 | 无问题 |
| 契约索引 | 无问题 |
| 错误分类与反馈层级 | 无问题 |
| 通知类别与时长 | notification-contract-duplication |
| 焦点与键盘索引 | dispatch-layer-order-conflict |
| 键盘、IME 与 CJK 索引 | ime-gating-drift |
| 性能预算 | 无问题 |
| 后台任务生命周期 | 无问题 |
| 安全边界索引 | app-managed-write-boundary |
| i18n 与主题 token 索引 | 无问题 |
| 跨组件不变量 | 无问题 |
| 验收与测试 | 无问题 |

### docs/devPlan/design/ux-framework.md
| 章节 | 结论 |
|---|---|
| 决策与红线 | dispatch-layer-order-conflict; dispatch-conflict-resolution-conflict |
| 命令与动作注册表 | dispatch-layer-order-conflict; system-key-layer-undefined |
| 快捷键模型与重绑 | key-name-format-drift; focus-cycle-rebind-reservation |
| 命令派发层序 | dispatch-layer-order-conflict; system-key-layer-undefined; focus-cycle-rebind-reservation |
| 焦点协调器 | 无问题 |
| 覆盖层单一栈 | 无问题 |
| IME 门控与 CJK | ime-gating-drift |
| 键盘与焦点原语 | 无问题 |
| 验收与测试 | dispatch-layer-order-conflict; focus-cycle-rebind-reservation |

## 问题条目

### dispatch-layer-order-conflict [P0] 命令派发层序在规格、裁决记录与设计中存在三套互相矛盾的顺序

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/ux-framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §快捷键页、§交互规则/键盘与焦点；`docs/featureSpec/README.md` §目标能力边界/命令优先级；`docs/featureSpec/cross-cutting.md` §交互规则/键盘与焦点；`docs/devPlan/design/ux-framework.md` §决策与红线、§命令派发层序；`docs/devPlan/spec-issues.md` §三/命令与焦点
- 原文：“派发优先级从高到低：临时面……→ 系统键 → 全局导航命令（含侧栏视图快捷键）→ 面板固定键 → 可重绑面板命令 → 未处理按键。”／“面板固定键/保留键 > 面板命令 > 侧栏视图快捷键 > 全局命令 > 终端输入。”
- 现象：同一份设置规格前文把全局导航命令和侧栏视图快捷键置于面板固定键与面板命令之上，后文又写“全局命令低于面板命令”；README 同样写“全局命令低于面板命令”；`spec-issues.md` 的设计层采纳写“临时面 > 系统键 > 全局导航命令 > 固定键 > 可重绑面板命令”；`design/ux-framework.md` 的派发表则把面板固定键、面板命令、侧栏视图快捷键依次排在全局命令之前。至少存在三套顺序，且规格同篇自相矛盾。
- 影响：同一和弦在终端、编辑器、侧栏与面板间由谁消费、快捷键页冲突警告是否兑现、验收“生效按优先级派发”均无法判定；实现必然在多个域之间漂移，属于键盘契约不可冻结的阻断问题。
- 证据：上述四份文档的派发顺序原文；`docs/devPlan/spec-issues.md` §三“命令与焦点”采纳结论；`docs/devPlan/README.md` §契约所有者索引把命令与快捷键归 `design/ux-framework.md`。
- 建议修法：由用户裁决唯一层序。若采纳 `ux-framework` 的确定性层序，须同步修改 `spec-issues.md` 采纳结论、featureSpec README、`cross-cutting.md` 与本篇快捷键页；若保留 `spec-issues` 的“全局导航命令优先”，须重写 `ux-framework` 的层序表并补说明面板固定键如何被全局命令截获。补充同和弦跨层代表用例的表驱动测试。
- 裁决对象：规格修订 + 设计修订 + 裁决记录

### dispatch-conflict-resolution-conflict [P1] 冲突裁定在规格采用“最近焦点”启发式，设计明确拒绝该启发式

- owner：`docs/featureSpec/cross-cutting.md`、`docs/devPlan/design/ux-framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/cross-cutting.md` §交互规则/键盘与焦点；`docs/featureSpec/README.md` §快捷键；`docs/featureSpec/panels/settings.md` §快捷键页；`docs/devPlan/design/ux-framework.md` §决策与红线
- 原文：“全局命令与面板命令冲突时，按当前上下文与最近焦点裁定；冲突在设置中心快捷键页可见。”／“上下文决定候选，层序决定胜负……不做‘最近按过谁’的启发式。”
- 现象：规格与 README 允许按“最近焦点”历史裁定，设计明确采用固定层序并拒绝历史启发式；`panels/settings.md` 的另一处又写“按命令优先级与焦点上下文”，没有“最近焦点”。三处口径不一致。
- 影响：同一焦点状态下的同和弦行为可能因用户先按过什么而不同，快捷键页的冲突警告与验收无法复现；终端输入是否让位也会随历史变化，违背“键盘行为确定性”目标。
- 证据：上述规格与设计原文；`docs/devPlan/README.md` 契约所有者索引把命令、快捷键、焦点归 `design/ux-framework.md`。
- 建议修法：删除“最近焦点”启发式，统一为“上下文成立 + 固定层序 + 同层 `order`”的确定性规则；若确实需要历史，必须定义可测试的状态机与失效条件。同步 README、cross-cutting 与快捷键页。
- 裁决对象：规格修订 + 设计修订

### close-flush-scope-conflict [P1] 规格要求项目移除与批量关闭前冲刷设置和布局，工作区/架构 owner 只把守卫冲刷放在窗口关闭链

- owner：`docs/featureSpec/settings-and-persistence.md`、`docs/devPlan/design/persistence.md`
- 类型：冲突
- 定位：`docs/featureSpec/settings-and-persistence.md` §关闭冲刷与崩溃恢复、§交互规则/关闭与冲刷；`docs/featureSpec/workspace/shell.md` §批量关闭、§项目移除与窗口关闭、§关闭链；`docs/devPlan/design/workspace/shell.md` §关闭链；`docs/devPlan/design/architecture.md` §关闭链
- 原文：“正常关闭窗口、移除项目、批量关闭前，先冲刷设置与布局；冲刷失败必须可见，并按工作区关闭守卫处理。”／“项目移除不复用窗口关闭链：确认后先成功提交状态库事务……成功后再删除恢复快照目录。”
- 现象：工作区规格的批量关闭只保存本次关闭范围内的需保存面板；项目移除走自己的确认与状态库事务；架构设计把 `Guarded` 冲刷限定在窗口关闭链。只有本规格把“移除项目、批量关闭”也纳入全局设置与布局冲刷。
- 影响：项目移除或批量关闭可能被无关的设置/布局写失败阻塞，与 `spec-issues.md`“项目移除先提交状态库事务”的裁决冲突；失败时是中止还是放弃、是否重试的语义与工作区 owner 不一致；关闭链验收无法统一。
- 证据：上述三份 owner 文档的关闭算法；`docs/devPlan/spec-issues.md` §五“小口径收敛”中“项目移除先提交状态库事务”；`docs/featureSpec/workspace/shell.md` §窗口关闭“受守卫文件包括应用设置、侧栏布局、项目索引、项目布局与终端簿”。
- 建议修法：把本规格的冲刷范围限定为“窗口关闭链”；批量关闭只按工作区关闭守卫冲刷被关闭面板的待写自动保存；项目移除只提交项目相关状态库事务。删除“移除项目、批量关闭前先冲刷设置与布局”的表述，或明确其为面板级而非全局。
- 裁决对象：规格修订

### sidebar-layout-persistence-conflict [P1] 侧栏默认布局被同时归入 2 秒应用设置与 500ms 状态库 facet

- owner：`docs/featureSpec/settings-and-persistence.md`、`docs/devPlan/design/panels/settings.md`
- 类型：冲突
- 定位：`docs/featureSpec/settings-and-persistence.md` §持久化对象与去抖；`docs/featureSpec/panels/settings.md` §外观页、§导入导出与恢复默认；`docs/devPlan/design/panels/settings.md` §作用域与保存模式、§页面契约/外观页；`docs/devPlan/design/sidebar/framework.md` §显隐与槽位；`docs/devPlan/design/persistence.md` §去抖；`docs/devPlan/design/architecture.md` §状态所有权矩阵
- 原文：“应用设置：外观、快捷键、终端、Agent 基础配置、应用、后台任务配置 | 2 秒”／“侧栏布局 + 视图状态 | 500ms”；“侧栏布局与视图状态不属于应用设置域；导入、导出与恢复所有默认不触碰侧栏布局与视图状态。”
- 现象：持久化表把“侧栏默认布局”列在应用设置 2 秒行；同一文档又把侧栏布局与视图状态列为 500ms 的侧栏框架对象；外观页规格明确侧栏布局不属于应用设置域且导入/导出/恢复默认不触碰；设计侧栏框架要求外观页与活动栏共用同一 `SidebarLayout` 并按侧栏域 500ms 去抖；架构状态所有权把侧栏布局落在状态库 `sidebar`，不是 `settings.json`。设计设置篇的“自动保存”表又把整个外观页归入 2 秒。
- 影响：侧栏布局可能出现两条写入通道、两个去抖值；导入/导出/恢复默认是否包含侧栏布局无法判定；实现可能把侧栏状态写进 `settings.json`，破坏“应用设置是唯一漫游状态”的双根边界。
- 证据：上述四份文档原文；`docs/devPlan/design/architecture.md` §状态所有权矩阵“侧栏布局 → 本地状态库 `sidebar.layout_json`”。
- 建议修法：把“侧栏默认布局”从应用设置 2 秒行删除，明确它是侧栏 facet（状态库、500ms），外观页只是同一 `SidebarLayout` 的控制面；设计设置篇的自动保存表注明外观页的侧栏控件走侧栏域，其余外观项走应用设置域；保持导入/导出/恢复默认不触碰侧栏布局。
- 裁决对象：规格修订 + 设计修订

### agent-config-scope-conflict [P1] Agent 基础页四类数据的“共享 / 按 CLI / 按 Agent”作用域互相矛盾

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/panels/settings.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §Agent 基础配置页、§关键决策/Agent 配置应用级共享；`docs/devPlan/design/panels/settings.md` §页面契约/Agent 基础配置页；`docs/devPlan/design/sidebar/agent-files.md` §过滤消费、§模型与模块
- 原文：“恢复本页默认只复位当前 CLI 的侧栏展示内容、CLI 别名与运行时文件清单；不清除其他 CLI 的配置。”／“侧栏展示、别名、自定义节点与运行时清单在所有 CLI 基础页共享同一份数据。”
- 现象：同一份规格的“恢复本页默认”和“`全部` 模式下若该 Agent 的运行时文件清单非空”按当前 CLI/Agent 限定；关键决策与设计设置篇却写四类数据在所有 CLI 基础页“共享同一份配置”；侧栏框架设计又写“按 Agent 分别配置”，且其 `AgentFilterConfig.runtime_names` 是扁平 `Vec<String>`，与规格“按 Agent 分组、每个 Agent/自定义节点各占一行”不一致。
- 影响：存储键是全局还是按 Agent、跨 CLI 页面编辑是否即时同步、页级恢复默认会复位哪些数据、过滤消费如何取每个 Agent 的运行时清单，均无法确定；实现可能复位其他 CLI 的配置或无法按 Agent 隐藏运行时文件。
- 证据：上述三份文档原文；`docs/devPlan/spec-issues.md` §三“设置中心”只写“Hooks 与 Agent 页联动”，未裁决四类数据的粒度。
- 建议修法：补一张逐字段作用域矩阵：侧栏展示模式/名单、`显示运行时文件`、CLI 别名、运行时文件清单、自定义节点分别标注应用级单例、按 Agent、按 CLI 或全局分组；据此修正关键决策、设计设置篇与 `agent-files` 的 `AgentFilterConfig` 模型；补跨 CLI 页面同步与页级恢复默认测试。
- 裁决对象：规格修订 + 设计修订

### app-update-switch-conflict [P1] 应用页更新开关在规格与设计间是“一个仅手动”还是“自动检查 + 后台下载 + 仅手动”三个

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/settings/app-page.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §应用页/更新、§应用页迁移表；`docs/devPlan/design/settings/app-page.md` §数据模型、§更新、§保存模式、状态与错误
- 原文：“更新：显示当前版本与检查状态；跟随系统代理；提供 `仅手动检查` 开关；不提供应用内代理凭据配置。”／“`自动检查更新` 开关，默认开；关闭后仍可手动‘检查更新’。`后台下载更新包` 开关，默认关。”
- 现象：规格迁移表写“检查/下载开关”（两个），正文只写“仅手动检查”（一个）；设计数据模型持久化 `auto_check` 与 `auto_download` 两个键，正文又额外写“提供‘仅手动检查’开关”，而它与 `auto_check` 语义互逆，未说明是否同一控件的反向展示。
- 影响：应用页可能出现三个互相覆盖的开关，或一个键两种语义；默认值、保存键与验收无法确定；用户同时看到“自动检查更新”和“仅手动检查”会产生直接冲突。
- 证据：上述两份文档原文；`docs/devPlan/spec-issues.md` §五“应用页范围”只写“更新为只读占位”，未裁决开关形态。
- 建议修法：二选一并统一。推荐保留 `auto_check`/`auto_download` 两个持久键，删除“仅手动检查”独立开关或明确它是 `auto_check` 的反向文案；规格迁移表与正文同步为两个开关及默认值，并补验收。
- 裁决对象：规格修订 + 设计修订

### app-language-failure-conflict [P1] 语言保存失败时规格要求回退原语言，设计要求保留本轮新语言

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/settings/app-page.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §应用页/界面语言；`docs/devPlan/design/settings/app-page.md` §界面语言；`docs/devPlan/design/theme-and-i18n.md` §决策与红线
- 原文：“语言切换即时作用于界面文案，失败保留原语言并提示。”／“保存失败时提示‘设置保存失败，修改仅在本次运行中生效’，保留本轮语言。”
- 现象：规格要求保存失败回退到原语言；设计要求保留本轮已切换的新语言并提示仅在本次运行中生效。主题设计只写切换立即更新全局 `UiLanguage`，没有失败回退路径。
- 影响：保存失败后用户看到的界面语言相反；语言切换的验收、失败提示文案与即时型设置“失败仍在本轮生效”的统一语义冲突；测试无法同时通过两份文档。
- 证据：上述三份文档原文；`docs/featureSpec/settings-and-persistence.md` §保存模型“即时型修改在本次运行中始终生效”。
- 建议修法：裁决失败语义。若遵循即时型统一规则，保留本轮语言并修改规格；若语言必须回退，设计需在保存失败时恢复旧 `UiLanguage` 并补失败路径测试。同步应用页验收。
- 裁决对象：规格修订

### hooks-contract-duplication [P1] 设置设计篇越权复制 Hooks 引擎契约，且重挂载层级重置与规格冲突

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：冲突 / 缺口
- 定位：`docs/devPlan/design/panels/settings.md` §页面契约/Hooks 页；`docs/devPlan/design/integrations/hooks.md` §职责与边界、§三层文档模型、§保存事务、§注入与卸载、§外部修改检测；`docs/devPlan/README.md` §文档地图与契约所有者索引；`docs/featureSpec/panels/settings.md` §左导航与配置页目录、§Hooks 页
- 原文：“页面重挂载模式重置 JSON，层级重置 User（关闭重开）。”／“本域负责：Hook 事件与 handler 能力目录、schema 校验、User/Project/Local 三层配置文档、GUI/JSON 双模式编辑模型、保存事务、注入与卸载、托管条目归属、外部修改检测、文件锁与上一代备份。”
- 现象：README 契约索引明确 Hooks 目录、schema、三层文档、注入/卸载归 `design/integrations/hooks.md`，但设置设计篇又把三层、保存事务、注入/卸载、外部修改检测整体复述一遍，形成两个 owner。复述还出现漂移：设置设计篇写“页面重挂载模式重置 JSON，层级重置 User（关闭重开）”，而规格写“在设置中心内部切换配置页会重新挂载目标页内容；离开的配置页的页内临时状态（……Hooks 层级/模式/未保存编辑）……返回时不恢复”，即切换配置页本身就不保留层级；`hooks.md` 只规定重挂载模式重置 JSON，未规定层级重置时机。
- 影响：Hooks 层级在“切换配置页”与“关闭重开”两种场景下的重置规则无法判定；实现可能切换配置页保留层级、关闭重开才重置，或反之；Hooks 引擎与设置页的保存/注入/外部修改契约可能各自演化，违反单一 owner 门禁。
- 证据：`docs/devPlan/README.md` 契约所有者索引；`design/integrations/hooks.md` 职责与边界；`featureSpec/panels/settings.md` §左导航与配置页目录；`design/panels/settings.md` §页面契约/Hooks 页。
- 建议修法：删除设置设计篇中的 Hooks 引擎细节，改为引用 `integrations/hooks.md`；设置页只保留页面壳、层级按钮布局、保存模式与焦点/重挂载的页面级状态契约。明确“切换配置页、关闭重开、应用重启”三者的层级/模式/未保存编辑重置矩阵，并与规格状态保留表对齐。补对应测试。
- 裁决对象：设计修订

### app-managed-write-boundary [P1] 安全设计缺少“应用托管写入”路径作用域，无法落地规格的写根分表

- owner：`docs/devPlan/design/security.md`
- 类型：缺口
- 定位：`docs/devPlan/design/security.md` §决策与红线、§路径允许根与真实路径校验；`docs/featureSpec/cross-cutting.md` §安全与隐私；`docs/devPlan/design/architecture.md` §数据根与状态所有权；`docs/devPlan/design/persistence.md` §数据根与文件
- 原文：“应用托管写入限定在应用数据目录、系统临时目录、备份/隔离/日志目录与 Hooks/CLI 配置文件；Hooks/CLI 配置是用户显式编辑的第三方配置，遵循各自权限边界。”／`RootScope { Project, Repo, Agent, UserSelected, SingleFileLink }`。
- 现象：规格要求安全边界区分业务写入与应用托管写入；安全设计的 `AllowedRoots` 只有项目根、仓库根、Agent 根，`RootScope` 也没有应用数据根/临时目录/备份/隔离/日志/Hooks 资产等托管写作用域，但设计同时声称“所有领域经此单一安全边界读写外部资源”。
- 影响：设置、日志、审计、隔离件、Hooks 注入资产、备份、更新状态等托管写入只能绕过 `PathGuard` 或各自实现路径检查，形成第二套写入通道；路径越界、原子写与审计规则无法统一测试；与“单一安全边界”红线冲突。
- 证据：上述规格与设计原文；`design/persistence.md` §数据根与文件列出 `<data>`/`<local>` 下的设置、日志、审计、hooks、backup、update 等目录；`design/diagnostics.md` 与 `design/integrations/hooks.md` 也直接使用这些目录。
- 建议修法：在 `security.md` 增加 `AppManaged` 或等价的托管写作用域，列明允许根（漫游根、本地根、系统临时目录、备份/隔离/日志目录、Hooks/CLI 配置），并规定审计、原子写与禁止用户内容路径混用；或者明确声明托管写入不走 `PathGuard`，但必须给出独立的受审计写入口与测试矩阵。同步 persistence/diagnostics/hooks 的引用。
- 裁决对象：设计修订

### save-mode-taxonomy [P2] 保存模式在“双模式 / 三模式 / 后台任务 2 秒”之间口径不一

- owner：`docs/featureSpec/panels/settings.md`、`docs/featureSpec/settings-and-persistence.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §保存模式与未完成状态、§交互规则/配置页操作与保存反馈；`docs/featureSpec/settings-and-persistence.md` §设置归属与作用域、§保存模型、§持久化对象与去抖；`docs/devPlan/spec-issues.md` §三/设置中心
- 原文：“设置中心采用按域双模式：即时型配置自动保存，Hooks 使用显式保存，后台定时任务使用提交即保存。”／“应用设置：外观、快捷键、终端、Agent 基础配置、应用、后台任务配置 | 2 秒”；“任务页的显式提交……不受自动保存去抖影响。”
- 现象：同一段文字称“双模式”却列出三种模式；持久化对象表把后台任务配置放进 2 秒应用设置行，但保存模型与设置归属又说后台任务提交不走自动保存去抖；`spec-issues.md` 明确写“三保存模式”。
- 影响：后台任务勾选/频率提交到底是立即写盘还是 2 秒去抖、页头保存状态如何显示、失败回滚与验收均无法确定；设置中心保存协调器可能为任务页创建第二套提交路径或错误地去抖。
- 证据：上述三份文档原文；`docs/devPlan/design/panels/settings.md` 的三模式表与 `PageSaveState`。
- 建议修法：统一采用三保存模式；从 2 秒应用设置行删除“后台任务配置”或标注“显式提交立即写，不走去抖”；把“双模式”改为“三模式”并同步交互规则与验收。
- 裁决对象：规格修订

### background-task-restore-default [P2] 后台定时任务页的“恢复本页默认”作用域与提交方式未定义

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/panels/settings.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/settings.md` §左导航与配置页目录、§后台定时任务页、§导入导出与恢复默认；`docs/devPlan/design/panels/settings.md` §页面契约/后台定时任务页、§导入导出与恢复默认
- 原文：“每个配置页页头右侧提供 `恢复本页默认`；Hooks 页不提供该项。”
- 现象：规格把“恢复本页默认”作为所有非 Hooks 页的通用动作，后台任务页属于应用设置域；但后台任务页契约只定义读取、勾选/频率提交、失败回显与空态，没有定义“恢复本页默认”复位哪些任务、用默认值还是上次值、是否二次确认以及走立即提交还是自动保存。该页是“提交即保存”模式，设计页面契约与恢复默认章节都没有说明恢复动作的复位范围（两个任务全部还是当前任务）和提交路径。
- 影响：实现可能把恢复默认错误地放进 2 秒去抖，或只复位当前行/只复位套餐余量；失败回滚与验收缺失，用户可能看到部分任务被重置。
- 证据：`featureSpec/panels/settings.md` §保存模式与未完成状态“后台定时任务：提交即保存”；§后台定时任务页任务表；§导入导出与恢复默认“每个配置页页头提供恢复本页默认”。
- 建议修法：明确“恢复本页默认”复位全部后台任务的启用状态与频率到默认值，二次确认后作为显式提交立即写盘；定义失败回显与回滚；补验收。
- 裁决对象：规格修订 + 设计修订

### app-page-save-mode-coverage [P2] 应用页未进入保存模式枚举与状态保留表，显式清理动作也在设计清单中缺失

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/settings/app-page.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/settings.md` §保存模式与未完成状态、§交互规则/配置页操作与保存反馈、§状态保留；`docs/devPlan/design/settings/app-page.md` §保存模式、状态与错误；`docs/devPlan/design/panels/settings.md` §状态保留
- 原文：“快捷键、外观、终端、Agent 基础配置：`自动保存`。”／“凭据、快捷方式、检查更新、连通性测试、备份/恢复是显式动作，立即提交。”
- 现象：应用页明确是即时型/自动保存，但两处保存模式枚举都只列快捷键、外观、终端、Agent 基础配置，漏掉应用页；规格状态保留表没有应用页配置行（设计状态保留表有）；规格说清理动作是显式型，设计显式动作清单没有“一键清理”。
- 影响：应用页页头可能不显示保存模式、保存状态/失败重试没有契约依据；应用页配置在切换项目/重启时的保留未覆盖；清理动作可能被错误地放进 2 秒去抖。
- 证据：上述文档原文；`docs/featureSpec/panels/settings.md` §应用页“保存模式为即时型……清理动作为显式型”。
- 建议修法：在两处自动保存枚举中加入“应用”；状态保留表增加应用页配置行；设计显式动作清单加入“一键清理”。
- 裁决对象：规格修订 + 设计修订

### settings-panel-model-gaps [P2] 设置面板模型无法表达“无选中页”、Hooks 分层作用域与页内参数

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/settings.md` §面板归属与入口、§页面注册表与目录、§作用域与保存模式；`docs/featureSpec/panels/settings.md` §选中页与面板参数、§Hooks 页
- 原文：`selected_page: SettingsPageId`；`scope: SettingsScope`；`page_params: BTreeMap<SettingsPageId, PageParams>`。
- 现象：规格允许“没有任何配置页时选中页为空”；Hooks 页同时承载应用级 User 与项目级 Project/Local，页头作用域随层级变化；规格要求各配置页页内参数独立保存，但本设计没有定义任何一个具体 `PageParams`；规格还规定外部更新面板参数中的选中页时同步切换、未知标识忽略，设计只写创建时读取与程序化初始页，没有运行期外部更新规则。
- 影响：实现需要哨兵 ID 或额外 `Option` 才能表示空选中页；注册表的静态 `scope` 无法驱动 Hooks 分层作用域标识；`page_params` 成为空契约，布局 DTO、持久化测试与未知参数忽略规则没有可验证对象。
- 证据：`featureSpec/panels/settings.md` §选中页与面板参数、§Hooks 页；`devPlan/spec-issues.md` §五“设置面板 `page_params` 持久化”。
- 建议修法：把 `selected_page` 改为 `Option<SettingsPageId>` 或定义明确的空 ID；把 `scope` 改为页面级函数/按层返回；列出实际页内参数（若暂无则声明保留且为空，并给出布局 schema 的空映射约定）。
- 裁决对象：设计修订

### settings-registry-layer-boundary [P2] 页面注册表把带 GPUI 类型的页面 trait 放在 core crate 名下，依赖方向未拆分

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/settings.md` §页面注册表与目录、§模块树与复用/重建；`docs/devPlan/design/ux-framework.md` §命令与动作注册表；`.claude/CLAUDE.md` §决策与红线/依赖方向
- 原文：`pub trait SettingsPage { fn render(&mut self, window: &mut Window, cx: &mut SettingsPageCx) -> AnyElement; }`／“领域 core crate settings/page_registry.rs # SettingsPageId、分组、顺序、覆盖规则”。
- 现象：模块树把 `page_registry.rs` 列在领域 core crate 下，而紧邻的 `SettingsPageDef.factory`/`SettingsPage` 直接引用 GPUI 的 `Window` 与 `AnyElement`；设计没有说明数据注册表（core）与 UI 工厂/页面 trait（app）如何拆分。项目的硬性约束是 core 不依赖 app、渲染框架包禁入 core/hook 生产依赖，ux-framework 也写明领域 crate 不引用 GPUI。
- 影响：若按模块树把注册表放进 core，依赖方向门禁直接变红；若拆成两套注册表，页面 ID/顺序/覆盖规则与工厂可能漂移；测试接缝也无法确定放在哪一层。
- 证据：项目 `CLAUDE.md` 依赖方向红线；`design/ux-framework.md` “领域 crate 不引用 GPUI”；`design/architecture.md` 新 core crate 边界。
- 建议修法：明确拆分为 core 的数据注册表（ID、分组、顺序、覆盖规则）与 app 的 UI 注册表/工厂；或者在模块树中把整个 `page_registry.rs` 移到 `slterm_app`，core 只保留 `SettingsPageId`。补依赖门禁用例。
- 裁决对象：设计修订

### multi-instance-restore-warning [P2] 布局恢复重复设置中心“静默关闭”与设计“聚合警告”不一致

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/panels/settings.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §面板归属与唯一性、§验收/面板唯一性与入口；`docs/devPlan/design/panels/settings.md` §面板归属与入口
- 原文：“保留第一个并静默关闭其余实例；该过程不产生未保存状态与关闭确认。”／“按布局 DFS 保留第一个，其余静默关闭；清理前隔离原布局并聚合警告。”
- 现象：规格要求静默关闭，设计在静默关闭之外增加“清理前隔离原布局并聚合警告”；警告的触发条件、聚合范围与文案未定义。
- 影响：用户是否看到警告、隔离件是否产生、验收“只保留一个”是否覆盖警告均不确定；隔离原布局还会改写持久化数据，超出规格描述。
- 证据：上述两份文档原文。
- 建议修法：裁决是否允许警告。若允许，规格改为“无关闭确认，但显示一条聚合警告并隔离原布局”，并定义警告文案与隔离路径；若不允许，删除设计的警告与隔离步骤。
- 裁决对象：规格修订 或 设计修订

### agent-node-reserved-dir-validation [P2] Agent 页设计校验清单漏掉盘符根、%WINDIR% 与 System32 拒绝

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/settings.md` §页面契约/Agent 基础配置页；`docs/featureSpec/panels/settings.md` §Agent 基础配置页/自定义 Agent 节点校验；`docs/devPlan/design/security.md` §路径允许根与真实路径校验；`docs/devPlan/design/sidebar/agent-files.md` §模型与模块
- 原文：“目录为盘符根、`%WINDIR%` 或 `System32` | `目录不可用于自定义 Agent 节点`”／“校验：名称空/路径分隔符/重名；目录空/非绝对。”
- 现象：规格定义了系统目录拒绝与固定错误文案，安全设计与 Agent 文件设计也包含该规则，但设置页设计页面的校验清单只列名称与“目录空/非绝对”，没有系统目录拒绝。
- 影响：页面可能先接受系统目录，直到 Agent 文件视图或安全层才失败；行内错误文案缺失；验收“目录不可用于自定义 Agent 节点”没有页面实现落点。
- 证据：上述三份文档原文。
- 建议修法：在 Agent 页校验清单加入“盘符根、`%WINDIR%`、`System32` → `目录不可用于自定义 Agent 节点`”，并引用 `security.md` 作为规则 owner；补行内错误测试。
- 裁决对象：设计修订

### osc52-nonfocused-scope [P2] 设置页称开关作用于“全部终端”，终端与安全 owner 限定活跃项目且排除隐藏项目

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/panels/settings.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/featureSpec/panels/settings.md` §终端页、§验收/终端页；`docs/devPlan/design/panels/settings.md` §页面契约/终端页；`docs/featureSpec/panels/terminal.md` §OSC 52；`docs/devPlan/design/security.md` §URL、终端 OSC 与剪贴板
- 原文：“开启后非聚焦终端也可写入……开关即时作用于全部终端面板。”／“开启后活跃项目内未聚焦终端可写入，隐藏项目中的终端始终不可写入。”
- 现象：设置页把开关描述为对全部终端生效；终端规格与安全设计限定为活跃项目内非聚焦终端，隐藏项目中的终端始终拒绝。
- 影响：设置页文案承诺超出安全边界；验收可能出现“隐藏项目终端写入”相反预期；用户误以为后台项目终端也能写剪贴板。
- 证据：上述四份文档原文。
- 建议修法：把设置页文案改为“允许活跃项目内非聚焦终端写入剪贴板”，并引用 `terminal.md`/`security.md`；补隐藏项目终端的负向验收。
- 裁决对象：规格修订 + 设计修订

### import-version-validation [P2] 设置导入缺少规格要求的版本校验

- owner：`docs/devPlan/design/panels/settings.md`、`docs/devPlan/design/security.md`
- 类型：缺口
- 定位：`docs/featureSpec/settings-and-persistence.md` §导入、导出与恢复默认、§交互规则/导入导出交互、§边界与错误；`docs/devPlan/design/panels/settings.md` §导入导出与恢复默认；`docs/devPlan/design/security.md` §安全边界索引/导入导出
- 原文：“导入前必须校验格式与版本，预览将要修改的域；校验失败不写盘。”／“导入：schema 校验 → 预览受影响域 → 用户确认 → 生成可恢复备份 → 原子写。”
- 现象：规格把“版本”与格式并列作为导入前置校验，并规定版本不兼容拒绝导入；设计只写 schema 校验，没有版本字段/兼容范围/错误映射。
- 影响：版本不兼容文件可能通过 schema 或失败在更晚阶段；规格验收“版本不兼容拒绝导入”没有设计落点与测试接缝。
- 证据：上述三份文档原文；`docs/devPlan/design/persistence.md` §无迁移策略“目标态只接受当前数据形态”。
- 建议修法：在导入流程 schema 校验前增加版本校验，定义支持版本、错误文案与“零写入”；补版本不兼容测试。
- 裁决对象：设计修订

### state-db-backup-recovery-entry [P2] 状态库启动错误界面与“从备份恢复”入口不一致

- owner：`docs/devPlan/design/persistence.md`
- 类型：冲突
- 定位：`docs/devPlan/design/persistence.md` §启动恢复、§损坏、备份与只读
- 原文：“版本不匹配或检查失败：关闭连接，把 `state.db`、`-wal`、`-shm` 移入……提供 `重试` 与 `以空状态继续`。”／“启动时主库不可用时提供‘从备份恢复’入口。”
- 现象：启动恢复只列“重试/以空状态继续”，没有“从备份恢复”；后文又说主库不可用时提供备份恢复入口，并规定损坏库与备份都移入隔离区，用户只能手动取回。
- 影响：状态库损坏时用户可见的恢复路径不确定；若库与备份都被隔离，“从备份恢复”入口没有可用备份；验收与诊断动作无法对应。
- 证据：上述两节原文；`docs/devPlan/design/architecture.md` §启动链也只列重试/以空状态继续。
- 建议修法：统一启动错误界面动作。若保留备份恢复，明确在隔离前先尝试 `state.db.bak`，失败再隔离两者；若只支持空状态，删除“从备份恢复”表述并说明手动恢复路径。补状态库损坏/备份可用/备份损坏三种测试。
- 裁决对象：设计修订

### reparse-point-write-rule [P2] 写路径“拒绝重解析点”与“按真实目标判定、根内允许”自相矛盾

- owner：`docs/devPlan/design/security.md`
- 类型：冲突
- 定位：`docs/devPlan/design/security.md` §决策与红线、§路径允许根与真实路径校验（校验顺序第 3、5 步）
- 原文：“符号链接与目录联接按最终目标判定。”／“写路径再校验……必须重新解析真实路径并拒绝重解析点（reparse point）与符号链接逃逸。”
- 现象：第 3 步按真实目标判定，暗示根内符号链接目标可允许；第 5 步又要求写路径拒绝重解析点，`PathDeny` 也含 `ReparsePointInWritePath`。规格只要求拒绝根外目标，未要求拒绝所有根内链接。
- 影响：项目根内指向根内文件的符号链接可读但不可写，编辑器保存会意外失败；测试无法确定“根内链接写入”应允许还是拒绝。
- 证据：上述原文；`docs/featureSpec/cross-cutting.md` §安全与隐私“符号链接与目录联接按真实目标路径校验，根外目标拒绝读写”。
- 建议修法：明确只拒绝真实目标越界的重解析点/链接，根内目标允许；或明确产品决定禁止所有链接写入并同步规格与错误文案。补根内/根外链接读写用例。
- 裁决对象：规格修订 + 设计修订

### theme-material-home [P2] 窗口材质有默认值、档位与诊断项，但规格与页面无入口、键与保存作用域

- owner：`docs/devPlan/design/theme-and-i18n.md`、`docs/devPlan/design/settings/app-page.md`
- 类型：缺口
- 定位：`docs/devPlan/design/theme-and-i18n.md` §密度、透明度、壁纸与窗口材质；`docs/featureSpec/panels/settings.md` §外观页；`docs/devPlan/design/settings/app-page.md` §数据模型、§节级交付矩阵；`docs/devPlan/spec-issues.md` §panels/settings.md
- 原文：“窗口材质 `BlurMode`：`None`（默认，性能安全）、`Mica`、`MicaAlt`、`Aero`、`Acrylic`。”／“外观页提供暗色主题、字体、侧栏默认布局、编辑器与 Diff 行为、自动保存与 Markdown 草稿恢复配置；不提供浅色主题或跟随系统选项。”
- 现象：材质是用户可见档位并进入诊断“功能状态”，但 featureSpec 没有材质条目，应用页数据模型没有材质键，外观页设计也没有材质控件；`spec-issues.md` 只说“窗口材质归应用或外观（设计定稿）”，设计未定稿到具体页面。
- 影响：用户无法设置材质，或实现会临时新增无规格控件；材质的默认值、持久化、导入/导出/恢复默认作用域与失败回退没有契约；诊断包报告一个无 owner 的设置。
- 证据：上述文档原文；`design/diagnostics.md` §诊断包“功能状态：主题、密度、透明度、壁纸启用、材质……”。
- 建议修法：裁决放在应用页还是外观页，补 featureSpec 条目、设置键/默认值、控件、持久化与恢复默认作用域；或明确材质仅内部默认、不暴露设置，并从诊断项移除。
- 裁决对象：规格修订 + 设计修订

### theme-token-validation-ambiguity [P2] “主题语义槽校验”是否包含对比度未定义，与“不做对比度判断”裁决冲突

- owner：`docs/featureSpec/panels/settings.md`
- 类型：缺口 / 冲突
- 定位：`docs/featureSpec/panels/settings.md` §外观页、§关键决策与依据/外观仅暗色、不做对比度判断
- 原文：“所有主题方案以及透明度、壁纸、密度组合须通过 主题语义槽校验。”／“不做对比度校验、不提示、不因对比度拒绝或自动回退。”
- 现象：“主题语义槽校验”未定义；把“透明度、壁纸、密度组合”与“校验”并列，容易被实现为对比度/可读性校验，而 `spec-issues.md` 已裁决删除 WCAG/对比度准入。
- 影响：实现可能新增对比度检查、提示或自动回退，直接违反已关闭裁决；验收无法区分“token 映射完整性”与“对比度准入”。
- 证据：`docs/devPlan/spec-issues.md` §五“无障碍与对比度”；`design/theme-and-i18n.md` §主题可读性边界。
- 建议修法：把该条改为“每套主题提供完整语义 token 映射；不计算对比度、不因组合拒绝或回退”，并引用 `theme-and-i18n.md`；补负向测试。
- 裁决对象：规格修订

### diagnostics-actions-scope [P2] 设计新增“导出诊断包/运行环境自检”，规格与裁决动作集没有这两项

- owner：`docs/devPlan/design/diagnostics.md`、`docs/devPlan/design/settings/app-page.md`
- 类型：缺口
- 定位：`docs/devPlan/design/diagnostics.md` §诊断包、§应用页入口与动作；`docs/featureSpec/panels/settings.md` §应用页/诊断；`docs/devPlan/spec-issues.md` §五/应用页范围
- 原文：“诊断：版本、数据目录、日志目录、配置健康、复制诊断信息、打开目录/日志、通知自检，以及一键清理日志/审计/更新下载/崩溃现场。”／设计动作表另有“导出诊断包…”与“运行环境自检”。
- 现象：规格与 `spec-issues` 已把诊断动作收敛为固定集合；设计新增两个“可选”动作，但未说明它们是规格内还是范围外迁移，也没有在应用页设计或验收中定义可见性。
- 影响：应用页可能多出无规格依据的控件；诊断包格式、脱敏、自检项与测试范围扩张；迁移前/后状态无法判定。
- 证据：上述三份文档原文。
- 建议修法：二选一。若要保留，补 featureSpec 动作与验收，并明确规格内是否可见；若属于范围外迁移，移入 `out-of-scope` 并仅在迁移后启用，规格内不渲染。
- 裁决对象：规格修订 或 设计修订

### system-key-layer-undefined [P2] “系统键”层没有定义，设计派发表也没有对应层

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/ux-framework.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/settings.md` §快捷键页；`docs/devPlan/design/ux-framework.md` §命令派发层序
- 原文：“派发优先级从高到低：临时面……→ 系统键 → 全局导航命令……”／设计派发表从录制、IME、覆盖层直接到面板固定键/保留键。
- 现象：“系统键”未说明包含哪些键（操作系统保留键、Alt+F4、IME、窗口管理器键等），设计没有该层，也没有与 IME/覆盖层的相对顺序。
- 影响：实现无法确定系统键是否在覆盖层之前/之后、是否暂停录制、是否被终端吞掉；派发顺序测试缺少该层。
- 证据：上述两份文档原文。
- 建议修法：删除“系统键”，或把它定义为明确集合并插入设计派发表（例如 OS 保留键在应用之前处理、IME 在应用命令之前）；补对应测试。
- 裁决对象：规格修订 + 设计修订

### focus-cycle-rebind-reservation [P2] `Ctrl+Tab` 焦点循环可重绑，设计却硬编码保留该和弦给焦点协调器

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/ux-framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §快捷键页命令表、§验收/快捷键页；`docs/devPlan/design/ux-framework.md` §命令派发层序、§快捷键模型与重绑
- 原文：“循环聚焦下一面板 | `Ctrl+Tab`”“循环聚焦上一面板 | `Ctrl+Shift+Tab`”／“`Ctrl+Tab`/`Ctrl+Shift+Tab` 是焦点逃逸键……派发器在面板命令层之前显式保留该和弦给焦点协调器。”
- 现象：命令表把两个焦点循环命令列为可重绑；设计按固定和弦在面板命令层之前保留逃逸。用户改绑后，旧和弦仍被保留，新和弦不一定获得逃逸语义；解绑后也没有定义。
- 影响：重绑焦点循环可能出现“新键不逃逸、旧键仍被吞”的孤儿保留；终端/编辑器是否拦截取决于固定和弦而非用户绑定，快捷键页冲突与验收不一致。
- 证据：上述两份文档原文；`docs/featureSpec/workspace/shell.md` §焦点“终端与编辑器不拦截该组合，作为焦点逃逸路径”。
- 建议修法：让焦点逃逸跟随焦点循环命令的当前生效绑定（含未绑定）；或把这两个命令标为不可重绑并从可重绑目录移除。补重绑/解绑后的逃逸测试。
- 裁决对象：规格修订 + 设计修订

### key-name-format-drift [P2] 快捷键表使用符号键名，显示/存储规则要求物理键名，标点键无映射

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/ux-framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §快捷键页命令表、§快捷键页显示规则；`docs/devPlan/design/ux-framework.md` §快捷键模型与重绑
- 原文：“默认键 `Ctrl+W`、`Ctrl+Shift+C`、`Alt+Z`、`Ctrl+,`、`Ctrl+Shift+[`”／“键位显示采用物理键名格式……例如 `Ctrl+KeyW`、`Ctrl+Shift+KeyC`、`Alt+KeyZ`。”
- 现象：命令表与验收使用符号键名，显示/存储规则使用物理键名；标点键（逗号、方括号）没有给出 `Comma`、`BracketLeft` 等映射，设计中 `PhysicalKey` 已包含这些枚举值。
- 影响：设置页可能显示 `Ctrl+Comma`/`Ctrl+Shift+BracketLeft`，而规格、验收与用户文档使用 `Ctrl+,`/`Ctrl+Shift+[`；冲突警告文案与测试断言不一致。
- 证据：上述两份文档原文；`design/ux-framework.md` §快捷键模型与重绑的 `PhysicalKey` 枚举。
- 建议修法：明确“面向用户的显示格式”和“内部存储/测试格式”两套表示及转换表；命令表统一使用一种格式或加注对应物理键名。
- 裁决对象：规格修订

### app-language-label-drift [P2] 语言选项用户标签在规格与设计间不一致，默认值也未定义

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/settings/app-page.md`、`docs/devPlan/design/theme-and-i18n.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/settings.md` §应用页/界面语言；`docs/devPlan/design/settings/app-page.md` §数据模型、§界面语言；`docs/devPlan/design/theme-and-i18n.md` §决策与红线
- 原文：“界面语言：`System` / `English` / `Chinese`。”／“单选：系统 / English / 简体中文。”“标签 `简体中文`。”
- 现象：规格把第三项写成 `Chinese`，设计与主题设计写 `简体中文`；应用页数据模型没有给出语言默认值，规格也未定义默认档。
- 影响：用户可见文案与翻译键不一致；首次启动语言默认值不确定；验收无法断言选项文案。
- 证据：上述三份文档原文；`docs/devPlan/spec-issues.md` §五“语言枚举统一 `LanguagePreference::{System, English, Chinese}`”。
- 建议修法：保留枚举名，明确用户标签为 `System`/`English`/`简体中文`（或统一为 `Chinese`），并定义默认 `System`；同步规格、设计与验收。
- 裁决对象：规格修订

### ime-gating-drift [P2] 规格只禁止组合态单键快捷键，设计要求组合态零命令匹配

- owner：`docs/featureSpec/cross-cutting.md`、`docs/devPlan/design/ux-framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/cross-cutting.md` §键盘与输入；`docs/devPlan/design/ux-framework.md` §IME 门控与 CJK、§命令派发层序
- 原文：“输入法组合期间不触发单键快捷键、不打断组合、不误提交。”／“`composing == true` 时，任何单键、组合键、功能键都不进入命令匹配。”
- 现象：规格只限制单键快捷键，设计把组合键与功能键也全部排除；规格没有说明带修饰键的命令在组合态是否可用。
- 影响：CJK 输入法组合期间按修饰键组合可能触发全局命令（按规格）或完全不触发（按设计）；输入法出字与快捷键验收不一致。
- 证据：上述两份文档原文；`docs/featureSpec/panels/settings.md` §快捷键页“输入法组合态中的按键被忽略”。
- 建议修法：统一采用设计的“组合态零命令匹配”并修改规格；或明确允许的组合键集合与条件。补 IME 组合态组合键/功能键用例。
- 裁决对象：规格修订

### sidebar-restore-default-wording [P2] “恢复默认侧栏布局”的“默认可见性”未区分视图按钮可见性与侧栏可见性意图

- owner：`docs/featureSpec/panels/settings.md`、`docs/devPlan/design/panels/settings.md`
- 类型：缺口 / 跨篇漂移
- 定位：`docs/featureSpec/panels/settings.md` §外观页/侧栏默认布局、§验收/外观页；`docs/devPlan/design/panels/settings.md` §页面契约/外观页；`docs/featureSpec/sidebar/framework.md` §视图隐藏与恢复；`docs/devPlan/design/sidebar/framework.md` §显隐与槽位
- 原文：“恢复内置顺序、槽位归属与默认可见性；不改变项目布局与面板内容。”／“恢复全部视图按钮可见与注册默认槽位/顺序；不改变侧栏可见性意图、宽度、分割比例。”
- 现象：设置页用“默认可见性”这一含糊表述，可能被实现为恢复侧栏显示；侧栏 owner 明确只恢复按钮可见性，保持侧栏可见性意图、宽度、比例不变，并定义已打开视图的迁移/关闭规则。设置页也没有写这些不变项。
- 影响：用户执行恢复默认后侧栏可能被强制显示、宽度/比例被重置；已打开视图的迁移行为无验收；与侧栏 owner 冲突。
- 证据：上述四份文档原文。
- 建议修法：改为“全部视图按钮恢复可见”，并补“不改变侧栏可见性意图、宽度、分割比例、项目布局与面板内容”；已打开视图按侧栏框架迁移/关闭规则引用，不重复定义。
- 裁决对象：规格修订

### settings-and-persistence-app-scope [P2] “设置内容为应用级共享配置”与 Hooks Project/Local 项目作用域冲突

- owner：`docs/featureSpec/settings-and-persistence.md`
- 类型：冲突
- 定位：`docs/featureSpec/settings-and-persistence.md` §设置归属与作用域
- 原文：“设置中心是项目内的普通面板，每个项目最多一个；设置内容为应用级共享配置，不按项目分叉。”／“Hooks 的 Project 与 Local 配置绑定所属项目根。”
- 现象：同一节先断言设置内容全部应用级、不按项目分叉，随后又规定 Hooks Project/Local 绑定所属项目根。
- 影响：读者可能把 Hooks 项目层当成应用级配置，影响导入/导出、备份、面板归属与项目切换语义；与 `panels/settings.md`“Hooks 的 Project/Local 属于项目级配置”冲突。
- 证据：上述原文；`docs/featureSpec/panels/settings.md` §项目作用域。
- 建议修法：改为“除 Hooks 的 Project/Local 层外，设置内容为应用级共享配置”，并明确 Hooks 项目层绑定面板所属项目。
- 裁决对象：规格修订

### window-geometry-scope [P2] 规格把窗口几何列入当前本地数据根，设计与平台文档把它列为范围外迁移

- owner：`docs/featureSpec/settings-and-persistence.md`、`docs/devPlan/design/persistence.md`
- 类型：冲突
- 定位：`docs/featureSpec/settings-and-persistence.md` §数据形态边界；`docs/devPlan/design/persistence.md` §数据根与文件；`docs/devPlan/design/platform-windows.md` §窗口几何；`docs/devPlan/design/architecture.md` §新数据目录与文件名
- 原文：“项目布局、项目数据、窗口几何、崩溃恢复快照……存放在机器本地数据目录。”／“窗口几何保持范围外迁移能力……规格内不创建该文件，关闭链不冲刷窗口几何。”
- 现象：规格把窗口几何作为当前目标态本地数据；设计与平台 owner 明确窗口几何是范围外迁移能力，规格内不创建、不冲刷。
- 影响：本地数据根清单、诊断/清理范围、迁移验收与测试夹具可能多出 `window.json`；规格与设计对当前交付范围不一致。
- 证据：上述三份文档原文；`docs/devPlan/spec-issues.md` §五“窗口几何与静默启动”。
- 建议修法：把“窗口几何”标注为迁移后能力，或从当前数据形态清单移除；同步诊断/清理与数据根说明。
- 裁决对象：规格修订

### notification-contract-duplication [P2] 通知时长、去重与缓存数值在规格与设计两处定义，悬停暂停口径也不一致

- owner：`docs/devPlan/design/cross-cutting.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/devPlan/design/cross-cutting.md` §通知类别与时长；`docs/featureSpec/workspace/shell.md` §通知浮层、§系统通知；`docs/featureSpec/cross-cutting.md` §错误与失败；`docs/devPlan/spec-issues.md` §五“通知”
- 原文：“默认信息 4s、成功 3s、警告 4s、错误 5s”“同一键 10 秒内只通知一次并只闪一次；缓存超过 200 条裁剪为最近 100 条”／规格同样写“成功 3 秒、警告 4 秒、错误 5 秒”“10 秒……200 条……100 条”。
- 现象：`spec-issues.md` 要求“通知时长/去重/缓存单源”，但规格工作区篇与设计 cross-cutting 同时写出完整数值；规格 cross-cutting 只写“悬停暂停”，设计写“悬停或聚焦暂停”。
- 影响：数值或暂停行为修改时两处漂移；验收可能按不同暂停条件断言；owner 索引与实际定义位置不一致。
- 证据：上述四份文档原文；`docs/devPlan/README.md` 契约所有者索引“错误、通知、任务服务、性能预算 → design/cross-cutting.md”。
- 建议修法：保留一个数值 owner（建议 featureSpec/workspace 写需求值，设计引用；或反之），删除另一处完整数值；统一悬停/聚焦暂停语义并在验收中写清。
- 裁决对象：规格修订 + 设计修订

### shortcut-contract-duplication [P2] 设置设计篇重复定义 ux-framework 的保留键与派发顺序

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/devPlan/design/panels/settings.md` §页面契约/快捷键页；`docs/devPlan/design/ux-framework.md` §快捷键模型与重绑、§命令派发层序；`docs/devPlan/README.md` §契约所有者索引
- 原文：“保留键：终端 `Ctrl+C/V/X/Z/A`；编辑器 `Ctrl+F`、`Ctrl+Z`、`Ctrl+Y`、`Ctrl+Shift+Z`、`Tab`、`Shift+Tab`；全局为并集。”／设计派发表“面板固定键/保留键 > 面板命令 > 侧栏视图快捷键 > 全局命令 > 终端输入。”
- 现象：README 把命令、快捷键、焦点归 `ux-framework.md`，设置设计篇又逐字复述保留键集合与派发层序，且与规格的派发顺序不一致（见 `dispatch-layer-order-conflict`）。
- 影响：保留键或层序修改时设置页复述滞后；两个 owner 的派发表可能给出不同顺序；文档门禁的单一 owner 规则被违反。
- 证据：上述三份文档原文。
- 建议修法：设置设计篇只保留“本页展示哪些命令、录制交互、修复横幅”等页面契约，保留键与派发顺序改为引用 `ux-framework.md`；删除重复表。
- 裁决对象：设计修订

### agent-filter-semantics-duplication [P2] 设置设计篇重复定义 Agent 文件视图的过滤消费语义

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/devPlan/design/panels/settings.md` §页面契约/Agent 基础配置页；`docs/devPlan/design/sidebar/agent-files.md` §过滤消费、§设置中心联动；`docs/devPlan/README.md` §契约所有者索引
- 原文：“侧栏展示内容：全部/自定义；自定义名单 chip 增删、空名单文案、添加行与校验（空、含路径分隔符、重名）；‘显示运行时文件’默认关，自定义模式置灰但值保留；全部模式显示运行时文件清单。”／“配置唯一来源是设置中心 Agent 基础配置页，按 Agent 分别配置；视图只消费，不内嵌编辑表单、不保存配置。”
- 现象：设置设计篇复述了根层精确匹配、全部/自定义模式、运行时文件隐藏集合、自定义节点重探测等过滤消费语义；这些由 `sidebar/agent-files.md` 拥有。两处对“按 Agent 配置”与“同一份配置”的表述还不一致。
- 影响：过滤规则修改时设置页复述滞后；实现可能把消费逻辑放进设置页或反之；与 `agent-config-scope-conflict` 叠加导致作用域无法确定。
- 证据：上述三份文档原文。
- 建议修法：设置设计篇只保留表单字段、校验与即时同步；根层匹配、隐藏集合、重探测与监听停止引用 `agent-files.md`。同时按 `agent-config-scope-conflict` 的字段作用域矩阵修正复述。
- 裁决对象：设计修订

### design-contract-duplication [P3] 设计文档重复其他 owner 的契约尺寸与通知/关闭顺序数值

- owner：`docs/devPlan/design/theme-and-i18n.md`、`docs/devPlan/design/persistence.md`、`docs/devPlan/design/cross-cutting.md`
- 类型：冲突 / 文档规则
- 定位：`docs/devPlan/design/theme-and-i18n.md` §语义 token 目录；`docs/devPlan/design/persistence.md` §线程与生命周期；`docs/devPlan/design/cross-cutting.md` §通知类别与时长；`docs/devPlan/README.md` §维护规则
- 原文：“自绘标题栏 34px、窗口控制钮 38×26px、面板组最小 120×80px、面板页签最小 80px、通知最大宽度 320px、设置中心左导航 180px。”／“关闭顺序：停止接受新写 → 冲刷设置与状态库 → 终止 PTY → 最终终端簿事务 → 生成备份 → 关闭连接 → 清理快照/隔离件。”
- 现象：主题设计重复工作区/平台/设置页的布局尺寸；持久化设计重复架构的关闭链顺序；cross-cutting 重复规格工作区的通知数值。README 要求“契约数值只在所有者文档定义，其他文档引用”。
- 影响：owner 文档修改后副本漂移；实现可能引用错误副本；文档门禁虽未报错，但违反单点所有权。
- 证据：上述文档原文；`docs/devPlan/spec-issues.md` §五“关闭状态机以 architecture.md 为准”“通知时长/去重/缓存单源”。
- 建议修法：把尺寸改为 token/引用，关闭顺序引用 `architecture.md`，通知数值引用单一 owner；删除重复数值。
- 裁决对象：设计修订

### state-db-resource-caps [P3] 状态库载荷与总大小上限只在设计定义，规格资源上限索引未覆盖

- owner：`docs/devPlan/design/persistence.md`
- 类型：缺口
- 定位：`docs/devPlan/design/persistence.md` §状态库 schema、§损坏、备份与只读；`docs/featureSpec/cross-cutting.md` §性能与资源
- 原文：“单载荷上限：布局 4 MiB、终端簿 1 MiB、侧栏 256 KiB、单视图 256 KiB；超限拒绝写入并通知。”“状态库总大小软上限 256 MiB，超限只读并提示。”
- 现象：这些是用户可见的拒绝/只读阈值，但 featureSpec 的资源上限索引只列终端、编辑器、Diff、Markdown、GitShow 与设置载荷，没有状态库载荷/总大小。
- 影响：诊断、错误文案与验收无法引用统一上限；实现可能选择不同阈值；超出上限时的用户提示没有规格依据。
- 证据：上述两份文档原文。
- 建议修法：把这些上限登记到 featureSpec 资源 owner（或明确为设计层内部阈值并说明不需用户可见），补超限拒绝/只读测试。
- 裁决对象：规格修订 或 维持并登记理由

### appearance-font-draft-retention [P3] 外观页字号输入草稿与行内错误未进入设计状态保留表

- owner：`docs/devPlan/design/panels/settings.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/settings.md` §状态保留；`docs/featureSpec/panels/settings.md` §状态保留
- 原文：“外观页字号输入草稿与行内错误 | 保留 | 保留 | 不保留 | 不保留”／设计表只有“快捷键录制态、任务输入草稿、终端/Agent 输入草稿”。
- 现象：外观页字号输入草稿与行内错误的重挂载/关闭丢失规则未在设计状态表出现。
- 影响：页面重挂载时可能保留或丢失草稿，测试缺少该行；与终端/Agent 输入草稿处理不一致。
- 证据：上述两份文档原文。
- 建议修法：在设计状态保留表补“外观页字号输入草稿与行内错误”，与规格一致。
- 裁决对象：设计修订

### diagnostics-path-placeholder [P3] 诊断脱敏占位符规格缺少 `%AGENT_ROOT%`

- owner：`docs/featureSpec/panels/settings.md`
- 类型：跨篇漂移
- 定位：`docs/featureSpec/panels/settings.md` §应用页/诊断；`docs/devPlan/design/diagnostics.md` §脱敏规则
- 原文：“绝对路径替换为 `%USERPROFILE%` / `%PROJECT_ROOT%`，保留需显式勾选。”／“项目根替换为 `%PROJECT_ROOT%`，Agent 目录替换为 `%AGENT_ROOT%`。”
- 现象：设计增加 Agent 根占位符，规格未列。
- 影响：诊断包脱敏验收与用户说明不一致；Agent 根路径可能未被规格要求替换。
- 证据：上述两份文档原文。
- 建议修法：在规格诊断脱敏占位符中加入 `%AGENT_ROOT%`，或明确 Agent 根以项目根/用户主目录规则覆盖。
- 裁决对象：规格修订

### cross-cutting-font-scaling-leftover [P3] 跨域验收仍要求“字体放大到契约上限”，与删除大字体适配的裁决冲突

- owner：`docs/featureSpec/cross-cutting.md`
- 类型：冲突 / 文档规则
- 定位：`docs/featureSpec/cross-cutting.md` §验收判据；`docs/devPlan/spec-issues.md` §五“无障碍与对比度”
- 原文：“Given 字体放大到契约上限，When 打开对话框、菜单与设置页，Then 关键操作不被截断，内容可滚动。”／“删除屏幕阅读器/角色/实时播报/200% 文本缩放等专用条款与现有对比度实现；保留键盘、焦点环、IME/CJK、减少动效与 24×24 命中区。”
- 现象：裁决删除了 200% 文本缩放/大字体适配，验收仍要求字体放大到契约上限时对话框/菜单/设置页不截断，但没有定义该上限或缩放能力。
- 影响：验收不可判定或引入已被删除的无障碍能力；测试可能实现未定义的 UI 字体缩放。
- 证据：上述两份文档原文。
- 建议修法：删除该验收，或改为终端/编辑器字号 8–32 的验收；若确需 UI 字体缩放，先恢复对应规格与裁决。
- 裁决对象：规格修订

## 交叉引用卡

### agent-files-runtime-list-model
- owner 域：`docs/devPlan/design/sidebar/agent-files.md`（侧栏与 Agent 域）
- 证据：`AgentFilterConfig { runtime_names: Vec<String> }` 为扁平列表；`docs/featureSpec/panels/settings.md` §Agent 基础配置页要求运行时文件清单“按 Agent 分组，每个内置 Agent 与每个自定义节点各占一行”，并称“同一份全局数据”。
- 不一致点：设置页按 Agent 分组编辑，消费侧只有扁平清单，无法区分每个 Agent 的默认/增删清单；需由侧栏 owner 明确数据结构或按 Agent 映射。

### workspace-close-flush-internal
- owner 域：`docs/featureSpec/workspace/shell.md`（工作区与终端域）
- 证据：同篇 §关闭链一处写“保存失败记录日志并通知 `设置保存失败，重启后将丢失`”，另一处写“受守卫文件……任一失败中止关闭并提供重试/取消”。
- 不一致点：设置冲刷失败是记录并继续，还是中止关闭，同一 owner 文档给出两种语义；影响本域 `close-flush-scope-conflict` 的裁决。

### platform-window-geometry-scope
- owner 域：`docs/devPlan/design/platform-windows.md`、`docs/devPlan/design/architecture.md`（架构与服务集成域）
- 证据：窗口几何为范围外迁移能力，规格内不创建 `window.json`、关闭链不冲刷；本域 `settings-and-persistence.md` 把窗口几何列入当前本地数据根。
- 不一致点：当前交付范围与数据根清单不一致，见 `window-geometry-scope`。

### background-task-interval-duplication
- owner 域：`docs/devPlan/design/integrations/agent-cli.md`、`docs/featureSpec/workspace/navigation.md`
- 证据：套餐余量轮询默认 10s/10–3600s、会话历史扫描默认 3s/2–300s 同时出现在设置页规格/设计与上述 owner 文档。
- 不一致点：任务配置默认值与范围由设置页还是各自域定义未收敛；建议设置页作为配置契约 owner，其余引用。

### notification-dedupe-duplication
- owner 域：`docs/devPlan/design/platform-windows.md`（架构与服务集成域）
- 证据：平台设计重复 `EventKey = (panel_id, kind, session_id, turn_key)`、10 秒去重窗口、200→100 缓存裁剪；这些由 `design/cross-cutting.md` 拥有，规格工作区篇也有同一组数值。
- 不一致点：同一组通知去重数值三处出现，违反“通知时长/去重/缓存单源”。

### app-shell-close-order-duplication
- owner 域：`docs/devPlan/design/out-of-scope/app-and-shell-services.md`
- 证据：该文档重复“聚合关闭确认 → 冲刷设置 JSON 与状态库全部 dirty facet → 并发终止 PTY → 最终终端簿/布局事务 → 托盘收尾 → 退出”。
- 不一致点：关闭链顺序归 `architecture.md`/`workspace/shell.md`，迁移文档不应复制目标态契约；建议改为引用。
