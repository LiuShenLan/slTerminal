# explorer

> Explorer 文件操作编排与侧栏渲染：树事实仍只属 `ExplorerRuntime`。

## 决策与红线

- **共享执行体**：新建、重命名、删除只经 `FileOperationService`，粘贴只经 `PasteService` 与应用会话文件剪贴板；行菜单与命令目录只产生意图。原因：树视图不得自建写盘、授权或剪贴板通道。边界：写前在后台重新解析真实路径与最新授权代次，成功只刷新受影响目录。
- **删除确认**：删除先展示确认；回收站不可用时保留待永久确认集合，不隐式永久删除。原因：破坏性操作必须由用户显式二次确认。边界：确认面文案与路径来自同一待执行意图。
- **点击与状态**：单击只改选择，目录双击只展开，文件双击与菜单打开走统一文件路由；多选时 Enter 不打开文件。Git 状态字母与忽略过滤消费同一状态投影，颜色只引用主题语义 token。原因：选择、展开与打开是不同副作用，不得互相隐式触发。边界：忽略过滤关闭时隐藏条目但不改写树模型。

## 验收与测试

- `explorer_click_and_enter_follow_selection_contract`、`explorer_directory_double_click_does_not_open_terminal` 覆盖点击、双击、多选与菜单打开。
- `explorer_menu_creates_renames_and_deletes_through_shared_executor`、`explorer_menu_copy_paste_uses_the_shared_service` 覆盖共享执行体与真实落盘。
- `explorer_git_status_and_ignored_filter_are_rendered` 覆盖状态投影与忽略过滤的真实绘制。
