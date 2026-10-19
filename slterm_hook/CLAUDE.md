# slterm_hook（hook 层，bin `slterm-hook`）

> AI CLI 每个回合事件调用一次的小进程桥：把原始载荷经命名管道转发给宿主后退出。

## 决策与红线

- **零生产依赖**：纯 std 是行为合同（冷启动预算），任何生产依赖条目即被 `architecture/dependencies.toml` 拒绝；工具代码在 lib 内自实现。
- **隐形红线**：每条路径（含 panic）必须 exit 0、快速。原因：Stop hook 的非零退出会弹错误横幅甚至阻断回合。边界：stdin 必须在有界调用内抽干，不得挂住调用方或占住已安装的可执行文件。
- **作用域闸门**：只认 `SLTERM_NOTIFY_PIPE`（仅 slTerminal 内 spawn 的进程存在）；别处一律不转发、无声退出。
- **有界**：载荷上限 1 MiB、转发截止 2 秒、一次管道写；不得引入重试风暴或长驻线程。原因：hook 在调用方关键路径上，任何阻塞都会放大为 AI CLI 卡顿。边界：超限即丢弃并 exit 0。
- **外家 runner 门**：`FOREIGN_HOOK_RUNNERS` 是承重门（防把别家 agent 事件当 claude 上报），静默失效风险由宿主载荷形状校验兜底；新增 provider 时在此加一行并补用例。
- **不解析 JSON**：宿主解析 JSON，本 crate 只转发原始载荷。原因：解析失败与版本漂移不应发生在调用方关键路径。

## 验收与测试

- 定向测试：`cargo test -p slterm-hook --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 真实进程生命周期用例统一收在 lib_tests（经 `CARGO_BIN_EXE_slterm-hook` 定位二进制）；覆盖退出码、stdin 抽干、超时、作用域缺失与外家 runner 拒绝。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增 hook provider：在 `FOREIGN_HOOK_RUNNERS` 登记并补拒绝/放行用例。
- 调整载荷上限或超时：属契约数值变更，须评审并补有界行为测试。
