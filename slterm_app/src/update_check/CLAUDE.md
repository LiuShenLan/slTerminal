# 更新检查

> 检查官方发布、过滤渠道、维护提示状态；不授予安装权威。

## 决策与红线

- **资产合同单名化**：只认 `slTerminal-v{version}-windows-x64-setup.exe`；名称/架构/版本/官方 URL 前缀多重精确匹配，无旧品牌回退。
- **渠道过滤**：GitHub `releases/latest` 之外的预发布/草稿一律拒绝；外部托管分发渠道（`platform::distribution`）在检查入口直接退出。
- **限流回退的唯一触发** = REST 403/429；终 URL 必须是本仓 HTTPS + 精确数字 tag，`SHA256SUMS` 只恢复下载元数据，不授予安装权威。禁止嵌 token、猜包 URL、把 API 失败当最新。
- **提示状态与用户设置解耦**：`update_state.json` 按版本生效；进程内 Mutex + 跨进程文件锁双闸，被占用时拒绝写入而非半写。
- **演练通道红线**：`test_source` 只接受 `http://127.0.0.1:<port>` 且要求隔离 `SLTERM_CONFIG_DIR`；存在 origin 时永不回落公网，release 构建编译期拒绝该 feature。
- **启动检查 best-effort**：独立线程延迟 12 秒；断网/坏 JSON/限流只落 debug 日志，不弹错、不阻塞 UI。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：资产名多字段匹配、渠道过滤、限流回退条件、提示状态按版本与文件锁、演练通道隔离、启动检查不阻塞。
- 真实更新链与发布面由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 启动：`spawn_once` 投递 `UpdateAvailable`。
- 更新弹窗：打开即 `mark_prompted`；「稍后提醒」/「跳过此版本」经 `remind_later`/`skip_version` 落盘。
- 下载元数据：`fallback` 消费 `SHA256SUMS`；`assets` 的资产名同时被下载侧合同复用。
