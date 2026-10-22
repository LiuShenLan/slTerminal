# 计划与全局一致性评审

> 评审对象：`docs/featureSpec/` 全局矩阵与验收，`docs/devPlan/` 计划、裁决、迁移与范围外设计。只列问题，不修改输入文档。

## 覆盖矩阵

### `docs/featureSpec/README.md`

| 章节 | 结论 |
|---|---|
| 定位与范围 | 无问题 |
| 术语 | 无问题 |
| 目标术语与当前代码术语的概念映射 | 无问题 |
| 面板能力矩阵 | 无问题 |
| 状态保留矩阵 | 无问题 |
| 快捷键矩阵 | `shortcut-matrix-markdown-wrap` |
| 跨域不变量 | `spec-revision-landing-gaps` |
| 关键跨域决策 | 无问题 |
| 明确不做 | 无问题 |
| 文档地图 | 无问题 |
| 维护规则 | 无问题 |

### `docs/featureSpec/acceptance.md`

| 章节 | 结论 |
|---|---|
| 验收原则 | 无问题 |
| 跨域验收 | `acceptance-cross-domain-gaps` |
| 明确不做 | `acceptance-runtime-v1-gap` |
| 终端 | 无问题 |
| Markdown | 无问题 |
| 文件树与 Commit | 无问题 |
| Agent 全局文件 | 无问题 |
| 工作区、编辑器与设置 | 无问题 |
| 范围外 | `acceptance-runtime-v1-gap` |
| 平台与第三方边界 | 无问题 |
| 不可自动化面 | `acceptance-unautomatable-gaps` |
| 验收判据 | 无问题 |
| 关键决策与依据 | 无问题 |

### `docs/devPlan/README.md`

| 章节 | 结论 |
|---|---|
| 定位 | `out-of-scope-owner-drift` |
| 文档地图 | `out-of-scope-owner-drift` |
| 关键裁决 | 无问题 |
| 契约所有者索引 | `out-of-scope-owner-drift`、`provider-type-drift` |
| 维护规则 | `migration-duplicates-target` |
| 使用方式 | 无问题 |

### `docs/devPlan/plan.md`

| 章节 | 结论 |
|---|---|
| 原则 | `old-shell-retire-before-replacement` |
| 前置条件与环境 | 无问题 |
| 里程碑 | `old-shell-retire-before-replacement`、`app-page-exit-placeholder-conflict`、`ai-fix-bar-double-ownership`、`update-signing-exit`、交叉引用卡 `testing-trace-milestone-gaps` |
| 依赖与并行 | `app-page-runtime-dependency`、`sync-point-undefined`、`plan-update-last-edge` |
| 分支、提交与回滚 | `public-release-gate-gap` |
| 风险与缓解 | `risk-table-gaps` |
| 追溯 | 交叉引用卡 `testing-trace-readme`、`testing-trace-milestone-gaps` |

### `docs/devPlan/spec-issues.md`

| 章节 | 结论 |
|---|---|
| 裁决摘要 | 无问题 |
| 一、用户裁决 | `root-relocation-detached-drift`、`credential-target-migration-unrecorded` |
| 二、featureSpec 修订矩阵 | `spec-revision-landing-gaps` |
| 三、设计层采纳 | `root-relocation-detached-drift` |
| 四、删除与收敛 | 无问题 |
| 五、优化轮裁决（施工图收敛） | `diagnostics-action-set-drift`、`out-of-scope-owner-drift`、`process-label-rot` |
| 六、目标态优化裁决 | 无问题 |
| 七、验证 | `spec-revision-landing-gaps`、`process-label-rot` |

### `docs/devPlan/design/migration.md`

| 章节 | 结论 |
|---|---|
| 迁移原则 | `old-shell-retire-before-replacement` |
| 逐域处置表 | `old-shell-retire-before-replacement`、`credential-target-migration-unrecorded` |
| 旧壳退役前隔离 | 无问题 |
| 逐域删除时机 | `old-shell-retire-before-replacement` |
| 旧测试删除映射 | 无问题 |
| 旧数据文件忽略 | 无问题 |
| 新数据目录与文件名 | `migration-duplicates-target` |
| 旧单实例命名空间隔离 | 无问题 |
| 旧能力删除清单 | 无问题 |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | `old-shell-retire-before-replacement` |
| 测试与验收 | `process-label-rot` |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/ai-providers-assistant.md`

| 章节 | 结论 |
|---|---|
| Providers | `out-of-scope-owner-drift`、`provider-type-drift`、`credential-target-migration-unrecorded` |
| 凭据域 | `secretvalue-redaction-drift` |
| AI 助手 | 无问题 |
| 应用页入口与所有权 | `out-of-scope-owner-drift` |
| 终端与编辑器集成 | `ai-fix-bar-double-ownership` |
| 测试与验收 | 无问题 |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | `credential-target-migration-unrecorded` |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/app-and-shell-services.md`

| 章节 | 结论 |
|---|---|
| 应用页 | 无问题 |
| 托盘 | `tray-retry-drift`、`out-of-scope-owner-drift` |
| 任务栏 | `out-of-scope-owner-drift` |
| 开机启动 | `out-of-scope-owner-drift` |
| 通知 | 无问题 |
| 更新入口 | `out-of-scope-owner-drift`、`app-page-exit-placeholder-conflict` |
| 壳生命周期、单实例与窗口几何 | 交叉引用卡 `startup-visibility` |
| 诊断集成 | 无问题 |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | `app-page-exit-placeholder-conflict` |
| 测试与验收 | 无问题 |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/encrypted-backup.md`

| 章节 | 结论 |
|---|---|
| 归档模型与路径 | `out-of-scope-owner-drift` |
| 导出 | 无问题 |
| 导入与恢复 | 无问题 |
| 敏感键与凭据边界 | 无问题 |
| 应用页入口与所有权 | `out-of-scope-owner-drift` |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | 无问题 |
| 测试与验收 | 无问题 |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/runtime-api-v2.md`

| 章节 | 结论 |
|---|---|
| 唯一契约源 | 无问题 |
| 删除的 v1 面 | 无问题 |
| 落点模块 | 无问题 |
| 迁移顺序 | 无问题 |
| conformance 增量 | 无问题 |
| 依赖里程碑 | 无问题 |
| 测试与验收 | 无问题 |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/terminal-enhancements.md`

| 章节 | 结论 |
|---|---|
| 终端面板扩展点 | 无问题 |
| Vi 模式 | `out-of-scope-owner-drift` |
| 终端补全 | 无问题 |
| 数学覆盖层 | 无问题 |
| AI 修复条 | `ai-fix-bar-double-ownership` |
| 提示符跳转删除 | 无问题 |
| 设置开关 | `out-of-scope-owner-drift` |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | 无问题 |
| 测试与验收 | 无问题 |
| 不可自动化面 | 无问题 |

### `docs/devPlan/design/out-of-scope/update-release.md`

| 章节 | 结论 |
|---|---|
| 更新状态与检查 | `out-of-scope-owner-drift` |
| 下载与校验 | `out-of-scope-owner-drift` |
| 安装交接 | `out-of-scope-owner-drift` |
| 代码签名 | `update-signing-exit` |
| 发布链 | `out-of-scope-owner-drift` |
| 应用页入口与所有权 | `out-of-scope-owner-drift` |
| 依赖里程碑 | 无问题 |
| 迁移顺序 | `app-page-exit-placeholder-conflict` |
| 测试与验收 | 无问题 |
| 不可自动化面 | 无问题 |

## 问题条目

本域未发现 P0。

### out-of-scope-owner-drift [P1] 范围外文档复制并声称拥有目标态契约

- owner：`docs/devPlan/README.md`、`docs/devPlan/design/out-of-scope/*.md`
- 类型：冲突
- 定位：`docs/devPlan/README.md` §定位、§文档地图、§契约所有者索引；`design/out-of-scope/ai-providers-assistant.md` §应用页入口与所有权；`design/out-of-scope/encrypted-backup.md` §应用页入口与所有权；`design/out-of-scope/update-release.md` §职责；`design/out-of-scope/app-and-shell-services.md` §托盘
- 原文：“`design/out-of-scope/` 只写迁移增量，不复制目标态契约。”
- 现象：除 `runtime-api-v2.md` 外，五篇范围外文档均直接定义目标态行为并声明所有权：AI Providers/助手定义 `ProviderRecord`、`ProviderStoreV2`、凭据与助手状态机；加密备份定义归档格式、类别与恢复点；更新与发布定义检查、下载、交接、发布链与包清单；应用页与壳服务定义托盘菜单、任务栏、开机启动与通知接线；终端增强定义 Vi/补全/数学/AI 修复条语义与开关默认值。README 契约所有者索引只登记 `build-and-release.md` 拥有更新、`platform-windows.md` 拥有托盘，未登记这些范围外能力的所有者。
- 影响：单一所有者规则被绕过，同一契约存在两套目标态描述；后续实现无法判断以哪篇为准。更新、备份、托盘、终端增强均属安全或用户数据敏感面，漂移会直接进入实现。
- 证据：`design/build-and-release.md` §更新检查与交接 已定义更新检查、下载、交接与恢复；`design/platform-windows.md` §托盘与开机启动 已定义 `TrayMenuModel`/`TrayCommand`；`design/settings/app-page.md` §数据模型 已定义应用页键与默认值；`design/out-of-scope/runtime-api-v2.md` §唯一契约源 是唯一按 README 规则只写增量的样例。
- 建议修法：二选一，并同步维护规则。方案 A（推荐）：范围外文档只保留删除项、落点、迁移顺序、增量验证与不可自动化面，目标态契约迁入实际所有者篇，并在 README 索引登记所有者。方案 B：承认范围外文档可拥有目标态契约，修订 README 的“只写迁移增量”规则，在契约所有者索引中逐项登记，并删除与所有者篇的重复定义。
- 裁决对象：规格修订 / 计划修订

### spec-revision-landing-gaps [P1] 修订矩阵条目缺少 featureSpec 落点

- owner：`docs/devPlan/spec-issues.md`
- 类型：缺口
- 定位：`docs/devPlan/spec-issues.md` §二 featureSpec 修订矩阵、§七 验证
- 原文：“本文件每条裁决在 featureSpec 对应篇目有落点。”
- 现象：多条矩阵条目在声称的目标篇目中没有对应文本，或落点被写到其他篇目：
  - `README.md` 条目要求“数据根改为设置漫游、工作区状态本地”“主题无可读性提示”，但 `featureSpec/README.md` 无数据根、漫游/本地根或对比度相关表述。
  - `workspace/shell.md` 条目要求“默认尺寸/最小尺寸”，shell 只有默认 1200×800，无 760×540 或等价最小窗口尺寸；最小尺寸只出现在设计 `platform-windows.md`。
  - `panels/terminal.md` 条目要求“全局创建顺序（项目顺序 → 布局 DFS → 页签顺序）；并发上限 8”，terminal 篇只有 PTY 上限 64 与“按布局顺序连接前 64 个”，无并发 8 与全局顺序；`design/panels/terminal.md` 有并发 8，但仍无全局顺序。
  - `panels/diff.md` 条目要求“根重定位按新仓库根重新解析”，diff 篇没有根重定位规则；只有 `workspace/shell.md` 的通用迁移条目。
  - `panels/gitshow.md` 条目要求“监听失效时仅窗口聚焦/面板激活复核 HEAD 指纹”，gitshow 篇没有该触发条件；设计 `design/panels/gitshow.md` 有。
  - `settings-and-persistence.md` 条目要求“业务写入 vs 应用托管写入”分表，该篇没有写入根章节；对应内容在 `cross-cutting.md`。
  - `cross-cutting.md` 条目要求“默认 3/4/5 秒；视图可覆盖”，该篇没有通知时长；时长在 `workspace/shell.md`。
- 影响：计划首个里程碑“规格裁决与冻结”的退出条件是“全部裁决有落点”，当前无法判定通过；实现会以设计篇或空白为准，规格冻结失去意义。PTY 全局创建顺序尤其会导致超限恢复时选择哪 64 个终端出现分歧。
- 证据：`plan.md` §里程碑“规格裁决与冻结”退出条件“全部裁决有落点；文档门禁绿”；`spec-issues.md` §二逐条矩阵；上述 featureSpec 篇目对应章节。`design/panels/terminal.md` 只写“PTY 并发创建上限 8”，未写项目顺序 → 布局 DFS → 页签顺序。
- 建议修法：逐条补齐落点或修正矩阵指向；terminal 全局创建顺序与并发 8 应同时写入 featureSpec 与设计并加入验收；无法落在原篇目的条目改为指向真实所有者篇，并在 §七 验证中按篇目列出核对结果。
- 裁决对象：规格修订

### root-relocation-detached-drift [P1] 根重定位脏缓冲被设计成根外只读并可另存为

- owner：`docs/devPlan/spec-issues.md`、`docs/devPlan/design/workspace/shell.md`
- 类型：冲突
- 定位：`spec-issues.md` §一 用户裁决“根重定位脏面板”；`design/workspace/shell.md` §根重定位的面板迁移；`featureSpec/panels/editor.md` §面板形态与运行模式
- 原文：“转脱离缓冲：保留文本/脏/撤销/光标，禁用原地保存，提供另存为，关闭走守卫。”
- 现象：裁决要求有未保存修改的文件型面板进入“脱离缓冲”状态；编辑器篇明确区分“根外只读”（不提供保存、不提供另存为）与“文件已移出允许根”（保留缓冲与脏状态、提供另存为）。设计壳篇却写“落到新根外则……切换为根外只读并显示提示，脏缓冲允许‘另存为’”，把两个状态合并，形成“根外只读却可另存为”的矛盾状态。
- 影响：根重定位后脏编辑器的保存能力、关闭守卫与布局恢复行为不可判定；可能丢失用户修改，或让本应只读的根外文件获得写通道，触碰安全边界。
- 证据：`featureSpec/workspace/shell.md` §项目根 写“有未保存修改的转脱离缓冲并仅提供另存为”；`featureSpec/panels/editor.md` §面板形态与运行模式 将“根外只读”和“文件已移出允许根”分列；`design/workspace/shell.md` §根重定位的面板迁移 合并两者。
- 建议修法：设计篇按编辑器契约拆分：无脏缓冲且新根外 → 根外只读；有脏缓冲且新根外 → 脱离缓冲（禁用原地保存、仅另存为、关闭走守卫）。同步补根重定位验收场景。
- 裁决对象：设计修订

### diagnostics-action-set-drift [P1] 诊断动作集违反已关闭的合并裁决

- owner：`docs/devPlan/spec-issues.md`
- 类型：冲突
- 定位：`spec-issues.md` §五 优化轮裁决“应用页范围”；`design/diagnostics.md` §应用页入口与动作；`design/settings/app-page.md` §诊断；`featureSpec/panels/settings.md` §应用页
- 原文：“诊断动作合并为版本/构建、数据/日志目录、配置健康、复制诊断信息、打开目录/日志、通知自检、一键清理。”
- 现象：诊断为已关闭裁决，动作集被明确收敛；设计诊断与应用页却又加入“导出诊断包…”与“运行环境自检”两个可选动作，featureSpec 应用页没有这两个动作。设计篇同时保留“动作集由本篇独占”的表述，等于在未重开裁决的情况下扩展了已冻结的动作集。
- 影响：应用页诊断节的可验收范围不确定；实现可能按设计多出入口，验收按 featureSpec 判定为越界；安全面（诊断包导出）未经规格评审即进入目标态。
- 证据：`design/diagnostics.md` §应用页入口与动作 的动作表列出“导出诊断包…”与“运行环境自检”；`design/settings/app-page.md` §诊断 写“可选导出诊断包与环境自检”；`spec-issues.md` §五 只列合并后的固定动作集。
- 建议修法：二选一。若保留新动作，新增 spec-issues 裁决并更新 featureSpec 应用页与验收；若维持已关闭裁决，从设计与应用页移除这两个动作，或降级为不进入目标态的候选。
- 裁决对象：规格修订 / 设计修订

### secretvalue-redaction-drift [P1] `SecretValue` 的 Debug 契约两处不一致

- owner：`docs/devPlan/design/out-of-scope/ai-providers-assistant.md`
- 类型：冲突
- 定位：`design/out-of-scope/ai-providers-assistant.md` §凭据域；`design/security.md` §凭据域
- 原文：“`SecretValue` 的 `Debug` 只报长度；转换字符串必须走 `Zeroizing`。”
- 现象：安全所有者定义 `SecretValue` 的 `Debug` 输出 `<redacted>`；AI 设计写“只报长度”。两者都是凭据类型的可观察契约，测试与日志格式会分叉。
- 影响：凭据脱敏红线依赖唯一契约；一处允许输出长度，一处完全替换，测试断言与下游日志可能按不同实现落地。长度本身虽非明文，但契约冲突会削弱“凭据不出域”的单点。
- 证据：`design/security.md` §凭据域 代码注释“Debug 输出 `<redacted>`”；`design/out-of-scope/ai-providers-assistant.md` §凭据域 写“只报长度”。安全篇为所有者。
- 建议修法：删除 AI 设计中的长度输出，统一引用 `security.md` 的 `<redacted>`；若确需长度，作为显式诊断方法而非 `Debug`，并在安全篇定义。
- 裁决对象：设计修订

### tray-retry-drift [P1] 托盘创建失败的重试策略与平台所有者冲突

- owner：`docs/devPlan/design/out-of-scope/app-and-shell-services.md`
- 类型：冲突
- 定位：`design/out-of-scope/app-and-shell-services.md` §托盘；`design/platform-windows.md` §托盘与开机启动
- 原文：“`NIM_ADD` 失败按 1 Hz 快照重试；持续失败在应用页显示可见状态。”
- 现象：平台所有者写“托盘创建失败时通知并保留设置值，下次启动重试”；范围外文档改为进程内 1 Hz 重试。两者对失败后的运行期行为、资源占用与用户可见状态定义不同。
- 影响：托盘不可用场景的行为不可判定；1 Hz 重试与“下次启动重试”会产生不同的日志、通知频率与线程生命周期，且平台所有权被绕过。
- 证据：`design/platform-windows.md` §托盘与开机启动 的失败策略；`design/out-of-scope/app-and-shell-services.md` §托盘 的 1 Hz 重试。
- 建议修法：范围外文档只保留迁移接线与路由键变更；失败重试引用 `platform-windows.md`。若确需改为进程内重试，先在平台所有者篇修订并说明理由。
- 裁决对象：设计修订

### provider-type-drift [P1] 应用页与 AI Providers 的 Provider 类型名不一致

- owner：`docs/devPlan/design/settings/app-page.md`、`docs/devPlan/design/out-of-scope/ai-providers-assistant.md`
- 类型：冲突
- 定位：`design/settings/app-page.md` §数据模型；`design/out-of-scope/ai-providers-assistant.md` §Providers
- 原文：“`ProviderCatalog` 与 `ProviderEntry` 由 AI Providers 设计定义。”
- 现象：应用页引用 `ProviderCatalog`/`ProviderEntry`，AI 设计实际定义 `ProviderRecord`/`ProviderStoreV2`；类型名、容器边界与持久化形态均不一致。
- 影响：设置 DTO、应用页状态与 Provider 存储无法直接对接；实现要么改名，要么增加一层未登记的适配，二者都会破坏类型单点。
- 证据：`design/settings/app-page.md` §数据模型 的 `pub providers: ProviderCatalog`；`design/out-of-scope/ai-providers-assistant.md` §Providers 的 `ProviderRecord`/`ProviderStoreV2`。
- 建议修法：确定一套类型命名与容器；应用页只引用所有者类型；若 AI 设计保留 `ProviderStoreV2`，应用页改为 `ProviderStoreV2` 或在所有者篇定义 `ProviderCatalog` 为别名并说明边界。
- 裁决对象：设计修订

### old-shell-retire-before-replacement [P1] 旧壳退役早于面板实现，违反删除即验收

- owner：`docs/devPlan/plan.md`、`docs/devPlan/design/migration.md`
- 类型：冲突
- 定位：`plan.md` §原则、§里程碑“垂直切片验收”“旧壳退役”；`migration.md` §迁移原则、§逐域处置表、§逐域删除时机
- 原文：“删除即验收：每个旧域删除前必须已有目标态实现与对应验收。”
- 现象：垂直切片只覆盖多项目、布局、终端、状态库、恢复、关闭守卫、侧栏框架与导航核心；不覆盖编辑器、Markdown、Diff、GitShow、Explorer、Commit、设置中心。旧壳退役只依赖垂直切片，却在同一里程碑一次性删除旧 workspace/tab/pane/dock/session 与旧 UI 宿主。迁移篇同时声明“旧壳/tab/pane/dock/session 的删除动作统一在旧壳退役里程碑完成”，与逐域表中“编辑器面板验收后删除旧 tab 宿主”“新侧栏视图全部迁移后删除旧状态”等检查点冲突。
- 影响：退役后主线只剩终端与导航等少数能力，编辑器、Markdown、Diff、GitShow、文件浏览器、Commit、设置中心在新里程碑完成前不可用；计划“缺口显式”要求只读占位或禁用态，但没有为缺失面板类型/侧栏视图定义占位契约。旧测试与旧行为知识也在替代实现验收前删除。
- 证据：`plan.md` §里程碑“垂直切片验收”交付物清单与“旧壳退役”依赖；`migration.md` §迁移原则“删除即验收”；`migration.md` §逐域处置表末段“删除动作统一在旧壳退役里程碑完成”；`migration.md` §逐域删除时机“旧壳退役一次性删除”条目。
- 建议修法：扩大垂直切片或退役前置条件，至少让所有被删除 UI 宿主有目标位置或显式占位；或把旧壳退役拆为“旧状态模型/写路径退役”与“旧 UI 宿主逐域删除”两步，使删除时机与替代验收一一对应。若维持一次性退役，必须新增缺失能力的占位/禁用契约与验收。
- 裁决对象：计划修订 / 迁移修订

### app-page-exit-placeholder-conflict [P1] 应用页范围外节里程碑的退出条件与占位状态冲突

- owner：`docs/devPlan/plan.md`
- 类型：冲突
- 定位：`plan.md` §里程碑“应用页范围外节落地与壳服务”；`design/settings/app-page.md` §节级交付矩阵；`design/out-of-scope/app-and-shell-services.md` §迁移顺序；`design/out-of-scope/update-release.md` §迁移顺序
- 原文：“节级矩阵逐项替换；托盘/快捷方式/窗口几何；首版无静默启动。”
- 现象：应用页规格内状态明确把界面语言、更新、托盘与开机启动、AI Providers/助手、备份与恢复、终端增强列为占位/禁用；这些功能分别由后续 AI Providers、加密备份、更新与发布链、终端增强里程碑实现。范围外迁移顺序还明确“更新入口在更新服务重接后开放；此前应用页显示不可用状态”。计划却把“节级矩阵逐项替换”放在较早的应用页里程碑退出条件中。
- 影响：该里程碑无法按退出条件判定通过；要么把占位误标为已替换，要么被迫提前实现后续里程碑功能，破坏依赖顺序。
- 证据：`design/settings/app-page.md` §节级交付矩阵；`plan.md` 后续 AI Providers/加密备份/更新/终端增强里程碑；`design/out-of-scope/app-and-shell-services.md` §迁移顺序“更新入口在更新服务重接后开放”条目；`design/out-of-scope/update-release.md` §迁移顺序“应用页接入手动检查”条目。
- 建议修法：退出条件改为“占位/入口接线完成，范围外节保持只读占位或禁用态”，功能替换分别归属后续里程碑；或调整依赖，把应用页里程碑移到这些迁移里程碑之后。
- 裁决对象：计划修订

### ai-fix-bar-double-ownership [P2] AI 修复条同时被两个里程碑声明为交付物

- owner：`docs/devPlan/plan.md`、`design/out-of-scope/ai-providers-assistant.md`、`design/out-of-scope/terminal-enhancements.md`
- 类型：冲突
- 定位：`plan.md` §里程碑“AI Providers/助手”“终端增强回迁”；`ai-providers-assistant.md` §终端与编辑器集成；`terminal-enhancements.md` §AI 修复条
- 原文：“AI Providers/助手：Providers、助手、命令修复、凭据。”
- 现象：AI Providers 里程碑交付“命令修复”，终端增强里程碑交付“AI 修复条”；AI 设计说终端面板实现完整 AI 修复条，终端增强设计说终端侧只保留接线。两处都声称交付，未区分请求/凭据层与终端 UI/按键接线。
- 影响：可能重复实现或互相等待，里程碑退出条件无法判断谁负责最终验收。
- 证据：`plan.md` 两个里程碑的交付物列；`ai-providers-assistant.md` §终端与编辑器集成；`terminal-enhancements.md` §AI 修复条。
- 建议修法：明确 AI Providers 里程碑只交付 Provider/凭据/请求器，终端增强回迁交付修复条 UI、按键与命令完成接线；或把修复条整体归入 AI Providers 并删除终端增强中的重复交付。
- 裁决对象：计划修订

### update-signing-exit [P2] 更新里程碑退出条件未覆盖无证书分支

- owner：`docs/devPlan/plan.md`
- 类型：缺口
- 定位：`plan.md` §里程碑“更新与发布链”；`spec-issues.md` §一 用户裁决“代码签名”
- 原文：“四重校验；handoff；签名；发布门禁。”
- 现象：裁决规定有证书才签名，无证书时登记残余风险；计划退出条件无条件要求“签名”，在无 Authenticode 证书的环境下不可判定。
- 影响：里程碑可能因环境无证书而无法通过，或被迫跳过签名检查且无登记。
- 证据：`spec-issues.md` §一“有证书则签名安装器与二进制并在 handoff 前校验；无证书时登记残余风险，摘要校验不放宽”；`plan.md` 更新与发布链退出条件。
- 建议修法：改为“有证书时签名并验证；无证书时在发布清单登记残余风险；摘要校验不放宽”，并把无证书场景加入验收分支。
- 裁决对象：计划修订

### app-page-runtime-dependency [P2] 应用页范围外节对 Runtime 完整面的依赖缺少依据

- owner：`docs/devPlan/plan.md`
- 类型：缺口
- 定位：`plan.md` §里程碑“应用页范围外节落地与壳服务”；`design/out-of-scope/app-and-shell-services.md` §迁移顺序
- 原文：“依赖：跨域收敛与规格验收、设置中心壳与基础页、诊断与日志迁移、Runtime API v2 完整面、状态库与设置存储。”
- 现象：应用页的 Runtime 节在控制面协议核心后只需只读状态、端点与协议版本；范围外壳服务（托盘、任务栏、开机启动、通知）与 Runtime 完整方法无直接依赖。迁移顺序把双数据根、通知/托盘接线放在早期，也未要求完整 v2。
- 影响：把壳服务与范围外节串行到完整 Runtime 之后，延长关键路径；若实际存在隐藏依赖，文档未说明。
- 证据：`design/settings/app-page.md` §Runtime API 只读状态；`design/out-of-scope/app-and-shell-services.md` §迁移顺序“双数据根与 owner 锁/端点”“通知服务与托盘/任务栏”“更新入口”等条目；`plan.md` 该里程碑依赖列。
- 建议修法：删除不必要依赖，或写明哪个节必须等完整 v2 并给出理由；壳服务可独立于完整 Runtime 交付。
- 裁决对象：计划修订

### sync-point-undefined [P2] “基础层完成”同步点没有对应里程碑或退出条件

- owner：`docs/devPlan/plan.md`
- 类型：缺口
- 定位：`plan.md` §依赖与并行“同步点”
- 原文：“基础层完成、垂直切片门、收敛门、Runtime v2 conformance 四个同步点。”
- 现象：里程碑表只有名称里程碑；没有“基础层完成”里程碑，也没有其退出条件。其余三个同步点可对应“垂直切片验收”“跨域收敛与规格验收”“Runtime API v2 完整面”（或控制面子集），但“基础层完成”是过程分组。
- 影响：同步点无法排期或判定；与“依赖关系只在里程碑表定义、里程碑只用名称”的维护规则不一致。
- 证据：`plan.md` §里程碑表；§依赖与并行同步点段；`devPlan/README.md` §维护规则“依赖关系只在 plan.md 定义”“里程碑使用名称，不使用阶段号”。
- 建议修法：把同步点绑定到具体里程碑（如“构建与打包基线”或“状态库与设置存储”完成），或删除该同步点并改为“基础层里程碑全部退出”。
- 裁决对象：计划修订

### plan-update-last-edge [P2] “更新与发布链最后”未在依赖表中表达

- owner：`docs/devPlan/plan.md`
- 类型：缺口
- 定位：`plan.md` §依赖与并行“收敛与迁移”；§里程碑
- 原文：“完整 Runtime v2 先于应用页范围外节与壳服务；更新与发布链最后。”
- 现象：更新与发布链只依赖应用页范围外节、构建与打包基线、壳与工作区、诊断与日志迁移、状态库；不依赖加密备份、AI Providers、终端增强。按“未列出的边不存在”，更新链可与这些里程碑并行，甚至先于它们完成。
- 影响：“最后迁移”的产品/发布顺序无法由唯一依赖表保证；公开发布门又要求更新链与加密备份都完成，若两者并行失败会拖住发布。
- 证据：`plan.md` §里程碑“更新与发布链”依赖列；§依赖与并行“更新与发布链最后”；§分支、提交与回滚“公开发布门”。
- 建议修法：若“最后”是硬约束，增加对加密备份（以及需要一起发布的 AI/终端增强）的依赖；若只是“没有后继里程碑”，改写为“无后继依赖，可与其余迁移并行”。
- 裁决对象：计划修订

### risk-table-gaps [P2] 风险表缺少凭据、备份恢复与更新安装的高影响风险

- owner：`docs/devPlan/plan.md`
- 类型：缺口
- 定位：`plan.md` §风险与缓解
- 原文：“风险与缓解”表。
- 现象：表覆盖状态所有权、依赖方向、事务、关闭链、焦点/IME、隐藏项目、新旧数据、状态库损坏、路径混用、旧测试、CLI/Hooks 复制、Windows 平台、范围外反向修改、文档漂移、GPUI fork、Tree-sitter、协议漂移；未覆盖凭据泄漏、加密备份恢复失败导致数据丢失、更新安装半完成/回滚失败。
- 影响：这些是高影响且已有专门设计的面，计划层没有触发信号与缓解跟踪；实现阶段可能只依赖域文档，跨域失败无升级路径。
- 证据：`design/security.md` §凭据域与威胁模型；`design/out-of-scope/encrypted-backup.md` §导入与恢复；`design/out-of-scope/update-release.md` §安装交接；`plan.md` §风险与缓解。
- 建议修法：新增三行风险及触发信号/缓解，引用对应所有者篇与验收；或将风险表限定为“计划级风险”，并在表头说明域级风险由各设计篇承担。
- 裁决对象：计划修订

### migration-duplicates-target [P2] 迁移文档复制目标态模块职责与数据路径

- owner：`docs/devPlan/design/migration.md`
- 类型：冲突
- 定位：`migration.md` §新建域与职责、§新数据目录与文件名；`devPlan/README.md` §维护规则
- 原文：“新数据目录与文件名由 `design/persistence.md` 单点定义；迁移只保证：”
- 现象：紧接着仍重复列出 `<data>/state/settings.json`、`<local>/state/state.db`、恢复快照、隔离区、Runtime 端点、实例锁、窗口几何、补全、更新、日志、审计等路径；§新建域与职责 表重复 architecture 的模块职责，并保留“依赖”列，虽然注释说“权威依赖见 plan.md”。
- 影响：路径与依赖出现第二份描述，后续修订容易漂移；违反 `devPlan/README.md` “依赖关系只在 plan.md 定义，其他文档引用，不复制依赖列表”的维护规则。
- 证据：`migration.md` §新数据目录与文件名；`design/persistence.md` §数据根与文件；`design/architecture.md` §新数据目录与文件名；`devPlan/README.md` §维护规则。
- 建议修法：删除路径清单与依赖列，改为链接 `persistence.md`/`architecture.md`/`plan.md`；迁移篇只保留“旧路径不读不删、新路径与旧路径不重叠、切换时机”等增量契约。
- 裁决对象：设计修订

### credential-target-migration-unrecorded [P2] 旧凭据 target 的一次性迁移与删除未登记

- owner：`docs/devPlan/design/out-of-scope/ai-providers-assistant.md`
- 类型：缺口
- 定位：`ai-providers-assistant.md` §Providers、§迁移顺序；`migration.md` §逐域处置表 AI Providers
- 原文：“旧 `Slterm/AI/<id>` 凭据按迁移映射读取一次并写回新 target，随后删除旧 target。”
- 现象：这是有破坏性的凭据迁移（读取、写新 target、删除旧 target），但 `spec-issues.md` 只裁决了 target 命名格式，没有裁决旧 target 的迁移与删除；`migration.md` 的 AI Providers 行只写“保留引擎与凭据纪律；标识从 PaneId 改为 PanelId”，没有该步骤与失败处理。
- 影响：实现可能在迁移中途删除旧凭据；失败时用户凭据丢失，且没有回滚或保留策略。与迁移篇“旧文件不读不删”的零迁移原则也容易被误读为不处理凭据。
- 证据：`ai-providers-assistant.md` §Providers；`migration.md` §逐域处置表 AI Providers；`spec-issues.md` §一“凭据 target”只定义 `slterm/<scope>/<id>`。
- 建议修法：新增裁决与迁移行，明确“先写新 target、校验可读、再删除旧 target；任一步失败保留旧 target 并提示”；在测试与验收加入迁移失败用例。
- 裁决对象：规格修订 / 迁移修订

### shortcut-matrix-markdown-wrap [P2] 快捷键矩阵漏列 Markdown 源码态的自动换行

- owner：`docs/featureSpec/README.md`
- 类型：缺口
- 定位：`featureSpec/README.md` §快捷键矩阵“切换自动换行”；`featureSpec/panels/markdown.md` §面板归属与两态；`design/ux-framework.md` §命令与动作注册表
- 原文：“切换自动换行 | `Alt+Z` | 编辑器、Diff、GitShow”
- 现象：Markdown 源码态继承编辑器能力，明确包含自动换行；`ux-framework.md` 的 `ShortcutContext::Editor` 也覆盖“编辑器、Markdown 源码态、Diff 工作区侧”；设置中心快捷键页写“同时作用于 Diff 与 GitShow 的文本视图”。全局快捷键矩阵却只列编辑器、Diff、GitShow，漏 Markdown 源码态。
- 影响：全局矩阵与 owner 上下文不一致；按矩阵验收会漏测 Markdown 源码态的 `Alt+Z`，或实现按矩阵不接线。
- 证据：`featureSpec/panels/markdown.md` §面板归属与两态“源码态继承白名单：……自动换行”；`design/ux-framework.md` §快捷键模型 `Editor` 上下文注释；`featureSpec/panels/settings.md` §快捷键页。
- 建议修法：矩阵改为“编辑器、Markdown 源码态、Diff、GitShow”，与 `Editor` 上下文定义一致；或在矩阵脚注说明“编辑器”包含 Markdown 源码态。
- 裁决对象：规格修订

### acceptance-cross-domain-gaps [P2] 跨域验收表未覆盖面板封闭集、去重与状态保留矩阵

- owner：`docs/featureSpec/acceptance.md`
- 类型：缺口
- 定位：`acceptance.md` §跨域验收；`featureSpec/README.md` §跨域不变量、§状态保留矩阵
- 原文：“跨域场景单表：工作区、路由、Git 根、焦点、关闭、持久化、快捷键等跨篇契约在一张表内验收。”
- 现象：跨域表覆盖工作区、项目生命周期、身份、应用页、崩溃恢复、Commit/Explorer 范围、路由、Git 根、焦点、关闭、持久化、快捷键、通知、单实例、键盘、安全；未覆盖面板类型封闭集/未知类型丢弃、跨面板去重键（编辑器/Markdown/Diff/GitShow/设置中心）、侧栏视图封闭集与状态保留矩阵。这些均在 README 跨域不变量中作为全局契约出现。
- 影响：跨篇集成缺口没有单表验收；各 owner 验收可能各自通过，但组合行为（例如未知类型丢弃后空项目、去重键冲突、状态保留矩阵跨项目切换）无人统一验证。
- 证据：`featureSpec/README.md` §跨域不变量；`acceptance.md` §跨域验收表；各 owner 篇验收判据。
- 建议修法：至少增加“面板类型与去重”“侧栏视图封闭集与显隐状态”“状态保留矩阵”三行，引用 owner 判据；或在表前声明跨域表只覆盖跨组件集成，矩阵类契约由 owner 验收覆盖并给出索引。
- 裁决对象：规格修订

### acceptance-unautomatable-gaps [P3] 不可自动化面未列系统文件夹选择器与系统标题栏交互

- owner：`docs/featureSpec/acceptance.md`
- 类型：缺口
- 定位：`acceptance.md` §不可自动化面、§平台与第三方边界
- 原文：不可自动化面表列终端中文输入法、Markdown 视觉、Diff 视觉、拖拽手感、系统默认程序与回收站、GPU/字体。
- 现象：导航树添加项目/重定位使用系统文件夹选择器，工作区自绘标题栏的拖动、双击最大化、系统贴靠依赖操作系统窗口能力；两者均为 in-scope 且依赖真实系统外壳，但未登记为不可自动化面或平台边界。
- 影响：实现阶段可能以“无法自动化”为由跳过登记，或误把虚拟窗口测试当作完整覆盖。
- 证据：`featureSpec/workspace/navigation.md` §创建/§重新定位根目录；`featureSpec/workspace/shell.md` §窗口、标题栏与单实例；`acceptance.md` 两张表。
- 建议修法：在不可自动化面或平台边界增加对应行，给出兜底层级（虚拟窗口状态机 + 实机冒烟/人工核验）。
- 裁决对象：规格修订

### acceptance-runtime-v1-gap [P3] Runtime v1 删除未进入 featureSpec 明确不做

- owner：`docs/featureSpec/acceptance.md`、`docs/devPlan/spec-issues.md`
- 类型：缺口
- 定位：`spec-issues.md` §四 删除与收敛；`acceptance.md` §明确不做、§范围外
- 原文：“Runtime v1 | 删除；v2 不保留兼容层。”
- 现象：spec-issues 删除表列出 Runtime v1，acceptance 的“明确不做”与“范围外”只写 Runtime API 属于迁移能力，没有“不保留 v1 兼容层/不读旧端口格式”的验收边界；负向断言只在 `design/testing.md`。
- 影响：规格层验收无法覆盖 v1 兼容性回归；实现可能以“范围外”名义保留旧入口。
- 证据：`spec-issues.md` §四；`acceptance.md` §范围外；`design/testing.md` §范围外与删除能力“Runtime v1 兼容层”负向断言。
- 建议修法：在 acceptance 范围外补一句“Runtime API 迁移不保留 v1 方法、旧环境变量、旧端口格式与兼容层”，或将该项明确标为设计层删除、不在规格验收内。
- 裁决对象：规格修订

### public-release-gate-gap [P3] 公开发布门未说明 AI Providers 与终端增强是否阻塞发布

- owner：`docs/devPlan/plan.md`
- 类型：方向建议
- 定位：`plan.md` §分支、提交与回滚“公开发布门”
- 原文：“Runtime API v2、应用页范围外节、更新链与加密备份完成前只发内部构建，不发公开版本。”
- 现象：AI Providers/助手与终端增强回迁都是计划中的迁移里程碑，但不在公开发布门清单内；若它们属于必须回归的产品能力，发布门不完整；若可后置，文档未说明。
- 影响：发布判定歧义，可能出现公开版本缺少已承诺迁移能力的争议。
- 证据：`plan.md` §里程碑 AI Providers/终端增强；§分支、提交与回滚公开发布门；`spec-issues.md` §一“vi 模式、终端补全、数学覆盖层、AI 修复条保留为迁移能力”。
- 建议修法：明确两者是“发布阻塞”还是“可后续版本交付”；若阻塞，加入发布门；若可后置，在范围外文档与发布说明中标注。此建议不触碰产品定位约束，但涉及发布范围，需用户裁决。
- 裁决对象：计划修订

### snapshot-count-crate-count [P3] 使用代码可自证的 crate 数量

- owner：`docs/devPlan/README.md`、`docs/devPlan/plan.md`
- 类型：方向建议
- 定位：`devPlan/README.md` §关键裁决；`plan.md` §里程碑“领域模型与骨架”
- 原文：“新增五个 core crate”“五个 crate 可编译”。
- 现象：crate 集合已在同一句列出名称，数量可由 workspace 成员自证；文档策略禁止代码可自证的条目计数。
- 影响：新增/合并 crate 时数量词先于名称腐化，产生无意义修订。
- 证据：`devPlan/README.md` §关键裁决；`plan.md` 该里程碑；`.claude/CLAUDE.md` 文档规范“快照数字禁令”。
- 建议修法：删除数量词，保留 crate 名称列表；退出条件改为“全部新 core crate 可编译”。
- 裁决对象：计划修订

### process-label-rot [P3] 文档保留“优化轮/本轮/本次”等过程性标签

- owner：`docs/devPlan/spec-issues.md`、`docs/devPlan/design/migration.md`、`docs/devPlan/design/out-of-scope/update-release.md`
- 类型：方向建议
- 定位：`spec-issues.md` §五“优化轮裁决”、§六“目标态优化裁决”；`migration.md` §测试与验收“本轮只更新”；`update-release.md` §发布链“本次目标态包清单变更”
- 原文：“## 五、优化轮裁决（施工图收敛）”
- 现象：章节名与正文使用过程轮次/本次/本轮的相对时间词，未带版本或日期锚点；文档策略禁止过程编号与无锚点的演进表述。
- 影响：过程结束后标签失去含义，后续读者无法判断“本轮”指哪次变更。
- 证据：`.claude/CLAUDE.md` 文档规范“短标识符禁令”“事实文档防腐”；上述章节标题与正文。
- 建议修法：按内容命名章节（如“施工图收敛裁决”“目标态优化裁决”），把“本轮/本次”改为带日期/版本锚点的表述或删除。
- 裁决对象：计划修订

## 交叉引用卡

以下问题不在本域定级，交对应 owner 域处理。

- **startup-visibility**：owner `design/architecture.md`（启动链）。`architecture.md` §启动链写“创建默认隐藏窗口……界面挂载：数据就绪后以标题栏 + 工作区 + 通知区替换加载界面”；`platform-windows.md` §窗口几何写“正常启动显示窗口但不抢键盘焦点”；`out-of-scope/app-and-shell-services.md` §壳生命周期写“打开主窗（首版始终可见）→……→ 恢复项目与布局”；`featureSpec/workspace/shell.md` §启动要求显示全屏“slTerminal 启动中…”。窗口在数据就绪前是否可见、是否存在加载界面，三处不一致。
- **state-db-backup-entry**：owner `design/persistence.md`（损坏、备份与只读）。该篇写“启动时主库不可用时提供‘从备份恢复’入口”；`featureSpec/workspace/shell.md` §启动与 `architecture.md` §启动链只定义“重试”与“以空状态继续”两个动作。多出的用户可见入口缺少规格依据。
- **process-snapshot-ttl**：owner `design/platform-windows.md` 与 `design/integrations/agent-cli.md`。平台篇 §进程树终止写“进程快照按 250ms TTL 复用”；agent-cli 篇 §进程快照与身份写“快照 TTL 1s”。同一进程快照契约两处数值不一致。
- **diff-dedupe-source**：owner `design/domain-model.md`（去重键）。`spec-issues.md` §五 写“Diff 去重键含 `mode/source`”；`domain-model.md` 的 `DedupeKey::Diff` 只有 `mode: DiffMode`，没有 `source`；`design/workspace/shell.md` 去重表写“面板模式/后缀”。来源是否参与身份未统一。
- **handoff-workspace-snapshot-owner**：owner `design/build-and-release.md` 与 `design/persistence.md`。`out-of-scope/update-release.md` §安装交接定义 `HandoffWorkspaceV2`（含 `window` 与 `sidebar`）并声称由本节拥有；窗口几何归 `platform-windows.md`，侧栏 schema 归 `persistence.md`，更新交接归 `build-and-release.md`。该快照类型没有在所有者索引登记。
- **testing-trace-readme**：owner `design/testing.md`（验收追溯）。追溯表从 `workspace/shell.md` 开始，没有 `featureSpec/README.md` 行；而该篇拥有术语、面板能力矩阵、状态保留矩阵、快捷键矩阵、跨域不变量与关键跨域决策。`spec-issues.md` §七要求“追踪矩阵覆盖全部 featureSpec 篇目”，当前无法满足。
- **testing-trace-milestone-gaps**：owner `design/testing.md` 与 `plan.md`。计划“应用基础服务”交付物含主题 token，但验收映射只列 `cross-cutting、ux-framework、platform-windows、diagnostics、security`，未列 `theme-and-i18n`；testing 篇虽有 theme/i18n 测试要点，追溯表的里程碑列未绑定到该里程碑。主题 token 的交付缺少里程碑级验收来源。
