# 窗口管理 facade

> 回窗、还原、前置与 HWND 查询的单点平台门面。

## 决策与红线

- **回窗单点**：toast 点击、托盘回窗、toast 动作按钮、legacy ATTACH 一律经 `focus::focus`（目标 = 窗口 / 窗内 pane）。Win32 焦点/还原原语（`SetForegroundWindow`/`ShowWindow`/`BringWindowToTop`/`SW_RESTORE`）只许出现在 `focus.rs`；其他文件出现即被门禁用例 `win32_focus_calls_only_live_in_the_facade` 拦下。托盘菜单的 owner 前置不是回窗语义，同样收在 `foreground_menu_owner`。
- **签名适配**：单窗壳的事件泵持有 `&mut Window`，且 GPUI 禁止在窗口更新回调内再次 `AnyWindowHandle::update`，故 facade 以显式 `&mut Window` 落点。目标选择与平台调用仍由本模块唯一承担。
- **还原语义**：最小化 → `IsIconic` → `SW_RESTORE` → 前置；正常窗口只前置。顺序由 `focus_steps` 纯函数锁死，真实 Win32 面按豁免走构建产物 smoke。
- **HWND 单源**：开窗时 `publish_window` 发布句柄；任务栏闪烁/进度只经 `window_hwnd` 查询。虚拟窗口无原生句柄 → None，调用方按 0 兜底。
- **托盘退出**：`quit_from_tray` 必须先过 `guard_file_window_close`；确认在途/保存在途 = 本轮拦截，异步放行后由守卫自身关窗。
- **静默启动窗口形态**：首窗 `show=false`/`focus=false` 创建，不进入 present-then-show（未曝光无需防黑窗）；托盘「显示」经本 facade 的 GPUI `activate_window` 消费首窗 placement 完成曝光 + 前台化。显式 CLI 启动（`--working-directory`/`-e`）恒不静默（设置两条件只作用于常规启动）。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：`minimized_window_restores_before_foregrounding`、`win32_focus_calls_only_live_in_the_facade`、回窗路径（`legacy_attach_focuses_the_single_window`/`action_button_choice_returns_to_the_named_pane`/托盘回窗）、托盘退出脏文件确认。
- 真实窗口还原/前置与视觉前台化由构建产物人工 smoke 兜底，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增回窗入口：一律经 `focus::focus`，不得直接调用平台原语。
- 新增 HWND 消费：经 `window_hwnd` 查询，不缓存原生句柄。
