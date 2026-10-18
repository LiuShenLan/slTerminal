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

## 步骤接缝

- M8.2 已落:Document/EditHistory/内核族/源码态可编辑可存/脏标记/脏关闭确认。
- M8.3:渲染态预览与虚拟化、live_edit 输入实体与装饰、Outline 消费、图片缩放。
  届时需收口 InputState 内建撤销与本仓 `EditHistory` 的双栈(源码态 Ctrl+Z 目前
  仍走组件内建历史;`EditHistory::travel` 尚未接线)。
- M8.5:gutter 与 diff 消费同一编辑器实体。
