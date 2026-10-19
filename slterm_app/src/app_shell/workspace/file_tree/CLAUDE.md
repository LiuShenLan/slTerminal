# 文件树渲染 / 路径栏

> 文件树的几何、双击路由、拖放载荷、路径栏与右键菜单；模型归侧栏文件树模块。

## 决策与红线

- **行几何单点**：行距 34 / 水洗 30 / 内缩常量在 `file_tree.rs` 顶部；行缝来自水洗矮于行距，禁止改用 flex gap（连续列表读感）。数值重定归视觉评审。
- **双击路由**：命中 = `doc_tabs::openable_in_app` → `open_document_path`；未命中 = 系统处理器。系统处理器在测试构建走注入记录，生产走 `platform::file_operations::open_in_system`；不要在渲染层直接调 `ShellExecuteW`。原因：打开策略单点，才能统一安全与测试边界。
- **拖放载荷单点**：`FileTreeDrag` 在 `app_shell::file_drop`；写 PTY 的路径形状规则在 `side_panel::drop_text_for_path`。行渲染只组合二者，不另写引用/控制字符逻辑。OS 拖出经 `external_drag_payload`（平台渲染系统图标）。
- **路径栏只拥有草稿**：输入变化不导航；提交时后台校验（磁盘检查不进渲染路径），根身份变了旧结果直接丢弃。原因：渲染路径不得做 I/O。边界：路径栏只负责输入与提交，不提供补全。
- **文件图标**：字形映射归 `side_panel::icons`；色归主题 token，禁止在本目录出现颜色字面量。
- **右键菜单**：`FileTreeContextMenu` 挂 workspace 根渲染（不受抽屉裁剪），遮罩点击关闭；删除动作单点 = `platform::file_operations::send_to_recycle_bin`，测试构建走注入记录。菜单只对真实条目开放，合成 `..` 行无删除入口。
- **搜索选项**：只改 `SidePanel::search_options` 并重排结果；不另存 UI 侧影子状态。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：行几何、双击路由、拖放载荷形状、路径栏提交与根身份失效、右键菜单只对真实条目开放、搜索选项重排。
- 外部文件变化后的树刷新由 UI 用例与人工验收兜底，见 `.claude/docs/testing/exemptions.md`。

## 操作

- 修改行几何或视觉：同步顶部常量与视觉评审结论。
- 新增文件操作：经 `platform::file_operations` 单点；不在渲染层直接调平台 API。
- 修改拖放：同时检查 `FileTreeDrag` 与 `drop_text_for_path` 两处形状规则。
