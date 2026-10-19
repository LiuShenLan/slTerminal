# 原生通知平台面

> AUMID 注册、WinRT toast 投递、激活回投与任务栏闪烁；漏斗策略在 `crate::notify`。

## 决策与红线

- **漏斗策略不在本模块**：前台门控、全局/pane 节流、失败 episode、160 字符截断、文案全部在 `crate::notify`。本模块只做 AUMID 注册、WinRT 投递、toast 激活回投与任务栏闪烁。
- **AUMID 单点**：值 = `brand::WINDOWS_APP_ID`，只写 `HKCU\Software\Classes\AppUserModelId\<AUMID>`（免 COM/快捷方式/安装器/管理员）；幂等重写即自愈，进程内缓存失败下次投递重试。
- **线程模型**：`Show` 是跨进程 RPC，只在常驻 MTA worker 上跑；队列有界 32（满即丢）、活通知保留 64（不足会提前释放 Activated 回调）。
- **COM 生命周期红线**：WinRT 对象必须先于 `CoUninitialize` 释放（反序真机实测 0xc0000005）；worker 收尾与 `show_sync` 同序。
- **时长映射**：`notification_duration` 在组装 xml 的 worker 处读取；Default 不写 `duration` 属性，5/10 秒 → `short`，30/90 秒/persistent → `long`（WinRT toast 只有 short/long，无常驻档）。
- **测试隔离**：`cfg(test)` 构建短路真实投递（`show`/`dispatch`），系统面由 `.claude/docs/testing/exemptions.md` 登记、`notify-test` 诊断命令与人工 smoke 兜底。
- **图标资产**：IconUri 物化所需 PNG 资产归发布面；当前只在数据目录/exe 旁发现 `toast_icon.png`/`slterm.png` 时写入，无资产则无 logo。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 漏斗策略单元层全量：前台门控、闪烁先行、全局与 pane 节流、失败 episode、截断、时长 XML 映射（`notification_tests`）。
- 真实系统投递、AUMID 生效与视觉呈现由构建产物人工 smoke 兜底，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 启动：`prepare()` 预注册 AUMID。
- 投递：`crate::notify::deliver_gpui*` 是唯一漏斗投递入口。
- 诊断：`slterm notify-test` 同步诊断命令（注册 → 时长 → 投递逐段 OK）。
- 动作按钮回传 `SltermShellEvent::NotificationChoice`；toast 点击回窗经 `NotificationFocus`（`windowing` facade 接管回窗路径）。
