# 架构与服务集成评审

> 评审范围：`docs/featureSpec/assets/README.md`、`docs/devPlan/design/` 下架构、领域模型、GPUI 运行时、Windows 平台、文件/Git 服务、Hooks、Runtime v2、构建发布、测试共十篇。只列问题，不列优点。

## 覆盖矩阵

### docs/featureSpec/assets/README.md

| 章节 | 结论 |
|---|---|
| 目录说明 | `assets-snapshot-counts` |
| 许可与来源 | `assets-logo-license` |
| 界面图标 | `assets-snapshot-counts`、`assets-logo-license` |
| CLI logo | `assets-logo-license`、`assets-agentkind-slug-ownership` |
| 应用图标 | 无问题 |
| 预览规则 | `assets-preview-nonreproducible` |
| 界面图标清单 | 无问题 |
| CLI logo 清单 | `assets-agentkind-slug-ownership` |
| 缺失的品牌资产 | `assets-agentkind-slug-ownership` |
| 应用图标 | 无问题 |

### docs/devPlan/design/architecture.md

| 章节 | 结论 |
|---|---|
| 新 core crate 边界 | `arch-agents-core-io` |
| crate 与模块拓扑 | 无问题 |
| 依赖方向与 architecture/dependencies.toml 登记 | `arch-settings-recovery-dir-dependency` |
| 状态所有权矩阵 | `arch-runtimehub-snapshot-ownership` |
| 线程与执行模型 | `arch-hidden-terminal-model-owner`、`arch-watch-debounce-value`、`arch-runtimehub-snapshot-ownership` |
| 启动链 | 无问题 |
| 关闭链 | `arch-close-state-order` |
| 错误边界与错误映射 | 无问题 |
| 隐藏项目的渲染与布局 | `arch-hidden-terminal-model-owner` |
| 旧壳退役 | `arch-gate-red-inside-milestone` |
| 新数据目录与文件名 | `arch-settings-recovery-dir-dependency`、`platform-window-geometry-scope` |
| 单实例与移交载荷 | `domain-handoff-id-form`、`runtime-handover-wire-owner` |

### docs/devPlan/design/domain-model.md

| 章节 | 结论 |
|---|---|
| 身份类型与分配器 | `arch-id-lifetime-state-loss` |
| 项目与项目注册表 | `domain-type-and-name-contract` |
| 项目布局、面板组与面板 | `domain-read-only-authority` |
| 面板封闭枚举与参数 schema | `domain-type-and-name-contract` |
| 终端编号与终端簿 | `domain-type-and-name-contract` |
| 去重键与标题 | `domain-type-and-name-contract` |
| 焦点模型 | 无问题 |
| 纯转换与恢复对账 | `domain-dto-contract-missing`、`domain-restore-orphan-records` |
| 不变量清单 | `domain-restore-orphan-records` |
| Runtime v2 资源 ID 前向合同 | `domain-handoff-id-form`、`arch-id-lifetime-state-loss` |
| 面板类型注册表扩展规则 | 无问题 |

### docs/devPlan/design/gpui-runtime.md

| 章节 | 结论 |
|---|---|
| Entity 拓扑与所有权 | 无问题 |
| 订阅图 | 无问题 |
| 渲染路径与隐藏项目 | `arch-hidden-terminal-model-owner` |
| 焦点与一次性焦点意图 | 无问题 |
| 异步任务与取消 | 无问题 |
| 窗口关闭钩子 | `arch-close-state-order` |
| 面板错误边界 | `gpui-poison-retry-dirty` |
| 终端与后台输出 | `arch-hidden-terminal-model-owner` |
| 测试接缝 | `test-virtual-window-feature-gate` |

### docs/devPlan/design/platform-windows.md

| 章节 | 结论 |
|---|---|
| 决策与红线 | `platform-window-geometry-scope`、`platform-name-validation-owner` |
| 单实例与移交载荷 | `runtime-handover-wire-owner`、`domain-handoff-id-form` |
| 窗口几何、DPI 与多显示器 | `platform-window-geometry-scope` |
| 系统通知与任务栏闪烁 | 无问题 |
| 托盘与开机启动 | 无问题 |
| 文件夹选择器、回收站与默认程序 | `platform-name-validation-owner` |
| 进程树终止 | 无问题 |
| 平台能力探测 | 无问题 |
| 验收与测试 | `platform-window-geometry-scope`、`test-virtual-window-feature-gate` |

### docs/devPlan/design/services/filesystem-git.md

| 章节 | 结论 |
|---|---|
| 职责与边界 | 无问题 |
| 决策与红线 | `fs-path-authority-duplication` |
| 模块树 | `fs-module-tree-src-level` |
| 路径权威与真实路径校验 | `fs-path-authority-duplication` |
| 允许根 | `fs-path-authority-duplication`、`fs-transfer-preflight-boundary` |
| 文件监听 | `arch-watch-debounce-value` |
| Git 执行体 | `fs-git-literal-pathspecs-order` |
| 仓库根解析 | 无问题 |
| 状态快照与缓存 | 无问题 |
| 外部修改检测 | 无问题 |
| 原子保存与回收站 | `fs-permanent-delete-scope` |
| 树内复制与移动 | `fs-transfer-preflight-boundary` |
| 消费方 Port 与假实现 | 无问题 |
| 错误映射 | 无问题 |
| 复用、改造与新建 | 无问题 |
| 验收与测试接缝 | 无问题 |

### docs/devPlan/design/integrations/hooks.md

| 章节 | 结论 |
|---|---|
| 职责与边界 | 无问题 |
| 决策与红线 | 无问题 |
| 能力目录 | 无问题 |
| Schema 校验 | 无问题 |
| 三层文档模型 | 无问题 |
| GUI/JSON 双模式状态机 | 无问题 |
| 保存事务 | 无问题 |
| 注入与卸载 | `hooks-injection-status-conflict`、`hooks-suspicious-confirmation-scope` |
| 外部修改检测 | 无问题 |
| 文件锁与备份 | 无问题 |
| 错误映射 | `hooks-injection-status-conflict` |
| 复用、改造与新建 | 无问题 |
| 验收与测试接缝 | 无问题 |

### docs/devPlan/design/integrations/runtime-api-v2.md

| 章节 | 结论 |
|---|---|
| 职责与边界 | `runtime-handover-wire-owner` |
| 决策与红线 | `runtime-internal-contract-conflicts`、`runtime-handover-idempotency` |
| 资源模型与稳定 ID | `runtime-command-params-missing`、`arch-id-lifetime-state-loss` |
| 传输、鉴权与发现 | `runtime-handover-wire-owner` |
| 命令语义 | `runtime-command-params-missing`、`runtime-project-create-activate`、`runtime-internal-contract-conflicts`、`runtime-exec-cwd-boundary` |
| 并发与幂等 | `runtime-mutation-cache-scope`、`runtime-handover-idempotency`、`runtime-internal-contract-conflicts` |
| CLI 与随包 skill | 无问题 |
| 审计 | `runtime-internal-contract-conflicts` |
| Conformance 与迁移 | `runtime-internal-contract-conflicts`、`runtime-handover-wire-owner` |
| 模块树 | `runtime-handover-wire-owner` |
| 复用、改造与新建 | 无问题 |
| 验收与测试接缝 | `runtime-command-params-missing` |

### docs/devPlan/design/build-and-release.md

| 章节 | 结论 |
|---|---|
| 决策与红线 | `build-package-cli-logos`、`build-update-manual-check-feedback` |
| 目标 workspace 布局 | `arch-agents-core-io`、`build-workspace-members-gate` |
| 依赖钉死与基线验收 | 无问题 |
| 二进制、包名与资产 | `build-package-cli-logos`、`assets-logo-license` |
| 安装器与便携包 | `build-panic-unwind-pin` |
| 更新检查与交接 | `build-update-manual-check-feedback` |
| 架构门禁与预算重登记 | `build-workspace-members-gate` |
| 验收与测试 | `test-virtual-window-feature-gate`、`test-gate-packaging-omitted` |

### docs/devPlan/design/testing.md

| 章节 | 结论 |
|---|---|
| 决策与红线 | `test-virtual-window-feature-gate` |
| 夹具 | 无问题 |
| 分域测试要点 | 无问题 |
| 验收追溯 | 无问题 |
| 范围外与删除能力 | 无问题 |
| 旧测试处置 | 无问题 |
| 豁免与人工验收 | 无问题 |
| 门禁 | `test-virtual-window-feature-gate`、`test-gate-packaging-omitted` |

## P0

未发现 P0。

## P1

### arch-close-state-order [P1] 关闭状态机进入 Guarding 的时点自相矛盾

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §关闭链 · 规则
- 原文：“`CloseState::{Idle, Guarding, Saving, ShuttingDown}` 防重入：确认后进入 `Guarding`，冲刷成功进入 `Saving`……”
- 现象：`architecture.md` 说确认后才进入 `Guarding`；`gpui-runtime.md` 说进入 `Guarding` 后收集脏面板并弹出聚合确认，取消回 `Idle`。两处把守卫对话框放在状态机两侧。
- 影响：实现可能先弹确认再进入 `Guarding`，或在 `Guarding` 中再次弹确认；取消、重试、防重入语义无法同时满足，关闭链可能绕过守卫。
- 证据：`architecture.md` §关闭链；`gpui-runtime.md` §窗口关闭钩子；`docs/featureSpec/workspace/shell.md` §启动与关闭 的聚合确认顺序。
- 建议修法：固定为 `Idle → Guarding（收集 + 确认）→ Saving（用户确认后）→ ShuttingDown`；`Guarding` 取消回 `Idle`，`Saving` 保存失败回 `Guarding`；三篇只引用这一条。
- 裁决对象：设计修订

### arch-settings-recovery-dir-dependency [P1] `slterm_settings::paths::recovery_dir(ProjectId)` 违反 settings 零依赖契约

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §新数据目录与文件名 · 隔离规则
- 原文：“`slterm_settings::paths` 提供……`recovery_dir(ProjectId)`……app 侧不得自行拼接路径。”
- 现象：`ProjectId` 唯一定义于 `slterm_workspace::ids`；`architecture/dependencies.toml` 对 `slterm_settings` 维持 `zero_production_dependencies = true`，且目标依赖表未增加 `settings → workspace`。settings 无法合法命名该类型。
- 影响：要么架构门禁红，要么复制 `ProjectId`/绕过路径权威；`slterm_settings` 的零依赖与“数据目录路径单点”两条约束不能同时成立。
- 证据：`architecture.md` §依赖方向与 dependencies.toml 登记；`architecture/dependencies.toml` 的 `[crates.slterm_settings]`；`domain-model.md` §身份类型与分配器。
- 建议修法：把按项目分段的访问器改为 `recovery_dir(project_id: u64)`，或把项目级路径组合移到 `slterm_workspace`/app 的持久化层；settings 只保留根级路径。
- 裁决对象：设计修订

### arch-gate-red-inside-milestone [P1] “里程碑内允许短暂红灯”与门禁常挂冲突

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §旧壳退役 · 节奏
- 原文：“主线始终可编译；里程碑边界门禁全绿，里程碑内允许短暂红灯。”
- 现象：项目级硬性约束是“提交前全部门禁全绿”；`plan.md` 也重复“里程碑内允许多次提交与短暂红灯”。架构把未提交红灯扩大为可提交红灯。
- 影响：main 分支可落在 clippy/测试/架构/文档/品牌门禁失败状态；`checkpoint` 回滚点与 bisect 失效；AI 执行者会据此提交红灯代码。
- 证据：项目 `CLAUDE.md` §决策与红线“门禁常挂”；`plan.md` §原则、§分支提交与回滚；`architecture.md` §旧壳退役。
- 建议修法：改为“每次提交全绿；未完成能力用 feature gate、占位或禁用态，不以红灯提交”；若确需例外，由用户裁决并在门禁文档登记。
- 裁决对象：计划修订

### domain-dto-contract-missing [P1] `WorkspaceDto`/`ProjectLayoutDto`/`TerminalsDto` 未定义

- owner：`docs/devPlan/design/domain-model.md`
- 类型：缺口
- 定位：`domain-model.md` §纯转换与恢复对账
- 原文：“`WorkspaceDto`、`ProjectLayoutDto`、`TerminalsDto` 作为本地状态库的 JSON 载荷持久化；……本文件定义 DTO 字段语义、恢复对账与不变量。”
- 现象：本文件只给出运行期结构 `ProjectViewState`、`ProjectLayout`、`TerminalBook` 和转换函数；没有 DTO 字段、serde tag、`revision`、数组/映射形态、重复 ID 表示。`persistence.md` 明确把 DTO 字段与恢复对账指回本文件。
- 影响：状态库 JSON 合同缺失；恢复对账“重复 ID 保留先出现者”在 BTreeMap 形态下不可发生，DTO 若为数组则需要顺序语义；实现与测试无法判定幂等。
- 证据：`persistence.md` §状态库 schema（“字段与恢复对账归 `domain-model.md`”）；`domain-model.md` §纯转换与恢复对账。
- 建议修法：补出三套 DTO 的精确结构（字段、tag、`revision`、容器类型、可选性），说明与运行期类型的映射及重复项表示；恢复顺序按 DTO 形态重写。
- 裁决对象：设计修订

### domain-restore-orphan-records [P1] 恢复对账未丢弃无人引用的 `PanelRecord`

- owner：`docs/devPlan/design/domain-model.md`
- 类型：缺陷
- 定位：`domain-model.md` §纯转换与恢复对账 · 顺序
- 原文：“丢弃引用不存在组的面板；丢弃不在树上的组；丢弃空组；按 `slterm_split` 规则塌缩树。”
- 现象：`PanelRecord` 不持有组引用，只有 `PanelGroup.panels` 引用面板。现有规则无法发现“存在于 `layout.panels`、但没有任何组引用”的记录；语句方向也写反了。
- 影响：孤儿记录留在布局中并随下次快照写回；去重索引、标题冲突表、终端簿对账会把不可见面板计入，违反“每个面板记录恰好被一个组引用”的不变量，恢复不再幂等。
- 证据：`domain-model.md` §项目布局、面板组与面板；§不变量清单。
- 建议修法：在组/树归一后增加“删除未被任何 `PanelGroup.panels` 引用的 `PanelRecord`”，并把原句改为“丢弃引用不存在面板的组条目”；补 `normalize`/恢复对账测试。
- 裁决对象：设计修订

### runtime-handover-wire-owner [P1] `runtime.handover` 线缆类型所有权分裂

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：缺口
- 定位：`runtime-api-v2.md` §命令语义 · `runtime.handover`；§模块树
- 原文：“移交载荷类型、校验与接收语义归 `platform-windows.md`；本协议只定义 `runtime.handover` 方法与信封。”
- 现象：协议 crate 模块树含 `methods/handover.rs`，但 `HandoffRequest`/`HandoffReply` 由平台文档定义，协议 crate 又不能依赖 app/平台；方法参数与响应 schema 没有落点。Conformance 要求“方法表 = capabilities = schema 方法枚举”。
- 影响：控制面协议核心里程碑无法实现该方法的 schema；第二实例与服务端可能各自定义载荷；`runtime.describe`/conformance 无法覆盖移交。
- 证据：`runtime-api-v2.md` §命令语义、§Conformance、§模块树；`platform-windows.md` §单实例与移交载荷；`domain-model.md` §Runtime v2 资源 ID 前向合同 还称移交使用资源 ID。
- 建议修法：在 `slterm_runtime_protocol::methods::handover` 定义线缆 `HandoffRequest/HandoffReply/HandoffOp`，平台层只做校验与结果映射；或在协议中显式声明 handover 不属于 schema 方法表并说明理由。
- 裁决对象：设计修订

### runtime-command-params-missing [P1] 多数写方法缺参数 schema 与结果类型

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：缺口
- 定位：`runtime-api-v2.md` §命令语义 · 关键命令类型
- 原文：方法表列出 `project.move`、`panel.move`、`panel-group.resize`、`panel.close`、`terminal.prompt`、`agent.*`、`git.*` 等；类型段只定义 `CreateProjectParams`、`CreatePanelParams`。
- 现象：绝大多数写方法没有参数字段、可选性、默认值、边界校验；`params_hash` 幂等键的规范化输入未定义；`RuntimeWarning` 之外的错误 details 形状也未定义。快照类型另缺 `TerminalResource`、`PanelLifecycle` 与文本提到的 `state_change_seq`。
- 影响：`slterm ctl`、随包 skill、服务端与 conformance 无法就同一方法达成类型合同；幂等缓存键不稳定；错误码无法逐方法验收。
- 证据：`runtime-api-v2.md` §命令语义、§并发与幂等、§资源模型与稳定 ID、§模块树。
- 建议修法：为每个方法补线缆参数/结果结构（或统一 `MethodParams` 枚举并逐变体校验），定义 `params_hash` 的规范化规则；补齐 `TerminalResource`、`PanelLifecycle`、`state_change_seq`。
- 裁决对象：设计修订

### runtime-mutation-cache-scope [P1] `mutation_id` 缓存按共享 token 作用域会跨客户端串响应

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：缺陷
- 定位：`runtime-api-v2.md` §并发与幂等
- 原文：“服务端按 `(mutation_id, method, params_hash)` 缓存最近响应（有界、短 TTL、按 token 作用域）……”
- 现象：本机端口文件只有一个 token，所有客户端共享；两个客户端可用相同 `mutation_id`（如 “1”）和相同 method/params，后到者会命中前者的缓存而不执行。请求 `id` 形如 `<pid>-<seq>`，但不参与幂等键。
- 影响：写操作被静默跳过并返回他人结果；破坏性操作可能“假成功”；与“请求 id 只作关联”一起放大误用。
- 证据：`runtime-api-v2.md` §传输、鉴权与发现（端口文件含单 token）；§并发与幂等；信封示例中的 `id`。
- 建议修法：幂等键加入连接/客户端身份，或要求 `mutation_id` 为 UUIDv4 且在缓存命中时校验来源连接；明确缓存按客户端隔离。
- 裁决对象：设计修订

### fs-git-literal-pathspecs-order [P1] status 命令示例把全局 flag 放在子命令之后

- owner：`docs/devPlan/design/services/filesystem-git.md`
- 类型：缺陷
- 定位：`filesystem-git.md` §状态快照与缓存
- 原文：“`status` 使用 `git status --porcelain=v2 --branch -z --untracked-files=all --literal-pathspecs`……”
- 现象：`--literal-pathspecs` 是 Git 全局选项，必须位于子命令之前。按示例执行会得到 `error: unknown option 'literal-pathspecs'`（已在 Git 2.49.0 实测）；本文件决策表又要求全局 flag 位于子命令之前。
- 影响：照文档实现会直接采集不到 status；写前 `revision` 校验、Commit/Explorer/Diff 全部失效。
- 证据：`filesystem-git.md` §Git 执行体 与 §状态快照；现有代码 `slterm_app::git::status` 已把 `--literal-pathspecs` 放在 `status` 之前（已抽查）。
- 建议修法：改为 `git --no-optional-locks --literal-pathspecs -C <cwd> status ...`，并统一说明所有全局 flag 的放置规则；补命令形态测试。
- 裁决对象：设计修订

### build-package-cli-logos [P1] 包内资产清单未包含 CLI 品牌 logo，嵌入/随包路径未定义

- owner：`docs/devPlan/design/build-and-release.md`
- 类型：缺口
- 定位：`build-and-release.md` §二进制、包名与资产
- 原文：“包内资产：应用图标 PNG/ICO、许可文件、第三方声明、Runtime API v2 文档与 schema、随包 skill、CHANGELOG……”
- 现象：目标 UI 需要全部 CLI 品牌图标；`agent-cli.md` 说品牌资产由构建脚本扫描 `assets/cli-svg/` 生成索引，实际资产位于 `docs/featureSpec/assets/cli-svg/`；`out-of-scope/update-release.md` 又要求发布包“保留 CLI logo”。构建文档没有定义 canonical 路径、嵌入方式或 `PackageEntries` 条目。
- 影响：运行时图标缺失或打包缺失；PackageEntries、安装器、便携包可“三处一致地漏掉”同一资产；许可与第三方声明无法对账。
- 证据：`docs/featureSpec/assets/README.md` §目录说明、§CLI logo 清单；`design/integrations/agent-cli.md` §AgentKind 能力注册表；`design/out-of-scope/update-release.md` §发布链；`scripts/build-windows-product.ps1` 的 `$script:PackageEntries` 当前无 CLI logo（已抽查）。
- 建议修法：二选一并写死：编译期 `include_bytes!` 嵌入（给出 canonical 源路径）或随包资产（补 PackageEntries、installer `[Files]`、便携 staging、钉测试与第三方声明）。
- 裁决对象：设计修订

### build-update-manual-check-feedback [P1] 手动检查更新失败只记录与设置页可见结果冲突

- owner：`docs/devPlan/design/build-and-release.md`
- 类型：冲突
- 定位：`build-and-release.md` §更新检查与交接
- 原文：“启动检查是延迟执行的后台任务；手动检查可随时触发。断网、坏 JSON、限流只记录，不弹错、不阻塞界面。”
- 现象：`settings/app-page.md` 的“更新”节要求手动检查结果非阻塞展示“已是最新、发现新版本、检查失败（断网/限流/坏响应）”；本句把手动检查也归入静默日志。
- 影响：用户主动点击“检查更新”后无反馈，误判功能失效；与跨域“影响用户的失败必须可见”冲突；验收无法判定。
- 证据：`build-and-release.md` §更新检查；`design/settings/app-page.md` §更新；`docs/featureSpec/panels/settings.md` 更新节。
- 建议修法：明确“启动自动检查失败只记录；手动检查必须显示成功/最新/失败”；后台预取失败保持静默。
- 裁决对象：设计修订

### test-virtual-window-feature-gate [P1] 门禁矩阵未运行虚拟窗口测试

- owner：`docs/devPlan/design/testing.md`
- 类型：缺口
- 定位：`testing.md` §门禁；§决策与红线 · 分层职责
- 原文：门禁只列 `cargo test --workspace -- --test-threads=1`；分层表把虚拟窗口测试归入 `gpui-test-support`。
- 现象：`gpui-test-support` 不是默认 feature（已抽查 `slterm_app/Cargo.toml`）；不传 `--features gpui-test-support` 时双门控用例整体不编译。当前仓库操作文档另有独立虚拟窗口测试命令。
- 影响：壳、焦点、关闭守卫、设置中心、通知、IME 等整层 UI 测试在标准门禁中永不运行；发布可在 UI 回归下全绿。
- 证据：`testing.md` §门禁；`build-and-release.md` §发布前门禁（只对 clippy 带 feature，不对 test 带）；`.claude/docs/operations/commands.md` 的虚拟窗口测试命令；`slterm_app/Cargo.toml` 的 `[features] gpui-test-support`（已抽查）。
- 建议修法：在两份门禁中显式加入 `cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`（或让该 feature 在测试 profile 默认开启），并写入 CI。
- 裁决对象：计划修订

### assets-snapshot-counts [P1] 资产 README 写入代码可自证的文件计数

- owner：`docs/featureSpec/assets/README.md`
- 类型：缺陷
- 定位：`README.md` §目录说明、§许可与来源、§预览规则
- 原文：“`icons/`：21 个界面 SVG 图标；其中 16 个来自 lucide-static……”；“`cli-svg/`：22 个 CLI 品牌 SVG logo……”；“`preview/icons/`：21 个文件全部生成。”
- 现象：文件数、分类数、预览数均直接对应目录内容；新增/删除资产必须同步改多处散文。`spec-issues.md` 的契约收敛裁决明确要求“删除……快照数字”，项目文档规范也禁止代码可自证的条目计数。
- 影响：文档门禁未覆盖该规则，计数会随资产增减漂移；与“单一真值源”裁决冲突。
- 证据：`docs/featureSpec/assets/README.md` 上述章节；项目 `CLAUDE.md` §文档规范“快照数字禁令”；`docs/devPlan/spec-issues.md` §六 契约收敛。
- 建议修法：删除所有总数与分类计数；用目录/表格/构建期扫描作为真值，正文只写筛选规则。
- 裁决对象：规格修订

### assets-logo-license [P1] CLI logo 许可未确证却计划随包/嵌入

- owner：`docs/featureSpec/assets/README.md`
- 类型：缺口
- 定位：`README.md` §许可与来源 · CLI logo
- 原文：“22 个 SVG 由项目作者提供……文件内嵌 id 显示部分素材源自 Devicon 与 Lobe Icons 生态；未取得逐文件授权确证，因此不在此虚构具体许可证。”
- 现象：同一文档承认无法逐文件确证许可；`THIRD-PARTY-NOTICES` 当前却对 Cursor/Copilot 图标声明 LobeHub MIT；发布迁移文档要求包内保留 CLI logo，Agent 设计还计划构建期嵌入。三者对同一资产的许可与分发范围不一致。
- 影响：发布合规无法自证；第三方声明可能失实；若随包/嵌入，属于未确权素材分发风险。
- 证据：`assets/README.md` §许可与来源、§CLI logo 清单；仓库 `THIRD-PARTY-NOTICES` 的 LobeHub 段；`design/out-of-scope/update-release.md` §发布链；`design/integrations/agent-cli.md` §AgentKind。
- 建议修法：逐文件取得并记录许可，或明确这些 logo 仅留在规格文档、不进入构建/安装包；同步 `THIRD-PARTY-NOTICES` 与资产门禁。
- 裁决对象：规格修订

## P2

### arch-runtimehub-snapshot-ownership [P2] RuntimeHub “不复制状态”与 transport 线程读取快照冲突

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §状态所有权矩阵 · Runtime v2 资源投影；§线程与执行模型
- 原文：“`RuntimeHub` 从 `Workspace` 派生；不复制状态”与 `runtime-api-v2.md`“读命令在 transport 线程读 `RuntimeHub` 的不可变快照”。
- 现象：跨线程读要求存在前台发布的不可变投影；“不复制状态”容易被实现为直接读 GPUI 实体，违反前台线程唯一读写规则。
- 影响：transport 线程可能跨线程访问实体，或另建一份可变副本，形成第二权威。
- 证据：`architecture.md` §状态所有权矩阵、§线程与执行模型；`runtime-api-v2.md` §并发与幂等。
- 建议修法：表述改为“RuntimeHub 持有前台发布的不可变投影（快照 + revision + 墓碑），不持有可变领域权威”；明确投影的发布点与线程。
- 裁决对象：设计修订

### arch-hidden-terminal-model-owner [P2] 隐藏项目终端模型更新线程归属矛盾

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §隐藏项目的渲染与布局；`gpui-runtime.md` §渲染路径与隐藏项目
- 原文：“终端输出继续解析进网格/回滚；前台只更新模型，不安排重绘计时器。”与终端设计“解析与事件处理与可见性无关……`TerminalView` 只在可见时 `notify`”。
- 现象：若前台更新模型，隐藏项目必须被逐批唤醒；若解析线程更新网格，前台不更新模型。两篇对谁写网格/回滚没有统一。
- 影响：隐藏项目输出可能填满有界通道或丢字节；切回时模型不是最新；与“隐藏项目不丢输出”的规格冲突。
- 证据：`architecture.md` §隐藏项目的渲染与布局；`gpui-runtime.md` §渲染路径与隐藏项目；`design/panels/terminal.md` §面板实体与 PTY 生命周期。
- 建议修法：明确 `TerminalSession` 解析线程在 `Term` 锁内写网格/回滚；前台只消费快照并在可见时 `cx.notify`；隐藏路径不投递逐块前台事件。
- 裁决对象：设计修订

### arch-id-lifetime-state-loss [P2] “ID 终生不复用”在空状态恢复后不成立

- owner：`docs/devPlan/design/domain-model.md`
- 类型：缺陷
- 定位：`domain-model.md` §身份类型与分配器；`runtime-api-v2.md` §资源模型与稳定 ID
- 原文：“三类 ID 均为应用级全局唯一，单调递增、终生不复用”；恢复游标取 `max(next_*, maxSeen + 1, maxDir + 1)`。
- 现象：状态库损坏后“以空状态继续”会重建游标；`recovery/` 目录只覆盖崩溃快照，已退役 ID 没有持久化墓碑。旧 Runtime 客户端重连时可能用旧 ID 命中新资源。
- 影响：ID 稳定/不复用合同只在单次状态库生命周期内成立；墓碑幂等与“未知 ID → `resource_not_found`”在灾难恢复后失真。
- 证据：`domain-model.md` §身份类型与分配器、§纯转换与恢复对账；`runtime-api-v2.md` §资源模型与稳定 ID；`persistence.md` §状态库 schema（`workspace.next_*`）。
- 建议修法：把不复用合同限定为“状态库生命周期内”，或把随机 epoch/UUID 纳入 ID；明确空状态恢复后旧 ID 的处理。
- 裁决对象：设计修订

### arch-watch-debounce-value [P2] Diff 外部修改去抖 200ms 与文件服务表 300ms 冲突

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §线程与执行模型
- 原文：“事件合并窗口由域定义（编辑器约 300ms，Diff 外部修改 200ms）。”
- 现象：`filesystem-git.md` §文件监听 的配置表列 Editor 300ms、Diff HEAD 侧 300ms、GitShow 300ms，没有 Diff 外部修改 200ms 条目。同一监听策略在两篇给出不同契约值。
- 影响：实现与验收二选一；外部修改提示延迟不一致。
- 证据：`architecture.md` §线程与执行模型；`filesystem-git.md` §文件监听。
- 建议修法：以文件服务表为唯一数值源，架构只引用；若确有 Diff 工作区 200ms 策略，补入该表并说明与编辑器的关系。
- 裁决对象：设计修订

### domain-type-and-name-contract [P2] 领域类型与字段约束不完整

- owner：`docs/devPlan/design/domain-model.md`
- 类型：缺口
- 定位：`domain-model.md` §面板封闭枚举与参数 schema、§去重键与标题、§项目与项目注册表、§终端编号与终端簿
- 原文：`SettingsParams.selected_page: SettingsPageId`、`EditorParams.language_override: Option<LanguageId>`、`DedupeKey::GitShow { repo_path: CaseFoldKey }`；`compute_titles(..., runtime: &TitleRuntime)`；`restore_project(..., ctx: &RestoreContext)`。
- 现象：`SettingsPageId`、`LanguageId`、`CaseFoldKey`、`TitleRuntime`、`RestoreContext`、`PersistedIdCeiling`、`PanelPathChange` 均未定义或未给出归属；`panels/settings.md` 又说 `SettingsPageId` 定义于本文件。显示名规格上限 80 字符、终端自定义标题 128 字符，本文件未落字段约束，Runtime 统一写“名称 256 字节”。
- 影响：`slterm_workspace` 无法在不依赖 settings/editor 的前提下实现 `PanelParams`；长度校验在 UI、领域、Runtime 三处不一致，CJK 字符/字节口径不明。
- 证据：`domain-model.md` 上述章节；`design/panels/settings.md` 对 `SettingsPageId` 的归属声明；`docs/featureSpec/workspace/navigation.md` 显示名上限；`docs/featureSpec/workspace/shell.md` 终端标题上限；`runtime-api-v2.md` §命令语义 参数约束。
- 建议修法：在 domain-model 定义或明确指向每个稳定类型；补字段级长度/字符口径；Runtime 方法按字段复用领域约束而非统一 256 字节。
- 裁决对象：设计修订

### domain-read-only-authority [P2] `PersistedPath.read_only` 持久化但恢复重算，权威不明

- owner：`docs/devPlan/design/domain-model.md`
- 类型：缺陷
- 定位：`domain-model.md` §项目布局、面板组与面板 · 路径描述
- 原文：“`read_only: bool, // 唯一只读标记；打开/恢复时按当前允许根重算`”
- 现象：手动只读被列入不持久化项；该字段在打开/恢复时被允许根重算覆盖，因此持久值不可能是权威。文档同时称其为“唯一只读标记/唯一持久化字段”。
- 影响：DTO 可能写入过期只读值；恢复后实现可能信任旧值或忽略新值；另存为授权与根外只读的判定出现双源。
- 证据：`domain-model.md` §路径描述、§排除项；`docs/devPlan/spec-issues.md` §五 小口径收敛“`PersistedPath.read_only` 是唯一只读字段”。
- 建议修法：二选一：从 DTO 移除并由允许根在加载时派生；或保留字段但明确“仅记录上次判定，加载时无条件重算且不回写快照”。
- 裁决对象：设计修订

### domain-handoff-id-form [P2] 领域模型称移交载荷使用资源 ID，与平台/协议冲突

- owner：`docs/devPlan/design/domain-model.md`
- 类型：冲突
- 定位：`domain-model.md` §Runtime v2 资源 ID 前向合同
- 原文：“单实例移交载荷、CLI `slterm ctl` 与随包 skill 使用同一 ID 形式。”
- 现象：`platform-windows.md` 的 `HandoffRequest` 只有根路径/文件路径/cwd，明确“不传内部 panel id”；`runtime-api-v2.md` 也说载荷用根路径定位。领域句把移交载荷纳入 ID 形式。
- 影响：实现可能给移交载荷加 `ProjectId`/`PanelId`，破坏“目标不存在只提示、不创建”和跨重启路径匹配语义。
- 证据：`domain-model.md` §Runtime v2 资源 ID 前向合同；`platform-windows.md` §单实例与移交载荷；`runtime-api-v2.md` §传输、鉴权与发现。
- 建议修法：把该句限定为 CLI、环境变量与随包 skill；明确移交载荷使用路径定位，接收端再映射为 ID。
- 裁决对象：设计修订

### gpui-poison-retry-dirty [P2] 面板错误重试丢弃旧 body 未经过脏守卫

- owner：`docs/devPlan/design/gpui-runtime.md`
- 类型：缺陷
- 定位：`gpui-runtime.md` §面板错误边界
- 原文：“`重试`：丢弃旧 body 实体，按 `PanelRecord.params` 重新创建；清空 poison。”
- 现象：若 body 持有未保存编辑/脏缓冲，重试直接丢弃，且不经过关闭守卫或保存提示；规格只要求“清除错误态并重新渲染”。
- 影响：渲染 panic 可能导致用户未保存内容丢失；违反“失败后保留可恢复状态”。
- 证据：`gpui-runtime.md` §面板错误边界；`docs/featureSpec/workspace/shell.md` §面板类型封闭集（重试语义）；`docs/devPlan/design/workspace/shell.md` §错误边界。
- 建议修法：重试前检查 body 脏/可保存；脏时走保存/丢弃确认，或保留崩溃快照后再重建；在面板错误占位中明确数据影响。
- 裁决对象：设计修订

### platform-window-geometry-scope [P2] 窗口几何在规格外却又定义完整恢复行为，且 DPI 字段与“不保存 scale factor”矛盾

- owner：`docs/devPlan/design/platform-windows.md`
- 类型：冲突
- 定位：`platform-windows.md` §决策与红线、§窗口几何、DPI 与多显示器、§验收与测试
- 原文：决策表“窗口几何按逻辑像素持久化……”；正文“规格内不创建该文件，关闭链不冲刷窗口几何”；`MonitorKey` 含 `dpi: u32`；正文“不保存 scale factor”。
- 现象：决策表与虚拟窗口验收把几何持久化/恢复当作已交付行为；正文又说本规格不创建 `window.json`。`MonitorKey` 随几何持久化却包含 `dpi`，与“不保存 scale factor”直接矛盾。
- 影响：实现无法判定是否创建/读取 `window.json`；验收可能断言不存在的路径；DPI 变化时可能用旧 `dpi` 夹取。
- 证据：`platform-windows.md` 上述章节；`docs/devPlan/spec-issues.md` §五 窗口几何与静默启动；`docs/featureSpec/settings-and-persistence.md` §数据形态边界。
- 建议修法：把决策表与测试明确标为“迁移里程碑”；本规格只保留纯函数夹取规则；`MonitorKey` 去掉 `dpi` 或注明仅诊断、不参与恢复。
- 裁决对象：设计修订

### platform-name-validation-owner [P2] 文件名校验落点与安全 `PathGuard` 单一所有者冲突

- owner：`docs/devPlan/design/platform-windows.md`
- 类型：冲突
- 定位：`platform-windows.md` §文件夹选择器、回收站与默认程序
- 原文：“文件树重命名/新建/删除的名称校验在平台文件操作模块单点执行”
- 现象：`security.md` 拥有路径校验，名称校验是其 `PathGuard` 校验顺序的一步；`filesystem-git.md` 要求所有文件访问经 `PathAuthority`/`AccessPolicy`。平台模块再声明单点，形成第三个规则源。
- 影响：名称规则可能分叉；绕过 `PathGuard` 的平台调用可写入非法名称；安全测试矩阵与平台测试重复且可能不一致。
- 证据：`platform-windows.md` 上述章节；`security.md` §路径允许根与真实路径校验；`filesystem-git.md` §路径权威与真实路径校验。
- 建议修法：名称校验保留在 `PathGuard`；平台文件操作只执行系统调用并返回类型化错误；平台文档改为引用安全所有者。
- 裁决对象：设计修订

### fs-path-authority-duplication [P2] `PathAuthority`/`AccessPolicy` 与 `PathGuard` 双类型体系

- owner：`docs/devPlan/design/services/filesystem-git.md`
- 类型：冲突
- 定位：`filesystem-git.md` §路径权威与真实路径校验、§允许根
- 原文：“`PathAuthority`/`AccessPolicy` 是 `design/security.md` 中 `PathGuard` 判定在本域的实现；本域不另立允许根规则。”
- 现象：安全文档定义 `PathGuard`/`AllowedRoots`/`PathDecision`/`PathAccess`；本域定义 `PathAuthority`/`AccessPolicy`/`AllowedRoot`/`Access`，字段与决策形态不同，也没有说明谁包装谁。
- 影响：调用方有两个路径决策入口；安全测试与服务测试各测一套；出现绕过或规则漂移时无法单点修复。
- 证据：`filesystem-git.md` 上述章节；`security.md` §路径允许根与真实路径校验；`docs/devPlan/README.md` 契约所有者索引（路径允许根归 security）。
- 建议修法：由 `PathGuard` 持有策略与决策；服务层 `PathAuthority` 作为其项目化适配器并委托校验；删除重复的 `AccessPolicy` 规则定义。
- 裁决对象：设计修订

### fs-permanent-delete-scope [P2] `DeleteMode::Permanent` 允许非回退永久删除

- owner：`docs/devPlan/design/services/filesystem-git.md`
- 类型：冲突
- 定位：`filesystem-git.md` §原子保存与回收站
- 原文：“`DeleteMode::Permanent` 只能在此回退或明确的永久删除语义下使用。”
- 现象：规格与跨域约定规定永久删除只能是回收站不可用时的回退；“明确的永久删除语义”没有定义，等于开了第二条入口。
- 影响：可能实现应用内永久删除，触碰“不提供 Shift+Delete/应用内永久删除”的数据安全边界。
- 证据：`filesystem-git.md` §原子保存与回收站；`docs/featureSpec/cross-cutting.md` §安全与隐私；`platform-windows.md` §决策与红线。
- 建议修法：限定 `Permanent` 只由调用方在 `RecycleUnavailable` 后二次确认触发；删除“或明确的永久删除语义”。
- 裁决对象：设计修订

### fs-transfer-preflight-boundary [P2] 粘贴 Cancel 零修改与跨根移动约束未由服务保证

- owner：`docs/devPlan/design/services/filesystem-git.md`
- 类型：缺口
- 定位：`filesystem-git.md` §树内复制与移动
- 原文：“`Cancel` 中止整个粘贴且零修改”；“源集合由视图按顶层项去重后传入。服务不接受任意外部路径。”
- 现象：批量逐项执行；若在首项写入后才发现冲突再 Cancel，已执行项无法零修改。Explorer 规格还要求剪切源与目标同属项目根或同属 Agent 根，服务层只校验写允许根（项目根与 Agent 根都可写），不会拒绝跨根 Move。
- 影响：Cancel 语义不可实现；跨项目/Agent 根移动可绕过产品边界，产生未定义状态。
- 证据：`filesystem-git.md` §树内复制与移动、§允许根；`docs/featureSpec/sidebar/explorer.md` §树内复制/剪切/粘贴、§冲突与并发。
- 建议修法：在首次写入前完成冲突与根兼容性预检；`Cancel` 只在预检阶段可返回；`TransferMode::Move` 显式拒绝项目根↔Agent 根；补测试。
- 裁决对象：设计修订

### hooks-injection-status-conflict [P2] 注入状态三值无法表达 Conflict

- owner：`docs/devPlan/design/integrations/hooks.md`
- 类型：缺口
- 定位：`hooks.md` §注入与卸载 · `InjectionStatus`；§其他 CLI
- 原文：“注入状态查询只返回 `Injected`、`NotInjected`、`Outdated`。”；另处“用户编辑过的同名文件报 Conflict，不覆盖。”
- 现象：`InstallOutcome` 有 `Conflict`，但状态查询没有；用户编辑过的托管文件只能被报成 `Outdated` 或 `NotInjected`，UI 可能据此提供“重新注入”。
- 影响：冲突不可见；重新注入可能覆盖或反复失败；状态机与 UI 映射缺一态。
- 证据：`hooks.md` §注入与卸载 的 `InjectionStatus`、`InstallOutcome`；§其他 CLI；§错误映射。
- 建议修法：给 `InjectionStatus` 增加 `Conflict { path }` 或 `OutdatedReason::Conflict`，并定义 UI 动作（查看/保留/人工处理）。
- 裁决对象：设计修订

### hooks-suspicious-confirmation-scope [P2] 可疑 statusline 确认“第二次注入恒放行”范围过宽

- owner：`docs/devPlan/design/integrations/hooks.md`
- 类型：缺陷
- 定位：`hooks.md` §注入与卸载 · Claude 注入
- 原文：“确认后第二次注入恒放行。”
- 现象：确认没有绑定具体命令/内容指纹，也没有限定只放行一次；若用户随后修改 statusline 为新的可疑命令，后续注入仍绕过审查。
- 影响：安全门可被一次确认永久关闭；恶意或误改命令不再提示。
- 证据：`hooks.md` §注入与卸载；`docs/featureSpec/panels/settings.md` §Hooks 注入（确认后发起第二次注入）。
- 建议修法：确认绑定 `(command, snapshot/hash)` 且只消费一次；命令变化重新进入待确认。
- 裁决对象：设计修订

### runtime-project-create-activate [P2] `project.create` 的 `activate` 参数与“并激活”冲突

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：冲突
- 定位：`runtime-api-v2.md` §命令语义 · 写方法
- 原文：“`project.create` | 按根路径创建项目；根已存在返回已有项目并激活 | 天然幂等”
- 现象：`CreateProjectParams` 有 `activate: bool`；表却说根已存在时总是激活。`activate=false` 时对已存在项目的行为未定义。
- 影响：客户端无法预测活跃项目是否变化；幂等调用可能产生副作用；验收用例冲突。
- 证据：`runtime-api-v2.md` §命令语义 的 `CreateProjectParams` 与写方法表；`domain-model.md` §项目与项目注册表（`AddProjectOutcome::Existing`）。
- 建议修法：定义 `activate` 对新项目/已有项目一致生效；或删除该参数并始终激活，另设 `project.activate`。
- 裁决对象：设计修订

### runtime-internal-contract-conflicts [P2] Runtime v2 内部条款互相冲突

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：冲突
- 定位：`runtime-api-v2.md` §命令语义、§审计、§Conformance 与迁移
- 原文：git 写方法“需 `revision`；写前重跑 status 校验 | 非幂等（重复回滚同状态应返回已一致）”；审计决策“只记操作/资源 ID/路径/结果/错误码”与审计节“资源 ID 不单列为审计字段”；v1 映射“`tab.close/rename/move` | `panel.close/rename/move`”。
- 现象：`revision` 在此指 status 摘要，与快照 `expected_revision` 同名异义；“非幂等”与“重复执行返回已一致”矛盾；审计字段集合前后不一致；v1 tab 是面板组，关闭映射到单个 panel 语义错误。
- 影响：客户端传错 revision；幂等预期不一致；审计 schema 与诊断所有者冲突；迁移映射误导旧客户端。
- 证据：`runtime-api-v2.md` 上述章节及其 v1→v2 映射表的 `window.tabs[].panes[]` 行；`filesystem-git.md` §状态快照与缓存（status digest）；`diagnostics.md` §审计 target。
- 建议修法：`git.*` 改用 `status_revision`/`status_digest`；定义重复回滚为成功 no-op（幂等）；审计字段对齐 `operation/path/source/result`；`tab.close` 映射 `panel-group.close`，rename/move 标注无等价方法或补组方法。
- 裁决对象：设计修订

### runtime-handover-idempotency [P2] `runtime.handover` 自然键幂等对 NewTerminal 不成立

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：缺陷
- 定位：`runtime-api-v2.md` §命令语义 · 写方法
- 原文：“`runtime.handover` | 第二实例移交：打开项目/文件、新建终端、聚焦 | 按载荷自然键幂等”
- 现象：`NewTerminal { project_root, cwd }` 的“自然键”只有路径；用户两次启动都要求新建终端时，按自然键会吞掉第二次；载荷已有 `request_id`，但文档没用它做幂等键。
- 影响：合法的重复启动被静默丢弃，或实现为不幂等却对外宣称幂等；与 `HandoffReply::Accepted { request_id }` 的去重能力脱节。
- 证据：`runtime-api-v2.md` §命令语义；`platform-windows.md` §单实例与移交载荷 的 `HandoffRequest.request_id`。
- 建议修法：按 op 定义：Focus/OpenProject/OpenFile 用自然键；NewTerminal 用 `request_id` 去重或明确非幂等；写入 conformance。
- 裁决对象：设计修订

### runtime-exec-cwd-boundary [P2] `terminal.exec` 未定义 cwd/env 允许根校验

- owner：`docs/devPlan/design/integrations/runtime-api-v2.md`
- 类型：缺口
- 定位：`runtime-api-v2.md` §命令语义 · 参数约束
- 原文：“`terminal.exec` | 独立非 TTY argv 执行，不改 shell | 非幂等”
- 现象：方法没有参数结构，未定义 cwd、env、argv 长度、超时与输出上限之外的边界；安全文档要求应用发起的工作目录限项目根与已登记 Agent 根。
- 影响：控制面可被用来在任意目录/环境执行命令，绕过 `PathGuard` 与审计；与终端安全边界冲突。
- 证据：`runtime-api-v2.md` §命令语义；`security.md` §进程启动与终端边界；`filesystem-git.md` §允许根；`design/panels/terminal.md` §工作目录。
- 建议修法：定义 exec 参数与允许根校验（复用 `PathGuard`）、env 白名单、stdout/stderr 上限与超时；写审计。
- 裁决对象：设计修订

### build-workspace-members-gate [P2] `WorkspaceMembers` 无机械校验

- owner：`docs/devPlan/design/build-and-release.md`
- 类型：缺口
- 定位：`build-and-release.md` §架构门禁与预算重登记
- 原文：表格要求 `scripts/build-windows-product.ps1` 的 `WorkspaceMembers` 与 `PackageEntries` 同步。
- 现象：`check_architecture.py` 只校验 Cargo workspace 成员与 `dependencies.toml`/行预算根；没有测试校验脚本内的 `WorkspaceMembers`。新增 crate 若漏登记，构建新鲜度源码根不包含它，仍可全绿。
- 影响：新 crate 改动不触发重链接，发布包可能使用陈旧二进制；“架构门禁全绿”不能证明打包输入完整。
- 证据：`scripts/architecture/dependencies.py` 的 `check`；`scripts/build-windows-product.ps1` 的 `$script:WorkspaceMembers`；`scripts/tests` 无该列表校验（已抽查）。
- 建议修法：新增门禁/测试比较 `Cargo.toml` workspace 成员与 `$script:WorkspaceMembers`，不一致即红；或从 Cargo 元数据生成该列表。
- 裁决对象：设计修订

### assets-agentkind-slug-ownership [P2] CLI logo 表复制 AgentKind 身份与缺失清单

- owner：`docs/featureSpec/assets/README.md`
- 类型：冲突
- 定位：`README.md` §CLI logo 清单、§缺失的品牌资产
- 原文：表逐行绑定文件与 AgentKind slug；另有“没有对应 logo”的 slug 清单。
- 现象：`AgentKind` 是 `slterm_agents::kind`/`agent-cli.md` 的封闭身份集；品牌资产索引按 `AgentKind::ALL` 构建期生成。资产 README 手工镜像 slug 与缺失集，新增 AgentKind/logo 要改两处。
- 影响：slug 漂移、缺失清单过期、同一 AgentKind 出现错误图标；构建期索引与文档不一致时无门禁。
- 证据：`assets/README.md` 上述章节；`design/integrations/agent-cli.md` §AgentKind 能力注册表（构建期扫描生成索引）；`slterm_app/src/ai_agents.rs` 的 `AgentKind::ALL`（已抽查，现为封闭集）。
- 建议修法：资产文档只维护“文件→显示名/来源”映射；slug 覆盖与缺失由 `AgentKind` 注册表/构建检查派生；删除手工缺失清单。
- 裁决对象：规格修订

### assets-preview-nonreproducible [P2] 预览副本一次性生成且不保留脚本

- owner：`docs/featureSpec/assets/README.md`
- 类型：缺口
- 定位：`README.md` §预览规则
- 原文：“生成规则一次性执行，不保留脚本”
- 现象：`preview/` 是提交的派生副本；新增/修改图标时必须手工按颜色规则重做，无法复现或校验；原始文件与预览可能漂移。
- 影响：暗色预览失真；预览集合与原始集合不一致时文档仍可合并；许可/颜色规则无法自动检查。
- 证据：`assets/README.md` §预览规则；目录中 `preview/icons`、`preview/cli-svg` 与原始目录并存（已抽查）。
- 建议修法：保留小型生成脚本或文档构建期生成；增加检查：预览集合 = 原集合中需要预览的子集，颜色替换符合规则。
- 裁决对象：规格修订

### arch-agents-core-io [P2] `slterm_agents` 既“不读文件”又承担“来源扫描”

- owner：`docs/devPlan/design/architecture.md`
- 类型：冲突
- 定位：`architecture.md` §新 core crate 边界；`build-and-release.md` §目标 workspace 布局
- 原文：`slterm_agents` 职责含“会话元数据与扫描、词法归属、删除账本”；`build-and-release.md` 称其“不启动进程、不读文件、不访问网络”。
- 现象：`agent-cli.md` 的 `SessionSource::scan` 是来源扫描接口；若实现位于 `slterm_agents`，必然读文件（Claude/Codex 会话目录）；若由 app 实现，文档未说明端口注入与纯函数边界。
- 影响：core 纯度与机械依赖约束可能被绕过；会话扫描执行体归属不明，测试分层无法落位。
- 证据：`architecture.md` §新 core crate 边界、§crate 与模块拓扑；`build-and-release.md` §目标 workspace 布局；`design/integrations/agent-cli.md` §历史会话扫描、恢复与删除 的 `SessionSource::scan`。
- 建议修法：明确 `slterm_agents` 只定义 `SessionSource`/投影/归属纯函数，I/O 实现留在 app 服务层；在依赖表与模块树中写明端口。
- 裁决对象：设计修订

## P3

### fs-module-tree-src-level [P3] 模块树多出一层 `src/`

- owner：`docs/devPlan/design/services/filesystem-git.md`
- 类型：缺陷
- 定位：`filesystem-git.md` §模块树
- 原文：“`slterm_app/src/services/`” 下又列 “`src/` → `fs/`、`git/`”。
- 现象：按字面会得到 `slterm_app/src/services/src/fs`，与 app 其他模块树和路径 `CLAUDE.md` 不一致。
- 影响：实现者可能创建错误目录；模块文档路径对不上。
- 证据：`filesystem-git.md` §模块树；`design/workspace/shell.md` §分层与模块树 的 `slterm_app/src/services/` 直列子模块。
- 建议修法：删除内层 `src/`，改为 `slterm_app/src/services/fs/...`、`services/git/...`。
- 裁决对象：设计修订

### build-panic-unwind-pin [P3] 发布 profile 未钉 `panic = "unwind"`

- owner：`docs/devPlan/design/build-and-release.md`
- 类型：缺口
- 定位：`build-and-release.md` §安装器与便携包 · 体积预算
- 原文：release profile 列 `opt-level`、thin LTO、单 codegen unit、strip；未提 `panic`。
- 现象：`gpui-runtime.md` 的面板错误边界依赖 `catch_unwind`，要求发布 profile 保持 unwind；当前只是 Cargo 默认值，未在文档与钉测试中固定。
- 影响：为体积改成 `panic = "abort"` 会静默禁用面板级隔离，变成架构回归。
- 证据：`build-and-release.md` §安装器与便携包；`gpui-runtime.md` §面板错误边界；`scripts/tests/package-release.tests.ps1` 的 `Assert-ReleaseProfile` 未检查 `panic`（已抽查）。
- 建议修法：在 release profile 显式写 `panic = "unwind"`，并加入打包钉测试。
- 裁决对象：设计修订

### test-gate-packaging-omitted [P3] 测试门禁未列打包钉测试与 `verify-release`

- owner：`docs/devPlan/design/testing.md`
- 类型：缺口
- 定位：`testing.md` §门禁
- 原文：门禁列表结束于 `python -m unittest discover -s scripts/tests -p "test_*.py"`，没有 `package-release.tests.ps1` 与 `verify-release`。
- 现象：`build-and-release.md` §发布前门禁 把它们列为发布门禁；两份“全部门禁”清单不一致。
- 影响：提交门禁与发布门禁边界不清；文档读者无法判断哪份是完整集合。
- 证据：`testing.md` §门禁；`build-and-release.md` §架构门禁与预算重登记；`scripts/release.ps1` 的实际链路。
- 建议修法：testing 门禁显式区分“提交门禁”与“发布门禁”，发布门禁引用 build-and-release，避免两份清单各自维护。
- 裁决对象：设计修订

## 交叉引用卡（不定级）

### workspace-shell-panel-params-owner

- owner 域：`docs/devPlan/design/workspace/shell.md`、`docs/devPlan/README.md`
- 证据：`workspace/shell.md` §关键类型称“`PanelParams` 字段由各面板篇定义”；`devPlan/README.md` 契约所有者索引同时列“枚举/ID/面板参数/路径”归 `design/domain-model.md` 与“面板参数与运行期模型（各面板）”归 `design/panels/*.md`；`domain-model.md` 已定义全部参数字段。
- 不一致点：面板参数字段的单一所有者不唯一，域外文档与领域模型互相覆盖。建议由 `domain-model.md` 独占字段定义，面板篇只引用。

### feature-spec-window-geometry-scope

- owner 域：`docs/featureSpec/settings-and-persistence.md`
- 证据：该规格 §数据形态边界把“窗口几何”列为本地数据目录持久化对象；`spec-issues.md` §五明确窗口几何为范围外迁移能力、规格内关闭链不冲刷；`platform-windows.md` §窗口几何 也写“规格内不创建该文件”。
- 不一致点：规格文本仍把窗口几何列为已承诺持久化对象，与裁决和平台设计冲突。建议规格删除该条或标注“迁移后”。

### persistence-startup-order

- owner 域：`docs/devPlan/design/persistence.md`
- 证据：`persistence.md` §启动恢复 把“读取设置 JSON”列为状态库恢复之后的步骤；`architecture.md` §启动链 在打开状态库之前读取设置并初始化窗口/服务。
- 不一致点：启动顺序两处不同；若设置影响状态库/恢复策略，执行顺序有语义差异。建议明确 persistence 小节只描述状态库内部恢复子序列，或与架构启动链对齐。
