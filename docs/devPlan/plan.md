# 实施计划

> 按依赖闭合组织命名阶段、集成检查点和提交证据，不要求中间形成完整可用产品。

## 决策与红线

### 通用交付规则

- 每阶段先建立类型、状态机和假端口测试，再接真实 I/O 与 GPUI；不得先写不可测试的界面流程再补领域规则。
- 每阶段列出的退出条件是进入下一阶段的依赖保证，不是允许跳过其他门禁的理由。所有提交持续执行架构、文档、禁名、clippy、fmt 和串行测试。
- 删除旧实现时同步删除或重写其测试、资源、文档和发布资产；不保留编译关闭的旧功能、兼容适配或占位入口。
- 阶段状态只在对应证据真实产生后更新到 [进度记录](progress.md)。未完成不得以“代码已存在”标记已验收。

### 依赖基线与删减

**入口**：目标设计与依赖裁决已完成。

**交付**：新 workspace 骨架与依赖方向；固定依赖和 grammar 清单；GPUI 补丁审计；删除 AI Providers／Assistant、Merge、终端数学、旧 Runtime、旧主题包和其他最终排除实现。

**退出证据**：所有 grammar 在 Windows x64 通过编译、ABI 与 query 加载；许可与 notice 完整；全量门禁在无旧模块引用的情况下全绿；记录新体积实测基线。

### 领域基础

**入口**：依赖基线与删减已验收。

**交付**：AppPaths、稳定 ID、PathPolicy、SettingsRegistry、主题 token、typed i18n、工作区库与学习库、ViewState DTO、Operation／ManagedOperationJournal、原子文件和审计端口。

**退出证据**：临时数据根下的数据库事务、损坏分流、授权能力并集、设置单字段回退、原子写和日志对账测试通过；无模块自行读取数据根环境变量。

### 工作区与生命周期

**入口**：领域基础已验收。

**交付**：AppRoot、ProjectView、PanelHost、SideViewHost、泛型布局、CommandRegistry、FocusManager、TaskService、Startup／CloseCoordinator、LocalControlServer 与单实例移交。

**退出证据**：布局属性测试、GPUI 焦点与 IME 测试、启动维护崩溃序列、统一关闭守卫和双实例移交通过；第二实例连接失败不会创建第二窗口。

### Document 与文本视图

**入口**：工作区与生命周期已验收。

**交付**：DocumentRegistry／DocumentSession、Rope 编辑与撤销、编码与行尾、分页只读、保存与外部修改、崩溃快照、LanguageRegistry、Tree-sitter、TextSource 和自定义 GPUI 文本视图。

**退出证据**：跨项目／跨视图共享 Document、并发保存、撤销清脏、编码失败、超限降级、快照恢复和 IME 测试通过；不存在 InputState 或 String 形式的第二实时缓冲。

### 终端、补全与学习

**入口**：Document 与文本视图已验收。

**交付**：TerminalSession worker、PTY 代次、背压与 resize 水位、ShellLineTracker、Shell 适配器、CompletionService、learning.db、终端搜索和元数据恢复。

**退出证据**：真实 ConPTY 的输入输出、洪峰、resize、退出和重连通过；不可信行不补全不采集，旧候选不回填，清空学习取消待写；64 个存活 PTY 上限按进程计数。

### 文件与 Git

**入口**：终端、补全与学习已验收。

**交付**：WatcherService、FileOperationService、GitFacade、Explorer、Commit、Diff、GitShow、Editor Git 行标和受限破坏操作。

**退出证据**：链接重定向、能力并集、批量部分成功、porcelain 解析、staged／unstaged 对账、固定仓库绑定和差异预算测试通过；回滚只从当前索引恢复工作区。

### Markdown 与安全资源

**入口**：Document、PathPolicy 和文本视图可用。

**交付**：Comrak 扁平块投影、锚点、查找、安全 HTML／SVG、图片授权与缓存、公式、渐进渲染和阅读状态。

**退出证据**：危险输入语料无脚本、危险协议、未授权网络或外部 SVG 引用；源码与渲染共享 Document；远程图片只经单次点击授权；`mermaid` 仅为普通代码块。

### Agent 与 Hooks

**入口**：终端、文件操作、设置注册表和侧栏框架可用。

**交付**：AgentCatalog、活跃会话关联、原生历史扫描、导航恢复／分支、隔离批次、AgentRoot、Hooks 草稿与注入、命名管道和 hook 小进程桥。

**退出证据**：能力目录与命令构造测试、历史来源故障、逐批崩溃对账、全批撤销预检、Hooks 外部修改和注入提交边界通过；不形成第二长期正文库。

### 设置、主题与恢复

**入口**：所有可移植设置与工作区状态已稳定。

**交付**：完整设置中心、自定义主题 JSON、凭据门面、普通导入导出、BackupPayload、加密导出、DPAPI 恢复点、维护期恢复与跨机重定位。

**退出证据**：设置队列、主题零写入失败、备份篡改、类别排除、恢复中断、指纹撤回和 store_epoch 更换通过；任何产物均不含真实凭据或永久排除数据。

### Runtime v2

**入口**：领域命令、Operation、LocalControlServer 和维护期恢复已验收。

**交付**：Serde 协议、生成 schema、全部资源方法、凭据作用域、发现 CLI、持久幂等、分页读取、事件重放和 Runtime conformance。

**退出证据**：真实 GUI／CLI 的握手、断线重查、重复 mutation、事件缺口、非交互关闭、关闭中拒绝和 schema 漂移测试通过；v1 与 token 环境变量无执行路径。

### Windows 与发布

**入口**：统一关闭、凭据和 Runtime 已可用。

**交付**：套餐余量、托盘、登录启动、材质、窗口几何、通知、任务栏、更新、诊断、清理和发布链。

**退出证据**：固定来源查询与凭据隔离、更新校验和 handoff 对账、诊断白名单、清理活跃排除、安装器与便携包验收通过；更新安装不得绕过关闭守卫。

### 全量验收

**入口**：全部功能阶段已验收。

**交付**：追溯矩阵核销、全量串行测试、真实 Windows 场景、人工豁免证据、发布清单和最终进度状态。

**退出证据**：每个 featureSpec 场景都有实际测试或登记豁免；最终排除项无隐藏入口；发布包内 Runtime、许可和数据边界符合合同。

## 验收与测试

- 集成检查点至少覆盖：工作区关闭崩溃序列、跨视图 Document 编辑、真实 ConPTY 洪峰与 resize、历史隔离崩溃对账、备份维护期中断、Runtime 双进程断线和更新移交。
- 每个检查点同时观察内存状态与真实磁盘／进程副作用，不以 DTO 状态推断操作已经成功。
- 阶段内新增不可自动化面时，先登记 `.claude/docs/testing/exemptions.md` 的根因和兜底层级，再允许对应提交进入验收。
- 进度状态与追溯矩阵必须一致；已验收阶段不得引用不存在的测试例或未执行的实机证据。

## 操作

- 全量测试、定向测试和静态门禁使用 [.claude 命令矩阵](../../.claude/docs/operations/commands.md)，所有 cargo 与脚本钉测试串行执行。
- 每个提交前运行与改动面相符的定向门禁；阶段验收和最终交付运行全部门禁，不能用前一次阶段结果替代新证据。
- 阶段状态变化后更新 [进度记录](progress.md)，只登记已验证事实和当前阻塞，不补写流水账。
