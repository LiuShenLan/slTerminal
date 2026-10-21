# 测试策略

> 目标态重建的测试分层、夹具、验收追溯、旧测试处置与门禁；实现级用例以代码为准。

### 决策与红线

- **显式测试 target**：每个 crate 使用 `[lib] test = false` + `[[test]] name = "lib_tests" path = "src/lib.rs"`；定向测试只走 `--test lib_tests`，禁止 `--lib` 或裸 filter 直打默认 lib target。原因：默认 lib test target 收不到测试专用链接参数，历史上表现为零输出崩溃。
- **分层职责**：

| 层 | 载体 | 覆盖对象 |
|---|---|---|
| 领域纯函数 | `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol` 的 `lib_tests` | 四层模型、布局树、焦点、去重、标题、序列化 DTO、恢复对账、侧栏状态机、Commit 投影、Agent 过滤、Hooks schema、Runtime 信封 |
| 持久化与服务 | app 或 core 的集成 target | 状态库事务、备份/完整性、损坏隔离、去抖、flush coordinator、关闭链、文件监听、Git 执行体、CLI 适配器 |
| 虚拟窗口 | `gpui-test-support` + `TestAppContext`/`VisualTestContext` | 壳、面板组、页签、侧栏、菜单、对话框、焦点、关闭守卫、设置中心、通知 |
| 真实进程/conformance | 独立集成 target | PTY 往返、Runtime v2、单实例移交、发布交接 |
| 人工验收 | 实机清单 + 豁免登记 | 真实输入法、GPU/字体观感、拖拽手感、系统通知/托盘/回收站、真实 CLI/凭据 |

- **全量串行**：`cargo test --workspace -- --test-threads=1`；性能与 PTY 容量断言对 CPU 负载敏感。
- **隔离优先**：`tempfile::tempdir()` 与 `SLTERM_CONFIG_DIR`/`SLTERM_LOCAL_DIR` 隔离文件系统；进程级资源用 `SPAWN_LOCK`/`ENV_LOCK`；UI 测试用虚拟窗口与隔离状态根。生产 `%APPDATA%`/`%LOCALAPPDATA%` 永不被测试触碰。
- **覆盖门禁**：改动代码的可自动化部分必须有全量测试；不可自动化面在 `.claude/docs/testing/exemptions.md` 登记根因与兜底层级，禁止未登记豁免。
- **键盘与输入保留**：键盘路径、焦点环、IME/CJK、减少动效作为普通交互测试；无障碍专用能力已删除，只保留“无入口/无设置键/无快捷键”的负向断言。
- **Mermaid 源码显示**：测试断言 `mermaid` 围栏按代码块显示源码、不执行内容、不生成图；保留渲染接口的类型测试，不测图布局。

### 夹具

```rust
pub struct WorkspaceHarness {
    pub workspace: Entity<Workspace>,
    pub clock: Arc<ManualClock>,
    pub store: Arc<TestStateStore>,
    pub launcher: Arc<FakeTerminalLauncher>,
    pub cli: Arc<FakeCliRegistry>,
    pub handoff: Arc<RecordingInstanceCoordinator>,
}
```

- **手动时钟**：驱动 2 秒/500ms/300ms/立即语义、关闭链超时、通知倒计时、撤销窗口；测试禁止真实 `sleep`。
- **故障注入**：`FaultPlan` 覆盖读失败、备份失败、临时文件半写、替换失败、写超时、迟到完成、磁盘满、只读目录。
- **PTY 替身**：确定性输入输出、容量上限、进程退出、尺寸变化、UTF-8 边界。
- **假 CLI 适配器**：hook 事件、历史会话扫描、恢复/分叉命令、用量/余量；排除真实 CLI 与网络。
- **虚拟窗口**：多项目布局、隐藏项目、页签溢出、焦点循环、IME 组合态、覆盖层栈。

### 分域测试要点

| 域 | 必测 |
|---|---|
| architecture/domain | `ProjectId` 稳定；根路径唯一与真实路径查重；布局树叶=组集合；空组移除；面板不跨项目；去重键；标题消歧；焦点转移；恢复对账幂等 |
| persistence | DTO 往返；`user_version` 拒收；去抖阈值；事务原子性；facet 合并；备份失败中止；损坏隔离；关闭链三阶段；快照 64 MiB/256 MiB/7 天；旧文件不读 |
| gpui-runtime | Entity 拓扑；订阅释放；异步结果 generation；隐藏项目不布局/不绘制；面板 `catch_unwind` 错误边界；关闭状态机防重入 |
| ux-framework | 命令层序；保留键；同上下文冲突；焦点意图令牌；覆盖层入栈/出栈；`Esc` 顺序；IME 门控；项目切换焦点恢复 |
| security | 路径决策表（绝对/相对/设备/UNC/符号链接/联接）；URL/OSC 白名单；Markdown 清洗；进程启动参数；凭据不出域；诊断脱敏 |
| services/filesystem-git | 仓库根解析；status 解析；HEAD 读取；监听去抖；外部修改复核；回收站/永久删除回退；写门串行 |
| integrations/agent-cli | AgentKind 能力表；hook 事件归一化；会话扫描合并；最深前缀归属；删除隔离区/撤销/永久化；余量来源解析 |
| integrations/hooks | 目录事件/handler 校验；未知字段保留；托管条目只读；备份失败中止；外部修改冲突；注入/卸载提交点 |
| integrations/runtime-api-v2 | 信封/错误码；资源 ID `project:/group:/panel:`；快照重建；命令幂等；关闭/移除守卫；并发与过期结果 |
| workspace/shell | 项目创建/切换/移除/重定位；布局拆分/合并/拖拽；页签溢出；脏状态与批量保存；关闭链；无项目应用浮层；单实例载荷 |
| workspace/navigation | 项目 CRUD；历史归属；搜索；刷新；删除撤销；套餐余量；状态作用域 |
| sidebar | 双槽状态机；按钮显隐/换区/排序；快捷键状态机；宽度/比例；视图状态清洗；树多选/剪贴板/文件操作；Commit 四态；Agent 节点探测/过滤 |
| panels/terminal | 单 PTY；编号/cwd；Shell 探测；OSC 8/52；粘贴守卫；PTY 上限 64；重连；关闭守卫 |
| panels/editor | 打开路由；编码/行尾；保存/冲突；查找替换；折叠；语言覆盖；自动保存；只读；外部修改；gutter 三类标记；崩溃快照 |
| panels/markdown | 两态；草稿；CommonMark/GFM；安全 HTML；公式；Mermaid 源码；链接/资源；查找/大纲/缩放；草稿快照 |
| panels/diff | 行对对齐；并排/行内；工作区侧编辑；同步滚动；HEAD 刷新；外部重命名；冲突快照；大文件/二进制 |
| panels/gitshow | HEAD 只读；刷新/过期；大文件/二进制；临时文件清理；去重 |
| settings | 页面注册；导航/搜索；三保存模式；快捷键固定键参考；应用页；无项目浮层；导入导出/恢复默认；Hooks；只读/损坏 |
| theme/i18n | token 消费；未知主题 id 回退 `Linear`；密度/透明度/壁纸；i18n 键集合同（迁移能力）；负向断言不做对比度校验、不提示、不静默改写用户设置 |
| platform-windows | 单实例锁与载荷；窗口几何夹取/DPI；通知/任务栏闪烁；托盘/开机启动；回收站；默认程序；进程树终止 |
| diagnostics/build | 日志脱敏；审计字段；panic；诊断包；一键清理；包清单/体积预算；代码签名（有证书时） |

### 验收追溯

| featureSpec 篇目 | 设计所有者 | 里程碑 | 验收场景 | 测试层/目标 |
|---|---|---|---|---|
| `workspace/shell.md` | `design/workspace/shell.md` | 壳与工作区 | 项目生命周期、布局拆分/合并、焦点、关闭链、单实例移交 | 领域 + 虚拟窗口 + 真实进程/conformance |
| `workspace/navigation.md` | `design/workspace/navigation.md` | 侧栏框架 + 导航核心；导航历史与余量 | 项目 CRUD/排序/重命名/移除、历史会话、套餐余量 | 领域 + 虚拟窗口 + 假 CLI |
| `panels/terminal.md` | `design/panels/terminal.md` | 终端面板 | PTY 生命周期、编号/cwd、OSC 8/52、关闭守卫 | 终端 core + PTY 集成 + 虚拟窗口 |
| `panels/editor.md` | `design/panels/editor.md` | 编辑器面板 | 打开路由、保存冲突、AgentRoot 可写、gutter、崩溃快照 | 引擎单测 + 文件集成 + 虚拟窗口 |
| `panels/diff.md` | `design/panels/diff.md` | Diff 面板 | 行对、冲突快照瞬态、保存并发、`mode/source` 去重 | 引擎单测 + 虚拟窗口 + 文件集成 |
| `panels/gitshow.md` | `design/panels/gitshow.md` | GitShow 面板 | HEAD 只读、刷新/过期、大文件/二进制 | 引擎单测 + 集成 |
| `panels/markdown.md` | `design/panels/markdown.md` | Markdown 面板 | 两态、安全 HTML、Mermaid 源码、草稿恢复 | 引擎单测 + 虚拟窗口 |
| `panels/settings.md` | `design/panels/settings.md` + `design/settings/app-page.md` | 设置中心壳与基础页；设置中心 Agent/Hooks/后台任务页 | 页面注册、保存模式、固定键参考、无项目浮层 | 纯函数 + 虚拟窗口 + 设置写通道 |
| `sidebar/framework.md` | `design/sidebar/framework.md` | 侧栏框架 + 导航核心 | 双槽、显隐、视图快捷键、视图状态矩阵 | 领域纯函数 + 虚拟窗口 |
| `sidebar/explorer.md` | `design/sidebar/explorer.md` | 文件浏览器 | 树/多选/批量/剪贴板、写边界、监听 | 模型 + 文件系统集成 + 虚拟窗口 |
| `sidebar/commit.md` | `design/sidebar/commit.md` | Commit 视图 | 四态、ignored、打开映射、回滚/删除 | 模型 + Git 集成 + 虚拟窗口 |
| `sidebar/agent-files.md` | `design/sidebar/agent-files.md` | Agent 全局文件 | 节点探测、过滤、监听、写边界 | 模型 + 文件系统集成 + 虚拟窗口 |
| `settings-and-persistence.md` | `design/persistence.md` | 状态库与设置存储；壳与工作区 | DTO 往返、去抖、事务原子性、崩溃恢复、关闭冲刷 | 状态库集成 + 故障注入 |
| `cross-cutting.md` | `design/cross-cutting.md` 与各所有者 | 应用基础服务；跨域收敛与规格验收 | 错误/通知/焦点/性能/后台任务/安全边界 | 跨域矩阵 + 虚拟窗口 |
| `acceptance.md` | `design/testing.md` | 跨域收敛与规格验收 | acceptance 全通过、范围外负向、门禁全绿 | 门禁 + 人工核验 |
| `assets/README.md` | `design/build-and-release.md` | 构建与打包基线 | 资产名/许可/第三方声明随包；清单三处一致 | 包合同钉测试 + 资产核验 |

### 范围外与删除能力

范围外迁移能力按里程碑正向验收；删除能力只做负向断言，不建正向测试。

| 范围外能力 | 里程碑 | 正向验收 | 迁移前边界 |
|---|---|---|---|
| Runtime v2 | 控制面协议核心；Runtime API v2 完整面 | conformance、幂等、`runtime.handover`、无 `window.*` 方法 | 应用页只读运行状态/不可用；无 v1 兼容层 |
| 应用页范围外节 | 应用页范围外节落地与壳服务 | 界面语言、更新、托盘与开机启动、Providers/助手、备份、终端增强逐项接线 | 只读说明 `该能力由迁移版本提供`；终端增强禁用态 `尚未启用` |
| AI Providers/助手 | AI Providers/助手 | 元数据 CRUD、凭据门面、脱敏、关闭后不发请求 | 无入口、无设置键 |
| 终端增强 | 终端增强回迁 | 默认值、按键顺序、取消、缓存按面板 | 禁用态，不拦截按键 |
| 加密备份 | 加密备份 | 加解密/校验/回滚、类别白名单、旧格式拒绝 | 无导出/恢复入口 |
| 更新与发布链 | 更新与发布链 | 四重校验、handoff、签名、发布门禁 | 无更新入口，无静默启动 |

| 删除能力 | 负向断言 |
|---|---|
| Image 面板 | 无面板类型、入口、快捷键或设置键；恢复未知类型时丢弃 |
| Merge 三栏 | 无 Merge 入口或快捷键；冲突来源只打开 Diff 只读快照；恢复丢弃 |
| 用户主题 | 无导入/导出/编辑器入口与设置键；未知主题 id 回退 `Linear` |
| Markdown 实时块编辑 | 渲染态无输入、无块编辑命令、无 live edit 状态或设置键 |
| 提示符跳转 | 无 API、无按键、无文档入口 |
| 无障碍专用能力 | 无屏幕阅读器/ARIA/播报/高对比度/大字体入口、快捷键或设置键 |
| 对比度校验 | 无校验、无提示、无自动回退、无设置键；主题与透明度/壁纸合成后不静默改写用户设置 |
| Mermaid 渲染 | 围栏代码块按源码显示；无图生成入口或设置键 |
| 外部文件拖入 | 无外部拖入处理或设置键；仅文件浏览器内部拖到编辑器面板组 |
| Runtime v1 兼容层 | 无 v1 方法、schema 或端口格式；不读旧端口文件 |

### 旧测试处置

- 旧 `workspace`/`tab`/`pane`/`dock`/`session` 相关测试在“旧壳退役”里程碑随旧代码一次性删除；删除前建立「旧测试 → 规格条目/新测试」映射，避免行为知识丢失。
- 保留引擎的测试随引擎迁移：`slterm_terminal`、`slterm_settings`、`slterm_completions`、`slterm_split` 的既有测试保留并适配新接口。
- 旧 UI 测试（标签页、分屏、侧栏固定槽位、设置单例）不迁移；对应行为由新的项目/面板组/侧栏/每项目设置测试覆盖。
- 旧 Runtime v1 conformance 删除；新建 v2 conformance，覆盖 `project:/group:/panel:` 资源、移交载荷与单实例。

### 豁免与人工验收

- 沿用 `.claude/docs/testing/exemptions.md` 单表；新增项在实现阶段登记。
- 已知不可自动化面：真实中文输入法候选窗、GPU/字体渲染观感、拖拽手感、系统通知/托盘/回收站、真实凭据管理器、真实 CLI hook 事件、安装器覆盖安装。
- 键盘路径、焦点环、IME/CJK、减少动效仍自动或半自动覆盖；无障碍专用能力已删除，不登记相关豁免。

### 门禁

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo clippy -p slterm --all-targets --features gpui-test-support
cargo test --workspace -- --test-threads=1
python scripts/check_architecture.py
python scripts/check_doc_policy.py full
python scripts/check_brand_regress.py staged
python -m unittest discover -s scripts/tests -p "test_*.py"
```

- 新 crate 必须登记 `architecture/dependencies.toml` 与 `architecture/file-budgets.txt`，并带显式 `lib_tests` target。
- 提交前全部门禁全绿；定向测试串行执行。
