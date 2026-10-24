# 旧实现处置与迁移边界

> 规定旧代码、旧数据和旧协议的保留、重写或删除方式；目标实现不建立兼容执行层。处置基线为 2026-10-24 工作树。

## 决策与红线

### 总体策略

- 当前只有单一用户使用，允许中间提交暂不具备完整产品能力；不为维持旧壳而保留双窗口、双 Document、双 Runtime 或永久开关。
- 旧实现只作行为参考、测试素材和可迁移算法的来源；凡与目标模型冲突的代码直接重写或删除，不能以“已有实现”作为保留理由。
- 每次删除同步处理调用方、测试、资源、发布资产和最近路径 CLAUDE.md；不留下编译关闭的旧功能、迁移占位或引用不存在模块的文档。
- 不保留 `spec-issues.md`、评审档案或 `design/out-of-scope/`。最终排除项只由 featureSpec 与目标代码入口证明，过程问题按协作纪律报告。

### 模块处置

| 旧实现或资产 | 目标处置 | 目标承接 |
|---|---|---|
| `app_shell` 多实体壳与旧面板自持状态 | 重建所有权树，不保留第二壳 | AppRoot、ProjectView、PanelHost、TaskService |
| `file_editor` 的 `String`／InputState 实时缓冲与旧撤销 | 重写为 Rope、EditTransaction、TextStateId；600 毫秒合并和 8 MiB 预算可作算法参考 | DocumentSession、TextSource |
| 旧 Diff 双侧只读 InputState | 删除双缓冲，工作区侧引用共享 Document，HEAD／冲突使用只读快照 | DiffInput、DiffBinding |
| 旧双 Markdown 解析与 GPUI Markdown 桥 | 删除第二语义源，Comrak 唯一投影；Tree-sitter 仅源码高亮 | MarkdownDocument、专用 GPUI 块视图 |
| 旧 terminal_math、终端扫描和数学覆盖层 | 删除终端能力，保留公式编译与位图生成 | Markdown 公式服务 |
| 旧 AI Providers、模型管理、AI 助手、AI 修复条 | 全量删除，不保留设置入口或凭据推导 | 无 |
| 旧 `ai_sessions.json` 长期注册表 | 删除读取与写入；不迁为第二正文库 | AgentCatalog＋原生历史适配器 |
| `ai_agents` 的 AgentKind、别名和命令构造 | 保留目录事实并迁入单一 AgentCatalog，重新核对恢复／分支能力 | `slterm_agents` |
| `ai_hook` 注入与 Claude 编辑器 | 保留适配器素材，重写草稿、子树保存和 Journal 恢复 | `slterm_hooks`、HookService |
| 旧 Shell 白名单与启动恢复直写命令 | 自定义 Shell 走绝对可执行路径；恢复等待 Shell 就绪并受代次取消 | TerminalSession、ShellLineTracker |
| PowerShell Enter 将正文放入 `OSC 133;C` | 删除正文传递，执行确认由适配器对账 | 可信 Shell 行与学习库 |
| 旧历史 JSONL／JSON 学习文件 | 不自动迁移、不覆盖；新 learning.db 只接受当前 schema | LearningStore |
| 旧 `history.rs` 与目录 frecency 算法 | 保留去重、容量和 frecency 语义，迁入独立 SQLite 写队列 | `learning.db` |
| 旧套餐读取 CLI 配置 token | 删除该认证通道；固定来源、专用 token、Windows 凭据管理器 | PlanBalanceService、CredentialFacade |
| 旧主题 ZIP、媒体、派生色与 WCAG 门禁 | 删除包格式和非颜色能力；完整 ThemeDefinition JSON 重写 | ThemeStore、ThemeTokens |
| 旧备份六类别与恢复点口令依赖 | 拒绝旧格式；改为 settings／workspace 逻辑载荷与 DPAPI 恢复点 | BackupPayload、维护期恢复 |
| 旧 Git `checkout HEAD` 回滚 | 删除该写路径；只从当前索引恢复工作区，staged added 删除单独 reset | GitFacade、Commit |
| 旧 best-effort ACL 保存 | 改为无法保留目标 ACL 即失败 | 原子文件与 PathResolver |
| 旧 Runtime v1、window／tab／pane、orchestrate、send_key、procs、delegate | 全量删除，不做兼容路由 | Runtime v2 资源方法 |
| 旧 Runtime token 环境变量、端口文件 token、独立 handover schema | 删除；统一 LocalControlServer、双凭据作用域和生成 v2 schema | `slterm_runtime_protocol`、CLI |
| 旧 `runtime_protocol` 零生产依赖合同 | 按目标改为 Serde DTO＋可选 schemars，并同步 crate CLAUDE.md | Runtime schema 生成器 |
| GPUI fork 的旧编辑器／Markdown／数学补丁 | 审计后做补丁减法，保留必要 Windows 修复，不升级版本线 | 依赖基线与补丁审计 |
| 旧设置全局路径推导与主题规则 | 重写为 AppPaths 注入、SettingsRegistry 和纯颜色主题 | `slterm_path`、`slterm_settings` |

### 数据与协议边界

- 不自动读取或迁移旧工作区库、旧 JSONL／JSON 学习历史、`ai_sessions.json`、旧主题 ZIP、旧备份和 v1 Runtime 端点材料。旧文件不因启动被覆盖或静默删除；需要清理时由用户显式操作。
- 新库、快照、备份和协议只接受各自当前版本。高版本只读，损坏按统一分流；不编写旧库升级链，也不以兼容 DTO 保存两套字段。
- 旧 Runtime token 不从环境变量或端口文件恢复；新凭据只在 Windows 凭据管理器生成和读取。旧 token 失效不影响新单实例锁。
- 旧备份不尝试解密或部分导入；新恢复只接受新魔数、当前版本和完整鉴权。旧自定义主题不自动转换，用户需按新 JSON 合同重新导入。
- 旧实现中仍符合目标的纯算法可迁移，但必须经过新领域类型、测试和安全边界接收，不能连同旧全局状态、旧路径推导或旧错误语义一起搬入。

### 文档同步责任

- 实施涉及 `slterm_app`、`slterm_settings`、`slterm_runtime_protocol`、plan_balance、主题和终端目录时，同步更新其最近 CLAUDE.md 中与目标冲突的旧红线；不以本目录设计默认覆盖代码层常驻指令。
- 根架构、依赖清单、发布合同和测试豁免随实际改动更新；文档只保留当前有效决策，历史沿革从 git log 查询。
- 本迁移表是目标处置合同，不是待办清单或问题登记；实际完成状态只看 [进度记录](../progress.md) 与代码验收。

## 验收与测试

- 静态检索确认旧 AI、旧 Runtime、旧主题包、第二 Markdown、终端数学和 `ai_sessions.json` 没有生产入口、设置入口或发布资产。
- 数据测试证明旧文件不会被启动读取、覆盖或静默删除；新格式拒绝旧版本且错误可见。
- 迁移算法逐项用新测试验证：撤销合并、编码、补全、frecency、公式、Kimi／DeepSeek 解析、更新校验和 hooks 注入。
- 文档策略检查确认没有新增 `spec-issues`、`out-of-scope`、过程编号或旧品牌名回流。

## 操作

- 删除模块前先以 Cargo tree、符号检索和发布清单找出全部引用，再一次性完成代码、测试、资源和文档处置。
- 发现旧实现缺陷但与目标无关时按协作纪律报告，不在迁移过程中顺手扩大修改范围。
- 任何需要保留旧行为的例外必须先取得用户裁决并写入对应所有者文档，不能在本表私下添加兼容分支。
