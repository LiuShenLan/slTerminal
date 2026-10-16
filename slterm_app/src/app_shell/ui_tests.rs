//! 虚拟窗口设施冒烟(M3 开工第一步,C-9 闭环):feature 链开窗 + Root 挂载 +
//! draw 不 panic。钉版/feature 形态与设计假设不符(虚拟窗口补丁须对不存在的
//! 原生句柄回 `Unavailable` 而非 panic)时本例起不来——起不来先查钉版与
//! feature 接线,不要先动断言。

use std::sync::mpsc;

use gpui::{AppContext, TestAppContext};
use gpui_component::Root;

use super::workspace::SltermWorkspace;

#[gpui::test]
fn shell_window_mounts_root_and_draws(cx: &mut TestAppContext) {
    // 与生产 init 同序的测试子集(11 篇模板同形):内嵌 Maple 在测试文本系统
    // 不栅格化真实字形,20MB 注册纯属拖慢,冒烟不挂。
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(super::config::Settings::load());
        super::theme::apply_dark_theme(cx);
        super::workspace::init(cx);
    });
    let (_tx, rx) = mpsc::channel();
    let (_root, window) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| SltermWorkspace::new(window, rx, cx));
        Root::new(workspace, window, cx)
    });
    window.run_until_parked();
    window.refresh().unwrap();
    // 真实绘制一帧:不 panic 即通过;clear 收尾是 arena 生命周期纪律。
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}
