# slterm_hook（hook 层，bin `slterm-hook`）

## 存在理由与边界

AI CLI 每个回合事件调用一次的小进程桥：把原始载荷经命名管道转发给宿主 slTerminal 后退出。宿主解析 JSON，本 crate 不碰 JSON。

- **零生产依赖契约**：纯 std 是行为合同（冷启动预算），任何生产依赖条目即被 `architecture/dependencies.toml` 拒绝；工具代码在 lib 内自实现。
- **隐形红线**：每条路径（含 panic）必须 exit 0、快速。Stop hook 的非零退出会弹错误横幅甚至阻断回合；stdin 必须在有界调用内抽干，不得挂住调用方或占住已安装的可执行文件。
- **作用域闸门**：只认 `SLTERM_NOTIFY_PIPE`（仅 slTerminal 内 spawn 的进程存在）；别处一律不转发、无声退出。
- **有界**：载荷上限 1 MiB、转发截止 2 秒、一次管道写；不得引入重试风暴或长驻线程。
- **外家 runner 门**：`FOREIGN_HOOK_RUNNERS` 是承重门（防把别家 agent 事件当 claude 上报），静默失效风险由宿主载荷形状校验兜底；新增 provider 时在此加一行并补用例。

## 测试

- 定向测试：`cargo test -p slterm-hook --test lib_tests <filter> -- --test-threads=1`；禁 `--lib`。
- 真实进程生命周期用例统一收在 lib_tests（经 `CARGO_BIN_EXE_slterm-hook` 定位二进制）；覆盖退出码、stdin 抽干、超时、作用域缺失与外家 runner 拒绝。
