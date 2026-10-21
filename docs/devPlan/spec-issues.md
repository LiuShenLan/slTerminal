# 规格问题与裁决

> 目标态重建前的规格冲突、缺口与裁决记录；`docs/featureSpec/` 修订的依据。所有条目已关闭，无待决项。

## 裁决摘要

- 需求真值源仍是 `docs/featureSpec/`；本文件记录为落地目标态而对规格做的修订与设计层裁决。
- 已确认的代码基线：分层重建；新增项目/布局/面板领域 core crates；重建 app 壳与持久化；旧 workspace/tab/pane/dock/session 代码整体废弃。
- 已确认的产品边界：Windows 10/11、单窗口单实例、仅暗色、默认 PowerShell 7、不走 WSL、不迁移旧数据、不保留 Runtime v1 兼容层。
- 已确认的能力取舍：Image 面板、Merge 三栏、用户自定义主题、Markdown 渲染态实时块编辑、提示符跳转删除；vi 模式、终端补全、数学覆盖层、AI 修复条保留为迁移能力；Mermaid 首版不渲染。
- 已确认的存储与计划口径：结构化工作区状态使用本地 SQLite 单库；设置 JSON 留漫游根；机器本地文件统一放本地根；Runtime v2 协议先行、方法增量；旧壳在垂直切片后一次性退役；会话领域归 `slterm_agents`；另存为授权不跨重启；主题不做任何对比度判断；契约按单一所有者收敛；主线开发不逐里程碑开分支。

## 一、用户裁决

| 主题 | 裁决 |
|---|---|
| 核心 crate 拓扑 | 领域分核：新增 `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；Git/文件服务留在 app。 |
| Runtime v2 资源 ID | 类型前缀 + 单调 u64：`project:7`、`group:12`、`panel:31`；内部 u64，外部字符串。 |
| 新状态根 | 双根：漫游根 `<data>/state/settings.json` 只存应用设置；本地根 `<local>/state/` 存结构化状态库 `state.db` 与机器本地文件（runtime.port、instance.lock、window.json、recovery、quarantine、completion、usage、hooks、update、backup）。 |
| 状态存储底座 | 项目/布局/终端簿/侧栏/视图状态使用单一 SQLite 库（WAL、单写线程、事务）；设置 JSON 继续经 `settings_store`；崩溃快照仍为文件；不保留逐文件 generation、跨文件提交顺序与孤儿目录对账。 |
| 机器本地文件 | 实例锁、Runtime 端点、窗口几何、崩溃快照、隔离区、补全数据、更新状态与日志/审计/崩溃现场统一放本地根；项目根路径是机器相关事实，工作区状态不随漫游配置跨机复制。 |
| 单实例通道 | 复用 Runtime v2 本机端点；`slterm_runtime_protocol` 在领域骨架期先行，壳实现 `runtime.handover` 接收端；完整 v2 服务器与方法按域增量交付。 |
| 根重定位脏面板 | 转脱离缓冲：保留文本/脏/撤销/光标，禁用原地保存，提供另存为，关闭走守卫。 |
| 跨类型重命名 | 脏缓冲或原形态为源码时保持源码态，否则渲染态；目标已打开时显式确认替换，目标有脏修改时二次确认保存/丢弃。 |
| 无项目设置浮层 | 仅应用页 + 导入/导出/恢复所有默认；范围外节显示只读占位；界面语言按迁移能力，featureSpec 不承诺多语言 UI。 |
| 无障碍 | 删除无障碍专用能力与现有实现（屏幕阅读器/ARIA/读屏播报/WCAG 对比度校验/高对比度/大字体适配）；保留键盘、焦点、IME/CJK 作为普通交互；保留“减少动效”。 |
| 主题可读性 | 不做任何对比度判断：不校验、不提示、不自动回退、不静默改写；内置暗色主题按设计保证可读，透明度/壁纸是用户显式选择。 |
| 崩溃快照配额 | 单文件 64 MiB、总配额 256 MiB、保留 7 天。 |
| 旧模型处置 | 垂直切片通过后一次性退役旧 workspace/tab/pane/dock/session 与旧测试，移除 `legacy-shell` feature 与 allowlist；保留已验证引擎；不再逐域增量删除。 |
| 会话领域归属 | 会话扫描、元数据、词法归属、删除账本与恢复计划统一归 `slterm_agents::sessions`；`slterm_workspace` 只提供项目根索引；导航树消费 agents 投影。 |
| 另存为授权 | `PathGrant::UserSelected` 只绑定当前文档与当前运行期；不写入布局；重启后根外路径按根外只读重新判定。 |
| 凭据 target | 统一小写 `slterm/<scope>/<id>`；AI Provider 的 id 为稳定 provider ID；后端实现保留现有 `CredentialStore`，对外契约类型为 `SecretStore`/`SecretValue`。 |
| 备份类别 | 删除独立 `ai_providers` 类别；Provider 元数据随 `app_settings` 导出与敏感键剥除；凭据值永不入包。 |
| 文档所有权 | 每类契约只留一个所有者；README 增补“契约所有者索引”；删除重复定义与重复“依赖里程碑”列表；计划依赖表是唯一依赖来源。 |
| 分支策略 | 主线开发 + `checkpoint/<ascii-slug>` 标签；仅并行工作流使用短生命周期分支；不使用逐里程碑分支。 |
| 外部文件拖入 | 不支持；仅保留文件浏览器内部拖到编辑器面板组。 |
| 单实例目标不存在 | 不自动创建项目；提示用户先在导航树添加或匹配现有项目。 |
| 托盘关窗 | 关闭即退出；托盘只做注意力与快捷入口。 |
| 应用页形态 | 单页分节；Runtime API 只读状态不提供总开关；通知仅总开关 + 任务栏闪烁开关。 |
| 历史删除撤销 | 移入应用数据隔离区并保存索引快照；撤销移回原路径，跨卷复制后删除。 |
| 自定义 Agent 根 | 拒绝盘符根、`%WINDIR%`、`System32`；普通绝对目录允许。 |
| 语法高亮 | 统一 Tree-sitter，每语言一个 grammar crate；依赖在应用层登记。 |
| Mermaid | 首版不渲染，仅按代码块显示源码；保留渲染接口与扩展点。 |
| Runtime 控制深度 | 仅生命周期/聚焦/终端/Agent；编辑器内容操作不进入 v2。 |
| 编辑器 AI | 只保留共享 Provider/AI Gateway 与扩展枚举，不注册编辑器命令。 |
| 数据目录 | 应用设置放 `%APPDATA%\slterm\state\settings.json`；工作区状态库与布局/项目数据、窗口几何、崩溃快照、隔离区、补全数据、更新状态、日志/审计/崩溃现场放 `%LOCALAPPDATA%\slterm`；应用页诊断节提供一键清理按钮（二次确认）。 |
| 代码签名 | 有证书则签名安装器与二进制并在 handoff 前校验；无证书时登记残余风险，摘要校验不放宽。 |
| 其余设计裁决 | 按本文件第三节采纳。 |
| 规格修订方式 | 直接改写 featureSpec，包括删除无障碍章节与 Mermaid 渲染要求。 |

## 二、featureSpec 修订矩阵

### `README.md`

- 跨域不变量与关键跨域决策：项目身份改为稳定 `ProjectId`；根路径唯一只是不变量。
- 术语表与文档地图：设置中心增加“应用”页；Markdown 条目改为“Mermaid 首版按源码显示”；删除无障碍描述；数据根改为“设置漫游、工作区状态本地”；主题无可读性提示；另存为授权不跨重启。
- 明确不做：增加“Mermaid 首版不渲染”“外部文件拖入不支持”“无项目时应用设置浮层”等边界；把范围外能力与“明确不做”分开表述。

### `workspace/shell.md`

- 模型与不变量：新增 `ProjectId`；根路径唯一是不变量，不是身份。
- 项目生命周期：创建/重定位保留 `ProjectId`；根重定位对文件型面板的迁移规则。
- 设置中心入口：无项目时打开仅含应用页的全局设置浮层；有项目时仍为项目内设置中心。
- 关闭链：受守卫文件范围、三阶段时限、永久失败只提供放弃/取消、PTY/最终项目保存失败只记录。
- 单实例：第二次启动可携带 `Focus/OpenProject/OpenFile/NewTerminal`；目标不存在不自动创建。
- 窗口：默认尺寸/最小尺寸；窗口几何持久化列为范围外迁移，不在规格承诺。
- 无障碍基线：删除；保留键盘、焦点、IME、减少动效作为普通交互条款。
- 验收：删除无障碍行；增加应用页浮层、ProjectId、关闭链分类、单实例载荷。

### `workspace/navigation.md`

- 身份与不变量：`ProjectId` 是身份；根路径词法唯一 + 真实路径查重。
- 根目录重定位：文件型面板、Hooks、Diff/GitShow、终端 cwd、崩溃快照的迁移规则。
- 视图状态：项目级状态以 `ProjectId` 为键，路径存相对形式。
- 无障碍与国际化：删除屏幕阅读器/播报/高对比度/大字体内容；保留 IME/CJK 与键盘操作。
- 验收：同步身份、重定位、状态作用域；删除读屏断言。

### `sidebar/framework.md`

- 配置钮：无项目时打开应用设置浮层；激活态规则不变。
- 视图状态：逐视图、逐字段作用域矩阵；应用级与项目级分列；存储位置为状态库 `view_states`。
- 无障碍：删除读屏/ARIA/播报/强制色/大字体内容；保留键盘操作与焦点。
- 验收：增加无项目浮层、视图状态作用域；删除无障碍行。

### `sidebar/explorer.md`

- 状态持久化：按 `ProjectId` 键，路径存相对形式。
- 树内剪贴板：应用级内部剪贴板；复制跨视图允许；剪切仅 Agent↔Agent 或同项目根内；跨边界拒绝。
- 外部文件拖入：不支持；拖入无动作或一次性提示。
- 无障碍与国际化：删除读屏/播报/高对比度内容；保留 IME/CJK 与键盘。
- 验收：同步 ProjectId、剪贴板边界、外部拖入；删除无障碍行。

### `sidebar/commit.md`

- 仓库范围 vs 项目根：明确为有意不对称；根外条目只读、不可定位/回滚/删除/在终端打开。
- 打开映射：删除行 gutter 入口、Diff 同键来源规则。
- 状态作用域：分组折叠/ignored 开关为应用级；列表/过滤/聚焦为瞬态。
- 无障碍：删除读屏/播报内容；保留键盘。
- 验收：同步根外边界、状态作用域；删除无障碍行。

### `sidebar/agent-files.md`

- 自定义节点：拒绝盘符根、`%WINDIR%`、`System32`；同目录多节点按规范化真实路径去重批量/破坏性操作。
- 标题：保留最短可区分路径 + Agent 显示名。
- 状态作用域：Agent 展开集合与每 Agent 目录展开集合为应用级持久化。
- 无障碍与国际化：删除读屏/播报内容；保留 IME/CJK 与键盘。
- 验收：同步系统目录限制、同目录去重；删除无障碍行。

### `panels/terminal.md`

- 范围外增强边界：vi、终端补全、数学覆盖层、AI 修复条保留为迁移能力；提示符跳转删除；OSC 9;4 任务栏进度为迁移能力。
- 进程生命周期：PTY 上限 64；全局创建顺序（项目顺序 → 布局 DFS → 页签顺序）；并发上限 8；重连/重试同一原子名额检查。
- 链接策略：OSC 8 仅 http/https/mailto；不支持 tel/本地路径。
- 无障碍与输入法：删除读屏/高对比度/大字体内容；保留 IME/CJK 与键盘。
- 验收：同步上限/重连/链接/增强边界；删除无障碍行。

### `panels/editor.md`

- 语法高亮：统一 Tree-sitter；高亮映射与 Diff/GitShow 共用。
- 跨类型重命名：默认形态保持源码（脏或原为源码），目标已打开显式确认替换。
- 保存冲突与关闭守卫：关闭时首次保存遇冲突直接进保存冲突对话框；批量遇冲突中止并保留全部面板。
- Git 行标：新增/修改/删除标记均可点击打开 Diff；删除标记定位到删除块。
- 崩溃恢复：配额 64 MiB / 256 MiB / 7 天；键 `(ProjectId, PanelId)`；聚合恢复提示。
- 另存为：授权只绑定当前文档与当前运行期，不写入布局；重启后根外路径按根外只读重新判定。
- 外部文件拖入：不支持。
- 无障碍：删除读屏/大字体/高对比度内容；保留 IME/CJK 与键盘。
- 验收：同步高亮、跨类型、冲突、gutter、快照；删除无障碍行。

### `panels/markdown.md`

- Mermaid：首版不渲染，按代码块显示源码；保留渲染接口；删除七类图与静态矢量要求。
- 源码态能力白名单：继承编辑/撤销/查找替换/换行/字号/编码/行尾/外部修改/保存冲突/只读分级；不继承语言选择/折叠/Git 标记/HTML 按钮/自动保存。
- 崩溃恢复：与编辑器共用 `RecoveryStore` 与配额；聚合恢复提示。
- 无障碍：删除读屏/大字体/高对比度内容；保留 IME/CJK 与键盘。
- 验收：Mermaid 改为源码显示；删除无障碍行；同步快照与源码态白名单。

### `panels/diff.md`

- 同键来源：已有干净面板按来源刷新；已有脏面板不静默覆盖，提供显式替换/重新加载。
- 删除行 gutter：可点击打开并定位删除块。
- 根重定位：按新仓库根重新解析；根外/移出仓库状态保留面板。
- 无障碍：删除读屏/高对比度内容；保留键盘。
- 验收：同步同键来源、删除标记、重定位；删除无障碍行。

### `panels/gitshow.md`

- 临时文件：唯一临时文件、保留扩展名、仅当前用户、启动清理过期；日志只记操作与结果。
- 监听失效：仅窗口聚焦/面板激活复核 HEAD 指纹，不轮询。
- 无障碍：删除读屏/大字体/高对比度内容；保留键盘。
- 验收：同步临时文件与过期检测；删除无障碍行。

### `panels/settings.md`

- 全局组顺序：快捷键 → 后台定时任务 → 外观 → 终端 → 应用。
- 应用页：单页分节；界面语言、更新、托盘与开机启动、通知、Runtime API（只读状态）、AI Providers/助手、诊断、备份与恢复、终端增强开关；范围外节在迁移前显示只读占位；诊断节提供一键清理日志/审计/更新下载/崩溃现场（二次确认）。
- 无项目浮层：仅应用页 + 导入/导出/恢复所有默认。
- 外观页：删除 WCAG 校验与可读性提示；保留仅暗色、透明度/壁纸/密度；窗口材质归应用或外观（设计定稿）。
- 终端页：显式 Shell 列表固定 pwsh/powershell/cmd + 自定义路径。
- Agent 基础页：自定义节点拒绝系统目录。
- 快捷键页：分“可重绑命令”与“固定键参考”；固定键不进入录制与冲突检测。
- 导入导出：侧栏布局/视图状态不属于应用设置域；恢复所有默认不触碰。
- 无障碍：删除读屏/高对比度/大字体内容；保留键盘。
- 验收：增加应用页、浮层、清理按钮、固定键参考；删除无障碍行。

### `settings-and-persistence.md`

- 数据根：应用设置在漫游根 `<data>/state/settings.json`；工作区状态库与布局/项目数据、窗口几何、恢复快照、隔离区、补全、更新状态、日志/审计/崩溃现场在本地根 `<local>`。
- 备份分层：设置 JSON 保留上一代备份；工作区状态以单库事务提交，正常关闭生成可恢复备份。
- 项目身份：持久化键使用 `ProjectId`；根路径唯一是不变量。
- 关闭冲刷：受守卫 facet 范围；永久失败只提供放弃/取消。
- 崩溃快照：64 MiB / 256 MiB / 7 天；键 `(ProjectId, PanelId)`；项目移除清理；损坏隔离。
- 写入根：业务写入 vs 应用托管写入；系统另存为与 Hooks 为显式例外。
- 数据形态：不迁移旧 `settings.json`/`session.json`；旧文件不读不删；新状态根隔离。
- 验收：同步路径、配额、关闭分类、写入根；删除无障碍相关。

### `cross-cutting.md`

- 删除 `### 无障碍基线` 整节及关键决策中的无障碍条目；保留键盘、焦点、IME/CJK、减少动效作为普通交互条款。
- 链接策略按来源分表：终端 OSC 8 仅 http/https/mailto；Markdown 另含 tel/本地路径；共同底线是拒绝 javascript/data/blob/file 等。
- 写入根：业务写入（项目根/Agent 根/另存为/回收站）与应用托管写入（状态根/系统临时目录/Hooks）分表。
- 通知：统一通知服务；默认 3/4/5 秒；视图可覆盖；面板内横幅不自动消失。
- 错误与反馈：保留层级；删除无障碍播报要求。
- 验收：删除无障碍行；增加链接分表、写入根、通知覆盖。

### `acceptance.md`

- 删除跨域验收“无障碍基线”行与不可自动化面中的无障碍相关项。
- 明确不做：增加“Mermaid 首版不渲染”“外部文件拖入不支持”“无障碍专用能力不提供”；把范围外迁移能力单列。
- 增加应用页/无项目浮层、ProjectId、快照配额、关闭冲刷分类、单实例载荷的验收。
- 修正 Mermaid 行为为源码显示；修正任务栏/终端增强的范围边界。

## 三、设计层采纳（不修订需求）

| 领域 | 采纳结论 |
|---|---|
| 持久化 | 本地 SQLite 单库 + 单写线程 + 事务；脏 facet 合并写与去抖；`PRAGMA user_version` 精确匹配、`integrity_check` 失败隔离整库；设置 JSON 保留 `.bak` 与损坏隔离；崩溃快照仍为文件并保留配额；旧文件不读不删。 |
| 领域模型 | `ProjectId`/`PanelGroupId`/`PanelId` 全局唯一；布局树叶为 `PanelGroupId`；持久化 DTO 与运行时实例分离；`PanelRecord.params` 唯一权威；去重键与标题纯函数；会话领域归 `slterm_agents`；`PersistedPath` 不携带另存为授权。 |
| GPUI 运行时 | `Workspace → ProjectView → PanelGroupView → PanelHost → PanelBody`；隐藏项目不布局/不绘制；`catch_unwind` 面板错误边界；一次性焦点意图；关闭状态机；面板能力与任务句柄统一由 `PanelHost` 与 `TaskService` 表达。 |
| 命令与焦点 | 分层派发表：临时面 > 系统键 > 全局导航命令 > 固定键 > 可重绑面板命令 > 未处理按键；焦点循环 DFS；侧栏快捷键优先于终端输入。 |
| 设置中心 | 页面注册表；三保存模式；应用页单页分节；无项目浮层；固定键参考；导入导出/恢复默认作用域；Hooks 与 Agent 页联动。 |
| 侧栏 | 应用级双槽 + 按钮组织；视图状态逐字段作用域；应用级内部文件剪贴板；Explorer/Commit/Agent 的 ProjectId 键与相对路径；导航会话投影依赖 `slterm_agents`。 |
| 终端 | 复用 `slterm_terminal`；项目级编号/标题/cwd；PTY 上限 64、并发 8、全局顺序；OSC 8/52 策略；终端增强迁移扩展点。 |
| 编辑器 | 复用文档内核；Tree-sitter；打开路由与去重；保存冲突状态机；崩溃快照；gutter 三类标记；HTML 源码与系统打开。 |
| Markdown | 两态 + 内存草稿；CommonMark/GFM；安全 HTML 子集；公式；Mermaid 源码显示 + 保留接口；链接/资源白名单；草稿快照。 |
| Diff/GitShow | HEAD↔工作区、并排/行内、工作区侧可编辑、冲突快照；HEAD 只读、刷新/过期、二进制临时文件。 |
| 通知 | 统一通知服务；默认 3/4/5 秒；视图覆盖 2/5/8 秒；系统通知仅窗口失焦；事件键去重 10 秒。 |
| 平台 | 单实例 Runtime v2 端点；移交载荷以根路径定位、目标不存在只提示；窗口几何矩形+最大化+显示器+DPI，落本地根；托盘关闭即退出；回收站/默认程序/进程树；日志分根。 |
| 安全 | 凭据只存 Windows 凭据管理器；target 统一 `slterm/<scope>/<id>`；路径允许根与真实路径校验；链接白名单；Markdown 清洗；诊断脱敏；审计只记操作/路径/结果/指纹；另存为授权不持久化。 |
| 迁移 | 旧模块保留/改造/删除/新建表；新旧不并行；旧数据不迁移；垂直切片后旧壳一次性退役；Runtime v2 协议先行、方法增量；范围外能力按应用页/终端/备份/更新分阶段迁移。 |
| Runtime v2 | `slterm_runtime_protocol` 在领域骨架期建立；壳实现最小端点与 `runtime.handover`；垂直切片后交付 project/panel/terminal 方法与快照；git/agent/orchestrate 随域增量；收敛门后完整 conformance、CLI 与随包 skill。 |

## 四、删除与收敛

| 能力 | 处置 |
|---|---|
| Image 面板 | 删除；不进入目标面板封闭集；二进制/图片走系统默认程序。 |
| Merge 三栏 | 删除；Git 冲突处理由终端承担；冲突差异由 Diff 只读快照承接。 |
| 用户自定义主题 | 删除；仅保留内置暗色主题；主题编辑器与导入包移除。 |
| Markdown 渲染态实时块编辑 | 删除；渲染态只读，编辑统一进源码态。 |
| 提示符跳转 | 删除；与“命令块”边界冲突。 |
| Mermaid 渲染 | 首版删除；围栏代码块按源码显示；保留渲染接口。 |
| 无障碍专用能力 | 删除屏幕阅读器/ARIA/读屏播报/WCAG 校验/高对比度/大字体适配及现有实现；保留键盘、焦点、IME/CJK、减少动效。 |
| 外部文件拖入 | 不支持；仅文件浏览器内部拖到编辑器面板组。 |
| Runtime v1 | 删除；v2 不保留兼容层。 |
| 旧数据迁移 | 删除；新状态根从默认值开始；旧文件不读不删。 |

## 五、优化轮裁决（施工图收敛）

| 主题 | 裁决 |
|---|---|
| 文档所有权 | 每个契约只保留一个所有者：协议归 `design/integrations/runtime-api-v2.md`；应用页归 `design/settings/app-page.md`；窗口/单实例/托盘/系统通知归 `design/platform-windows.md`；状态库/侧栏与状态 schema 归 `design/persistence.md`；焦点/命令归 `design/ux-framework.md`；通知/错误/任务服务归 `design/cross-cutting.md`；ID/面板参数/路径归 `design/domain-model.md`；主题/语言归 `design/theme-and-i18n.md`；诊断归 `design/diagnostics.md`；会话领域归 `design/integrations/agent-cli.md`。`design/out-of-scope/` 只保留迁移增量，不复制契约；README 维护“契约所有者索引”，重复定义改为引用。 |
| 应用页范围 | 规格范围内交付页壳、全局组顺序、节序、自动保存、通知开关、诊断与 Runtime API 只读状态；控制面协议核心交付后显示运行状态/端点/协议版本，完整方法随 Runtime v2 完整面补齐；界面语言、更新、托盘与开机启动、AI Providers/助手、备份与恢复、终端增强为只读占位或禁用态。诊断动作合并为版本/构建、数据/日志目录、配置健康、复制诊断信息、打开目录/日志、通知自检、一键清理；清理范围不含恢复快照。无项目浮层补导入/导出/恢复所有默认，项目激活后自动关闭。 |
| 无障碍与对比度 | 删除 WCAG/对比度准入校验、自动回退与可读性提示；不校验、不提示、不静默改写用户设置。删除屏幕阅读器/角色/实时播报/200% 文本缩放等专用条款与现有对比度实现；保留键盘、焦点环、IME/CJK、减少动效与 24×24 命中区。 |
| 旧模型处置 | 垂直切片通过后一次性删除旧 `workspace`/`tab`/`pane`/`dock`/`session` 与旧测试，移除 `legacy-shell` feature 与 `check_legacy_symbols` allowlist；保留已验证引擎；迁移映射与旧测试替换在退役里程碑一次完成。单实例锁统一 `instance.lock`，归壳与工作区。 |
| 计划结构 | 依赖关系以“里程碑全名 → 直接依赖”表为唯一来源，删除树形依赖图与各迁移文档的重复依赖列表；每个里程碑增加退出条件；新增 Hooks 引擎、诊断与日志、构建与打包基线、状态库与设置存储、旧壳退役、控制面协议核心等里程碑；垂直切片扩展为多项目切换、隐藏项目 PTY 存活、设置/布局持久化、崩溃快照、焦点恢复与关闭守卫。 |
| Runtime v2 | 以 `design/integrations/runtime-api-v2.md` 为唯一契约源；协议先行、方法增量：协议 crate 与信封在领域骨架期建立，壳实现最小端点与 `runtime.handover`，垂直切片后交付 project/panel/terminal 方法，git/agent/orchestrate 随域增量，收敛门后完整 conformance；ID 为 `project:<n>`/`group:<n>`/`panel:<n>`；环境变量 `SLTERM_PANEL_GROUP_ID`；端口文件 `slterm.runtime/2 <port> <token> <pid>`；写命令用 `mutation_id` + 可选 `expected_revision`；错误码 `idempotency_conflict`/`revision_conflict`；不提供 `window.*` 方法；未知资源 `resource_not_found`；快照面板 DTO 不暴露原始参数；冲突快照不入快照与布局；`agent.delegate` 入方法表。移交载荷以根路径定位，目标不存在只提示、不创建项目。 |
| 窗口几何与静默启动 | 窗口几何保持范围外迁移能力，`design/platform-windows.md` 为所有者，迁移后使用本地根 `<local>/state/window.json`；规格内关闭链不冲刷几何。首版不做静默启动；删除 `app.startup.silent` 与 `launch_at_login` 持久键，开机启动状态只读 Startup 快捷方式。窗口最小 760×540，内容最小宽 406px。 |
| 交互语义 | 焦点意图与协调归 `design/ux-framework.md`，`FocusMemory` 只归 `ProjectView`；异步打开在项目活跃、面板仍为组内激活面板且无更新焦点意图时消费；`Ctrl+Tab` 从活动栏/侧栏/主区均可进入面板循环。通知归 `design/cross-cutting.md`，撤销用可操作通知；事件键 `(panel_id, kind, session_id, turn_key)`；`taskbar_flash` 独立门控闪烁。Diff 去重键含 `mode/source`；冲突快照为瞬态 Diff 面板，不持久化、不参与去重。Agent 根新增显式可写作用域，编辑器可保存；保存链原子替换前经 `PathGuard` 复核；编辑器与 Diff 统一保存前后版本校验。 |
| 小口径收敛 | `desktop_enabled` 只管系统 toast；删除 `app.diagnostics.debug_log`；终端增强默认 vi 关、补全开、数学开、AI 修复关，应用页补 `completion.style`；AI 助手只存 provider ID；补全数据路径统一本地根 `<local>/state/completion/*`；`PersistedPath.read_only` 是唯一只读字段，另存为授权不持久化；设置面板 `page_params` 持久化；语言枚举统一 `LanguagePreference::{System, English, Chinese}`；通知时长/去重/缓存单源，通知去重键命名 `NotificationDedupeKey`；任务令牌统一 `TaskToken { id, scope, generation }`；侧栏 schema 以 `persistence.md` 为准；关闭状态机以 `architecture.md` 为准；项目移除先提交状态库事务；ID 游标 `max(next_*, maxSeen+1, maxDir+1)`；`build-and-release.md` 明确命令/持久化/i18n 留在 app；`slterm_settings::paths` 增加状态库、本地根与本地日志根访问器；备份类别删除 `ai_providers`；凭据 target 统一 `slterm/<scope>/<id>`。 |

## 六、目标态优化裁决

| 主题 | 裁决 | 原因 | 边界 |
|---|---|---|---|
| 状态存储 | 结构化状态使用单一 SQLite 库（WAL、单写线程、事务）；设置 JSON 保留；崩溃快照保留文件 | 消除跨文件提交顺序、孤儿目录、逐文件 generation 与恢复对账 | DB 放本地根；`PRAGMA user_version` 精确匹配；`integrity_check` 失败隔离整库 |
| 数据根划分 | 设置漫游；工作区状态、窗口几何、崩溃快照、隔离区、补全、更新、日志/审计/崩溃现场本地 | 项目根与窗口是机器相关事实；漫游 profile 不应携带锁、token 与大快照 | `SLTERM_CONFIG_DIR` 覆盖漫游根，`SLTERM_LOCAL_DIR` 覆盖本地根 |
| Runtime 节奏 | 协议先行、方法增量；壳只实现最小端点与 `runtime.handover`；垂直切片后交付 project/panel/terminal；git/agent/orchestrate 随域；收敛门后完整 conformance | 消除壳退出条件与控制面空窗；让领域模型尽早被 AI CLI 消费方验证 | 协议 v2 版本冻结；只追加方法，不改旧语义 |
| 旧壳退役 | 垂直切片通过后一次性删除旧 workspace/tab/pane/dock/session 与旧测试；移除 `legacy-shell` 与 allowlist | 双壳并存只增加编译面、误引用与门禁维护 | 保留已验证引擎；迁移映射在退役里程碑一次完成 |
| 会话归属 | 会话扫描、元数据、归属、删除账本、恢复计划归 `slterm_agents::sessions` | 消除 workspace 与 agents 两套 `SessionRecord`/来源抽象 | `slterm_workspace` 只提供项目根索引；sidebar 依赖 agents |
| 另存为授权 | 不持久化；重启后根外路径按根外只读重新判定 | 根外写权限不应因一次布局持久化长期存在 | 当前运行期仍可编辑；再次另存为可重新授权 |
| 主题可读性 | 不做任何对比度判断，不提示 | 无判定标准的提示会变成实现分歧；产品只提供内置暗色主题 | 用户显式选择透明度/壁纸即接受结果；不自动回退 |
| 契约收敛 | 单一所有者 + README 契约索引；删除重复定义/依赖列表/快照数字；统一类型与字段名 | 施工图重复会直接复制进代码 | 凭据 target `slterm/<scope>/<id>`；删除 `ai_providers` 备份类别 |
| 分支策略 | 主线 + `checkpoint/<ascii-slug>` 标签；仅并行工作流用短分支 | 单开发者/AI 顺序重建，逐里程碑分支只增加合并成本 | 并行组仍可隔离；checkpoint 是回滚点 |

## 七、验证

- 本文件每条裁决在 featureSpec 对应篇目有落点；设计文档引用本文件而非重复裁决。
- `python scripts/check_doc_policy.py full` 零 finding；全部相对链接可解析；表格列数一致。
- 规格中不再出现“无障碍基线/读屏/ARIA/WCAG/高对比度/大字体”等已删除承诺；“减少动效/IME/CJK/键盘”保留。
- 规格中 Mermaid 仅以“源码显示 + 保留接口”出现；不再要求七类静态矢量图。
- 追踪矩阵覆盖全部 featureSpec 篇目 → 设计文档 → 里程碑 → 验收判据 → 测试层，无遗漏；不写篇目计数。
- 设计文档不再出现重复的 ID/焦点/窗口/通知/侧栏/任务/凭据定义；重复内容改为引用所有者文档。
- 规格与设计不再出现对比度准入、可读性提示、屏幕阅读器、ARIA、读屏播报、高对比度、大字体适配等已删除承诺。
- 单实例移交目标不存在时只提示、不创建项目；状态库与机器本地文件不落在漫游根。
