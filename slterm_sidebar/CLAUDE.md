# slterm_sidebar（core 层）

> 纯文件树与侧栏状态核心。

## 决策与红线

- **纯侧栏边界**：只承载文件树与侧栏状态。原因：树结构和视图状态须可独立验证。边界：文件树与侧栏状态均保持纯数据和纯转换，不得引入副作用。

## 验收与测试

- 定向测试：`cargo test -p slterm_sidebar --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
