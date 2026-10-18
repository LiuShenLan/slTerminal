# 更新检查

## 决策与红线

- **资产合同单名化**:只认 `slTerminal-v{version}-windows-x64-setup.exe`;
  名称/架构/版本/官方 URL 前缀四重精确匹配,无 legacy 品牌回退。
- **渠道过滤**:GitHub `releases/latest` 之外的预发布/草稿一律拒绝;外部
  托管分发渠道(`platform::distribution`)在检查入口直接退出。
- **限流回退的唯一触发** = REST 403/429;终 URL 必须是本仓 HTTPS + 精确
  数字 tag,`SHA256SUMS` 只恢复下载元数据,不授予安装权威。禁止嵌 token、
  猜包 URL、把 API 失败当最新。
- **提示状态与用户设置解耦**:`update_state.json` 按版本生效;进程内 Mutex
  + 跨进程文件锁双闸,被占用时拒绝写入而非半写。
- **演练通道红线**:`test_source` 只接受 `http://127.0.0.1:<port>` 且要求
  隔离 `SLTERM_CONFIG_DIR`;存在 origin 时永不回落公网,release 构建编译期
  拒绝该 feature。
- **启动检查 best-effort**:独立线程延迟 12s;断网/坏 JSON/限流只落 debug
  日志,不弹错、不阻塞 UI。

## 步骤接缝

- 壳启动 `spawn_once` 投递 `UpdateAvailable`;安装授权 UI 归更新弹窗步。
- `fallback` 消费 `SHA256SUMS`;`assets` 的资产名同时被下载侧合同复用。
