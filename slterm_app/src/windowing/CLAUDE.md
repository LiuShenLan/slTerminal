# 窗口管理 facade

## 决策与红线

- **四路回窗单点**:toast 点击、托盘回窗、toast 动作按钮、legacy ATTACH
  一律经 `focus::focus`(目标 = 窗口 / 窗内 pane)。Win32 焦点/还原原语
  (`SetForegroundWindow`/`ShowWindow`/`BringWindowToTop`/`SW_RESTORE`)只许
  出现在 `focus.rs`;其他文件出现即被门禁用例
  `win32_focus_calls_only_live_in_the_facade` 拦下。托盘菜单的 owner 前置
  不是回窗语义,同样收在 `foreground_menu_owner`。
- **签名适配**:设计稿 `focus_window(cx)`/`focus_pane(pane, cx)` 是多窗注册表
  形态;单窗壳的事件泵持有 `&mut Window`,且 GPUI 禁止在窗口更新回调内再次
  `AnyWindowHandle::update`,故 facade 以显式 `&mut Window` 落点。目标选择与
  平台调用仍由本模块唯一承担。
- **还原语义(B46)**:最小化 → `IsIconic` → `SW_RESTORE` → 前置;正常窗口
  只前置。顺序由 `focus_steps` 纯函数锁死,真实 Win32 面按豁免走
  构建产物 smoke。
- **HWND 单源**:开窗时 `publish_window` 发布句柄;任务栏闪烁/进度只经
  `window_hwnd` 查询。虚拟窗口无原生句柄 → None,调用方按 0 兜底。
- **托盘退出(B47)**:`quit_from_tray` 必须先过 `guard_file_window_close`;
  确认在途/保存在途 = 本轮拦截,异步放行后由守卫自身关窗。
- **静默启动窗口形态(M10.6)**:首窗 `show=false`/`focus=false` 创建,不
  进入 present-then-show(未曝光无需防黑窗);托盘「显示」经本 facade 的
  GPUI `activate_window` 消费首窗 placement 完成曝光 + 前台化。显式 CLI
  启动(`--working-directory`/`-e`)恒不静默(设置两条件只作用于常规启动)。
