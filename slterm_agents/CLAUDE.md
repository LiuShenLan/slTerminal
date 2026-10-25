# slterm_agents（core 层）

> agent 目录、会话身份、命令构造与原生记录解析核心，不执行进程。

## 决策与红线

- **非执行边界**：只承载目录、会话身份、命令构造与原生记录解析。原因：agent 语义与进程生命周期必须分离。边界：不得执行进程。

## 验收与测试

- 定向测试：`cargo test -p slterm_agents --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
