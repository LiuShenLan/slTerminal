# file_editor

> 文件编辑器与预览：源码编辑、保存、脏标记、撤销、文档图片与 git gutter。

## 决策与红线

- **只读是类型层合同**：`TextSnapshot` 的 `read_only` 在 `encode` 入口拒绝；UI 的 `disabled` 只是辅助，不得作为唯一防线。普通源码 tab 的源码态可编辑。原因：只读意图必须在写入边界强制。边界：只读文档（truncated/gitshow/diff）不得挂 gutter，也不得经可写入口。
- **保存链单点**：`DocumentSaveService` 消费 core 捕获的不可变保存请求，与既有视图保存共用 `Document` 的「重新解析与 Write 授权 → 真实句柄身份、权限和磁盘字节复核 → core 编码 → `atomic_file` 原子替换」链；保存只写当前绑定位置，不接受调用方另传路径。Changed/ReadOnly/Unauthorized/TargetOpen/Unrepresentable/Io 结果经提示横幅或保存回执上抛，失败不破坏磁盘；替换后的文件身份直接来自原子写句柄。原因：重定向、失权、外部修改与编码失败必须可观测且不得产生半份文件。
- **另存为边界**：`DocumentSaveService::save_as` 使用用户选定目标的单文件 Write 授权，允许尚不存在的目标但只规范化其父目录并拒绝悬空链接；目标已关联任何打开 Document 时写前拒绝。编码后、原子替换前再次解析并比较目标，成功回执只写新位置并保持 DocumentId，原文件不变。原因：另存为不得覆盖其他实时聚合或跟随编码期间的重定向。
- **脏标记语义**：当前输入文本 vs 已保存快照；保存完成只清理「写完时」的版本，保存期间的新草稿仍保持脏。脏 tab 关闭必须走确认（保存/不保存/取消）。
- **后台读盘**：读盘/解码在后台 executor，前台只在载入结果回调里落快照；Gitshow 的 HEAD 字节与本地文件走同一快照通路，区别只在数据源与只读意图。`Document` 在真实打开句柄上记录规范位置与 core 文件身份，保存后刷新替换目标的身份；`app_shell::documents` 是唯一项目内真实打开服务，依次执行解析、PathPolicy 读授权、聚合查重、bounded load 与身份复核；AppRoot 的新 Editor 面板只经 `TextFileView::from_document` 消费已载入快照，既有聚合的新视图复用原已保存快照，不再触发第二次正文读盘。原因：前台不得阻塞 I/O，且授权、正文与文件身份不得因重定向错绑。边界：服务只由后台执行体调用；不得在前台整文件物化，失败不得注册空 Document。
- **外部变化服务**：`DocumentExternalService` 在后台重新解析并执行 Read 授权，只经 `Document::load` 有界读取；干净修改生成重载，脏修改与在途保存由 core 转为共享冲突，删除保留正文。PathPolicy 拒绝与 OS PermissionDenied 都标记 ReadRevoked，之后不再解析或读盘；真实改名复核目标占用、身份与基线后经 `Relocated` 重绑。原因：外部事件不得绕过授权、覆盖本地草稿或重建 Document。
- **大文件语义**：真实字节数与内容分流由 core `DocumentContent` 决定；不超过 50,000,000 字节才允许建立完整可编辑正文，超过后只读取 2 MiB 前缀并保持只读，超过 10,000,000 关闭文档内补全，超过 200,000,000 关闭语法。原因：尺寸上限必须先于解码生效，有限前缀不得伪装成完整正文。边界：Paged 的页索引、LRU 与外部增长切换由后续切片实现；未知编码、非法序列和 NUL 在真实 Document 打开时直接失败，不建立空正文或可保存基线。
- **内核族语义**：outline/inline_edit/block_*/input_rules 照抄上游语义；未消费件带 `#[allow(dead_code)]` + 消费点注释，不得靠删测试规避。
- **撤销单栈**：根视图 `capture_action` 拦截 `input::{Undo,Redo}` 并走 `EditHistory::travel`；`InputState::replace` 是静默更新（不触发 Change 订阅），程序化替换必须显式记录历史/脏标记/预览失效。原因：第二份撤销栈会产生状态分叉。边界：不得引入平行撤销实现。
- **预览虚拟化**：`Outline` 块清单 → `gpui::list` 只物化可见行；切走 tab 由 workspace 调 `set_render_active(false)` 释放块槽与图片激活，草稿/历史存活。
- **语法高亮降档**：钉版组件未启用 tree-sitter 语言集，`SyntaxHighlighter` 对映射语言回退纯文本——按「朴素渲染 + 语言标签」呈现，不回填旧编辑器高亮栈。原因：避免为单一呈现能力引入大依赖面。
- **文档图片**：GIF/动画 WebP 压单帧 PNG 进缓存是正确性措施（共用 element id 越界 panic 链）；文档级解码图走 `DocumentImageCache` 的字节/条目双上限 LRU，不得进进程级常驻资产图。引用式图片 `![alt][id]` 的定义 `[id]: url` 同走压帧；纯链接引用不改写。
- **git gutter**：数据源 = `slterm_app::git` 子进程（`file_status` + `file_hunks`），与文件树 VCS 同一执行体；仓库根经 `repository_root_cached` 短 TTL 缓存，状态查询走共享缓存。触发点 = 打开/保存/重载 + watcher 检测到外部修改（`refresh_gutter_from_disk`）；文件状态为 `None` 时直接空行标，不为无变更文件起 hunk 子进程。行标只在行标变化时推进 revision。行标颜色只引用 `EditorThemeTokens`，禁裸色值；滚动同步读输入实体上一帧布局（`visible_row_range`/`scroll_offset`）。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：保存链三类结果、脏标记与脏关闭、撤销/重做、大文件截断、文档图片压帧与 LRU、gutter 状态映射与 revision 推进、gutter 点击打开只读双栏 diff。
- 像素级对齐与真实手势等不可自动化边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 打开文件：经 `doc_tabs::openable_in_app` 路由到 Code/Document/Image；gitshow 只读臂复用 Code tab。
- 保存：只经统一保存链；不要绕过 `encode` 或直接写盘。
- 预览：由 workspace 控制渲染激活；切走 tab 时释放块槽与图片激活。
