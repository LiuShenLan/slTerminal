# slTerminal 功能需求规格

> 定义完整产品能力的范围、领域模型与文档导航；具体行为、边界和验收由所有者文档给出。

## 产品范围

slTerminal 是面向 Windows 10/11、面向 AI CLI 的通用终端模拟器：Windows 原生运行，不走 WSL；单进程、单窗口、单实例；仅暗色；GPU 加速渲染；默认 Shell 按 PowerShell 7、Windows PowerShell、`cmd.exe` 顺序回退。

本规格覆盖终端、文档编辑、Markdown、Diff、GitShow、项目与布局、导航与历史会话、文件树、Commit、Agent 文件、设置与 Hooks、命令补全、Runtime、加密备份及系统集成。它描述产品契约，不描述当前实现、开发阶段、兼容迁移或详细协议字段。最终排除项集中在 [验收与豁免](acceptance.md)。

## 领域模型

```text
Application
├─ Document[DocumentId]                 应用级实时文档，可跨项目被引用
├─ Project[ProjectId]
│  └─ ProjectLayout
│     └─ PanelGroup
│        └─ Panel[PanelId] ──引用──> Document
└─ ViewState[ViewKey]                   逻辑视图的持久状态
```

| 概念 | 身份与职责 |
|---|---|
| Project | 项目容器；`ProjectId` 稳定，根路径只是位置与授权来源。 |
| ProjectLayout | 每个项目恰好一个网格布局；多项目同时存活，切换只改变可见性。 |
| PanelGroup | 布局叶节点与页签容器；空组自动移除；面板不得跨项目移动。 |
| Panel | 存活视图实例；`PanelId` 在其生命周期内稳定，关闭后不复用。 |
| Document | 应用级可编辑聚合；`DocumentId` 独立于项目与路径。同一文档只有一份实时文本、脏状态、撤销、冲突与外部修改状态。 |
| ViewKey | 逻辑视图身份，由稳定领域标识组成；持久视图状态绑定它，而不是绑定已销毁的 `PanelId`。 |
| PathPolicy | 按操作判定读、写、终端 cwd、资源读取与恢复授权；路径本身不授予能力。 |
| Terminal | 逻辑终端记录；元数据恢复与 PTY 创建分离，显示编号不是身份。 |
| AgentRoot | Agent 文件与终端能力的授权根；内置和自定义根默认只读且禁止终端。 |

面板类型为封闭集：Terminal、Editor、Markdown、Diff、GitShow、Settings。侧栏视图也为封闭集；未知类型恢复时过滤并聚合提示，不创建兼容替代类型。

## 全局状态规则

- 同一逻辑对象只有一份持久视图状态，关闭重开与应用重启共用；重开可创建新的 `PanelId`，但按 `ViewKey` 恢复状态。
- Document 的实时文本与撤销只在存活期共享；正常关闭或退出不借持久视图状态恢复未保存正文。异常退出按崩溃恢复契约处理。
- 阅读位置、语言覆盖、Markdown 形态与缩放、Diff 显示参数、设置选中页等是否持久化，由各所有者逐项声明。
- 搜索、多选、内联编辑、菜单、通知、错误、焦点等瞬态在关闭重开与重启时都清除。项目切换对项目内 Panel 只改变可见性，不清空其运行态；应用级侧栏按各自数据根规则刷新并清理瞬态。
- 项目移除先经过统一守卫，再删除该项目的布局与 `ViewKey` 状态；不得遗留可恢复的重复面板。
- 同一契约只由一篇文档拥有；其他文档链接并验证集成行为，不复制数值、状态矩阵或例外。

## 文档地图

### 跨域契约

- [路径与授权](path-policy.md)：路径身份、能力并集、显式授权、根重定位与启动恢复。
- [跨域约定](cross-cutting.md)：错误、命令目录、焦点与输入、后台生命周期、安全隐私与资源约束。
- [设置与持久化](settings-and-persistence.md)：应用设置单例、保存队列、持久状态、损坏分流与崩溃快照。
- [加密备份与恢复](backup-and-recovery.md)：备份类别、导出、整包恢复、恢复点与指纹撤回。
- [Runtime v2](runtime.md)：本机控制面、凭据边界、非交互写入与结果语义。
- [系统集成](system-integration.md)：托盘、启动、窗口几何与材质、更新、语言、系统通知、任务栏和诊断。
- [验收与豁免](acceptance.md)：验收规则、跨域场景、最终排除项、平台边界与人工核验。

### 工作区

- [工作区壳](workspace/shell.md)：项目布局、面板组、面板、`ViewKey`、焦点、关闭状态机与单实例意图。
- [导航树与会话](workspace/navigation.md)：项目操作、活跃与历史会话、隔离区撤销及套餐余量。

### 面板

- [Document](panels/document.md)：文档身份、共享缓冲、保存、自动保存、外部修改与最后引用守卫。
- [编辑器](panels/editor.md)：文本编辑、编码、搜索替换、语言、大文件与 Git 行标。
- [Markdown](panels/markdown.md)：源码与渲染两态、链接资源、安全 HTML、公式、Mermaid 与阅读状态。
- [Diff](panels/diff.md)：HEAD 与工作区对比、共享工作区 Document、只读快照与差异导航。
- [GitShow](panels/gitshow.md)：固定仓库、版本和路径的只读版本内容。
- [终端](panels/terminal.md)：PTY、Shell、输入输出、选择、搜索、恢复及命令/Git 补全。
- [设置中心](panels/settings.md)：设置视图、命令目录、主题、后台任务、Agent 与 Hooks。

### 侧栏与资产

- [侧栏框架](sidebar/framework.md)：活动栏、双槽、显隐、关闭与持久状态。
- [文件浏览器](sidebar/explorer.md)：项目文件树、文件操作、过滤与 Git 状态。
- [Commit](sidebar/commit.md)：仓库变更、打开映射、受限回滚与删除。
- [Agent 全局文件](sidebar/agent-files.md)：统一 Agent 根、能力开关与文件操作。
- [资产说明](assets/README.md)：资产用途与许可边界。

## 维护规则

- 修改行为时同步所有者需求与验收；跨域场景只在 `acceptance.md` 验证集成，不复制完整契约。
- 契约数值可写入需求；内部文件名、表结构、协议字段和实现模块不作为产品需求。
- 禁止本仓源码或测试行号、过程性编号、无锚点的阶段措辞与代码可自证的快照计数。
- 性能判据必须有可复现条件；不得以“流畅”“尽快”等主观措辞替代验收。
