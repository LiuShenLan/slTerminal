# 有界 Git 发现

> 为补全提供本地 Git 上下文：预算、只读 facade、sentinel 与代次缓存。

## 决策与红线

- **双预算不可放宽**：3 秒总时限 + 1 MiB 输出上限，两次元数据读取共用；超限/取消即降级为空结果，不猜、不重试。原因：补全不得阻塞或拖垮终端。
- **`--no-optional-locks` 经 git facade**：本模块不得自行拼 Git 命令行；只读操作不得触发 index 锁或后台 gc。
- **`completion.snapshot=true` sentinel**：保证无相关配置时 `git config --get-regexp` 也返回 0，空配置不是读取失败。
- **快照 2 秒 TTL + generation 代次**：提交命令后旧请求不得回填缓存。
- **无网络 I/O**：只读本地引用与配置；`GIT_TERMINAL_PROMPT=0`、`GIT_NO_LAZY_FETCH=1`、`GIT_ALLOW_PROTOCOL=` 出厂写死。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：双预算超限降级、facade 命令行形态、sentinel 空配置、TTL/代次失效、无网络环境变量。
- `tracking.rs` 只做 refspec/上游推断的纯函数，不执行 fetch 或 shell。

## 操作

- 新增 Git 读取：经 `git::command` 构造口，复用同一预算与缓存语义。
- 调整预算或 TTL：属契约数值变更，须评审并补降级用例。
