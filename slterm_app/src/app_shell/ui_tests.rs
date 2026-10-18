//! 虚拟窗口设施测试:feature 链开窗 + Root 挂载 + 真实绘制一帧。
//!
//! 钉版/feature 形态与设计假设不符(虚拟窗口补丁须对不存在的原生句柄回
//! `Unavailable` 而非 panic)时本组起不来——起不来先查钉版与 feature 接线,
//! 不要先动断言。

use std::sync::mpsc;
use std::time::Duration;

use gpui::{AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use gpui_component::Root;
use slterm_split::PaneId;

use super::windowing::WorkspaceStartup;
use super::workspace::{CloseActivePane, SltermWorkspace, SplitRight, ToggleZoom};
use crate::ai_hook::AiHookEvent;

/// 与生产 init 同序的测试子集(11 篇模板同形):内嵌 Maple 在测试文本系统
/// 不栅格化真实字形,20MB 注册纯属拖慢,测试不挂。
fn bootstrap(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(super::config::Settings::load());
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
    bootstrap(cx);
    let (_tx, rx) = mpsc::channel();
    let (ai_tx, ai_rx) = mpsc::channel();
    let mut slot = None;
    let (_root, window) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| {
            SltermWorkspace::new(
                window,
                rx,
                Some(ai_rx),
                WorkspaceStartup::LaunchTerminal {
                    cwd: None,
                    command: crate::cli::Program::Just(
                        "slterm-test-missing-shell-executable".into(),
                    ),
                },
                cx,
            )
        });
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
