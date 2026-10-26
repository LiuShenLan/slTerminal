# slterm_workspace（core 层）

> 稳定 ID、`ViewKey`、布局 DTO 与关闭恢复的纯状态机。

## 决策与红线

- **纯工作区边界**：只承载稳定 ID、`ViewKey`、布局 DTO 与关闭恢复状态机。原因：工作区身份、布局与恢复语义须保持单一权威。边界：上述能力必须保持纯状态与纯转换。
- **面板身份不持久化**：`PanelId` 由 application 注入的实例纪元与分配器单调序列组成；分配器不可复制，关闭不回收，耗尽禁止回绕。原因：存活实例不得与持久 `ViewKey` 混用。边界：不提供持久文本解析，布局与恢复材料只保存 `ViewKey`。

## 验收与测试

- 定向测试：`cargo test -p slterm_workspace --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
