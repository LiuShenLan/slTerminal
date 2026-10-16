//! `SltermWorkspace`(13 篇锚点;05 篇不变式的宿主)。M3.1 = 冒烟挂载的最简 stub。
//!
//! M3.2 回填(字段语义与 05 篇终态严格一致,扩不改):tabs/active/
//! next_pane/next_tab 单调分配、initial_grid、pane_bounds/split_bounds 矩形回写
//! 机制、new_pane/on_terminal_event、壳事件泵(120ms drain)。

use std::sync::mpsc;

use gpui::{App, Context, IntoElement, Render, Styled as _, Window, div};
use gpui_component::ActiveTheme as _;

use super::SltermShellEvent;

/// 壳内状态权威雏形:M3.2 起直持 pane 实体与展示态(无投影层,13 篇改造节 2)。
pub struct SltermWorkspace {
    /// 壳事件队列接收端;事件泵接线归 M3.2+(M3.1 只持有,防通道发送端断)。
    #[allow(dead_code)]
    shell_rx: mpsc::Receiver<SltermShellEvent>,
}

impl SltermWorkspace {
    pub(crate) fn new(
        _window: &mut Window,
        shell_rx: mpsc::Receiver<SltermShellEvent>,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        Self { shell_rx }
    }
}

impl Render for SltermWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // 终端 pane 归 M3.2;M3.1 以主题底色撑满,验证暗色装载与渲染闭环。
        div().size_full().bg(cx.theme().background)
    }
}

/// workspace 级键绑定:M3 只注册窗口/复制族(13 篇改造节 3),归 M3.3/M5 接线;
/// M3.1 为空——空绑定不等于无纪律,绑定面以改造节 3 为唯一权威。
pub fn init(_cx: &mut App) {}
