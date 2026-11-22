# slterm_agents（core 层）

> agent 目录、会话身份、命令构造与原生记录解析核心，不执行进程。

## 决策与红线

- **非执行边界**：只承载目录、会话身份、命令构造与原生记录解析。原因：agent 语义与进程生命周期必须分离。边界：不得执行进程。
- **恢复身份单源**：`AgentSession` 是旧会话 schema 与当前 TerminalRecord 共用的身份类型；来源宽松保存，恢复命令必须经 `AgentKind` 的封闭能力与原生 ID 形状校验，未知来源、无恢复能力或异形身份一律不生成命令。原因：持久输入不能直接变成 Shell 注入。边界：原生路径只用于提取经验证的会话身份，不拼入命令正文。

## 验收与测试

- 定向测试：`cargo test -p slterm_agents --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
