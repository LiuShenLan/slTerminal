# 工作区与终端文档评审

## 覆盖矩阵

> 状态列中的问题键表示该章节涉及的问题；缺陷归属以问题条目的 owner 为准。

### `docs/featureSpec/workspace/shell.md`

| 章节 | 状态 |
|---|---|
| 目标行为 | 无问题 |
| 目标行为·模型与不变量 | 无问题 |
| 目标行为·项目生命周期 | 无问题 |
| 目标行为·面板组与布局操作 | 无问题 |
| 目标行为·面板类型封闭集 | `panel-lifecycle-destroy-wording` |
| 目标行为·面板标题与页签呈现 | `terminal-book-persistence-conflict`、`agent-global-file-title-summary` |
| 目标行为·布局持久化与恢复 | `terminal-book-persistence-conflict`、`active-project-storage-conflict` |
| 目标行为·焦点模型 | `focus-project-switch-conflict` |
| 目标行为·脏状态与关闭守卫 | `aggregate-dialog-settings-disambiguation` |
| 目标行为·设置中心入口与 `Ctrl+,` | 无问题 |
| 目标行为·窗口、标题栏与单实例 | `handover-new-terminal-undefined` |
| 目标行为·启动与关闭 | `close-chain-guarded-facet-ambiguity` |
| 目标行为·通知与确认框 | `taskbar-flash-gate-missing`、`notification-trigger-phantom` |
| 目标行为·状态保留 | `terminal-book-persistence-conflict` |
| 交互规则 | 无问题 |
| 交互规则·项目与导航入口 | `focus-project-switch-conflict` |
| 交互规则·布局与面板组操作 | 无问题 |
| 交互规则·面板页签 | 无问题 |
| 交互规则·焦点与快捷键 | `focus-project-switch-conflict` |
| 交互规则·设置中心入口 | 无问题 |
| 交互规则·浮层 | 无问题 |
| 边界与错误 | 无问题 |
| 边界与错误·空态 | 无问题 |
| 边界与错误·加载 | 无问题 |
| 边界与错误·错误 | `close-chain-guarded-facet-ambiguity` |
| 边界与错误·并发、恢复与数据安全 | 无问题 |
| 边界与错误·性能与资源 | 无问题 |
| 边界与错误·键盘与焦点 | 交叉卡：`keyboard-escape-terminal` |
| 边界与错误·平台与系统边界 | 无问题 |
| 边界与错误·安全边界 | 无问题 |
| 验收判据 | 无问题 |
| 验收判据·模型与项目 | 无问题 |
| 验收判据·空态 | 无问题 |
| 验收判据·布局与面板组 | 无问题 |
| 验收判据·面板类型、标题与去重 | `terminal-book-persistence-conflict`、`diff-dedupe-key-conflict` |
| 验收判据·焦点 | `focus-project-switch-conflict` |
| 验收判据·脏状态与关闭 | 交叉卡：`settings-close-copy` |
| 验收判据·批量保存并关闭 | 无问题 |
| 验收判据·`Ctrl+,` | 无问题 |
| 验收判据·单实例与窗口 | `handover-new-terminal-undefined` |
| 验收判据·通知与系统通知 | `taskbar-flash-gate-missing`、交叉卡：`notification-override-markdown` |
| 验收判据·恢复与状态保留 | `terminal-book-persistence-conflict` |
| 验收判据·键盘与焦点 | 交叉卡：`keyboard-escape-terminal` |
| 关键决策与依据 | 无问题 |

### `docs/featureSpec/workspace/navigation.md`

| 章节 | 状态 |
|---|---|
| 目标行为 | 无问题 |
| 目标行为·视图定位与内容边界 | 无问题 |
| 目标行为·界面结构与行 | 无问题 |
| 目标行为·项目行与项目操作 | 无问题 |
| 目标行为·活跃会话 | 无问题 |
| 目标行为·历史会话 | 无问题 |
| 目标行为·搜索 | 无问题 |
| 目标行为·刷新与数据更新 | 无问题 |
| 目标行为·套餐余量页脚 | 无问题 |
| 目标行为·行度量与状态保留 | 无问题 |
| 交互规则 | 无问题 |
| 交互规则·入口与默认状态 | 无问题 |
| 交互规则·鼠标 | 无问题 |
| 交互规则·键盘 | 无问题 |
| 交互规则·焦点恢复 | `focus-project-switch-conflict` |
| 交互规则·右键菜单通用行为 | 无问题 |
| 交互规则·拖拽排序 | 无问题 |
| 交互规则·行内重命名 | 无问题 |
| 交互规则·删除撤销提示 | 无问题 |
| 交互规则·跨区行为 | 无问题 |
| 交互规则·反馈 | 无问题 |
| 边界与错误 | 无问题 |
| 边界与错误·空态 | 无问题 |
| 边界与错误·加载与进度 | 无问题 |
| 边界与错误·错误 | 无问题 |
| 边界与错误·冲突与并发 | 无问题 |
| 边界与错误·数据与路径 | 无问题 |
| 边界与错误·性能与规模 | 无问题 |
| 边界与错误·键盘与输入 | 无问题 |
| 边界与错误·平台与生命周期 | 无问题 |
| 验收判据 | 无问题 |
| 验收判据·结构与项目 | 无问题 |
| 验收判据·根目录重定位 | 无问题 |
| 验收判据·重命名与排序 | 无问题 |
| 验收判据·活跃会话 | 无问题 |
| 验收判据·历史会话与未归属 | 无问题 |
| 验收判据·删除与撤销 | 无问题 |
| 验收判据·搜索 | 无问题 |
| 验收判据·刷新与余量 | 无问题 |
| 验收判据·键盘、空态与错误 | 无问题 |
| 验收判据·状态保留 | 无问题 |
| 关键决策与依据 | 无问题 |

### `docs/featureSpec/panels/terminal.md`

| 章节 | 状态 |
|---|---|
| 目标行为 | 无问题 |
| 目标行为·面板模型与归属 | 无问题 |
| 目标行为·面板构成与页签 | 无问题 |
| 目标行为·新建入口与落点 | `handover-new-terminal-undefined` |
| 目标行为·Shell 选择与工作目录 | `shell-failure-recovery-mode-inverted` |
| 目标行为·输出、渲染与滚动缓冲 | 无问题 |
| 目标行为·键盘输入、选区与剪贴板 | `terminal-keyboard-only-copy-unreachable` |
| 目标行为·链接与终端剪贴板序列 | 无问题 |
| 目标行为·尺寸、字号与光标 | 无问题 |
| 目标行为·标题、状态与 CLI 识别 | 无问题 |
| 目标行为·进程生命周期与重连 | `terminal-restore-order-undefined` |
| 目标行为·历史会话恢复 | 无问题 |
| 目标行为·状态保留与持久化 | 无问题 |
| 交互规则 | 无问题 |
| 交互规则·焦点与切换 | 无问题 |
| 交互规则·鼠标与键盘 | `terminal-keyboard-only-copy-unreachable`、`terminal-fixed-scroll-keys-undefined` |
| 交互规则·面板页签菜单与重命名 | 无问题 |
| 交互规则·复制与粘贴 | 无问题 |
| 交互规则·链接与剪贴板反馈 | 无问题 |
| 交互规则·尺寸与字号 | 无问题 |
| 交互规则·退出与重连 | 无问题 |
| 交互规则·反馈与状态呈现 | 无问题 |
| 边界与错误 | 无问题 |
| 边界与错误·空态 | 无问题 |
| 边界与错误·加载 | 无问题 |
| 边界与错误·错误 | `shell-failure-recovery-mode-inverted` |
| 边界与错误·并发与竞争 | 无问题 |
| 边界与错误·恢复与失效 | `terminal-restore-order-undefined` |
| 边界与错误·性能与资源 | 无问题 |
| 边界与错误·键盘与输入法 | `terminal-keyboard-only-copy-unreachable` |
| 边界与错误·平台与 Shell 能力 | 无问题 |
| 边界与错误·安全与隐私 | 无问题 |
| 验收判据 | 无问题 |
| 验收判据·面板模型与编号 | 无问题 |
| 验收判据·入口、Shell 与工作目录 | 无问题 |
| 验收判据·输出、回滚与渲染 | 无问题 |
| 验收判据·字号、字体与光标 | 无问题 |
| 验收判据·键盘、选区与粘贴 | `terminal-keyboard-only-copy-unreachable`、`terminal-fixed-scroll-keys-undefined` |
| 验收判据·链接与剪贴板序列 | 无问题 |
| 验收判据·生命周期、退出栏与恢复 | `terminal-restore-order-undefined` |
| 验收判据·通知、状态同步与安全 | 无问题 |
| 关键决策与依据 | 无问题 |

### `docs/devPlan/design/workspace/shell.md`

| 章节 | 状态 |
|---|---|
| 分层与模块树 | 无问题 |
| 关键类型 | `close-candidate-foreground-type` |
| 不变量 | 无问题 |
| 项目生命周期 | 无问题 |
| 根重定位的面板迁移 | `root-relocation-dirty-mode` |
| 面板组与布局操作 | 无问题 |
| 面板类型、标题与去重 | `diff-dedupe-key-conflict`、`gitshow-dedupe-suffix`、`gitshow-title-root-outside`、`diff-conflict-title-missing`、`window-title-sync` |
| 焦点模型 | 无问题 |
| 脏状态与关闭守卫 | `editor-autosave-close-missing`、`aggregate-dialog-settings-disambiguation`、交叉卡：`settings-close-copy` |
| `Ctrl+,`、配置钮与无项目设置浮层 | 无问题 |
| 窗口、标题栏与单实例 | `handover-new-terminal-undefined`、`instance-lock-path`、交叉卡：`handover-payload-order` |
| 启动与关闭 | `close-chain-guarded-facet-ambiguity`、`shell-final-save-active-project` |
| 通知与确认框 | `confirm-target-deleted`、交叉卡：`notification-override-markdown` |
| 错误边界 | 无问题 |
| 持久化与协调 | 无问题 |
| 复用与重建 | 无问题 |

### `docs/devPlan/design/workspace/navigation.md`

| 章节 | 状态 |
|---|---|
| 模型与模块 | 无问题 |
| 行模型 | `active-session-usage-fallback`、`nav-history-modified-optional`、`nav-design-boundary-gaps` |
| 扫描与操作接口 | 无问题 |
| 项目与会话归属 | 无问题 |
| 项目操作 | 无问题 |
| 活跃会话操作 | `active-session-usage-fallback` |
| 历史会话 | `nav-result-cap-2000`、`nav-design-boundary-gaps` |
| 删除与撤销 | 无问题 |
| 搜索 | `nav-design-boundary-gaps` |
| 刷新与自动扫描 | `nav-design-boundary-gaps` |
| 套餐余量页脚 | 无问题 |
| 状态作用域矩阵 | 无问题 |
| 复用与重建 | 无问题 |

### `docs/devPlan/design/panels/terminal.md`

| 章节 | 状态 |
|---|---|
| 复用与重建边界 | 无问题 |
| 模块树 | 无问题 |
| 关键类型 | `agent-cli-type-drift`、`terminal-panel-model-focused` |
| 面板实体与 PTY 生命周期 | `shell-failure-recovery-mode-inverted`、`terminal-restore-order-undefined`、`agent-cli-type-drift`、`terminal-concurrency-8`、`terminal-auto-shell-500ms` |
| 渲染、尺寸与滚动 | `terminal-fixed-scroll-keys-undefined` |
| 输入、选区与剪贴板 | `terminal-keyboard-only-copy-unreachable`、`terminal-fixed-scroll-keys-undefined` |
| OSC 8 链接与 OSC 52 剪贴板 | 无问题 |
| 标题、状态与 CLI 识别 | `agent-cli-type-drift` |
| 持久化与关闭守卫 | `terminal-book-persistence-conflict`、`close-candidate-foreground-type`、`terminal-state-table-conflation` |
| 范围外增强的扩展点 | 无问题 |
| 测试接缝 | 无问题 |

## 问题条目

### `focus-project-switch-conflict` [P0] 项目激活后的焦点落点两处规格互斥

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：冲突
- 定位：`docs/featureSpec/workspace/shell.md` §焦点模型、§交互规则·项目与导航入口、§验收判据·焦点；`docs/featureSpec/workspace/navigation.md` §交互规则·焦点恢复
- 原文：“切换项目：主区显示目标项目的布局；恢复该项目运行期最后聚焦的面板组与面板，并把键盘焦点送入该面板的焦点落点” / “项目激活后焦点留在项目行”
- 现象：同一“激活项目/切换项目”动作，工作区壳要求把键盘焦点送入目标项目的面板焦点落点，导航树要求焦点留在项目行。工作区壳还在状态保留与验收中重复该行为，导航树在焦点恢复中重复相反行为。
- 影响：项目切换后的键盘输入目标不确定：既可能进入树区，也可能进入终端/编辑器；焦点相关验收无法同时通过；键盘用户连续切换项目时行为分裂；违反跨域声明中焦点由工作区壳单点拥有的约定。
- 证据：工作区壳 §焦点模型、§交互规则·项目与导航入口、§验收判据·焦点、§状态保留；导航树 §交互规则·焦点恢复；`docs/devPlan/design/ux-framework.md` §焦点协调器明确项目切换请求 `ProjectSwitch` 意图并恢复面板焦点。设计导航的状态作用域矩阵对“树区键盘焦点·切换项目”的表述不能消解两篇规格的互斥。
- 建议修法：由焦点所有者裁决唯一行为。若保留工作区壳行为，导航树 §焦点恢复改为“项目激活后焦点按工作区壳契约进入目标面板焦点落点”，并删除“焦点留在项目行”的表述；若确实需要“仅浏览树”的激活模式，必须新增显式入口与按键并在两篇规格和验收中分别定义。设计导航的状态矩阵同步写明焦点去向。
- 裁决对象：规格修订

### `terminal-book-persistence-conflict` [P0] 终端标题与编号的持久化位置与 `TerminalBook` 单点冲突

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：冲突
- 定位：`docs/featureSpec/workspace/shell.md` §布局持久化与恢复、§面板标题与页签呈现、§状态保留；`docs/devPlan/design/panels/terminal.md` §持久化与关闭守卫；`docs/devPlan/design/domain-model.md` §终端编号与终端簿
- 原文：“每个项目持久化其项目布局，包含：……终端自定义标题与终端编号计数” / “`project_layouts.layout_json` 不保存 `terminal_counter` 或 `number`。恢复时以 terminal book 行为准”
- 现象：工作区壳把终端自定义标题与编号计数列入项目布局的持久化内容，并称“重启按布局恢复编号”；终端设计与领域模型把二者只放在状态库 `project_terminals` 的 `TerminalBook`，并明确布局 JSON 不保存编号或计数器。终端规格又写“随项目数据保存”，未指向布局。
- 影响：同一状态出现两个持久化权威。按规格实现会把标题/编号写入布局，按设计实现只写终端簿；恢复时可能从布局或终端簿取值，导致编号重算、自定义标题丢失或两处不一致；破坏状态库 schema 与“布局只存结构与面板参数”的单点约束；重启编号验收不可判定。
- 证据：工作区壳上述章节；终端规格 §状态保留与持久化（终端面板数据随项目数据保存）；终端设计 §持久化与关闭守卫；领域模型 §终端编号与终端簿（`TerminalBook.next_ordinal`/`TerminalEntry`）；`docs/devPlan/design/persistence.md` §状态库 schema（`project_terminals.terminals_json`）。
- 建议修法：从工作区壳的布局持久化清单删除“终端自定义标题与终端编号计数”，明确终端标题、编号与最近 cwd 只归 `TerminalBook`/`project_terminals`；把“重启按布局恢复编号”改为“按终端簿恢复编号”；若布局需要展示用缓存，必须标注为非权威且恢复时以终端簿覆盖。同步终端规格措辞为“随终端簿/项目终端数据”。
- 裁决对象：规格修订

### `terminal-keyboard-only-copy-unreachable` [P0] 终端验收要求纯键盘复制，但唯一选区路径是鼠标拖拽

- owner：`docs/featureSpec/panels/terminal.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/terminal.md` §键盘与输入法、§验收判据·键盘、选区与粘贴、§键盘输入、选区与剪贴板；`docs/devPlan/design/panels/terminal.md` §输入、选区与剪贴板；`docs/featureSpec/acceptance.md` §明确不做·终端
- 原文：“终端操作可仅用键盘完成：切换面板、复制、粘贴、重连、关闭” / “拖拽建立选区”
- 现象：复制以选区为前提，而终端规格只定义鼠标拖拽建立选区；终端设计明确不提供双击/三击语义选区、`Shift` 扩展选区、`Ctrl+Shift+A` 全选与 `Ctrl+End`，验收文档也把这些列为明确不做。纯键盘用户没有任何建立选区的路径，无法执行复制。
- 影响：终端验收判据“不使用鼠标可完成复制”不可满足；与跨域“核心流程键盘可达”及工作区壳的键盘可达承诺冲突；键盘用户只能复制已由鼠标建立的选区；测试无法给出通过证据。
- 证据：终端规格 §键盘与输入法、§验收判据·键盘、选区与粘贴；终端设计 §输入、选区与剪贴板；`acceptance.md` §明确不做·终端（扩展选区、全选、`Ctrl+End` 不提供）；跨域规格 §键盘与输入。
- 建议修法：二选一并同步全部文档：一是增加键盘选区路径（如 `Shift+方向键`、`Shift+PageUp/PageDown` 扩展、`Ctrl+Shift+A` 全选），并定义其在交替屏幕/鼠标跟踪下的行为及验收；二是删除“纯键盘可复制”的承诺，明确复制需要指针建立选区，并调整跨域键盘可达表述。若采用第一种，需同时更新终端快捷键表、验收与设置中心固定键参考。
- 裁决对象：规格修订

### `active-project-storage-conflict` [P1] 活跃项目标识的持久化归属与状态库单点冲突

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：冲突
- 定位：`docs/featureSpec/workspace/shell.md` §布局持久化与恢复；`docs/featureSpec/settings-and-persistence.md` §持久化对象与去抖；`docs/devPlan/design/persistence.md` §状态库 schema
- 原文：“快捷键覆盖、活跃项目标识等用户配置由统一设置存储持久化” / `workspace.active_project_id`
- 现象：工作区壳把活跃项目标识归入“统一设置存储”；状态库设计把 `active_project_id` 放在本地根 `state.db` 的 `workspace` 表。设置与持久化规格列出的应用设置域不含活跃项目，且明确工作区状态位于本地根。
- 影响：活跃项目可能被写入漫游的设置 JSON，与状态库形成双写或错放；跨机漫游时活跃项目指向不存在的本地项目；启动恢复来源不确定；违反“设置漫游、工作区状态本地”的数据根划分。
- 证据：工作区壳 §布局持久化与恢复；`settings-and-persistence.md` §持久化对象与去抖、§数据形态边界；`design/persistence.md` §状态库 schema、§数据根与文件；`docs/devPlan/spec-issues.md` 状态存储与数据根裁决。
- 建议修法：工作区壳改为“活跃项目标识由状态库 `workspace` facet 持久化”，仅快捷键覆盖留在应用设置；同步链接到持久化所有者，不在工作区壳复制写入时机。
- 裁决对象：规格修订

### `close-chain-guarded-facet-ambiguity` [P1] 关闭链受守卫 facet 与最终项目数据保存的失败语义重叠

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：冲突
- 定位：`docs/featureSpec/workspace/shell.md` §启动与关闭、§边界与错误·错误、§验收判据·单实例与窗口；`docs/devPlan/design/workspace/shell.md` §启动与关闭；`docs/devPlan/design/persistence.md` §写入模型与事务
- 原文：“受守卫文件包括应用设置、侧栏布局、项目索引、项目布局与终端簿；任一失败中止关闭并提供重试/取消” / “PTY 终止与最终项目数据保存失败只记录日志，不阻止窗口关闭”
- 现象：项目索引、项目布局与终端簿既属于“任一失败中止关闭”的受守卫集合，又属于“最终项目数据保存失败只记录”的非阻断集合；同一类别同时出现在阻断与非阻断规则中。设计用“阶段冲刷 dirty facet（Guarded）”与“最终终端簿/布局事务（BestEffort）”区分，但规格没有画出这条界线，验收同时保留两种相反期望。
- 影响：同一个写入失败可能被实现为中止关闭，也可能被实现为继续关闭；数据安全行为不可判定；关闭链测试无法同时满足两条验收；用户可能遇到“同样失败、有时窗口保留、有时退出”的不一致。
- 证据：工作区壳 §启动与关闭（三阶段与受守卫文件段）、§边界与错误·错误、§验收判据·单实例与窗口；设计工作区壳 §启动与关闭（阶段 2 与阶段 4）；`design/persistence.md` §写入模型与事务（`Guarded` 与 `BestEffort`）；`design/cross-cutting.md` §性能预算（关闭链引用 persistence）。
- 建议修法：在规格中明确阶段 2 的受守卫集合（设置 JSON 与状态库全部 dirty facet，含 `view_states`）失败即中止；阶段 4 的最终终端簿/项目布局事务与备份为 best-effort，失败只记录；删除或改写歧义句，并把验收拆成“dirty facet 冲刷失败中止”与“最终事务失败继续”两条。
- 裁决对象：规格修订

### `editor-autosave-close-missing` [P1] 关闭守卫算法遗漏编辑器自动保存关闭例外

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：缺口
- 定位：`docs/devPlan/design/workspace/shell.md` §脏状态与关闭守卫；`docs/featureSpec/workspace/shell.md` §脏状态与关闭守卫；`docs/featureSpec/panels/editor.md` §自动保存
- 原文：“单面板：无脏、无前台进程直接关闭；脏弹出标题 `未保存的修改`……” / “自动保存开启的编辑器面板：关闭时先请求一次自动保存；成功则直接关闭，不显示确认框”
- 现象：设计关闭算法只有通用脏面板确认与设置中心待写自动保存冲刷；没有“启用自动保存的编辑器在关闭前先保存一次，成功则静默关闭”的分支。
- 影响：按设计实现会对启用自动保存的脏编辑器弹出保存/丢弃确认，违反工作区规格与编辑器规格；关闭守卫验收中的自动保存例外无法通过；用户被多一次确认打断。
- 证据：设计工作区壳 §脏状态与关闭守卫；工作区壳规格 §脏状态与关闭守卫；编辑器规格 §自动保存、§保存与关闭；编辑器设计（`docs/devPlan/design/panels/editor.md`）关闭守卫接缝。
- 建议修法：在关闭算法中新增“面板声明自动保存且脏”分支：请求一次保存；成功直接关闭；失败或进入保存冲突则保留面板并进入三键确认。`PanelCapabilities` 增加自动保存/关闭前保存能力，或在 `PanelBody` 增加关闭前保存钩子；同步设计验收。
- 裁决对象：设计修订

### `shell-failure-recovery-mode-inverted` [P1] 显式 Shell 失败后的“改用自动探测”被写成自动模式

- owner：`docs/featureSpec/panels/terminal.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/terminal.md` §Shell 选择与工作目录、§边界与错误·错误；`docs/devPlan/design/panels/terminal.md` §面板实体与 PTY 生命周期、§持久化与关闭守卫·错误映射
- 原文：“显式选择缺文件或启动失败时创建失败，不静默切换到其他 Shell；错误包含路径与原因，并提供「改用自动探测重试」” / “自动探测模式下提供「改用自动探测重试」”
- 现象：规格正文把“改用自动探测重试”作为显式选择失败后的恢复动作；同一篇错误表与设计错误映射写成“自动探测模式下提供”。自动模式已经在探测，提供“改用自动探测”没有意义；显式模式反而可能没有恢复入口。
- 影响：显式 Shell 配置错误时用户可能看不到切换入口；自动探测全部失败时出现无效动作；错误恢复验收不可判定；设计实现会复制错误分支。
- 证据：终端规格 §Shell 选择与工作目录、§边界与错误·错误；终端设计 §面板实体与 PTY 生命周期（启动失败操作栏）、§持久化与关闭守卫·错误映射；终端验收 §入口、Shell 与工作目录。
- 建议修法：统一为“显式选择缺文件或启动失败时提供「改用自动探测重试」”；自动探测失败时只提供 `重试` 与 `关闭面板`，并列出各候选失败原因。同步规格错误表、设计错误映射与验收。
- 裁决对象：规格修订

### `diff-dedupe-key-conflict` [P1] Diff 去重键包含来源/模式，与 Diff 单面板契约冲突

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：冲突
- 定位：`docs/featureSpec/workspace/shell.md` §面板标题与页签呈现·去重与聚焦；`docs/featureSpec/panels/diff.md` §定位与入口、§冲突快照对比；`docs/devPlan/design/domain-model.md` §去重键与标题；`docs/devPlan/design/workspace/shell.md` §面板类型、标题与去重
- 原文：“Diff 使用项目 + 规范化工作区路径 + 模式/来源” / “同一项目内同一工作区路径只存在一个 Diff 面板。再次从任一入口打开同一文件时，聚焦已有面板”
- 现象：工作区壳把“来源”纳入 Diff 去重键，意味着 Commit 来源与编辑器 gutter 来源可为同一文件各建一个 Diff 面板；Diff 规格明确再次从任一入口打开同一文件只聚焦已有面板，来源不构成新面板。工作区壳还把“模式” broadly 纳入键，而冲突快照按“源面板 + 快照代次”去重且不持久化；设计去重表写“模式/后缀”；领域模型明确只有 `HeadToWorktree` 参与去重。
- 影响：同一文件可能打开多个默认 Diff 面板；冲突快照可能被错误地按工作区路径去重或写入布局；去重身份与 Diff 规格、领域模型不一致；验收“同文件只一个 Diff”无法判定。
- 证据：工作区壳 §去重与聚焦；Diff 规格 §定位与入口、§冲突快照对比；领域模型 §去重键与标题（`DedupeKey::Diff` 仅默认模式参与、冲突快照瞬态）；设计工作区壳 §面板类型、标题与去重。
- 建议修法：工作区壳去重规则改为引用 Diff 规格：默认模式键为“项目 + 规范化工作区路径”，来源不参与；冲突快照为瞬态面板，按“项目 + 源面板 + 快照代次”去重，不持久化、不进入布局；设计表删除“来源/后缀”并注明例外。
- 裁决对象：规格修订

### `active-session-usage-fallback` [P1] 活跃会话用量回退到套餐余量，违反 agent-cli 用量独立

- owner：`docs/devPlan/design/workspace/navigation.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/navigation.md` §行模型、§活跃会话操作；`docs/devPlan/design/integrations/agent-cli.md` §Hook 摄取与 statusline 用量
- 原文：`usage: SessionUsage, // CLI 状态事件用量优先；缺失时匹配套餐余量窗口；无数据为 None` / “用量不是套餐余量，两者模型与刷新节奏独立”
- 现象：导航设计让活跃会话用量在缺失时回退到套餐余量窗口；会话领域所有者明确活跃会话行消费 `UsageState`，套餐余量与上下文用量是两套模型、两套刷新节奏；导航设计还引入了未定义的 `SessionUsage` 类型，而所有者定义的是 `ContextUsage`/`UsageState`。
- 影响：活跃会话行可能显示套餐额度而非上下文窗口用量；刷新节奏与来源错误；无数据状态被套餐数据掩盖；类型不存在导致实现阶段被迫自造第二套用量模型，违反领域单点。
- 证据：导航设计 §行模型、§活跃会话操作；agent-cli 设计 §Hook 摄取与 statusline 用量（`ContextUsage`、`UsageState`、导航行消费约定）；领域模型/集成设计中的类型定义。
- 建议修法：导航行只持有 `UsageState`（`Unavailable`/`Fresh`/`Stale`），无数据或过期显示 `--`；删除套餐余量回退与 `SessionUsage` 类型；套餐余量仅用于页脚，刷新节奏保持独立。同步行模型注释与验收。
- 裁决对象：设计修订

### `handover-new-terminal-undefined` [P1] 单实例 `NewTerminal` 移交的项目、面板组与焦点语义未定义

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺口
- 定位：`docs/featureSpec/workspace/shell.md` §窗口、标题栏与单实例；`docs/featureSpec/panels/terminal.md` §新建入口与落点；`docs/devPlan/design/workspace/shell.md` §窗口、标题栏与单实例；`docs/devPlan/design/platform-windows.md` §单实例与移交载荷
- 原文：“第二次启动可携带结构化移交意图：仅聚焦、打开项目、打开文件、新建终端；目标项目或路径不存在时提示” / “`NewTerminal`……目标为当前聚焦面板组，没有面板组时创建第一个”
- 现象：规格只列出 `NewTerminal` 意图，没有定义目标项目如何解析（cwd 所属项目还是活跃项目）、目标面板组属于哪个项目、cwd 越界/缺失如何回退、是否受 PTY 上限与关闭流程约束、成功后焦点去向；终端新建入口表没有单实例移交这一行。设计工作区壳说“取 cwd 所属项目或活跃项目”，平台设计说“目标为当前聚焦面板组”，两者在 cwd 属于非活跃项目时对“当前聚焦面板组”的归属不一致。
- 影响：第二次启动的新终端可能落进活跃项目而非 cwd 所属项目，或落入错误项目的面板组；cwd 越界行为可能在“回退项目根”与“创建失败”之间分裂；PTY 上限/关闭中拒绝未定义；验收无法判定。
- 证据：工作区壳 §窗口、标题栏与单实例；终端规格 §新建入口与落点；设计工作区壳 §窗口、标题栏与单实例；平台设计 §单实例与移交载荷；终端设计 §面板实体与 PTY 生命周期（PTY 上限/关闭中拒绝）。
- 建议修法：在终端新建入口表增加“单实例移交 `NewTerminal`”行，并定义：项目按 cwd 在允许根内最深匹配，否则活跃项目，无项目只聚焦；目标面板组为该项目的聚焦组，无组则建首组；cwd 越界按终端工作目录规则失败或回退项目根并提示（二选一）；达到 PTY 上限不创建并提示；关闭流程中拒绝；成功后按新建规则聚焦。同步平台与工作区设计措辞。
- 裁决对象：规格修订

### `gitshow-dedupe-suffix` [P2] GitShow 去重键设计多出“后缀”

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/shell.md` §面板类型、标题与去重；`docs/featureSpec/panels/gitshow.md` §唯一入口与去重；`docs/devPlan/design/domain-model.md` §去重键与标题
- 原文：“GitShow | 项目 + 仓库相对路径（大小写不敏感）+ 后缀” / “去重键为‘项目 + 仓库内文件路径 + GitShow 类型’”
- 现象：设计给 GitShow 去重键追加可变“后缀”；GitShow 规格的键是项目 + 仓库路径 + 固定类型，领域模型也没有后缀。标题后缀 `(git delete)` 是展示结果，不是身份维度。
- 影响：若实现把后缀纳入键，同名文件消歧或标题变化时可能为同一路径创建第二个 GitShow 面板；去重行为与规格/领域模型不一致。
- 证据：设计工作区壳 §面板类型、标题与去重；GitShow 规格 §唯一入口与去重；领域模型 §去重键与标题。
- 建议修法：GitShow 去重键删除“后缀”，使用“项目 + 折叠大小写的仓库相对路径”（类型固定隐含）；标题后缀只在 `title::resolve_titles` 中生成。
- 裁决对象：设计修订

### `gitshow-title-root-outside` [P2] 项目根外 GitShow 标题规则与绝对路径规则冲突

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/shell.md` §面板类型、标题与去重；`docs/featureSpec/panels/gitshow.md` §标题；`docs/devPlan/design/domain-model.md` §去重键与标题；`docs/featureSpec/workspace/shell.md` §面板标题与页签呈现
- 原文：“项目根外文件使用规范化绝对路径” / “冲突消歧优先使用相对项目根的路径；文件不在项目根下时使用相对仓库根的路径”
- 现象：设计给所有文件型面板规定项目根外使用规范化绝对路径；GitShow 规格对项目根外、仓库内的文件要求使用相对仓库根路径。工作区壳规格本身允许“规范化绝对路径或仓库相对路径”，设计收窄成绝对路径。
- 影响：项目根外仓库内 GitShow 面板标题与 GitShow 规格不一致；同名文件消歧、悬停完整路径、标题验收可能出现分歧；同一路径在编辑器与 GitShow 的标题规则被错误统一。
- 证据：设计工作区壳 §面板类型、标题与去重；GitShow 规格 §标题；领域模型 §去重键与标题；工作区壳规格 §面板标题与页签呈现。
- 建议修法：在设计的标题规则中为 GitShow 增加面板级例外：项目根外但仓库内使用仓库相对路径；或删除全局“项目根外绝对路径”断言，改为“按所属面板规格计算，编辑器/Markdown 使用规范化绝对路径”。
- 裁决对象：设计修订

### `root-relocation-dirty-mode` [P2] 根重定位脏文件被设计归为“根外只读”，与脱离缓冲语义不符

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/shell.md` §根重定位的面板迁移；`docs/featureSpec/workspace/shell.md` §项目生命周期·项目根；`docs/featureSpec/panels/editor.md` §打开与运行模式、§外部修改
- 原文：“落到新根外则保留面板、缓冲与撤销历史，切换为根外只读并显示提示，脏缓冲允许‘另存为’” / “根外只读：……不提供保存”；“文件已移出允许根：保留缓冲、编辑能力与脏状态，禁用原地保存，提供另存为”
- 现象：设计把“根外只读”与“脏缓冲可另存为”组合成一个未定义模式。编辑器规格区分两种模式：根外只读不可编辑、不可另存为；文件已移出允许根的脱离缓冲可编辑、可另存为、不可原地保存。工作区壳规格要求“有未保存修改的转脱离缓冲并仅提供另存为”。
- 影响：根重定位后脏文件可能被错误锁定为只读（用户无法继续编辑），或在“根外只读”模式下出现规格未允许的另存为；与编辑器模式、关闭守卫和另存为授权规则冲突；验收无法按模式判定。
- 证据：设计工作区壳 §根重定位的面板迁移；工作区壳规格 §项目生命周期·项目根；编辑器规格 §打开与运行模式、§外部修改、§保存与另存为；编辑器设计根重定位接缝。
- 建议修法：按脏状态映射：干净且在新根外 → 根外只读（不可编辑/不可保存）；脏且在新根外 → 脱离缓冲（保留编辑与缓冲、禁用原地保存、提供另存为）；使用编辑器规格的模式名，不再把两者混写为“根外只读 + 另存为”。同步关闭守卫与验收。
- 裁决对象：设计修订

### `nav-result-cap-2000` [P2] 导航设计引入 2,000 条历史结果硬上限，规格只有性能要求

- owner：`docs/devPlan/design/workspace/navigation.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/navigation.md` §历史会话；`docs/featureSpec/workspace/navigation.md` §边界与错误·性能与规模
- 原文：“结果上限 2,000 条” / “2,000 条历史会话行规模下纵向滚动与键盘导航保持响应”
- 现象：规格把 2,000 条作为性能验收规模，并未规定超过 2,000 条时截断；设计把它变成硬上限，却没有定义保留哪 2,000 条、如何计数、是否通知、搜索是否只在截断集合内匹配。
- 影响：历史会话超过 2,000 条的用户会静默丢失行；计数徽标、搜索、未归属归属可能与规格不一致；“2,000 行性能”验收无法区分“支持 2,000 行”与“只显示 2,000 行”。
- 证据：导航设计 §历史会话；导航规格 §性能与规模、§历史会话、§搜索。
- 建议修法：删除硬上限，或明确“按修改时间降序保留最近 2,000 条，计数徽标显示总数/截断提示，搜索只覆盖保留集合”，并补充验收与失败提示；不要在设计中静默引入规格外行为。
- 裁决对象：设计修订

### `nav-history-modified-optional` [P2] 历史行 `modified: SystemTime` 无法表示规格要求的无效时间

- owner：`docs/devPlan/design/workspace/navigation.md`
- 类型：缺陷
- 定位：`docs/devPlan/design/workspace/navigation.md` §行模型；`docs/featureSpec/workspace/navigation.md` §历史会话、§边界与错误·数据与路径；`docs/devPlan/design/integrations/agent-cli.md` §历史会话扫描
- 原文：`pub modified: SystemTime,` / “按会话文件修改时间降序；时间无效的排最后”“时间无效显示 `-`”
- 现象：导航行模型的修改时间是非可选 `SystemTime`，无法表达缺失/无效时间；会话领域 `SessionRecord.modified` 是 `Option<SystemTime>`。
- 影响：实现只能以 epoch/当前时间伪造无效值，导致排序错误或显示错误；规格要求的“无效排最后”和 `-` 无法表达；与所有者类型不匹配。
- 证据：导航设计 §行模型；导航规格 §历史会话、§数据与路径；agent-cli 设计 §历史会话扫描（`SessionRecord.modified: Option<SystemTime>`）。
- 建议修法：改为 `modified: Option<SystemTime>`，排序时 `None` 置后，显示 `-`；同步行投影与测试。
- 裁决对象：设计修订

### `nav-design-boundary-gaps` [P2] 导航设计缺空态、错误、搜索计数、孤儿分支与焦点恢复落点

- owner：`docs/devPlan/design/workspace/navigation.md`
- 类型：缺口
- 定位：`docs/devPlan/design/workspace/navigation.md` §行模型、§历史会话、§搜索、§刷新与自动扫描、§项目操作、§删除与撤销；`docs/featureSpec/workspace/navigation.md` §空态、§搜索、§历史会话、§键盘、§焦点恢复
- 原文：“无项目且无未归属历史会话：树区显示……搜索框禁用”“历史会话计数徽标：无搜索时为该项目全部历史会话数，搜索时为当前可见匹配数”“孤儿行……分支恢复禁用”“删除历史会话后，焦点移到下一条可见历史行”
- 现象：设计没有给出以下行为落点：无项目/无未归属时空态与搜索禁用、有未归属无项目时仅显示未归属节点、项目展开无会话的“暂无会话”、搜索命中计数（可见匹配数 vs 项目总数）、项目行运行中徽标不受搜索影响、孤儿行分支禁用的原因、活跃会话标题空回退“终端会话”、移除项目/删除历史后的焦点归位、视图关闭停止相对时间定时器。
- 影响：这些规格判据在实现层无对应设计，容易按直觉实现或遗漏；空态优先级、计数与焦点行为测试缺少落点；与验收表无法一一追溯。
- 证据：导航设计上述章节；导航规格 §空态、§搜索、§历史会话、§键盘、§焦点恢复、§删除撤销提示。
- 建议修法：在行模型/状态机或独立小节补齐这些行为，至少以“规格判据 → 设计落点 → 测试接缝”表列出；与现有空态/计数/焦点实现保持一致。
- 裁决对象：设计修订

### `terminal-restore-order-undefined` [P2] PTY 恢复超限的全局顺序在规格中未定义

- owner：`docs/featureSpec/panels/terminal.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/terminal.md` §进程生命周期与重连、§边界与错误·恢复与失效、§验收判据·生命周期、退出栏与恢复；`docs/devPlan/design/panels/terminal.md` §面板实体与 PTY 生命周期
- 原文：“布局恢复时若终端数超过上限，按布局顺序连接前 64 个” / “项目列表顺序 → 项目布局树深度优先（左/上先于右/下）→ 面板组页签顺序”
- 现象：规格只说“按布局顺序”，但每个项目各有一个布局，跨项目的“布局顺序”不存在；设计补充了项目顺序 → 布局 DFS → 页签顺序。规格验收“仅前 64 个连接”没有全局顺序就无法判定。
- 影响：启动时哪些终端获得 PTY 不确定；不同实现可能按项目顺序、恢复顺序或时间顺序；超过上限的失败面板不可复现；验收不可判定。
- 证据：终端规格 §进程生命周期与重连、§恢复与失效、§验收判据；终端设计 §面板实体与 PTY 生命周期（全局顺序）。
- 建议修法：把设计的全局顺序写入规格与验收判据：项目列表顺序 → 项目布局树深度优先（左/上先于右/下）→ 面板组页签顺序；其余进入容量失败态，关闭其他终端后可重试。
- 裁决对象：规格修订

### `terminal-fixed-scroll-keys-undefined` [P2] 设计新增本地滚动固定键，规格未定义且与“功能键发送 PTY”冲突

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：冲突
- 定位：`docs/devPlan/design/panels/terminal.md` §渲染、尺寸与滚动；`docs/featureSpec/panels/terminal.md` §键盘输入、选区与剪贴板、§验收判据·键盘、选区与粘贴
- 原文：“固定键：`Shift+PageUp/PageDown` 翻页，`Shift+Home/End` 到顶/到底；交替屏幕下不拦截，交给前台程序” / “任意可打印字符、控制键、方向键、功能键 | 编码后发送到 PTY”
- 现象：设计把 `Shift+PageUp/PageDown/Home/End` 作为终端固定键拦截用于本地回滚；终端规格的快捷键表没有这些键，且声明功能键编码后发送 PTY；验收也没有本地键盘滚动的判据。
- 影响：依赖这些组合键的 TUI/CLI 收不到按键；实现与规格输入契约不一致；用户可见行为没有验收；交替屏幕下行为虽有不拦截规则，但普通屏幕仍可能影响前台程序。
- 证据：终端设计 §渲染、尺寸与滚动；终端规格 §键盘输入、选区与剪贴板、§验收判据·键盘、选区与粘贴；`acceptance.md` 终端明确不做仅涉及滚动条/滚动锁定，未提键盘滚动。
- 建议修法：把固定键写入终端规格的快捷键表与验收（含交替屏幕、鼠标跟踪下的行为），或删除这些固定键、只保留鼠标滚轮与 `Shift+滚轮`；若保留，明确“仅在本地回滚可用且前台未消费时拦截”。
- 裁决对象：设计修订

### `close-candidate-foreground-type` [P2] 关闭候选的前台进程类型与终端结构化状态不一致

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/shell.md` §关键类型、§脏状态与关闭守卫；`docs/devPlan/design/panels/terminal.md` §持久化与关闭守卫
- 原文：`pub foreground: Option<String>` / `ForegroundProcessState { None, Running(Vec<ProcessInfo>), Unknown }`
- 现象：`PanelCapabilities` 使用终端定义的结构化 `ForegroundProcessState`，但 `CloseCandidate` 只保留 `Option<String>`；`Unknown` 语义（按存在前台进程处理）无法在候选类型中表达，进程列表也丢失。
- 影响：关闭守卫可能无法区分“无前台进程”“有前台进程”“查询失败”；未知状态可能被当作 None 而不弹确认；聚合确认无法列出进程信息；类型边界与终端契约不一致。
- 证据：设计工作区壳 §关键类型、§脏状态与关闭守卫；终端设计 §持久化与关闭守卫（`ForegroundProcessState`、查询失败归 Unknown）；终端规格 §进程生命周期与重连（关闭确认提供前台进程信息）。
- 建议修法：`CloseCandidate.foreground` 使用 `ForegroundProcessState`（或携带同样的三态与 `ProcessInfo` 列表），UI 层再格式化为文本；明确 `Unknown` 按 Running 处理。
- 裁决对象：设计修订

### `agent-cli-type-drift` [P2] 终端设计引用的 Agent CLI 类型与所有者不一致

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：冲突
- 定位：`docs/devPlan/design/panels/terminal.md` §关键类型、§历史会话恢复、§标题、状态与 CLI 识别；`docs/devPlan/design/integrations/agent-cli.md` §命令识别、§历史会话扫描、§系统通知事件
- 原文：`ValidatedRestoreCommand` 只接受 `AgentSession::resume_command` 产出的命令；`AgentNotification { category, agent, project, panel }`
- 现象：会话领域所有者定义 `AgentDescriptor::resume_command(&SessionId)` 与 `AgentNotificationEvent { kind, agent, panel_id, project_name, session_id, turn_key, at }`；终端设计引用的 `AgentSession::resume_command` 与 `AgentNotification` 在所有者文档中不存在或字段不同（缺 `session_id`/`turn_key`/`project_name`）。
- 影响：恢复命令校验路径无法按所有者接口实现；系统通知事件键无法由终端上报的字段组成，去重会失效；类型在实现阶段需要二次改名/补字段，跨域契约漂移。
- 证据：终端设计上述章节；agent-cli 设计 §命令识别、§历史会话扫描、§系统通知事件；平台设计 §系统通知与任务栏闪烁（事件键需要 panel/kind/session/turn）。
- 建议修法：恢复命令使用 `AgentDescriptor`/`CommandTemplate`/`SessionId`；终端只上报 owner 定义的 `AgentNotificationEvent`（或定义终端局部事件并在组合根映射为 owner 事件），确保 `project_name`、`session_id`、`turn_key` 可用。同步类型名与字段。
- 裁决对象：设计修订

### `confirm-target-deleted` [P2] 确认框目标项目被移除时的取消/提示规则设计遗漏

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：缺口
- 定位：`docs/devPlan/design/workspace/shell.md` §通知与确认框；`docs/featureSpec/workspace/shell.md` §通知与确认框·确认框、§交互规则·浮层
- 原文：“确认框在项目切换时保持挂起，目标对象由发起时确定” / “程序化活跃项目变化时，若目标项目仍存在则继续挂起，目标项目不存在则取消并提示”
- 现象：设计只写“保持挂起、目标由发起时确定”，没有覆盖发起后目标项目被移除/不存在的分支；规格明确要求取消并提示。
- 影响：确认框可能在已移除项目上继续挂起，确认后操作悬空目标；或取消/提示路径缺失，焦点返回失效控件；关闭/移除流程出现竞态。
- 证据：设计工作区壳 §通知与确认框；工作区壳规格 §通知与确认框·确认框、§交互规则·浮层；`design/ux-framework.md` §覆盖层单一栈（锚点失效与关闭顺序）。
- 建议修法：在确认框服务中记录目标项目/面板身份；程序化项目变化时校验目标仍存在，不存在则按取消处理并提示；同步覆盖层锚点失效与焦点返回规则。
- 裁决对象：设计修订

### `taskbar-flash-gate-missing` [P2] 系统通知规格未写任务栏闪烁开关门控

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺口
- 定位：`docs/featureSpec/workspace/shell.md` §通知与确认框·系统通知；`docs/featureSpec/panels/settings.md` §应用页·通知；`docs/devPlan/design/cross-cutting.md` §通知类别与时长；`docs/devPlan/design/platform-windows.md` §系统通知与任务栏闪烁
- 原文：“所有类别均触发任务栏闪烁，闪烁是唯一回窗引导” / “通知：桌面通知总开关与任务栏闪烁开关”
- 现象：工作区壳写“所有类别均触发任务栏闪烁”，未说明受“任务栏闪烁”设置开关门控；设置规格存在该开关，跨域设计与平台设计均以 `taskbar_flash` 独立门控。
- 影响：实现可能忽略设置，关闭闪烁后仍闪烁；或规格与设置/平台行为不一致；验收缺少关闭闪烁场景。
- 证据：工作区壳 §系统通知；设置中心规格 §应用页·通知；`design/cross-cutting.md` §通知类别与时长；`design/platform-windows.md` §系统通知与任务栏闪烁。
- 建议修法：工作区壳改为“任务栏闪烁开关开启时，所有类别均触发闪烁；关闭时只发送系统通知/不闪”；补充关闭开关的验收。
- 裁决对象：规格修订

### `notification-trigger-phantom` [P2] 通知触发点包含无定义消息的“无项目时打开应用设置浮层”

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺陷
- 定位：`docs/featureSpec/workspace/shell.md` §通知与确认框·本篇范围内的通知与确认框触发点
- 原文：“通知：项目根路径设置失败、设置保存失败、项目数据保存失败、面板保存并关闭失败、批量保存失败、配置损坏、布局恢复聚合警告、无项目时打开应用设置浮层、设置中心打开失败、终端后端回退……”
- 现象：打开无项目设置浮层被列为通知触发点，但规格没有定义该通知的文案、类型或条件；设置规格与验收把“打开浮层”描述为直接显示浮层，不产生通知。
- 影响：实现者可能自行发明一条通知，或在触发点表中留下死项；通知验收出现无法对应的条目；用户打开设置浮层时收到无意义提示。
- 证据：工作区壳 §通知与确认框·触发点、§设置中心入口与 `Ctrl+,`、§错误表“无项目时打开应用设置浮层”；设置中心规格 §打开与聚焦；`acceptance.md` §跨域验收·应用页与无项目浮层。
- 建议修法：删除该触发点；若确实需要提示（例如浮层被阻止），定义具体条件、文案与类型，并加入验收。
- 裁决对象：规格修订

### `aggregate-dialog-settings-disambiguation` [P2] 聚合关闭列表面板标题未消歧，设置中心可重名

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺口
- 定位：`docs/featureSpec/workspace/shell.md` §脏状态与关闭守卫·批量关闭、§脏状态与关闭守卫·项目移除与窗口关闭；`docs/featureSpec/panels/settings.md` §保存模式与未完成状态
- 原文：“正文列出需保存面板标题与运行中终端” / “跨项目聚合确认以 `设置（<项目名>）` 区分”
- 现象：每个项目最多一个设置中心面板，标题固定为 `设置中心`；窗口关闭的聚合确认跨全部项目列出需保存面板标题时，可能出现多个同名 `设置中心`，工作区壳未要求项目限定，设置规格却定义了 `设置（<项目名>）`。
- 影响：用户无法判断要保存/丢弃的是哪个项目的设置中心；可能对错误项目执行保存/放弃；聚合关闭验收缺少重名场景。
- 证据：工作区壳 §脏状态与关闭守卫；设置中心规格 §保存模式与未完成状态、§关闭确认；工作区壳 §面板标题（设置中心固定标题）。
- 建议修法：规定聚合确认中重复标题必须加项目名限定（至少设置中心），采用设置规格的 `设置（<项目名>）` 格式；批量/窗口关闭验收增加两个项目各含设置中心的场景。
- 裁决对象：规格修订

### `window-title-sync` [P3] 设计称面板标题变化同步窗口标题，与标题栏中段固定项目名冲突

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`docs/devPlan/design/workspace/shell.md` §面板类型、标题与去重；`docs/featureSpec/workspace/shell.md` §窗口、标题栏与单实例
- 原文：“标题变化后同步页签、窗口标题（若涉及活跃项目）与名称” / “中段：活跃项目显示名”
- 现象：设计把面板标题变化与“窗口标题”同步；工作区壳规定标题栏中段固定显示活跃项目显示名，面板标题不进入标题栏。平台设计也未定义以面板标题作为窗口标题。
- 影响：标题栏文本可能随面板切换而变化，违背“中段显示活跃项目名”；任务栏/Alt+Tab 标题来源不明确；标题验收出现歧义。
- 证据：设计工作区壳 §面板类型、标题与去重；工作区壳规格 §窗口、标题栏与单实例；`design/platform-windows.md` §窗口几何、DPI 与多显示器（标题栏布局）。
- 建议修法：删除“窗口标题”同步；只同步页签与可访问名称。若需要 OS 窗口标题，单独定义为活跃项目显示名，并明确无项目时的空值。
- 裁决对象：设计修订

### `panel-lifecycle-destroy-wording` [P3] “仅有的销毁路径”与应用退出统一销毁自相矛盾

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺陷
- 定位：`docs/featureSpec/workspace/shell.md` §面板类型封闭集
- 原文：“应用退出时统一销毁。关闭与项目移除是仅有的销毁路径。”
- 现象：前句把应用退出列为销毁，后句称关闭与项目移除是仅有路径；应用退出显然也会销毁面板实例。
- 影响：实现者可能把应用退出视为例外而非销毁路径，影响资源释放/统计口径；文档自相矛盾。
- 证据：工作区壳 §面板类型封闭集、§启动与关闭；终端规格 §面板模型与归属（应用退出后内容不可恢复）。
- 建议修法：改为“面板在关闭、项目移除或应用退出时销毁；前两者是运行期内的销毁路径，应用退出由进程结束统一销毁”，或等价表述。
- 裁决对象：规格修订

### `root-relocation-acceptance-ambiguity` [P3] 根重定位验收“终端默认工作目录更新”歧义

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺陷
- 定位：`docs/featureSpec/workspace/shell.md` §验收判据·模型与项目、§项目生命周期·项目根
- 原文：“终端默认工作目录更新” / “既有终端保持原 cwd，新终端使用新根”
- 现象：验收句可能被读成既有终端的默认工作目录被更新，也可能被读成新终端使用新根；规格正文明确既有终端保持原 cwd。
- 影响：根重定位验收对既有终端 cwd 的期望不明确；实现可能迁移既有终端目录，违背“不重建终端”。
- 证据：工作区壳 §验收判据·模型与项目、§项目生命周期·项目根；终端规格 §工作目录（最近已知目录/项目根优先级）。
- 建议修法：改为“新终端默认工作目录使用新根；既有终端保持原 cwd 不变”，并增加既有终端 cwd 的验收断言。
- 裁决对象：规格修订

### `agent-global-file-title-summary` [P3] Agent 全局文件标题摘要遗漏“跨 Agent 追加显示名”

- owner：`docs/featureSpec/workspace/shell.md`
- 类型：缺陷
- 定位：`docs/featureSpec/workspace/shell.md` §面板标题与页签呈现
- 原文：“采用文件名加最短可区分上级路径” / “文件名 + 最短可区分路径；跨 Agent 追加显示名”
- 现象：工作区壳的例外摘要只写文件名 + 最短可区分上级路径，遗漏跨 Agent 追加显示名；所有者规格明确跨 Agent 需要追加显示名。
- 影响：实现者若只读工作区壳摘要，会在不同 Agent 的同名文件间产生相同标题；例外规则被截断。
- 证据：工作区壳 §面板标题与页签呈现；`docs/featureSpec/sidebar/agent-files.md` §面板标题；`docs/devPlan/design/sidebar/agent-files.md` 标题规则。
- 建议修法：摘要补“跨 Agent 追加显示名”，或改为完全引用 Agent 文件规格，不复述具体规则。
- 裁决对象：规格修订

### `diff-conflict-title-missing` [P3] 设计标题规则未覆盖冲突快照标题

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：缺口
- 定位：`docs/devPlan/design/workspace/shell.md` §面板类型、标题与去重；`docs/featureSpec/panels/diff.md` §冲突快照对比
- 原文：“Diff 默认模式追加 `(git diff)`；GitShow 追加 `(git delete)`”（设计未提冲突快照） / “标题为 `<文件名>(冲突差异)`”
- 现象：设计声称 `title::resolve_titles` 统一计算标题，但只列了默认 Diff 后缀；冲突快照模式是另一套标题规则且为瞬态面板。
- 影响：冲突快照面板可能得到默认 Diff 标题或纯文件名，无法区分快照与默认 Diff；验收与展示不一致。
- 证据：设计工作区壳 §面板类型、标题与去重；Diff 规格 §冲突快照对比；领域模型 §去重键与标题（冲突快照标题）。
- 建议修法：在标题函数中补冲突快照 `(冲突差异)`，或明确冲突快照标题由 Diff 面板自带状态提供、不进入工作区标题函数。
- 裁决对象：设计修订

### `terminal-panel-model-focused` [P3] 终端面板模型自存 `focused`，与焦点协调器单点存在重复权威风险

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：缺陷
- 定位：`docs/devPlan/design/panels/terminal.md` §关键类型；`docs/devPlan/design/ux-framework.md` §焦点协调器；`docs/devPlan/design/workspace/shell.md` §焦点模型
- 原文：`pub visible: bool, pub focused: bool`
- 现象：终端面板模型持有 `focused` 布尔；焦点所有者规定 `FocusMemory` 只归 `ProjectView`，当前焦点由 `FocusCoordinator` 唯一持有。面板模型中的 `focused` 若被当作权威状态，会形成第二份焦点。
- 影响：面板焦点与协调器状态可能漂移；切换项目/关闭面板时出现错误焦点判断；违反“焦点单一权威”。
- 证据：终端设计 §关键类型；`design/ux-framework.md` §焦点协调器（唯一 `FocusOwner`、`FocusMemory` 归 `ProjectView`）；设计工作区壳 §焦点模型（壳只接线）。
- 建议修法：删除 `focused`，或注明它只是渲染派生的只读快照、由协调器每帧写入且不作为判断依据；焦点查询统一走协调器。
- 裁决对象：设计修订

### `terminal-state-table-conflation` [P3] 终端设计状态保留表把标题与状态/CLI 元数据合并

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：缺陷
- 定位：`docs/devPlan/design/panels/terminal.md` §持久化与关闭守卫；`docs/featureSpec/panels/terminal.md` §状态保留与持久化
- 原文：“标题/状态/CLI 元数据 | 保留并更新 | 保留并更新 | 无 | 不恢复”
- 现象：终端规格区分“自定义标题”“未自定义标题（关闭重开得到新 `terminal-N` 或历史会话标题；重启历史标题不保留，回落 `terminal-N`）”“状态圆点/CLI 图标”“CLI 会话元数据”。设计表把标题与状态/CLI 元数据合并为一行，关闭重开写“无”，丢失未自定义标题的新标题规则。
- 影响：实现者可能把关闭重开后的默认标题也当作“无”，或把历史标题误当持久状态；状态保留验收的标题分支不清晰。
- 证据：终端设计 §持久化与关闭守卫；终端规格 §状态保留与持久化、§验收判据·生命周期、退出栏与恢复。
- 建议修法：拆成自定义标题、未自定义标题、状态圆点/CLI 图标、CLI 元数据四行，分别写四维行为。
- 裁决对象：设计修订

### `terminal-concurrency-8` [P3] PTY 并发创建上限 8 只存在于设计，未进入规格或验收

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：方向建议
- 定位：`docs/devPlan/design/panels/terminal.md` §面板实体与 PTY 生命周期；`docs/featureSpec/panels/terminal.md` §性能与资源
- 原文：“PTY 并发创建上限 8；界面在全部会话就绪前保持可交互”
- 现象：规格只定义 64 个 PTY 总量上限，没有并发创建上限 8；该值影响启动时终端就绪时间，但验收不覆盖。
- 影响：设计值可能被误当契约，也可能在实现中被随意调整；启动性能与“布局恢复并行连接”验收缺少可判定阈值。
- 证据：终端设计 §面板实体与 PTY 生命周期；终端规格 §性能与资源、§验收判据·输出、回滚与渲染（恢复并行连接）。
- 建议修法：若 8 是产品/性能契约，写入规格并增加启动就绪验收；若只是实现细节，在设计中标明“非契约、可调”，不进入验收。
- 裁决对象：设计修订

### `terminal-auto-shell-500ms` [P3] “未立即退出”的稳定窗阈值未进入规格

- owner：`docs/devPlan/design/panels/terminal.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/terminal.md` §面板实体与 PTY 生命周期；`docs/featureSpec/panels/terminal.md` §Shell 选择与工作目录
- 原文：设计“某个候选 spawn 失败或在 500ms 稳定窗内立即退出，则尝试下一个” / 规格“使用第一个成功创建 PTY 且未立即退出的 Shell”
- 现象：规格用“立即退出”未给阈值；设计把“立即”定义为 500ms。边界 Shell 在 600ms 退出时，规格可能判定成功、设计判定失败。
- 影响：自动 Shell 探测结果在边界条件下不一致；验收“自动探测全失败”无法覆盖稳定窗。
- 证据：终端设计 §面板实体与 PTY 生命周期（Shell 选择）；终端规格 §Shell 选择与工作目录、§验收判据·入口、Shell 与工作目录。
- 建议修法：将 500ms 稳定窗写入规格与验收，或把“立即退出”改为“在稳定窗内退出”并给出契约值。
- 裁决对象：规格修订

### `instance-lock-path` [P3] 单实例锁路径占位与平台所有者不一致

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：缺陷
- 定位：`docs/devPlan/design/workspace/shell.md` §窗口、标题栏与单实例；`docs/devPlan/design/platform-windows.md` §单实例与移交载荷
- 原文：设计“单实例锁为 `<state>/instance.lock` 句柄锁” / 平台“`<local>/state/instance.lock`”
- 现象：工作区设计用 `<state>` 占位，未说明是本地根；平台所有者明确实例锁在本地根。实例锁属于机器本地文件，不能放漫游根。
- 影响：实现者可能把锁放到漫游根，导致跨机/同步目录锁冲突；与数据根划分不一致。
- 证据：设计工作区壳 §窗口、标题栏与单实例；平台设计 §单实例与移交载荷；`design/persistence.md` §数据根与文件（`<local>/state/instance.lock`）。
- 建议修法：统一写 `<local>/state/instance.lock`，并注明路径由 `slterm_settings::paths` 解析。
- 裁决对象：设计修订

### `shell-final-save-active-project` [P3] 关闭链最终事务未写明活跃项目标识

- owner：`docs/devPlan/design/workspace/shell.md`
- 类型：缺口
- 定位：`docs/devPlan/design/workspace/shell.md` §启动与关闭；`docs/featureSpec/workspace/shell.md` §启动与关闭
- 原文：设计“以最终内存状态提交终端簿与项目布局事务” / 规格“保存全部项目数据（含当前完整布局与上次活跃项目标识）”
- 现象：设计最终事务只提终端簿与项目布局，未提 `workspace`/活跃项目 facet；规格要求保存上次活跃项目标识。
- 影响：若活跃项目变更未在阶段 2 冲刷，最终阶段可能不写入，重启恢复到错误项目；持久化验收缺少活跃项目最终保存断言。
- 证据：设计工作区壳 §启动与关闭；工作区壳规格 §启动与关闭；`design/persistence.md` §写入模型与事务（workspace facet 立即提交）、§状态库 schema。
- 建议修法：在最终事务中显式包含 `workspace` facet（至少活跃项目标识），或注明活跃项目变更走立即提交且阶段 2 必然已落盘；补充关闭前切换活跃项目的验收。
- 裁决对象：设计修订

## 交叉引用卡（不定级）

### `notification-override-markdown` 通知覆盖表缺 Markdown 错误 8 秒

- owner 域：设置与交互框架域（`docs/devPlan/design/cross-cutting.md`）
- 证据：`docs/featureSpec/workspace/shell.md` §通知与确认框 规定“编辑器/Markdown 错误 8 秒”；`docs/devPlan/design/cross-cutting.md` §通知类别与时长 的覆盖表只列“编辑器错误 8s”，没有 Markdown。`docs/devPlan/design/workspace/shell.md` §通知与确认框 将时长/覆盖委托给该表。
- 不一致点：Markdown 保存/外部冲突错误会落回默认 5 秒，与工作区规格的 8 秒覆盖不一致。
- 建议：在覆盖表增加“编辑器/Markdown 错误 8s”，或明确 Markdown 错误复用编辑器行；同步验收。

### `settings-close-copy` 设置中心关闭确认正文与工作区通用模板冲突

- owner 域：设置与交互框架域（`docs/featureSpec/panels/settings.md`）
- 证据：`docs/featureSpec/workspace/shell.md` §脏状态与关闭守卫 规定脏面板正文为“面板「<标题>」有未保存的修改，关闭将丢弃这些修改。”；`docs/featureSpec/panels/settings.md` §保存模式与未完成状态 规定设置中心单个关闭正文为“设置中心有未保存的修改，关闭将丢弃这些修改。”。
- 不一致点：同一关闭守卫出现两套正文模板；设置中心按所有者规格实现会偏离工作区壳的通用文案。
- 建议：由工作区壳明确“面板可提供关闭守卫文案覆盖”，或让设置中心使用通用模板；二者只能保留一个。

### `handover-payload-order` 单实例“按载荷顺序处理”与单一 `op` 载荷冲突

- owner 域：架构与服务集成域（`docs/devPlan/design/platform-windows.md`）
- 证据：`docs/devPlan/design/workspace/shell.md` §窗口、标题栏与单实例 写“已有实例按载荷顺序处理：`OpenProject`……`OpenFile`……`NewTerminal`……`Focus` 最后执行前置”；`docs/devPlan/design/platform-windows.md` §单实例与移交载荷 的 `HandoffRequest` 只有单个 `op: HandoffOp`。
- 不一致点：工作区设计暗示一次移交可含多个意图并排序执行；平台载荷只能表达一个意图。`Focus` 是独立 op 还是隐式步骤也未定义。
- 建议：平台明确“一次移交一个 op”，工作区设计删除“按载荷顺序/最后执行”措辞；若需要多意图，改为 `Vec<HandoffOp>` 并定义顺序与失败语义。

### `keyboard-escape-terminal` 终端输入区缺少到活动栏/侧栏的键盘逃逸路径

- owner 域：设置与交互框架域（`docs/devPlan/design/ux-framework.md`）
- 证据：`docs/featureSpec/workspace/shell.md` §键盘与焦点 声明焦点顺序“活动栏 → 侧栏 → 主区页签栏 → 激活面板 → 浮层”且所有控件可键盘到达；`docs/devPlan/design/panels/terminal.md` §输入、选区与剪贴板 规定 `Tab` 在终端上下文优先于组件焦点遍历，`Ctrl+Tab`/`Ctrl+Shift+Tab` 只在活跃项目面板间循环。没有定义从终端输入区进入活动栏/侧栏的按键或命令。
- 不一致点：键盘用户在终端内无法按 `Tab` 离开；`Ctrl+Tab` 只在面板内循环，无法到达活动栏/侧栏；与“核心流程键盘可达”冲突。
- 建议：在焦点/命令所有者定义显式逃逸（如 `Ctrl+Shift+Tab` 到侧栏、专用焦点命令或可重绑命令），并更新工作区壳焦点顺序、终端按键分发与验收；若产品接受终端内仅 `Ctrl+Tab`，需删除“所有控件可键盘到达”的绝对表述。
