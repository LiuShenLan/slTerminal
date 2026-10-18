# 工作区壳层

## 决策与红线

- **文件职责**:`mod.rs` = tab/pane 生命周期与状态权威;`render.rs` = 终端 tab 与
  分屏树渲染;`pumps.rs` = 自动保存/AI hook/屏幕看门狗/壳事件/系统关窗。
  渲染仍读提交比例,拖拽预览只走 overlay(壳层职责)。
- **键位单源**:静态默认表 + `STATIC_DEFAULT_COMBOS` 镜像在
  `keyboard_bindings.rs`;用户绑定只来自 settings.json `keybind` 段(行序即
  优先级、整表替换)。`Ctrl+S`/`Ctrl+/` 只在 `FileEditor` 上下文注册,无
  context 的全局绑定会吞终端焦点按键。`Ctrl+C`/`Ctrl+Shift+C` 是终端保留键
  (中断透传/复制),用户表任何条目被准入门拒绝;保留键在 `RESERVED_ROWS`
  只读登记。新增可重绑动作 = 同时更新 `EDITABLE_ACTIONS` 与默认镜像。
- **文件 tab 脏关闭**:单 tab 与整窗关闭共用「保存 / 不保存 / 取消」语义;
  `pending_file_close` 单槽保证同一时刻只有一个确认框,`window_close_confirmed`
  防异步放行后 `remove_window` 重入钩子。保存中在途一律拦截本轮。
- **文件 tab 订阅**:内容观察(`file_observations`)与 code tab 事件
  (`file_tab_events`,gutter 点击 → diff)都按稳定 TabId 存放,`remove_tab_entry`
  同步释放。
- **diff 落位**:通用 diff 是 `WorkspaceTab::Code` 的 Diff 模式(不新增
  tab 变体);gutter 点击/重开命中都走 `open_diff_tab` 单入口。
- **侧栏双槽比例**:`SidebarState.ratio_permille`(100–900,默认 500)随
  session 1 Hz 快照持久化;分隔条拖拽只改内存态,不触发 pane 重排或写盘。
