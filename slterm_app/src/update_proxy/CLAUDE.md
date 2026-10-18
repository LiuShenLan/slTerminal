# 更新器 HTTP/代理

## 决策与红线

- **不交给 ureq 的 `win-system-proxy`**:它把注册表 `socks=` 条目也拼成
  `http://`,会对 SOCKS 端口说 HTTP;本模块自解析 `ProxyServer`,socks 正确
  归 `socks5://`。
- **优先级**:系统代理(含注册表例外表)优先,解析失败回退
  `Proxy::try_from_env()`(`NO_PROXY` 同样生效),都没有则直连。
- **客户端纪律**:`https_only(true)`;总时限归调用方(检查 10s / 下载
  15min),连接单独 30s——不把正文截在连接时限。
- **错误文案不含代理 URL/认证信息**;测试夹具经同一代理解析进入本地回环,
  绝不触真实网络。

## 步骤接缝

- `update_check` 与 `update_download` 共用 `agent`;测试夹具
  `test_support` 只在 `cfg(test)` 编译。
