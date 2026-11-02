# LocalControlServer

> 仅监听 loopback 的本机控制面；OS 单实例锁、GPUI 前台与凭据门面分别归各自所有者。

## 决策与红线

- **首包认证**：连接首条请求必须是 `runtime.hello`，版本、client_id、凭据和 handover 作用域全部有效才开放 `instance.handover`；认证失败直接断开，不返回协议预言机，不记录请求行或凭据。
- **有界传输**：请求上限 128 KiB，握手读取限时 5 秒，后续读取限时 600 秒；单连接顺序处理，连接上限 64。超限或无效请求不得进入应用回调。
- **线程所有权**：accept 与连接 worker 均有显式句柄和 shutdown 路径；关闭拒绝新请求，唤醒 accept、断开活跃连接并 join，不留孤儿线程。不得在 GPUI 前台执行监听、读取、写入或 join。
- **凭据写入所有权**：仅已持 OS 单实例锁的首实例可加载或创建 Runtime/handover 凭据，客户端只读加载，缺失返回 NotFound，绝不生成或写入。原因：以单写入者避免首次创建竞态，不另设互斥锁。边界：空值、长度或编码非法的持久凭据明确失败，不自动覆盖或轮换。
- **能力边界**：当前只实现 hello 与 handover，不添加 Runtime 占位方法。路径授权、项目选择和窗口行为由后续 AppRoot 接线复核，不在传输层扩权。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests local_control -- --test-threads=1`。
- 使用临时 loopback 端点与 `sk-test` 假凭据，覆盖有效握手、Focus 接受、错误凭据、首包越序、超限请求与活跃连接 shutdown；断言真实线程退出，不访问真实用户数据根。
