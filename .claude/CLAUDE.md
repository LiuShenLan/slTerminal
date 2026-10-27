# CLAUDE.md

本文件是项目级常驻指令。模块细节遵守目标文件路径上最近的 `CLAUDE.md`；非常驻内容按文末「文档地图」按需加载。

## 项目定位与不可违背约束

**slTerminal** — 面向 Windows 10/11、面向所有 AI CLI 调优的通用终端模拟器。

- Windows 原生跑 AI CLI（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 AI CLI 取消）。

## 开发取向（未来最优）

长期迭代、无 deadline 项目：所有开发决策以**未来最优**为准——宁要未来的正确与简洁，不迁就历史包袱与既有妥协。权衡取舍时不计成本项：

- **git 历史**：允许大重构/重写/改名，不因破坏历史脉络而否决。
- **改动工作量**：允许为长远正确投入短期成本，不因「改动量大」否决。
- **现状兼容**：允许 breaking change——DTO/接口/布局/存储形态一律按理想终态重设计，不做兼容过渡。

适用边界：

- **仅限已批准的改动范围**：本原则是「改动该怎么做」的取向，不构成「发现问题就动手改」的授权（→ 协作纪律）。
- 对未来新决策生效；既有登记妥协不自动作废，但重审或提案一律以本原则为准，「动现状成本高/风险大」不再构成否决理由。
- **尽量不要豁免**：新决策与既有妥协的重审一律优先按理想终态修复；能修则不豁免、不登记妥协。豁免只保留给真正不可消除的边界（产品定位约束、操作系统/第三方限制、不可自动化面），且须写明根因与兜底层级。
- 不推翻：产品定位约束、用户已明确否决的方向、安全与正确性纪律（硬性开发约束、测试覆盖门禁）。

## 协作纪律

AI 拥有执行权，用户拥有**改动决策权**。任何任务（探索、开发、梳理逻辑、测试、review）中发现的 bug、可优化点、历史缺陷，一律**先报告、不擅自修**——由用户决定是否修、怎么修、何时修；擅自改动会污染用户正在进行的工作、打断专注。

- **唯一例外**：修复自己本轮刚改坏的东西（编译失败、自己引入的测试红灯）——属当前任务的组成部分，可直接修，但须在汇报中说明。
- **报告形态**：不打断进行中的任务，在当前任务交付点汇总；每条含定位（`file:line`）、现象、影响面、建议修法。若该问题**阻塞**当前任务（继续做会白做），立即报告并停下等指令。
- **不落盘**：口头汇报即可，不新建 backlog/清单文件；是否登记由用户当场裁决。

## 文档规范

- **代码自证**：凡能通过阅读代码直接理解的信息一律不写入文档——模块职责、文件清单、入口路径、接口签名、数据流、技术栈、现状描述。文档只记代码无法自证的信息：设计决策与原因、外部依赖的坑与红线、操作指令、约定与豁免登记。
- **快照数字禁令**：代码可自证的条目计数（文件数、行数、测试数、主题数、agent 数等）禁止入文档；契约数值（超时/容量/阈值/尺寸）、证据锚点（实测日期/commit hash/上游版本号）与结构性概念名不在此列。
- **行号引用禁令**：本仓源码/测试行号禁止引用——源码引符号名、测试引用例名；外部 URL 的锁定行锚点允许。
- **短标识符禁令**：禁止阶段号、批次/条目号、决策号、规则号、文档编号与测试分层编号进入文档、注释或代码符号名。原因：过程性编号会随阶段结束腐化。保留 commit hash、日期/状态锚、契约数值与领域术语。
- **统一风格**：项目文档使用固定三段式——一级标题 + 一句话职责 + `## 决策与红线` + `## 验收与测试` + 按需的 `## 操作`；决策条目写「规则 / 原因 / 边界」。模板与细则见 `.claude/docs/operations/doc-style.md`。产品文档保留自身体裁，同样遵守内容规则。
- **渐进式披露**：规则按目录分层；读取某目录代码时自动加载该路径 `CLAUDE.md`。跨模块且非常驻的内容放 `.claude/docs/<子域>/`，不平铺在 `.claude/` 根下；根文件只留路径指针。
- **收录判定**：跨多个模块适用、或每次会话必需的指令入本文件；仅触碰某模块才需要的归该模块子路径 `CLAUDE.md`。
- **子文件维护**：新建模块目录时同步创建该路径 `CLAUDE.md`；改动约定/决策/红线时同步对应文档。只改实现不改约定时无需动文档。
- **决策就近记录**：当前有效决策写入目标代码路径最近的 `CLAUDE.md`；跨模块决策写入 `.claude/docs/` 对应子域。不集中维护 ADR 或因果档案；完整历史从 git log 查。
- **事实文档防腐**：只记经核验的当前事实；计划、待办、临时记录不入文档；表达演进必须带版本/日期/状态锚，禁无锚点的「以前/现在/不再」。

## 架构与依赖

**单进程、单窗口：GPUI 壳拥有全部 UI 状态与 OS 访问；无 WebView、无 IPC、无 Node 运行时。** 依赖方向 = application → core；`slterm_hook` 是独立小进程桥，不被 app 依赖。方向由 `architecture/dependencies.toml` 机械强制。

workspace 成员、ownership、类型锚点、错误信封、数据目录与命名等跨模块架构决策见 `.claude/docs/architecture/overview.md`。

## 决策与红线（硬性开发约束）

- **依赖方向**：core 不依赖 app；渲染框架包禁入 core/hook 生产依赖；根 workspace 成员与 `architecture/dependencies.toml` 键集全等（`scripts/check_architecture.py` 机械强制）。
- **模块不穿透**：crate 内模块共享只经公开小接口/类型锚点；跨 crate 共享只经 core crate 公开 API；禁领域自建第二套读取/执行/写盘通道。
- **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在 PTY/平台封装模块；测试 `#[cfg(windows)]` 原则上改运行时 `cfg!(windows)` 分支。
- **GPUI 前台上限**：GPUI 前台线程禁止阻塞 I/O、网络、子进程等待；阻塞工作走 background executor 或专用线程，结果回前台更新状态并唤醒重绘。
- **线程与生命周期**：pane/PTY/窗口/托盘/watcher 的创建与销毁必须有显式所有权与 shutdown 路径；不留孤儿线程/句柄；跨线程只经有界通道，禁共享可变状态裸穿。
- **原子写**：一切持久化写盘 = 临时文件 + rename；损坏状态处理；设置写只经 `settings_store`/`audit` 包装，写核心函数私有。
- **凭据红线**：真实凭据值禁止写入任何 git 追踪文件——代码、测试夹具、文档、脚本一律不行；测试与文档仅允许假值占位符（`sk-test` 形态）。真实凭据只存 Windows 凭据管理器；token/key 不出凭据域，日志/错误消息禁插值。
- **审计通道**：安全写事件与哈希对账只经 `tracing` target `audit`；事件字段只含 `operation/path/source/result` 与 `expected/actual` 指纹，禁插值凭据值或文件内容。
- **配色单点**：颜色定义于主题/设置体系，组件经语义 token 引用，禁硬编码颜色；仅暗色。新增方案/色值须提供完整语义 token 映射；不做对比度校验、不提示、不自动回退。
- **布局单点**：会话布局只经 session schema 的 layout 转换对存取；禁第二套序列化或绕过快照。
- **面板封闭**：tab 与侧栏视图为封闭枚举（`WorkspaceTab`/`SideViewId`）；新增类型须在单一注册点登记，恢复时过滤未知类型。
- **测试覆盖**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在 `.claude/docs/testing/exemptions.md` 登记豁免原因与兜底层级，禁止未登记豁免。
- **门禁常挂**：提交前全部门禁全绿（架构/依赖、文档策略、禁名、clippy、fmt、测试）；禁名词表只允许 `slTerminal`/`slterm`，旧品牌名不得回流。

## 验收与测试

- 全量 cargo、core release 与脚本钉测试串行执行。
- 定向测试必须走显式 `lib_tests` target；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 不可自动化面必须登记豁免。
- 提交前全部门禁全绿；详细分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 构建：`cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook`；验收用 `target/debug/slterm.exe` 实测，不走交互式 dev 模式。
- 全量测试：`cargo test --workspace -- --test-threads=1`。
- core 发布模式测试：`cargo test --release --workspace --exclude slterm --exclude slterm_hook -- --test-threads=1`。
- 定向测试：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`。
- 静态检查：`cargo clippy --workspace --all-targets`；`cargo clippy -p slterm --all-targets --features gpui-test-support`；`cargo fmt --all -- --check`。
- 门禁：`python scripts/check_architecture.py`；`python scripts/check_doc_policy.py full`；`python scripts/check_brand_regress.py staged`。
- 脚本门禁自测：`python -m unittest discover -s scripts/tests -p "test_*.py"`。
- git hooks 启用（一次性）：`git config core.hooksPath .githooks`。
- 完整命令矩阵见 `.claude/docs/operations/commands.md`。

## 发布

正式发布只走 `scripts/release.ps1 -Version <X.Y.Z>`；发布链、清单合同、体积预算与合规落点见 `.claude/docs/release/packaging.md`；发布前按 `scripts/release-checklist.md` 人工核验。

## 文档地图

- `CONTEXT.md` — 领域术语表。
- `.claude/docs/architecture/overview.md` — 架构总览、workspace ownership、类型锚点、错误信封、数据目录。
- `.claude/docs/architecture/provenance.md` — 上游基线、fork 与旧栈删除沿革。
- `.claude/docs/testing/strategy.md` — 测试分层、隔离、串行、命名与防复发。
- `.claude/docs/testing/exemptions.md` — 不可自动化测试面与条件跳过登记。
- `.claude/docs/release/packaging.md` — 发布链、体积预算、合规与首发核验证据。
- `.claude/docs/operations/commands.md` — 完整命令矩阵。
- `.claude/docs/operations/doc-style.md` — 统一文档模板与维护规则。
- `release-notes/` — 按版本发布说明。
