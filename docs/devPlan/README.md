# slTerminal 目标态开发计划与开发设计

> 目标态重建的施工图：计划、架构、按域详细设计、迁移与测试；需求真值源是 `docs/featureSpec/`，裁决记录在 `spec-issues.md`。

## 定位

- `docs/featureSpec/` 定义“做什么”：目标态功能需求、验收判据与明确不做。
- `docs/devPlan/` 定义“怎么建、按什么顺序、怎么验收”：领域模型、模块拓扑、存储 schema、状态机、迁移映射与命名里程碑。
- 两者冲突时以 featureSpec 为需求真值源；若设计发现规格缺口，先更新 `spec-issues.md` 并修订 featureSpec，再改设计。
- 本目录是重建期工作文档，允许随实现演进；完成后由代码、`.claude/docs/` 与各路径 `CLAUDE.md` 承接长期决策。

## 文档地图

| 文档 | 内容 |
|---|---|
| `spec-issues.md` | 规格冲突、缺口、用户裁决、featureSpec 修订矩阵、设计层采纳项 |
| `plan.md` | 命名里程碑、依赖图、交付物、验收映射、风险、cutover |
| `design/architecture.md` | crate/模块拓扑、状态所有权、线程模型、启动关闭、旧壳隔离、数据根 |
| `design/domain-model.md` | 项目/布局/面板/终端簿、稳定 ID、DTO、恢复对账、不变量、Runtime ID 合同 |
| `design/gpui-runtime.md` | Entity 拓扑、订阅、异步取消、隐藏项目、面板错误边界、测试接缝 |
| `design/ux-framework.md` | 命令/动作注册、快捷键、派发优先级、焦点协调、覆盖层、IME |
| `design/persistence.md` | 文件与 schema、版本、写序、原子写、去抖、关闭冲刷、崩溃快照 |
| `design/cross-cutting.md` | 错误、通知、键盘与输入、性能、安全、i18n 的跨域索引 |
| `design/security.md` | 路径、URL/OSC、Markdown 清洗、进程、凭据、威胁模型与测试矩阵 |
| `design/platform-windows.md` | 单实例、窗口几何/DPI、通知、托盘、开机启动、回收站、进程树 |
| `design/theme-and-i18n.md` | 语义 token、内置暗色主题、密度/透明度/壁纸/材质、i18n 合同 |
| `design/diagnostics.md` | 日志、审计、panic、诊断包、脱敏、清理 |
| `design/build-and-release.md` | 新 crate 布局、包名、安装器、更新交接、门禁与体积预算 |
| `design/testing.md` | 测试分层、夹具、验收追溯、旧测试处置、豁免、门禁 |
| `design/migration.md` | 旧模块保留/改造/删除/新建、旧模型退役、范围外迁移总表 |
| `design/services/filesystem-git.md` | 路径权威、监听、Git 执行体、status/HEAD 缓存、写门 |
| `design/integrations/agent-cli.md` | AgentKind、hook 摄取、会话扫描、余量、删除隔离区 |
| `design/integrations/hooks.md` | Hooks 事件/handler 目录、schema、注入/卸载、托管条目 |
| `design/integrations/runtime-api-v2.md` | Runtime v2 资源模型、ID、传输、鉴权、命令、幂等 |
| `design/workspace/shell.md` | 项目生命周期、布局、焦点、关闭守卫、通知、单实例、无项目浮层 |
| `design/workspace/navigation.md` | 项目/活跃/历史会话、搜索、刷新、删除撤销、套餐余量 |
| `design/sidebar/framework.md` | 活动栏、双槽、视图注册、显隐、拖拽、宽度与比例 |
| `design/sidebar/explorer.md` | 文件树、多选、文件操作、过滤、忽略、Git 状态 |
| `design/sidebar/commit.md` | Commit 四态、分组、ignored、打开映射、回滚/删除 |
| `design/sidebar/agent-files.md` | Agent 节点、统一树、过滤消费、监听、标题消歧 |
| `design/panels/terminal.md` | PTY、渲染、输入、链接、剪贴板、编号、恢复 |
| `design/panels/editor.md` | 打开路由、编辑、Tree-sitter、保存、查找、折叠、外部修改 |
| `design/panels/markdown.md` | 两态、渲染管线、链接/资源、安全 HTML、公式、Mermaid 源码 |
| `design/panels/diff.md` | HEAD↔工作区、并排/行内、冲突快照、同步滚动 |
| `design/panels/gitshow.md` | HEAD 只读、刷新/过期、大文件/二进制 |
| `design/panels/settings.md` | 页面注册、导航、保存模式、Hooks、应用页入口 |
| `design/settings/app-page.md` | 应用页各节、无项目浮层、凭据边界、诊断清理 |
| `design/out-of-scope/*` | 应用页/壳服务、Runtime v2、AI Providers、终端增强、加密备份、更新发布 |

## 关键裁决

- 代码基线：分层重建；新增五个 core crate `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；保留 `slterm_terminal`、`slterm_settings`、`slterm_split`、`slterm_completions`、`slterm_hook`。
- 状态根：`<数据根>/state/`；设置/布局/项目数据在 `%APPDATA%\slterm`；日志/审计/更新/崩溃现场在 `%LOCALAPPDATA%\slterm`。
- 项目身份：稳定 `ProjectId`；根路径词法唯一并做真实路径查重。
- Runtime v2：资源 ID `project:<n>` / `group:<n>` / `panel:<n>`；不保留 v1 兼容层。
- 设置中心：每项目一个面板；新增应用页；无项目时打开仅含应用页的全局设置浮层。
- 能力取舍：Image、Merge、用户自定义主题、Markdown 渲染态实时块编辑、提示符跳转删除；vi/终端补全/数学覆盖层/AI 修复条保留为迁移能力；Mermaid 首版按源码显示。
- 无障碍：不提供屏幕阅读器/ARIA/读屏播报/WCAG 校验/高对比度/大字体适配；保留键盘、焦点、IME/CJK、减少动效。
- 数据迁移：不迁移旧 `settings.json`/`session.json`；旧文件不读不删；新状态根隔离。
- 测试：旧模型测试随旧代码删除并重建；新域按层覆盖，不可自动化面登记豁免。

## 维护规则

- 先改 `spec-issues.md`，再改 featureSpec，最后改设计；不静默偏离规格。
- 决策条目写「规则 / 原因 / 边界」；不写过程编号、行号、快照数字。
- 跨文件重复内容收敛到单一真值源，其他文档引用；契约数值只在所有者文档定义。
- 里程碑使用名称，不使用阶段号；计划变化同步 `plan.md` 与 `testing.md`。
- 迁移映射只在 `migration.md` 与 `out-of-scope/` 记录旧实现去向；目标态设计不得引用旧 tab/pane 模型。
- 代码落地后，长期决策迁入目标路径最近的 `CLAUDE.md` 或 `.claude/docs/` 对应子域。

## 使用方式

1. 实现前读对应域设计与 `design/testing.md` 的验收追溯。
2. 每个里程碑按 `plan.md` 的依赖与退出条件执行；先过垂直切片门，再铺开面板。
3. 范围外能力在收敛门后按 `design/out-of-scope/` 迁移；迁移前应用页显示只读占位。
4. 提交前跑 `design/testing.md` 的门禁矩阵；文档改动跑 `check_doc_policy.py full` 与链接检查。
