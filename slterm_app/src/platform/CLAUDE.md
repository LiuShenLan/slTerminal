# 平台适配

> application 侧的唯一平台边界：Windows 集成端口与各平台封装模块。

## 决策与红线

- **WindowsIntegration 端口**：application 只经 `windows_integration::WindowsIntegration` 访问托盘、登录启动，以及后续材质、窗口几何、系统通知与任务栏；业务模块不得直接调用 Win32 或读取注册表。原因：平台能力必须可注入替身，测试才能覆盖外部启动项变化与托盘不可用。边界：新增 Windows 能力先加端口方法、真实实现与假实现，再接入业务服务；托盘线程装配（初始化回调与收尾）仍归壳启动链，不属业务访问。
- **静默驻留依赖托盘可用**：静默启动 = 登录自启标记 `--autostart` ∧ 设置两条件 ∧ 端口托盘可用；手工启动不携带该标记，启动路径先把托盘设置交给端口，再判定可用性。原因：隐藏窗口没有恢复入口，托盘不可用时必须回退为可见窗口。边界：手动再次启动仍走单实例移交；产品不提供关闭到托盘。
- **登录启动对账**：设置值只表达意图，`launch_at_login` 实际状态以 Startup 快捷方式内容为准；外部改指向、删除或读取失败都不得伪造启用。设置页显示对账说明，读失败显示「实际状态未知」。原因：外部修改后设置值不再代表 Windows 启动项。边界：写入失败只记录脱敏诊断，不改实际状态；真实目录读写由构建产物核验。
- **窗口材质能力**：`material::blur_support_for` 按系统构建号把材质档位折算为原生、经典模糊回退或无材质；设置页对不可用档位不提供点击入口，对回退档位显示说明，不改主题 token、不宣称静默生效。原因：系统 backdrop 只在 22H2+ 原生存在，能力必须显式呈现。边界：构建号获取集中在 `material::windows_build_number`（缓存 RtlGetVersion），壁纸与设置页共用同一判定。
- **任务栏进度端口**：通知与终端进度只经 `WindowsIntegration::set_taskbar_progress` 投递，真实实现转 `taskbar::apply`（COM 失败静默），测试替身记录投递序列。原因：任务栏是进程级外部状态，application 不得直接调用 ITaskbarList3。边界：无原生 HWND 时静默跳过；优先级与去重归 `taskbar::TaskbarState`，端口只接收仲裁后的形态。
- **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在本目录的封装模块；真实桌面行为按测试豁免人工核验。

## 验收与测试

- 端口与对账：`cargo test -p slterm --test lib_tests --features gpui-test-support platform::windows_integration -- --test-threads=1`。
- 假端口覆盖托盘不可用、启动项缺失／其他目标／不可读与写失败；真实托盘线程与快捷方式写入由构建产物 smoke 覆盖。
- 设置页对账与写入接线：`app_root::tests::settings_overlay`。

## 操作

- 新增平台能力：端口方法 → `SystemWindowsIntegration` → `FakeWindowsIntegration` → 业务消费点。
