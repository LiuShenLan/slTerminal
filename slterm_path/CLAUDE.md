# slterm_path（core 层）

> 纯 `AppPaths`、路径与授权规则核心，不读取环境或执行 I/O。

## 决策与红线

- **纯路径边界**：只承载 `AppPaths`、路径与授权规则。原因：路径与授权语义须可脱离系统环境独立验证。边界：不得读取环境变量或执行 I/O。

## 验收与测试

- 定向测试：`cargo test -p slterm_path --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
