# 有界 Git 发现

## 决策与红线

- **双预算不可放宽**:3s 总时限 + 1MB 输出上限,两次元数据读取共用;
  超限/取消即降级为空结果,不猜、不重试。
- **`--no-optional-locks` 经 git facade**:本模块不得自行拼 Git 命令行;
  只读操作不得触发 index 锁或后台 gc。
- **`completion.snapshot=true` sentinel**:保证无相关配置时
  `git config --get-regexp` 也返回 0,空配置不是读取失败。
- **快照 2s TTL + generation 代次**:提交命令后旧请求不得回填缓存。
- **无网络 I/O**:只读本地引用与配置;`GIT_TERMINAL_PROMPT=0`、
  `GIT_NO_LAZY_FETCH=1`、`GIT_ALLOW_PROTOCOL=` 出厂写死。

## 步骤接缝

- `tracking.rs` 只做 refspec/上游推断的纯函数,不执行 fetch 或 shell。
- M9.1 只落来源;呈现接线归 M9.5。
