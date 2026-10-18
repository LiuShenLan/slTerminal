# file_editor

## 决策与红线

- 只读仍是**类型层**合同:`TextSnapshot` 的 `read_only` 在 `encode` 入口拒绝;
  UI 的 `disabled` 只是辅助,不得作为唯一防线。M8.2 起普通源码 tab 源码态可编辑。
- 保存链单点 = `verify`(磁盘字节)→ `encode`(BOM/CRLF 往返)→ `atomic_file`
  原子替换;Changed/ReadOnly/Io 三态经提示横幅上抛,失败不破坏磁盘。
- 脏标记 = 当前输入文本 vs 已保存快照;保存完成只清理「写完时」的版本,保存
  期间的新草稿仍保持脏。脏 tab 关闭必须走确认(保存/不保存/取消)。
- 读盘/解码在后台 executor,前台只在载入结果回调里落快照;Gitshow 的 HEAD 字节
  与本地文件走同一快照通路,区别只在数据源与只读意图。
- 大文件语义 = `MAX_BYTES` 截断 + `truncated` 只读提示;内存上限先于解码
  (`document_io::read_prefix`),不得先整文件物化再截断。
- 内核族(outline/inline_edit/block_*/input_rules)照抄上游语义;未消费件带
  `#[allow(dead_code)]` + 消费点注释,不得靠删测试规避。
- 撤销统一(M8.3):根视图 `capture_action` 拦截 `input::{Undo,Redo}` 并走
  `EditHistory::travel`;`InputState::replace` 是静默更新(不触发 Change 订阅),
  程序化替换必须显式记录历史/脏标记/预览失效。不得引入第二份撤销栈。
- 预览虚拟化:`Outline` 块清单 → `gpui::list` 只物化可见行;切走 tab 由
  workspace 调 `set_render_active(false)` 释放块槽与图片激活,草稿/历史存活。
- 预览代码块高亮:钉版 gpui-component 未启用 `tree-sitter-languages`,
  `SyntaxHighlighter` 对映射语言回退纯文本——按设计开放问题 5 降档为
  「朴素渲染 + 语言标签」,不回填 CM6 高亮栈。
- 文档图片:GIF/动画 WebP 压单帧 PNG 进缓存是正确性措施(共用 element id
  越界 panic 链);文档级解码图走 `DocumentImageCache` 的字节/条目双上限 LRU,
  不得进进程级常驻资产图。引用式图片 `![alt][id]` 的定义 `[id]: url` 同走
  压帧;纯链接引用不改写。
- git gutter(M8.5):数据源 = `crate::git` 子进程(`file_status` + `file_hunks`),
  与文件树 VCS 同一执行体;仓库根经 `repository_root_cached` 短 TTL 缓存,
  状态查询走共享缓存。触发点 = 打开/保存/重载 + watcher 检测到外部修改
  (`refresh_gutter_from_disk`);文件状态为 `None` 时直接空行标,不为无变更
  文件起 hunk 子进程。行标只在行标变化时推进 revision。只读文档
  (truncated/gitshow/diff)不挂 gutter。行标颜色只引用 `EditorThemeTokens`,
  禁裸色值;滚动同步读输入实体上一帧布局(`visible_row_range`/`scroll_offset`)。

## 步骤接缝

- M8.2 已落:Document/EditHistory/内核族/源码态可编辑可存/脏标记/脏关闭确认。
- M8.3 已落:双模态渲染态 + 虚拟化 + live_edit/decorations/结构命令 + 撤销
  双栈收口 + Document 可编辑与脏关闭守卫 + 图片缓存/真实缩放 + 详情侧栏/
  工具条/几何常量族。
- M8.5 已落:gutter 行标(打开/保存/重载三触发点)+ `TextFileEvent::GutterClicked`
  事件转发 + 通用 diff 消费同一编辑器实体;`Outline::source_span`/
  `replace_block` 预留给代码块语言切换与结构回写。
