# 开发计划

> 目标态重建的命名里程碑、直接依赖、退出条件、并行组与回滚策略；不写工期估算，不使用阶段编号。

### 原则

- **单一真值源**：每个契约只由一个文档拥有；重复内容改为引用，不在计划里复制设计细节。
- **分层重建**：保留已验证引擎，重建项目/布局/面板领域与 app 壳；旧 workspace/tab/pane/dock/session 整体废弃。
- **规格先行**：`spec-issues.md` 的裁决先落回 featureSpec，再进入设计与实现。
- **垂直切片**：先打通多项目、布局、终端、持久化、恢复与关闭守卫，再铺开面板；切片门是所有后续功能里程碑的前置。
- **主线可编译**：`legacy-shell` feature 门控旧代码，主线始终可编译；里程碑边界门禁全绿，里程碑内允许短暂红灯。
- **门禁前置**：规格冻结、垂直切片、跨域验收三道门；每道门不通过不进入下一批。
- **命名里程碑**：里程碑只用名称，不使用 M/N/阶段号。
- **缺口显式**：过渡期不可用的能力以只读占位或禁用态呈现，并登记迁移里程碑；不提供半成品控件。

### 前置条件与环境

- Rust 工具链按 `rust-toolchain.toml` 固定；GPUI 与 gpui-component 使用根 `Cargo.toml` 钉死的 fork rev，升级必须单独评审。
- Windows 10 SDK、Inno Setup、可选 Authenticode 证书；CI 使用 Windows runner，构建与测试命令遵循 `.claude/docs/operations/commands.md`。
- 测试隔离使用 `SLTERM_CONFIG_DIR` 与 `SLTERM_LOCAL_DIR` 指向临时目录；生产 `%APPDATA%`/`%LOCALAPPDATA%` 永不被测试触碰。
- Tree-sitter grammar crate 在编辑器里程碑前完成选型、许可与体积登记；新增依赖必须进 `architecture/dependencies.toml` 与体积基线。
- 一次性启用 git hooks：`git config core.hooksPath .githooks`。
- 每个里程碑退出前同步更新对应路径 `CLAUDE.md`、`CONTEXT.md` 与 `.claude/docs/`；文档改动跑 `check_doc_policy.py full` 与相对链接检查。

### 里程碑

依赖列只写里程碑全名；本表是依赖关系的唯一来源，未列出的边不存在。

| 里程碑 | 交付物 | 直接依赖 | 退出条件 | 验收映射 |
|---|---|---|---|---|
| 规格裁决与冻结 | `spec-issues.md` 关闭；featureSpec 修订；`CONTEXT.md` 术语切换 | — | 全部裁决有落点；文档门禁绿 | README、全部后续 |
| 领域模型与骨架 | `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；ID、四层模型、DTO、纯转换、测试 target、架构登记 | 规格裁决与冻结 | 五个 crate 可编译；不变量与恢复对账测试通过；依赖策略与行预算登记 | shell/README 模型与不变量 |
| 应用基础服务 | 命令/焦点、通知/对话框/覆盖层、主题 token、持久化原语、Windows 平台薄层、凭据门面、日志/panic 骨架 | 领域模型与骨架 | 纯函数与虚拟窗口测试通过；焦点/通知/日志单源；日志根可注入；凭据不出域 | cross-cutting、ux-framework、platform-windows、diagnostics、security |
| 设置存储与写通道 | 设置 schema、原子写/备份、去抖/冲刷、导入导出、只读/损坏 | 应用基础服务 | 故障注入全绿；备份失败中止；关闭冲刷分类正确 | settings-and-persistence |
| 构建与打包基线 | workspace 成员、依赖策略、`PackageEntries`、安装器/便携包 staging、体积基线 | 领域模型与骨架 | 架构门禁全绿；包清单三处一致；体积测试有可复现基线 | build-and-release |
| 诊断与日志迁移 | 日志、审计、panic、诊断包、脱敏、一键清理、本地/漫游分根 | 应用基础服务 | 脱敏与清理测试通过；路径由 `slterm_settings::paths` 单点；清理不触碰恢复快照 | diagnostics |
| 壳与工作区 | 启动/关闭、项目生命周期、布局、焦点、关闭守卫、标题栏、单实例、无项目浮层 | 应用基础服务、设置存储与写通道、构建与打包基线 | 单项目可运行；关闭链分类正确；主线可编译；移交走 `runtime.handover` | workspace/shell |
| 旧模型隔离 | `legacy-shell` feature、`check_legacy_symbols` allowlist、新状态根、`instance.lock`、旧数据不动 | 壳与工作区、构建与打包基线 | 旧符号只出现在 allowlist；旧文件逐字节不变；新端点与新锁独立 | migration |
| 终端面板 | PTY 生命周期、渲染、输入、链接、剪贴板、编号/cwd/标题（`terminals.json`）、恢复、关闭守卫 | 壳与工作区 | PTY 上限 64、并发 8；OSC 8/52；编号恢复；关闭守卫 | panels/terminal |
| 侧栏框架 + 导航核心 | 活动栏、双槽、视图注册、侧栏 schema；项目 CRUD/切换/排序/重命名/移除 | 壳与工作区、应用基础服务 | 状态矩阵测试；schema 与 `persistence.md` 一致；导航 CRUD 全通过 | sidebar/framework、workspace/navigation |
| 垂直切片验收 | 多项目创建/切换、拆分、终端、隐藏项目 PTY 存活、设置/布局持久化、重启恢复、崩溃快照基础设施（夹具）、焦点恢复、关闭守卫 | 终端面板、侧栏框架 + 导航核心、设置存储与写通道、诊断与日志迁移 | 场景全通过；重启不抢焦；隐藏项目 PTY 继续运行；快照配额/隔离/恢复提示可验证 | acceptance 子集 |
| Git 与文件服务 | PathAuthority、仓库根、status/HEAD、监听、外部修改、回收站、写门 | 垂直切片验收 | 路径决策表；仓库根一致；外部修改冲突；写门串行 | services/filesystem-git |
| CLI/Agent 集成核心 | AgentKind、hook 摄取、状态/用量、历史扫描、假 CLI 适配器 | 终端面板、设置存储与写通道、垂直切片验收 | 能力表；事件归一化；历史归属；UI 不复制 CLI 逻辑 | integrations/agent-cli |
| Hooks 引擎与托管条目 | 事件/handler 目录、schema、User/Project/Local、GUI/JSON、保存事务、注入/卸载、托管条目 | CLI/Agent 集成核心、Git 与文件服务 | 注入/卸载提交点；外部修改冲突；托管条目只读；备份失败中止 | integrations/hooks |
| 导航历史与余量 | 历史会话、恢复、套餐余量 | 侧栏框架 + 导航核心、CLI/Agent 集成核心 | 归属/恢复/余量；状态作用域 | workspace/navigation |
| 编辑器面板 | 路由、文档内核、Tree-sitter、gutter、外部修改、崩溃快照、AgentRoot 可写、PathGuard 复核 | 垂直切片验收、Git 与文件服务、设置存储与写通道、诊断与日志迁移 | 打开路由；保存冲突；gutter 三类标记；快照配额；Agent 根可保存 | panels/editor |
| Markdown 面板 | 两态、渲染、公式、Mermaid 源码、草稿恢复 | 编辑器面板 | 两态；安全 HTML；Mermaid 源码；草稿快照 | panels/markdown |
| 文件浏览器 | 树、多选、批量、剪贴板、Git 状态、过滤、监听 | 编辑器面板、Markdown 面板、Git 与文件服务 | 树模型；写边界；监听；项目级状态 | sidebar/explorer |
| Diff 面板 | 并排/行内、冲突快照、保存冲突、同步滚动、`mode/source` 去重 | 编辑器面板、Markdown 面板、Git 与文件服务、垂直切片验收 | 行对；工作区编辑；冲突快照瞬态；保存并发统一 | panels/diff |
| GitShow 面板 | HEAD 只读、刷新/过期、大文件/二进制、临时文件 | Git 与文件服务、垂直切片验收 | 只读；刷新；临时文件生命周期；去重 | panels/gitshow |
| Commit 视图 | 四态、分组、ignored、打开映射、回滚/删除 | 文件浏览器、Diff 面板、GitShow 面板、编辑器面板 | 打开映射走文件路由；根外只读；回滚/删除守卫 | sidebar/commit |
| 设置中心壳、基础页与应用页占位 | 页面注册/导航/保存模式；快捷键、外观、终端；应用页壳、节序、自动保存、通知、诊断、Runtime 只读不可用、范围外节占位 | 壳与工作区、设置存储与写通道、侧栏框架 + 导航核心、诊断与日志迁移 | 页面注册；保存模式；应用页节级矩阵；浮层导入/导出/恢复默认 | panels/settings、settings/app-page |
| 设置中心 Agent 页 | Agent 基础配置 | CLI/Agent 集成核心、设置中心壳、基础页与应用页占位 | 配置项；自定义根校验；运行时文件清单 | panels/settings |
| 设置中心 Hooks 页 | Hooks 页面与保存 | Hooks 引擎与托管条目、设置中心壳、基础页与应用页占位 | 三层；GUI/JSON；外部修改；注入状态 | panels/settings |
| 设置中心后台任务页 | 任务配置 | CLI/Agent 集成核心、设置中心壳、基础页与应用页占位 | 启用/频率/失败；视图订阅 | panels/settings |
| Agent 全局文件 | 节点探测、统一树、过滤、监听、标题消歧 | 文件浏览器、设置中心 Agent 页、CLI/Agent 集成核心 | 节点；过滤；App watcher + 视图订阅；写边界 | sidebar/agent-files |
| 跨域收敛与规格验收 | 验收矩阵、键盘/IME、性能、安全、门禁、删除能力负向测试 | 全部功能里程碑 | `acceptance.md` 全通过；无无障碍残留；范围外负向测试；门禁绿 | acceptance |
| legacy-shell 清理与门禁重登记 | 删除 `legacy-shell`、空 allowlist、预算重基线 | 跨域收敛与规格验收 | 旧符号零引用；包清单/预算更新；全量门禁绿 | migration、architecture |
| Runtime API v2 | 协议、CLI、`slterm env`、随包 skill、conformance、审计、`runtime.handover` | 跨域收敛与规格验收、壳与工作区、终端面板、设置中心壳、基础页与应用页占位 | conformance；幂等/错误码；移交；审计字段；无 `window.*` 方法 | integrations/runtime-api-v2、out-of-scope/runtime-api-v2 |
| 应用页范围外节落地与壳服务 | 界面语言、更新、托盘与开机启动、Providers/助手入口、备份/恢复入口、终端增强开关；托盘、任务栏、窗口几何、更新入口、诊断服务集成 | 跨域收敛与规格验收、设置中心壳、基础页与应用页占位、诊断与日志迁移、Runtime API v2 | 节级矩阵逐项替换；托盘/快捷方式/窗口几何；首版无静默启动 | out-of-scope/app-and-shell-services |
| AI Providers/助手 | Providers、助手、命令修复、凭据 | 应用页范围外节落地与壳服务、CLI/Agent 集成核心、应用基础服务 | 元数据 CRUD；凭据门面；脱敏；关闭后不发请求 | out-of-scope/ai-providers-assistant |
| 终端增强回迁 | vi、补全、数学覆盖层、AI 修复条 | 终端面板、应用页范围外节落地与壳服务、AI Providers/助手、设置存储与写通道 | 开关默认；按键顺序；取消；缓存按面板 | out-of-scope/terminal-enhancements |
| 加密备份 | 归档、类别、敏感键、恢复点、UI | 设置存储与写通道、应用页范围外节落地与壳服务、AI Providers/助手、领域模型与骨架 | 加解密/校验/回滚；类别白名单；旧格式拒绝 | out-of-scope/encrypted-backup |
| 更新与发布链 | 更新链、交接、发布链、代码签名 | 应用页范围外节落地与壳服务、构建与打包基线、壳与工作区、诊断与日志迁移 | 四重校验；handoff；签名；发布门禁 | out-of-scope/update-release |

### 依赖与并行

- **依赖规则**：每个里程碑只依赖上表列出的里程碑；跨层依赖必须显式登记，不允许“按顺序隐含依赖”。
- **基础层并行**：领域模型与骨架、应用基础服务、设置存储与写通道、构建与打包基线、诊断与日志迁移可并行，共用 `spec-issues.md` 裁决。
- **壳与切片**：壳与工作区先于旧模型隔离、终端面板、侧栏框架 + 导航核心；垂直切片门依赖终端、导航核心、设置存储、诊断与恢复快照基础设施。
- **服务与内容**：Git 与文件服务、CLI/Agent 集成核心在切片门后并行；Hooks 引擎依赖 CLI 集成与文件服务；编辑器依赖切片门、文件服务、设置与诊断；Markdown、Explorer、Diff、GitShow、Commit 按表中依赖推进。
- **设置与 Agent**：设置中心壳先于 Agent/Hooks/后台任务页；Agent 全局文件依赖 Explorer、Agent 页与 CLI 集成。
- **收敛与迁移**：全部功能里程碑完成后过收敛门；`legacy-shell` 清理先于范围外迁移；Runtime v2 先于应用页范围外节与壳服务，更新与发布链最后。
- **文件所有权**：并行开发按 `src/app/`、`src/services/`、`src/persistence/`、`src/runtime_api/` 与面板目录划分写域；同一文件同一时间只由一个工作流修改；跨域改动先改所有者文档再改实现。
- **同步点**：基础层完成、垂直切片门、收敛门、Runtime v2 conformance 四个同步点；同步点必须全量构建与门禁通过。

### 分支、提交与回滚

- 每个里程碑使用短生命周期分支 `rebuild/<milestone-name>`；主线始终可编译。
- 里程碑内允许多次提交与短暂红灯；合并回主线前必须 `cargo fmt`、clippy、全量测试、架构/文档/品牌门禁全绿。
- 每个里程碑合并后在主线打 `checkpoint/<milestone-name>` 标签；回滚点就是最近一个绿色 checkpoint。
- 旧模型删除前保留最后一个可运行 checkpoint；删除后只向前修复，不回滚旧模型。
- 范围外迁移失败以功能开关/占位降级，不回退目标态核心。
- 公开发布门：Runtime API v2、应用页范围外节、更新链与加密备份完成前只发内部构建，不发公开版本。

### 风险与缓解

| 风险 | 触发信号 | 缓解 |
|---|---|---|
| 状态所有权与 GPUI 实体双源 | 同一事实在两个实体出现可变副本 | 所有权矩阵；领域纯数据 + 运行时注册表；契约测试 |
| 新 core 依赖 GPUI 或 app 穿透 core | `check_architecture.py` 红 | core 只暴露 ID/纯函数/port/DTO；依赖策略即时登记；每 port 假实现 |
| 多 owner 去抖与跨文件一致性 | 故障注入/generation 测试失败 | 单一 `PersistenceCoordinator`；串行写 + generation；提交顺序与孤儿清理 |
| 关闭链超时或丢数据 | 关闭阶段超过 3 秒或 Guarded 写失败 | `CloseState` 状态机；后台三阶段；Guarded 失败中止并提供重试/取消 |
| 焦点与 IME 分散 | 虚拟窗口键盘/IME 用例失败 | 中央焦点协调器；一次性意图；统一 IME 门控；键盘测试 |
| 隐藏项目资源失控 | 切换项目 p95 超过 16.7ms 或后台 CPU 持续增长 | 隐藏子树不布局/不绘制；PTY 继续但 UI 更新节流；多项目基准 |
| 新旧 app 共用数据/单实例命名空间 | 新启动读到旧端口或旧锁 | 新 `state/` 根、`instance.lock`、旧文件不读不删；启动隔离测试 |
| 旧测试删除导致知识丢失 | 删除测试无替代映射 | 删除前建立旧测试 → 规格/新测试映射；四类夹具先行 |
| CLI/Hooks/Agent 逻辑在 UI 复制 | UI 出现 hook 解析或进程探测 | 单一 Agent 能力注册表与 hook/session 执行体；UI 只消费 |
| Windows 平台面集中爆雷 | 业务层出现 `#[cfg(windows)]` | 平台薄接口 + 假实现；实机冒烟与豁免登记 |
| 范围外能力反向修改核心 | 协议/领域契约在迁移期变更 | 领域阶段冻结 Runtime ID 与资源语义；壳/设置/终端预留扩展点 |
| 文档再次漂移 | 同一契约在两篇出现不同数值或类型 | 单一所有者；链接与文档策略门禁；评审时先查所有者 |
| GPUI fork 升级失控 | 升级需要改协议/领域类型 | 钉死 rev；升级单独评审；先跑全量门禁与体积基线 |
| Tree-sitter 体积/编译时间超预算 | 打包钉测试红 | 编辑器里程碑前完成 grammar 选型与体积登记；超预算即裁剪语言集 |
| 控制面空窗 | 收敛后、Runtime v2 前无控制面 | 只发内部构建；Runtime v2 作为范围外第一项 |

### 追溯

- `design/testing.md` 维护 featureSpec 篇目 → 设计所有者 → 里程碑 → 验收场景 → 测试层的矩阵。
- `spec-issues.md` 记录每条规格修订与设计裁决；实现偏差必须先更新该文件并同步 featureSpec。
- 里程碑退出条件 = 交付物完成 + 对应验收判据通过 + 门禁全绿 + 旧代码/旧测试处置完成 + 文档同步完成。
