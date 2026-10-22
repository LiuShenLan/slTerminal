# slTerminal 目标态开发计划与开发设计

> 目标态重建的施工图：计划、架构、按域详细设计、迁移与测试；需求真值源是 `docs/featureSpec/`，裁决记录在 `spec-issues.md`。

## 定位

- `docs/featureSpec/` 定义“做什么”：目标态功能需求、验收判据与明确不做。
- `docs/devPlan/` 定义“怎么建、按什么顺序、怎么验收”：领域模型、模块拓扑、存储 schema、状态机、迁移映射与命名里程碑。
- 两者冲突时以 featureSpec 为需求真值源；若设计发现规格缺口，先更新 `spec-issues.md` 并修订 featureSpec，再改设计。
- 本目录是重建期工作文档，允许随实现演进；完成后由代码、`.claude/docs/` 与各路径 `CLAUDE.md` 承接长期决策。
- 契约所有权：每类契约只在一个文档定义，其他文档引用；`design/out-of-scope/` 只保留删除项、落点、迁移顺序、增量验证与不可自动化面，不定义目标态契约；目标态契约归实际所有者篇，并在下方索引登记。

## 文档地图

| 文档 | 内容 |
|---|---|
| `spec-issues.md` | 规格冲突、缺口、用户裁决、featureSpec 修订矩阵、设计层采纳项 |
| `plan.md` | 命名里程碑、依赖图、交付物、验收映射、风险、cutover |
| `design/architecture.md` | crate/模块拓扑、状态所有权、线程模型、启动关闭、旧状态模型退役、双数据根 |
| `design/domain-model.md` | 项目/布局/面板/终端簿、稳定 ID、DTO、恢复对账、不变量、Runtime ID 合同 |
| `design/gpui-runtime.md` | Entity 拓扑、订阅、异步取消、隐藏项目、面板错误边界、测试接缝 |
| `design/ux-framework.md` | 命令/动作注册、快捷键、派发优先级、焦点协调、覆盖层、IME；焦点与命令唯一 owner |
| `design/persistence.md` | 状态库 schema、事务、去抖、损坏隔离、备份分层、关闭冲刷、崩溃快照；侧栏与状态 schema 唯一 owner |
| `design/cross-cutting.md` | 错误、通知、性能、后台任务索引；通知与错误唯一 owner |
| `design/security.md` | 路径、URL/OSC、Markdown 清洗、进程、凭据、威胁模型与测试矩阵 |
| `design/platform-windows.md` | 单实例、窗口几何/DPI、系统通知、托盘、开机启动、回收站、进程树；窗口/实例/托盘唯一 owner |
| `design/theme-and-i18n.md` | 语义 token、内置暗色主题、密度/透明度/壁纸/材质、i18n 合同；主题与语言唯一 owner |
| `design/diagnostics.md` | 日志、审计、panic、诊断包、脱敏、清理；应用页诊断动作唯一 owner |
| `design/build-and-release.md` | 新 crate 布局、包名、安装器、更新交接、门禁与体积预算 |
| `design/testing.md` | 测试分层、夹具、验收追溯、旧测试处置、豁免、门禁 |
| `design/migration.md` | 旧模块保留/改造/删除/新建、旧状态模型退役、范围外迁移总表 |
| `design/services/filesystem-git.md` | 路径权威、监听、Git 执行体、status/HEAD 缓存、写门 |
| `design/integrations/agent-cli.md` | AgentKind、hook 摄取、会话扫描、余量、删除隔离区 |
| `design/integrations/hooks.md` | Hooks 事件/handler 目录、schema、注入/卸载、托管条目 |
| `design/integrations/runtime-api-v2.md` | Runtime v2 唯一协议契约源：资源模型、ID 线缆格式、传输、鉴权、命令、幂等 |
| `design/workspace/shell.md` | 项目生命周期、布局、焦点、关闭守卫、单实例、无项目浮层入口 |
| `design/workspace/navigation.md` | 项目/活跃/历史会话、搜索、刷新、删除撤销、套餐余量 |
| `design/sidebar/framework.md` | 活动栏、双槽、视图注册、显隐、拖拽、宽度与比例；状态 schema 引用 persistence |
| `design/sidebar/explorer.md` | 文件树、多选、文件操作、过滤、忽略、Git 状态 |
| `design/sidebar/commit.md` | Commit 四态、分组、ignored、打开映射、回滚/删除 |
| `design/sidebar/agent-files.md` | Agent 节点、统一树、过滤消费、监听、标题消歧 |
| `design/panels/terminal.md` | PTY、渲染、输入、链接、剪贴板、编号、恢复 |
| `design/panels/editor.md` | 打开路由、编辑、Tree-sitter、保存、查找、折叠、外部修改、Agent 根可写 |
| `design/panels/markdown.md` | 两态、渲染管线、链接/资源、安全 HTML、公式、Mermaid 源码 |
| `design/panels/diff.md` | HEAD↔工作区、并排/行内、冲突快照、同步滚动 |
| `design/panels/gitshow.md` | HEAD 只读、刷新/过期、大文件/二进制 |
| `design/panels/settings.md` | 页面注册、导航、保存模式、Hooks、应用页入口；`SettingsSurface` 唯一定义 |
| `design/settings/app-page.md` | 应用页唯一契约源：节结构、键、默认值、保存、浮层、节级迁移矩阵 |
| `design/out-of-scope/*` | 只写删除项、落点、迁移顺序、增量验证与不可自动化面；目标态契约见契约所有者索引 |

## 关键裁决

- 代码基线：分层重建；新增 core crate `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；保留 `slterm_terminal`、`slterm_settings`、`slterm_split`、`slterm_completions`、`slterm_hook`。
- 文档所有权：每类契约只在一个文档定义；`out-of-scope/` 只写迁移增量，目标态契约归实际所有者并登记在下方索引；重复内容必须改成引用。
- 状态存储：项目/布局/终端簿/侧栏/视图状态使用本地 SQLite 单库；应用设置保留 `settings.json`；崩溃快照保留文件。
- 数据根：设置漫游 `<data>/state/settings.json`；状态库与机器本地文件在本地根 `<local>`（`state.db`、`runtime.port`、`instance.lock`、`window.json`、`recovery/`、`quarantine/`、`completion/`、`usage/`、`hooks/`、`update/`、`backup/`、日志/审计/崩溃现场）。
- 项目身份：稳定 `ProjectId`；根路径词法唯一并做真实路径查重。
- 会话领域：扫描、元数据、归属、删除账本与恢复计划归 `slterm_agents::sessions`；工作区只提供项目根索引。
- Runtime v2：协议先行、方法增量；资源 ID `project:<n>` / `group:<n>` / `panel:<n>`；环境变量 `SLTERM_PANEL_GROUP_ID`；端口文件 `slterm.runtime/2 <port> <token> <pid>`；写命令 `mutation_id` + 可选 `expected_revision`；移交 `runtime.handover`；不保留 v1 兼容层。
- 设置中心：每项目一个面板；新增应用页；无项目时打开仅含应用页的全局设置浮层。
- 应用页范围：规格内交付页壳、节序、自动保存、通知开关、诊断（含导出诊断包与环境自检）、Runtime 只读状态；界面语言、更新、托盘与开机启动由范围外节里程碑接线；Providers/助手、备份恢复、终端增强在各自迁移里程碑前保持占位或禁用。
- 公开发布：Runtime API v2、更新链与加密备份完成前只发内部构建；AI Providers/助手与终端增强不阻塞首次公开发布，对应应用页节保持禁用或占位并明确后续交付。
- 能力取舍：Image、Merge、用户自定义主题、Markdown 渲染态实时块编辑、提示符跳转删除；vi/终端补全/数学覆盖层/AI 修复条保留为迁移能力；Mermaid 首版按源码显示。
- 无障碍与主题：不提供屏幕阅读器/ARIA/读屏播报/WCAG 校验/高对比度/大字体适配；不做对比度校验、可读性提示或自动回退；保留键盘、焦点、IME/CJK、减少动效。
- 旧模型：垂直切片通过后先退役旧状态模型与写路径；旧 UI 宿主按域在替代实现验收后删除；每步独立验收；移除 `legacy-shell` 与 allowlist；保留已验证引擎。
- 另存为：授权只绑定当前文档与当前运行期，不写入布局，不跨重启。
- 窗口几何：范围外迁移能力；`platform-windows.md` 所有；迁移后写本地根 `window.json`；首版不做静默启动。
- 数据迁移：不迁移旧 `settings.json`/`session.json`；旧文件不读不删；新数据根隔离。
- 分支策略：主线 + `checkpoint/<ascii-slug>` 标签；仅并行工作流用短生命周期分支。
- 测试：旧模型测试随旧代码删除并重建；新域按层覆盖，不可自动化面登记豁免；范围外与删除能力必须有负向测试。

## 契约所有者索引

| 契约 | 所有者 |
|---|---|
| 枚举/ID/面板参数/路径/布局不变量/恢复对账 | `design/domain-model.md` |
| crate 拓扑、状态所有权、线程、启动/关闭、数据根 | `design/architecture.md` |
| 状态库 schema、事务、去抖、损坏隔离、备份分层 | `design/persistence.md` |
| 命令、快捷键、焦点意图、覆盖层、IME | `design/ux-framework.md` |
| 错误、通知、任务服务、性能预算 | `design/cross-cutting.md` |
| 路径允许根、链接/OSC、Markdown 清洗、进程、凭据 | `design/security.md` |
| 单实例、窗口几何、系统通知、托盘、回收站、进程树 | `design/platform-windows.md` |
| 主题 token、内置主题、密度/透明度/材质、i18n | `design/theme-and-i18n.md` |
| 日志、审计、panic、诊断包、清理 | `design/diagnostics.md` |
| 会话扫描、归属、删除账本、恢复计划、套餐余量 | `design/integrations/agent-cli.md` |
| Hooks 目录、schema、三层文档、注入/卸载 | `design/integrations/hooks.md` |
| Runtime v2 协议、资源 ID、方法、错误码、移交载荷 | `design/integrations/runtime-api-v2.md` |
| 面板类型注册、生命周期、能力与焦点接缝 | `design/workspace/shell.md` |
| 面板参数与运行期模型（各面板） | `design/panels/*.md`、`design/sidebar/*.md` |
| 设置页注册、保存模式、应用页键表 | `design/panels/settings.md`、`design/settings/app-page.md` |
| AI Providers 元数据、AI 助手配置与 Provider 类型 | `design/settings/app-page.md` |
| AI 修复条触发、建议状态与终端增强行为 | `design/panels/terminal.md` |
| 加密备份归档、类别、恢复点与敏感键边界 | `design/persistence.md`、`design/security.md` |
| 构建、包清单、更新、发布门禁 | `design/build-and-release.md` |

## 维护规则

- 先改 `spec-issues.md`，再改 featureSpec，最后改设计；不静默偏离规格。
- 决策条目写「规则 / 原因 / 边界」；不写过程编号、行号、快照数字。
- 跨文件重复内容收敛到单一真值源，其他文档引用；契约数值只在所有者文档定义；类型/字段同名异义不得跨所有者出现。
- 里程碑使用名称，不使用阶段号；依赖关系只在 `plan.md` 定义，其他文档引用，不复制依赖列表。
- 计划变化同步 `plan.md` 与 `testing.md`；分支/标签使用 ASCII slug。
- 迁移映射只在 `migration.md` 与 `out-of-scope/` 记录旧实现去向；目标态设计不得引用旧 tab/pane 模型。
- `out-of-scope/` 不定义目标态契约；目标态契约只写实际所有者篇。迁移文档不复制数据路径与依赖清单，分别引用 `persistence.md`/`architecture.md` 与 `plan.md`。
- 代码落地后，长期决策迁入目标路径最近的 `CLAUDE.md` 或 `.claude/docs/` 对应子域。

## 使用方式

1. 实现前读对应域的所有者文档与 `design/testing.md` 的验收追溯；其他文档中的同名契约只作引用。
2. 每个里程碑按 `plan.md` 的依赖与退出条件执行；先过垂直切片门，再铺开面板。
3. 范围外能力在收敛门后按 `design/out-of-scope/` 迁移；迁移前应用页显示只读占位。
4. 提交前跑 `design/testing.md` 的门禁矩阵；文档改动跑 `check_doc_policy.py full` 与链接检查。
