# 需求规格与开发计划全局最优评审汇总

> 评审基线：commit `f48b1ca`；输入 56 篇文档。机械门禁：`check_doc_policy.py full` 0 finding，相对链接 0 失效。只列问题，不写优点。

## 总体结论

- **规格冻结：不可。** 存在 5 个 P0，分别阻断焦点、终端状态、终端键盘可达性、保存并发语义与命令派发层序；这些是用户可见行为与核心契约的分歧，必须先裁决。
- **计划放行：不可。** P0 未关闭前，首个里程碑“规格裁决与冻结”无法判定通过；另有旧壳退役、应用页退出条件、测试门禁与 Runtime 契约等 P1 级计划缺口。
- **评审覆盖：** 6 域全部完成，56 篇文档均有覆盖矩阵；问题按域落盘，跨域重复项在本汇总中合并。
- **结论边界：** 本报告只给裁决建议；产品定位约束不可动，其余既有裁决可重审但最终由用户决定。
- **复核口径：** 分域评审文件保留各 subagent 的原始条目；本汇总在独立复核后对 owner、合并项与统计口径做了修订，冲突时以本汇总为准。

## 问题统计

| 域 | 评审文件 | P0 | P1 | P2 | P3 |
|---|---|---|---|---|---|
| 工作区与终端 | `review-workspace-terminal.md` | 3 | 7 | 14 | 11 |
| 编辑器栈 | `review-editor-stack.md` | 1 | 7 | 10 | 2 |
| 侧栏与 Agent | `review-sidebar-agent.md` | 0 | 7 | 16 | 6 |
| 设置与交互框架 | `review-settings-interaction.md` | 1 | 8 | 25 | 5 |
| 架构与服务集成 | `review-architecture-services.md` | 0 | 14 | 23 | 3 |
| 计划与全局一致性 | `review-plan-consistency.md` | 0 | 9 | 10 | 5 |

P1 共 52 条；其中 3 条吸收了同根因 P2，定级取 P1 的理由见本节末。P2 原始 98 条，3 组并入 P1 后为 95 条；另有同主题 P2 重复项（`gitshow-title-rootout`/`gitshow-title-root-outside`、`framework-design-mixed-state-scope` 等）在主题索引中并列。P3 共 32 条。

## P0 全量

| 问题键 | owner | 现象 | 影响 | 来源 |
|---|---|---|---|---|
| `save-concurrency-baseline` | `design/panels/diff.md`、`design/panels/editor.md` | 规格要求保存完成后以实际磁盘状态为新基线、不重复冲突；设计与编辑器设计要求写入后版本不一致即进入保存冲突，明确不以写入后状态建立基线。规格为裁决依据，设计为待修 owner。 | 同一并发场景按规格与按设计产生相反的保存/冲突行为；Diff 验收与保存链测试无法同时通过。 | 编辑器栈 |
| `focus-project-switch-conflict` | `featureSpec/workspace/shell.md` | 工作区壳要求切换项目后把键盘焦点送入目标面板焦点落点；导航树要求项目激活后焦点留在项目行。 | 切换项目后的键盘输入目标不确定；焦点验收互斥；违反焦点由工作区壳单点拥有的约定。 | 工作区与终端 |
| `terminal-book-persistence-conflict` | `featureSpec/workspace/shell.md` | 工作区壳把终端自定义标题与编号计数列入项目布局持久化内容；终端设计与领域模型把二者只归 `TerminalBook`/`project_terminals`，布局 JSON 明确不保存。 | 同一状态出现两个持久化权威；重启后编号可能重算、标题丢失或两处不一致。 | 工作区与终端 |
| `terminal-keyboard-only-copy-unreachable` | `featureSpec/panels/terminal.md` | 终端验收要求纯键盘完成复制；规格只定义鼠标拖拽建立选区，且明确不提供扩展选区与全选。 | 纯键盘用户没有建立选区的路径，复制验收不可满足；与跨域键盘可达承诺冲突。 | 工作区与终端 |
| `dispatch-layer-order-conflict` | `design/ux-framework.md`（唯一 owner；featureSpec README、`cross-cutting.md`、`panels/settings.md` 为需同步消费者） | 设置规格前文、设置规格后文、README、`spec-issues.md` 与设计派发表至少给出三套互相矛盾的层序；侧栏视图快捷键相对面板命令的位置不一致。 | 同一和弦由谁消费不可判定；快捷键冲突警告、终端输入让位与验收无法冻结。 | 设置与交互框架 |

## P1 全量（跨域去重后）

来源缩写：WT=工作区与终端，ED=编辑器栈，SB=侧栏与 Agent，SI=设置与交互框架，AS=架构与服务集成，PC=计划与全局一致性。

### 关闭链与生命周期

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `arch-close-state-order` | `design/architecture.md` | 架构规定用户确认后进入 `Guarding`；`gpui-runtime.md` 规定进入 `Guarding` 后收集脏面板并弹确认。 | AS |
| `close-chain-guarded-facet-ambiguity` | `featureSpec/workspace/shell.md` | 项目索引、布局、终端簿同时属于“失败中止关闭”的受守卫集合与“只记录日志”的最终保存集合。 | WT |
| `close-flush-scope-conflict` | `featureSpec/settings-and-persistence.md`、`design/persistence.md` | 设置持久化要求项目移除与批量关闭前冲刷设置和布局；工作区/架构 owner 只把守卫冲刷放在窗口关闭链。 | SI |
| `editor-autosave-close-missing` | `design/workspace/shell.md` | 关闭守卫算法没有“启用自动保存的编辑器先保存一次、成功则静默关闭”的分支。 | WT |
| `old-shell-retire-before-replacement` | `plan.md`、`design/migration.md` | 旧壳退役只依赖垂直切片，但切片不覆盖编辑器、Markdown、Diff、GitShow、Explorer、Commit、设置中心；退役后这些能力无替代或占位契约。 | PC |
| `app-page-exit-placeholder-conflict` | `plan.md` | 应用页范围外节里程碑的退出条件要求“节级矩阵逐项替换”，但同一里程碑声明范围外节保持只读占位，功能替换属后续里程碑。 | PC |
| `arch-gate-red-inside-milestone` | `design/architecture.md`、`plan.md` | 计划与架构允许“里程碑内短暂红灯”并提交，与项目“提交前全部门禁全绿”的硬性约束冲突。 | AS |
| `root-relocation-detached-drift` | `design/workspace/shell.md` | 根重定位把“干净根外只读”与“脏缓冲脱离模式”合并为“根外只读 + 另存为”，形成不可写却可另存为的矛盾状态。合并 `root-relocation-dirty-mode`。 | PC/WT |

### 焦点与命令

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `dispatch-conflict-resolution-conflict` | `design/ux-framework.md`（规格为需同步消费者；从属 `dispatch-layer-order-conflict`） | 规格允许按“最近焦点”历史裁定冲突；设计明确采用固定层序并拒绝历史启发式。 | SI |

### 持久化、状态与裁决落点

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `crash-snapshot-identity` | `design/persistence.md`（恢复索引与配额）、`design/panels/editor.md`、`design/panels/markdown.md` | 规格与已关闭裁决要求快照键 `(ProjectId, PanelId)`；编辑器设计以 `DocumentKey`（项目+路径）为键，Markdown 设计未给键与配额。 | ED |
| `sidebar-layout-persistence-conflict` | `featureSpec/settings-and-persistence.md`、`design/panels/settings.md` | 侧栏默认布局同时被归入 2 秒应用设置与 500ms 状态库侧栏 facet；导入/导出/恢复默认是否包含侧栏布局无法判定。 | SI |
| `active-project-storage-conflict` | `featureSpec/workspace/shell.md` | 工作区壳把活跃项目标识归入统一设置存储；持久化设计放在本地状态库 `workspace` 表，且数据根划分要求工作区状态在本地根。 | WT |
| `domain-dto-contract-missing` | `design/domain-model.md` | `WorkspaceDto`、`ProjectLayoutDto`、`TerminalsDto` 被声明为状态库 JSON 载荷，但没有字段、tag、`revision`、容器形态与重复项表示。 | AS |
| `domain-restore-orphan-records` | `design/domain-model.md` | 恢复对账没有删除“存在于 `layout.panels` 但无任何组引用”的 `PanelRecord`，且丢弃规则语句方向写反。 | AS |
| `arch-settings-recovery-dir-dependency` | `design/architecture.md` | `slterm_settings::paths::recovery_dir(ProjectId)` 要求 settings 依赖 `slterm_workspace`，违反 settings 零生产依赖契约。 | AS |
| `spec-revision-landing-gaps` | `spec-issues.md` | 修订矩阵多条声称的 featureSpec 落点不存在或落在其他篇：数据根、最小窗口尺寸、Diff 根重定位、GitShow 聚焦复核、写入根分表、通知时长。PTY 全局创建顺序与并发 8 设计已有，缺口只在 featureSpec/acceptance；与 `terminal-restore-order-undefined`、`terminal-concurrency-8` 同源，先裁决 8 是否契约。 | PC |

### 终端与单实例移交

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `shell-failure-recovery-mode-inverted` | `featureSpec/panels/terminal.md` | 规格正文把“改用自动探测重试”作为显式 Shell 失败后的恢复动作；错误表与设计写成自动探测模式下提供，语义反转。 | WT |
| `handover-new-terminal-undefined` | `featureSpec/workspace/shell.md` | 单实例 `NewTerminal` 未定义目标项目解析、目标面板组归属、cwd 越界回退、PTY 上限/关闭中拒绝与成功焦点；工作区设计与平台设计对“当前聚焦面板组”归属不一致。 | WT |
| `runtime-handover-wire-owner` | `design/integrations/runtime-api-v2.md` | 协议把移交载荷类型与校验归平台文档，但协议 crate 模块树包含 `methods/handover.rs`，且协议 crate 不能依赖平台/app；方法 schema 无落点。 | AS |

### 编辑器、Markdown、Diff

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `markdown-agentroot-boundary` | `featureSpec/panels/markdown.md`、`design/panels/markdown.md` | 编辑器与安全设计允许 AgentRoot 内文件可编辑/资源可读；Markdown 链接与资源规则只以项目根为边界，缺少 AgentRoot 分支。 | ED |
| `markdown-large-doc-form` | `featureSpec/panels/markdown.md`、`design/panels/markdown.md` | 未定义“持久化形态为渲染 + 大文档”在打开/重启时谁优先；超过 50MB 时可能进入没有“仍要渲染”入口的渲染态。 | ED |
| `diff-large-dirty-gutter` | `featureSpec/panels/diff.md`、`design/panels/diff.md` | 从 gutter 打开需带入编辑器未保存缓冲并标脏；但工作区侧超过 10MiB 又规定只读且不存在未保存状态。 | ED |
| `editor-detached-autosave` | `featureSpec/panels/editor.md`、`design/panels/editor.md` | “文件已移出允许根”是可编辑且可脏的状态，却不在自动保存暂停列表；计时器到期后的行为未定义。 | ED |
| `editor-language-map-drift` | `featureSpec/panels/editor.md`、`design/panels/editor.md` | 设计把 `.sh`、`.ps1`、`.go` 等扩展为共享 `LanguageRegistry`；编辑器规格仍把这些归为纯文本，语言选择器选项集不一致。 | ED |
| `diff-head-polling` | `design/services/filesystem-git.md`（监听契约）、`design/panels/diff.md`、`featureSpec/panels/diff.md` | Diff 规格/设计要求每 10 秒周期复核 HEAD；文件监听服务设计明确“监听是租约，不轮询”，配置表也没有 Diff 项。 | ED |
| `diff-dedupe-key-conflict` | `featureSpec/workspace/shell.md` | 工作区壳把“来源/模式”纳入 Diff 去重键，与 Diff 规格“同一工作区路径只存在一个默认 Diff 面板”冲突；冲突快照去重规则也被混入。 | WT |

### 侧栏与 Agent

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `framework-navigation-state-scope` | `featureSpec/sidebar/framework.md` | 导航树展开/滚动作用域在框架规格、导航规格、侧栏设计之间出现应用级、项目级、混合三种口径。 | SB |
| `framework-view-hidden-term` | `featureSpec/sidebar/framework.md` | 框架把“视图隐藏”定义为只移除活动栏按钮、内容可继续显示交互；文件浏览器与 Agent 文件把“视图隐藏”当作不可见，据此停快捷键与监听。 | SB |
| `explorer-reveal-contract-missing` | `featureSpec/sidebar/explorer.md` | Commit 的“在文件浏览器中显示”要求文件浏览器定位，但文件浏览器没有定义过滤、忽略显隐、祖先展开、选中/滚动/聚焦与失败反馈。 | SB |
| `explorer-clipboard-source-conflict` | `featureSpec/sidebar/explorer.md` | 安全边界只接受“本视图”选中集为剪贴板源；同篇树内剪贴板允许跨视图复制，Agent 文件也要求跨视图复制源。 | SB |
| `commit-rollback-index-state` | `featureSpec/sidebar/commit.md`、`design/sidebar/commit.md` | 回滚只描述覆盖工作区，设计用 `git checkout HEAD -- path` 同时改索引；renamed 回滚未清除新路径索引条目，会残留 `AD`/`A.` 状态。 | SB |
| `agent-files-watcher-scope` | `design/sidebar/agent-files.md` | 设计保留 App 级常驻 watcher；规格要求视图隐藏、Agent 收起或不可见时停止监听并取消合并刷新计时。 | SB |
| `agent-cli-session-provider-io` | `design/integrations/agent-cli.md`、`design/architecture.md` | `SessionProvider` 的 scan/delete/restore 直接承载文件 I/O，模块树又放在 core；架构与文件服务要求 I/O 留在 app 并经 PathAuthority。合并 `arch-agents-core-io`。 | SB/AS |
| `active-session-usage-fallback` | `design/workspace/navigation.md` | 导航设计让活跃会话用量在缺失时回退到套餐余量；会话领域 owner 明确用量与套餐余量是两套模型、两套刷新节奏。 | WT |

### 设置、Agent 配置与安全

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `agent-config-scope-conflict` | `featureSpec/panels/settings.md`、`design/panels/settings.md` | Agent 基础页四类数据在“应用级共享 / 按 CLI / 按 Agent”之间口径矛盾；页级恢复默认的复位范围无法判定。 | SI |
| `app-update-switch-conflict` | `featureSpec/panels/settings.md`、`design/settings/app-page.md` | 更新开关在规格与设计之间是“仅手动检查”一个，还是“自动检查 + 后台下载 + 仅手动”三个；同一语义存在互逆控件。 | SI |
| `app-language-failure-conflict` | `featureSpec/panels/settings.md`、`design/settings/app-page.md` | 规格要求语言保存失败回退原语言；设计要求保留本轮新语言并提示仅本次运行生效。 | SI |
| `hooks-contract-duplication` | `design/panels/settings.md` | 设置设计篇复制 Hooks 三层、保存事务、注入/卸载与外部修改契约，并新增“页面重挂载层级重置 User”与规格的切换页重置规则冲突。 | SI |
| `app-managed-write-boundary` | `design/security.md` | 安全设计的允许根只有项目根、仓库根、Agent 根，没有应用数据根、临时目录、备份/隔离/日志/Hooks 等“应用托管写入”作用域。 | SI |
| `build-update-manual-check-feedback` | `design/build-and-release.md` | 手动检查更新失败只记录日志；应用页要求非阻塞展示“已是最新/发现新版本/检查失败”。 | AS |

### Runtime、构建、测试与资产

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `runtime-command-params-missing` | `design/integrations/runtime-api-v2.md` | 多数写方法只有方法名，没有参数 schema、可选性、默认值、边界校验；`params_hash` 规范化与错误 details 形状未定义。 | AS |
| `runtime-mutation-cache-scope` | `design/integrations/runtime-api-v2.md` | `mutation_id` 缓存按共享单 token 作用域；两个客户端可用相同 mutation id 命中彼此响应，写操作被静默跳过。 | AS |
| `fs-git-literal-pathspecs-order` | `design/services/filesystem-git.md` | `git status ... --literal-pathspecs` 把全局 flag 放在子命令之后，按示例执行会报 unknown option；决策表又要求全局 flag 在子命令之前。 | AS |
| `build-package-cli-logos` | `design/build-and-release.md` | 包内资产清单没有 CLI 品牌 logo，嵌入/随包 canonical 路径未定义；`PackageEntries`、安装器、便携包可一致地漏掉。 | AS |
| `test-virtual-window-feature-gate` | `design/testing.md`、`build-and-release.md` | 标准测试门禁未带 `gpui-test-support` feature，虚拟窗口整层 UI 测试在门禁中不编译、不运行。 | AS |
| `assets-snapshot-counts` | `featureSpec/assets/README.md` | 资产 README 写入文件数、分类数、预览数等代码可自证计数，违反快照数字禁令。 | AS |
| `assets-logo-license` | `featureSpec/assets/README.md` | 文档承认 CLI logo 未逐文件确证许可，但发布迁移要求随包保留、Agent 设计计划构建期嵌入；第三方声明却声明 LobeHub MIT。 | AS |

### 计划、范围外与裁剪

| 问题键 | owner | 现象 | 来源 |
|---|---|---|---|
| `out-of-scope-owner-drift` | `devPlan/README.md`、`design/out-of-scope/*.md` | 五篇范围外文档直接定义目标态行为并声明所有权，违反“范围外只写迁移增量”；契约所有者索引未登记这些 owner。 | PC |
| `diagnostics-action-set-drift` | `spec-issues.md`、`design/diagnostics.md`、`design/settings/app-page.md` | 已关闭裁决把诊断动作收敛为固定集合；设计新增“导出诊断包”“运行环境自检”，未重开裁决。合并 `diagnostics-actions-scope`。 | PC/SI |
| `secretvalue-redaction-drift` | `design/out-of-scope/ai-providers-assistant.md`、`design/security.md` | `SecretValue` 的 `Debug` 在安全 owner 输出 `<redacted>`，AI 范围外设计写“只报长度”。 | PC |
| `tray-retry-drift` | `design/out-of-scope/app-and-shell-services.md`、`design/platform-windows.md` | 平台 owner 写托盘创建失败“下次启动重试”；范围外文档改为进程内 1 Hz 重试。 | PC |
| `provider-type-drift` | `design/settings/app-page.md`、`design/out-of-scope/ai-providers-assistant.md` | 应用页引用 `ProviderCatalog`/`ProviderEntry`；AI 设计定义 `ProviderRecord`/`ProviderStoreV2`，类型与容器不一致。 | PC |

合并定级说明：

- `root-relocation-detached-drift` 取 P1：根重定位后脏缓冲被当作根外只读，涉及未保存数据与根外写权限。
- `diagnostics-action-set-drift` 取 P1：违反已关闭裁决，并新增诊断包导出等安全敏感动作。
- `agent-cli-session-provider-io` 取 P1：core/app 边界与 `PathAuthority` 可能被绕过，影响安全与测试分层。
- `assets-snapshot-counts` 取 P1 而 `snapshot-count-crate-count` 取 P3：前者直接违反已关闭的“删除快照数字”裁决，属规格合规缺陷；后者是方向建议。

## 跨域冲突裁决

| 冲突簇 | 涉及 owner | 裁决建议 |
|---|---|---|
| 命令派发层序 | `design/ux-framework.md`（唯一 owner）、featureSpec README、`cross-cutting.md`、`panels/settings.md`、`spec-issues.md` | 由用户选定唯一层序。推荐采用 `ux-framework` 的确定性层序（临时面 > 系统键 > 面板固定键/保留键 > 面板命令 > 侧栏视图快捷键 > 全局命令 > 终端输入），并同步规格、README 与裁决记录；若保留“全局导航命令优先”，必须重写设计层序并说明固定键如何被截获。 |
| 关闭链 | `design/architecture.md`（状态机）、`featureSpec/workspace/shell.md`（行为）、`design/persistence.md`（冲刷）、`featureSpec/settings-and-persistence.md`（范围） | 状态机固定为 `Idle → Guarding（收集+确认）→ Saving → ShuttingDown`；Guard 阶段覆盖设置 JSON 与状态库全部 dirty facet，失败中止；最终终端簿/布局事务为 best-effort；项目移除与批量关闭不触发全局设置/布局冲刷。 |
| 去重键与标题 | 去重键归 `design/domain-model.md`；标题规则归各面板/侧栏规格；`domain-model.md` 提供纯函数并在 owner 索引登记 | 默认 Diff 键为“项目 + 规范化工作区路径”，来源不参与；冲突快照瞬态，按源面板+代次去重；Commit 不把标题后缀纳入键；GitShow 键不含后缀；标题规则按面板规格计算，领域模型只保留纯函数。 |
| 单实例移交 | `design/integrations/runtime-api-v2.md`（线缆/协议）、`design/platform-windows.md`（校验/映射）、`design/workspace/shell.md`（路由）、`featureSpec/workspace/shell.md`（NewTerminal 行为）、`design/panels/terminal.md`（NewTerminal 执行） | 明确一次移交是单 op 还是 `Vec<HandoffOp>` 及顺序；`NewTerminal` 的项目按 cwd 最深匹配、否则活跃项目，目标组为该项目的聚焦组；`request_id` 用于幂等；线缆类型归协议 crate，平台只做校验与映射。 |
| 窗口几何 | `design/platform-windows.md`（唯一 owner）、`featureSpec/settings-and-persistence.md` | 窗口几何保持范围外迁移能力；从当前数据根清单移除或标注“迁移后”；`MonitorKey` 的 DPI 字段不参与恢复或删除。 |
| 通知 | `design/cross-cutting.md`（时长/去重 owner）、`design/platform-windows.md`（系统通知）、`featureSpec/workspace/shell.md`（行为） | 数值只在 `cross-cutting.md` 定义，其余引用；规格保留需求值并注明 owner；补 Markdown 8 秒覆盖与任务栏闪烁开关门控。 |
| 路径与名称安全 | `design/security.md`（PathGuard 唯一 owner）、`design/services/filesystem-git.md`、`design/platform-windows.md` | `PathGuard` 持有策略与决策；文件服务 `PathAuthority` 作为适配器委托；平台只执行系统调用；名称校验归 PathGuard。 |
| Agent 配置作用域 | `featureSpec/panels/settings.md`、`design/panels/settings.md`、`design/sidebar/agent-files.md` | 先补逐字段作用域矩阵（侧栏展示、显示运行时文件、CLI 别名、运行时清单、自定义节点），再修正设置页与 Agent 文件模型；`AgentFilterConfig` 改为按节点键控。 |
| 范围外文档所有权 | `devPlan/README.md`、`design/out-of-scope/*.md`、各目标态 owner | 推荐方案 A：范围外文档只保留删除项、落点、迁移顺序、增量验证；目标态契约迁入实际 owner 并在索引登记。方案 B 需正式修订维护规则并删除重复定义。 |
| 测试门禁 | `design/testing.md`（门禁 owner）、`design/build-and-release.md` | 区分提交门禁与发布门禁；提交门禁加入带 `gpui-test-support` 的虚拟窗口测试；发布门禁引用 `build-and-release.md`，不各自维护清单。 |

## 方向建议

| 建议键 | 内容 | 约束标注 |
|---|---|---|
| `markdown-plaintext-memory` | 用块级文本索引或按需构建的查找索引替代渲染文档全文 `plain_text` 副本。 | 不触碰产品定位约束。 |
| `terminal-concurrency-8` | 若 PTY 并发创建上限 8 是产品/性能契约，写入规格与启动就绪验收；否则在设计中标明非契约、可调。 | 不触碰产品定位约束。 |
| `public-release-gate-gap` | 明确 AI Providers/助手与终端增强回迁是否阻塞公开发布；若阻塞则加入发布门，若可后置则标注交付版本。 | 涉及发布范围，需用户裁决。 |
| `snapshot-count-crate-count` | 删除“新增五个 core crate”“五个 crate 可编译”等数量词，保留 crate 名称列表。 | 文档规则。 |
| `process-label-rot` | 删除“优化轮/本轮/本次”等过程性标签，或改为带日期/版本锚点的表述。 | 文档规则。 |

## P2/P3 统计与索引

P2 原始 98 条（3 组并入 P1 后为 95 条）、P3 共 32 条；完整条目见各域评审文件。同主题重复项：`framework-design-mixed-state-scope` 是 `framework-navigation-state-scope` 的设计落点；`gitshow-title-rootout` 与 `gitshow-title-root-outside` 同源；`nav-result-cap-2000` 与 `agent-cli-session-result-cap` 同源；`platform-window-geometry-scope` 与 `window-geometry-scope` 同源。主题分布：

| 主题 | 主要内容 | 所在文件 |
|---|---|---|
| 去重、标题与打开路由 | Diff/GitShow/Commit 后缀与来源参与身份；根外标题路径基准；冲突快照标题；窗口标题同步。 | 工作区与终端、编辑器栈、侧栏与 Agent |
| 持久化与状态模型 | `view_states` 按项目/节点键控；视图状态保存失败与容量；状态库备份恢复入口；状态库载荷上限；设置面板空选中页与 `page_params`。 | 设置与交互框架、侧栏与 Agent |
| 侧栏与交互 | 多选双击事件顺序；视图内容随项目刷新；单一 `StateScope`；独立侧栏状态文件；隐藏视图重置顺序；嵌套滚动；根重定位相对路径。 | 侧栏与 Agent |
| Agent 与 CLI | 运行时清单默认值；`global_dirs` 多目录映射；会话结果上限；隔离区路径与生命周期；未知来源事件；过滤配置键控；成功反馈继承；只读内置根写操作。 | 侧栏与 Agent |
| 设置与命令 | 保存模式双/三模式；后台任务恢复默认；应用页保存模式；页面注册表 GPUI 边界；Agent 页系统目录校验；OSC 52 作用域；导入版本校验；主题材质归属；语义槽校验；系统键层；`Ctrl+Tab` 重绑；键名格式；语言标签；IME 门控；侧栏恢复默认措辞；Hooks 项目作用域；通知数值重复。 | 设置与交互框架 |
| 架构与运行时 | RuntimeHub 快照所有权；隐藏终端模型线程；ID 终生不复用；去抖数值；类型与字段约束；`PersistedPath.read_only`；移交 ID 形式；面板重试脏守卫；窗口几何 DPI；文件名校验；PathAuthority 双体系；永久删除入口；粘贴预检；Hooks 注入状态；可疑 statusline 确认；`project.create` 激活；Runtime 内部条款；`runtime.handover` 幂等；`terminal.exec` 边界；WorkspaceMembers 门禁；资产 slug 与预览生成。 | 架构与服务集成 |
| 计划与验收 | AI 修复条双 owner；更新无证书退出；应用页 Runtime 依赖；同步点未定义；更新链最后未表达；风险表缺口；迁移重复目标契约；旧凭据迁移；快捷键矩阵漏 Markdown；跨域验收缺口。 | 计划与全局一致性 |

P3 主要集中在：不可自动化面补登记、Runtime v1 负向断言、发布门方向建议、文档过程标签、模块树 `src/` 层级、`panic = unwind` 钉死、打包门禁、Diff `mode/source` 术语、Markdown 内存、侧栏滚动/顺序、导航相对路径、Agent 过滤键控、窗口标题、终端模型 `focused`、状态表合并、PTY 并发 8、Shell 稳定窗、实例锁路径、最终事务活跃项目、设计契约重复、状态库上限、外观草稿、诊断占位符、字体缩放残留。

## 建议修订顺序

1. **用户裁决 P0 行为分歧**：项目切换焦点、终端复制键盘路径、命令派发层序、保存并发语义、终端簿持久化权威。这五项改变用户可见行为，不能由设计单方收敛。
2. **规格修订**：按 owner 篇修正关闭链、终端/移交、Diff/Commit 去重、Agent 配置作用域、侧栏视图语义、路径安全、窗口几何范围；同步 README 状态/快捷键矩阵与 `acceptance.md`。
3. **裁决记录补齐**：逐条修复 `spec-issues.md` 修订矩阵的无落点条目；新增裁决必须带原因与边界。
4. **设计修订**：按契约 owner 修正 `architecture.md`、`domain-model.md`、`persistence.md`、`security.md`、`ux-framework.md`、`cross-cutting.md`、Runtime v2、构建/测试与各面板/侧栏设计；删除跨篇重复定义。
5. **计划与迁移修订**：修复旧壳退役顺序、应用页退出条件、同步点、更新链依赖、风险表、范围外文档所有权；同步 `migration.md` 与 `out-of-scope/`。
6. **文档规则清理**：删除快照计数、过程标签、行号引用与重复契约；复跑文档策略、链接、架构与测试门禁。
7. **复审**：P0/P1 修订完成后，按本报告覆盖矩阵重新评审对应章节，再判定规格冻结与计划放行。

## 覆盖审计

- 6 份分域评审均含覆盖矩阵，覆盖全部 56 篇输入文档。
- 5 个 P0 均已核对规格/设计原文；P1 均有 owner、来源评审文件与可执行修法，完整证据见分域文件。
- 机械门禁基线仍为绿；本次未修改 `featureSpec`、`devPlan`、`spec-issues.md` 或代码。
- 本汇总与分域文件中的问题键均为语义键，不含行号、过程编号或阶段号。
