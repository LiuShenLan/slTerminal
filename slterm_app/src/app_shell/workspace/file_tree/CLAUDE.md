# 文件树渲染 / 路径栏

## 决策与红线

- **行几何单点**:行距 34 / 水洗 30 / 内缩常量在 `file_tree.rs` 顶部;行缝
  来自水洗矮于行距,禁止改用 flex gap(连续列表读感)。数值重定归视觉评审。
- **双击路由**:命中 = `doc_tabs::openable_in_app` → `open_document_path`;
  未命中 = 系统处理器。系统处理器在测试构建走注入记录,生产走
  `platform::file_operations::open_in_system`;不要在渲染层直接调
  `ShellExecuteW`。
- **拖放载荷单点**:`FileTreeDrag` 在 `app_shell::file_drop`;写 PTY 的路径
  形状规则在 `side_panel::drop_text_for_path`。行渲染只组合二者,不另写
  引用/控制字符逻辑。OS 拖出经 `external_drag_payload`(平台渲染系统图标)。
- **路径栏只拥有草稿**:输入变化不导航;提交时后台校验(磁盘检查不进
  渲染路径),根身份变了旧结果直接丢弃。
- **补全缺口**:设计稿没有补全合同,本实现按签名稿
  提供输入/校验/提交;若后续要补全,先补设计合同再落 UI。
- **文件图标**:字形映射归 `side_panel::icons`;色归主题 token,禁止在本目录
  出现颜色字面量。
- **右键菜单**(M8.5):`FileTreeContextMenu` 挂 workspace 根渲染(不受抽屉裁剪),
  遮罩点击关闭;删除动作单点 = `platform::file_operations::send_to_recycle_bin`,
  测试构建走注入记录。菜单只对真实条目开放,合成 `..` 行无删除入口。
- **搜索选项三钮**:只改 `SidePanel::search_options` 并重排结果;不另存 UI 侧
  影子状态。
