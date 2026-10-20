# 开发计划

> 目标态重建的命名里程碑、依赖、交付物与验收映射；不写工期估算，不使用阶段编号。

### 原则

- **分层重建**：保留已验证引擎，重建项目/布局/面板领域与 app 壳；旧 workspace/tab/pane/dock/session 整体废弃。
- **规格先行**：`spec-issues.md` 的裁决先落回 featureSpec，再进入设计与实现。
- **垂直切片**：先打通「创建项目 → 拆分布局 → 终端 → 重启恢复 → 关闭守卫」，再铺开面板。
- **门禁前置**：规格冻结、垂直切片、跨域验收三道门；每道门不通过不进入下一批。
- **命名里程碑**：里程碑只用名称，不使用 M/N/阶段号。
- **过渡期允许不可编译/不可用**；旧模型隔离后新旧不并行运行。

### 里程碑

| 里程碑 | 交付物 | 依赖 | 验收映射 |
|---|---|---|---|
| 规格裁决与冻结 | `spec-issues.md` 关闭；featureSpec 修订；`CONTEXT.md` 术语切换 | — | 全部后续 |
| 领域模型与骨架 | `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`；ID、四层模型、纯转换、DTO、测试 target、架构登记 | 规格冻结 | shell/README 模型与不变量 |
| 应用基础服务 | 命令/焦点、通知/对话框/覆盖层、主题 token、持久化原语、Windows 平台薄层 | 领域模型 | cross-cutting、ux、platform |
| 设置存储与写通道 | 设置 schema、原子写/备份、去抖/冲刷、导入导出、只读/损坏 | 领域模型、持久化原语 | settings-and-persistence |
| 壳与工作区 | 启动/关闭、项目生命周期、布局操作、焦点、关闭守卫、标题栏、单实例、无项目应用浮层 | 基础服务、设置存储 | workspace/shell |
| 旧模型隔离 | 旧 app 冻结；旧数据/二进制/单实例命名空间隔离 | 壳 | migration |
| 终端面板 | PTY 生命周期、渲染、输入、链接、剪贴板、编号/cwd、恢复、关闭守卫 | 壳 | panels/terminal |
| 侧栏框架 + 导航核心 | 活动栏、双槽、视图注册；项目 CRUD/切换/排序/重命名/移除 | 壳、基础服务 | sidebar/framework、navigation |
| 垂直切片验收 | 项目→拆分→终端→重启恢复→关闭守卫 | 终端、导航核心 | 跨域验收子集 |
| Git 与文件服务 | 仓库根、status、HEAD、监听、外部修改、回收站 | 壳 | services/filesystem-git |
| CLI/Agent 集成核心 | AgentKind、hook 摄取、状态/用量、历史扫描 | 终端、设置存储 | integrations/agent-cli |
| 导航历史与余量 | 历史会话、恢复、套餐余量 | 导航核心、CLI 集成 | navigation |
| 编辑器面板 | 路由、编辑内核、Tree-sitter、gutter、外部修改、崩溃快照 | 壳、Git/文件服务、设置 | panels/editor |
| Markdown 面板 | 两态、渲染、公式、Mermaid 源码、草稿恢复 | 编辑器 | panels/markdown |
| 文件浏览器 | 树、多选、批量、剪贴板、Git 状态、过滤、监听 | 编辑器、Markdown、Git/文件服务 | sidebar/explorer |
| Diff 面板 | 双栏/行内、冲突快照、保存冲突、同步滚动 | 编辑器、Markdown、Git 服务 | panels/diff |
| GitShow 面板 | HEAD 只读、刷新/过期、大文件/二进制 | Git 服务、面板壳 | panels/gitshow |
| Commit 视图 | 四态、分组、ignored、打开映射、回滚/删除 | Explorer、Diff、GitShow、文件路由 | sidebar/commit |
| 设置中心壳与基础页 | 导航、快捷键（固定键参考）、外观、终端、应用页 | 壳、设置存储、侧栏框架 | panels/settings、settings/app-page |
| 设置中心 Agent 页 | Agent 基础配置 | CLI/Agent 集成 | panels/settings |
| 设置中心 Hooks 页 | Hooks 引擎与 UI | integrations/hooks、设置壳 | panels/settings |
| 设置中心后台任务页 | 任务配置 | CLI 集成、设置壳 | panels/settings |
| Agent 全局文件 | 节点探测、统一树、过滤、监听、标题消歧 | Explorer、Agent 页、CLI 集成 | sidebar/agent-files |
| 跨域收敛与规格验收 | 验收矩阵、键盘/IME、性能、安全、门禁 | 全部功能里程碑 | acceptance |
| 旧模型删除与门禁重登记 | 删除旧 workspace/tab/pane/dock/session 与旧测试；依赖/预算重登记 | 收敛门 | migration、architecture |
| 应用页与壳服务 | 应用页落地、托盘/任务栏/开机启动/更新入口 | 收敛门 | out-of-scope/app-and-shell-services |
| Runtime API v2 | 协议、CLI、skill、conformance、审计 | 收敛门 | out-of-scope/runtime-api-v2、integrations/runtime-api-v2 |
| AI Providers/助手 | Providers、助手、命令修复、凭据 | 应用页 | out-of-scope/ai-providers-assistant |
| 终端增强回迁 | vi、终端补全、数学覆盖层、AI 修复条 | 终端面板、应用页 | out-of-scope/terminal-enhancements |
| 加密备份 | 备份/恢复/敏感键/UI | 持久化、应用页 | out-of-scope/encrypted-backup |
| 更新与发布链 | 更新链、交接、发布链、代码签名 | 应用页、构建发布 | out-of-scope/update-release |

### 依赖图

```text
规格裁决与冻结
  → 领域模型与骨架
    → 应用基础服务 / 设置存储与写通道
      → 壳与工作区
        → 旧模型隔离
        → 终端面板
        → 侧栏框架 + 导航核心
          → 垂直切片验收
          → Git 与文件服务
          → CLI/Agent 集成核心
            → 导航历史与余量
          → 编辑器面板
            → Markdown 面板
            → 文件浏览器
            → Diff 面板
            → GitShow 面板
              → Commit 视图
          → 设置中心壳与基础页
            → 设置中心 Agent / Hooks / 后台任务页
            → Agent 全局文件
              → 跨域收敛与规格验收
                → 旧模型删除与门禁重登记
                → 范围外能力迁移
```

### 并行工作流

- **领域与持久化**：领域模型、DTO、持久化协调器、设置存储可并行；共用 `spec-issues.md` 的裁决。
- **服务底座**：Git/文件服务、CLI/Agent 集成、Hooks 引擎在壳稳定后并行；均提供假实现供 UI 域先行。
- **面板族**：编辑器 → Markdown → Diff/GitShow 有依赖；Explorer 依赖编辑器与 Markdown；Commit 依赖 Explorer/Diff/GitShow。
- **侧栏与设置**：侧栏框架先于导航/Explorer/Commit/Agent 文件；设置壳先于 Agent/Hooks/后台任务页。
- **范围外迁移**：全部在收敛门后；应用页与 Runtime v2 先行，更新/发布链最后。

### 风险与缓解

| 风险 | 缓解 |
|---|---|
| 状态所有权与 GPUI 实体拓扑双源 | 所有权矩阵；领域纯数据 + 运行时注册表；面板只持句柄/ID；契约测试 |
| 新 core 依赖 GPUI 或 app 穿透 core | core 只暴露 ID/纯函数/port/DTO；依赖策略即时登记；每 port 假实现 |
| 多 owner 去抖与跨文件一致性 | 单一 `PersistenceCoordinator`；串行写 + generation；提交顺序与孤儿清理；故障注入 |
| 同步关闭钩子 vs 最长 9 秒关闭链 | `CloseState` 状态机；后台三阶段；完成后再销毁窗口；防重入 |
| 焦点与 IME 分散 | 中央焦点协调器；一次性意图；统一 IME 门控；虚拟窗口键盘测试 |
| 隐藏项目渲染与 panic 边界 | 隐藏子树不布局/不绘制；`catch_unwind` + poisoned；多项目基准 |
| 新旧 app 共用数据/单实例命名空间 | 新 `state/` 根、新二进制名、新 owner 锁；旧文件不读不删 |
| 旧测试删除导致知识丢失 | 删除前建立旧测试 → 规格/新测试映射；四类夹具先行 |
| CLI/Hooks/Agent 逻辑在 UI 复制 | 单一 Agent 能力注册表与 hook/session 执行体；UI 只消费 |
| Windows 平台面集中爆雷 | 平台薄接口 + 假实现；实机冒烟与豁免登记 |
| 范围外能力反向修改核心 | 领域阶段冻结 Runtime ID；壳/设置/终端预留扩展点 |
| Mermaid/无障碍范围变化 | 设计只保留源码显示与键盘/IME/减少动效；不实现已删除能力 |

### Cutover 与回滚

- 旧模型隔离后新旧不并行；旧数据不读、不删；新二进制/单实例命名空间独立。
- 每个里程碑独立提交；垂直切片与收敛门是回滚点。
- 旧模型删除前保留最后一次可运行构建标签；删除后只向前修复，不回滚旧模型。
- 范围外迁移在收敛后进行；迁移失败以功能开关/占位降级，不回退目标态核心。

### 追溯

- 每篇 featureSpec 在 `design/testing.md` 有设计所有者与测试层映射。
- `spec-issues.md` 记录每条规格修订与设计裁决；实现偏差必须先更新该文件并同步 featureSpec。
- 里程碑退出条件：交付物完成、对应验收判据通过、门禁全绿、旧代码/旧测试处置完成。
