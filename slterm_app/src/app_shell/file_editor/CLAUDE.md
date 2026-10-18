# file_editor

## 决策与红线

- M8.1 只点亮**只读臂**:快照经 `TextSnapshot` 类型层只读合同(`encode` 拒绝),
  UI 输入体 `disabled`;可编辑态 M8.2 才接,不得先放开输入再靠 UI 自觉。
- 读盘/解码在后台 executor,前台只在载入结果回调里落快照;Gitshow 的 HEAD 字节
  与本地文件走同一快照通路,区别只在数据源与只读意图。
- 大文件语义 = `MAX_BYTES` 截断 + `truncated` 只读提示;内存上限先于解码
  (`document_io::read_prefix`),不得先整文件物化再截断。

## 步骤接缝

- M8.2:Document/EditHistory/Outline/live_edit/保存链(Changed/ReadOnly/Io)挂到
  `TextFileView`;Code 可编辑态翻只读位。
- M8.3:渲染态预览与虚拟化、图片缩放几何。
- M8.5:gutter 与 diff 消费同一编辑器实体。
