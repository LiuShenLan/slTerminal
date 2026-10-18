# 原生通知平台面

## 决策与红线

- **漏斗策略不在本模块**:前台门控、全局/pane 节流、失败 episode、
  160 字符截断、文案全部在 `crate::notify`。本模块只做 AUMID 注册、
  WinRT 投递、toast 激活回投与任务栏闪烁。
- **AUMID 单点**:值 = `brand::WINDOWS_APP_ID`,只写
  `HKCU\Software\Classes\AppUserModelId\<AUMID>`(免 COM/快捷方式/安装器/
  管理员);幂等重写即自愈,进程内缓存失败下次投递重试。
- **线程模型**:`Show` 是跨进程 RPC,只在常驻 MTA worker 上跑;队列有界
  32(满即丢)、活通知保留 64(不足会提前释放 Activated 回调)。
- **COM 生命周期红线**:WinRT 对象必须先于 `CoUninitialize` 释放(反序
  真机实测 0xc0000005);worker 收尾与 `show_sync` 同序。
- **时长映射**:`notification_duration` 在组装 xml 的 worker 处读取;
  Default 不写 `duration` 属性,5/10s → `short`,30/90s/persistent → `long`
  (WinRT toast 只有 short/long,无常驻档)。
- **测试隔离**:`cfg(test)` 构建短路真实投递(`show`/`dispatch`),系统面
  由 `.claude/test-exemptions.md` 登记、`notify-test` 诊断命令与人工 smoke
  兜底。
- **图标待接**:IconUri 物化所需 PNG 资产归发布面;当前只在数据
  目录/exe 旁发现 `toast_icon.png`/`slterm.png` 时写入，无资产则无 logo。

## 步骤接缝

- 壳启动 `prepare()` 预注册;`crate::notify::deliver_gpui*` 是唯一漏斗
  投递入口;`slterm notify-test` 是同步诊断命令(注册→时长→投递逐段 OK)。
- 动作按钮回传 `SltermShellEvent::NotificationChoice`;确认语义归
  安全半程。toast 点击回窗经 `NotificationFocus`(`windowing` facade 接管四路)。
