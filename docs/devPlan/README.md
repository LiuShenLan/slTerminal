# slTerminal 开发设计

> 以功能需求规格为行为真值，规定目标实现的架构、状态机、存储、实施顺序与验收证据。

## 决策与红线

- **行为真值**：产品行为以 [功能需求规格](../featureSpec/README.md) 为准；本目录只规定如何实现，不复制需求正文或另建产品例外。
- **目标态优先**：按未来最优直接重建，不设新旧双壳、兼容层、永久功能开关或占位产品。中间提交可以暂不形成完整产品，但不得混入未经批准的替代语义。
- **改动边界**：开发决策先经用户裁决；实施中发现需求冲突、依赖缺口或既有缺陷时先报告，不以本计划作为擅自扩权的依据。
- **提交底线**：每个提交必须编译、测试和静态门禁全绿；删除旧能力、更新测试和更新所有者文档属于同一改动，不允许留下引用已删除模块的红灯。
- **协议真值**：Runtime 的方法语义在本目录设计，字段级线缆契约只由目标实现生成的 `assets/runtime-api-v2.schema.json` 与 `assets/runtime-control-api.md` 拥有，本目录不维护第二份字段表。
- **Mermaid 边界**：目标实现的其他功能完成前，`mermaid` 只作为普通类型化代码块；不实现渲染，也不预留专用 renderer trait、插件注册表、缓存或占位界面。最终静态渲染由用户后续单独实现。
- **过程信息**：实际实施状态只写 [进度记录](progress.md)；问题、临时想法和评审过程不写入设计文档。

## 验收与测试

- 文档链接、文档策略、禁名、空白和范围检查必须通过；不得出现源码行号、过程性编号、无锚点演进措辞或易腐计数。
- [追溯矩阵](traceability.md) 必须覆盖 featureSpec 的每个所有者及跨域验收场景；最终核销必须指向实际测试例名或已登记豁免。
- 每个设计章节必须给出可自动化的失败、并发和恢复测试接缝；不可自动化部分必须指向 [.claude 测试豁免](../../.claude/docs/testing/exemptions.md) 的登记要求。
- 实施验收使用 `target/debug/slterm.exe` 和真实 Windows 环境；不得以单元测试通过替代真实进程、ConPTY、双实例与发布验收。

## 操作

### 阅读顺序

| 文档 | 用途 |
|---|---|
| [实施计划](plan.md) | 命名阶段、依赖顺序、集成检查点和提交规则。 |
| [进度记录](progress.md) | 当前实施位置、阶段状态与已验收证据。 |
| [架构](design/architecture.md) | crate 边界、AppRoot 所有权、任务与命令单点。 |
| [领域模型](design/domain-model.md) | 稳定身份、ViewKey、Document、Terminal、RepositoryBinding 与代次。 |
| [依赖](design/dependencies.md) | 固定版本、许可、grammar、GPUI 补丁与体积审计。 |
| [生命周期](design/lifecycle.md) | 启动维护、正常关闭、崩溃对账和更新移交。 |
| [持久化](design/persistence.md) | 数据根、SQLite schema、DTO、快照、Operation 与原子文件。 |
| [工作区](design/workspace.md) | 项目布局、Panel、SideView、焦点和命令路由。 |
| [Document](design/document.md) | Rope、编码、撤销、分页、语法和文本视图。 |
| [Markdown](design/markdown.md) | Comrak 投影、安全 HTML／SVG、图片、公式与渐进渲染。 |
| [终端](design/terminal.md) | PTY 执行体、可信输入、补全、学习库和恢复。 |
| [文件与 Git](design/files-git.md) | watcher、文件操作、GitFacade、Diff、GitShow、Explorer 与 Commit。 |
| [Agent](design/agents.md) | AgentCatalog、活跃会话、原生历史与隔离撤销。 |
| [Hooks](design/hooks.md) | 草稿、受限写入、注入事务和 hook 小进程桥。 |
| [Runtime](design/runtime.md) | JSON-RPC v2、资源方法、凭据、幂等、事件和 CLI。 |
| [设置与恢复](design/settings-recovery.md) | 设置注册表、主题、凭据、加密备份和维护期恢复。 |
| [Windows](design/windows.md) | 套餐、托盘、通知、语言、更新、诊断、清理和发布。 |
| [迁移处置](design/migration.md) | 旧模块保留、重写、删除及旧数据边界。 |
| [测试设计](design/testing.md) | 自动化分层、故障注入、真实进程与豁免规则。 |
| [追溯矩阵](traceability.md) | 需求场景到设计、测试和人工证据的映射。 |

### 实施规则

- 按 [实施计划](plan.md) 的依赖顺序推进；阶段内仍拆小提交，不把整个阶段做成一次不可审查改写。
- 阶段状态或集成检查点变化时同步 [进度记录](progress.md)；完成状态必须带日期、commit hash 和实际验证输出。
- 完整命令矩阵使用 [.claude 操作命令](../../.claude/docs/operations/commands.md)，不在本目录复制第二份命令合同。
