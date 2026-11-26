# 独立审查汇总

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`（当前分支）；汇总 23 份独立分报告与独立复核台账，只做导航、聚合与排序，不重新验证代码、不运行构建、测试或网络。

## 审查基线与方法

- 仓库：`D:/data/learn/code/slTerminal`；基线 HEAD `53c2a1504a287917f78e58f580a7bd4b3e752060`，当前分支；只审当前工作树，不做历史 diff。
- 审查环境：Windows NT 10.0.26300，PowerShell 7.7.0，rustc 1.97.1，cargo 1.97.1；门禁执行日期 2026-11-26。
- 输入：`docs/review/` 下 23 份分报告、`docs/review/verification-ledger.md`、作业临时目录下的门禁摘要与关键日志。门禁日志为外部输入，本次汇总未复跑。
- 方法：分报告负责取证与提出怀疑；复核台账重读源码、文档与测试后裁定确认、驳回或无法验证，并给出最终严重度；本汇总以台账的最终严重度与结论为准，合并同一问题的全部来源，不重新判定代码。
- 判据层级：`featureSpec` 为行为真值，`docs/devPlan` 为实现约束，`.claude/docs` 与 `CLAUDE.md` 为开发纪律，代码为被审对象；判据互相冲突时不预设谁对，单列待裁决。
- 汇总边界：只列问题，不写优点、修复计划、代码补丁或建议步骤；详细证据留在分报告与台账。

## 门禁执行证据

| 门禁 | exit | 结果说明 |
|---|---|---|
| build | 0 | 构建通过。 |
| fmt | 0 | 格式检查通过。 |
| clippy-workspace | 0 | 输出全部为同一 `src/lib.rs` 同时属于 `lib` 与 `integration-test` target 的 Cargo 清单警告，无其他 clippy 诊断。 |
| clippy-gpui | 0 | 与 workspace 相同，全部为重复 target 清单警告。 |
| test-workspace | 0 | 全量测试结束为 `test result: ok`。 |
| test-release-core | 0 | core release 测试结束为 `test result: ok`。 |
| test-gpui | 0 | GPUI 测试结束为 `test result: ok`。 |
| check-architecture | 0 | 架构与依赖门禁通过。 |
| check-doc-policy | 0 | 文档策略门禁通过；该结果只证明脚本规则集未命中，不证明台账所列文档问题不存在。 |
| check-brand | 0 | 禁名扫描通过。 |
| script-unittests | 1 | 首轮在共享 `%TEMP%` 下失败一项；失败输出显示测试夹具向上遍历到共享临时目录并读取无关 notice 文件。 |
| script-unittests-clean-temp | 0 | 干净临时目录重跑全部通过，含条件跳过项；证明确认失败根因是测试隔离缺陷而非产品逻辑失败。 |
| package-selftest | 0 | 打包自检通过。 |
| asset-previews | 0 | 资产预览检查通过。 |
| cargo-audit | 1 | 报告安全公告并因漏洞非零退出；仓库无 audit/deny 例外配置，CI 无 audit 步骤。 |
| cargo-tree-duplicates | 0 | 依赖重复树检查通过。 |
| cargo-metadata | 0 | 元数据生成通过。 |
| orphans | 0 | 日志为空，测试后未发现孤儿进程。 |
| git-status | 0 | 工作树仅 `docs/review/` 未追踪。 |

- 脚本自测首轮失败与干净 TEMP 重跑全过共同表明：用例在隔离临时根下通过；共享 `%TEMP%` 被无关文件污染时常规门禁仍可能失败；该测试隔离缺陷本身未修复。
- `cargo audit` 命中的安全公告包括 `cookie` 的 RUSTSEC-2026-0594、`ctutils` 的 RUSTSEC-2026-0385、`rustls-webpki` 的 RUSTSEC-2026-0382、`url` 的 RUSTSEC-2026-0592、`webpki-roots` 的 RUSTSEC-2026-0381。
- `cargo audit` 告警覆盖多类：unmaintained 包括 `bincode`、`chrono`、`instant`、`paste`、`rustls-pemfile`、`rustybuzz`、`ttf-parser`、`yaml-rust`、`zbus_names`、`zvariant`、`zvariant_derive`；unsound 包括 `heapless` 多组公告与 `rustix`；yanked 为 `async-executor`。
- `progress.md` 记录的 GPUI 测试通过数低于门禁日志聚合值，说明进度中的运行快照计数已过期；该差异也可能是不同 target 或不同时间的口径差异，需以同一命令、同一基线复核。
- 门禁全绿只证明构建、静态检查与测试命令通过；不证明台账所列产品入口存在、运行时行为正确或验收锚语义成立。

## 覆盖矩阵

| 分报告 | 审查范围 | 报告文件 |
|---|---|---|
| app-root-content | 应用壳内容域：Document、Editor、Diff、GitShow、Explorer、Commit、文件操作、watcher、粘贴。 | `docs/review/app-root-content.md` |
| app-root-lifecycle | 应用壳生命周期：AppRoot 所有权、焦点、布局、统一关闭、启动与单实例移交、侧栏与页签。 | `docs/review/app-root-lifecycle.md` |
| app-root-settings-runtime-ui | 设置中心、Runtime 快照 UI、项目/导航/语言/主题库与 i18n。 | `docs/review/app-root-settings-runtime-ui.md` |
| architecture-contracts | 架构与跨模块合同：依赖、所有权、路径、关闭、备份、更新、Runtime 与验收锚。 | `docs/review/architecture-contracts.md` |
| completion-learning | 终端补全、建议、本地学习、项目脚本与 workspace 发现。 | `docs/review/completion-learning.md` |
| concurrency-resources | 跨模块并发、任务、线程与资源生命周期。 | `docs/review/concurrency-resources.md` |
| docs-spec | featureSpec、devPlan、`.claude/docs`、各级 `CLAUDE.md`、术语表与用户文档一致性。 | `docs/review/docs-spec.md` |
| document-core | `slterm_document` 编辑、注册、外部变化、编码、搜索、Diff、Markdown 投影与安全投影。 | `docs/review/document-core.md` |
| gpui-terminal-view | GPUI 终端视图、按键、粘贴、OSC、搜索、恢复与会话端点。 | `docs/review/gpui-terminal-view.md` |
| hooks-agent-integration | Hooks 域、hook 小进程桥、安装/卸载、草稿与 Agent 集成。 | `docs/review/hooks-agent-integration.md` |
| markdown-math | Markdown 投影、渲染、资源安全链、数学引擎与公式渲染。 | `docs/review/markdown-math.md` |
| performance-crosscut | 跨模块性能、资源预算与热路径。 | `docs/review/performance-crosscut.md` |
| persistence-backup | 持久化、崩溃恢复、Document 快照、加密备份与维护链。 | `docs/review/persistence-backup.md` |
| plan-audit | devPlan 完成情况、进度/追溯证据链、豁免与发布说明。 | `docs/review/plan-audit.md` |
| platform-windows | Windows 平台集成、更新、套餐、通知、诊断、清理、语言与窗口几何。 | `docs/review/platform-windows.md` |
| runtime-v2 | Runtime v2 协议、服务、派发、事件环、随包 schema 与 AppRoot 接线。 | `docs/review/runtime-v2.md` |
| scripts-ci-release | 脚本、CI、发布链、许可、安装器与生成资产。 | `docs/review/scripts-ci-release.md` |
| security-crosscut | 凭据与审计、路径授权、命令执行、HTML/SVG、更新链、备份、Hooks、unsafe/FFI、依赖与许可。 | `docs/review/security-crosscut.md` |
| settings-agents | 设置、Agent 域、主题、历史隔离与套餐凭据。 | `docs/review/settings-agents.md` |
| terminal-core | `slterm_terminal` 事件循环、PTY、终端状态、搜索与真实 PTY 测试。 | `docs/review/terminal-core.md` |
| test-quality | 测试用例质量、覆盖层级、隔离、追溯例名与门禁接线。 | `docs/review/test-quality.md` |
| textview-editor | `text_view`、`file_editor`、分页读取与编辑器接线。 | `docs/review/textview-editor.md` |
| workspace-shell | `slterm_workspace`、`slterm_sidebar`、`slterm_split`、`slterm_path` 与路径授权接线。 | `docs/review/workspace-shell.md` |

## 严重度分布

按复核台账去重后的独立问题计：台账中标注“同源并入”“已并入”的条目合并入主条目；同一问题跨报告只计一次；不含分报告中未被台账单列的条目。

| 最终严重度 | 数量 |
|---|---|
| 阻断 | 37 |
| 重要 | 166 |
| 一般 | 30 |
| 提示 | 6 |
| 合计 | 239 |

## 跨模块系统性风险

- **引擎态与生产接线脱节**：更新、套餐余量、诊断/清理、普通设置导入导出、自定义主题库、导航历史、历史隔离、加密备份、托盘快照等均存在引擎或服务实现，但启动链、设置页或命令面没有消费点；同一模式还出现在 hook 管道、Document 崩溃快照与 Runtime 事件面。代表条目：更新链在运行态没有入口、套餐余量功能在运行态不存在、诊断与清理没有产品面、加密备份没有生产入口、Document 崩溃快照只有恢复器、生产环境没有 hook 事件入口。来源：`architecture-contracts.md`、`platform-windows.md`、`plan-audit.md`、`persistence-backup.md`、`hooks-agent-integration.md`、`settings-agents.md`。
- **关闭与决定入口**：交互式关闭、批量/项目/应用关闭、恢复失败分支与 Hooks 草稿没有统一的保存/丢弃/取消决定面；Runtime 非交互关闭又在保存、停止与清理落定前返回成功。代表条目：交互式关闭没有保存/丢弃/取消决定入口、恢复失败分支跳过局部关闭段、项目移除 Save 阶段没有代次重收集、Hooks 草稿未进入应用退出与面板关闭守卫、非交互关闭在保存/停止/清理落定前返回 Succeeded。来源：`app-root-content.md`、`app-root-lifecycle.md`、`hooks-agent-integration.md`、`runtime-v2.md`。
- **授权与路径边界**：授权代次在生产路径恒为零；关闭保存漏掉 AgentRoot 能力并集；Markdown 本地资源先解析后授权且接受 UNC；Runtime `git.diff` 接受盘符绝对路径并可跟随符号链接；显式终端 cwd 只做词法包含；Git 读取未统一禁网、deadline 与凭据提示约束。代表条目：授权代次在生产路径恒为零、本地图片路径在授权前被解析、Runtime `git.diff` 路径校验放过盘符绝对路径/符号链接、显式终端 cwd 只做词法包含校验、Git 读取未统一遵守有界执行/禁网与凭据提示约束。来源：`app-root-content.md`、`markdown-math.md`、`runtime-v2.md`、`workspace-shell.md`、`security-crosscut.md`。
- **加密备份**：没有工作区类别与逻辑工作区导出；没有生产入口、预览、跨重启恢复点与撤回；解密条目以明文暂存且暂存先于 owner 记录；没有明文大小上限；设置类别使用敏感键黑名单；灾难副本不生成不刷新；恢复点与暂存目录缺少当前用户 ACL。代表条目：加密备份没有工作区类别、加密备份没有生产入口/预览/原子导出/跨重启恢复点与撤回、`stage_restore` 把解密后的备份条目明文写盘、备份明文没有 64 MiB 上限、设置导出使用敏感键黑名单、灾难副本从未生成与刷新。来源：`persistence-backup.md`、`security-crosscut.md`、`docs-spec.md`、`architecture-contracts.md`。
- **崩溃快照与恢复**：Document 快照只有恢复器，没有编辑/保存/关闭/退出写入与删除链路；快照索引与历史隔离表无业务写入；恢复失败或布局解码失败时以空工作区开窗；维护请求停在 `NeedsAttention` 后没有继续、放弃或清理入口；双损隔离部分失败后重试以空库启动。代表条目：Document 崩溃快照只有恢复器、布局或快照领域恢复失败时静默以空工作区开窗、未完成关闭意图/NeedsAttention/遗留恢复点只有阻断、双损隔离部分失败或中断后重试以空工作区正常启动。来源：`concurrency-resources.md`、`persistence-backup.md`、`app-root-content.md`。
- **Runtime 事件与操作**：快照 revision 固定为握手起点，事件积压后进入不可恢复 resync 循环；`expected_revision` 线上未生效；Operation 在持久快照提交前报告成功且停在 Queued/Running 后没有恢复；事件面只发布项目激活；schema 未覆盖多数请求结构且不在固定门禁内；双进程 conformance 缺失。代表条目：快照 revision 固定为握手起点、非交互关闭在落定前返回 Succeeded、expected_revision 在线上端到端未生效、Operation 停在 Queued/Running 后没有恢复路径、Runtime 合同测试不在任何固定门禁内、Runtime 双进程 conformance 与事件断线矩阵缺失。来源：`runtime-v2.md`、`concurrency-resources.md`、`test-quality.md`、`scripts-ci-release.md`。
- **终端与 PTY**：存活上限预检发生在真实 PTY 创建之后且拒绝路径在前台 join worker；空输入可永久卡住写队列；重连期间旧 worker 仍可注入事件；退出时不排空已读输出；读取线程 EOF 不转故障事件；OSC 缓冲无界；搜索每帧重查；退出后键盘复制/搜索/回滚失效；终端外观与运行时设置不热应用。代表条目：存活上限预检发生在真实 PTY 创建之后、空 `Msg::Input` 永久卡住写队列、重连期间旧 worker 仍可注入退出事件或显示命令、子进程退出时未排空已读输出、读取线程 EOF 或读取错误不会转成退出/故障事件、OSC 原始缓冲无界、搜索的帧驱动重查会重置导航位置并形成搜索风暴。来源：`gpui-terminal-view.md`、`terminal-core.md`、`performance-crosscut.md`。
- **设置、导航、AgentRoot 产品面**：设置中心页面集合与导航结构未按规格实现，导入/导出/备份/诊断/更新/Runtime/AgentRoot/套餐凭据入口缺失；大量控件没有生产消费方；导航历史与活跃会话不可达；AgentRoot 真实目录查重绕过 Registry，改路径沿用旧能力；主题库没有管理界面且主题模型仍承载非颜色字段。代表条目：设置中心页面集合与导航结构未按规格实现、设置中心缺少导入/导出/备份/诊断/更新/Runtime/AgentRoot 与套餐凭据入口、Agent 基础配置/AgentRoot 管理与 Agent 文件操作面未实现、项目重定位与导航历史恢复没有生产入口、真实目录查重漏掉内置根/生产路径绕过 Registry。来源：`app-root-settings-runtime-ui.md`、`settings-agents.md`、`plan-audit.md`、`docs-spec.md`。
- **文档与追溯漂移**：追溯矩阵引用不存在的例名或把纯函数/服务层用例当作已自动化；全量验收锚以源码字符串和局部状态代替跨域行为；测试豁免表含失效设计路径与不存在的例名；架构总览、README、CHANGELOG 与发布说明仍描述旧栈；进度把未接线的产品面标为已验收。代表条目：追溯矩阵引用不存在的测试例名、部分矩阵例名存在但覆盖的不是声明场景、全量验收锚以源码字符串和局部状态代替跨域行为、测试豁免表含失效设计路径/目标态待实现项与超范围无障碍项、架构总览与常驻规则仍使用旧栈数据名/类型名与已取消的校验。来源：`plan-audit.md`、`test-quality.md`、`docs-spec.md`、`architecture-contracts.md`、`hooks-agent-integration.md`、`persistence-backup.md`、`runtime-v2.md`、`settings-agents.md`、`textview-editor.md`、`app-root-content.md`。
- **测试隔离与证据层级**：脚本自测依赖共享 `%TEMP%` 并在污染时失败；AppRoot 虚拟窗口用例读取真实 home；Git 夹具未隔离全局配置；Runtime schema 测试受 feature 门控且不在 CI/发布门禁；双进程 conformance 缺失；终端容量、搜索与真实 ConPTY 用例使用内存替身或手动调用生产路径。代表条目：AppRoot 后台 Agent 根加载不可注入/虚拟窗口用例读取真实 home、Runtime 合同测试不在任何固定门禁内、Runtime 双进程 conformance 与事件断线矩阵缺失、真实 ConPTY 验收场景覆盖不足、测试语义与生产路径断裂。来源：`test-quality.md`、`app-root-content.md`、`runtime-v2.md`、`terminal-core.md`、`gpui-terminal-view.md`。
- **依赖健康与许可**：`cargo audit` 命中安全公告、unmaintained、unsound 与 yanked 依赖；CI 无 audit 常驻门禁；内嵌 Maple Mono 字体缺少 OFL 许可与版权文本；许可审计根包名与实际 workspace 包名不一致；手工 notice 缺版权行并引用不存在路径；dav1d 从可变 tag 克隆且审计 commit 不进入构建。代表条目：生产 TLS 栈命中 emSign 弃用中间证书公告、依赖审计未进入 CI 常驻门禁、内嵌 Maple Mono 字体缺少 OFL 许可与版权文本、许可审计根包名与实际 workspace 包名不一致、dav1d 原生源从可变 tag 克隆。来源：`security-crosscut.md`、`scripts-ci-release.md`。
- **性能热路径**：Markdown 每次滚动清空全部已渲染 outcome 并在前台复制整篇文档文本；渲染查找在前台按匹配数乘块数扫描；高 DPI 公式位图被放大后按 1:1 显示；布局缓存只插入不淘汰；终端搜索每帧重查；Diff 与 Markdown 的共享编辑缺少可见性门控与取消协议。代表条目：每次滚动清空全部已渲染 outcome、每次滚动在前台复制整篇文档文本、渲染查找在前台按匹配数乘块数扫描、布局缓存没有容量或可见窗口回收、搜索的帧驱动重查会重置导航位置并形成搜索风暴。来源：`markdown-math.md`、`performance-crosscut.md`、`textview-editor.md`、`gpui-terminal-view.md`、`terminal-core.md`。

## 合同冲突

以下冲突需要所有者裁决；本次汇总不预设哪一方为真值。

| 冲突点 | 一方 | 另一方 |
|---|---|---|
| GitShow 非法 UTF-8 | `docs/featureSpec/panels/gitshow.md` 要求非 UTF-8 字节以替换字符显示并给出正文外提示。 | `slterm_app/src/git/CLAUDE.md` 要求非法 UTF-8 不生成伪文本；实现返回 `InvalidUtf8` 并只提供系统打开。 |
| WCAG 对比度校验 | `docs/featureSpec/acceptance.md` 与根开发纪律要求不做对比度校验、不提示、不自动回退。 | `slterm_settings/CLAUDE.md` 与 `theme_library/CLAUDE.md` 要求 WCAG 必测/通过；代码仍保留 `validate_wcag`、阈值与建议色。 |
| 主题职责边界 | 设计要求主题只负责颜色。 | 主题值模型与模块合同要求排版、布局、效果、背景图与光标形状等字段，并写回用户设置。 |
| 加密备份 AAD 与恢复时序 | 设计写“完整头部作为 AAD”，且运行期只暂存、启动维护期应用。 | `encrypted_backup/CLAUDE.md` 写“魔数同时作 AAD”，并把 `restore_selected` 描述为生产接缝；代码只把 `MAGIC` 作为 AAD。 |
| 静默启动触发范围 | featureSpec 与设计称仅登录自启。 | 平台 `CLAUDE.md` 与实现使用常规启动、设置与托盘可用合取；启动项与手动启动不可区分。 |
| Mermaid 终态 | `docs/featureSpec/panels/markdown.md` 要求静态渲染与失败源码/错误。 | `docs/devPlan/design/markdown.md` 与 devPlan README 明确本轮只保留普通代码块；追溯标为延期，进度又称全部阶段验收完成。 |
| `app.close` 回执语义 | featureSpec 要求脏关闭保存失败返回失败并保持现场。 | `local_control/CLAUDE.md` 与协议 crate 文档把成功定义为“关闭已开始推进”；实现对 `Started`/`Deferred` 均返回成功。 |
| 非交互关闭的终端处置 | 设计要求 `app.close`/`panel.close`/`project.remove` 显式提供 Document、Hooks 与运行终端处置策略。 | 线缆请求只有 documents 与 hooks；实现把终端对象固定为 `CloseDecision::NotRequired` 并直接停止 PTY。 |
| `Ctrl+Shift+A` 键位归属 | 终端 featureSpec 要求该组合执行全选。 | 侧栏 Agent 文件以同一组合注册为无上下文全局绑定；终端全选能力同时缺失。 |

## 延期与豁免审计

- **已登记豁免存在失效引用**：`.claude/docs/testing/exemptions.md` 引用 `design/ux-framework.md`、`design/panels/*.md`、`design/platform-windows.md` 等当前不存在的路径；引用 `spawn_lock_serializes_and_capacity_32_enforced`、`mirror_da1_query`、`code_and_math_edit_bodies_without_touching_authored_fences` 等仓内不存在的例名；AccessKit 行与 featureSpec 排除项冲突；加密备份 UI 人工验收没有对应行；多条“目标态待实现登记”与进度“全部验收完成”矛盾。
- **未登记缺口**：历史隔离的到期清理、ZIP 导出与永久删除没有实现，也没有追溯行或豁免；Runtime 双进程 conformance 缺失，豁免表无条目；Runtime schema 合同测试受 feature 门控且不在命令矩阵、CI 或发布门禁，未登记豁免；`release.ps1` 的安装器参数测试缺失、运行时钉测试未执行，命令矩阵与打包文档却把它们列为钉测试；`cargo audit` 未进入 CI，仓库无 audit/deny 例外配置；AppRoot 虚拟窗口用例的真实 home 读取与 Git 夹具的全局配置依赖未登记为隔离缺口。
- **延期有效性**：Mermaid 的“用户裁决延期”没有日期、范围或决策记录，无法确认仍有效；安装器、真实安装与更新移交被延期到发布机，属于真实外部边界，但进度已把 Windows 与发布、全量验收标为完成，豁免表只有人工兜底而没有完成记录；导航树、设置中心、AgentRoot 等产品面缺失可能是未记录的产品缩减，当前没有延期或范围调整说明；Win10、真实 GUI、真实凭据库等人工面同样缺少带日期的核验记录。
- **审计结论**：豁免表不能作为“每个场景都有测试或豁免”的闭合证据；有效延期应同时具备所有者裁决、日期/范围锚、追溯状态回退与兜底记录，当前多数不满足。

## 完整问题索引

本索引只列复核台账确认的条目；同一问题跨报告合并，严重度取台账最终值。来源后的路径为该报告的仓库路径。

### 应用壳、关闭与生命周期

- 交互式关闭没有保存/丢弃/取消决定入口｜阻断｜来源：app-root-content、app-root-lifecycle、hooks-agent-integration｜路径：`docs/review/app-root-content.md`、`docs/review/app-root-lifecycle.md`、`docs/review/hooks-agent-integration.md`
- Commit 永久删除确认把任意失败升级为绕过执行体的强删｜阻断｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 未暂存删除回滚删除任意同内容未跟踪文件｜阻断｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 文件树 watcher 未接线｜阻断｜来源：app-root-content、concurrency-resources｜路径：`docs/review/app-root-content.md`、`docs/review/concurrency-resources.md`
- 外部冲突与另存为没有生产落点｜阻断｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 恢复失败分支跳过局部关闭段，已收集决定被丢弃｜阻断｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- Hooks 草稿未进入应用退出与面板关闭守卫｜阻断｜来源：app-root-lifecycle、hooks-agent-integration、app-root-settings-runtime-ui｜路径：`docs/review/app-root-lifecycle.md`、`docs/review/hooks-agent-integration.md`、`docs/review/app-root-settings-runtime-ui.md`
- 项目移除 Save 阶段没有代次重收集，失配后永久失败｜阻断｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- 关闭保存授权漏掉 AgentRoot 能力并集｜重要｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 页签缺少脏标记、CLI 状态与单面板关闭入口｜重要｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- Runtime 关闭策略覆盖非最后引用文档｜重要｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- 无项目浮层 Hooks 页没有草稿键｜重要｜来源：app-root-lifecycle、app-root-settings-runtime-ui｜路径：`docs/review/app-root-lifecycle.md`、`docs/review/app-root-settings-runtime-ui.md`
- IME 组合期间全局命令仍经 GPUI 键位层派发｜重要｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- 布局与侧栏恢复警告只写日志｜重要｜来源：app-root-lifecycle、workspace-shell｜路径：`docs/review/app-root-lifecycle.md`、`docs/review/workspace-shell.md`
- 批量/项目关闭遗留 pane 级任务栏与 AgentHub 清理｜一般｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- 活动栏缺少固定底部配置按钮（并入：设置悬浮窗无活动栏配置按钮）｜重要｜来源：app-root-lifecycle、app-root-settings-runtime-ui｜路径：`docs/review/app-root-lifecycle.md`、`docs/review/app-root-settings-runtime-ui.md`
- 浮层已打开时设置入口改开项目面板｜重要｜来源：app-root-lifecycle｜路径：`docs/review/app-root-lifecycle.md`
- Commit/Explorer 数据根切换时在途状态读取被丢弃或错投｜重要｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 项目重定位与导航历史恢复没有生产入口｜阻断｜来源：app-root-content、app-root-settings-runtime-ui、plan-audit、settings-agents、workspace-shell｜路径：`docs/review/app-root-content.md`、`docs/review/app-root-settings-runtime-ui.md`、`docs/review/plan-audit.md`、`docs/review/settings-agents.md`、`docs/review/workspace-shell.md`
- 启动恢复对缺失/不可解码的干净 Document 不建面板｜重要｜来源：app-root-content、persistence-backup｜路径：`docs/review/app-root-content.md`、`docs/review/persistence-backup.md`
- Agent 根路径变更（同 id）沿用旧 ViewState｜重要｜来源：app-root-content｜路径：`docs/review/app-root-content.md`
- 授权代次在生产路径恒为零｜重要｜来源：app-root-content、architecture-contracts、workspace-shell｜路径：`docs/review/app-root-content.md`、`docs/review/architecture-contracts.md`、`docs/review/workspace-shell.md`
- GitShow 非法 UTF-8 的合同冲突｜重要｜来源：app-root-content｜路径：`docs/review/app-root-content.md`

### 设置、主题、导航与 Agent

- 设置中心页面集合与导航结构未按规格实现（并入：设置中心页面集合未按规格交付（文档域））｜阻断｜来源：app-root-settings-runtime-ui、docs-spec、plan-audit｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/docs-spec.md`、`docs/review/plan-audit.md`
- 设置中心缺少导入、导出、备份、诊断、更新、Runtime、AgentRoot 与套餐凭据入口（并入：Runtime 运行状态与凭据失败没有设置界面可见面）｜阻断｜来源：app-root-settings-runtime-ui、docs-spec、plan-audit、platform-windows、settings-agents、security-crosscut、runtime-v2｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/platform-windows.md`、`docs/review/settings-agents.md`、`docs/review/security-crosscut.md`、`docs/review/runtime-v2.md`
- 自定义主题库没有设置界面，主题选择只列内置主题（并入：自定义主题的选择、导入、重命名与删除没有生产入口）｜重要｜来源：app-root-settings-runtime-ui、settings-agents｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/settings-agents.md`
- 主题文档仍强制承载排版、卡片几何、背景图与效果字段｜重要｜来源：app-root-settings-runtime-ui、docs-spec、settings-agents｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/docs-spec.md`、`docs/review/settings-agents.md`
- 最终排除项仍在设置页暴露｜重要｜来源：app-root-settings-runtime-ui、docs-spec、plan-audit｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/docs-spec.md`、`docs/review/plan-audit.md`
- WCAG 对比度校验与提示残留，合同互相矛盾｜重要｜来源：app-root-settings-runtime-ui、docs-spec、architecture-contracts、settings-agents｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/docs-spec.md`、`docs/review/architecture-contracts.md`、`docs/review/settings-agents.md`
- 设置页大量控件没有生产消费方｜重要｜来源：app-root-settings-runtime-ui、gpui-terminal-view、terminal-core、completion-learning｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`、`docs/review/completion-learning.md`
- 设置保存状态与失败不可见；文本/颜色控件跨视图不同步且缺少路径校验｜重要｜来源：app-root-settings-runtime-ui、settings-agents｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/settings-agents.md`
- 根重定位实现为 dead code，缺少真实路径查重与决定守卫｜重要｜来源：app-root-settings-runtime-ui、workspace-shell｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/workspace-shell.md`
- 重复 Settings 恢复不按规格保留首个并提示｜重要｜来源：app-root-settings-runtime-ui｜路径：`docs/review/app-root-settings-runtime-ui.md`
- 主题下拉选择不清除 custom_theme｜重要｜来源：app-root-settings-runtime-ui、settings-agents｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/settings-agents.md`
- Runtime v2 快照没有设置界面，快照测试未覆盖资源边界｜重要｜来源：app-root-settings-runtime-ui、runtime-v2｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/runtime-v2.md`
- 测试主要覆盖引擎函数与 DTO，未覆盖页面场景｜重要｜来源：app-root-settings-runtime-ui、settings-agents、plan-audit、test-quality｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/settings-agents.md`、`docs/review/plan-audit.md`、`docs/review/test-quality.md`
- Hooks 页只实现 Claude JSON 子集，缺少 GUI、事件目录、外部监听、托管只读与保存提示｜重要｜来源：app-root-settings-runtime-ui、hooks-agent-integration｜路径：`docs/review/app-root-settings-runtime-ui.md`、`docs/review/hooks-agent-integration.md`
- 浮层不持久化 ViewState，失效页回退未接线（并入：无项目浮层不持久化 ViewState、失效页回退未接线）｜重要｜来源：app-root-settings-runtime-ui｜路径：`docs/review/app-root-settings-runtime-ui.md`
- 真实目录查重漏掉内置根，生产路径绕过 Registry｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- 改路径沿用旧根能力｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- agent.start 注入 slug 而不是 AgentCatalog 内置命令｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- Claude 无原生会话 ID 时仍生成 `--continue`｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- 撤销状态在恢复成功前被置为 Undone｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- quarantine_batch 不先持久化清单，部分失败丢失材料身份｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- 主题身份不是 ThemeId/UUIDv7，导入冲突被自动改名｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- ANSI 16–255 被持久化成第二份色表｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- 设置损坏分流缺“高版本”和“权限/IO 只读重试”｜重要｜来源：settings-agents、persistence-backup｜路径：`docs/review/settings-agents.md`、`docs/review/persistence-backup.md`
- 设置写失败、损坏与单字段回退没有用户可见反馈｜重要｜来源：settings-agents、app-root-settings-runtime-ui｜路径：`docs/review/settings-agents.md`、`docs/review/app-root-settings-runtime-ui.md`
- 恢复默认旁路统一设置写通道、审计与保存锁｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- AgentCatalog 适配器声明未被消费，CLI 名单多源｜重要｜来源：settings-agents、hooks-agent-integration｜路径：`docs/review/settings-agents.md`、`docs/review/hooks-agent-integration.md`
- Hooks 事件目录远小于 featureSpec 的 Claude 事件集合｜重要｜来源：settings-agents、hooks-agent-integration｜路径：`docs/review/settings-agents.md`、`docs/review/hooks-agent-integration.md`
- 设置热应用在 GPUI 前台同步读取自定义主题文件｜重要｜来源：settings-agents｜路径：`docs/review/settings-agents.md`
- 套餐凭据目标不含本地根稳定散列｜重要｜来源：settings-agents、security-crosscut｜路径：`docs/review/settings-agents.md`、`docs/review/security-crosscut.md`
- 普通设置导入、导出与恢复默认只有内存纯函数｜阻断｜来源：settings-agents、app-root-settings-runtime-ui｜路径：`docs/review/settings-agents.md`、`docs/review/app-root-settings-runtime-ui.md`
- Agent 基础配置、AgentRoot 管理与 Agent 文件操作面未实现｜阻断｜来源：settings-agents、app-root-settings-runtime-ui｜路径：`docs/review/settings-agents.md`、`docs/review/app-root-settings-runtime-ui.md`

### Hooks 与 hook 桥

- 生产环境没有 hook 事件入口｜阻断｜来源：architecture-contracts、concurrency-resources、hooks-agent-integration、platform-windows、security-crosscut｜路径：`docs/review/architecture-contracts.md`、`docs/review/concurrency-resources.md`、`docs/review/hooks-agent-integration.md`、`docs/review/platform-windows.md`、`docs/review/security-crosscut.md`
- 打开 Hooks 页留下无法清除的草稿，使项目移除/根重定位永久报 HooksDraftPending｜阻断｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- 管道事件不校验 pane 归属与 PTY 代次｜重要｜来源：hooks-agent-integration、security-crosscut｜路径：`docs/review/hooks-agent-integration.md`、`docs/review/security-crosscut.md`
- 注入/卸载没有 Journal 与启动恢复器｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- 危险命令确认没有生产链路｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- GUI 编辑模式与事件树/handler 表单不存在｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- Hooks 编辑保存绕过审计与 PathPolicy｜重要｜来源：hooks-agent-integration、security-crosscut｜路径：`docs/review/hooks-agent-integration.md`、`docs/review/security-crosscut.md`
- 加载路径不检测重复键｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- config_guard 没有生产调用点｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- AgentHooks 开关状态按 AgentHook::ALL 顺序索引 AGENTS 行｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- 保存进行中继续编辑会被保存回执静默覆盖｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- Hooks 层默认与切换错误｜重要｜来源：hooks-agent-integration、app-root-settings-runtime-ui｜路径：`docs/review/hooks-agent-integration.md`、`docs/review/app-root-settings-runtime-ui.md`
- Hooks 草稿任务身份只含面板｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- statusline 用量桥接与信号通道整体缺失｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- 追溯矩阵与进度把未接线的生产链记为已自动化/已验收｜重要｜来源：hooks-agent-integration、plan-audit、concurrency-resources、settings-agents｜路径：`docs/review/hooks-agent-integration.md`、`docs/review/plan-audit.md`、`docs/review/concurrency-resources.md`、`docs/review/settings-agents.md`
- Hooks 页面错误文案硬编码中文｜重要｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- 项目根不可访问时未禁用 Project/Local｜一般｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`
- Hooks 外部修改监听与重新加载缺失｜一般｜来源：hooks-agent-integration｜路径：`docs/review/hooks-agent-integration.md`

### Document、Markdown 与文本视图

- SVG 清洗重建 XML 时未转义属性值｜阻断｜来源：document-core｜路径：`docs/review/document-core.md`
- 同一规范路径的外部重载在身份变化时覆写其他 Document 身份索引｜重要｜来源：document-core｜路径：`docs/review/document-core.md`
- 可编辑 Document 缺行尾切换、编码重开与手动只读活体通道｜重要｜来源：document-core、textview-editor｜路径：`docs/review/document-core.md`、`docs/review/textview-editor.md`
- 查找与替换只实现字面量查找和全部替换，且没有生产接线｜重要｜来源：document-core、textview-editor｜路径：`docs/review/document-core.md`、`docs/review/textview-editor.md`
- 本地图片路径在授权前被解析，UNC 可自动触发 SMB 认证｜阻断｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 行内公式与行内图片被移出文本流｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 引用、列表与脚注内的块结构被压平｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 脚注引用不渲染、不可点击｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 安全 HTML 白名单只做文本降级｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 程序化滚动与恢复位置不走视口调度｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 每次滚动清空全部已渲染 outcome｜重要｜来源：markdown-math、performance-crosscut｜路径：`docs/review/markdown-math.md`、`docs/review/performance-crosscut.md`
- 高 DPI 下公式位图被放大后再按 1:1 逻辑像素显示｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 已加载的远程图片再次点击仍发起新请求｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 渲染查找在前台按匹配数乘块数扫描｜重要｜来源：markdown-math、document-core、performance-crosscut｜路径：`docs/review/markdown-math.md`、`docs/review/document-core.md`、`docs/review/performance-crosscut.md`
- 每次滚动在前台复制整篇文档文本｜重要｜来源：markdown-math、performance-crosscut｜路径：`docs/review/markdown-math.md`、`docs/review/performance-crosscut.md`
- 大纲无层级且常驻挤占内容区｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 渲染态缺少键盘滚动、大纲与链接激活｜重要｜来源：markdown-math｜路径：`docs/review/markdown-math.md`
- 安全与授权关键路径只有服务/投影测试｜重要｜来源：markdown-math、document-core｜路径：`docs/review/markdown-math.md`、`docs/review/document-core.md`
- Editor 面板规定能力未实现｜阻断｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 本地编码检测与编码重开在实际打开链中不存在｜重要｜来源：textview-editor、document-core｜路径：`docs/review/textview-editor.md`、`docs/review/document-core.md`
- 追溯矩阵的自动化证据层级与场景不一致｜重要｜来源：textview-editor、test-quality、plan-audit、app-root-content｜路径：`docs/review/textview-editor.md`、`docs/review/test-quality.md`、`docs/review/plan-audit.md`、`docs/review/app-root-content.md`
- 分页只读视图不实现换行，且与软换行 Editor 共用滚动坐标单位｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 分页页读取失败后永久停留 Loading｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 外部快照与撤销/重做把滚动位置重置到顶部｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- Git 行标丢失增改删语义，颜色与越界防护不满足合同｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- reveal_line 在软换行下把逻辑行当显示行｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 未知扩展名的文本文件在读取前被拒绝｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 分页恢复占位是无出口面板｜重要｜来源：textview-editor｜路径：`docs/review/textview-editor.md`
- 布局缓存没有容量或可见窗口回收｜重要｜来源：textview-editor、performance-crosscut｜路径：`docs/review/textview-editor.md`、`docs/review/performance-crosscut.md`

### 终端核心与 GPUI 终端

- 存活上限预检发生在真实 PTY 创建之后｜阻断｜来源：gpui-terminal-view、terminal-core、performance-crosscut、concurrency-resources｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`、`docs/review/performance-crosscut.md`、`docs/review/concurrency-resources.md`
- 空 `Msg::Input` 永久卡住写队列｜阻断｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- 重连期间旧 worker 仍可注入退出事件或显示命令｜重要｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- 帧唤醒标记与最新帧存储存在丢失竞态｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- 子进程退出时未排空已读输出｜重要｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- 读取线程 EOF 或读取错误不会转成退出/故障事件｜重要｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- OSC 原始缓冲无界，嗅探上限在分发后才生效｜重要｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- 运行时设置未作用到终端核心｜重要｜来源：terminal-core、gpui-terminal-view、completion-learning、app-root-settings-runtime-ui｜路径：`docs/review/terminal-core.md`、`docs/review/gpui-terminal-view.md`、`docs/review/completion-learning.md`、`docs/review/app-root-settings-runtime-ui.md`
- 搜索的帧驱动重查会重置导航位置并形成搜索风暴｜重要｜来源：gpui-terminal-view、terminal-core、performance-crosscut｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`、`docs/review/performance-crosscut.md`
- 退出后键盘复制、搜索与回滚全部失效｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- 一次瞬态操作错误会替换并永久遮住终端画面｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- `Ctrl+Enter` 未发送 LF，行镜像把修饰 Enter 当作提交｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- 键盘选区扩展与全选未实现，Shift+PageUp/PageDown/Home/End 语义相反｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- `Ctrl+Shift+A` 同时被终端全选与侧栏 Agent 文件占用｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- 光标形状、闪烁与相关设置不符合行为真值｜重要｜来源：gpui-terminal-view、terminal-core｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`
- 主题、字体、字号、连字与复制/补全外观设置不热应用｜重要｜来源：gpui-terminal-view、terminal-core、completion-learning｜路径：`docs/review/gpui-terminal-view.md`、`docs/review/terminal-core.md`、`docs/review/completion-learning.md`
- 真实 ConPTY 验收场景覆盖不足｜重要｜来源：terminal-core｜路径：`docs/review/terminal-core.md`
- 测试语义与生产路径断裂｜重要｜来源：terminal-core、gpui-terminal-view、test-quality｜路径：`docs/review/terminal-core.md`、`docs/review/gpui-terminal-view.md`、`docs/review/test-quality.md`
- 项目脚本与 workspace 有界预算缺少真实测试｜重要｜来源：completion-learning｜路径：`docs/review/completion-learning.md`
- 可信边界与旧代丢弃的验收测试不足｜重要｜来源：completion-learning｜路径：`docs/review/completion-learning.md`

### 持久化、崩溃恢复与加密备份

- Document 崩溃快照只有恢复器，没有生产写入与删除链路｜阻断｜来源：concurrency-resources、persistence-backup、performance-crosscut｜路径：`docs/review/concurrency-resources.md`、`docs/review/persistence-backup.md`、`docs/review/performance-crosscut.md`
- 加密备份没有工作区类别，跨机重定位与逻辑工作区导出不存在｜阻断｜来源：docs-spec、persistence-backup、security-crosscut、architecture-contracts｜路径：`docs/review/docs-spec.md`、`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`、`docs/review/architecture-contracts.md`
- 加密备份没有生产入口、预览、原子导出、跨重启恢复点与撤回｜阻断｜来源：persistence-backup、security-crosscut、docs-spec｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`、`docs/review/docs-spec.md`
- stage_restore 把解密后的备份条目明文写盘｜阻断｜来源：persistence-backup、security-crosscut｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 灾难副本从未生成与刷新｜阻断｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 备份恢复在 store_epoch 轮换与 Applied 之间崩溃会永久阻断启动｜阻断｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 双损隔离部分失败或中断后，重试以空工作区正常启动｜阻断｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 备份明文没有 64 MiB 上限｜重要｜来源：persistence-backup、security-crosscut｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 设置导出使用敏感键黑名单而不是登记字段白名单，Session 原文完全未过滤｜重要｜来源：docs-spec、persistence-backup、security-crosscut｜路径：`docs/review/docs-spec.md`、`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 恢复点与暂存目录没有当前用户 ACL 限制｜重要｜来源：persistence-backup、security-crosscut｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 暂存明文先于 owner 落盘，清理先清行后删文件｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 即时恢复在指纹回写失败时已写盘却返回失败｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 撤回只返回单一错误，逐项结果不可见，成功后恢复点不失效｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 备份恢复、应用、回滚与撤回没有审计/恢复日志，设置写绕过 settings_store｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 审计成功事件在 Warn 级日志过滤下被丢弃｜重要｜来源：persistence-backup、security-crosscut｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 根外 Document 的会话重授权没有生产调用点｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 布局或快照领域恢复失败时静默以空工作区开窗（并入：恢复失败以空工作区开窗（应用壳视角））｜重要｜来源：persistence-backup、app-root-content｜路径：`docs/review/persistence-backup.md`、`docs/review/app-root-content.md`
- 未完成关闭意图、NeedsAttention 维护请求与遗留恢复点只有阻断｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 更高版本数据库先以读写连接打开｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 设置 IO/权限失败回退默认值，设置文件没有高版本判定｜重要｜来源：persistence-backup、settings-agents｜路径：`docs/review/persistence-backup.md`、`docs/review/settings-agents.md`
- 追溯矩阵把函数级、未接线或不同场景的测试当成已自动化验收锚｜重要｜来源：persistence-backup、docs-spec、plan-audit、test-quality｜路径：`docs/review/persistence-backup.md`、`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/test-quality.md`
- 崩溃与边界测试缺少关键窗口｜重要｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 快照存储运行期遇任意损坏文件即阻断全部新写入｜一般｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 快照文件名与头身份不一致时被当作损坏｜一般｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- snapshot_index 与历史隔离表没有生产写入｜一般｜来源：persistence-backup、settings-agents｜路径：`docs/review/persistence-backup.md`、`docs/review/settings-agents.md`
- 主日志写入系统临时目录而不是本地数据根｜一般｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`
- 备份归档 DTO 接受未知字段，空类别集也能解封成功｜一般｜来源：persistence-backup｜路径：`docs/review/persistence-backup.md`

### Windows 平台、更新、套餐、通知与 Runtime

- 更新链在运行态没有入口｜阻断｜来源：platform-windows、plan-audit、security-crosscut、concurrency-resources｜路径：`docs/review/platform-windows.md`、`docs/review/plan-audit.md`、`docs/review/security-crosscut.md`、`docs/review/concurrency-resources.md`
- 套餐余量功能在运行态不存在｜阻断｜来源：platform-windows、plan-audit、security-crosscut、concurrency-resources｜路径：`docs/review/platform-windows.md`、`docs/review/plan-audit.md`、`docs/review/security-crosscut.md`、`docs/review/concurrency-resources.md`
- 诊断与清理没有产品面｜阻断｜来源：platform-windows、plan-audit、security-crosscut｜路径：`docs/review/platform-windows.md`、`docs/review/plan-audit.md`、`docs/review/security-crosscut.md`
- 通知事件源与托盘快照没有接入｜阻断｜来源：platform-windows、architecture-contracts、security-crosscut｜路径：`docs/review/platform-windows.md`、`docs/review/architecture-contracts.md`、`docs/review/security-crosscut.md`
- 系统关机与注销不走统一关闭链｜阻断｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 通知前台状态从未写入｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 应用内错误不可见，错误类别不进入任务栏错误态｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 任务栏聚合没有资源身份｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 更新预约在崩溃或知情放弃关闭时仍保留｜重要｜来源：platform-windows、concurrency-resources｜路径：`docs/review/platform-windows.md`、`docs/review/concurrency-resources.md`
- 启动安装依赖已退役的 session.json，恢复票没有消费方（并入：旧会话 schema 与更新 handoff 的第二套布局序列化）｜重要｜来源：platform-windows、workspace-shell、architecture-contracts｜路径：`docs/review/platform-windows.md`、`docs/review/workspace-shell.md`、`docs/review/architecture-contracts.md`
- 托盘可用性判定早于建窗，失败后没有恢复节拍｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 合同冲突：静默启动的触发范围｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- 诊断脱敏不覆盖真实 Runtime token 形态｜重要｜来源：platform-windows、security-crosscut｜路径：`docs/review/platform-windows.md`、`docs/review/security-crosscut.md`
- 追溯矩阵把服务层或注册表层用例标成运行态集成｜重要｜来源：platform-windows、plan-audit、test-quality｜路径：`docs/review/platform-windows.md`、`docs/review/plan-audit.md`、`docs/review/test-quality.md`
- 套餐周期契约与后台任务注册表不一致｜重要｜来源：platform-windows｜路径：`docs/review/platform-windows.md`
- Runtime `git.diff` 路径校验放过盘符绝对路径/符号链接，工作区读取越过仓库根｜阻断｜来源：runtime-v2、security-crosscut｜路径：`docs/review/runtime-v2.md`、`docs/review/security-crosscut.md`
- 快照 revision 固定为握手起点，事件续传进入不可恢复 resync 循环｜阻断｜来源：runtime-v2、concurrency-resources｜路径：`docs/review/runtime-v2.md`、`docs/review/concurrency-resources.md`
- 非交互关闭在保存、停止或清理落定前返回 Succeeded｜阻断｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 事件环移除滞后订阅后，订阅连接保持静默｜重要｜来源：runtime-v2、concurrency-resources｜路径：`docs/review/runtime-v2.md`、`docs/review/concurrency-resources.md`
- expected_revision 在线上端到端未生效｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- Operation 停在 Queued/Running 后没有恢复路径｜重要｜来源：runtime-v2、concurrency-resources｜路径：`docs/review/runtime-v2.md`、`docs/review/concurrency-resources.md`
- 项目、布局、面板与终端写 Operation 在持久快照提交前报告成功｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- Hooks 草稿在决定齐备前被丢弃｜重要｜来源：runtime-v2、hooks-agent-integration｜路径：`docs/review/runtime-v2.md`、`docs/review/hooks-agent-integration.md`
- 合同冲突：app.close 回执语义与“保存失败返回失败”冲突｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 合同冲突：非交互关闭缺少运行终端处置策略｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 控制面一次 join 失败被永久重放｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 随包 schema 未覆盖多数方法的请求结构，method 与 params/result 无关联校验｜重要｜来源：runtime-v2、scripts-ci-release、test-quality｜路径：`docs/review/runtime-v2.md`、`docs/review/scripts-ci-release.md`、`docs/review/test-quality.md`
- Runtime 合同测试不在任何固定门禁内｜重要｜来源：runtime-v2、scripts-ci-release、test-quality｜路径：`docs/review/runtime-v2.md`、`docs/review/scripts-ci-release.md`、`docs/review/test-quality.md`
- Runtime 双进程 conformance 与事件断线矩阵缺失｜重要｜来源：runtime-v2、test-quality｜路径：`docs/review/runtime-v2.md`、`docs/review/test-quality.md`
- 事件面只发布项目激活｜重要｜来源：runtime-v2、app-root-settings-runtime-ui｜路径：`docs/review/runtime-v2.md`、`docs/review/app-root-settings-runtime-ui.md`
- 路由与能力摘要不按方法注册表作用域执行｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- `GitStatusResponse.root` 返回项目根而不是仓库根｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 快照不暴露布局树与分割几何，resize 路径不可推导且视口由客户端自报｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 等待操作不能因客户端断开而取消｜重要｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- `runtime_token_absent_from_materials` 锚只覆盖诊断材料｜重要｜来源：runtime-v2、platform-windows、security-crosscut、test-quality｜路径：`docs/review/runtime-v2.md`、`docs/review/platform-windows.md`、`docs/review/security-crosscut.md`、`docs/review/test-quality.md`
- 注册表 kind/idempotent 语义与真实副作用不符（并入：注册表 kind/idempotent 与写请求参数散列等 Runtime 契约细节）｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 握手后的解析失败直接断开连接｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- 分页与响应预算按原始字节估算｜一般｜来源：runtime-v2、performance-crosscut｜路径：`docs/review/runtime-v2.md`、`docs/review/performance-crosscut.md`
- 分页游标不绑定资源身份，也接受非 UTF-8 字符边界偏移｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- Handover 客户端不核对 hello 协议版本与合同散列｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- git.open_diff/git.open_deleted 超时按失败处理但打开可能仍完成｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`
- agent.start/resume/fork 在提示符注入前报告成功｜一般｜来源：runtime-v2、settings-agents｜路径：`docs/review/runtime-v2.md`、`docs/review/settings-agents.md`
- 封闭文本字段在 DTO 与 schema 中仍是自由字符串｜一般｜来源：runtime-v2｜路径：`docs/review/runtime-v2.md`

### 脚本、发布、许可与依赖

- 内嵌 Maple Mono 字体缺少 OFL 许可与版权文本｜阻断｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 发布说明哈希与当前 dist 及打包证据不一致（并入：CHANGELOG 版本状态与发布说明、打包证据不一致）｜阻断｜来源：scripts-ci-release、docs-spec｜路径：`docs/review/scripts-ci-release.md`、`docs/review/docs-spec.md`
- 许可审计根包名与实际 workspace 包名不一致｜重要｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 手工 THIRD-PARTY-NOTICES 缺版权行且引用不存在路径｜重要｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- dav1d 原生源从可变 tag 克隆，审计 commit 未进入构建｜重要｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 构建新鲜度摘要漏掉非 .rs/.toml 构建输入｜重要｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 随包 ConPTY 运行时的校验与实际嵌入二进制脱节｜重要｜来源：scripts-ci-release、terminal-core、security-crosscut｜路径：`docs/review/scripts-ci-release.md`、`docs/review/terminal-core.md`、`docs/review/security-crosscut.md`
- release.ps1 的钉测试只执行体积与 profile（并入：发布钉测试接线断点）｜重要｜来源：scripts-ci-release、test-quality｜路径：`docs/review/scripts-ci-release.md`、`docs/review/test-quality.md`
- “清单即合同”没有机械化全等检查｜重要｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- CI 不执行许可清单核验｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- CI 未按发布口径验证（无 --locked、无 release 构建、无 dumpbin）｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 架构门禁未使用 --base｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 版本校验与默认值跨脚本不一致｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 发布产物无签名链｜一般｜来源：scripts-ci-release、security-crosscut｜路径：`docs/review/scripts-ci-release.md`、`docs/review/security-crosscut.md`
- i18n 合同只约束目录占位符｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`
- 数学语料引用不存在的测试、主题与截图｜一般｜来源：scripts-ci-release｜路径：`docs/review/scripts-ci-release.md`

### 安全、凭据与依赖

- Git 读取未统一遵守有界执行、禁网与凭据提示约束｜重要｜来源：security-crosscut、runtime-v2｜路径：`docs/review/security-crosscut.md`、`docs/review/runtime-v2.md`
- 生产 TLS 栈命中 emSign 弃用中间证书公告｜重要｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`
- 加密备份 AAD 范围与恢复时序合同冲突｜重要｜来源：persistence-backup、security-crosscut｜路径：`docs/review/persistence-backup.md`、`docs/review/security-crosscut.md`
- 依赖审计未进入 CI 常驻门禁｜提示｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`
- 诊断脱敏词表与备份敏感键表不一致｜提示｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`
- 凭据明文副本没有全程清零｜提示｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`
- AI hook 解析在 debug 日志输出整条事件｜提示｜来源：security-crosscut、hooks-agent-integration｜路径：`docs/review/security-crosscut.md`、`docs/review/hooks-agent-integration.md`
- 更新下载 .part 打开跟随预置链接｜提示｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`
- 凭据 Win32 FFI 调用缺少逐块 SAFETY 不变量｜提示｜来源：security-crosscut｜路径：`docs/review/security-crosscut.md`

### 测试质量、文档与计划证据

- 追溯矩阵引用不存在的测试例名｜重要｜来源：plan-audit、test-quality、workspace-shell、app-root-content｜路径：`docs/review/plan-audit.md`、`docs/review/test-quality.md`、`docs/review/workspace-shell.md`、`docs/review/app-root-content.md`
- 部分矩阵例名存在，但覆盖的不是声明场景｜重要｜来源：plan-audit、settings-agents、app-root-settings-runtime-ui、textview-editor｜路径：`docs/review/plan-audit.md`、`docs/review/settings-agents.md`、`docs/review/app-root-settings-runtime-ui.md`、`docs/review/textview-editor.md`
- 全量验收锚以源码字符串和局部状态代替跨域行为｜重要｜来源：docs-spec、plan-audit、architecture-contracts、test-quality｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/architecture-contracts.md`、`docs/review/test-quality.md`
- 测试豁免表含失效设计路径、目标态待实现项与超范围无障碍项｜重要｜来源：docs-spec、plan-audit、terminal-core、markdown-math、settings-agents｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/terminal-core.md`、`docs/review/markdown-math.md`、`docs/review/settings-agents.md`
- 架构总览与常驻规则仍使用旧栈数据名、类型名与已取消的校验｜重要｜来源：docs-spec、architecture-contracts｜路径：`docs/review/docs-spec.md`、`docs/review/architecture-contracts.md`
- README、CHANGELOG 与发布说明的用户能力清单停留在旧栈｜重要｜来源：docs-spec、plan-audit、platform-windows｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/platform-windows.md`
- 被排除的图片面板实现仍编译存在｜重要｜来源：docs-spec、plan-audit｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`
- featureSpec 未覆盖已实现的壁纸与屏幕检测｜重要｜来源：docs-spec、plan-audit｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`
- Mermaid 终态在 featureSpec、devPlan 与进度之间构成合同冲突｜重要｜来源：docs-spec、plan-audit、markdown-math｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/markdown-math.md`
- devPlan core crate 依赖方向与实际策略相反｜一般｜来源：docs-spec｜路径：`docs/review/docs-spec.md`
- devPlan Runtime 方法表写错 operation.get/wait｜一般｜来源：docs-spec、runtime-v2｜路径：`docs/review/docs-spec.md`、`docs/review/runtime-v2.md`
- 进度与追溯文档保留测试通过计数、过期状态锚与计划期自述｜一般｜来源：docs-spec、plan-audit、test-quality｜路径：`docs/review/docs-spec.md`、`docs/review/plan-audit.md`、`docs/review/test-quality.md`
- 模块与设计文档存在第二份源码式复述｜一般｜来源：docs-spec｜路径：`docs/review/docs-spec.md`
- Runtime v2 验收 hash 指向 Windows 阶段提交｜重要｜来源：plan-audit｜路径：`docs/review/plan-audit.md`
- “全部阶段验收完成”与安装器、发布包实机待办矛盾｜重要｜来源：plan-audit、docs-spec｜路径：`docs/review/plan-audit.md`、`docs/review/docs-spec.md`
- 旧实现与最终排除面未清理｜重要｜来源：plan-audit、docs-spec、app-root-content｜路径：`docs/review/plan-audit.md`、`docs/review/docs-spec.md`、`docs/review/app-root-content.md`
- 进度表用同一提交充当多个阶段锚｜一般｜来源：plan-audit｜路径：`docs/review/plan-audit.md`
- 追溯矩阵存在同名测试与重复行｜一般｜来源：plan-audit、document-core｜路径：`docs/review/plan-audit.md`、`docs/review/document-core.md`
- 计划行为切片没有可核销例名｜一般｜来源：plan-audit｜路径：`docs/review/plan-audit.md`

### 跨域补充

- 显式终端 cwd 只做词法包含校验，未解析真实目标｜重要｜来源：workspace-shell｜路径：`docs/review/workspace-shell.md`
- 工作区布局恢复没有未知 ViewKey 过滤与重复视图归一｜重要｜来源：workspace-shell、architecture-contracts｜路径：`docs/review/workspace-shell.md`、`docs/review/architecture-contracts.md`
- AppRoot 后台 Agent 根加载不可注入，虚拟窗口用例读取真实 home｜重要｜来源：test-quality｜路径：`docs/review/test-quality.md`

## 复核驳回与证据修正

- **“以下目标例名在源码中不存在”这一组证据部分被驳回**：`document_discard_respects_external_reference`、`ordinary_open_does_not_deduplicate_diff_or_gitshow`、`document_rename_preserves_identity_and_view`、`explorer_rejects_cross_root_cut_and_descendant_paste`、`diff_conflict_snapshot_is_immutable`、`diff_large_and_binary_side_degradation` 均以函数定义存在；只有 `commit_rollback_preserves_staged_changes` 确实不存在。子证据驳回，但“用例只覆盖 core/服务层、未经过生产面板或产品入口”的证据层级不足结论仍成立。
- **`diff_conflict_snapshot_is_immutable` 与 `diff_large_and_binary_side_degradation` 的存在不等于产品能力成立**：两者验证 core 的不可变快照与大文件/二进制按侧降级；应用 `build_source_content` 仍要求可编辑 Rope，`DiffInput::conflict` 无生产调用。产品级冲突快照/大文件降级缺失仍确认，但以“例名不存在”为主要证据的表述被驳回。
- **GitShow 非法 UTF-8 是合同冲突而非单一实现错误**：两份文档同时有效且互相冲突，复核不判定哪一方为真值，保留所有者裁决。
- **`show_ignored`“无消费者”需限定范围**：Commit 视图存在该状态与分组过滤；问题仅指 Explorer 树设置/过滤路径，不能推广到全部文件树视图。
- **`Settings` 的 `accept` 设置并非完全不存在**：`slterm_settings/src/keys.rs` 有该键校验；运行 `Settings` 结构与终端视图不读取它。“无消费端”成立，“没有该设置项”不成立。
- **`settings_transfer` 的普通导出有白名单实现**：“内存纯函数、无文件流程与生产入口”成立；“没有白名单”的推广被驳回。
- **灾难副本“从未生成”需限定**：生产恢复分支在已有有效副本时会读取并生成临时恢复库，测试也会写副本；问题指正常运行不生成/刷新 `workspace.backup.db`。“全仓无 `backup_to_new` 生产调用”若绝对化则不准确。
- **证据范围需限定**：`app-root-content` 与 `test-quality` 的“例名不存在”组证据需按上述修正；`security-crosscut` 与 `runtime-v2` 对 `git.diff` 的描述分别为符号链接与盘符绝对路径，已合并为同一路径授权缺口；`performance-crosscut` 的崩溃快照条目是跨域重复，已并入持久化域。
- **台账覆盖缺口（不进入主索引）**：分报告中另有若干“重要”条目未被台账单列，因而未获得最终严重度，也未进入本索引，包括但不限于：`concurrency-resources` 的 hook 事件通道无界与全局关闭工作线程 writer 失败退出；`completion-learning` 的设置页缺少命令/目录采集开关、目录学习未受可信门控、补全按键绕过命令目录、补全显示设置不热应用、Popup/Hybrid 语义偏离、清空竞争、缺失 Git 缓存测试前置不成立；`performance-crosscut` 的 Diff 共享编辑重建、Diff 大文件降级、Markdown 每键全量解析、终端搜索匹配无上限、Runtime 分页每页重建、Explorer 目录无分批；`app-root-content` 的 Diff 展示参数、GitShow 重试、Explorer 双击/多选、Explorer/AgentFiles 文件操作、Commit 破坏操作确认；`settings-agents` 的历史隔离区执行面；`plan-audit` 的隔离到期清理/导出/永久删除；`architecture-contracts` 的 core 纯边界与第二写盘通道。上述条目需补核后再定严重度，不能按已驳回处理。
- **报告合规修正**：多份报告把“代码路径确认”与“运行时后果确认”混在同一置信度标签下；相关条目应按“确认（代码）/疑似（运行时）”理解。`persistence-backup` 与 `hooks-agent-integration` 把“合同冲突”写在严重度标题位，复核已改按严重度字段记录并在本汇总单列冲突。`architecture-contracts`、`docs-spec`、`plan-audit` 的跨所有者聚合条目已在台账中拆分为设置中心、导航、更新等独立条目。

## 存疑未证

- 真实 UNC/SMB 认证：解析先于授权与普通 UNC 可被 `CanonicalPath` 接受已由源码确认；打开文档即完成 NTLM 认证需要真实共享、抓包或认证日志。
- SVG 注入后的实际利用：属性值注入与白名单绕过已确认；注入的滤镜/超大 `stdDeviation` 是否形成像素/CPU 预算绕过，需要真实光栅化测量；事件属性是否被 `usvg` 忽略也需运行时验证。
- 空 `Msg::Input` 的写入返回值：状态机无法在空项上前进已确认；Windows 管道对空写的返回以及事件循环是永久忙转还是故障退出，需要真实 PTY 运行。
- IME 组合被打断：GPUI 先派发绑定再进入监听层的顺序已从钉定源码确认；真实 Windows IME 组合期间按下已绑定组合键是否破坏组合、丢失文本，需要实机录屏或事件日志。
- `store_epoch` 轮换与 `Applied` 之间崩溃：代码顺序与维护器世代闸门已确认；真实子进程在提交边界崩溃后重开是否必然永久阻断，需要故障注入。
- 双损隔离中途失败后的重试：逐项 rename、失败不回滚、下次对缺失主库初始化空库的代码路径已确认；真实“主库已移走但副本 rename 失败”的复现需要故障注入。
- TLS emSign 利用：版本、ureq 默认特性与更新链哈希信任面已确认；用已公开私钥构造伪造链是否被该组合接受，需要受控 DNS/代理与中间人实验。`cargo audit` 只证明公告版本命中，不证明可利用。
- 真实安装器/发布包/更新移交：Inno Setup 编译、AppId 展开、卸载失败注入、签名链、真实安装/回滚与 `apply_scheduled` 端到端均未执行；相关条目只能到脚本与文件态证据。
- 真实 Windows 凭据管理器往返：凭据管理器用例在未设置测试环境变量时直接返回；真实写入/覆盖/删除/失败分类没有自动化证据。
- 真实 ConPTY/Win10 行为：DA1/DSR/CPR、备用屏、UTF-8 跨块、退出尾帧、重连时序、捆绑 OpenConsole 的 Win10 路径均需要实机或真实 PTY 用例。
- 真实 Git 仓库中的 `git.diff` 盘符路径读取：路径校验接受绝对路径、`join` 替换路径、`File::open` 读取已由源码确认；目标位于 Git 仓库内且 `head_file` 成功解析的实际返回，以及符号链接/junction 跟随，需要真实 Windows 仓库复现。
- 字体名称表中的 OFL 文本：仓库无 OFL 文本、无字体许可文件、打包清单无该字体许可是已确认事实；字体自身是否声明 OFL 1.1 需要字体解析工具或上游分发页核对。
- 真实慢客户端/事件断线：事件环移除滞后订阅、连接线程不观察事件线程终止的代码路径已确认；真实 TCP 半关闭、客户端不读、写失败与连接关闭时序需要网络实验。
- 真实 writer 故障下的 Operation 停滞：`dispatch` 的等待、超时返回、`Existing` 无条件返回原记录、维护期无 Runtime Operation 对账已确认；超时/队列满/写线程停止在真实负载下的可达性需要故障注入。
- 真实显示器拔插/DPI 变化：运行期无窗口夹回监听已确认；Windows 是否自动重定位、标题栏是否不可达需要实机。
- 真实 Agent CLI 入口名：`AgentCatalog` 的 `builtin_command` 与 `kind.slug()` 的差异已确认；Cursor/Antigravity 在真实安装中能否由 slug 启动需要实机 CLI 验证。
- AppRoot 虚拟窗口用例是否实际完成后台 Agent 目录枚举：代码路径会读取真实 home，但是否在 `run_until_parked` 前完成、是否实际触发目录读取依赖调度，需要运行时插桩。
- Git 测试是否实际受全局签名/hooks/autocrlf 影响：夹具未隔离全局配置，但当前开发机/CI 是否触发失败未实测。
- 加密备份的真实用户流程：口令输入、预览、关闭预检、恢复点列表、撤回冲突没有 UI 实现，无法做运行态验收。
- 设置中心/导航/AgentRoot/套餐/Runtime 的最终产品形态是否被有意裁剪缺少所有者裁决记录；若确认缩减，应按文档规范登记延期并回退追溯状态。
- `cargo audit` 的安全公告是否全部可达生产路径未做调用图/运行时验证；完整依赖闭包、宏展开与内联后的调用面仍需机械核对。
- `clippy` 重复 target 清单警告是否影响 `--all-targets` 语义未评估；当前只确认无其他 clippy 诊断。
- `progress.md` 测试通过数与门禁日志聚合不一致可能是不同 target/不同时间的口径差异，也可能是文档过期；需以同一命令、同一基线复核。

## 审查局限

- 未做 clean build；门禁构建基于当前工作树与既有构建缓存。
- 未运行真实 GUI、真实 ConPTY/IME、真实安装器、真实 Win10、发布机或更新移交；涉及桌面会话、安装包、凭据库、外部网络与 SMB 的结论只到代码路径或脚本/文件态证据。
- 未做覆盖率测量、Miri、ASan、fuzz、基准测试或真实性能剖析；性能结论为静态路径与预算审查。
- 未审第三方源码与二进制内部，包括 GPUI fork、vendor 补丁、ConPTY 二进制、字体名称表、dav1d 上游与 RustSec 公告的完整传播路径。
- 未使用真实用户数据目录、真实凭据或真实会话正文；所有敏感值均为测试假值形态。
- 未审历史 diff，只审当前工作树；门禁日志为外部输入，本汇总未复跑 cargo、测试、脚本或网络。
