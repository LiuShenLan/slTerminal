# 领域身份与状态

> 规定稳定身份、逻辑视图键、资源代次和跨聚合引用，消除路径、标题或存活实例被误当身份的情况。

## 决策与红线

### 身份类型

- ProjectId、DocumentId、PanelGroupId、TerminalId、AgentRootId、ThemeId 使用 UUIDv7 新类型；序列化采用规范小写 UUID 字符串，数据库与 DTO 不裸传无类型字符串。
- 为避免 crate 成环，DocumentId、TextStateId、RepositoryBinding、DiffBinding 由 `slterm_document` 定义；ProjectId、PanelGroupId、TerminalId、SideViewId、ViewKey 由 `slterm_workspace` 定义；ThemeId、AgentRootId 由零依赖 `slterm_settings` 定义；AgentKind、AgentSessionKey 由 `slterm_agents` 定义。跨 crate 只沿架构依赖方向引用。
- PanelId 只标识存活面板，可使用进程内递增整数并与实例纪元组合；关闭后销毁且不复用。PanelId 不进入持久布局、ViewState 或崩溃快照。
- DocumentId 独立于路径与项目。另存为、外部改名和视图切换不改变 DocumentId；重新打开前发现文件身份已变成另一文件时不得继承旧正文。
- TerminalId 属于逻辑终端记录；PTY 代次只在每次成功创建 PTY 时递增。显示编号、cwd、标题和进程 PID 都不是 TerminalId。
- AgentSessionKey 由 AgentKind、原生会话 ID 和适配器版本构成历史记录身份。活跃关联由 TerminalId、PTY 代次和 AgentSessionKey 共同证明，不另建持久活跃映射。
- Runtime 对外 ID 是带 `project:`、`group:`、`panel:`、`terminal:`、`agent:` 前缀的不透明字符串。外部客户端不得解析其内部结构；Document 不成为独立 Runtime 资源。

### ViewKey 与持久状态

ViewKey 是显式版本化、规范序列化的封闭枚举：

```text
Terminal       { ProjectId, TerminalId }
Editor         { ProjectId, DocumentId }
Markdown       { ProjectId, DocumentId }
Diff           { ProjectId, DocumentId }
GitShow        { ProjectId, RepositoryBinding, CommitId, RepoPath }
Settings       { ProjectId }
SideView       { SideViewId }
AppSettingsOverlay
```

- 普通文档 ViewKey 的项目只决定视图归属，不限制 Document 被其他项目引用。Editor 与 Markdown 显式切换保留 PanelId 和 Document，ViewKey 随逻辑视图类型改变。
- Diff 的 `DiffBinding { repository_binding, head_path }` 存入独立持久绑定，不混入列宽、换行等展示参数。
- GitShow 把创建时仓库、提交和路径放入 ViewKey；HEAD 移动和项目根重定位不产生新解释。
- SideView 与 AppSettingsOverlay 使用应用级键，不伪造 ProjectId。SideView 内部的按项目或按 AgentRoot 状态必须再以对应稳定 ID 分区。
- ViewStateStore 是唯一持久视图状态入口。搜索、焦点、选区、菜单、通知、临时错误、录制态、多选、内联编辑、折叠语法区和手动只读均不得进入 ViewState。
- 恢复遇到未知 ViewKey 类型时过滤并聚合警告；结构非法与未知类型分开处理，不创建替代 Panel。

### Document 聚合

- DocumentRegistry 持有全部存活 Document；同一文件身份在全应用最多一个实时聚合。无法取得文件身份时使用规范路径去重并保留保存冲突检测。
- 每个 Document 对应唯一 DocumentSession，统一持有文本、磁盘基线、编码、行尾、撤销、冲突、位置、授权变化和保存代次。
- Editor、Markdown 与 Diff 工作区侧只引用该聚合；冲突和大文件对比使用独立只读快照，GitShow 使用固定只读版本，均不建立第二实时 Document。
- `text_revision` 每次编辑单调递增，用于拒绝旧后台结果；`TextStateId` 标识可由撤销／重做返回的文本状态，用于脏状态和保存对账。
- 脏状态是当前 TextStateId 与 saved_state_id 不同。保存成功记录被保存状态；保存期间继续编辑时更新基线但当前仍为脏。
- 最后引用关闭才进入内容守卫。关闭集合外仍有引用时，任何批量决定都不得丢弃该 Document。

### 项目、布局与路径

- ProjectLayout 由 PanelGroup 组成；持久 DTO 只保存稳定 PanelGroupId、布局几何、结构化 ViewKey 和激活关系，不保存 PanelId。
- ProjectId、PanelGroupId、TerminalId 跨重启稳定。项目移除提交后删除其布局和专属 ViewKey，不删除仍被其他项目引用的 Document 内容或恢复材料。
- 词法路径用于项目唯一性、历史归属和展示；规范路径用于包含性与授权；文件身份用于别名合并。三者不得相互替代。
- PathAuthority 对读取、写入、终端 cwd、资源读取和启动恢复分别判定，嵌套根取能力并集。只读根不会抹除另一根的写能力。
- 根重定位只改变 ProjectRoot 关联，不重绑 Document、既有 PTY、Diff 或 GitShow。删除根立即撤销其能力并增加授权代次。

### RepositoryBinding 与版本内容

`RepositoryBinding` 保存规范 worktree_root、git_dir、common_dir、可取得的目录文件身份和创建时 HEAD：

- 复核时要求规范路径一致，并优先以目录文件身份确认同一仓库；无法取得身份时，只有创建时 HEAD 对象仍存在才可保守继续，否则报绑定不可用。
- Diff 可在同一绑定仓库内刷新 HEAD，但 HEAD 旧路径固定。项目根重定位不得改绑到新仓库的同相对路径。
- GitShow 固定创建时 CommitId 与 RepoPath，只重读该对象，不跟随 HEAD。
- 新仓库恰好复用旧路径时不得仅凭路径继承绑定；无法证明身份时显示错误并保留已加载内容。

### 代次与纪元

- `store_epoch` 标识持久工作区世代；整库恢复、重置或灾难恢复提交后更换。Runtime 幂等命名空间以其为边界。
- `stream_epoch` 标识当前进程的 Runtime 事件流；每次启动更换。事件 revision 只在 store_epoch＋stream_epoch 内单调比较。
- `operation_id` 标识持久副作用操作；Terminal、PTY、Document、Prompt、View、Task 和 Agent 关联各有自己的代次，不共用一个全局计数器。
- 所有异步结果至少携带其资源身份和对应代次。跨进程结果还必须携带 store_epoch，防止恢复后的旧客户端写入新状态。

## 验收与测试

- 属性测试覆盖 ViewKey 规范序列化往返、未知变体拒绝、ID 类型边界和持久 DTO 不含有 PanelId。
- 同一文件经不同项目、词法别名和硬链接打开时复用 Document；路径相同但文件身份改变时必须建立不同聚合。
- 保存期间继续编辑、撤销回保存状态、另存为和外部改名均保持 DocumentId 与正确脏状态。
- 根重定位到新仓库后，既有 Diff 与 GitShow 的绑定复核不得读取新仓库内容。
- 普通重启更换 stream_epoch 但不更换 store_epoch；逻辑恢复、灾难恢复和显式重置必须按各自规则更换 store_epoch。

## 操作

- 新增稳定身份时同步 DTO、Runtime 外部 ID、备份资格和恢复测试；不得只在 UI 层增加字符串 ID。
- 新增 ViewKey 变体必须在封闭注册点、规范序列化、恢复过滤、ViewState 生命周期和追溯矩阵同时登记。
