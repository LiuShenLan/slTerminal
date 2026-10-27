# 工作区壳层

> 单窗口工作区：tab/pane 生命周期、分屏树、键位、脏关闭、侧栏双槽与复制语义。

## 决策与红线

- **单窗口实体**：`SltermWorkspace` 直持 tab/pane 实体与展示态，无投影层。`WorkspaceTab` 是封闭枚举（Terminal/Settings/Document/Code/Image）；新增类型须在单一注册点登记，恢复时过滤未知。原因：单窗口定位下无独立页宿主与跨页终端存活需求。
- **分屏树**：每个 Terminal tab 持 `panes` + `SplitTree<PaneId>` + `focused`；pane id 全 workspace 唯一、终生不复用；关 pane 由 `RemoveOutcome` 裁定（关整 tab / 兄弟收编）。布局只经 session schema 的快照/恢复转换对。
- **布局提交与拖拽预览**：渲染读提交比例；拖拽预览只走 overlay，不触发 pane 重排或写盘。原因：拖拽期间重排会打断终端网格。边界：终端网格只读提交比例。
- **侧栏双槽状态机**：ActivityBar + 上下双槽内容区；视图封闭集 `SideViewId`；每半区最多一个打开视图，无「打开未展示」中间态与历史记忆，展示由状态纯推导。点击未打开视图 = 打开并隐式替换同槽旧视图；点击已打开视图 = 关闭该槽；双空则侧栏整体隐藏。原因：状态最少、展示无临时态。边界：拖拽换区只改归属或立即跟随，不保留隐藏态。
- **SideViewHost 实例所有权**：`slterm_sidebar::SidebarState` 只裁定转换效果，application `SideViewHost` 独占实例表并执行创建与关闭。原因：组织状态与运行实例必须分离，宿主不得再次推断生命周期。边界：关闭与同槽替换销毁实例并取消私有任务；整栏隐藏、隐藏按钮和跨槽交换保留实例；显式 shutdown 与 Drop 共用关闭路径，应用级后台职责不随视图销毁取消。
- **侧栏状态归 session**：collapsed/width/active_view 与双槽比例跟随现场恢复，不进 settings 偏好域。`SidebarState.ratio_permille`（100–900，默认 500）随 session 1 Hz 快照持久化；分隔条拖拽只改内存态，不触发 pane 重排或写盘。
- **键位单源**：静态默认表 + `STATIC_DEFAULT_COMBOS` 镜像在 `keyboard_bindings.rs`；用户绑定只来自 settings `keybind` 段（行序即优先级、整表替换）。`Ctrl+S`/`Ctrl+/` 只在 `FileEditor` 上下文注册，无 context 的全局绑定会吞终端焦点按键。`Ctrl+C`/`Ctrl+Shift+C` 是终端保留键（中断透传/复制），用户表任何条目被准入门拒绝；保留键在 `RESERVED_ROWS` 只读登记。新增可重绑动作 = 同时更新 `EDITABLE_ACTIONS` 与默认镜像。
- **复制语义**：`Ctrl+Shift+C` 有选区 → 复制并反馈；无选区 → 静默吞，不透传中断、不写剪贴板。`Ctrl+C` 恒中断并透传 PTY，供 AI CLI 取消。原因：复制键永不进 PTY；中断键始终可用。
- **文件 tab 脏关闭**：单 tab 与整窗关闭共用「保存 / 不保存 / 取消」语义；`pending_file_close` 单槽保证同一时刻只有一个确认框，`window_close_confirmed` 防异步放行后 `remove_window` 重入钩子。保存中在途一律拦截本轮。
- **文件 tab 订阅**：内容观察（`file_observations`）与 code tab 事件（`file_tab_events`，gutter 点击 → diff）都按稳定 TabId 存放，`remove_tab_entry` 同步释放。
- **diff 落位**：通用 diff 是 `WorkspaceTab::Code` 的 Diff 模式（不新增 tab 变体）；gutter 点击/重开命中都走 `open_diff_tab` 单入口。
- **快照范围**：快照只收 Terminal tab；Settings/文件 tab 是纯视图，无进程/PTY 生命周期。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：侧栏双槽状态机、分屏删除裁定、键位准入与保留键、脏关闭语义、文件 tab 订阅释放、diff 单入口、复制语义、拖拽预览不重排。
- 真实拖拽手势与拖拽手感由实机验收兜底，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增 tab 类型：扩封闭枚举 + 单一注册点 + 恢复过滤。
- 新增可重绑动作：同步 `EDITABLE_ACTIONS` 与默认镜像，并补准入门用例。
- 新增侧栏视图：扩 `SideViewId` + 单一注册点，未注册 id 恢复时静默丢弃。
