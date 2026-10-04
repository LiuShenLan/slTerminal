# 分片 09：系统集成（托盘 / 通知 / 任务栏进度 / 自动更新 / 开机启动 / 窗口特效 / DPI）

## 优化面

slTerminal 的五个系统集成面全部空白，现状是：

- **通知**：`src/features/notifications/useAgentNotifications.ts` + `src/ipc/notification.ts` 走 Tauri plugin-notification 的 `sendNotification`——未打包 Win32 WebView2 无 AUMID，banner 被系统抑制、无点击路由、无 close shim，实测只剩通知中心条目；`flashTaskbar`（`requestUserAttention(Critical)`）是唯一回窗引导。信号源只有 hooks 信号文件一链，BEL/OSC 9/OSC 133;D 全部不进通知。事件类别判定委托 CLI profile（`classifyEvent`），去重靠 `seenRef`，无节流、无失败冷却。
- **任务栏进度**：无 OSC 9;4 消费（xterm.js 侧无任何 handler，任务栏恒无进度）。
- **托盘 / 开机启动 / 自动更新**：全仓零痕迹。分发靠 `.claude/package.ps1` 手动 zip。
- **窗口特效**：`tauri.conf.json` 纯色背景（`backgroundColor: "#0a0a0b"`）+ `decorations: false` 自绘标题栏，无 Mica/透明度/壁纸。
- **命名注意**：`src-tauri/src/notify/` 是文件系统监听模块（watcher 池 + pause/resume），与通知中心无关，不在本片问题域。

pebrel 侧这五个子系统共享同一套骨架：**单一漏斗（一个枚举收全部信号源）+ 自有线程/消息泵（绝不阻塞 UI 线程）+ 注册表/文件自愈（幂等重写、坏态下次启动自修）+ 全程 best-effort（任何失败降级为日志行，永远不弹错、不拖垮事件循环）**。全局决策已定为托盘、AUMID toast、OSC 9;4 进度、自动更新、单实例移交交界、剪贴板 OSC 52 交界、窗口特效、开机启动照抄；Quick terminal、mux 驻留、SSH、多平台、Lua 砍；`nebula_*` → `slterm_*` 改名。

**交界登记**（本片只消费，机制归对应分片）：

- OSC 52 剪贴板：机制归分片 02，本片零涉及。
- BEL / OSC 9 文本 / OSC 133;C;D / OSC 9;4 解析：归分片 02（`nebula_terminal/src/osc_cwd.rs` 事件族上抛），本片只接消费事件。
- ai_hook 生命周期（TurnDone / NeedsAttention 带上下文）：归分片 03，本片漏斗的 `AiTurn`/`AiTurnIssue` 来源。
- 单实例移交（二次启动 ATTACH/tab.new）：机制归分片 04；本片托盘点击「显示/退出」与移交唤醒共用同一 FocusWindow 语义。
- 托盘 agent 清单 / 通知来源标注的 CLI 名单：单点在 CLI profile 注册表（分片 03），对应 pebrel 的 `ai_agents::AgentKind` 角色。
- 设置键（tray / silent_start / auto_check_updates / visual_* / launch_at_login）：归分片 06。

**slTerminal 侧现状归宿**：`src/features/notifications/` 前端通知层整体消亡，「类别判定委托 profile」的知识以 Rust 漏斗的 source classification 重生（不再按事件文本猜来源）；`src-tauri/src/notify/`（fs watcher）原样保留，GPUI 单进程世界继续服务编辑器/文件树，与通知零耦合；`titleBar/` 由 GPUI 壳重建窗口三钮/拖拽（归分片 05），本片只管闪烁与特效交互；`sideViews/` 归分片 05 壳布局，托盘右键 agent 直达清单的数据源是 runtime snapshot（分片 04），不是侧栏；`src/features/backgroundTasks/` 双端调度器在单进程下收编为 Rust 任务注册表 + GPUI executor，本片所有节拍（托盘 1Hz、更新检查延迟线程）一律挂既有节拍/后台执行器，零新增常驻定时器。

## 采纳点

### 托盘

1. **独立托盘线程 + 永不显示的隐藏 Win32 窗口 + 自有消息泵，照抄。** `Shell_NotifyIconW` 回调必须挂窗口，且不能挂在壳窗口上（壳不背 OS 消息泵职责）。pebrel 侧：`nebula_app/src/tray.rs` 的 `win::tray_thread`、`win::tray_wnd_proc`、消息常量 `WM_APP_TRAY`/`WM_APP_REFRESH`/`WM_APP_SET`/`WM_APP_SHUTDOWN`/`WM_APP_ICON`。
2. **app→托盘单向 `PostMessageW` 摇醒 + `Mutex<Shared>` 状态快照，照抄。** 两方向不共享锁跨越阻塞调用；HWND 用 `AtomicIsize` 发布。pebrel 侧：`tray.rs` 的 `win::post`、`win::STATE`、`win::HWND`、`win::update`（内容相等直接返回，1Hz 调用方天然去抖）。
3. **agent attention 橙点双图标 + 预乘 BGRA HICON 像素管线，照抄。** DWM 按 premultiplied 合成托盘图标，直 alpha 在深色任务栏泛白边；橙点选警示橙而非主题 accent（托盘图标不知道主题）。pebrel 侧：`tray.rs` 的 `win::build_icons`、`win::draw_attention_dot`、`win::create_icon`（负高自顶向下 DIB）。
4. **右键菜单列全部 agent pane 点击直达 + 左键 `focus_best` 兜底（先等输入的、再任意 agent、最后裸窗口），照抄。** 菜单构建与模态展示分离，HMENU 可不开线程回归测试；菜单期间用快照，命令 id 映射回打开菜单那一刻的清单。pebrel 侧：`tray.rs` 的 `win::build_menu`、`win::show_menu`、`win::focus_best`、`win::send_focus`、`MENU_AGENT_BASE`/`MENU_SHOW`/`MENU_QUIT`。
5. **状态推送挂既有 1Hz chrome 时钟，照抄。** 托盘是环境信息面，秒级延迟无感，换零新增定时器。pebrel 侧：`nebula_app/src/event.rs` 的托盘 1Hz 调用点、`crate::tray::update`；清单投影 `nebula_app/src/window_context/agents.rs` 的 `tray_agents`。
6. **点击命令走 `GpuiTrayCommand` 回调枚举（Focus(pane)/Quit），照抄。** GPUI 主窗没有 winit `EventLoopProxy`，托盘反向只发命令回调。pebrel 侧：`tray.rs` 的 `GpuiTrayCommand`、`win::init_gpui`、`win::GPUI_COMMAND`。
7. **关图标留线程（开/关是高频往复，不值得反复建窗），参考。** pebrel 侧：`tray.rs` 的 `win::set_enabled`、`win::apply_enabled`（NIM_ADD 失败留 `added=false` 下次重试）。
8. **进程收尾 WM_APP_SHUTDOWN 删图标 + 退消息泵（不删图标要等用户悬停才消失），照抄。** pebrel 侧：`tray.rs` 的 `win::shutdown`、`win::remove_icon`；两壳收尾调用点 `nebula_app/src/gpui_shell/mod.rs`。

### 通知

9. **通知中心单一漏斗 `Notification` 枚举，照抄。** 「新来源 = 新变体、新输出 = deliver 里新一行」的加法扩展合同，AI-CLI 特有钩子落地不重接骨架。pebrel 侧：`nebula_app/src/notify.rs` 的 `Notification`（`Bell`/`CommandDone`/`CommandFailed`/`Text`/`AiTurn`/`AiTurnIssue`）、`Notification::command_finished`、`COMMAND_NOTIFY_MIN`。
10. **AUMID 经 `HKCU\Software\Classes\AppUserModelId\<AUMID>` 注册，照抄（身份换成 slTerminal 自有 AUMID）。** 免 COM、免快捷方式、免安装器、免管理员，幂等重写即自愈。pebrel 侧：`nebula_app/src/platform/notifications.rs` 的 `win::register_aumid`、`win::ensure_aumid`、`win::set_reg_sz`、`win::ensure_icon_file`。
11. **toast 常驻 MTA worker 线程 + 有界队列 + 通知对象保留 64 防 Activated 回调 premature 释放，照抄。** Show RPC 是跨进程调用（几十 ms），永不在事件循环上跑。pebrel 侧：`nebula_app/src/platform/notifications/windows.rs` 的 `enqueue`、`run`、`show`、`xml`。
12. **投递纪律：任务栏闪烁恒发先行（幂等、静默、shell 自动合并），toast 被节流只挡系统 toast 不挡闪烁，照抄。** pebrel 侧：`notify.rs` 的 `deliver`、`deliver_gpui`（`set_urgent(true)` 在节流判定之前）。
13. **失焦门控（聚焦中不打扰，视觉 bell 覆盖该场景），照抄。** pebrel 侧：`notify.rs` 的 `deliver` 调用约定。
14. **双层节流：全局 3s toast 间隔 + pane 级失败冷却 30s（恢复事件结束旧 episode、同错冷却、新错独立、跨 pane 独立），照抄。** pebrel 侧：`notify.rs` 的 `TOAST_THROTTLE`、`FAILURE_COOLDOWN`、`PaneFailureThrottle`、`PaneNotificationThrottle`。
15. **toast 正文单行长预览上限 160 字符（超长出省略号、空白折叠、幂等截断），照抄。** 通知是瞥读层不是阅读层，长回答不能撑破卡片遮住关闭钮。pebrel 侧：`notify.rs` 的 `TOAST_BODY_MAX_CHARS`、`clamp_toast_body`、`toast_text`。
16. **toast 点击激活回焦来源 pane（复用 toast/托盘同一条 FocusWindow 路径：还原最小化、选中 tab、聚焦 pane），照抄；权限确认的 toast 动作按钮直达选择，照抄。** pebrel 侧：`notify.rs` 的 `ToastActivation`、`gpui_activation`、`application_activation`、`init_gpui_activation`、`deliver_gpui_with_choices`。
17. **零用户配置靠 CLI 自带信号（BEL / OSC 9 文本 / OSC 133;D），照抄。** 不向用户索要 hook 脚本编辑；CLI 回合完成自会响铃。pebrel 侧：`notify.rs` 模块文档与 `Notification::Bell`。
18. **`notify-test` 同步诊断命令（注册 + 发 toast 全链路逐段打印，供排障），参考。** pebrel 侧：`nebula_app/src/platform/notifications.rs` 的 `notify_test`。

### 任务栏进度

19. **`TaskProgress` 五态模型 + ConEmu 状态码宽容映射（0 清除 / 1 正常 / 2 错误 / 3 不确定 / 4 暂停，规范外码含实测的 `9;4;5;0` 一律归清除），照抄。** 把未知码当非法丢掉等于让进度条永远停在最后一个状态。pebrel 侧：`nebula_app/src/taskbar.rs` 的 `TaskProgress`、`from_osc`、`is_active`、`taskbar_flag`、`percent`。
20. **`ITaskbarList3` 每次调用自管 COM 初始化（`RPC_E_CHANGED_MODE` 容忍、`HrInit` 先调）、失败一律静默，照抄。** 任务栏是纯装饰，拿不到不该影响终端。pebrel 侧：`taskbar.rs` 的 `apply`。
21. **解析层不收窄语义、消费层宽容映射的分层契约，照抄**（与分片 02 `OscEvent::Progress` 的交界：原始 state 原样上抛）。pebrel 侧：`nebula_terminal/src/osc_cwd.rs` 的 `OscEvent::Progress` 注释契约。
22. **进度事件→窗口 HWND 路由 + agent 退出即清进度，参考。** pebrel 侧：`nebula_app/src/gpui_shell/workspace/terminal_activity.rs` 的 `taskbar::apply` 调用点；`nebula_app/src/window_context/agent_activity.rs` 的清除点。

### 自动更新

23. **启动检查全程 best-effort、断网/坏 JSON/限流全落 debug 日志不出横幅，照抄。** 线程延迟 12s 等首窗与首个会话安顿后再联网。pebrel 侧：`nebula_app/src/update_check.rs` 的 `spawn_gpui_once`。
24. **提示状态独立 `update_state.json`、按版本生效（prompted / remind-later 3 天 / skip 单版本互不串味），照抄。** 后台版本检查不得改写用户设置正文。pebrel 侧：`update_check.rs` 的 `UpdatePromptState`、`should_prompt`、`mark_prompted`、`remind_later`、`skip_version`、`update_state_path`、`UPDATE_STATE_LOCK` + 文件锁双闸。
25. **手写数字版本比较（点分段数值、段内后缀忽略、拒绝降级），照抄。** pebrel 侧：`update_check.rs` 的 `is_newer`、`can_install_version`、`version_is_installable`。
26. **release 资产精确匹配合同（名称/架构/版本/官方 URL 前缀四重校验）+ digest 字段优先、release body 内嵌 checksum 回退，照抄。** 下载器不信任 UI 数据，二次收紧。pebrel 侧：`update_check.rs` 的 `parse_latest_release`、`windows_x64_installer_names`、`select_windows_x64_installer`、`checksum_from_release_body`、`normalize_sha256`；`nebula_app/src/update_check/assets.rs` 的 `select`、`native_names`。
27. **ureq + 自解析代理解（Win 注册表 `ProxyServer` 按协议取值、socks 正确归 socks5、env 回退、`NO_PROXY`、https_only、双超时），照抄。** 不交给 ureq 的 `win-system-proxy`（它会把 socks 拼成 http）。pebrel 侧：`nebula_app/src/update_proxy.rs` 的 `resolve`、`agent`、`raw_system_proxy`、`parse_windows_proxy_server`。
28. **下载 `.part` 流式 + 长度/MZ 头/SHA-256/512MiB 上限全部通过才原子替换，照抄。** 中断下载或错误响应不能变成可执行文件。pebrel 侧：`nebula_app/src/update_download.rs` 的 `download_and_verify`、`download_with_job`、`verify_download`、`verify_file`、`validate_asset_contract`、`download_paths`。
29. **下载会话 generation 作废链（取消/换版本后旧任务的进度与完成写入一律无效），照抄。** pebrel 侧：`update_download.rs` 的 `DownloadJob`、`begin`、`cancel`、`run`、`set_progress`。
30. **缓存与会话双证据 hydrate（启动无网也能恢复失败态/就绪态）+ 安装失败「未见过」重提示判定（result.json mtime 晚于提示状态 mtime），照抄。** pebrel 侧：`update_download.rs` 的 `hydrate`、`cached_asset`、`installation_failure_unseen`；`nebula_app/src/update_download/cache.rs`。
31. **两阶段 handoff 事务目录文件态机（plan.json → ready.json → commit.json 授权安装 / cancel.json + 杀 helper）+ restore ticket 按工作区快照恢复 + 最多 3 次尝试 + 成功后留档，照抄。** 文件是持久证据，崩溃后状态可重建。pebrel 侧：`nebula_app/src/update_download/handoff.rs` 的 `Plan`、`prepare`、`PreparedUpdate::commit`、`Drop`、`restore_ticket`、`acknowledge_restore`、`failed_update`、`failure_unseen`、`schedule`、`apply_scheduled`。
32. **`handoff.ps1` 独立安装 helper（子进程跑版本验证、工作区快照重启、env 恢复变量）+ 安装目录写探测前置 + 旁置守卫锁（同二进制全配置共享，安装器不动它），照抄。** helper 无 commit.json 无安装权威。pebrel 侧：`nebula_app/src/update_download/handoff.ps1`；`nebula_app/src/platform/update_installation.rs` 的 `spawn_prepared_helper`、`installation_in_progress`、`guard_base`。env 名 `PEBREL_UPDATE_RESTORE`/`PEBREL_CONFIG_DIR`/`NEBULA_CONFIG_DIR` 落地改 `SLTERM_*` 单名。
33. **外部托管分发渠道直接退出自动更新（externally_managed 闸门），参考。** slTerminal 初期即 zip 便携形态，该闸门天然放行；为将来安装器/商店形态留位。pebrel 侧：`update_check.rs` 的 distribution 检查；`nebula_app/src/platform/distribution.rs`。
34. **本地 `update-test-source` 演练通道（注入假 release 服务器，永不回落公网），参考。** 发布流程的彩排与排障位。pebrel 侧：`update_check.rs` 的 `test_source` 模块。

### 开机启动

35. **Windows 登录启动 = Startup 已知文件夹 `.lnk`（COM `IShellLinkW` 写入、参数静默启动、工作目录 home），照抄。** 免注册表、免管理员、幂等重写；设置页与安装器管理同一快捷方式。pebrel 侧：`nebula_app/src/platform/startup.rs` 的 `set_launch_at_login`、`startup_shortcut`、`launch_at_login`。
36. **静默启动 = `silent_start && tray && hide_window_on_close` 三条件合取，照抄。** 无托盘的静默启动会把应用藏成不可达进程。pebrel 侧：`startup.rs` 的 `start_hidden`；`nebula_app/src/platform/capabilities.rs` 的 `Capabilities::hide_window_on_close`。

### 窗口特效

37. **模糊一律走 GPUI `WindowBackgroundAppearance` 平台通道（Mica/MicaAlt 走原生枚举、22H2 门控、低档 Aero/Acrylic 走 `Blurred` AccentPolicy 通道、切换档显式清理另一通道），照抄。** 壳不自调 DWM backdrop API。pebrel 侧：`nebula_app/src/gpui_shell/wallpaper.rs` 的 `background_appearance`、`initial_background_appearance`、`effective_material`。
38. **窗口透明度只透壳底色与终端默认背景、文字与彩色单元背景保持不透明，照抄。** 全窗 alpha 会让文字对比度塌掉。pebrel 侧：`wallpaper.rs` 的 `VisualEffects`、`refresh`、`refresh_surface_opacity`、`window_opacity`、`chrome_surface_opacity`。
39. **壁纸 = 底色之上、单元格之下的一层图 + fit/alignment/cover_chrome/图自身独立透明度（独立于窗口 opacity）+ 设置五键，照抄。** pebrel 侧：`wallpaper.rs` 的 `Wallpaper`、`update_wallpaper`；`nebula_app/src/renderer/image` 的 `BackgroundImageFit`、`BackgroundImageAlignment`、`wallpaper_rect`。
40. **壁纸后台串行解码缓存（单 job 串行、generation 过期即弃、按 (路径, 文件戳) 缓存、绘制只复用一张纹理、文件 64MiB/解码 128MiB/边长 2048 上限），照抄。** UI 线程零文件 I/O。pebrel 侧：`nebula_app/src/gpui_shell/wallpaper/image_loader.rs` 的 `load`、`load_preview`、`Request::cancelled`、`FileStamp`、`LoadedImage`。
41. **live 调透明度即时预览 + 设置页预览图小尺寸分支，参考。** pebrel 侧：`wallpaper.rs` 的 `set_opacity_live`；`nebula_app/src/gpui_shell/wallpaper/preview.rs`。

### 显示器 / DPI

42. **首窗创建前主显示器 DPI 查询（`MonitorFromPoint` + `GetDpiForMonitor`，供初始网格尺寸一步到位），参考。** pebrel 侧：`nebula_app/src/platform/startup.rs` 的 `primary_display_scale`。
43. **启动几何推导（按基准字号量网格而非持久化缩放、主显 95% 上限、最低尺寸地板、同设备像素 round-trip 亚像素容忍不二次 SetWindowPos），参考。** pebrel 侧：`nebula_app/src/gpui_shell/workspace/windowing/startup_geometry.rs` 的 `preferred_size`、`prepare_initial_grid`、`default_size`、`fit_preflight_size`、`same_device_size`。

### shell 集成（交界登记）

44. **Windows 侧自家 OSC 133 上报链归分片 02**（`nebula_app/src/platform/shell_integration.rs` 的 Windows 分支是空操作，Unix 才干活）；本片不重复登记。
45. **通知/进度消费与 shell 集成的契约：OSC 133;D 退出码进 `CommandDone`/`CommandFailed`、133;C 建 running 边沿，照抄语义**（与分片 02/04 的交界——`Notification::command_finished` 的 `hook_seen` 参数决定「hook 已确认的短成功命令不再通知」）。

## 不采纳点

1. **Unix shell integration 整支（zsh ZDOTDIR 劫持 / bash rcfile 注入 / fish 支持判定），不采纳（已定多平台砍）。** pebrel 侧：`nebula_app/src/platform/shell_integration.rs` 的 `prepare_unix`、`supports` 及 `res/shell/` 脚本族——slTerminal 只玩 Windows，shell 集成只有 pwsh 自家 OSC 133 一链。
2. **`shell_detect.rs` 的 WSL 发行版探测链（`wsl:` 前缀 id / Tux 图标 / 按来宾 passwd 起默认 shell），不采纳。** WSL pane 不在产品面；pwsh → powershell → cmd 回退链 slTerminal 已有 `src-tauri/src/pty/shell.rs` 对应物，GPUI 世界的 shell 发现归分片 02 裁定。pebrel 侧：`nebula_app/src/shell_detect.rs` 的 WSL 分支。
3. **winit third_party mixed-DPI backport patch，不采纳。** 那是 legacy 壳 fork 窗口库的补丁（Win10 旧行为 workaround + 键盘事件 backport），GPUI 平台层自带 DPI 处理，不引入、不 fork 任何窗口库；跨显示器移动的重定位行为以 GPUI 实测为准。pebrel 侧：`third_party/winit-0.30.13/README.nebula.md` 记录的两个 backport。
4. **`window_transition.rs` WinEvent hook（`EVENT_SYSTEM_MOVESIZESTART/END` 协调渲染暂停），不采纳。** 旧壳渲染器在模态移动期的暂停件，GPUI 壳自绘自调度无此需求。pebrel 侧：`nebula_app/src/window_transition.rs` 的 `NativeWindowStageTracker`。
5. **`display/animations.rs` 旧渲染器帧快照动画状态，不采纳。** 渲染器私有的动画存储形态随旧壳消亡。pebrel 侧：`nebula_app/src/display/animations.rs`。
6. **`motion.rs` 的 `Spring`/`Tween`/`Easing`/`MotionPolicy` 动画数学，不搬代码、语义参考。** GPUI 内建动画为一选；若壳内自定义动效（开关/面板展开）需要补间语义，以其 `MotionPolicy`（Full/Reduced 动效降级政策）为参照。pebrel 侧：`nebula_app/src/motion.rs`。
7. **`notify.rs` 的 legacy-shell 投递分支（winit `EventLoopProxy`/`PROXY`/按窗口 `deliver`），不采纳。** 单壳只留 `deliver_gpui` 形态；失焦判定改问 GPUI 窗口焦点态。pebrel 侧：`notify.rs` 的 `PROXY`、`init_proxy`、`deliver(window,...)`。
8. **`tray.rs` 的 legacy-shell `init(EventLoopProxy)` 分支，不采纳。** 单壳只留 `init_gpui` 回调。pebrel 侧：`tray.rs` 的 `init`、`PROXY`。
9. **Linux/macOS toast 通道（notify_rust / NSBundle identifier / wait_for_response），不采纳（已定多平台砍）。** pebrel 侧：`nebula_app/src/platform/notifications.rs` 的非 Windows `show`/`toast_actionable`。
10. **自动更新的 macOS 资产面（dmg 命名族 / `koly` trailer 校验），不采纳（已定多平台砍）。** pebrel 侧：`update_check/assets.rs` 的 `macos_names`；`update_download.rs` 的 `verify_package_trailer` dmg 分支。
11. **legacy 品牌资产名兼容（`NebulaTerminal-*-setup.exe` 命名回退、双官方下载前缀），不采纳。** 新应用无历史资产，资产名单一 `slTerminal-*` 合同。pebrel 侧：`update_check.rs` 的 `windows_x64_installer_names` 第三元；`update_download.rs` 的 `LEGACY_RELEASE_DOWNLOAD_PREFIX`。
12. **应用图标多变体选择面（`AppIconName` 变体族驱动托盘/toast/任务栏图标切换），不采纳。** slTerminal 单一图标，注册/物化逻辑照抄但变体维度塌缩为单态。pebrel 侧：`nebula_app/src/app_icon.rs` 的 `selected`；`platform/notifications.rs` 的 `win::REGISTERED` 变体缓存键。
13. **应用内 toast 通知面（壳内悬浮通知卡 + 通知历史）与通知设置页 UI，不采纳为本片内容。** 系统通道漏斗归本片；壳内通知面的有无与形态归分片 05，通知偏好设置页归分片 06。pebrel 侧：`nebula_app/src/gpui_shell/toast.rs`；`nebula_app/src/gpui_shell/workspace/notifications.rs`。
14. **托盘图标的多图标变体重建（`refresh_app_icon` 按变体换图标），不采纳。** 单图标无重建需求；物化/注册逻辑照抄（采纳点 3/10）。pebrel 侧：`tray.rs` 的 `refresh_app_icon`、`win::replace_app_icon`。
15. **`pebrel notify-test` 之外的其余 CLI 诊断子命令面，不采纳。** 只留通知链路一条诊断通道。pebrel 侧：`nebula_app/src/main.rs` 的 CLI 分派。

## 优化方向

系统集成层按「单一漏斗 + 自有线程 + 注册表自愈 + best-effort」的统一骨架落地：通知、托盘、进度三个面共享一个信号分类与策略内核（来源标注、失焦门控、节流、失败冷却都是一套），自动更新与开机启动共享「事务文件 + 幂等注册」的注册表纪律；任何系统 API 失败一律落日志不弹错，任何跨线程通信一律消息/命令回调，不共享锁跨越阻塞调用。

通知与任务栏进度是纯粹的消费端：源信号全部来自分片 02 的 OSC/BEI 解析族（BEL、`OscEvent::Notify`、`OscEvent::Progress`、`CommandDone{exit_code}`）与分片 03 的 hook 生命周期边（TurnDone/NeedsAttention），本片只做分类、策略与投递；前端 `useAgentNotifications` 消亡后，「类别判定委托 profile」以 Rust 侧 source classification 重生——CLI 身份判定单点在 profile 注册表，通知漏斗不认事件文本里的关键字。toast 点击、托盘点击、动作按钮三条回窗路径合并为同一条 FocusWindow 语义（还原最小化 + 选中 tab + 聚焦 pane），与分片 04 的单实例移交唤醒同源。

托盘成为 attention 的常驻面：右键 agent 清单与橙点态消费 runtime snapshot 的同一份 `RuntimeTaskState` 投影（分片 04），1Hz 节拍推送、内容相等不打扰；托盘「退出」是真退出通道，与关窗即退出的定位一致。任务栏进度宽容映射，规范外状态码归清除，进度随 agent 退出/窗口失配即清，不留残条。

自动更新从零建但一次建对：GitHub Releases 检查 → 资产合同四重校验 → `.part` 流式下载（MZ/SHA-256/上限全过才原子替换）→ 事务目录 handoff（commit 才授权安装）→ restore ticket 恢复工作区；提示状态独立文件按版本生效，与用户设置正文解耦；落盘根目录沿用 slTerminal 应用数据目录（便携语义）。该链与 `package.ps1` 手工分发并存，`externally_managed` 闸门为将来安装器形态留位，不留双轨升级通道。

窗口特效走 GPUI 原生通道不绕路：Mica/MicaAlt 只到平台枚举（22H2 门控，Win10 降级档 = 纯色 + 透明度），透明度只透底色、文字永不透明，壁纸后台解码有界、绘制单纹理；自绘标题栏与三钮由壳重建（分片 05），本片只保证闪烁/进度/特效的 Win32 交互面。DPI 与多显示器整包交给 GPUI 平台层，slTerminal 只保留「首窗创建前主屏 DPI 查询」一个 Win32 调用点与启动几何推导，不 fork 窗口库、不背 legacy 壳的 DPI workaround。

开机启动极简：Startup 文件夹 `.lnk` + 静默启动三条件合取；无托盘不静默，关窗即退出，不引驻留语义。

测试分布：任务栏状态映射、`UpdatePromptState` 状态机、版本比较、资产合同校验、下载 generation 作废、handoff 文件态机、restore ticket 尝试上限、托盘菜单构建（不开线程验 HMENU）、节流/冷却策略全为纯函数与文件系统隔离可测（L1）；真实 toast 投递、Mica 渲染、真实 `.lnk` 登记、真实 OSC 上抛归豁免清单登记。全链演练走 `update-test-source` 本地假 release 服务器，永不回落公网。
