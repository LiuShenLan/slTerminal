# 侧栏与 Agent 评审

评审范围：`docs/featureSpec/sidebar/` 全部文档、`docs/devPlan/design/sidebar/` 全部文档、`docs/devPlan/design/integrations/agent-cli.md`。只列问题，不写优点。

## 覆盖矩阵

### docs/featureSpec/sidebar/framework.md

| 章节 | 结论 |
|---|---|
| 目标行为 | `framework-navigation-state-scope`、`framework-agent-content-refresh` |
| 总体构成 | `framework-agent-content-refresh` |
| 视图集合与注册 | 无问题 |
| 视图按钮与配置钮 | 无问题 |
| 布局状态与可见性 | 无问题 |
| 视图隐藏与恢复 | `framework-view-hidden-term`、`framework-restore-default-order` |
| 按钮组织与槽位语义 | 无问题 |
| 视图生命周期与状态声明 | `framework-navigation-state-scope`、`view-state-save-failure-feedback` |
| 宽度与分割比例 | `framework-sidebar-vertical-scroll` |
| 持久化范围与恢复 | `framework-restore-default-order`、`view-state-save-failure-feedback` |
| 交互规则 | 无问题 |
| 视图按钮操作 | 无问题 |
| 拖拽排序与换区 | 无问题 |
| 右键与键盘菜单 | `framework-reset-order-hidden` |
| 视图快捷键 | 无问题 |
| 键盘导航与焦点 | 无问题 |
| 分隔线操作 | 无问题 |
| 配置钮操作 | 无问题 |
| 状态迁移 | 无问题 |
| 边界与错误 | `view-state-save-failure-feedback` |
| 空态与隐藏 | `framework-view-hidden-term` |
| 加载与恢复 | 无问题 |
| 错误 | 无问题 |
| 冲突与并发 | `view-state-save-failure-feedback` |
| 损坏与清洗 | 无问题 |
| 性能 | 无问题 |
| 键盘与焦点 | 无问题 |
| 平台与尺寸 | `framework-sidebar-vertical-scroll` |
| 安全与隐私 | 无问题 |
| 验收判据 | `framework-view-hidden-term`、`framework-navigation-state-scope` |
| 默认与启动 | 无问题 |
| 双槽与显隐 | `framework-view-hidden-term` |
| 拖拽与菜单 | 无问题 |
| 隐藏与恢复 | `framework-view-hidden-term` |
| 视图快捷键 | 无问题 |
| 宽度与比例 | 无问题 |
| 视图状态与持久化 | `framework-navigation-state-scope`、`view-state-save-failure-feedback` |
| 配置钮 | 无问题 |
| 键盘与焦点 | `framework-view-hidden-term` |
| 性能与平台 | 无问题 |
| 关键决策与依据 | `framework-agent-content-refresh`、`framework-view-hidden-term` |

### docs/featureSpec/sidebar/explorer.md

| 章节 | 结论 |
|---|---|
| 目标行为 | `explorer-reveal-contract-missing`、`explorer-multiselect-doubleclick` |
| 树根与界面 | 无问题 |
| 加载与展开 | 无问题 |
| 行构成与视觉 | 无问题 |
| 选中与多选 | `explorer-multiselect-doubleclick` |
| 打开文件 | `explorer-multiselect-doubleclick` |
| 新建与重命名 | 无问题 |
| 删除 | 无问题 |
| 复制相对路径 | `explorer-relative-path-fallback` |
| 在终端中打开 | 无问题 |
| 树内复制/剪切/粘贴 | `explorer-clipboard-source-conflict`、`explorer-cross-agent-cut-scope` |
| 名称过滤 | 无问题 |
| 忽略文件显示 | 无问题 |
| Git 状态 | 无问题 |
| 自动刷新与手动刷新 | `framework-view-hidden-term` |
| 状态保留 | `view-state-save-failure-feedback` |
| 交互规则 | 无问题 |
| 入口与出口 | `framework-view-hidden-term` |
| 鼠标 | `explorer-multiselect-doubleclick` |
| 键盘与焦点 | 无问题 |
| 右键菜单 | 无问题 |
| 快捷键 | 无问题 |
| 反馈 | 无问题 |
| 状态迁移 | `framework-view-hidden-term` |
| 边界与错误 | 无问题 |
| 空态与初始态 | 无问题 |
| 加载与性能 | 无问题 |
| 错误与恢复 | `view-state-save-failure-feedback` |
| 冲突与并发 | 无问题 |
| 路径与平台 | 无问题 |
| 键盘与输入 | 无问题 |
| 安全边界 | `explorer-clipboard-source-conflict` |
| 验收判据 | `explorer-multiselect-doubleclick`、`explorer-reveal-contract-missing` |
| 关键决策与依据 | `explorer-clipboard-source-conflict`、`explorer-cross-agent-cut-scope` |

### docs/featureSpec/sidebar/commit.md

| 章节 | 结论 |
|---|---|
| 目标行为 | `commit-rollback-index-state`、`commit-dedupe-key-drift` |
| 数据根与范围 | 无问题 |
| 四渲染态 | 无问题 |
| 列表内容与计数 | 无问题 |
| 头部过滤 | 无问题 |
| ignored 分区与显示开关 | 无问题 |
| 排序 | 无问题 |
| 自动刷新与手动刷新 | 无问题 |
| 双击打开映射 | `commit-dedupe-key-drift` |
| 右键菜单 | `commit-menu-outside-root-contradiction`、`explorer-reveal-contract-missing` |
| 回滚与删除 | `commit-rollback-index-state` |
| 项目切换与状态保留 | 无问题 |
| 不变量 | `commit-rollback-index-state` |
| 交互规则 | 无问题 |
| 入口、退出与可见性 | 无问题 |
| 鼠标 | 无问题 |
| 键盘 | 无问题 |
| 分组与滚动 | 无问题 |
| 面板打开与落点 | `commit-dedupe-key-drift` |
| 右键菜单与定位、复制流程 | `commit-menu-outside-root-contradiction`、`explorer-reveal-contract-missing` |
| 回滚与删除流程 | `commit-rollback-index-state` |
| 项目切换与视图隐藏 | `framework-view-hidden-term` |
| 反馈 | 无问题 |
| 边界与错误 | `commit-rollback-index-state` |
| 空态与无变更 | 无问题 |
| 错误分类与恢复 | 无问题 |
| 并发与冲突 | 无问题 |
| 恢复与生命周期 | `framework-view-hidden-term` |
| 性能 | 无问题 |
| 键盘与焦点 | 无问题 |
| 平台、路径与安全 | 无问题 |
| 验收判据 | `commit-rollback-index-state`、`commit-menu-outside-root-contradiction`、`commit-dedupe-key-drift` |
| 仓库根与四态 | 无问题 |
| 列表内容、计数与排序 | 无问题 |
| ignored 开关与分区 | 无问题 |
| 打开映射 | `commit-dedupe-key-drift` |
| 右键菜单与定位、复制 | `commit-menu-outside-root-contradiction`、`explorer-reveal-contract-missing` |
| 回滚与删除 | `commit-rollback-index-state` |
| 项目切换与状态保留 | 无问题 |
| 错误与恢复 | 无问题 |
| 键盘、平台与安全 | 无问题 |
| 关键决策与依据 | `commit-dedupe-key-drift`、`commit-rollback-index-state` |

### docs/featureSpec/sidebar/agent-files.md

| 章节 | 结论 |
|---|---|
| 目标行为 | `agent-files-unwritable-builtin-root` |
| 视图定位与范围 | `framework-agent-content-refresh`、`agent-files-error-banner-project-switch` |
| Agent 节点 | 无问题 |
| 展开与目录探测 | 无问题 |
| 文件树浏览 | `agent-files-unwritable-builtin-root`、`explorer-cross-agent-cut-scope` |
| 显示过滤与运行时文件清单 | `agent-cli-runtime-manifest-default` |
| 打开、终端与复制 | 无问题 |
| 目录监听与刷新 | `agent-files-watcher-scope`、`framework-view-hidden-term` |
| 状态保留与持久化 | `agent-files-error-banner-project-switch`、`view-state-save-failure-feedback` |
| 交互规则 | 无问题 |
| 入口、出口与焦点 | `framework-view-hidden-term` |
| 鼠标 | 无问题 |
| 键盘与焦点 | 无问题 |
| 右键菜单 | 无问题 |
| 反馈 | `agent-files-success-feedback-inheritance`、`agent-files-error-banner-project-switch` |
| 状态迁移 | `agent-files-watcher-scope`、`framework-view-hidden-term` |
| 边界与错误 | `agent-files-error-banner-project-switch` |
| 状态与错误反馈 | `agent-files-error-banner-project-switch` |
| 加载与恢复 | 无问题 |
| 冲突与并发 | 无问题 |
| 性能与资源 | `agent-files-watcher-scope` |
| 安全、平台与权限 | `agent-files-unwritable-builtin-root`、`explorer-cross-agent-cut-scope` |
| 键盘与输入 | 无问题 |
| 验收判据 | `agent-files-watcher-scope`、`agent-files-success-feedback-inheritance`、`framework-view-hidden-term` |
| 关键决策与依据 | `agent-files-watcher-scope`、`agent-files-success-feedback-inheritance` |

### docs/devPlan/design/sidebar/framework.md

| 章节 | 结论 |
|---|---|
| 注册表与模块 | 无问题 |
| 布局状态与不变量 | 无问题 |
| 视图按钮与配置钮 | 无问题 |
| 纯迁移 | 无问题 |
| 快捷键状态机 | 无问题 |
| 宽度与比例 | 无问题 |
| 视图状态声明与持久化 | `framework-design-mixed-state-scope`、`view-state-save-failure-feedback` |
| 键盘与焦点 | 无问题 |
| 状态作用域矩阵 | `framework-navigation-state-scope`、`framework-design-mixed-state-scope` |
| 复用与重建 | `framework-design-stale-state-file` |

### docs/devPlan/design/sidebar/explorer.md

| 章节 | 结论 |
|---|---|
| 模型与模块 | 无问题 |
| 树模型 | 无问题 |
| 选中与多选 | `explorer-multiselect-doubleclick` |
| 打开与文件操作 | 无问题 |
| 树内剪贴板 | `explorer-clipboard-source-conflict`、`explorer-cross-agent-cut-scope` |
| 名称过滤与忽略文件 | 无问题 |
| Git 状态 | 无问题 |
| 监听与刷新 | `framework-view-hidden-term` |
| 持久化与状态矩阵 | `view-state-save-failure-feedback`、`explorer-root-relocation-relative-rewrite` |
| 复用与重建 | 无问题 |

### docs/devPlan/design/sidebar/commit.md

| 章节 | 结论 |
|---|---|
| 模型与模块 | 无问题 |
| 数据根与四态 | 无问题 |
| 分组、计数与排序 | 无问题 |
| 过滤与 ignored 开关 | 无问题 |
| 打开映射与去重 | `commit-dedupe-key-drift` |
| 右键菜单与定位 | `commit-menu-outside-root-contradiction`、`explorer-reveal-contract-missing` |
| 回滚与删除 | `commit-rollback-index-state` |
| 刷新与项目切换 | 无问题 |
| 错误与性能 | 无问题 |
| 状态矩阵与持久化 | 无问题 |
| 复用与重建 | 无问题 |

### docs/devPlan/design/sidebar/agent-files.md

| 章节 | 结论 |
|---|---|
| 模型与模块 | `agent-cli-global-dirs-node-mapping`、`agent-files-filter-config-keying` |
| 节点来源与顺序 | `agent-cli-global-dirs-node-mapping` |
| 探测与展开 | 无问题 |
| 统一树与文件操作 | `agent-files-unwritable-builtin-root`、`explorer-cross-agent-cut-scope` |
| 打开、终端与标题 | 无问题 |
| 过滤消费 | `agent-cli-runtime-manifest-default`、`agent-files-filter-config-keying` |
| 监听与刷新 | `agent-files-watcher-scope`、`framework-view-hidden-term` |
| 设置中心联动 | 无问题 |
| 状态矩阵与持久化 | `agent-files-error-banner-project-switch`、`view-state-save-failure-feedback` |
| 错误与安全 | `agent-files-success-feedback-inheritance`、`agent-files-unwritable-builtin-root` |
| 复用与重建 | 无问题 |

### docs/devPlan/design/integrations/agent-cli.md

| 章节 | 结论 |
|---|---|
| 职责与边界 | `agent-cli-session-provider-io` |
| 决策与红线 | `agent-cli-unknown-source-key` |
| 模块树 | `agent-cli-session-provider-io` |
| AgentKind 能力注册表 | `agent-cli-runtime-manifest-default`、`agent-cli-global-dirs-node-mapping`、`agent-files-unwritable-builtin-root` |
| 命令识别、启动、恢复与分叉 | 无问题 |
| Hook 摄取与 statusline 用量 | `agent-cli-unknown-source-key` |
| 历史会话扫描、恢复与删除 | `agent-cli-session-provider-io`、`agent-cli-session-result-cap`、`agent-cli-isolation-path` |
| 套餐余量 | 无问题 |
| 进程快照与身份 | 无问题 |
| 系统通知事件 | 无问题 |
| 错误映射 | 无问题 |
| 复用、改造与新建 | 无问题 |
| 验收与测试接缝 | `agent-cli-session-result-cap`、`agent-cli-isolation-path` |

跨域核对：Commit 覆盖仓库实际根、Explorer 仅覆盖项目根的不对称已在 `docs/featureSpec/cross-cutting.md` §跨域不变量明确声明，未发现相关文档冲突；文件打开路由由 `docs/featureSpec/panels/editor.md` 单点拥有，Commit、Explorer、Agent 文件均引用，未发现第二套路由；Explorer 状态按 `ProjectId` 键与相对路径持久化，符合要求。导航树作用域与视图状态容量问题另列。

## 问题

P0：无。

### framework-navigation-state-scope [P1] 导航树状态作用域在框架、导航规格与侧栏设计之间三方漂移

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/framework.md` §视图生命周期与状态声明；`docs/devPlan/design/sidebar/framework.md` §状态作用域矩阵
- 原文：“视图状态作用域：导航树的展开/滚动为应用级；……项目级字段以 `ProjectId` 为键，路径存相对形式。”
- 现象：框架规格把导航树展开与滚动都声明为应用级；导航规格声明展开按项目、历史节点与未归属节点分别持久化，滚动按导航树单例持久化；侧栏设计又写成“应用级布局 + 项目级展开”。同一状态出现三种作用域口径。
- 影响：持久化键、恢复粒度和跨项目行为无法确定；实现可能只存一份全局展开集，切项目丢失展开，或错误地按项目复制滚动位置。
- 证据：`docs/featureSpec/workspace/navigation.md` §行度量与状态保留（“展开状态按项目、历史节点与未归属节点分别持久化；滚动位置按导航树单例持久化”）；`docs/devPlan/design/sidebar/framework.md` §状态作用域矩阵（导航树行“应用级布局 + 项目级展开”）。框架规格同时声明“各视图的界面状态声明由各篇定义”，此处越权声明了导航树作用域。
- 建议修法：框架规格只保留“随侧栏布局持久化”的机制与清洗规则，删除逐视图作用域断言；导航树状态由导航篇声明。设计侧把作用域从单值枚举扩展为逐字段键控，明确滚动为应用级、展开为按项目。
- 裁决对象：规格修订 / 设计修订

### framework-view-hidden-term [P1] “视图隐藏”在框架与文件浏览器、Agent 文件之间语义冲突

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/framework.md` §视图隐藏与恢复；`docs/featureSpec/sidebar/explorer.md` §入口与出口、§自动刷新与手动刷新；`docs/featureSpec/sidebar/agent-files.md` §目录监听与刷新
- 原文：框架“隐藏视图保持打开状态（若隐藏前已打开）……其内容若仍打开，继续显示于所属槽位并正常交互。”；文件浏览器“视图隐藏时不响应树快捷键、不弹出菜单。”
- 现象：框架把“视图隐藏”定义为只移除活动栏按钮，内容可继续显示和交互；文件浏览器与 Agent 文件把“视图隐藏”当作“视图不可见”，据此停快捷键、暂停或停止监听。
- 影响：用户隐藏某视图按钮后，若该视图仍打开在槽位中，文件树会变成可见但快捷键失效、菜单不弹、自动刷新停止；Agent 文件会停止监听。行为取决于实现者采用哪一语义。
- 证据：框架同节“隐藏视图仍打开时，其内容正常显示与交互”；文件浏览器 §自动刷新与手动刷新“视图隐藏或切到其他侧栏视图时暂停监听”；Agent 文件 §目录监听与刷新“视图隐藏、关闭、应用退出时停止监听”。Commit 篇使用“视图不可见（切到其他侧栏视图、换槽、隐藏侧栏）”的写法，可作为正确口径参照。
- 建议修法：在框架中新增术语“视图不可见”（非其槽位的打开视图，或侧栏实际隐藏），与“隐藏视图”（按钮隐藏）严格区分；文件浏览器、Agent 文件、Commit 的监听与快捷键条目统一改用“视图不可见”。
- 裁决对象：规格修订

### explorer-reveal-contract-missing [P1] Commit 的“在文件浏览器中显示”缺少文件浏览器侧定位契约

- owner：`docs/featureSpec/sidebar/explorer.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/explorer.md` 目标行为与交互规则；`docs/featureSpec/sidebar/commit.md` §右键菜单 / §在文件浏览器中显示
- 原文：Commit“执行时向文件浏览器发出定位请求……展开必要层级、选中目标行、滚动到可见区域并聚焦；名称过滤与忽略文件显隐的处理归文件浏览器篇。”
- 现象：文件浏览器全文没有定位请求章节，未定义：名称过滤激活时是否临时清空过滤、目标被忽略且“显示忽略文件”关闭时是否自动开启、祖先目录折叠时是否展开、目标不存在时如何反馈、定位后焦点与选中集如何设置、失败时是否保留原视图状态。
- 影响：Commit 的菜单项无法确定性实现；不同实现可能静默改过滤器或显示被忽略目标，破坏用户的视图状态。
- 证据：`docs/featureSpec/sidebar/commit.md` §在文件浏览器中显示明确把过滤与忽略显隐处理交给文件浏览器；`docs/featureSpec/sidebar/explorer.md` 无对应条款；`docs/devPlan/design/sidebar/explorer.md` 也没有定位入口或 API。
- 建议修法：在文件浏览器规格新增“定位请求”章节并声明 owner：过滤处理（建议保留过滤并提示不可见，或明确定义临时清空并恢复）、忽略开关、祖先展开、选中/滚动/聚焦、目标缺失、定位失败、返回焦点；设计侧补对应命令/接口。
- 裁决对象：规格修订

### explorer-clipboard-source-conflict [P1] 文件浏览器安全边界拒绝跨视图剪贴板源，与共享剪贴板及 Agent 继承冲突

- owner：`docs/featureSpec/sidebar/explorer.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/explorer.md` §安全边界；同篇 §树内复制/剪切/粘贴；`docs/featureSpec/sidebar/agent-files.md` §文件树浏览
- 原文：安全边界“树内剪贴板只接受来自本视图选中集的源路径”；树内剪贴板“复制源可跨视图；剪切源与粘贴目标必须同属项目根或同属 Agent 根。”
- 现象：同一篇内一处只允许“本视图”选中集，另一处允许跨视图复制；Agent 全局文件明确要求复制源可来自文件浏览器或任一 Agent 根。按安全边界字面实现会拒绝跨视图粘贴。
- 影响：Agent 继承的树内复制/剪切/粘贴能力失效；安全边界与产品能力互相矛盾，测试无法同时通过。
- 证据：`docs/featureSpec/sidebar/explorer.md` §树内复制/剪切/粘贴；`docs/featureSpec/sidebar/agent-files.md` §文件树浏览（“复制源可来自文件浏览器或任一 Agent 根”）。
- 建议修法：把安全边界改为“只接受由文件浏览器或 Agent 文件树选中集写入应用内文件剪贴板的源路径”，并在粘贴时逐项校验源与目标所属根；明确跨视图复制允许、跨根剪切限制。
- 裁决对象：规格修订

### commit-rollback-index-state [P1] 回滚结果未定义索引状态，renamed 回滚会残留索引条目

- owner：`docs/featureSpec/sidebar/commit.md`
- 类型：缺陷 / 缺口
- 定位：`docs/featureSpec/sidebar/commit.md` §回滚与删除；`docs/devPlan/design/sidebar/commit.md` §回滚与删除
- 原文：modified“用 HEAD 内容覆盖工作区文件”；renamed“恢复旧路径，删除新路径”；验收“Unversioned Files 不新增残留条目”。
- 现象：Commit 展示的是工作区与索引合并后的状态，但规格只描述覆盖工作区。设计使用 `git checkout HEAD -- path`，该命令同时改索引与工作区；renamed 回滚只恢复旧路径并删除新路径的工作区文件，未清除新路径在索引中的条目。
- 影响：对已暂存的修改执行回滚后，Changes 可能仍显示该文件；对已暂存的 rename 回滚后，新路径会以“已暂存新增/工作区已删除”残留，列表与仓库状态不一致，违反“Unversioned Files 不新增残留条目”的验收。
- 证据：`docs/featureSpec/sidebar/commit.md` §数据根与范围（“Changes 展示工作区与索引合并后的状态”）、§回滚与删除、验收 §回滚与删除；`docs/devPlan/design/sidebar/commit.md` §回滚与删除“复用 `git::rollback`（`git checkout HEAD -- path`）”。临时仓库实测：staged rename 后执行 `git checkout HEAD -- old.txt`，`git status --porcelain=v2` 仍显示 `A. new.txt`；删除工作区 new.txt 后显示 `AD new.txt`。
- 建议修法：把回滚定义为“索引与工作区同时回到 HEAD”；renamed 回滚在旧路径恢复成功后清除新路径的索引条目（如 `git reset --` / `git rm --cached`），再删除工作区文件，并定义索引清理失败的部分成功语义；补 staged modified/renamed 的验收用例。
- 裁决对象：规格修订 / 设计修订

### agent-files-watcher-scope [P1] Agent 全局文件设计保留 App 级 watcher，与规格“不可见即停止监听”冲突

- owner：`docs/devPlan/design/sidebar/agent-files.md`
- 类型：冲突
- 定位：`docs/devPlan/design/sidebar/agent-files.md` §监听与刷新；`docs/featureSpec/sidebar/agent-files.md` §目录监听与刷新、§关键决策与依据
- 原文：设计“App 级目录 watcher……不随视图可见性、Agent 展开状态或项目切换启停”；规格“视图隐藏、关闭、应用退出时停止监听并取消未触发的合并刷新计时。”
- 现象：规格把监听启用条件限定为视图可见、Agent 已展开、根可读，任一不满足即停止；设计为所有已登记 Agent 根常驻 OS watcher，仅在视图层取消订阅。
- 影响：视图隐藏或收起 Agent 后仍持有句柄并接收事件，违反跨域“侧栏视图隐藏后相关监听必须停止或取消”；Agent 数量增多时资源占用与唤醒不受控。
- 证据：`docs/featureSpec/sidebar/agent-files.md` §目录监听与刷新、§关键决策与依据“监听按可见性与展开状态启停”；`docs/featureSpec/cross-cutting.md` §监听/定时器；设计同篇明确写出 App watcher 不停止。
- 建议修法：设计改为按视图可见性与 Agent 展开状态持有租约，收起/隐藏即释放；若确需 App 级 watcher，先在规格中登记例外、资源上限与生命周期，并说明为何不违反跨域不变量。
- 裁决对象：设计修订（或规格修订并登记理由）

### agent-cli-session-provider-io [P1] SessionProvider 的扫描/删除/恢复 I/O 归属与 core/app 边界冲突

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：冲突
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §职责与边界、§模块树、§历史会话扫描、恢复与删除；`docs/devPlan/design/architecture.md` §新 core crate 边界；`docs/devPlan/design/services/filesystem-git.md` §职责与边界
- 原文：“SessionProvider……fn scan……fn delete……fn restore……fn finalize”；同篇“会话文件的读取、移动与删除经 app 服务层的路径权威与原子操作执行……不自行裸 `std::fs`。”
- 现象：`SessionProvider` 的签名直接承载扫描、删除、恢复、永久化，模块树又把 `provider.rs`/`claude.rs`/`codex.rs` 放在 `slterm_agents/sessions`（core）；但同篇与架构都要求文件 I/O 留在 app 服务层并经 PathAuthority。trait 没有任何读取端口或路径权威参数，也没有说明 provider 实现位于 app 还是 core。
- 影响：实现者若按模块树在 core 实现 provider，会绕过允许根与原子操作；若在 app 实现，则模块树与 trait 归属失真。会话域的 crate 拓扑和测试接缝无法稳定。
- 证据：`docs/devPlan/design/architecture.md` §新 core crate 边界（“Git/文件系统执行体……留在 app”）；`docs/devPlan/design/services/filesystem-git.md` §职责与边界（“所有消费者只依赖本域公开 port，不得自行 `std::fs`”）；`docs/devPlan/design/workspace/navigation.md` §模型与模块（“删除事务的隔离目录与启动清理由 app 服务执行，账本纯逻辑在 core”）。
- 建议修法：把会话域拆为 core 纯逻辑（解析、归属、删除账本、恢复计划）与 app provider 适配器（经 FileServices/PathAuthority）；provider trait 移入 app 或显式注入读取端口；模块树同步迁移。
- 裁决对象：设计修订

### framework-agent-content-refresh [P2] 框架称视图内容随活跃项目刷新，Agent 全局文件明确不随项目变化

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/framework.md` §总体构成；`docs/featureSpec/sidebar/agent-files.md` §视图定位与范围
- 原文：框架“视图内容随活跃项目刷新”；Agent 文件“切换项目不改变其布局、状态或内容。”
- 现象：框架对所有侧栏视图作统一声明，但 Agent 全局文件是应用级视图，项目切换不应刷新其内容；导航树也只是更新活跃标记，不是重新加载。
- 影响：实现者可能在切项目时重载 Agent 树或清空其状态，违反应用级视图契约并造成无谓 I/O。
- 证据：`docs/featureSpec/sidebar/agent-files.md` §视图定位与范围；同篇 §状态保留与持久化（切换项目保留展开集合、选中、滚动）。
- 建议修法：框架改为“视图内容按各视图声明响应活跃项目变化；导航树更新活跃标记，Agent 全局文件不受影响”，并把逐视图刷新规则交还各篇。
- 裁决对象：规格修订

### framework-design-mixed-state-scope [P2] 单一 StateScope 无法表达导航树的混合作用域

- owner：`docs/devPlan/design/sidebar/framework.md`
- 类型：缺口
- 定位：`docs/devPlan/design/sidebar/framework.md` §视图状态声明与持久化、§状态作用域矩阵
- 原文：`const SCOPE: StateScope; // App 或 PerProject`；表“导航树 | 应用级布局 + 项目级展开”。
- 现象：`ViewState` 只有一个 `SCOPE`，而导航树被声明为应用级布局加项目级展开；表中滚动字段也没有单独作用域。
- 影响：导航树状态序列化与恢复键无法确定；按单值实现会丢失按项目的展开或把滚动错误地按项目复制。
- 证据：同篇 §状态作用域矩阵；`docs/featureSpec/workspace/navigation.md` §行度量与状态保留。
- 建议修法：把作用域建模为逐字段（或把导航树状态移出通用 `ViewState`，由导航域持久化），并在设计矩阵中逐字段标注应用级/项目级。
- 裁决对象：设计修订

### framework-design-stale-state-file [P2] 设计迁移表仍称“独立侧栏状态文件”，与状态库单点冲突

- owner：`docs/devPlan/design/sidebar/framework.md`
- 类型：冲突
- 定位：`docs/devPlan/design/sidebar/framework.md` §复用与重建
- 原文：“重建按钮组织、按持久化域去抖与独立侧栏状态文件。”
- 现象：持久化设计已把侧栏布局写入状态库表 `sidebar`、视图状态写入 `view_states`，同事务提交；迁移表仍指独立文件。
- 影响：实现者可能新建第二套状态文件，绕过单写线程、事务与载荷上限，破坏持久化单点。
- 证据：`docs/devPlan/design/persistence.md` §状态库 schema、§写入模型与事务；同篇 §数据根与文件。
- 建议修法：改为“写入状态库 `sidebar` / `view_states`，与侧栏布局同事务”；删除“独立侧栏状态文件”表述。
- 裁决对象：设计修订

### view-state-save-failure-feedback [P2] 视图状态保存失败与容量上限缺少可见反馈

- owner：`docs/featureSpec/sidebar/framework.md`、`docs/featureSpec/sidebar/explorer.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/framework.md` §持久化范围与恢复、§冲突与并发；`docs/featureSpec/sidebar/explorer.md` §状态保留；`docs/devPlan/design/persistence.md` §写入模型与事务、§状态库 schema
- 原文：文件浏览器“状态持久化失败不阻塞使用，下次启动回退默认状态，并写入诊断日志。”；框架“视图状态保存失败不改变内存状态；下次保存周期重试。”
- 现象：持久化设计规定写失败保留 dirty 并显示失败徽标/通知，单视图载荷超过 256 KiB 时拒绝写入并通知；侧栏框架与文件浏览器只写“重试/诊断日志”，未定义用户可见反馈与容量耗尽行为。
- 影响：展开状态、当前行等静默丢失；项目多、展开集大时可能周期性超限而无提示，违反“写盘失败必须可见”。
- 证据：`docs/devPlan/design/persistence.md` 单载荷上限（侧栏 256 KiB、单视图 256 KiB）与“超限拒绝写入并通知”；`docs/featureSpec/settings-and-persistence.md` §保存模式（写盘失败可见）。
- 建议修法：框架统一声明视图状态写失败/超限的非阻塞可见提示与重试；为按 ProjectId 累积的展开状态定义容量预算、裁剪或降级策略；文件浏览器同步更新状态保留表。
- 裁决对象：规格修订 / 设计修订

### explorer-multiselect-doubleclick [P2] “多选时双击不打开”与单击清空多选的事件顺序冲突

- owner：`docs/featureSpec/sidebar/explorer.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/explorer.md` §选中与多选、§打开文件、§鼠标
- 原文：“多选时 Enter 与双击不打开文件”；“单击任意行清空其他选中，仅选中该行。”
- 现象：双击的第一次单击会先清空多选并只选中该行，等到双击事件成立时已不满足“多选时”；按事件顺序实现会打开文件，与规则相反。
- 影响：多选状态下双击的行为取决于计时/延迟实现；用户可能误打开文件，测试无法稳定判定。
- 证据：同篇 §选中与多选、§打开文件、§鼠标（“双击文件：打开……多选时双击不触发打开”）。
- 建议修法：明确双击判定基于第一次按下时的选中集；若按下时属于多选，则整次双击不打开；或定义单击延迟合并规则并给出验收用例。
- 裁决对象：规格修订

### explorer-cross-agent-cut-scope [P2] “同属 Agent 根”未区分同一 Agent 根与任意 Agent 根

- owner：`docs/featureSpec/sidebar/explorer.md`、`docs/featureSpec/sidebar/agent-files.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/explorer.md` §树内复制/剪切/粘贴；`docs/featureSpec/sidebar/agent-files.md` §文件树浏览、§安全、平台与权限
- 原文：文件浏览器“剪切源与粘贴目标必须同属项目根或同属 Agent 根”；Agent 文件“剪切源必须位于 Agent 根内……跨 Agent 选中不放宽写入边界。”
- 现象：两处都只要求“位于 Agent 根内”，没有说明从 Agent A 根剪切到 Agent B 根是否允许；是同一根内移动，还是任意两个 Agent 根之间都可移动，契约不明。
- 影响：跨 Agent 剪切/移动的实现与安全边界不一致；批量部分失败、撤销与状态迁移无法确定。
- 证据：`docs/featureSpec/sidebar/explorer.md` §树内复制/剪切/粘贴；`docs/featureSpec/sidebar/agent-files.md` 同节；`docs/featureSpec/sidebar/agent-files.md` §安全“跨 Agent 粘贴逐项校验源与目标所属根”。
- 建议修法：明确“同属 Agent 根”指同一 Agent 根还是任意 Agent 根；若允许跨 Agent 移动，定义逐项校验、写入边界与部分失败；若不允许，给出拒绝文案与验收。
- 裁决对象：规格修订

### commit-menu-outside-root-contradiction [P2] 根外行“只提供复制路径”与“定位/破坏性项禁用并提示”矛盾

- owner：`docs/featureSpec/sidebar/commit.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/commit.md` §右键菜单；`docs/devPlan/design/sidebar/commit.md` §右键菜单与定位
- 原文：“项目根外、仓库根内的行只提供复制路径；定位与破坏性操作禁用，破坏性项提示 `项目根外文件需在终端或外部工具中处理`。”
- 现象：“只提供复制路径”表示菜单只有一项；紧随其后的“定位与破坏性操作禁用”“破坏性项提示”又表示这些项存在但禁用。两句话不能同时成立。
- 影响：菜单组成与键盘可达项不确定；实现者可能省略禁用项，用户看不到原因，或保留禁用项与“只提供”文案冲突。
- 证据：`docs/featureSpec/sidebar/commit.md` §右键菜单；同篇 §在文件浏览器中显示（定位项禁用并提示）与 §回滚与删除（破坏性操作仅项目根内）；`docs/devPlan/design/sidebar/commit.md` 同节复述同一矛盾。
- 建议修法：采用“定位与破坏性项存在但禁用并带原因提示”的口径，删除“只提供复制路径”；验收明确菜单项集合与禁用态。
- 裁决对象：规格修订

### commit-dedupe-key-drift [P2] Commit 去重键含标题后缀，与 domain-model 与编辑器路由不一致

- owner：`docs/featureSpec/sidebar/commit.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/commit.md` §双击打开映射；`docs/devPlan/design/sidebar/commit.md` §打开映射与去重；`docs/devPlan/design/domain-model.md` §去重键与标题；`docs/featureSpec/panels/editor.md` §文件打开路由
- 原文：Commit“去重键为项目 + 规范绝对路径（Windows 上大小写不敏感）+ 面板标题后缀。”；设计“后缀不参与路由去重键。”
- 现象：Commit 规格把标题后缀作为去重键组成部分；设计说后缀不参与，去重键归 domain-model；编辑器路由使用“项目 + 规范化真实路径”，Diff/GitShow 分别用工作区路径加 mode/source 与仓库相对路径。路径基准与键组成有三种表述。
- 影响：同一文件经不同来源打开时是否复用面板可能不一致；符号链接与项目根外文件尤其容易分叉。
- 证据：`docs/devPlan/design/domain-model.md` §去重键与标题；`docs/featureSpec/panels/editor.md` §文件打开路由；`docs/devPlan/design/sidebar/commit.md` §打开映射与去重。
- 建议修法：Commit 规格删去自建去重键，只保留“Git 状态 → 面板类型/标题后缀”的映射并引用 domain-model；统一“规范绝对路径/规范化真实路径”的术语。
- 裁决对象：规格修订

### agent-files-error-banner-project-switch [P2] 应用级 Agent 视图在切换项目时清空错误横幅，自相矛盾

- owner：`docs/featureSpec/sidebar/agent-files.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/agent-files.md` §视图定位与范围、§状态保留与持久化、§反馈；`docs/devPlan/design/sidebar/agent-files.md` §状态矩阵与持久化
- 原文：“切换项目不改变其布局、状态或内容。”；状态表“错误横幅……切换项目 清空。”
- 现象：Agent 全局文件不依赖活跃项目，项目切换却清空其错误横幅；同表其他应用级状态（节点展开、过滤配置）均保留。
- 影响：用户正在阅读的错误被无关的项目切换丢弃；与应用级视图模型和“切换项目不改变状态”冲突。
- 证据：同篇 §状态保留与持久化表；设计同表复述“切换项目 清空”。
- 建议修法：删除切换项目清空规则，仅保留手动关闭、新错误替换或成功重试；若确需清空，给出与项目无关的理由并修改应用级声明。
- 裁决对象：规格修订

### agent-files-success-feedback-inheritance [P2] Agent 继承文件浏览器的删除/粘贴成功 toast，却声明成功不弹提示

- owner：`docs/featureSpec/sidebar/agent-files.md`
- 类型：冲突
- 定位：`docs/featureSpec/sidebar/agent-files.md` §反馈；`docs/featureSpec/sidebar/explorer.md` §删除、§树内复制/剪切/粘贴、§反馈
- 原文：Agent“树内写操作成功：刷新受影响目录，不弹成功提示。”；文件浏览器“显示短暂 toast「已移到回收站」……「已复制 N 项」/「已移动 N 项」。”
- 现象：Agent 声明继承文件浏览器的删除、树内复制/剪切/粘贴与反馈规则，但差异表没有覆盖成功反馈；反馈章又说写操作成功不弹提示。删除与粘贴成功是否显示 toast 无法确定。
- 影响：实现与验收在批量删除、剪切移动的成功反馈上分叉；Agent 与文件浏览器同类操作体验不一致。
- 证据：`docs/featureSpec/sidebar/explorer.md` §删除（回收站/toast 数量）、§树内复制/剪切/粘贴（成功 toast）、§反馈（成功类短暂 toast）；`docs/featureSpec/sidebar/agent-files.md` §反馈。
- 建议修法：在 Agent 差异表明确“继承 Explorer 的成功 toast”或“Agent 有意不显示成功 toast”，并删除另一处相反表述；补批量删除/粘贴验收。
- 裁决对象：规格修订

### agent-files-unwritable-builtin-root [P2] 内置 Agent 根可按注册只读，但视图对所有 Agent 根统一提供写操作

- owner：`docs/featureSpec/sidebar/agent-files.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/agent-files.md` §文件树浏览、§安全、平台与权限；`docs/devPlan/design/security.md` §路径允许根与真实路径校验；`docs/devPlan/design/integrations/agent-cli.md` §AgentKind 能力注册表
- 原文：Agent 文件“自定义节点与内置节点使用同一树结构、只读边界与写操作规则。”；安全设计“writable: bool, // 自定义节点可写；内置按登记”；能力表 `GlobalDir { relative_to_home, writable }`。
- 现象：内置 Agent 根是否可写由注册表逐根声明，但视图规格只说“写操作限制在 Agent 根内”，没有定义只读内置根下的新建、重命名、删除、粘贴如何禁用与提示。
- 影响：用户可在只读内置根发起写操作，最终以通用路径错误失败；或实现者忽略 writable 标志，越过安全域声明的写授权。
- 证据：`docs/devPlan/design/security.md` §路径允许根与真实路径校验；`docs/devPlan/design/services/filesystem-git.md` §允许根（写允许根为项目根与已登记 AgentRoot）；`docs/featureSpec/cross-cutting.md` §安全边界。
- 建议修法：在 Agent 视图规格声明“每个 Agent 根带可写标志；只读根禁用新建/重命名/删除/粘贴并在菜单与拖拽中说明原因”；补验收与错误文案。
- 裁决对象：规格修订

### agent-cli-runtime-manifest-default [P2] 内置 Agent 的默认运行时文件清单没有设计所有者与字段

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：缺口
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §AgentKind 能力注册表；`docs/devPlan/design/sidebar/agent-files.md` §过滤消费与模型；`docs/featureSpec/panels/settings.md` §Agent 基础配置页
- 原文：Agent 文件“内置 Agent 的默认清单来自内置能力清单（如会话、缓存、历史等根层名称）。”；`AgentDescriptor` 只有 `global_dirs`，没有运行时清单字段。
- 现象：设置页需要“恢复默认”回到内置默认清单，Agent 文件需要按默认清单隐藏根层条目，但设计没有声明默认清单的来源、key 或 owner；`AgentFilterConfig.runtime_names` 只是配置值，未说明内置默认与用户增删如何合并。
- 影响：设置页“恢复默认”和视图隐藏集合无法确定性实现；不同实现会各自硬编码默认清单，造成漂移。
- 证据：`docs/featureSpec/panels/settings.md` §运行时文件清单（内置 Agent 恢复默认回到内置清单，自定义节点为空）；`docs/devPlan/design/sidebar/agent-files.md` §过滤消费。
- 建议修法：在 `AgentDescriptor` 增加默认运行时条目字段（或指定其他单点 owner），并定义“内置默认 + 用户增删”的合并与恢复规则；自定义节点默认为空。
- 裁决对象：设计修订

### agent-cli-global-dirs-node-mapping [P2] AgentDescriptor.global_dirs 为列表，Agent 文件节点却只有一个根

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：缺口
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §AgentKind 能力注册表；`docs/devPlan/design/sidebar/agent-files.md` §模型与模块、§节点来源与顺序；`docs/featureSpec/sidebar/agent-files.md` §Agent 节点
- 原文：`global_dirs: &'static [GlobalDir]`；设计 `AgentNodeDef { root: PathBuf }`。
- 现象：能力表允许一个 Agent 声明多个全局目录，Agent 文件视图每个 Agent 只有一个节点与一个根；多个目录如何映射成一个节点、还是拆成多个节点，未定义。`GlobalDir` 只有 `relative_to_home`，也无法表达环境变量覆盖的根。
- 影响：多目录 Agent 的节点根选择不确定，部分目录不可见；环境变量根无法登记。
- 证据：同篇 §AgentKind 能力注册表；`docs/devPlan/design/sidebar/agent-files.md` §模型与模块、§节点来源与顺序；`docs/featureSpec/sidebar/agent-files.md` §Agent 节点（“全局配置目录”单数表述）。
- 建议修法：要么把能力表约束为每 Agent 一个全局目录，要么定义“每 GlobalDir 一个节点”的稳定 ID、显示名与标题消歧；若需环境变量根，扩展 `GlobalDir`。
- 裁决对象：设计修订

### agent-cli-session-result-cap [P2] 会话扫描设计未落实导航规格的结果上限

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：缺口
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §历史会话扫描、恢复与删除；`docs/devPlan/design/workspace/navigation.md` §历史会话
- 原文：导航设计“结果上限 2,000 条”；Agent CLI `scan` 返回 `Vec<SessionRecord>`，未提上限。
- 现象：扫描器、合并与投影链路没有定义每来源/总结果上限，导航视图却按 2,000 条做性能契约。
- 影响：大历史目录可能产生超量记录，内存与 UI 投影超出预算；不同实现的上限位置不一致。
- 证据：`docs/devPlan/design/workspace/navigation.md` §历史会话；`docs/devPlan/design/integrations/agent-cli.md` §历史会话扫描。
- 建议修法：在扫描/合并层声明每来源与总结果上限、排序后截断规则，并说明与导航 2,000 条契约的关系。
- 裁决对象：设计修订

### agent-cli-isolation-path [P2] 会话删除隔离区没有路径、所有者与清理生命周期

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：缺口
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §历史会话扫描、恢复与删除；`docs/devPlan/design/persistence.md` §数据根与文件
- 原文：“删除事务把来源文件原子移入应用会话隔离区……启动时清扫遗留隔离区（视为永久删除）。”
- 现象：持久化设计列出恢复快照、状态库隔离件等目录，但没有会话删除隔离区；隔离区路径、容量、写入失败后的可见反馈与启动清扫 owner 均未定义。
- 影响：删除/撤销无法确定落盘位置；崩溃后清扫可能找不到隔离文件或误清其他目录；审计与配额无法计算。
- 证据：`docs/devPlan/design/persistence.md` §数据根与文件；同篇 §状态库 schema（无会话隔离表）；Agent CLI 同节。
- 建议修法：在持久化数据根中登记会话隔离区路径（如本地根下的专用目录），定义生命周期、容量、清扫与失败反馈，并由 app 服务执行。
- 裁决对象：设计修订（跨持久化域）

### agent-cli-unknown-source-key [P2] “未知来源事件可宽松落盘会话注册表”与 SessionKey 的封闭 AgentKind 冲突

- owner：`docs/devPlan/design/integrations/agent-cli.md`
- 类型：冲突
- 定位：`docs/devPlan/design/integrations/agent-cli.md` §决策与红线、§Hook 摄取与 statusline 用量
- 原文：“未知来源事件可宽松落盘会话注册表，但恢复动作必须经 `AgentKind` 归一。”
- 现象：会话注册表按 `(kind, id)` 合并，`SessionKey.kind` 是封闭 `AgentKind`；未知来源无法构造键。若“未知来源”指未知事件类型但 Agent 已知，则表述不准确；若指未知 CLI，则与类型定义矛盾。
- 影响：未知事件可能被丢弃、被错归到已知 CLI，或产生无法恢复的注册表条目。
- 证据：同篇 §历史会话扫描（`SessionKey { kind: AgentKind, id }`、合并按 `(kind, id)`）；§决策与红线同条。
- 建议修法：区分“未知事件类型”和“未知会话来源”；只允许已知 `AgentKind` 的事件落盘，或增加显式未归属桶并禁止恢复；明确日志与丢弃规则。
- 裁决对象：设计修订

### framework-reset-order-hidden [P3] “重置顺序”的启用条件忽略隐藏视图的记住顺序

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/framework.md` §右键与键盘菜单
- 原文：“重置顺序 | 至少一个按钮组内可见按钮顺序与注册相对顺序不同 | 对全部按钮组……隐藏视图保持隐藏，其记住的组内顺序同步归位”
- 现象：菜单项只在可见按钮顺序非默认时启用，但执行时也会重置隐藏视图的记住顺序；隐藏视图顺序非默认而可见按钮顺序默认时，菜单项禁用，无法通过该入口归位。
- 影响：隐藏视图的记住顺序可能长期偏离默认，只能去设置中心处理；菜单语义与可用条件不一致。
- 证据：同篇 §右键与键盘菜单表；同篇 §视图隐藏与恢复（隐藏视图保留记住的槽位与组内顺序）。
- 建议修法：启用条件纳入隐藏视图的记住顺序，或明确“重置顺序”只作用于可见按钮、隐藏视图仅随外观页处理。
- 裁决对象：规格修订

### framework-sidebar-vertical-scroll [P3] 侧栏区整体纵向滚动与各视图内部滚动的嵌套关系未定义

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/framework.md` §宽度与分割比例、§平台与尺寸
- 原文：“窗口高度不足以让两个打开槽位各保持 120px 时，侧栏区整体垂直滚动；不自动关闭任一视图。”
- 现象：两个槽位的视图本身各有内部滚动（文件树、Commit 列表、Agent 统一树）；未定义侧栏整体滚动的滚轮命中、键盘焦点、滚动条归属，以及与视图内部滚动的边界传递。
- 影响：窗口极矮时可能出现嵌套滚动、滚轮捕获冲突或焦点滚出视口；无障碍与键盘操作难以预测。
- 证据：同篇 §宽度与分割比例、§平台与尺寸；文件浏览器 §树区域（垂直滚动）、Commit §分组与滚动（单一纵向滚动容器）、Agent 文件 §文件树浏览（整视图单一纵向滚动区）。
- 建议修法：定义只有侧栏外壳一个滚动容器，或明确“视图内部先滚到边界再滚外壳”的规则，并补键盘/滚轮验收。
- 裁决对象：规格修订

### framework-restore-default-order [P3] “恢复默认侧栏布局”的同槽冲突与缺失视图合并顺序未定义

- owner：`docs/featureSpec/sidebar/framework.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/framework.md` §视图隐藏与恢复、§持久化范围与恢复
- 原文：“若多个打开视图落入同一槽位，按注册顺序保留靠前者，其余关闭并保留实例。”；“保存记录缺少某个已注册视图：按该视图注册的默认槽位与默认顺序做稳定合并插入。”
- 现象：“注册顺序”未说明是 `ViewDef` 的注册顺序还是当前保存布局顺序；多个缺失视图同时插入同一位置时的相对顺序也未定义。
- 影响：恢复默认与损坏清洗的结果不确定，测试无法给出稳定期望。
- 证据：同篇 §视图隐藏与恢复、§持久化范围与恢复；`docs/devPlan/design/sidebar/framework.md` §注册表与模块（`default_order`）与 §纯迁移（`restore_default_layout`）。
- 建议修法：明确以注册表默认顺序为唯一排序键，并定义缺失视图按默认顺序两两比较插入；补多缺失视图的验收。
- 裁决对象：规格修订

### explorer-relative-path-fallback [P3] 树根固定项目根，却保留“复制绝对路径”回退

- owner：`docs/featureSpec/sidebar/explorer.md`
- 类型：缺口
- 定位：`docs/featureSpec/sidebar/explorer.md` §复制相对路径
- 原文：“目标无法相对化（路径不在项目根内）时，复制规范化的绝对路径。”
- 现象：文件浏览器树根固定为活跃项目根，根外目标拒绝展开/读写，Commit 的根外条目也禁止在文件浏览器中定位；因此正常路径下不存在“不在项目根内”的可复制条目。
- 影响：规则要么不可达，要么暗示存在未声明的根外条目模式；实现者会猜测是否需要支持根外行。
- 证据：同篇 §树根与界面（树根固定活跃项目根）、§路径与平台（根外目标拒绝展开/读写）；`docs/featureSpec/cross-cutting.md` §跨域不变量（项目根外、仓库根内只读；该例外不适用于文件树）。
- 建议修法：删除该回退，或明确唯一可达场景（例如符号链接条目按链接路径而非真实目标相对化）。
- 裁决对象：规格修订

### explorer-root-relocation-relative-rewrite [P3] 设计称根重定位时“改写相对路径”，与相对路径持久化语义不符

- owner：`docs/devPlan/design/sidebar/explorer.md`
- 类型：缺口
- 定位：`docs/devPlan/design/sidebar/explorer.md` §持久化与状态矩阵
- 原文：“路径存相对项目根的形式；根重定位时尝试按新根重写相对路径，无法重写的路径丢弃。”
- 现象：持久化路径已经是相对项目根的形式，根重定位后相对路径仍然有效，不需要“重写”；该句更像是在描述运行期绝对路径重新解析。
- 影响：实现者可能把持久化相对路径转成绝对路径再重写，引入不必要的失败与路径大小写问题。
- 证据：同篇 §持久化与状态矩阵；`docs/featureSpec/sidebar/explorer.md` §状态保留（“路径保存为相对项目根的形式”）；`docs/devPlan/design/workspace/navigation.md` §项目操作（根重定位只提交 `projects.root`）。
- 建议修法：改为“持久化相对路径保持不变；运行期绝对路径按新根重新解析，失效路径丢弃”，并明确大小写与链接校验。
- 裁决对象：设计修订

### agent-files-filter-config-keying [P3] AgentFilterConfig 未定义按内置 Agent/自定义节点的键控与继承

- owner：`docs/devPlan/design/sidebar/agent-files.md`
- 类型：缺口
- 定位：`docs/devPlan/design/sidebar/agent-files.md` §模型与模块、§过滤消费；`docs/featureSpec/panels/settings.md` §Agent 基础配置页
- 原文：`pub struct AgentFilterConfig { mode, names, runtime_names, show_runtime }`；设置规格“按 Agent 分组，每个内置 Agent 与每个自定义节点各占一行”。
- 现象：过滤配置在规格中按内置 Agent 与自定义节点分别配置，设计中只有一个无键的 `AgentFilterConfig`；视图如何按节点取配置、节点编辑/删除后配置如何迁移未定义。
- 影响：自定义节点或同目录多节点的过滤行为不确定；设置变更增量应用可能作用到错误的树。
- 证据：`docs/featureSpec/panels/settings.md` §Agent 基础配置页（按 Agent 分组，每个内置 Agent 与每个自定义节点各占一行）；`docs/devPlan/design/sidebar/agent-files.md` §模型与模块、§设置中心联动。
- 建议修法：把过滤配置建模为按 `AgentNodeId` 键控的映射，明确内置 Agent 与自定义节点的默认值、编辑/删除时的迁移与清理。
- 裁决对象：设计修订

## 交叉引用卡（域外，不定级）

### navigation-usage-balance-fallback 交叉引用卡：活跃会话用量与套餐余量的回退关系

- owner 域：`docs/devPlan/design/workspace/navigation.md`（工作区/导航树设计）
- 证据：导航设计 `ActiveSessionRow.usage` 注释“CLI 状态事件用量优先；缺失时匹配套餐余量窗口；无数据为 None”；`docs/devPlan/design/integrations/agent-cli.md` 明确“用量不是套餐余量，两者模型与刷新节奏独立”；`docs/featureSpec/workspace/navigation.md` 只规定无数据时显示 `--`，未定义回退套餐余量。
- 不一致点：导航设计把套餐余量作为活跃会话用量的回退来源，Agent CLI 设计把两者定义为独立模型。
- 建议：以规格为准删除回退，或在规格与 Agent CLI 设计中同时定义回退语义与刷新节奏；不要只在视图设计中加隐式回退。

### navigation-session-status-duplicate 交叉引用卡：导航设计定义了第二套会话状态枚举

- owner 域：`docs/devPlan/design/workspace/navigation.md`
- 证据：导航设计 `SessionStatus { Running { panel }, Completed, Unknown }`；Agent CLI 设计拥有 `SessionRunState { Running, Completed, Unknown }`，并声明会话领域唯一所有者。
- 不一致点：导航的 `SessionStatus` 是投影还是第二套状态模型未写明；`Running { panel }` 需要活跃会话关联，映射规则未定义。
- 建议：明确 `SessionStatus = project(SessionRunState, panel)`，映射规则写在导航设计；禁止反向写回会话域。

### navigation-session-orchestration-boundary 交叉引用卡：导航视图的扫描/恢复/删除模块与“会话域唯一所有者”边界

- owner 域：`docs/devPlan/design/workspace/navigation.md`
- 证据：导航模块树列出 `scan_service.rs`（扫描编排、取消、失败来源）、`restore.rs`（恢复/分支恢复编排）、`delete.rs`（删除事务与撤销）；同篇文字又说“导航只定义命令与行投影”，Agent CLI 设计声明“会话领域……来源扫描/归属/删除的唯一所有者”。
- 不一致点：文件命名与职责描述像第二套扫描/删除实现；即使只是命令编排，也需要明确不持有扫描、归属与删除账本。
- 建议：在导航设计标注这些模块仅做视图编排与命令派发，不复制会话域逻辑；必要时改名以避免与 `integrations/agent_cli` 的扫描/删除服务混淆。

### persistence-view-state-project-keying 交叉引用卡：`view_states` 单行 schema 与按项目/按节点视图状态

- owner 域：`docs/devPlan/design/persistence.md`
- 证据：`view_states(view_id TEXT PRIMARY KEY, state_json TEXT NOT NULL)`；单视图载荷上限 256 KiB，超限拒绝写入并通知。文件浏览器需要按 `ProjectId` 保存展开/当前行/滚动/忽略开关，Agent 文件需要按 `AgentNodeId` 保存展开集合。
- 不一致点：持久化设计没有定义 `state_json` 内部的按项目/按节点键格式、容量分摊与超限降级；侧栏框架设计也只给单一 `StateScope`。
- 建议：在持久化设计中定义逐项目/逐节点映射的稳定键、大小预算与超限策略，并让侧栏框架/视图设计引用同一格式。

### filesystem-git-tree-source-port 交叉引用卡：树模型 `TreeSource` 接收裸 `Path`，与 `ExplorerBackend` 的 `PathRef` 端口不一致

- owner 域：`docs/devPlan/design/services/filesystem-git.md`
- 证据：服务设计 `ExplorerBackend::list_dir(&self, dir: &PathRef, cursor: Option<DirCursor>)`；侧栏文件浏览器设计 `pub trait TreeSource { fn list_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, TreeError>; }`，且 `TreeModel.root/expanded` 使用 `PathBuf`。
- 不一致点：core 树模型以裸路径调用目录读取，绕过 `PathRef`/`PathAuthority` 的类型边界；若适配器在 app 侧重新解析，容易形成第二套路径校验或失效路径。
- 建议：把 `TreeSource` 定义为 app 侧适配器并显式接收 `PathRef`/访问策略，或在 core 使用不透明根/相对路径句柄；禁止 core 侧直接拼绝对路径调用文件系统。
