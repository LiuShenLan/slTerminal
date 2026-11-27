# 更新器 HTTP/代理

> 更新检查与下载共用的 HTTP 客户端与代理解析。

## 决策与红线

- **系统代理归 `platform::system_proxy`**：注册表 `ProxyServer` 的解析与 socks 归一已上移到平台单点，更新与终端子进程环境共用；本模块只做目标协议选择与 ureq 客户端构造。原因：两套解析会让 SOCKS 端口被当 HTTP 或产生例外表漂移。
- **优先级**：系统代理（含注册表例外表）优先，解析失败回退 `Proxy::try_from_env()`（`NO_PROXY` 同样生效），都没有则直连。
- **客户端纪律**：`https_only(true)`；总时限归调用方（检查 10 秒 / 下载 15 分钟），连接单独 30 秒——不把正文截在连接时限。
- **错误文案不含代理 URL/认证信息**；测试夹具经同一代理解析进入本地回环，绝不触真实网络。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：注册表 SOCKS 解析、代理优先级、环境变量回退、`NO_PROXY`、https-only、错误文案不泄露代理信息。
- 测试夹具只在 `cfg(test)` 编译，经同一代理解析进入本地回环。

## 操作

- `update_check` 与 `update_download` 共用 `agent`；新增下载路径必须复用它，不得另建客户端。
