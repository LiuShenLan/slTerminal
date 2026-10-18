//! 虚拟窗口设施测试:feature 链开窗 + Root 挂载 + 真实绘制一帧。
//!
//! 钉版/feature 形态与设计假设不符(虚拟窗口补丁须对不存在的原生句柄回
//! `Unavailable` 而非 panic)时本组起不来——起不来先查钉版与 feature 接线,
//! 不要先动断言。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gpui::{AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use gpui_component::Root;
use slterm_split::PaneId;

use super::windowing::{WorkspaceStartup, workspace_window_options};
use super::workspace::session_recovery::{StartupPlan, capture_window_state, prepare_startup_at};
use super::workspace::sidebar::{SideViewId, SidebarState};
use super::workspace::title_bar::TitleBarCommand;
use super::workspace::{
    CloseActivePane, CloseActiveTab, NewTerminalTab, SelectNextTab, SelectPreviousTab,
    SltermWorkspace, SplitRight, ToggleSidebar, ToggleZoom,
};
use crate::ai_hook::AiHookEvent;
use crate::cli::Program;
use crate::session::{AgentSession, LaunchSession, WindowState, load_from};
use slterm_terminal::event_loop::Msg as EventLoopMsg;

mod hooks_tests;
mod runtime_projection_tests;
mod theme_tests;

/// 与生产 init 同序的测试子集(11 篇模板同形):内嵌 Maple 在测试文本系统
/// 不栅格化真实字形,20MB 注册纯属拖慢,测试不挂。
fn bootstrap(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(super::config::Settings::default());
        super::theme::apply_dark_theme(cx);
        super::workspace::init(cx);
    });
}

/// 挂载完整工作区(启动一个「不存在 shell」的终端——spawn 必败、不拉真
/// PTY,测试会话经 `session::test_session` 顶替;真 PTY 会让 PTY reader
/// 线程跨测试存活,wake 已换线程的 executor 任务直接 fail-fast),返回工作区
/// 与虚拟窗口。VisualTestContext own TestAppContext(Clone 共享底层):clone
/// 出拥有值,调用方手持窗口的同时仍可经 cx 读写实体(zed 测试惯例)。
fn open_workspace(cx: &mut TestAppContext) -> (gpui::Entity<SltermWorkspace>, VisualTestContext) {
    let (workspace, window, _ai_tx) = open_workspace_with_ai(cx);
    (workspace, window)
}

/// 同 [`open_workspace`],额外返回 hook 事件发送端(M4.5 事件泵断言用)。
fn open_workspace_with_ai(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<SltermWorkspace>,
    VisualTestContext,
    mpsc::Sender<AiHookEvent>,
) {
    open_workspace_with_command(
        cx,
        Program::Just("slterm-test-missing-shell-executable".into()),
    )
}

/// 以指定 `-e` 命令开窗(启动身份冻结用例专用)。
fn open_workspace_with_command(
    cx: &mut TestAppContext,
    command: Program,
) -> (
    gpui::Entity<SltermWorkspace>,
    VisualTestContext,
    mpsc::Sender<AiHookEvent>,
) {
    open_workspace_with_startup(
        cx,
        WorkspaceStartup::LaunchTerminal { cwd: None, command },
        StartupPlan::fresh(),
    )
}

/// 以任意启动变体 + 恢复计划开窗(会话恢复 E2E 用例专用)。
fn open_workspace_with_startup(
    cx: &mut TestAppContext,
    startup: WorkspaceStartup,
    plan: StartupPlan,
) -> (
    gpui::Entity<SltermWorkspace>,
    VisualTestContext,
    mpsc::Sender<AiHookEvent>,
) {
    bootstrap(cx);
    let (_tx, rx) = mpsc::channel();
    let (ai_tx, ai_rx) = mpsc::channel();
    let mut slot = None;
    let (_root, window) = cx.add_window_view(|window, cx| {
        let workspace =
            cx.new(|cx| SltermWorkspace::new(window, rx, Some(ai_rx), startup, plan, cx));
        slot = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    window.run_until_parked();
    window.refresh().unwrap();
    (slot.unwrap(), window.clone(), ai_tx)
}

/// 一条最小 claude 信封事件(直接走协议解析,与生产链同一入口)。
fn ai_prompt_event(pane: u64, session: &str) -> AiHookEvent {
    let wire = format!(
        "slterm-hook/1 source=claude pane={pane}\n\
         {{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"{session}\"}}"
    );
    crate::ai_hook::parse_envelope(wire.as_bytes()).expect("测试信封应可解析")
}

/// M3.1 冒烟:开窗 + Root 挂载 + draw 不 panic(clear 收尾是 arena 纪律)。
#[gpui::test]
fn shell_window_mounts_root_and_draws(cx: &mut TestAppContext) {
    let (_workspace, mut window) = open_workspace(cx);
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

/// 渲染关键路径(退出标准 1):经 test_session 顶替 PTY,喂字节进 Term,
/// process_event 走泵消化,绘制后 `slterm-terminal` 的 debug_bounds 为真实
/// 坐标且非零面积。
#[gpui::test]
fn terminal_renders_fed_bytes_with_real_bounds(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    // 拿到唯一 pane 的视图实体,替换会话为不拉进程的 test_session。
    let view = workspace.read_with(cx, |ws, _| ws.first_pane_view().unwrap());
    view.update(cx, |view, _| {
        let (session, _receiver) = crate::gpui_terminal::session::test_session();
        view.session = Some(session);
        // 注入可渲染文本(不经 PTY,标准注入姿势)。
        let mut term = view.session.as_ref().unwrap().term.lock();
        let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
        parser.advance(&mut *term, b"hello slterm\r\n");
    });
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    // 真实坐标:debug_bounds 命中终端容器,矩形非零。
    let bounds = window
        .debug_bounds("slterm-terminal")
        .expect("terminal element must paint");
    assert!(bounds.size.width > gpui::px(0.0));
    assert!(bounds.size.height > gpui::px(0.0));
}

/// 启动零 resize 锚(退出标准 2):spawn 网格 = 首帧布局网格。开窗户形按
/// 116×30 基准反推,首帧 set_layout 命中的行列必须等于 prepare_initial_grid
/// 给出的 spawn 网格,否则 ConPTY 在首屏输出中途被重排(回显错行的温床)。
#[gpui::test]
fn startup_grid_matches_first_frame_layout(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let view = workspace.read_with(cx, |ws, _| ws.first_pane_view().unwrap());
    // spawn 网格 = workspace.initial_grid;首帧布局网格 = view.cols/rows。
    let spawn_grid = workspace.read_with(cx, |ws, _| ws.initial_grid());
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let (cols, rows) = view.read_with(cx, |view, _| (view.grid_cols(), view.grid_rows()));
    assert_eq!(
        (cols, rows),
        (usize::from(spawn_grid.0), usize::from(spawn_grid.1)),
        "first-frame layout grid must equal the spawn grid"
    );
}

/// M4.5 事件泵:前台任务排空 hook 通道,按信封 pane 写入对应宿主的
/// `AgentActivity`(不 mock 通道,验证 GPUI foreground 排水形态)。
#[gpui::test]
fn ai_hook_pump_routes_event_to_the_owning_pane_host(cx: &mut TestAppContext) {
    let (workspace, window, ai_tx) = open_workspace_with_ai(cx);
    let pane_id = workspace.read_with(cx, |ws, _| ws.first_pane_id().unwrap());
    ai_tx
        .send(ai_prompt_event(pane_id.0, "ui-pump-session"))
        .unwrap();
    // 泵批间隔 75ms:受控时钟推进后前台任务消费并落状态。
    window.executor().advance_clock(Duration::from_millis(80));
    window.run_until_parked();
    let status = workspace.read_with(cx, |ws, _| ws.agent_status(pane_id));
    assert_eq!(status, Some(crate::ai_agents::AgentStatus::Working));
}

/// 画一帧并清 arena(现有 UI 测试的统一收尾姿势)。
fn draw_frame(window: &mut VisualTestContext) {
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

/// 不变式「panes == 树叶」路径 1/3:新建 tab。
#[gpui::test]
fn new_terminal_tab_keeps_panes_equal_to_tree_leaves(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    let (panes, leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("新 tab 必须有活跃终端");
    assert_eq!(panes, leaves, "panes 与树叶必须恒等");
    assert_eq!(panes.len(), 1);
    assert_eq!(focused, panes[0]);
    assert!(!zoomed);
}

/// 不变式「panes == 树叶」路径 2/3 + L4 渲染关键路径:split 后真实分屏树
/// 渲染出两个 pane 元素(两处 debug_bounds 非零)。
#[gpui::test]
fn split_renders_two_panes_and_keeps_panes_equal_to_tree_leaves(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let (panes, leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(panes, leaves, "split 后 panes 与树叶必须恒等");
    assert_eq!(panes.len(), 2);
    assert_eq!(focused, panes[1], "split 后焦点交新 pane");
    assert!(!zoomed);

    for selector in ["slterm-pane-1", "slterm-pane-2"] {
        let bounds = window
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} 必须绘制"));
        assert!(
            bounds.size.width > px(0.0) && bounds.size.height > px(0.0),
            "{selector} 矩形必须非零: {bounds:?}"
        );
    }
}

/// 不变式「panes == 树叶」路径 3/3:关 pane(收编路径)。
#[gpui::test]
fn close_pane_keeps_panes_equal_to_tree_leaves(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.dispatch_action(CloseActivePane);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let (panes, leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(panes, leaves, "关 pane 后 panes 与树叶必须恒等");
    assert_eq!(panes, vec![PaneId(1)], "兄弟收编而非关 tab");
    assert_eq!(focused, PaneId(1), "焦点交幸存子树首叶");
    assert!(!zoomed);
}

/// zoom 特例:满卡只渲染聚焦 pane;toggle 两次回到分屏。
#[gpui::test]
fn zoom_renders_only_the_focused_pane_and_toggles_back(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(window.debug_bounds("slterm-pane-1").is_some());
    assert!(window.debug_bounds("slterm-pane-2").is_some());

    window.dispatch_action(ToggleZoom);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);
    let (_, _, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert!(zoomed, "第一次 toggle 进入满卡");
    assert_eq!(focused, PaneId(2));
    assert!(window.debug_bounds("slterm-pane-2").is_some());
    assert!(
        window.debug_bounds("slterm-pane-1").is_none(),
        "满卡路径只渲染聚焦 pane"
    );

    window.dispatch_action(ToggleZoom);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);
    let (_, _, _, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert!(!zoomed, "第二次 toggle 回到分屏");
    assert!(window.debug_bounds("slterm-pane-1").is_some());
    assert!(window.debug_bounds("slterm-pane-2").is_some());
}

/// 满卡内点击聚焦 pane(选字起手)不得解除 zoom。
#[gpui::test]
fn clicking_the_zoomed_pane_keeps_zoom(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.dispatch_action(ToggleZoom);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let pane = window
        .debug_bounds("slterm-pane-2")
        .expect("满卡 pane 必须绘制");
    let center = pane.center();
    window.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
    window.run_until_parked();

    let (_, _, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert!(zoomed, "点击已聚焦 pane 不应退出满卡");
    assert_eq!(focused, PaneId(2));
}

/// 单 pane 无缩放语义:toggle 是空操作(05 篇 zoom 状态机)。
#[gpui::test]
fn zoom_is_a_no_op_with_a_single_pane(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(ToggleZoom);
    window.run_until_parked();
    let (_, _, _, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert!(!zoomed);
}

/// 分隔条拖拽关键路径:overlay 预览不动提交比例/pane 几何;松手 commit
/// 后几何生效(05 篇「拖拽提交流」)。
#[gpui::test]
fn divider_drag_previews_without_reflow_and_commits_geometry(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let ratio_before = workspace.read_with(cx, |ws, _| ws.active_split_ratio().unwrap());
    let first_before = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)).unwrap());
    let divider = window
        .debug_bounds("split-divider-0-[]")
        .expect("分隔条命中区必须绘制");
    let start = divider.center();

    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = point(start.x + px(60.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    // 预览期间:overlay 出现;提交比例与 pane 几何不动(终端网格零重排)。
    assert!(
        window.debug_bounds("split-drag-line").is_some(),
        "拖拽预览线必须绘制"
    );
    let ratio_during = workspace.read_with(cx, |ws, _| ws.active_split_ratio().unwrap());
    assert_eq!(ratio_during, ratio_before, "预览不得改提交比例");
    let first_during = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)).unwrap());
    assert_eq!(
        first_during.size.width, first_before.size.width,
        "预览不得重排 pane/终端网格"
    );

    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let ratio_after = workspace.read_with(cx, |ws, _| ws.active_split_ratio().unwrap());
    assert!(
        ratio_after > ratio_before + 0.01,
        "松手提交后比例生效: {ratio_before} -> {ratio_after}"
    );
    let first_after = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)).unwrap());
    assert!(
        first_after.size.width > first_before.size.width,
        "松手后第一 pane 几何应变宽"
    );
}

/// 拖拽进入关闭区:松手走 `close_pane` 收编,panes == 树叶不变式不破。
#[gpui::test]
fn dragging_divider_into_close_zone_closes_the_squeezed_pane(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let first_before = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)).unwrap());
    let divider = window
        .debug_bounds("split-divider-0-[]")
        .expect("分隔条命中区必须绘制");
    let start = divider.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    // 指针压到第一 pane 左缘:原始比例低于 CLOSE_MARGIN,进入关闭区。
    let target = point(first_before.origin.x + px(2.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let (panes, leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(panes, leaves, "拖拽关闭后 panes 与树叶必须恒等");
    assert_eq!(panes, vec![PaneId(2)], "被挤压的首 pane 关闭、兄弟收编");
    assert_eq!(focused, PaneId(2), "焦点保持在幸存 pane");
    assert!(!zoomed);
}

/// M4.5 屏幕看门狗:1 Hz 拍帧取网格末尾窗口,身份与规则命中后写入仲裁器。
#[gpui::test]
fn screen_watchdog_identifies_agent_and_updates_status(cx: &mut TestAppContext) {
    let (workspace, window) = open_workspace(cx);
    let view = workspace.read_with(cx, |ws, _| ws.first_pane_view().unwrap());
    let pane_id = workspace.read_with(cx, |ws, _| ws.first_pane_id().unwrap());
    view.update(cx, |view, _| {
        let (session, _receiver) = crate::gpui_terminal::session::test_session();
        view.session = Some(session);
        let mut term = view.session.as_ref().unwrap().term.lock();
        let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
        parser.advance(&mut *term, b"welcome to kimi code!\r\n");
        parser.advance(
            &mut *term,
            "\u{1F314} \u{00B7} Tip: keep going\r\n".as_bytes(),
        );
    });
    // 观察窗口本身:末尾行必须可见(最近 24 行窗口)。
    let sample = view.read_with(&window, |view, _| view.agent_screen_sample());
    let sample = sample.expect("有会话应产出屏幕样本");
    assert!(sample.contains("welcome to kimi code!"), "{sample:?}");
    // 看门狗一拍:屏幕品牌 chrome 建立身份,规则引擎给出 Working。
    window.executor().advance_clock(Duration::from_secs(1));
    window.run_until_parked();
    let status = workspace.read_with(cx, |ws, _| ws.agent_status(pane_id));
    assert_eq!(status, Some(crate::ai_agents::AgentStatus::Working));
}

// ── M6.2 设置页关键路径 ──

/// 在临时数据目录打开设置 tab,返回设置页实体(不触碰真实 `%APPDATA%`)。
fn open_settings_at(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    path: std::path::PathBuf,
) -> gpui::Entity<super::settings_pane::SettingsPane> {
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.open_settings_tab_at(path, window, cx);
        });
    });
    window.run_until_parked();
    workspace
        .read_with(cx, |workspace, _| workspace.settings_pane_for_test())
        .expect("设置 tab 必须已打开")
}

/// ActivityBar 配置钮 → 设置 tab;导航切换;返回工作区关闭设置 tab。
#[gpui::test]
fn settings_open_from_activity_bar_and_nav_switches_pages(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-settings-btn").is_some());

    click_selector(&mut window, "sidebar-settings-btn");
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-application").is_some(),
        "默认落在应用页"
    );
    assert!(window.debug_bounds("settings-nav-appearance").is_some());

    click_selector(&mut window, "settings-nav-appearance");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-appearance").is_some(),
        "导航切换必须渲染外观页"
    );
    assert!(window.debug_bounds("settings-page-application").is_none());

    click_selector(&mut window, "settings-back");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-appearance").is_none(),
        "返回工作区必须关闭设置 tab"
    );
    assert!(
        workspace
            .read_with(cx, |workspace, _| workspace.settings_pane_for_test())
            .is_none()
    );
}

/// 三纪律之一/二:滑条拖动只改内存(不触盘),松手提交落盘;全局
/// `Settings` 在预览阶段即热更新(滚轮消费同一份事实源)。
#[gpui::test]
fn settings_slider_previews_in_memory_and_release_commits(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, path.clone());
    window.refresh().unwrap();
    draw_frame(&mut window);

    pane.update(cx, |pane, cx| {
        pane.preview_scroll_speed_for_test(2.0, cx);
    });
    assert!(!path.exists(), "预览阶段绝不落盘");
    let (runtime_speed, global_speed) = cx.update(|cx| {
        (
            pane.read(cx).runtime_for_test().scroll_speed,
            cx.global::<super::config::Settings>().scroll_speed,
        )
    });
    assert_eq!(runtime_speed, 2.0, "预览必须即时更新运行时值");
    assert_eq!(global_speed, 2.0, "预览必须热应用全局 Settings");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_scroll_speed_for_test(2.25, window, cx);
        });
    });
    assert!(path.exists(), "松手提交必须落盘");
    let tree = slterm_settings::json::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        tree.get("scrolling")
            .and_then(|section| section.get("scroll_speed"))
            .and_then(|value| value.as_f64()),
        Some(2.25),
        "归一化后的滚速写入 scrolling 段"
    );
    cx.update(|cx| {
        assert_eq!(pane.read(cx).runtime_for_test().scroll_speed, 2.25);
        assert!(pane.read(cx).status_for_test().is_none());
    });
}

/// `Changed` 驱动热应用:字号提交后,已打开终端视图立即换字号(字体
/// 度量变化走结构性 resize,不等下一 tab)。
#[gpui::test]
fn settings_font_size_change_hot_applies_to_open_terminals(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let view = workspace.read_with(cx, |workspace, _| workspace.first_pane_view().unwrap());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, path);
    window.refresh().unwrap();
    draw_frame(&mut window);

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.save_text_for_test("font_size", "20", window, cx);
        });
    });
    window.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.font_size),
        px(20.0),
        "已打开终端必须热应用新字号"
    );
    cx.update(|cx| {
        assert_eq!(cx.global::<super::config::Settings>().font_size_px, 20.0);
    });
}

/// 三纪律之三:写失败可感知 + 读回已存值复位控件(预览态不得残留)。
#[gpui::test]
fn settings_write_failure_rolls_back_the_slider_and_reports_status(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    // 路径是目录 → 读写通道均报错,写失败路径确定可复现。
    let path = directory.path().join("settings.json");
    std::fs::create_dir_all(&path).unwrap();
    let pane = open_settings_at(cx, &workspace, &mut window, path);
    window.refresh().unwrap();
    draw_frame(&mut window);

    pane.update(cx, |pane, cx| pane.preview_scroll_speed_for_test(2.0, cx));
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_scroll_speed_for_test(2.0, window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert_eq!(
            pane.runtime_for_test().scroll_speed,
            1.0,
            "写失败必须回滚到已存值"
        );
        assert_eq!(
            pane.scroll_speed_slider_value_for_test(cx),
            1.0,
            "控件同步复位"
        );
        assert!(
            pane.status_for_test()
                .is_some_and(|status| status.is_error()),
            "失败必须留下可感知的语义状态"
        );
        assert_eq!(cx.global::<super::config::Settings>().scroll_speed, 1.0);
    });
}

/// 搜索是路由查找:查询命中跳到拥有该控件的页面,不是过滤当前页。
#[gpui::test]
fn settings_search_jumps_to_the_owning_page(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    open_settings_at(cx, &workspace, &mut window, path);
    window.refresh().unwrap();
    draw_frame(&mut window);

    click_selector(&mut window, "settings-search");
    window.simulate_input("字体");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-appearance").is_some(),
        "中文别名必须跳到外观页"
    );
    assert!(window.debug_bounds("settings-page-application").is_none());

    // 清空查询后回到搜索前页面。
    window.simulate_keystrokes("backspace backspace");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-application").is_some(),
        "清空查询必须回到原页面"
    );

    click_selector(&mut window, "settings-search");
    window.simulate_input("language");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-localization").is_some(),
        "英文别名必须跳到语言页"
    );
}

/// 值/显示分离:语言切换只刷显示项,控件实体身份不变(焦点/编辑值/
/// undo/订阅不重建),同时语言值持久化。
#[gpui::test]
fn settings_language_switch_refreshes_display_without_rebuilding_controls(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    std::fs::write(&path, r#"{"appearance":{"language":"en-US"}}"#).unwrap();
    let pane = open_settings_at(cx, &workspace, &mut window, path.clone());
    window.refresh().unwrap();
    draw_frame(&mut window);

    let before = cx.update(|cx| {
        (
            pane.read(cx)
                .select_entity_id_for_test("language")
                .expect("语言下拉必须存在"),
            pane.read(cx)
                .select_entity_id_for_test("bell")
                .expect("铃铛下拉必须存在"),
        )
    });
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_select_for_test("language", "zh-CN", window, cx);
        });
    });
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert_eq!(
            pane.select_entity_id_for_test("language"),
            Some(before.0),
            "语言切换不得重建下拉实体"
        );
        assert_eq!(
            pane.select_entity_id_for_test("bell"),
            Some(before.1),
            "其它下拉实体同样保留"
        );
        assert_eq!(
            pane.runtime_for_test().language,
            slterm_settings::LanguagePref::ZhCn
        );
        assert_eq!(
            pane.language_for_test(),
            crate::i18n::UiLanguage::ZhCn,
            "语言切换后渲染语言缓存必须同步刷新"
        );
    });
    let tree = slterm_settings::json::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        tree.get("appearance")
            .and_then(|section| section.get("language"))
            .and_then(|value| value.as_str()),
        Some("zh-CN")
    );
}

// ── M5.3b 侧栏骨架 / 标题栏 / tab 动作 ──

/// 按 debug selector 取元素中心并完成一次左键点击。
fn click_selector(window: &mut VisualTestContext, selector: &'static str) {
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} 必须绘制"));
    let center = bounds.center();
    window.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
}

/// 侧栏默认骨架:ActivityBar 四钮 + 上槽导航树占位 + 宽度拖拽热区。
#[gpui::test]
fn sidebar_renders_activity_bar_and_the_default_nav_slot(cx: &mut TestAppContext) {
    let (_workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);

    let bar = window
        .debug_bounds("sidebar-activity-bar")
        .expect("ActivityBar 必须绘制");
    assert!(bar.size.width > px(0.0) && bar.size.height > px(0.0));
    for selector in [
        "sidebar-view-btn-nav",
        "sidebar-view-btn-explorer",
        "sidebar-view-btn-commit",
        "sidebar-view-btn-agent_files",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制"
        );
    }
    assert!(
        window.debug_bounds("sidebar-slot-top-nav").is_some(),
        "默认应打开导航树上槽"
    );
    assert!(
        window.debug_bounds("sidebar-resize-handle").is_some(),
        "内容区可见时应有宽度拖拽热区"
    );
}

/// 双槽 active view 切换关键路径:R1 同槽替换、R2 关闭、下槽独立
/// 打开(双开 split)、双空回到 hidden。
#[gpui::test]
fn sidebar_activity_buttons_switch_both_slots(cx: &mut TestAppContext) {
    let (_workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-slot-top-nav").is_some());

    // R1:同槽打开新视图,旧视图隐式关闭。
    click_selector(&mut window, "sidebar-view-btn-explorer");
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-slot-top-explorer").is_some());
    assert!(window.debug_bounds("sidebar-slot-top-nav").is_none());

    // 下槽独立打开:双槽 split。
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-slot-top-explorer").is_some());
    assert!(
        window
            .debug_bounds("sidebar-slot-bottom-agent_files")
            .is_some(),
        "下槽视图必须渲染"
    );

    // R2:下槽关闭,上槽不受影响。
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("sidebar-slot-bottom-agent_files")
            .is_none()
    );
    assert!(window.debug_bounds("sidebar-slot-top-explorer").is_some());

    // R2+R3:上槽也关闭 → 内容区整体隐藏,ActivityBar 常驻。
    click_selector(&mut window, "sidebar-view-btn-explorer");
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-area").is_none());
    assert!(window.debug_bounds("sidebar-activity-bar").is_some());
}

/// collapsed 开合:内容区与拖拽热区隐藏,ActivityBar 常驻;再开恢复现场。
#[gpui::test]
fn sidebar_toggle_action_collapses_and_expands_content(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(window.debug_bounds("sidebar-area").is_some());

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(workspace.read_with(cx, |ws, _| ws.sidebar_state().collapsed));
    assert!(
        window.debug_bounds("sidebar-area").is_none(),
        "collapsed 时内容区不占布局"
    );
    assert!(
        window.debug_bounds("sidebar-resize-handle").is_none(),
        "collapsed 时无宽度拖拽热区"
    );
    assert!(window.debug_bounds("sidebar-activity-bar").is_some());

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(!workspace.read_with(cx, |ws, _| ws.sidebar_state().collapsed));
    assert!(window.debug_bounds("sidebar-slot-top-nav").is_some());
}

/// 宽度拖拽:跟手更新,越界钳到旧栈 [160, 500]。
#[gpui::test]
fn sidebar_width_drag_clamps_to_old_stack_bounds(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);
    let initial = workspace.read_with(cx, |ws, _| ws.sidebar_state().width);
    assert_eq!(initial, 250.0);

    let handle = window
        .debug_bounds("sidebar-resize-handle")
        .expect("拖拽热区必须绘制");
    let start = handle.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = point(start.x + px(120.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);

    let widened = workspace.read_with(cx, |ws, _| ws.sidebar_state().width);
    assert!(
        (widened - 370.0).abs() < 1.0,
        "宽度应跟手到 370,实际 {widened}"
    );

    // 第二次拖到窗口左缘:超出下限被钳到 160。
    let handle = window
        .debug_bounds("sidebar-resize-handle")
        .expect("拖拽热区必须仍在");
    let start = handle.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = point(px(1.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();

    let narrowed = workspace.read_with(cx, |ws, _| ws.sidebar_state().width);
    assert_eq!(narrowed, 160.0, "旧栈宽度下限 160");
}

/// 恢复形态:仅下槽 active 时渲染 single-bottom。
#[gpui::test]
fn sidebar_bottom_only_restore_renders_single_bottom_slot(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    workspace.update(cx, |ws, _| {
        ws.set_sidebar_views_for_test(None, Some(SideViewId::AgentFiles));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert!(
        window
            .debug_bounds("sidebar-slot-bottom-agent_files")
            .is_some()
    );
    assert!(window.debug_bounds("sidebar-slot-top-nav").is_none());
    assert!(window.debug_bounds("sidebar-area").is_some());
}

/// 标题栏:自绘三钮 + 活跃 tab 展示名;自定义名优先于终端标题。
#[gpui::test]
fn titlebar_renders_active_tab_title_and_window_buttons(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);

    for selector in [
        "titlebar",
        "titlebar-title",
        "titlebar-min",
        "titlebar-max",
        "titlebar-close",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制"
        );
    }

    let title = workspace.read_with(cx, |ws, cx| ws.active_tab_title_for_test(cx));
    assert_eq!(title, "slterm-test-missing-shell-executable");

    workspace.update(cx, |ws, _| {
        ws.set_active_tab_custom_name_for_test(Some("后端".into()));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    let title = workspace.read_with(cx, |ws, cx| ws.active_tab_title_for_test(cx));
    assert_eq!(title, "后端", "TabMeta.custom_name 是展示名第一优先源");
}

/// 三钮行为:虚拟窗口记录命令分派(最小化/最大化切换/关闭)。
#[gpui::test]
fn titlebar_buttons_dispatch_window_commands(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);

    click_selector(&mut window, "titlebar-min");
    click_selector(&mut window, "titlebar-max");
    click_selector(&mut window, "titlebar-close");
    draw_frame(&mut window);

    let commands = workspace.read_with(cx, |ws, _| ws.window_commands().to_vec());
    assert_eq!(
        commands,
        [
            TitleBarCommand::Minimize,
            TitleBarCommand::MaximizeToggle,
            TitleBarCommand::Close,
        ]
    );
}

/// tab 动作:ctrl-shift-t 新建、ctrl-tab/ctrl-shift-tab 循环切换、
/// 关闭活跃 tab 后焦点与活跃下标收敛。
#[gpui::test]
fn tab_actions_create_switch_and_close(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 1);

    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    draw_frame(&mut window);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 2);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 1);
    let (panes, _, _, _) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("新 tab 必须有活跃终端");
    assert_eq!(panes, vec![PaneId(2)], "新 tab 分配下一个 pane id");

    window.dispatch_action(SelectPreviousTab);
    window.run_until_parked();
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 0);
    let (panes, _, _, _) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("首个 tab 必须存活");
    assert_eq!(panes, vec![PaneId(1)]);

    window.dispatch_action(SelectNextTab);
    window.run_until_parked();
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 1);

    window.dispatch_action(CloseActiveTab);
    window.run_until_parked();
    draw_frame(&mut window);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 1);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 0);
    let (panes, _, _, _) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("幸存 tab 必须存活");
    assert_eq!(panes, vec![PaneId(1)]);
}

// ── M5.4 tab 拖拽停靠 + split_bounds 键修复 ──

/// 两个 tab 的通用起手:tab1 左右分屏(panes 1/2),tab2 单 pane(3),
/// 最后回到 tab1 作停靠目标(活跃 tab)。
fn two_tabs_with_target_split(
    cx: &mut TestAppContext,
) -> (gpui::Entity<SltermWorkspace>, VisualTestContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    window.dispatch_action(SelectPreviousTab);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);
    (workspace, window)
}

/// tab 行拖到活跃 tab 内容区外圈:根级停靠,整棵源 tab 树并到活跃树右侧。
#[gpui::test]
fn tab_drag_to_the_content_edge_docks_the_source_at_the_root(cx: &mut TestAppContext) {
    let (workspace, mut window) = two_tabs_with_target_split(cx);
    let (left, right) = workspace.read_with(cx, |ws, _| {
        (
            ws.pane_bounds_of(PaneId(1)).unwrap(),
            ws.pane_bounds_of(PaneId(2)).unwrap(),
        )
    });
    let x0 = f32::from(left.origin.x.min(right.origin.x));
    let y0 = f32::from(left.origin.y.min(right.origin.y));
    let x1 = f32::from((right.origin.x + right.size.width).max(left.origin.x + left.size.width));
    let y1 = f32::from((left.origin.y + left.size.height).max(right.origin.y + right.size.height));
    // 外圈停靠带:距内容区右缘 2px、纵向居中。
    let drop = point(px(x1 - 2.0), px((y0 + y1) * 0.5));

    let row = window
        .debug_bounds("sidebar-tab-1")
        .expect("源 tab 行必须绘制");
    window.simulate_mouse_down(row.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(drop, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    let preview = window
        .debug_bounds("tab-dock-preview")
        .expect("外圈落点必须画出停靠预览");
    assert!(
        (f32::from(preview.size.width) - (x1 - x0) * 0.5).abs() < 3.0,
        "根级停靠预览应占内容区一半宽: {preview:?}"
    );
    assert!(
        (f32::from(preview.origin.x + preview.size.width) - x1).abs() < 1.0,
        "Right 停靠预览应贴内容区右缘"
    );

    window.simulate_mouse_up(drop, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        1,
        "源 tab 并入后消失"
    );
    let (panes, leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(panes, leaves, "停靠后 panes 与树叶必须恒等");
    assert_eq!(
        leaves,
        vec![PaneId(1), PaneId(2), PaneId(3)],
        "整棵树按 Right 嫁接到根"
    );
    assert_eq!(focused, PaneId(3), "焦点交被拖入 tab 的聚焦 pane");
    assert!(!zoomed);
    for selector in ["slterm-pane-1", "slterm-pane-2", "slterm-pane-3"] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制(提交后树形与渲染生效)"
        );
    }
}

/// tab 行拖到 pane 内四分位:只切该 pane,预览形状半卡。
#[gpui::test]
fn tab_drag_into_a_pane_quadrant_splits_only_that_pane(cx: &mut TestAppContext) {
    let (workspace, mut window) = two_tabs_with_target_split(cx);
    let pane1 = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)).unwrap());
    // 距左缘 20px(越过 14px 外圈带),纵向居中 → pane1 的最近边 = Left。
    let drop = point(
        pane1.origin.x + px(20.0),
        pane1.origin.y + pane1.size.height * 0.5,
    );

    let row = window
        .debug_bounds("sidebar-tab-1")
        .expect("源 tab 行必须绘制");
    window.simulate_mouse_down(row.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(drop, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    let preview = window
        .debug_bounds("tab-dock-preview")
        .expect("pane 落点必须画出停靠预览");
    assert!(
        (f32::from(preview.size.width) - f32::from(pane1.size.width) * 0.5).abs() < 3.0,
        "pane 四分位预览应占该 pane 一半宽: {preview:?}"
    );
    assert!(
        (f32::from(preview.size.height) - f32::from(pane1.size.height)).abs() < 1.0,
        "Left 停靠预览应占满目标 pane 高度"
    );

    window.simulate_mouse_up(drop, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 1);
    let (mut panes, mut leaves, focused, zoomed) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    panes.sort();
    leaves.sort();
    assert_eq!(panes, leaves, "停靠后 panes 与树叶必须恒等");
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_snapshot().unwrap().1),
        vec![PaneId(3), PaneId(1), PaneId(2)],
        "源树插到 pane1 左侧,其余分屏不动"
    );
    assert_eq!(focused, PaneId(3));
    assert!(!zoomed);
}

/// 落点消失退回:手势途中目标 pane 被关,松手必须 Err(source) 原样退回,
/// 源 tab 的 panes/树/身份一样不少。
#[gpui::test]
fn tab_drag_rolls_back_when_the_target_pane_disappears(cx: &mut TestAppContext) {
    let (workspace, mut window) = two_tabs_with_target_split(cx);
    let source_id = workspace.read_with(cx, |ws, _| ws.tab_id_at(1).unwrap());
    let pane2 = workspace.read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(2)).unwrap());
    let drop = pane2.center();

    let row = window
        .debug_bounds("sidebar-tab-1")
        .expect("源 tab 行必须绘制");
    window.simulate_mouse_down(row.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(drop, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("tab-dock-preview").is_some(),
        "悬停目标 pane 时必须已有落点"
    );

    // 手势途中关掉目标 pane(活跃 tab 的聚焦 pane = pane2):落点从树上消失。
    window.dispatch_action(CloseActivePane);
    window.run_until_parked();
    draw_frame(&mut window);
    let (panes, leaves, ..) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(panes, vec![PaneId(1)], "目标 pane 已关闭");
    assert_eq!(leaves, vec![PaneId(1)]);

    // 松手仍按缓存落点提交 → dock_at_leaf 返回 Err(source) → 原样退回。
    window.simulate_mouse_up(drop, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        2,
        "源 tab 必须退回"
    );
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 0);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_id_at(1)),
        Some(source_id),
        "稳定身份与位置原样保留"
    );
    let (panes, leaves) = workspace
        .read_with(cx, |ws, _| ws.tab_snapshot(1))
        .expect("源 tab 必须存活");
    assert_eq!(panes, vec![PaneId(3)]);
    assert_eq!(leaves, vec![PaneId(3)], "源树未被半提交");
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_snapshot(0).unwrap().1),
        vec![PaneId(1)],
        "目标 tab 保持关闭后的树"
    );
}

/// split_bounds 键修复:关闭 tab 清除其缓存键,幸存 tab 的键不随下标漂移;
/// 关 tab 后拖分隔条仍按幸存 tab 自己的上一帧矩形换算。
#[gpui::test]
fn closing_a_tab_purges_its_split_bounds_and_keeps_dragging_correct(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    draw_frame(&mut window);
    let first_id = workspace.read_with(cx, |ws, _| ws.tab_id_at(0).unwrap());

    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    draw_frame(&mut window);
    let second_id = workspace.read_with(cx, |ws, _| ws.tab_id_at(1).unwrap());
    let keys_before = workspace.read_with(cx, |ws, _| ws.split_bounds_keys());
    assert!(
        keys_before.iter().any(|(tab, _)| *tab == first_id)
            && keys_before.iter().any(|(tab, _)| *tab == second_id),
        "两个 tab 都应留下节点矩形键: {keys_before:?}"
    );

    // 关掉第一个 tab:幸存 tab 前移到下标 0。
    window.dispatch_action(SelectPreviousTab);
    window.run_until_parked();
    window.dispatch_action(CloseActiveTab);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let keys_after = workspace.read_with(cx, |ws, _| ws.split_bounds_keys());
    assert!(
        !keys_after.iter().any(|(tab, _)| *tab == first_id),
        "关闭 tab 的缓存键必须清除: {keys_after:?}"
    );
    assert!(
        keys_after.iter().any(|(tab, _)| *tab == second_id),
        "幸存 tab 的键随稳定身份保留"
    );

    // 幸存 tab 现处于下标 0;分隔条拖拽读的是它自己的矩形。
    let ratio_before = workspace.read_with(cx, |ws, _| ws.active_split_ratio().unwrap());
    let divider = window
        .debug_bounds("split-divider-0-[]")
        .expect("幸存 tab 的分隔条必须绘制");
    let start = divider.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = point(start.x + px(60.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    let ratio_after = workspace.read_with(cx, |ws, _| ws.active_split_ratio().unwrap());
    assert!(
        ratio_after > ratio_before + 0.01,
        "关 tab 后分隔条拖拽仍须生效: {ratio_before} -> {ratio_after}"
    );
}

/// 重排后键随 tab 走:TabId 键不随位置平移,重排前后键集合逐项相等。
#[gpui::test]
fn split_bounds_keys_follow_the_tab_identity_across_reorder(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    draw_frame(&mut window);
    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    draw_frame(&mut window);

    let keys_before = workspace.read_with(cx, |ws, _| ws.split_bounds_keys());
    let first_id = workspace.read_with(cx, |ws, _| ws.tab_id_at(0).unwrap());
    let second_id = workspace.read_with(cx, |ws, _| ws.tab_id_at(1).unwrap());

    workspace.update(cx, |ws, _| ws.move_tab_for_test(0, 1));
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_id_at(0)),
        Some(second_id),
        "重排后第一个位置是原第二个 tab"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_id_at(1)),
        Some(first_id)
    );
    let keys_after = workspace.read_with(cx, |ws, _| ws.split_bounds_keys());
    assert_eq!(
        keys_after, keys_before,
        "矩形缓存键随稳定 TabId 走,不随下标平移"
    );
}

/// M5.3a 问题 1:`-e` 启动的 tab 在建 tab 时把完整命令冻结进
/// `TabMeta.launch`(M5.5 快照/恢复按此重建)。
#[gpui::test]
fn launch_terminal_tab_freezes_the_full_command_into_meta(cx: &mut TestAppContext) {
    let command = Program::WithArgs {
        program: "slterm-test-missing-shell-executable".into(),
        args: vec!["--flag".into(), "two words".into()],
    };
    let (workspace, _window, _ai_tx) = open_workspace_with_command(cx, command.clone());

    let launch = workspace
        .read_with(cx, |ws, _| ws.active_tab_launch())
        .expect("建 tab 必须冻结启动身份");
    assert_eq!(
        launch,
        LaunchSession::Profile {
            name: "slterm-test-missing-shell-executable".into(),
            command: command.program().to_owned(),
            args: command.args().to_vec(),
            cwd: None,
            shell_id: None,
        }
    );
}

// ── M5.5 快照 / 恢复闭环 ──

/// 冷恢复夹具:两 tab、tab0 左右分屏、逐叶 cwd/launch/agent、侧栏双槽与
/// 窗口态全部写进磁盘 session.json(崩溃形态 `clean_exit=false`)。
fn write_crash_snapshot(path: &std::path::Path, cwd1: &str, cwd2: &str) {
    let profile = |cwd: &str| {
        serde_json::json!({
            "kind": "profile",
            "name": "missing-shell",
            "command": "slterm-test-missing-shell-executable",
            "args": [],
            "cwd": cwd
        })
    };
    let session = serde_json::json!({
        "version": 1,
        "boot_attempts": 0,
        "active_tab": 1,
        "tabs": [
            {
                "cwd": cwd2,
                "custom_name": "后端",
                "color": [97, 175, 239],
                "launch": profile(cwd1),
                "layout": {
                    "kind": "split",
                    "axis": "left_right",
                    "ratio_permille": 618,
                    "first": {
                        "kind": "pane",
                        "cwd": cwd1,
                        "agent": { "source": "claude", "session_id": "restored-session" },
                        "launch": profile(cwd1)
                    },
                    "second": {
                        "kind": "pane",
                        "cwd": cwd2,
                        "custom_name": "日志",
                        "launch": profile(cwd2)
                    }
                },
                "active_pane": 1
            },
            {
                "cwd": cwd2,
                "active_pane": 0,
                "layout": {
                    "kind": "pane",
                    "cwd": cwd2,
                    "launch": profile(cwd2)
                }
            }
        ],
        "window": { "width": 1024, "height": 640, "maximized": false },
        "sidebar": {
            "collapsed": false,
            "active_view_top": "explorer",
            "active_view_bottom": "agent_files",
            "width": 312.0
        },
        "clean_exit": false
    });
    std::fs::write(path, serde_json::to_string_pretty(&session).unwrap()).unwrap();
}

/// 「崩溃恢复 → 一秒内现场」E2E:崩溃快照 → 启动计划(计数/隔离判定)→
/// 逐叶 spawn 建树 → 首帧布局/tab/分屏/cwd/侧栏恢复 → 首拍自动保存归零
/// 断路器并原样续写现场。
#[gpui::test]
fn crashed_snapshot_restores_tabs_split_cwd_and_sidebar_within_one_tick(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cwd1 = dir.path().join("one");
    let cwd2 = dir.path().join("two");
    std::fs::create_dir_all(&cwd1).unwrap();
    std::fs::create_dir_all(&cwd2).unwrap();
    let cwd1 = cwd1.to_string_lossy().into_owned();
    let cwd2 = cwd2.to_string_lossy().into_owned();
    let path = dir.path().join("session.json");
    write_crash_snapshot(&path, &cwd1, &cwd2);

    let plan = prepare_startup_at(&path, true);
    let expected_tabs = plan.session.as_ref().expect("应恢复").tabs.clone();
    let (workspace, mut window, _ai) =
        open_workspace_with_startup(cx, WorkspaceStartup::NewTerminal { cwd: None }, plan);

    // 恢复前先计数落盘(断路器材料)。
    assert_eq!(
        load_from(&path).unwrap().boot_attempts,
        1,
        "恢复尝试必须先计数"
    );
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 2);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_index()),
        1,
        "会话里的活跃 tab 回放"
    );

    // panes == 树叶:tab0 两叶、tab1 单叶;DFS 序分配 pane id。
    let (panes0, leaves0) = workspace
        .read_with(cx, |ws, _| ws.tab_snapshot(0))
        .expect("tab0 必须恢复");
    assert_eq!(panes0, leaves0, "恢复路径 panes == 树叶");
    assert_eq!(leaves0, vec![PaneId(1), PaneId(2)]);
    let (panes1, leaves1) = workspace
        .read_with(cx, |ws, _| ws.tab_snapshot(1))
        .expect("tab1 必须恢复");
    assert_eq!(panes1, leaves1);
    assert_eq!(leaves1, vec![PaneId(3)]);

    // 逐叶 cwd 按 DFS 配对注入。
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.pane_cwd_for_test(0, 0, cx)),
        Some(cwd1.clone())
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.pane_cwd_for_test(0, 1, cx)),
        Some(cwd2.clone())
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.pane_cwd_for_test(1, 0, cx)),
        Some(cwd2.clone())
    );

    // 侧栏骨架态(D05-3)随现场恢复。
    let sidebar = workspace.read_with(cx, |ws, _| *ws.sidebar_state());
    assert_eq!(
        sidebar,
        SidebarState {
            collapsed: false,
            active_view_top: Some(SideViewId::Explorer),
            active_view_bottom: Some(SideViewId::AgentFiles),
            width: 312.0,
        }
    );

    // agent 恢复身份在 hook 确认前已进宿主(快照可续接)。
    let agent = workspace
        .read_with(cx, |ws, _| ws.agent_session_for_test(PaneId(1)))
        .expect("恢复的 agent 身份必须进宿主");
    assert_eq!(agent.source, "claude");
    assert_eq!(agent.session_id.as_deref(), Some("restored-session"));

    // 首帧:活跃 tab1 的 pane 绘制;切回 tab0 后两叶都有真实矩形。
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(window.debug_bounds("slterm-pane-3").is_some());
    window.dispatch_action(SelectPreviousTab);
    window.run_until_parked();
    draw_frame(&mut window);
    for selector in ["slterm-pane-1", "slterm-pane-2"] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制(恢复的树形生效)"
        );
    }

    // 一秒内现场:首拍自动保存(boot_attempts=0)与恢复的 tabs 字段级相等。
    let captured = Rc::new(RefCell::new(None));
    workspace.update(cx, |ws, _| {
        let captured = Rc::clone(&captured);
        ws.set_session_writer_for_test(Rc::new(move |session: &crate::session::Session| {
            *captured.borrow_mut() = Some(session.clone());
            Ok(())
        }));
    });
    workspace.update(cx, |ws, cx| ws.autosave_tick_for_test(cx));
    let saved = captured.borrow().clone().expect("首拍必须写盘");
    assert_eq!(saved.boot_attempts, 0, "首次成功快照归零断路器");
    assert_eq!(saved.tabs, expected_tabs, "恢复的头一秒即回到现场");
    assert_eq!(saved.sidebar, sidebar);
    assert!(saved.window.is_some(), "快照带窗口尺寸");
}

/// 快照 = 导出同 schema:pretty 导出文件经 `load_from` 回读字段级相等。
#[gpui::test]
fn export_workspace_is_the_snapshot_schema_read_back(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slterm-workspace.json");

    window.update(|window, cx| {
        workspace
            .read(cx)
            .export_workspace(&path, window, cx)
            .unwrap();
    });
    let exported = load_from(&path).expect("导出文件必须可回读");
    let snapshot = window.update(|window, cx| {
        workspace
            .read(cx)
            .snapshot_for_test(Some(capture_window_state(window)), cx)
    });
    assert_eq!(exported, snapshot, "导出与快照字段级相等");
}

/// D05-1:窗口尺寸/最大化态进快照,并在窗口定形前原样回放。
#[gpui::test]
fn window_state_persists_and_replays_before_window_creation(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let state = window.update(|window, _| capture_window_state(window));
    assert!(state.width > 0 && state.height > 0);
    let snapshot = workspace.read_with(cx, |ws, cx| ws.snapshot_for_test(Some(state), cx));
    assert_eq!(snapshot.window, Some(state), "窗口态随快照持久化");

    let replay = WindowState {
        width: 1024,
        height: 640,
        maximized: true,
    };
    let options =
        cx.update(|cx| workspace_window_options(cx, false, Some(replay), &SidebarState::default()));
    let bounds = options.window_bounds.expect("窗口定形必须有 bounds");
    let gpui::WindowBounds::Maximized(bounds) = bounds else {
        panic!("最大化态必须在定形前回放");
    };
    assert_eq!(bounds.size, gpui::size(px(1024.0), px(640.0)));

    let options =
        cx.update(|cx| workspace_window_options(cx, false, None, &SidebarState::default()));
    assert!(matches!(
        options.window_bounds,
        Some(gpui::WindowBounds::Windowed(_))
    ));
}

/// 恢复注入:AgentSession → resume 命令写进 PTY;异形 id 不上屏。
#[gpui::test]
fn restored_agent_command_is_injected_and_hostile_ids_are_rejected(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    let view = workspace.read_with(cx, |ws, _| ws.first_pane_view().unwrap());
    let receiver = view.update(cx, |view, _| {
        let (session, receiver) = crate::gpui_terminal::session::test_session();
        view.session = Some(session);
        receiver
    });

    let agent = AgentSession {
        source: "claude".into(),
        session_id: Some("abc-123".into()),
        session_file: None,
    };
    view.update(cx, |view, cx| view.restore_agent(&agent, cx));
    match receiver.try_recv().expect("恢复命令必须写进 PTY") {
        EventLoopMsg::Input(bytes) => {
            assert_eq!(&*bytes, b"claude --resume abc-123\r");
        }
        other => panic!("unexpected message: {other:?}"),
    }

    let hostile = AgentSession {
        source: "claude".into(),
        session_id: Some("abc; calc".into()),
        session_file: None,
    };
    view.update(cx, |view, cx| view.restore_agent(&hostile, cx));
    assert!(receiver.try_recv().is_err(), "异形 id 不得写进 shell");
    assert!(
        view.read_with(cx, |view, _| view.pending_agent_command().is_none()),
        "拒收后不保留待提交命令"
    );
}
