# 编辑器栈评审

## 覆盖矩阵

### docs/featureSpec/panels/editor.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 面板形态与运行模式 | agentroot-runmode |
| 文件打开路由 | 无问题 |
| 拖文件打开 | 无问题 |
| 文档身份与路径变化 | editor-detached-autosave |
| 编辑、语言高亮与脏状态 | editor-language-map-drift |
| 代码折叠 | 无问题 |
| 语言选择器 | editor-language-map-drift |
| 自动保存 | editor-detached-autosave |
| 查找与替换 | 无问题 |
| 保存、脏状态与关闭守卫 | editor-detached-autosave |
| 手动只读 | agentroot-runmode |
| 外部修改 | save-concurrency-baseline |
| 大文件分级 | outside-root-large |
| Git 行标与 gutter Diff | 无问题 |
| 字号与自动换行 | 无问题 |
| 主题与配色 | 无问题 |
| 页签标题 | 无问题 |
| 焦点与键盘 | 无问题 |
| 状态保留 | 无问题 |
| 轻量状态条 | 无问题 |
| 崩溃恢复备份 | 无问题 |
| 与 Diff、GitShow 的边界 | 无问题 |
| 文件系统与平台约束 | 无问题 |
| 入口、出口与状态迁移 | 无问题 |
| 鼠标与键盘 | 无问题 |
| 反馈与对话框 | 无问题 |
| 空、加载与读取 | 无问题 |
| 外部修改、保存与并发 | save-concurrency-baseline |
| 大文件与性能 | outside-root-large |
| 只读与手动只读 | agentroot-runmode |
| 平台与文件系统 | 无问题 |
| 打开路由与单一视图 | 无问题 |
| 根外只读与 HTML | 无问题 |
| 保存、关闭与自动保存 | editor-detached-autosave |
| 编码、大文件与只读 | outside-root-large |
| 外部修改与 gutter Diff | save-concurrency-baseline |
| 代码折叠与语言选择器 | editor-language-map-drift |
| 拖文件、崩溃恢复与状态条 | 无问题 |
| 关键决策与依据 | 无问题 |

### docs/featureSpec/panels/markdown.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 面板归属与两态 | markdown-large-doc-form |
| 渲染能力 | editor-language-map-drift |
| 链接分派 | markdown-agentroot-boundary |
| 本地与远程资源 | markdown-agentroot-boundary; markdown-remote-referer |
| 缩放、滚动与阅读位置 | 无问题 |
| 标题大纲与查找 | 无问题 |
| 安全边界 | markdown-agentroot-boundary; markdown-remote-referer |
| 入口、去重与焦点 | 无问题 |
| 形态切换 | 无问题 |
| 渲染内容交互 | 无问题 |
| 键盘与焦点 | 无问题 |
| 状态保留 | 无问题 |
| 保存与关闭 | 无问题 |
| 反馈 | 无问题 |
| 空态与加载 | 无问题 |
| 渲染降级 | 无问题 |
| 外部变更与冲突 | 无问题 |
| 大文档分级 | markdown-large-doc-form |
| 错误与提示 | 无问题 |
| 草稿恢复 | 无问题 |
| 键盘与输入 | 无问题 |
| 平台与运行环境 | 无问题 |
| 形态、默认渲染与关闭 | markdown-large-doc-form |
| 渲染、语法与安全 | editor-language-map-drift |
| 链接与资源 | markdown-agentroot-boundary; markdown-remote-referer |
| 查找、大纲与状态 | 无问题 |
| 外部变更、恢复与大文档 | markdown-large-doc-form |
| 关键决策/两态互斥与默认渲染 | 无问题 |
| 关键决策/内存草稿作为唯一渲染源 | 无问题 |
| 关键决策/源码态纳入脏状态与关闭守卫 | 无问题 |
| 关键决策/HTML 不渲染 | 无问题 |
| 关键决策/安全 HTML 子集 | 无问题 |
| 关键决策/链接白名单与根外只读 | markdown-agentroot-boundary |
| 关键决策/本地资源限项目根 | markdown-agentroot-boundary |
| 关键决策/远程图片手动单次加载 | markdown-remote-referer |
| 关键决策/Mermaid 源码显示与渲染接口 | 无问题 |
| 关键决策/大文档分级 | markdown-large-doc-form |
| 关键决策/大纲可选、查找限渲染文本 | 无问题 |
| 关键决策/冲突差异摘要 | 无问题 |
| 关键决策/草稿周期快照恢复 | 无问题 |
| 关键决策/外部重命名跟随 | 无问题 |
| 关键决策/键盘与输入边界 | 无问题 |
| 关键决策/内容操作边界 | 无问题 |
| 关键决策/阅读位置与缩放持久化 | 无问题 |
| 关键决策/渲染调度 | 无问题 |

### docs/featureSpec/panels/diff.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 定位与入口 | diff-dedupe-source-term |
| 冲突快照对比 | diff-conflict-snapshot-lifecycle |
| 视图模式 | diff-inline-rootout-edit |
| 左右交换与单侧折叠 | 无问题 |
| 面板结构与列宽 | 无问题 |
| 行标记与占位 | 无问题 |
| 差异计算与实时重算 | 无问题 |
| 滚动同步 | 无问题 |
| 编辑、保存与冲突 | save-concurrency-baseline; diff-large-dirty-gutter |
| HEAD 刷新 | diff-head-polling |
| 工作区外部修改 | diff-head-polling; diff-delete-recreate-readonly |
| 外部重命名跟随 | 无问题 |
| 大文件与二进制 | diff-large-dirty-gutter |
| 焦点与键盘导航 | 无问题 |
| 面板参数与状态持久化 | 无问题 |
| 打开与关闭 | diff-dedupe-source-term |
| 工具栏与菜单 | 无问题 |
| 鼠标与拖拽 | 无问题 |
| 键盘 | 无问题 |
| 焦点迁移 | 无问题 |
| 反馈与状态迁移 | 无问题 |
| 空态与无差异 | 无问题 |
| 读取与差异错误 | 无问题 |
| 保存与冲突 | save-concurrency-baseline |
| 文件与路径 | diff-delete-recreate-readonly |
| 并发 | save-concurrency-baseline |
| 大文件与性能 | 无问题 |
| 生命周期与恢复 | 无问题 |
| 键盘与焦点 | 无问题 |
| 平台 | 无问题 |
| 视图与结构 | diff-inline-rootout-edit |
| 冲突快照对比（验收） | diff-conflict-snapshot-lifecycle |
| 键盘导航 | 无问题 |
| 交换与折叠 | 无问题 |
| 参数持久化 | 无问题 |
| 外部重命名 | 无问题 |
| 差异计算与刷新 | diff-head-polling |
| 滚动同步（验收） | 无问题 |
| 保存与冲突（验收） | save-concurrency-baseline |
| 外部修改与错误分类 | diff-delete-recreate-readonly |
| 大文件与二进制（验收） | diff-large-dirty-gutter |
| 焦点与生命周期 | 无问题 |
| 键盘与焦点（验收） | 无问题 |
| 关键决策与依据 | 无问题 |

### docs/featureSpec/panels/gitshow.md
| 章节 | 结论 |
|---|---|
| 目标行为 | 无问题 |
| 面板定位与只读边界 | 无问题 |
| 唯一入口与去重 | 无问题 |
| 标题 | gitshow-title-rootout |
| HEAD 内容读取 | 无问题 |
| 内容形态 | 无问题 |
| 只读查看能力 | 无问题 |
| 大文件分级 | 无问题 |
| 刷新与过期 | 无问题 |
| 焦点与激活 | 无问题 |
| 状态保留与持久化 | 无问题 |
| 打开、去重与关闭 | 无问题 |
| 鼠标 | 无问题 |
| 键盘 | 无问题 |
| 查找 | 无问题 |
| 刷新（交互） | 无问题 |
| 反馈 | 无问题 |
| 键盘与焦点 | 无问题 |
| 空内容 | 无问题 |
| 加载 | 无问题 |
| 错误 | 无问题 |
| HEAD 与工作区 | 无问题 |
| 大文件边界 | 无问题 |
| 并发与生命周期 | 无问题 |
| 恢复 | 无问题 |
| 平台与安全 | 无问题 |
| 性能 | 无问题 |
| 入口与标题 | gitshow-title-rootout |
| HEAD 只读 | 无问题 |
| 查找与查看 | 无问题 |
| 大文件与二进制 | 无问题 |
| 刷新与过期（验收） | 无问题 |
| 错误与恢复 | 无问题 |
| 焦点与键盘 | 无问题 |
| 关键决策与依据 | gitshow-title-rootout |

### docs/devPlan/design/panels/editor.md
| 章节 | 结论 |
|---|---|
| 打开路由与面板身份 | agentroot-runmode |
| 文档缓冲与编码/行尾 | editor-bom-model; save-concurrency-baseline |
| 编辑视图、语法高亮与折叠 | editor-language-map-drift |
| 查找/替换与跳转 | 无问题 |
| 自动保存、崩溃恢复与外部修改 | editor-detached-autosave; crash-snapshot-identity; editor-readonly-external-refresh; save-concurrency-baseline |
| Git 行标与 gutter Diff | 无问题 |
| 拖文件、HTML 按钮、焦点与状态条 | 无问题 |
| 状态保留 | 无问题 |
| 模块树与复用/重建 | 无问题 |
| 测试接缝 | manual-exemptions-unregistered; crash-snapshot-identity |

### docs/devPlan/design/panels/markdown.md
| 章节 | 结论 |
|---|---|
| 面板模型与两态 | markdown-agentroot-boundary; markdown-large-doc-form |
| 渲染管线 | markdown-plaintext-memory |
| 安全 HTML、链接与资源 | markdown-agentroot-boundary; markdown-remote-referer |
| 公式与 Mermaid | 无问题 |
| 查找、大纲、缩放与滚动锚点 | 无问题 |
| 外部修改、重命名与冲突快照 | 无问题 |
| 大文档分级与错误 | markdown-large-doc-form |
| 草稿快照 | crash-snapshot-identity |
| 模块树与复用/重建 | 无问题 |
| 测试接缝 | manual-exemptions-unregistered; crash-snapshot-identity |

### docs/devPlan/design/panels/diff.md
| 章节 | 结论 |
|---|---|
| 面板模型与入口 | diff-dedupe-source-term |
| 差异模型与行对齐 | 无问题 |
| 布局、虚拟化与滚动同步 | diff-inline-rootout-edit |
| 编辑、保存与冲突 | save-concurrency-baseline; diff-large-dirty-gutter |
| HEAD 刷新与外部修改 | diff-head-polling; diff-delete-recreate-readonly |
| 重命名跟随与移出仓库 | 无问题 |
| 冲突快照对比 | diff-conflict-snapshot-lifecycle |
| 大文件与二进制 | diff-large-dirty-gutter |
| 焦点、键盘与参数持久化 | 无问题 |
| 错误映射 | 无问题 |
| 模块树与复用/重建 | 无问题 |
| 测试接缝 | manual-exemptions-unregistered |

### docs/devPlan/design/panels/gitshow.md
| 章节 | 结论 |
|---|---|
| 面板模型、入口与去重 | gitshow-title-rootout |
| HEAD 读取与内容形态 | 无问题 |
| 大文件分级 | 无问题 |
| 刷新与过期 | 无问题 |
| 查找、换行、字号、复制与系统打开 | 无问题 |
| 焦点、状态保留与错误 | 无问题 |
| 模块树与复用/重建 | 无问题 |
| 测试接缝 | manual-exemptions-unregistered |

## 问题条目

### save-concurrency-baseline [P0] 保存完成后的并发写入语义在规格与设计间相反

- owner：`docs/devPlan/design/panels/diff.md`、`docs/devPlan/design/panels/editor.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/diff.md` §编辑、保存与冲突、§并发、验收 §保存与冲突；`docs/devPlan/design/panels/diff.md` §编辑、保存与冲突；`docs/featureSpec/panels/editor.md` §外部修改；`docs/devPlan/design/panels/editor.md` §文档缓冲与编码/行尾
- 原文：“外部程序在保存过程中同时写入时，以本面板本次写入完成后的磁盘状态为基线……不产生重复冲突提示。”／“保存后检查磁盘版本与本次写入预期一致……任一次不一致即进入保存冲突”。
- 现象：规格与验收要求保存完成后把实际磁盘状态作为新基线、不重复提示；两份设计都要求写入后版本与预期不一致即进入保存冲突，并明确“不以写入后的磁盘状态静默建立新基线”。
- 影响：同一并发场景下，按设计实现会在成功写入后弹出冲突并让用户选择覆盖/重载，直接违反 Diff 验收；按规格实现则设计的写入后校验与测试接缝失效。用户可见行为与数据安全语义分裂。
- 证据：`docs/featureSpec/panels/diff.md` §并发及验收“保存过程中外部程序同时写入”条；`docs/devPlan/design/panels/diff.md` §编辑、保存与冲突；`docs/devPlan/design/panels/editor.md` §文档缓冲与编码/行尾保存链。
- 建议修法：二选一并统一三处。推荐保留规格口径：写入前/替换前校验，原子替换后若磁盘已再变化，按新的外部修改处理并把观察到的最新磁盘状态作为基线；若坚持设计口径，则修改规格与验收，定义“覆盖磁盘”如何避免覆盖并发写入者，并补编辑器对应验收。
- 裁决对象：设计修订（或规格修订）

### crash-snapshot-identity [P1] 崩溃快照键在设计中偏离已定契约，配额未落到设计

- owner：`docs/devPlan/design/panels/editor.md`、`docs/devPlan/design/panels/markdown.md`
- 类型：冲突 / 缺口
- 定位：`docs/devPlan/design/panels/editor.md` §自动保存、崩溃恢复与外部修改；`docs/devPlan/design/panels/markdown.md` §草稿快照；`docs/featureSpec/panels/editor.md` §崩溃恢复备份；`docs/featureSpec/panels/markdown.md` §草稿恢复；`docs/devPlan/design/persistence.md` §崩溃快照与隔离；`docs/devPlan/spec-issues.md` §panels/editor.md
- 原文：“快照键为 `(ProjectId, PanelId)`”／`fn snapshot(&self, key: &DocumentKey, doc: &EditorDocument)`。
- 现象：规格、已关闭裁决与持久化设计一致要求快照按项目和面板 ID 定位，文件路径为 `recovery/<project-id>/<panel-id>.json`；编辑器设计的 `RecoveryStore` 却以 `DocumentKey`（项目 + 规范化真实路径）为键。Markdown 规格要求与编辑器共用同一恢复存储、配额和键，设计未给键，也未提 64 MiB/256 MiB/7 天配额与淘汰。
- 影响：快照文件/索引无法按面板 ID 对账；重命名、另存为、跨类型转换会改变路径键，导致删除/恢复目标错位；Markdown 快照无法证明与编辑器共用同一配额与淘汰；持久化启动对账与测试接缝无法落地。
- 证据：`docs/devPlan/spec-issues.md` “崩溃恢复：配额 64 MiB / 256 MiB / 7 天；键 `(ProjectId, PanelId)`”；`docs/devPlan/design/persistence.md` §崩溃快照与隔离；规格 Markdown §草稿恢复“共用同一恢复存储与配额”。
- 建议修法：把 `RecoveryStore` 的键改为 `(ProjectId, PanelId)`，路径随快照载荷保存；在编辑器与 Markdown 设计中显式引用持久化的配额、淘汰与 7 天保留规则；补恢复/删除/重命名/跨类型转换测试。
- 裁决对象：设计修订

### markdown-agentroot-boundary [P1] Markdown 链接与资源边界未纳入 AgentRoot

- owner：`docs/featureSpec/panels/markdown.md`、`docs/devPlan/design/panels/markdown.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/featureSpec/panels/markdown.md` §链接分派、§本地与远程资源、§安全边界；`docs/devPlan/design/panels/markdown.md` §面板模型与两态、§安全 HTML、链接与资源；`docs/featureSpec/panels/editor.md` §面板形态与运行模式、§文件打开路由；`docs/devPlan/design/security.md` §路径允许根与真实路径校验
- 原文：“解析后位于项目根内的本地文件，走与文件浏览器一致的应用内打开链路……解析后位于项目根外的本地路径……只读打开。”／“资源路径经归一与边界校验后，仅允许读取项目根内的本地文件。”
- 现象：编辑器规格与安全设计把已登记 Agent 全局目录（AgentRoot）列为可读且可写；Markdown 设计也承认 AgentRoot 内文档可编辑，但链接与资源规则只以项目根为边界，没有 AgentRoot 分支。
- 影响：AgentRoot 内 Markdown 链接到另一 AgentRoot 文件会按根外只读打开，与同一文件经文件浏览器打开时可编辑不一致；AgentRoot 内文档的图片会显示项目根外占位，尽管安全设计允许该根内资源加载；实现与验收将按入口分叉。
- 证据：`docs/featureSpec/panels/editor.md` §面板形态与运行模式“可编辑：路径位于项目根或已登记 Agent 全局目录内”；`docs/devPlan/design/security.md` 允许根表“资源加载（Markdown）| AgentRoot 允许”；`docs/devPlan/design/panels/markdown.md` §面板模型“AgentRoot 内可编辑保存”。
- 建议修法：在 Markdown 链接分派、资源解析与安全边界中加入 AgentRoot：链接到可写根内文件走可编辑打开，资源读取允许 AgentRoot 真实路径；明确根外链接与 AgentRoot 的优先级；补链接与图片验收。
- 裁决对象：规格修订 + 设计修订

### markdown-large-doc-form [P1] 大文档强制源码态与形态持久化没有优先级

- owner：`docs/featureSpec/panels/markdown.md`、`docs/devPlan/design/panels/markdown.md`
- 类型：缺口 / 冲突
- 定位：`docs/featureSpec/panels/markdown.md` §面板归属与两态、§大文档分级；`docs/devPlan/design/panels/markdown.md` §面板模型与两态、§大文档分级与错误
- 原文：“形态作为面板参数随项目布局持久化：……应用重启后保留上次形态”／“Markdown 文件超过 5MB 或超过 100,000 行时，默认进入源码态”。
- 现象：没有规定“重启/布局恢复时，大文档且持久化形态为渲染”谁优先；超过 50MB 又规定不提供“仍要渲染”，但持久化形态仍可能是渲染。
- 影响：同一文档在重启后会因实现选择不同而强制源码或恢复渲染；超过 50MB 时可能进入无“仍要渲染”入口的渲染态；规格验收只覆盖“打开大文档”，未覆盖恢复场景。
- 证据：规格 §面板归属与两态与 §大文档分级；设计 §面板模型与两态与 §大文档分级与错误；验收 §渲染、语法与安全“大文档”条。
- 建议修法：明确优先级。建议打开/恢复时先执行大文档判定，强制源码态并覆盖持久化形态；超过 50MB 时源码只读并禁止渲染。补充“持久化渲染形态 + 大文档重启”的验收。
- 裁决对象：规格修订

### diff-large-dirty-gutter [P1] >10MiB 工作区侧“无未保存状态”与 gutter 带入脏缓冲冲突

- owner：`docs/featureSpec/panels/diff.md`、`docs/devPlan/design/panels/diff.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/diff.md` §定位与入口、§编辑、保存与冲突、§大文件与二进制；`docs/devPlan/design/panels/diff.md` §面板模型与入口、§大文件与二进制
- 原文：“从编辑器 gutter 打开时，工作区侧取编辑器当前缓冲，包含未保存修改；缓冲与磁盘不同时面板以脏状态打开”／“工作区侧超过 10 MiB 时不存在未保存状态”。
- 现象：编辑器可编辑上限为 50,000,000 字节，因此存在 >10MiB 且脏的编辑器缓冲；从 gutter 打开 Diff 时，规则同时要求带入该缓冲并标脏、又要求工作区侧超过 10MiB 时只读且无未保存状态。
- 影响：实现要么丢弃 Diff 视图中的未保存修改（与“包含未保存修改”相反），要么保留只读脏状态（与“无未保存状态/关闭不确认”相反）；保存、关闭守卫与数据丢失提示均无定义。
- 证据：规格 §定位与入口、§大文件与二进制；设计 §面板模型与入口、§大文件与二进制；编辑器规格可编辑上限 50,000,000 字节。
- 建议修法：定义该场景。可选：>10MiB 时 gutter 打开先要求保存，或以只读快照显示并明确不属于 Diff 脏状态、不提供保存；或提高 Diff 工作区侧可编辑上限。补验收。
- 裁决对象：规格修订

### editor-detached-autosave [P1] “文件已移出允许根”状态的自动保存未定义

- owner：`docs/featureSpec/panels/editor.md`、`docs/devPlan/design/panels/editor.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/editor.md` §自动保存、§文档身份与路径变化、§保存、脏状态与关闭守卫；`docs/devPlan/design/panels/editor.md` §自动保存、崩溃恢复与外部修改
- 原文：“以下情形暂停自动保存：外部修改冲突横幅显示期间、保存冲突对话框打开期间、只读浏览、根外只读、手动只读、二进制或其他错误态。”／脱离状态“保留缓冲、编辑能力与脏状态，禁用原地保存，提供另存为”。
- 现象：脱离状态是可编辑且可脏的，但不在自动保存暂停列表；设计的 `AutoSaveState::Suspended` 原因也没有 Detached。计时器到期后既不能原地保存，也没有定义是否打开另存为。
- 影响：自动保存可能按周期失败并重复提示，或静默跳过；关闭面板时“自动保存例外先保存一次”与脱离状态只能另存为冲突；用户可能误以为已保存。
- 证据：规格 §自动保存暂停条件与 §文档身份与路径变化；设计 `AutoSaveState` 与运行模式 `Detached`。
- 建议修法：把 Detached 加入暂停条件与 `SuspendReason`，明确自动保存不做任何写盘且不弹另存为；补脱离状态下自动保存与关闭守卫的验收。
- 裁决对象：规格修订 + 设计修订

### editor-language-map-drift [P1] 编辑器语言映射表在规格与设计间不一致

- owner：`docs/featureSpec/panels/editor.md`、`docs/devPlan/design/panels/editor.md`
- 类型：冲突 / 跨篇漂移
- 定位：`docs/featureSpec/panels/editor.md` §编辑、语言高亮与脏状态、§语言选择器；`docs/devPlan/design/panels/editor.md` §编辑视图、语法高亮与折叠；`docs/featureSpec/panels/markdown.md` §渲染能力
- 原文：规格表末尾“其他或无扩展名｜纯文本”／设计表新增 `.sh`、`.ps1`、`.go`、`.java`、`.c/.h/.cpp`、`.cs`、`.sql`、`.yml`、`.toml`、`.ini`、`.diff`、`Dockerfile`、`Makefile` 等映射。
- 现象：设计把语言表扩展为编辑器、Diff、GitShow 与 Markdown 代码块共用的 `LanguageRegistry`，Markdown 规格也要求这些语言；编辑器规格仍按旧表把这些扩展名归为纯文本，语言选择器可选项也按旧表生成。
- 影响：同一 `.sh`/`.ps1` 文件按设计高亮、按规格纯文本；语言选择器选项集不一致；验收无法同时满足两份文档；共享映射的单点声明出现第二张表。
- 证据：编辑器规格语言表与 §语言选择器；设计语言表及“唯一语言映射”声明；Markdown 规格代码块语言清单。
- 建议修法：以设计表为准更新编辑器规格的语言表与选择器列表，或拆分“编辑器扩展名映射”和“Markdown 代码块语言注册表”；补代表扩展名验收。
- 裁决对象：规格修订

### diff-head-polling [P1] Diff 的周期复核与文件监听服务“不轮询”契约冲突

- owner：`docs/featureSpec/panels/diff.md`、`docs/devPlan/design/panels/diff.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/diff.md` §HEAD 刷新、§工作区外部修改；`docs/devPlan/design/panels/diff.md` §HEAD 刷新与外部修改；`docs/devPlan/design/services/filesystem-git.md` §文件监听
- 原文：“每 10 秒一次的周期复核作为兜底，用于变化通知可能缺失或网络驱动器不可靠的情况。”／“监听是租约，不轮询；视图不可见即释放或暂停。”
- 现象：Diff 规格与设计要求 10 秒定时复核，文件监听服务设计明确不做监听失效轮询，监听配置表也没有 Diff 周期复核项；GitShow 规格/设计反而明确不轮询。
- 影响：实现要么在监听服务外私建 Diff 定时器，违背服务所有权与租约模型；要么去掉规格要求的兜底，网络驱动器上的最终一致性无法兑现；测试无法按单一契约编写。
- 证据：Diff 规格/设计上述章节；`docs/devPlan/design/services/filesystem-git.md` §文件监听及 GitShow“不引入轮询”条款。
- 建议修法：由服务域裁决。若保留周期复核，在监听服务中增加 Diff 专用定时复核配置与生命周期；若不允许轮询，删除 Diff 周期复核，改为激活/窗口聚焦/手动刷新并显示“自动刷新不可用”提示。同步验收。
- 裁决对象：规格修订 + 设计修订（或 filesystem-git 契约扩展）

### agentroot-runmode [P2] 设计新增 AgentRoot 运行模式，与规格“可编辑”及手动只读判据不对齐

- owner：`docs/devPlan/design/panels/editor.md`
- 类型：漂移
- 定位：`docs/devPlan/design/panels/editor.md` §打开路由与面板身份；`docs/featureSpec/panels/editor.md` §面板形态与运行模式、§手动只读
- 原文：规格“运行模式分为可编辑、只读浏览、根外只读、文件已移出允许根与错误态”；设计 `EditorRunMode` 枚举另列 `AgentRoot`。
- 现象：规格把 AgentRoot 路径归入“可编辑”，并以“运行模式为可编辑且缓冲干净”作为手动只读开关条件；设计把 AgentRoot 作为独立模式，又声明手动只读叠加在“可编辑模式”上。规格“项目根外、仓库根内以根外只读打开”也未排除位于仓库根内的 AgentRoot，与可编辑定义存在重叠。
- 影响：AgentRoot 文件能否开启手动只读、状态条只读原因映射、模式切换与重叠路径优先级均取决于实现如何解释；规格无对应模式名。
- 证据：规格 §面板形态与运行模式与 §手动只读；设计 `EditorRunMode` 与手动只读说明。
- 建议修法：二选一：把 AgentRoot 折叠进 Editable（以可写作用域字段区分），或在规格中增加 AgentRoot 模式并逐条更新手动只读、自动保存、状态条与验收。
- 裁决对象：规格修订或设计修订

### editor-bom-model [P2] 编码与 BOM 双字段表示产生第二真值源

- owner：`docs/devPlan/design/panels/editor.md`
- 类型：缺陷
- 定位：`docs/devPlan/design/panels/editor.md` §文档缓冲与编码/行尾
- 原文：`pub encoding: TextEncoding, pub bom: bool`；`TextEncoding` 同时含 `Utf8Bom`、`Utf16LeBom`、`Utf16BeBom`。
- 现象：BOM 既由枚举变体表达，又由独立布尔字段表达；允许出现 `Utf8Bom + bom=false` 或 `Utf8 + bom=true` 等矛盾组合。
- 影响：载入、保存、恢复快照对 BOM 的读写可能出现双写、丢失或两处判断不一致；测试与 DTO 需要额外不变量。
- 证据：设计 `EditorDocument`、`TextEncoding` 定义；规格要求保存保持当前 BOM 策略。
- 建议修法：单一表示。删除 `bom` 字段并保留枚举变体，或把枚举收敛为无 BOM 编码并仅用 `bom` 标记；定义构造不变量与往返测试。
- 裁决对象：设计修订

### outside-root-large [P2] 根外只读大文件的分页与关闭高亮未定义

- owner：`docs/featureSpec/panels/editor.md`、`docs/devPlan/design/panels/editor.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/editor.md` §大文件分级；`docs/devPlan/design/panels/editor.md` §文档缓冲与编码/行尾
- 原文：“根外只读模式不受可编辑阈值影响……超过 200,000,000 字节时同样关闭语法高亮。”／“超过 50,000,000 字节的只读浏览使用分页文本源。”
- 现象：设计把分页文本源与 200MB 关闭高亮写在“只读浏览”下，但根外只读是独立的 `OutsideRoot` 模式；该模式的大文件是否分页、是否在 200MB 关闭高亮、是否支持查找进度均未定义。
- 影响：经 Markdown 链接打开的 100MB/1GB 根外文件可能被整份载入或继续高亮，违背大文件内存与流畅性目标；验收只覆盖只读浏览模式。
- 证据：规格 §大文件分级根外只读条款；设计运行模式枚举与分页文本源条款。
- 建议修法：为 `OutsideRoot` 明确定义按大小分页、搜索流式、>200MB 关闭高亮与信息条文案；补根外大文件验收。
- 裁决对象：规格修订 + 设计修订

### editor-readonly-external-refresh [P2] 设计未覆盖只读模式的外部修改静默刷新

- owner：`docs/devPlan/design/panels/editor.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/editor.md` §自动保存、崩溃恢复与外部修改；`docs/featureSpec/panels/editor.md` §外部修改
- 原文：“只读浏览、根外只读与手动只读模式的外部修改：文件事件命中后立即重新读取……全过程静默，不弹确认。”
- 现象：设计的外部修改处理表只列干净/脏可编辑、删除、重命名、重载失败与大文件切换，没有只读浏览、根外只读、手动只读的静默刷新与失败保留规则。
- 影响：实现可能对只读模式套用冲突横幅或忽略文件事件；只读视图内容陈旧；测试接缝缺整类场景。
- 证据：规格 §外部修改只读模式条款；设计外部修改表。
- 建议修法：在设计外部修改表中增加只读模式行：事件即重读、保持顶部行、失败保留最后成功内容并轻提示；补测试接缝。
- 裁决对象：设计修订

### diff-inline-rootout-edit [P2] 行内视图“工作区侧仍可编辑”未排除根外只读

- owner：`docs/devPlan/design/panels/diff.md`
- 类型：冲突
- 定位：`docs/devPlan/design/panels/diff.md` §布局、虚拟化与滚动同步；`docs/featureSpec/panels/diff.md` §视图模式、§边界与错误
- 原文：设计“行内视图为单列统一视图……工作区侧仍可编辑”；规格“行内视图中，项目根内工作区侧内容可编辑，项目根外、仓库根内工作区侧只读”。
- 现象：设计的行内视图说明没有限定项目根，可能让项目根外、仓库根内文件在行内视图可编辑/可保存。
- 影响：与只读对比模式、保存禁用和 PathGuard 写边界冲突；行内/并排切换出现能力不一致。
- 证据：规格 §视图模式；设计 §布局、虚拟化与滚动同步。
- 建议修法：设计改为“项目根内工作区侧可编辑；根外只读侧保持只读”，并补行内视图根外文件验收。
- 裁决对象：设计修订

### diff-delete-recreate-readonly [P2] 只读工作区侧被外部删除后仍提供“重新创建”写入

- owner：`docs/featureSpec/panels/diff.md`、`docs/devPlan/design/panels/diff.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/diff.md` §工作区外部修改、§保存与冲突；`docs/devPlan/design/panels/diff.md` §HEAD 刷新与外部修改
- 原文：“文件被外部删除且工作区侧干净……提供 `重新创建`（用最近基线内容写回磁盘）与 `关闭面板`。”／“项目根外、仓库根内的 Diff 以只读对比模式打开：工作区侧不可编辑、不可保存。”
- 现象：删除态规则未排除只读对比模式，而“重新创建”是写盘动作；只读侧本应不可写。
- 影响：只读对比面板会展示一个必然被 PathGuard 拒绝或越界写入的动作；用户可见动作与实际权限冲突；验收未覆盖。
- 证据：规格 §工作区外部修改、§保存与冲突根外只读条款；设计 §HEAD 刷新与外部修改。
- 建议修法：删除态动作按工作区侧可写性分派：仅项目根内提供重新创建；只读侧只提供重试/关闭。补根外 Diff 删除验收。
- 裁决对象：规格修订 + 设计修订

### diff-conflict-snapshot-lifecycle [P2] 冲突快照“源面板关闭/冲突解决即关闭”缺少设计触发

- owner：`docs/devPlan/design/panels/diff.md`
- 类型：缺口
- 定位：`docs/devPlan/design/panels/diff.md` §冲突快照对比；`docs/featureSpec/panels/diff.md` §冲突快照对比
- 原文：“源面板关闭、冲突解决或再次点击 `查看差异` 时关闭旧面板并按新快照新建。”
- 现象：设计只保存 `source_panel: PanelId` 与 `generation`，没有定义如何获知源面板关闭或冲突解决，也没有 snapshot 面板对源面板生命周期的订阅/检查机制。
- 影响：源面板关闭或冲突消解后快照面板可能残留，违背瞬态生命周期；快照文本与内存占用泄漏；去重与新建规则无法测试。
- 证据：规格 §冲突快照对比；设计 `DiffSource::ConflictSnapshot` 与 §冲突快照对比。
- 建议修法：定义源面板生命周期事件订阅或快照面板对源面板存在性/冲突状态的检查；把“源关闭/冲突解决”作为显式关闭触发并加入测试接缝。
- 裁决对象：设计修订

### gitshow-title-rootout [P2] GitShow 根外同名标题的路径基准与设计/领域模型不一致

- owner：`docs/featureSpec/panels/gitshow.md`
- 类型：冲突
- 定位：`docs/featureSpec/panels/gitshow.md` §标题、验收 §入口与标题；`docs/devPlan/design/panels/gitshow.md` §面板模型、入口与去重；`docs/devPlan/design/domain-model.md` §去重键与标题
- 原文：规格“冲突消歧优先使用相对项目根的路径；文件不在项目根下时使用相对仓库根的路径。”／设计与领域模型“项目根外使用规范化绝对路径”。
- 现象：同一根外同名文件，规格要求相对仓库根消歧，设计与标题纯函数要求绝对路径。
- 影响：标题文本与去重展示不一致；领域模型作为标题单点被规格绕过；验收只覆盖项目根内同名文件，根外场景不可判定。
- 证据：规格 §标题；设计 §面板模型、入口与去重；领域模型标题纯函数规则。
- 建议修法：统一到领域模型规则（根外绝对路径），或在领域模型为 GitShow 增加“相对仓库根”例外并写清原因；补根外同名验收。
- 裁决对象：规格修订（或领域模型例外）

### markdown-remote-referer [P2] 远程图片加载未纳入“不带 Referer”安全约束

- owner：`docs/featureSpec/panels/markdown.md`、`docs/devPlan/design/panels/markdown.md`
- 类型：缺口
- 定位：`docs/featureSpec/panels/markdown.md` §本地与远程资源、§安全边界；`docs/devPlan/design/panels/markdown.md` §安全 HTML、链接与资源；`docs/devPlan/design/security.md` §Markdown 清洗与资源加载
- 原文：安全设计“点击后单次加载且不带 Cookie/凭据/Referer”；Markdown 规格/设计“不携带 Cookie 或凭据”。
- 现象：Markdown 规格与设计只禁止 Cookie/凭据，漏掉安全设计要求的 Referer；测试接缝也未列该断言。
- 影响：实现可能带 Referer 发起远程请求，违反隐私约束；安全测试矩阵与 Markdown 验收不一致。
- 证据：安全设计 §Markdown 清洗与资源加载；Markdown 规格 §本地与远程资源；Markdown 设计 §安全 HTML、链接与资源。
- 建议修法：在规格、设计与测试接缝中明确“不携带 Cookie、凭据、Referer”；补远程图片请求头断言。
- 裁决对象：规格修订 + 设计修订

### manual-exemptions-unregistered [P2] 设计声明的人工豁免未登记到豁免清单

- owner：`docs/devPlan/design/panels/editor.md`、`docs/devPlan/design/panels/markdown.md`、`docs/devPlan/design/panels/diff.md`、`docs/devPlan/design/panels/gitshow.md`
- 类型：缺口
- 定位：四篇设计的 §测试接缝；`.claude/docs/testing/exemptions.md`
- 原文：“人工豁免：IME、字体渲染与视觉对齐、系统默认程序打开。”／“人工豁免：公式/代码高亮观感、CJK 换行。”／“人工豁免：行对对齐观感与同步滚动舒适度。”／“人工豁免：系统默认程序打开、超大文件滚动观感。”
- 现象：豁免清单已有公式观感、gutter/diff 像素对齐等条目，但 IME、系统默认程序打开、CJK 换行、超大文件滚动观感未找到对应登记。
- 影响：违反“不可自动化面必须登记豁免原因与兜底层级”的门禁；设计声明的测试边界与登记表不一致，验收覆盖无法审计。
- 证据：四篇设计测试接缝；`.claude/docs/testing/exemptions.md` 现有条目。
- 建议修法：逐项登记到豁免清单（原因、兜底层级、人工核验方式），或改为可自动化测试；设计测试接缝引用该清单。
- 裁决对象：设计修订 + 测试文档修订

### diff-dedupe-source-term [P3] Diff 去重键的 `mode/source` 表述与类型、规格术语不一致

- owner：`docs/devPlan/design/panels/diff.md`
- 类型：漂移
- 定位：`docs/devPlan/design/panels/diff.md` §面板模型与入口；`docs/devPlan/design/domain-model.md` §去重键与标题；`docs/featureSpec/panels/diff.md` §定位与入口；`docs/devPlan/spec-issues.md` §交互语义
- 原文：设计“默认模式去重键 = 项目 + 规范化工作区路径 + `mode/source`”；领域模型 `DedupeKey::Diff { project, workspace, mode: DiffMode }`。
- 现象：设计写作 `mode/source`，领域模型类型只有 `mode`，`DiffSource` 枚举也没有入口来源字段；规格只表述“同一工作区路径只存在一个 Diff 面板”。当前默认模式只有 `HeadToWorktree`，冲突快照又不参与去重，因此实际行为尚不冲突，但 `source` 一词没有定义。
- 影响：读者可能把 `source` 理解为 Commit/gutter 入口来源，从而推导出同路径双面板；未来增加入口来源或 Diff 变体时会无意改变去重语义；验收无法机械对齐。
- 证据：设计 §面板模型与入口；领域模型去重键类型；`docs/devPlan/spec-issues.md` “Diff 去重键含 `mode/source`”。
- 建议修法：把表述改为 `mode`（或明确的 `DiffMode`），或定义 `source` 字段并说明是否参与去重；同步规格、设计、领域模型与验收。
- 裁决对象：设计修订

### markdown-plaintext-memory [P3] 渲染文档保存全文 plain_text 与虚拟化内存目标不一致

- owner：`docs/devPlan/design/panels/markdown.md`
- 类型：方向建议
- 定位：`docs/devPlan/design/panels/markdown.md` §渲染管线
- 原文：`pub plain_text: String, // 渲染态查找索引`；步骤“长文档内存不随块数线性增长”。
- 现象：`plain_text` 是渲染后全文的完整副本，内存随文档字符数线性增长；与按块虚拟化、屏外释放的目标叠加时，5–50MB 文档仍可能保留整份文本副本。
- 影响：大文档渲染内存峰值偏高；与“1MB 内 1 秒首屏”之外的资源预算缺少说明；块虚拟化的收益被全文索引部分抵消。
- 证据：设计 `RenderDocument` 与渲染管线虚拟化步骤；规格 §大文档分级性能目标。
- 建议修法：方向建议：用块级文本索引或按需构建的查找索引替代全文 `plain_text`，或明确该副本的内存预算与淘汰策略。不触碰产品定位约束。
- 裁决对象：设计修订

## 交叉引用卡（域外 owner，不定级）

| 域外 owner | 证据 | 不一致点 |
|---|---|---|
| `docs/devPlan/design/services/filesystem-git.md`（文件监听） | §文件监听“监听是租约，不轮询”；监听配置表无 Diff 周期复核行 | Diff 规格/设计要求每 10 秒周期复核，需该域确认是否扩展监听服务或删除 Diff 兜底 |
| `docs/devPlan/design/security.md`（路径允许根） | §路径允许根与真实路径校验：Markdown 资源加载在 AgentRoot 允许 | Markdown 规格/设计仅允许项目根内资源，需该域确认 AgentRoot 资源策略 |
| `docs/devPlan/design/domain-model.md`（标题与去重） | §去重键与标题：项目根外使用规范化绝对路径；`DedupeKey::Diff` 仅含 mode | GitShow 规格要求根外相对仓库根；Diff 设计的 `mode/source` 在该类型中无 source 字段 |
| `docs/devPlan/design/persistence.md`（恢复存储） | §崩溃快照与隔离：`recovery/<project-id>/<panel-id>.json`、`recovery_index(project_id, panel_id)`、64 MiB/256 MiB/7 天 | 编辑器设计 `RecoveryStore` 以 `DocumentKey`（项目+路径）为键，需该域与编辑器设计统一 |
