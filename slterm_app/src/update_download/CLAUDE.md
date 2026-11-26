# 更新下载与安装 handoff

> 下载、校验、安装交接与恢复；不信任 UI 数据，不自行安装。

## 决策与红线

- **下载器不信任 UI 数据**：资产名/版本/官方 URL 前缀/大小/摘要格式二次校验；无 SHA-256 一律拒绝下载。
- **校验链通过才原子替换**：长度、`MZ` 头、SHA-256、512 MiB 上限；中断或错误响应只能留在 `.part`，不得变成可执行文件。原因：半份或伪造产物不得进入安装链。
- **代次作废链**：取消或换版本后旧任务的进度/完成写入全部失配；取消不需要旧任务句柄。
- **文件机是持久证据**：helper 只有见到 `commit.json` 才有安装权威；未 commit 即 Drop 的 `PreparedUpdate` 写 `cancel.json` 并终止 helper。孤儿 `plan`/未 commit 的 `ready` 事务在启动时按 cancel 安全默认收尾。原因：跨进程安装必须以文件证据而非内存状态裁决。
- **helper 红线**：不按映像名杀进程、不自行安装、不绕过参与者身份核对；helper 物化自 `handoff.ps1`，`SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR`/`SLTERM_LOCAL_DIR` 为单名恢复变量，计划必须同时保存双数据根。
- **restore ticket**：成功或回滚恢复都要版本匹配；最多 3 次尝试，`restored.json` 留档防重放。
- **安装目录归属**：安装器管理的安装（旁置 `unins000.exe`）才允许自覆盖；便携 zip 走下载页手动替换。原因：覆盖不受管的目录会破坏用户文件。
- **关闭守卫**：更新安装预约分「待定」与「确认」两种持久态：`arm` 只写 `install-pending.json`；只有统一关闭的最终状态提交完成（`install_scheduled`）才原子提升为 `install-next.json`。关闭取消、失败、知情放弃、无 writer 完成或 AppRoot 销毁时待定文件被删除；进程崩溃留下的待定文件在启动时清理且从不作为安装授权。原因：安装预约不得绕过脏 Document 关闭守卫，内存标志也不足以表达最终提交事实。边界：`AppRoot::begin_update_install` 只在关闭前登记守卫并启动关闭；`cancel_update_install` 是用户显式放弃入口；启动消费仍走 `apply_scheduled`，只读确认文件。
- **消费面**：启动链在系统安装事务守卫之后消费 `apply_scheduled`：预约成功即退出进程交给 helper 覆盖安装，失败解除预约并继续正常启动；`prepare/commit`、`UpdateInstallGuard` 与 restore ticket 由更新域的协调器装配，不读取旧 session。`integration_update` 覆盖退出中断（未 commit 按 cancel 收尾）、已 commit 等待 helper 结果与回滚恢复票防重放。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：校验链（`update_asset_validation_rejects_mismatch`）、代次失配、文件机 commit/cancel 与孤儿恢复、restore ticket 尝试次数与防重放、安装目录归属判定。
- `fake_release_server_full_chain_rehearsal_is_green` 覆盖发布链彩排；`update-test-source` 提供隔离彩排通道。
- 真实 setup.exe 覆盖安装与提权链由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 下载：`spawn_once` 的 hydrate 消费 `failed_update`/cache 双证据。
- 更新协调：消费 `DownloadStatus` 进度、`ready_path`、`prepare/commit` 与 `UpdateInstallGuard`，并经新 ProjectSnapshot 完成恢复。
