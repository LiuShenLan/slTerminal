# slterm_workspace（core 层）

> 稳定 ID、`ViewKey`、`ViewState`、布局 DTO 与关闭恢复的纯状态机。

## 决策与红线

- **纯工作区边界**：只承载稳定 ID、`ViewKey`、`ViewState`、布局 DTO 与关闭恢复状态机。原因：工作区身份、布局与恢复语义须保持单一权威。边界：上述能力必须保持纯状态与纯转换。
- **ViewState 注册解码**：DTO 使用独立版本与封闭 `ViewKeyKind`；各所属视图注册原始 JSON 精确解码器，框架不开放任意持久字段。原因：持久字段必须由所有者声明，且重复字段不能先经 map 合并而被掩盖。边界：未知类型、未注册 schema、版本不支持、信封损坏和状态非法分别报告；作用域由匹配的 `ViewKey` 推导，不重复序列化，瞬态字段不得登记。
- **面板身份不持久化**：`PanelId` 由 application 注入的实例纪元与分配器单调序列组成；分配器不可复制，关闭不回收，耗尽禁止回绕。原因：存活实例不得与持久 `ViewKey` 混用。边界：不提供持久文本解析，布局与恢复材料只保存 `ViewKey`。
- **ViewKey 规范 DTO**：使用独立版本、BTreeMap 键序和精确字段集；文件身份的 `u128` 部分使用规范十进制字符串，非 Unicode 路径拒绝编码。原因：逐字节重放与跨机恢复不能依赖 map 遍历、JSON 数值精度或有损路径转换。边界：未知 `kind` 与结构／领域损坏分别报错；恢复时重新校验全部领域值，`PanelId` 不进入 DTO。
- **固定 Diff 绑定 DTO**：与 ViewKey 各自独立版本；仓库路径与身份只经同一 crate 内编解码转换。原因：两种持久对象可独立演进，且不得产生第二份仓库身份事实。边界：固定旧路径与仓库绑定共同恢复，未知类型、版本与结构损坏分别报告。

## 验收与测试

- 定向测试：`cargo test -p slterm_workspace --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
