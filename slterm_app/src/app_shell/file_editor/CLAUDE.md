# file_editor

> 文件编辑器与预览：源码编辑、保存、脏标记、撤销、文档图片与 git gutter。

## 决策与红线

- **只读是类型层合同**：`TextSnapshot` 的 `read_only` 在 `encode` 入口拒绝；UI 的 `disabled` 只是辅助，不得作为唯一防线。普通源码 tab 的源码态可编辑。原因：只读意图必须在写入边界强制。边界：只读文档（truncated/gitshow/diff）不得挂 gutter，也不得经可写入口。
- **保存链单点**：`verify`（磁盘字节）→ `encode`（BOM/CRLF 往返）→ `atomic_file` 原子替换；Changed/ReadOnly/Io 三类结果经提示横幅上抛，失败不破坏磁盘。原因：保存失败必须可观测且不产生半份文件。
- **脏标记语义**：当前输入文本 vs 已保存快照；保存完成只清理「写完时」的版本，保存期间的新草稿仍保持脏。脏 tab 关闭必须走确认（保存/不保存/取消）。
- **后台读盘**：读盘/解码在后台 executor，前台只在载入结果回调里落快照；Gitshow 的 HEAD 字节与本地文件走同一快照通路，区别只在数据源与只读意图。原因：前台不得阻塞 I/O。边界：不得在前台整文件物化。
- **大文件语义**：`MAX_BYTES` 截断 + `truncated` 只读提示；内存上限先于解码（`document_io::read_prefix`），不得先整文件物化再截断。
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
