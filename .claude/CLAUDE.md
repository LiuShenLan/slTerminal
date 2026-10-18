# CLAUDE.md

本文件是项目级指令。模块细节按目录分层：遵守目标文件路径上最近的 CLAUDE.md，模块细节不回填本文件。

## 项目性质

**slTerminal** — 面向 Windows 10/11、面向所有 AI CLI 调优的通用终端模拟器。

定位约束（贯穿全程，不可违背）：

- Windows 原生跑 AI CLI（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 AI CLI 取消）。

## 上游重构基线

本项目 fork 上游终端项目（Rust + GPUI 模块化单体终端），重构为单进程 GPUI 应用。

- **baseline commit**：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（上游 main，2026-10-04）。
- **增量参考**：在上游仓库 `git log e537d528..HEAD` 获取新改动逐个评审回采；不引入 remote/submodule。
- 旧 WebView 双进程栈已在 M0 整体删除，不与新 workspace 并存；git 历史保留提取来源。

## 开发取向（未来最优）

长期迭代、无 deadline 项目：所有开发决策以**未来最优**为准——宁要未来的正确与简洁，不迁就历史包袱与既有妥协。权衡取舍时不计成本项：

- **git 历史**：允许大重构/重写/改名，不因破坏历史脉络而否决。
- **改动工作量**：允许为长远正确投入短期成本，不因"改动量大"否决。
- **现状兼容**：允许 breaking change——DTO/接口/布局/存储形态一律按理想终态重设计，不做兼容过渡。

适用边界：

- **仅限已批准的改动范围**：本原则是「改动该怎么做」的取向，不构成「发现问题就动手改」的授权（→ 协作纪律）。
- 对未来新决策生效；既有登记妥协（ADR 关闭项、test-exemptions、豁免登记）**不自动作废**，但重审或提案一律以本原则为准，「动现状成本高/风险大」不再构成否决理由。
- 不推翻：产品定位约束（Windows 原生/单窗口/仅暗色/复制语义等）、用户已明确否决的方向、安全与正确性纪律（硬性开发约束、测试覆盖门禁）。

## 协作纪律

AI 拥有执行权，用户拥有**改动决策权**。任何任务（探索、开发、梳理逻辑、测试、review）中发现的 bug、可优化点、历史缺陷，一律**先报告、不擅自修**——由用户决定是否修、怎么修、何时修；擅自改动会污染用户正在进行的工作、打断专注。

- **唯一例外**：修复自己本轮刚改坏的东西（编译失败、自己引入的测试红灯）——属当前任务的组成部分，可直接修，但须在汇报中说明。
- **报告形态**：不打断进行中的任务，在当前任务交付点汇总；每条含定位（`file:line`）、现象、影响面、建议修法。若该问题**阻塞**当前任务（继续做会白做），立即报告并停下等指令。
- **不落盘**：口头汇报即可，不新建 backlog/清单文件；是否登记由用户当场裁决。

## 文档规范

- **代码自证原则（ADR-0011）**：凡能通过阅读代码直接理解的信息（模块职责、文件清单、入口路径、接口签名、数据流、技术栈、现状描述等）一律不写入文档——此类内容必然腐化为失真文档。文档只记录代码无法自证的信息：设计决策与原因（why）、外部依赖的坑与红线、操作指令、约定与豁免登记。
- **快照数字禁令**：代码可自证的条目计数（spec 数/文件数/事件数/名单项数/变体数等）禁止入文档——代码即真值，计数必腐化且无决策价值；契约数值（超时/容量/阈值/尺寸，本身即设计决策）、证据锚点（实测基准/日期/hash/上游版本号）、结构性计数（概念名组成部分，如「四层测试形态」）不在禁列。
- **行号引用禁令**：本仓源码/测试行号禁止引用（插行即腐）——源码引符号名、测试引用例名；上游 node_modules 行号与锁定版本共锚，允许。
- **渐进式披露**：规则按目录分层，遵守目标文件路径上最近的 CLAUDE.md；读取某目录代码时 Claude Code 自动加载该路径 CLAUDE.md，无需登记索引。
- **收录判定**：跨 ≥2 个模块适用、或每次会话必需的指令入根文件；仅触碰某模块才需要的归该模块子路径 CLAUDE.md。记录规则/说明/条件/经验/踩坑等文本内容时同样按此归位，注意精简/下沉。
- **子文件维护**：新建模块目录时同步创建该路径 CLAUDE.md（模板结构读任一现有子文件即知，不在此复述）；改动约定/决策/红线时同步对应 CLAUDE.md——只改实现不改约定时无需动文档。
- **短标识符解码**：新启用的短标识符（SEC-*/B*/FE-*/IC-* 等）就近在所属模块 CLAUDE.md 定义，根文件不设编号索引；docs/ 已删设计稿遗留的历史编号（MC-*/TB-*/NAV-*/ZQ-*/AC-*/IHE-* 等）以行内语境为准，不回溯解码。
- **ADR 维护**：adr.md 各条正文恒为当前有效形态——后续修订/推翻一律原位收敛改写（被推翻段落直接改写或删除），条末加一行「沿革」记日期与要点，禁文末追加块/编年史累积；完整历史从 git log 查。
- **事实文档防腐**：只记经核验的当前事实，计划/待办/临时记录不入文档；表达演进必须带版本/日期/状态锚，禁无锚点「以前、现在、不再」；不手抄可从 schema/类型定义/命令帮助/生成文件直接得到的清单，链接单一权威来源；单篇单一职责、可审查长度。
- 配套文档：领域术语表 `@../CONTEXT.md`；架构决策记录 `@.claude/adr.md`；自动化测试豁免与定位 `@.claude/test-exemptions.md`；因果档案 `architecture/notes/`（规则见该目录 CLAUDE.md）。

## 架构（单进程 GPUI 模块化单体）

**单进程、单窗口：GPUI 壳拥有全部 UI 状态与 OS 访问；无 WebView、无 IPC、无 Node 运行时。** 依赖方向 = application → core；`slterm_hook` 是独立小进程桥，不被 app 依赖。方向由 `architecture/dependencies.toml` 机械强制。

### workspace 成员与 ownership

| crate | owns | must not become |
|---|---|---|
| `slterm_app`（lib+bin `slterm`） | GPUI 壳、workspace/tab/pane 生命周期、设置 UI 与写通道、平台适配、Runtime API、AI 集成、文件/编辑器、i18n、打包入口 | 不承载 VT/PTY 解析；不把业务逻辑塞进渲染层；不绕过领域通道直写磁盘/凭据 |
| `slterm_terminal` | 网格、VT 解析、PTY 行为、输入编码、选择与回放、渲染快照 | 不依赖产品视图、窗口状态或 GPUI 类型 |
| `slterm_settings` | 设置值模型、JSON 值与键注册表、路径权威、主题色表与 WCAG 校验 | 零生产依赖；不持写通道硬机制（归 app `settings_store`） |
| `slterm_split` | 分屏树、几何切割、拖拽比例与导航纯函数 | 零生产依赖；不出现 UI 框架类型 |
| `slterm_completions` | 补全匹配引擎与来源合同 | core 层；不依赖 app 或渲染包 |
| `slterm_hook`（bin `slterm-hook`） | AI CLI hook 小进程桥：有界转发后退出 | 零生产依赖；必须 exit 0、快速、隐形，禁阻塞调用方 |

跨领域类型锚点：pane/tab 标识与分屏树归 `slterm_split`；设置/主题类型归 `slterm_settings`；终端网格/OSC/快照类型归 `slterm_terminal`；通用错误 `AppError` 与品牌常量归 `slterm_app::error`/`slterm_app::brand`。他处只许 `use` 或经 facade 传参，禁重复定义、别名漂移与字段语义改写；新增跨领域类型先登记归属。

## 硬性开发约束（新增功能必须遵守）

1. **依赖方向**：core 不依赖 app；`renderer_packages`（gpui/gpui_platform/gpui-component/winit 等）禁入 core/hook 生产依赖；根 workspace 成员与 `dependencies.toml` 键集全等（`scripts/check_architecture.py` 机械强制）。
2. **模块不穿透**：crate 内模块共享只经公开小接口/类型锚点；跨 crate 共享只经 core crate 公开 API；禁领域自建第二套读取/执行/写盘通道（单一执行体原则）。
3. **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在 PTY/平台封装模块；测试 `#[cfg(windows)]` 原则上改运行时 `cfg!(windows)` 分支。
4. **GPUI 前台上限**：GPUI 前台线程禁止阻塞 I/O、网络、子进程等待；阻塞工作走 background executor 或专用线程，结果回前台更新状态并唤醒重绘。
5. **线程与生命周期**：pane/PTY/窗口/托盘/watcher 的创建与销毁必须有显式所有权与 shutdown 路径；不留孤儿线程/句柄；跨线程只经有界通道，禁共享可变状态裸穿。
6. **原子写**：一切持久化写盘 = 临时文件 + rename；损坏三态处理；设置写只经 `settings_store`/`audit` 包装，写核心函数私有。
7. **凭据红线（SEC-18）**：真实凭据值禁止写入任何 git 追踪文件——代码、测试夹具、文档、脚本一律不行；测试与文档仅允许假值占位符（`sk-test` 形态）。真实凭据只存 Windows 凭据管理器；token/key 不出凭据域，日志/错误消息禁插值。
8. **审计通道**：安全写事件与哈希对账只经 `tracing` target `audit`；事件字段只含 `operation/path/source/result` 与 `expected/actual` 指纹，禁插值凭据值或文件内容。
9. **配色单点**：颜色定义于主题/设置体系，组件经语义 token 引用，禁硬编码颜色；仅暗色。新增方案/色值须过 WCAG 语义槽校验。
10. **布局单点**：会话布局只经 session schema 的 layout 转换对（`session_restore`）存取；禁第二套序列化或绕过快照。
11. **面板封闭**：tab 与侧栏视图为封闭枚举（`WorkspaceTab`/`SideViewId`）；新增类型须在单一注册点登记，恢复时过滤未知类型。
12. **会话元数据单点**：PTY 进程映射只在 `TerminalRegistry` 管理；pane/视图只订阅，不自存。
13. **测试覆盖**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在 `.claude/test-exemptions.md` 登记豁免原因与兜底层级，禁止未登记豁免。
14. **门禁常挂**：提交前五件门禁全绿（架构/依赖、禁名、clippy、fmt、测试）；禁名词表只允许 `slTerminal`/`slterm`，旧品牌名不得回流。

## 命令

> 工作区根即仓根；构建/测试全部为 cargo 与脚本族，无 npm/Node。

- 构建：`cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook`；发布构建走脚本链同一 feature 图。
- 使用/验收：用 debug 构建产物 `target/debug/slterm.exe` 实测（用户固定习惯），不走交互式 dev 模式。
- 全量测试（串行红线 BE-06）：`cargo test --workspace -- --test-threads=1`。
- 定向测试（TQ-COV-06 红线）：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；**禁止**用 `--lib` 或直打默认 lib target。
- GPUI 虚拟窗口测试：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 平台/真机测试：`cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1`；`cargo test -p slterm --test windows_console_startup -- --test-threads=1`。
- 静态检查：`cargo clippy --workspace --all-targets`；`cargo clippy -p slterm --all-targets --features gpui-test-support`；`cargo fmt --all -- --check`。
- 架构门禁：`python scripts/check_architecture.py`。
- 禁名门禁：`python scripts/check_brand_regress.py staged`；提交信息与推送由 `.githooks/` 三钩扫描。
- git hooks 启用（一次性）：`git config core.hooksPath .githooks`。
- 脚本门禁自测：`python -m unittest discover -s scripts/tests -p "test_*.py"`。
- 打包钉测试：`pwsh -File scripts/tests/package-release.tests.ps1`（体积预算/清单/新鲜度）；`pwsh -File scripts/tests/windows-runtime.tests.ps1`（随包运行时校验）。
- 发布：`pwsh -File scripts/release.ps1 -Version <X.Y.Z>`（共享构建 → Inno 安装器 → 便携 zip → 钉测试 → `verify-release`）；发布脚本要求 PowerShell 7（脚本内含 `#Requires -Version 7`）。
- 发布核验：`scripts/release-checklist.md` 人工全勾；「命令成功 ≠ 发布核验完成」。

## 测试策略

四层测试形态（自下而上；各层职责不重不漏）：

| 层 | 形态 | 载体与命令 | 职责 |
|---|---|---|---|
| 1 | Rust 单测（主力） | 各 crate 内嵌 `mod <领域>_tests` + `tests/` 集成 target；`cargo test --workspace -- --test-threads=1` | 协议语义、纯函数契约、serde round-trip、注册表契约、平台串行化 |
| 2 | GPUI 虚拟窗口 UI 测试 | `#[gpui::test]` + `TestAppContext`/`VisualTestContext`，feature `gpui-test-support` 统一门控 | 真实控件/布局/键盘/hover 路径 |
| 3 | 脚本层门禁与工具箱 | python unittest、PS1、cargo | 行数棘轮、依赖方向、禁名回流、打包钉测试、Win32 输入矩阵、Runtime conformance |
| 4 | 豁免登记制度 | `.claude/test-exemptions.md` 单表真值源 + 模块 CLAUDE.md 明细 | 一切残余不可自动化面（实机验收点、条件跳过） |

核心原则：

- **TQ-COV-06 红线（预防性知识）**：定向测试带 filter 或 `--lib` 可能绕过 `[lib] test=false` 的显式 test target，使测试二进制的链接参数/manifest 控制失效，出现零输出崩溃（历史上表现为 0xc0000139）。定向测试一律 `cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；遇同类症状先查测试 target 形态与链接参数，勿当业务代码 bug 排查。
- **全量回归串行执行（BE-06）**：cargo 全量与脚本钉测试禁止并行——性能基准与 PTY 容量断言对 CPU 负载敏感。
- **隔离优先**：Rust 用 `tempfile::tempdir()` 与 `SLTERM_CONFIG_DIR` 隔离文件系统、`SPAWN_LOCK`/`ENV_LOCK` 串行化进程级资源；UI 测试用虚拟窗口与隔离配置目录。
- **负载裕度登记口径**：超时/轮询预算提额治理间歇红时，必须在改动点注释标明「负载裕度，非断言/语义放宽」；判别 = 断言语义不变、只放大时间预算，且真 bug 条件下该红仍红。凡须同步放宽断言值/判据的，不属负载裕度，按语义变更走完整评审。
- **bugfix 须附防复发测试**：修复缺陷或优化历史功能时，除对改动代码添加全量测试外，还须对照改动前老代码补回归用例。
- **命名约定**：内嵌测试模块 = `mod <领域>_tests`（一文件多测试模块用功能具名）；测试函数 = 「对象_行为_场景」snake_case 裸名，禁 `test_` 前缀与规格编号前缀；集成测试 = `<域>_tests.rs`（snake_case）；脚本 = `test_*.py` / `*.tests.ps1`。
- **模块测试模式见各子路径 CLAUDE.md**，不在根文件展开。

## 发布打包

- **单一入口**：`scripts/release.ps1 -Version <X.Y.Z>`；正式路径禁 `-SkipBuild`/`-AllowStale`（仅脚本自测与彩排允许，且须显式出现）。
- **清单即合同**：包内文件集唯一真值源 = `build-windows-product.ps1` 的 `PackageEntries`；zip staging 全等校验与 `installer.iss` `[Files]` 必须同步，多一件少一件都红。
- **版本线**：续 `0.3.0`；更新链对 `< 0.3.0` 一律视为「需手动重装」（旧安装基不构成可更新对象）。
- **产物命名**：`slTerminal-v<version>-windows-x64-setup.exe` / `slTerminal-v<version>-windows-x64.zip` + `SHA256SUMS`。
- **体积门禁**：`[profile.release]` 的冷代码 `opt-level = "s"` + 热路径 O3 钉包清单；预算值钉在 `package-release.tests.ps1`。超预算即红；收紧合法，放宽须在提交信息说明原因；改动 profile 体积相关数值必须重跑钉测试。
- **合规四落点**：仓库根 `LICENSE` 全文 / workspace `Cargo.toml` `license = "GPL-3.0"`（成员 crate `license.workspace = true`）/ 安装器许可页 / 应用内关于页。真实凭据禁入任何打包输入。
