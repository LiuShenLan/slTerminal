# 架构与所有权

> 规定 application → core 的依赖方向、GPUI 根实体、后台任务和跨域服务单点。

## 决策与红线

### crate 边界

目标 workspace 由 `architecture/dependencies.toml` 机械约束，允许的内部依赖方向为：

```text
slterm_path                 纯路径、授权判定与 AppPaths 值
slterm_document             文本、编码、差分、语法、Markdown 投影、公式语义
slterm_split                不含产品 ID 的泛型布局树与几何纯函数
slterm_settings             设置与主题值、字段注册表；零生产依赖
slterm_completions          方言解析、匹配和编辑计算
slterm_terminal             VT、网格、ConPTY、Shell 语义事实
slterm_agents               AgentCatalog、会话身份、命令构造和原生记录解析
slterm_hooks                Hooks 目录、schema、草稿模型和托管识别
slterm_runtime_protocol     Serde 线缆 DTO 与 schema 生成；不依赖内部领域 crate
slterm_workspace            稳定 ID、ViewKey、布局 DTO、关闭与恢复状态机
slterm_sidebar              侧栏与文件树的纯状态、过滤和选择模型
slterm_hook                 零依赖小进程桥
slterm_app                  全部 GPUI、I/O、子进程、网络、OS 与服务编排
```

- `slterm_workspace` 可依赖 path、document、split；`slterm_agents` 可依赖 path、settings、workspace；`slterm_sidebar` 可依赖 workspace、agents；`slterm_hooks` 可依赖 workspace、agents。其余领域 crate 不得为取得类型而反向依赖 app。
- core 只持有纯数据、纯状态机和可独立测试的解析／计算。文件、网络、SQLite、注册表、凭据、系统剪贴板、进程与 watcher 都经 application 端口适配。
- GPUI、winit、glutin 与平台渲染框架不得进入 core 或 hook 生产依赖。CPU 公式布局和不可变计算结果可留在 core；图片解码、位图呈现和网络留在 application。
- `slterm_split` 不定义跨领域 `PaneId`／`TabId`。`slterm_workspace` 以泛型布局树包装自己的 PanelGroupId 与 PanelId，避免 ID 循环依赖。
- ID 所有权单向分配：`slterm_document` 定义 DocumentId、TextStateId、RepositoryBinding 与 DiffBinding；`slterm_workspace` 定义 ProjectId、PanelGroupId、TerminalId、SideViewId 与 ViewKey；`slterm_settings` 定义 ThemeId、AgentRootId 及配置值；`slterm_agents` 定义 AgentKind 与 AgentSessionKey。不得为跨 crate 取类型而反向成环。
- settings、split、hook 保持零生产依赖；settings 的 JSON 值层和 ThemeId／AgentRootId 的 UUID 值解析在本 crate 内实现，UUIDv7 随机源由 application 提供；runtime_protocol 只为线缆 DTO 和 schema 引入经批准的依赖。

### AppRoot 所有权

`AppRoot` 是唯一窗口根 Entity，逻辑构成为：

```text
AppRoot
├─ ProjectRegistry / ProjectView[ProjectId]
├─ DocumentRegistry / DocumentSession[DocumentId]
├─ TerminalRegistry
├─ SideViewHost
├─ AppSettingsOverlay?
├─ CommandRegistry / FocusManager
├─ TaskService / OperationService
└─ AppServices
   ├─ PathAuthority / PathResolver
   ├─ PersistenceCoordinator
   ├─ StartupCoordinator / CloseCoordinator
   ├─ LocalControlServer
   └─ 其余领域服务与平台端口
```

- 禁止模块级可变全局状态。不可变语言目录、命令目录和内置目录可编译期构造；运行值必须有明确 Entity 或 worker 所有者。
- `ProjectView` 只持 ProjectLayout、项目运行态和聚焦位置，不复制应用级 Document、Terminal 映射或设置。
- `PanelHost` 只持 PanelId、ViewKey、面板实例和瞬态；持久展示参数只经 ViewStateStore。PanelId 不进入持久布局。
- `DocumentSession` 是前台 Entity，内含 core 文档状态与 I/O 代次。Rope 快照可克隆给后台，编辑、保存结果和生命周期决定必须回前台。
- `TerminalRegistry` 独占逻辑 Terminal 到 PTY 代次、进程身份和通道端点的映射；Panel 与 Runtime 只订阅或发命令。
- `SideViewHost` 拥有槽位和存活 SideView 实例。隐藏整个侧栏保留实例，关闭或同槽替换销毁实例，二者不得混用。

### 后台任务与结果代次

`TaskService` 的所有权树使用 `Application`、`Project`、`Document`、`Panel`、`SideView`、`Operation` 作用域：

- 每个任务登记所有者、取消令牌、输入代次和有界输出通道；所有者销毁时取消私有任务并回收句柄。
- 结果必须携带作用域、资源身份和输入代次。前台采纳前重新读取当前代次，旧结果只丢弃，不“补偿性”写回。
- 跨线程只传不可变结果或有界消息，不共享可变状态裸穿。GPUI 前台不得执行阻塞 I/O、网络、子进程等待或无界计算。
- 需要跨重启查询的副作用由 OperationService 接管并持久化；普通视图刷新不升级为持久 Operation。
- 显式保存、删除、恢复和更新可跨视图隐藏完成，其 Operation 结果与错误不得随视图销毁丢失。

### 命令、路径与错误单点

- `CommandRegistry` 注册封闭 CommandId、上下文、默认绑定、保留键和 application 命令。菜单、键盘、设置和 Runtime 都转换为同一 `CommandRequest`；Runtime 不模拟按键。
- 上下文顺序为模态／临时面 → 补全或菜单 → Panel／SideView → 全局 → 终端原始输入。IME 组合期间不派发命令。
- `FocusManager` 使用带实例代次的 FocusToken 和逻辑父链；非模态内容不抢焦。焦点、选区、搜索、菜单和录制态不持久化。
- `slterm_path` 只提供纯授权判定。application `PathAuthority` 持根与会话授权，`PathResolver` 负责词法、规范路径和文件身份；敏感操作执行前重新解析。
- 路径请求统一为 `PathRequest { operation, lexical_path, root_hints, session_grant, expected_authorization_generation }`，判定结果包含规范路径、文件身份、能力集合和持久性。能力位至少区分读取、写入、终端 cwd、启动恢复和声明的集成操作，不能用一个布尔“已授权”替代。
- 授权适配器按来源生成最小能力：ProjectRoot 可提供项目读写、终端和恢复；RepoRoot 只提供 Git／相关文件读取；AgentRoot 默认只读，写入与终端分别显式开启；根外显式读取、另存为和导出目标只产生当前会话或当次操作的单文件能力；内置 CLI 目标只产生声明文件所需能力。嵌套根取并集，删除根递增授权代次。
- 自动启动读取只接受持久授权；Markdown 根外文件链接的用户显式点击可产生该文件会话级只读授权，但不派生资源、目录或终端能力。Runtime 与第二实例不因调用来源获得额外能力。
- 设备路径和盘符相对路径不作为隐式授权输入，UNC 只有真实目标获准才可使用。最终句柄解析、链接重定向和授权代次在保存、删除、回滚、粘贴及恢复执行前重新检查。
- 错误信封使用封闭领域 kind、操作、资源身份、可重试性与受限详情。不得把任意底层错误文本、正文或凭据插值进日志；安全写与哈希对账只走 `tracing` target `audit`。
- 一切持久化写入由对应所有者串行执行；文件采用临时文件＋rename，SQLite 采用事务。禁止领域模块自建第二套读取、执行或写盘通道。

## 验收与测试

- `scripts/check_architecture.py` 的 workspace 键集必须与根 Cargo workspace 全等；增加依赖方向负例，拒绝 core → app、渲染框架入 core 和未登记 crate。
- 所有权测试反复创建、隐藏、关闭 Project、Panel、SideView、Document 和 Terminal，断言 watcher、线程、通道和句柄均随所有者回收。
- 假端口测试覆盖任务完成于所有者销毁之后、输入代次变化之后和关闭开始之后，结果不得回填。
- GPUI 虚拟窗口验证所有入口最终形成相同 CommandRequest，Runtime 不经过键盘事件或焦点伪造。
- 静态检索禁止模块级 `static mut`、无所有者的线程 spawn、前台阻塞调用和直接持久化写盘；必要平台例外必须收在平台封装模块。

## 操作

- 新增 crate 时同步根 Cargo workspace、`architecture/dependencies.toml`、架构测试和最近路径的 CLAUDE.md。
- 新增后台工作时先登记 TaskService 所有者、取消与结果代次，再实现执行体；不得先以游离线程验证后补所有权。
- 新增持久对象时先确定其权威表或文件、事务边界和备份资格，再接入对应 Store。
