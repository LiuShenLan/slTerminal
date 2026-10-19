# 更新下载与安装 handoff

> 下载、校验、安装交接与恢复；不信任 UI 数据，不自行安装。

## 决策与红线

- **下载器不信任 UI 数据**：资产名/版本/官方 URL 前缀/大小/摘要格式二次校验；无 SHA-256 一律拒绝下载。
- **校验链通过才原子替换**：长度、`MZ` 头、SHA-256、512 MiB 上限；中断或错误响应只能留在 `.part`，不得变成可执行文件。原因：半份或伪造产物不得进入安装链。
- **代次作废链**：取消或换版本后旧任务的进度/完成写入全部失配；取消不需要旧任务句柄。
- **文件机是持久证据**：helper 只有见到 `commit.json` 才有安装权威；未 commit 即 Drop 的 `PreparedUpdate` 写 `cancel.json` 并终止 helper。孤儿 `plan`/未 commit 的 `ready` 事务在启动时按 cancel 安全默认收尾。原因：跨进程安装必须以文件证据而非内存状态裁决。
- **helper 红线**：不按映像名杀进程、不自行安装、不绕过参与者身份核对；helper 物化自 `handoff.ps1`，`SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR` 为单名恢复变量。
- **restore ticket**：成功或回滚恢复都要版本匹配；最多 3 次尝试，`restored.json` 留档防重放。
- **安装目录归属**：安装器管理的安装（旁置 `unins000.exe`）才允许自覆盖；便携 zip 走下载页手动替换。原因：覆盖不受管的目录会破坏用户文件。
- **消费面**：更新弹窗接 `prepare/commit/schedule`；`run_shell` 启动链消费 `restore_ticket`，建窗后 `acknowledge_restore`。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：校验链、代次失配、文件机 commit/cancel 与孤儿恢复、restore ticket 尝试次数与防重放、安装目录归属判定。
- `fake_release_server_full_chain_rehearsal_is_green` 覆盖发布链彩排；`update-test-source` 提供隔离彩排通道。
- 真实 setup.exe 覆盖安装与提权链由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 下载：`spawn_once` 的 hydrate 消费 `failed_update`/cache 双证据。
- 更新弹窗：消费 `DownloadStatus` 进度、`ready_path`、`prepare/commit`、`schedule`。
- 启动恢复：消费 `restore_ticket`，建窗后确认恢复。
