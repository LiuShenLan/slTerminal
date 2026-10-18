# 更新下载与安装 handoff

## 决策与红线

- **下载器不信任 UI 数据**:资产名/版本/官方 URL 前缀/大小/摘要格式二次
  校验;无 SHA-256 一律拒绝下载。
- **四重通过才原子替换**:长度、`MZ` 头、SHA-256、512MiB 上限;中断或错误
  响应只能留在 `.part`,不得变成可执行文件。
- **generation 作废链**:取消或换版本后旧任务的进度/完成写入全部失配;
  取消不需要旧任务句柄。
- **文件机是持久证据**:helper 只有见到 `commit.json` 才有安装权威;
  未 commit 即 Drop 的 `PreparedUpdate` 写 `cancel.json` 并终止 helper。
  孤儿 `plan`/未 commit 的 `ready` 事务在启动时按 cancel 安全默认收尾。
- **helper 红线**:不按映像名杀进程、不自行安装、不绕过参与者身份核对;
  helper 物化自 `handoff.ps1`,`SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR`
  为单名恢复变量。
- **restore ticket**:成功或回滚恢复都要版本匹配;最多 3 次尝试,
  `restored.json` 留档防重放。
- **安装目录归属**:安装器管理的安装(旁置 `unins000.exe`)才允许自覆盖;
  便携 zip 走下载页手动替换。
- **未消费件**:安装授权 UI/恢复入口尚未接线,handoff 与平台安装模块整
  模块豁免 dead_code;接线时逐个删除豁免。

## 步骤接缝

- `spawn_once` 的 hydrate 消费 `failed_update`/cache 双证据。
- 更新弹窗消费 `DownloadStatus` 进度、`ready_path`、`prepare/commit`、
  `schedule`;启动恢复消费 `restore_ticket`。
