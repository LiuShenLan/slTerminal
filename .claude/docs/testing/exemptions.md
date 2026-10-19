# 自动化测试豁免与条件跳过

> 不可自动化测试面的单表真值源；未登记豁免违反测试覆盖纪律。

## 决策与红线

- **登记先行**：改动的代码可自动化部分必须添加全量自动化测试；不可自动化部分必须先在本表登记根因与兜底层级，禁止未登记豁免。原因：豁免是覆盖缺口的显式债务，不是默认路径。边界：能自动化的部分不得以「难测」为由登记豁免。
- **判据**：只有真实不可自动化边界才可保留——操作系统/GPU/桌面会话/凭据库/安装器/真实网络/真实设备/物理输入，且兜底测试或人工验收仍存在。原因：区分真实边界与测试偷懒。边界：旧载体、已修复问题、重复纪律说明一律删除，不占豁免额度。
- **复审结论口径**：保留 = 边界成立且兜底有效；改写 = 边界成立但引用或描述需更新；上报 = 兜底只有人工目测或无法确认覆盖真实存在，等待裁决；删除 = 不再成立或不应登记。
- **来源锚点**：用归属模块路径或登记日期替代历史短标识；不复用过程编号。

## 既定豁免清单

| 项目 | 归属模块 | 豁免原因（根因） | 当前兜底层级 | 复审结论/登记锚点 |
|---|---|---|---|---|
| `EventLoop` 读/写 I/O 编排残余分支（输入通道投递失败、子进程退出事件、DA1/DSR 代答注入、跨块查询残片扣留、EOF 冲刷、读/写错误） | `slterm_terminal/src/event_loop.rs` | 依赖 Mutex/Channel/系统调用与真实管道，无法纯函数化；决策点已抽为纯函数 | 单元层 `event_loop` 纯函数全分支用例（含 EOF 残片冲刷）+ `pty_integration_tests` 真实 ConPTY 往返/DA1 身份/句柄回收 | 保留 |
| PTY 收尾的 `ClosePseudoConsole` 阻塞路径（`Pty::drop` → `TerminateProcess` → `conout.drain_detached` → backend close） | `slterm_terminal/src/tty/windows` | `ClosePseudoConsole` 等待 conhost 冲刷剩余输出，阻塞不可在单元层构造；收尾顺序错误会留下孤儿进程/线程 | `drain_detached` + `conpty_spawn_write_read_resize_close_full_chain`/`bundled_conpty_resources_return_after_close_and_failed_spawn` + 句柄计数断言 + 实机人工验收（杀会话后应用无挂起） | 保留 |
| ConPTY spawn 纯 Win32 组合（`UpdateProcThreadAttribute(PSEUDOCONSOLE)` → `CreateProcessW` → Job Object 指派） | `slterm_terminal/src/tty/windows` | Win32 API 组合，参数错误无单测定位价值；可纯化部分已抽出补测 | `pty_integration_tests`（全链/显式命令绕过白名单/捆绑资源归还）+ Windows runner | 保留 |
| `slterm_app::run()` 启动链胶水（CLI 解析/同 exe 分流、panic 钩、console 准备、日志初始化、`run_shell`） | `slterm_app/src/lib.rs` | 完整进程入口与真实窗口事件循环不可在单元层启动；子命令人格直接 `process::exit` | `windows_console_startup` 集成 target + `runtime_conformance`（真实 exe）+ UI `shell_window_mounts_root_and_draws`；各副作用由单元层用例锁死 | 保留 |
| `main.rs` 的 `fn main` 胶水 | `slterm_app/src/main.rs` | 结构性零覆盖：仅委托 `slterm::run()`，单元层无法启动完整 GUI 进程 | 同 `run()`；`windows_console_startup` + 构建产物启动实测 | 保留 |
| GPUI GPU 渲染与真实鼠标跟踪/滚轮视觉（真实合成器、物理滚轮 delta、全屏 TUI mouse tracking 上屏） | `slterm_app/src/gpui_terminal` | 虚拟窗口不跑真实 GPU 合成，物理设备事件不可注入；编码与滚动行为已可纯函数化 | 单元层 `mouse_protocol` 编码用例 + `wheel_scroll_lines`/`selection_scroll_lines` 用例 + `math_overlay_paint_leaves_grid_and_copy_source_untouched` 绘制冒烟 + 构建产物人工验收（全屏 TUI 滚轮、鼠标跟踪、选区与复制） | 保留 |
| 非默认 flags 矩阵（尤其 0x8）真实滚轮行为 | `slterm_terminal/src/tty/windows` | 自动化不可守卫：0x8 滚轮失效仅真实 claude 场景复现（最小复现实验失败、阻断条件仅 pwsh→claude 进程树 + kitty 协议），矩阵位翻转假阴性 | 默认矩阵零漂移由单元层 `conpty_flags_default_matrix_matches_legacy_tristate` 锁死；任何 0x8 启用/默认翻转须真实 claude 全屏 TUI 滚轮 + Win10 build 阈值核对人工实测，无实测记录禁合入 | 保留 |
| 真实 OS 级按键与修饰键 CHORD（Shift+Enter、Ctrl+Space 等） | `slterm_app/src/gpui_terminal/keymap` + `scripts/win32_input_matrix.ps1` | `PostMessage` 不改变真实键盘状态，CHORD 需前台 `SendInput` + 值守空闲机；虚拟窗口只收合成事件 | 脚本键矩阵逐字节比对（`win32_input_matrix.ps1` + 基线）覆盖普通键路径；CHORD 由构建产物人工验收（真实键盘组合） | 保留 |
| 会话容量上限拒绝与 bundled OpenConsole 提取/加载回退的残余 Win32 分支 | `slterm_terminal/src/tty/windows` | 清理段为 I/O + 平台 API 组合，不可纯函数化；上限判定已由用例锁死 | 单元层 `pty_capacity_*`/`conpty_prefers_complete_runtime_pair`/`conpty_does_not_mix_partial_runtime_with_legacy_pair`/`conpty_returns_none_without_complete_pair`/`ensure_extracted_writes_then_idempotent` + `pty_integration_tests`（`spawn_lock_serializes_and_capacity_32_enforced`/`bundled_conpty_resources_return_after_close_and_failed_spawn`） | 保留 |
| plan_balance 真实 provider API 出网（真实账号一轮） | `slterm_app/src/plan_balance` | 真实外部 API + 真实凭据依赖，测试进程不发出境请求 | 请求形态/响应解析/错误映射由本地 HTTP 服务单元层全覆盖（`query.rs` 的 `serve_once`，TcpListener 单请求，不出网；`kimi.rs`/`deepseek.rs` 请求头与解析用例）+ 编排/合并/推送单元层 + 构建产物人工实测（真实账号一轮） | 保留 |
| Win11/Win10 真实终端 conda 激活（profile 加载链路 + conda 钩子 + prompt 包装链） | `slterm_terminal/src/tty/windows/shell.rs` | 依赖真实 conda/miniforge 环境与交互会话，测试机无此环境 | 单元层 `pwsh_args_respect_user_profile` + 双系统 debug build 人工实测 | 保留 |
| 后台任务周期驱动循环（GPUI background executor 定时器、订阅生命周期、任务间节拍复用） | `slterm_app/src/background_tasks` | 真实计时器与 executor 唤醒不可在单元层按真实时间断言；循环本体属编排胶水 | 单元层 `background_task_tests`（注册表/订阅归零/重入闸门/失败续跑/配置热更新/Once/OnDemand）+ `startup_timing_tests` 虚拟时钟 + UI `screen_watchdog_identifies_agent_and_updates_status` + 人工观察（运行中改配置） | 保留 |
| fs-notify 事件溢出/丢失与重扫补漏链 | `slterm_app/src/side_panel` + 文件树 | notify 内部溢出与事件丢失无法从测试进程构造确定性复现；目录定时重扫已裁定不做 | 单元层 `fs_watch_tests`（树根/文件兴趣/共享父 watch/关闭停报）+ `side_panel/search` 的 `need_rescan` 脏标记 + UI `external_file_change_refreshes_the_gutter`/`external_file_creation_refreshes_the_file_tree` + 构建产物人工验收（外部改文件后树与 gutter 刷新） | 保留 |
| tab/分屏/侧栏分隔条真实拖拽手势（pointer down/move/up 命中 drop target 与 OS 级指针） | `slterm_app/src/app_shell/workspace` | 虚拟窗口可合成鼠标事件但无真实 OS 指针/hit-test 设备路径；真实手势手感不可自动化 | 单元层 `tab_drag_tests`（外缘/象限/方向/阈值/预览几何）+ `split_drag_tests`（几何/压挤标记）+ UI `tab_drag_to_the_content_edge_docks_the_source_at_the_root`/`tab_drag_into_a_pane_quadrant_splits_only_that_pane`/`tab_drag_rolls_back_when_the_target_pane_disappears`/`dragging_divider_into_close_zone_closes_the_squeezed_pane`/`sidebar_split_divider_drags_the_slot_ratio` + 实机验收点（拖拽手感 + 终端 TUI 保活） | 保留 |
| Win10 捆绑 conhost（OpenConsole 1.24）握手差异（DA1 多发 + DSR 不发）与键事件输入（CPR 应答 = F3 毒键）的端到端行为 | `slterm_terminal/src/tty/windows` | 本机 Win11 inbox conhost 握手形态与输入解析均不同——差异只在 Win10 路径触发，自动化不可达 | 单元层纯函数全分支（`strip_conpty_startup`/`strip_da1_queries`/`strip_dsr_queries`/`split_trailing_query_prefix`/`apply_output_strip` 门控两值/`mirror_da1_query`/`mirror_dsr_query`/`should_answer_dsr`/`inject_da1_response`/`inject_cpr_response`；门控 `conhost_input_corrupts_cpr` 边界锁定）+ `pty_integration_tests` DA1 身份/句柄回收 + Win10 实机人工验收点（新建终端无蜂鸣无 `[?1;2c` / 敲键不丢首字符 / 双击恢复无前缀污染 / claude 功能正常） | 保留 |
| 回收站真实落桶（`SHFileOperationW` 实际移入系统回收站） | `slterm_app/src/platform/file_operations.rs` | 真实调用会污染开发机/测试机用户回收站，且无法从测试进程断言系统落桶位置；不可重复 | `file_operations_tests` 锁死 `FOF_ALLOWUNDO`/静默位、双 NUL 编码、错误码与取消映射、缺失路径拒绝 + UI `context_menu_delete_routes_through_the_recycle_bin` + 构建产物人工验收（删文件后回收站可见） | 保留 |
| gutter 行标与编辑器内部滚动的像素级对齐 | `slterm_app/src/app_shell/file_editor` | 行标同屏位置依赖组件库内部布局，虚拟窗口只能断言元素存在与矩形非零，不能断言与文本行像素级重合 | 单元层 `gutter_tests` 锁死状态→行标映射与 revision 推进；UI `gutter_click_opens_a_read_only_dual_pane_diff` 断言行标绘制与点击打开 diff；实际对齐由构建产物人工目测 | 保留 |
| 双栏 diff 的像素级行对齐与同步滚动视觉 | `slterm_app/src/app_shell/file_editor` | 两栏独立 `InputState` 的滚动由组件库布局决定，hunk 导航只保证滚动偏移写入，像素级行对位不可在虚拟窗口断言 | 单元层 `diff_tab_tests` 锁死 hunk 计算/只读合同；UI `diff_hunk_navigation_moves_the_active_hunk` 断言 hunk 集与活动下标；实际对位由构建产物人工目测 | 保留 |
| 整窗关闭的真实 OS 手势（Alt+F4/系统菜单关闭） | `slterm_app/src/app_shell` | 虚拟窗口无 OS 级关窗注入通道；`simulate_close` 走真实 `on_window_should_close` 回调，但 OS 手势到回调的一段不可自动化 | UI `closing_the_window_with_dirty_files_confirms_first`（拦截/取消/保存落盘）+ `shell_window_mounts_root_and_draws`；真实 Alt+F4 由构建产物人工验收 | 保留 |
| AI assistant 真实模型 HTTP 请求（`ai_assistant::send_model_request` 四协议族出网） | `slterm_app/src/ai_assistant.rs` | 真实外部 API + 凭据依赖，测试进程不发出境请求 | 本地 mock HTTP server 覆盖四协议族 URL/头/body/响应解析（`model_request_anthropic_posts_messages_with_protocol_headers`/`model_request_google_strips_models_prefix_and_posts_generate_content`/`model_request_azure_posts_deployment_path_with_api_version`/`model_request_openai_posts_chat_completions_with_bearer`，含缺字段/非 2xx/full URL/无 key）+ 单元层 `should_suggest`/`redact_secrets`/`parse_fix`/`request_target`/`fallback_api_key` + UI `ai_fix_bar_pastes_only_and_drops_stale_responses`（Pending/Ready/粘贴/过期 seq）+ 构建产物人工实测 | 保留 |
| terminal_math 覆盖层上屏后的像素级字形观感（公式落点/抗锯齿/裁剪边） | `slterm_app/src/terminal_math` | 位图与布局可在单元层逐像素/几何断言，但最终屏幕合成由 GPUI 文本与图像管线在真实 DPI 下完成，虚拟窗口不能代表字形观感 | 单元层 `math_overlay_tests`（扫描/投影/掩码/fit 计划）+ `math_render_tests`（位图合成预算）+ UI `math_overlay_paint_leaves_grid_and_copy_source_untouched` + 构建产物人工目测（`$x^2$`、`\\[ \\]` 块、宽字符） | 保留 |
| 真实系统 toast 投递与 AUMID 注册（WinRT `Show` 跨进程 RPC + `HKCU\Software\Classes\AppUserModelId` 写） | `slterm_app/src/platform/notifications.rs` | 真实投递会污染开发机通知中心、依赖桌面会话与通知开关；系统抑制/AUMID 生效不可从测试进程断言；测试构建短路真实投递 | 漏斗策略单元层全量（前台门控/闪烁先行/全局与 pane 节流/失败 episode/160 截断/时长 XML 映射，`notification_tests`）+ `slterm notify-test` 诊断命令构建产物实跑 + 人工 smoke（通知中心可见 banner 且点击回窗） | 保留 |
| 真实任务栏进度投递（`ITaskbarList3::SetProgressState`/`SetProgressValue` 的 COM 调用与视觉呈现） | `slterm_app/src/taskbar.rs` | 真实调用依赖 Explorer 任务栏与桌面会话，条状视觉不可从测试进程断言；虚拟窗口无原生 HWND，任务栏是纯装饰面 | 映射/状态机单元层全量（`taskbar_flags_and_percentages_match_the_five_states` + 焦点路由/清除路径/跨 pane 隔离；测试构建 `apply` 只记录调用序列不触 COM）+ UI `taskbar_tests` + 构建产物人工 smoke（OSC 9;4 时任务栏出现对应条，agent 退出后消失） | 保留 |
| 真实托盘交互（`Shell_NotifyIconW` 图标挂载/右键 `TrackPopupMenu` 点击/左键点击/通知区图标呈现与橙点视觉） | `slterm_app/src/tray.rs` | 依赖 Explorer 通知区、真实鼠标点击与桌面会话；托盘是进程级外部状态不可重复；测试构建显式不启动托盘线程 | 菜单构建/命令 id 映射/1 Hz 去抖/attention 聚合/命令动作映射/橙点像素单元层全量（`native_menu_encodes_agent_snapshot_and_commands_without_a_thread` 等，不开线程）+ UI `tray_focus_and_quit_events_reach_the_workspace`/`tray_quit_confirms_dirty_files_before_exiting` + 构建产物人工 smoke（图标出现、右键 agent 直达与「显示」回窗、「退出」删图标且退出） | 保留 |
| 真实窗口还原/前置（`IsIconic`/`SW_RESTORE`/`SetForegroundWindow` 对真实 HWND 的调用与视觉前台化） | `slterm_app/src/windowing.rs` | 依赖真实桌面会话与窗口管理器（前台锁定/焦点窃取策略），虚拟窗口无原生 HWND | 还原顺序单元层 `minimized_window_restores_before_foregrounding` + 单一实现点门禁 `win32_focus_calls_only_live_in_the_facade` + UI 回窗路径用例（`legacy_attach_focuses_the_single_window`/`action_button_choice_returns_to_the_named_pane`/托盘回窗）+ 构建产物人工 smoke（最小化后托盘/通知点击还原并前置） | 保留 |
| 忙碌 pane 关闭确认框的触发路径（运行中/等输入/attention） | `slterm_app/src/app_shell/workspace` | 依赖真实 AI 任务生命周期边（hook 事件驱动），虚拟窗口无法稳定构造 running/attention 态 | 忙碌判据 `busy_panes` 由 runtime `confirmation_required` 用例覆盖（pane.close/window.close 拒绝路径）；UI `session_end_closes_the_managed_agent_and_busy_panes_block_window_close`；确认框分支 `confirm_close_busy_pane` 由构建产物人工 smoke 兜底 | 保留；上报待决（是否可构造稳定运行态） |
| 托盘线程 `CreateWindowExW` 失败注入与重试 | `slterm_app/src/tray.rs` | 原生窗口创建失败无注入通道（依赖系统资源/桌面会话状态） | 闸门复位语义在失败分支就地实现；重试节拍由 1 Hz 快照投递覆盖（`enabling_the_tray_reposts_mount_requests_until_mounted`/`mount_action_retries_add_while_the_icon_is_not_mounted`） | 保留；上报待决（是否需要注入通道） |
| config_guard 后台线程启动可达性 | `slterm_app/src/ai_hook/local` | 线程启动无法在虚拟窗口断言；自愈逻辑依赖真实配置文件与 watcher | 自愈/失败边纯逻辑由 `config_guard_tests`（失败边闸门/冷却/忽略码）与 `ai_hook/local/settings.rs` 的 inspect/apply 用例覆盖；`ConfigGuard` 随 `run_shell` 作用域持有并在 Drop join | 保留；上报待决（线程启动是否需专门探针） |
| 真实 setup.exe 覆盖安装与 helper 提权链（PowerShell helper 启动/安装目录写/等待应用退出/Inno 静默安装/版本回读/失败回滚重启） | `slterm_app/src/update_download/handoff` + `scripts/release.ps1` | 依赖安装器管理的真实安装目录、真实 setup.exe 与进程重启；单元层执行会覆盖开发机安装并打断测试进程；helper 是独立 PowerShell 子进程，其 Win32 文件锁/进程身份/提权语义无法在测试进程内对偶 | handoff 文件机单元层（`handoff_file_machine_commits_and_cancels_without_installation_authority`/`orphan_and_prepared_transactions_are_cancelled_on_recovery`/`restore_ticket_covers_success_rollback_attempts_and_acknowledgement`/`installation_guard_lock_reports_an_in_progress_transaction`）+ 更新弹窗状态机单元层 + `fake_release_server_full_chain_rehearsal_is_green` + `update-test-source` 隔离彩排通道 + 构建产物人工验收（真实 setup.exe 安装、版本回读、工作区恢复、失败回滚重启） | 保留 |
| 真实用户 Startup 文件夹 `.lnk` 写入/删除（`SHGetKnownFolderPath(FOLDERID_Startup)` 定位后落盘） | `slterm_app/src/platform/startup.rs` | 真实写入会污染开发机开机启动项且不可重复；测试构建显式短路真实 Startup 路径 | COM 写读 round-trip 单元层（tempdir 假 Startup 目录，`IShellLinkW`/`IPersistFile` 逐字段保真）+ 关闭幂等删除单元层 + Startup 已知文件夹路径解析单元层 + 静默两条件真值表单元层 + UI `launch_at_login_toggle_persists_the_setting_key` + 构建产物人工 smoke（Startup 出现 `slTerminal.lnk`，关闭后删除） | 保留 |
| 真实 DWM 材质渲染与壁纸 GPU 合成（`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`/`SetWindowCompositionAttribute`/`DwmEnableBlurBehindWindow` 对真实 HWND 的调用，Mica/MicaAlt/Aero/Acrylic 视觉呈现，壁纸纹理上屏与透明度叠加） | `slterm_app/src/app_shell/wallpaper.rs` | 依赖真实桌面合成器、系统壁纸与 build（Mica 需 22621+）、真实显卡合成，虚拟窗口无原生 HWND；材质是否被系统实际接纳与观感不可从测试进程断言 | 纯函数 gate 单元层（`background_appearance_gate_matrix_follows_windows_builds`/`material_plan_keeps_the_two_native_channels_mutually_exclusive`：WCA/DWMSBT 互斥、22H2+ Acrylic 走 `DWMSBT_TRANSIENTWINDOW`、写入失败/旧系统回退 WCA state 4）+ 壁纸有界加载单元层（64/128 MiB、2048 边、8 MiB 纹理预算、文件戳缓存、代际失配丢弃、BGRA、预览 512 边）+ 透明度纪律单元层（`shell_opacity_only_dims_surfaces_and_is_idempotent`）+ 静默缝单元层（`visual_effects_never_show_or_activate_windows`）+ 构建产物人工 smoke（五档材质切换、透明度滑块、壁纸透出与对比度） | 保留 |
| 应用图标视觉质量（母版构图/16px 降采样可辨性/ico 嵌入正确性） | `assets/app-icon.png`/`assets/app-icon.ico` | 纯资源替换无可自动化代码逻辑；视觉呈现依赖人眼判定 | 无自动化像素断言；构建产物人工检查清单（图标随包、exe 图标属性/任务栏/Alt-Tab、安装器快捷方式图标目测） | 保留；上报待决（是否需要自动化像素基线） |
| markdown 数学公式渲染的像素级观感（字形/字号缩放/裁剪边/暗色对比） | `slterm_app/src/markdown` | 公式布局可单元层断言，最终字形光栅与阅读列合成由 GPUI 文本管线在真实 DPI 下完成，虚拟窗口不能代表观感 | 单元层 `math_run_tests`（测量/缩放下限/缓存复用）+ `markdown/parser.rs` 数学解析用例 + UI `document_preview_renders_markdown_code_and_image_placeholder` + 构建产物人工目测（行内/块公式、宽公式缩放、暗色协调） | 保留 |

## 条件跳过用例

以下用例依赖环境能力，条件不满足时打印跳过并计「通过」；兜底覆盖必须仍然成立。

- **Windows 应用执行别名/ACL 写入**：
  - `slterm_terminal/src/tty/windows/shell.rs::allowlist_accepts_real_alias_when_present`
  - `slterm_app/src/atomic_file.rs::replacement_preserves_an_explicit_target_acl`
- **真实桌面会话与预构建产物**：`SLTERM_E2E_EXE` 未设置时跳过。
  - `runtime_conformance::runtime_api_conformance_and_second_instance_handover`：真实 GUI 进程 + 双进程移交。兜底 = 单元层 `server_tests` 移交/stale/Drop 用例 + UI `runtime_control_tests` 执行器用例 + 本机真机实跑（2026-10-18 通过）。串行执行：`SLTERM_E2E_EXE=target/debug/slterm.exe cargo test -p slterm --test runtime_conformance -- --test-threads=1`。
- **真实 Windows 凭据管理器**：未设 `SLTERM_TEST_CREDENTIAL_STORE=1` 时跳过；无凭据库的机器/服务账户会失败。
  - `secrets.rs::credential_three_operations_round_trip_through_windows_manager`：三原语往返 + 覆盖写 + 幂等删除。兜底 = 内存后端契约用例 + `SecretBytes` 类型边界/日志边界/借出白名单扫描 + 源码门禁（凭据后端仅门面可达）。

本地开发机（已开开发者模式）为真实覆盖来源；环境不满足时上述分支覆盖记为「不确定」，不得虚构通过。

## 验收与测试

- 新增/修改豁免行时，必须同步核验兜底用例名与路径仍存在。
- 本轮复审已核验所有被引用的测试函数/模块/路径存在；历史悬空来源标识已清除，不再保留编号。
- 本轮不补新测试；标注「上报待决」的条目由用户裁决是否补测或调整兜底。

## 操作

- 新豁免：在本表追加一行，写明归属模块、根因、兜底层级与结论。
- 豁免失效：删除整行；若豁免内容其实是通用纪律，移入 `.claude/docs/testing/strategy.md`。
- 复审：优先检查根因是否仍成立、兜底用例是否仍存在、是否已有新的自动化手段可以覆盖。
