//! 终端垂直切片根(模块名映射:上游 gpui_shell/terminal → gpui_terminal,
//! 层级压平一格)。
//!
//! 边界(不可变更项 2):本模块之外不出现 `slterm_terminal` 类型;
//! `slterm_terminal` 之内不出现 GPUI 类型。唯一例外缝:app_shell 经
//! `session::local_options` 的 `tty::Options`/`tty::Shell` 参数面接触
//! (锚定的 seam)。

pub mod colors;
pub(crate) mod copy_feedback;
pub(crate) mod cursor_painter;
pub(crate) mod element;
pub(crate) mod event_mailbox;
pub(crate) mod keymap;
/// 公式覆盖层的 term 锁内扫描/绘制计划与 paint pass。
pub(crate) mod math_overlay;
/// 公式位图的 GPUI 呈现(有界缓存 + 基线吸附)。
pub(crate) mod math_render;
pub(crate) mod mouse_protocol;
pub(crate) mod session;
pub(crate) mod session_pump;
pub(crate) mod view;

use gpui::{App, KeyBinding};

// 终端按键上下文与 Tab 动作(上游 terminal/mod.rs 同构):终端必须先于
// 组件库 Root 的焦点遍历接住 Tab,否则补齐/菜单场景下光标失焦。
gpui::actions!(slterm_terminal, [TerminalTab, TerminalBackTab]);

pub(crate) const KEY_CONTEXT: &str = "SltermTerminal";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", TerminalTab, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-tab", TerminalBackTab, Some(KEY_CONTEXT)),
    ]);
}

#[cfg(test)]
mod key_binding_tests {
    use super::{KEY_CONTEXT, TerminalTab};
    use gpui::{KeyBinding, KeyContext, Keymap, Keystroke};

    gpui::actions!(test_root, [RootTab]);

    /// 终端上下文里的 Tab 绑定必须先于组件库 Root 的焦点遍历被解析——
    /// 否则 Tab 会把焦点挪走而不是送进 PTY。
    #[test]
    fn terminal_tab_binding_shadows_root_focus_traversal() {
        let keymap = Keymap::new(vec![
            KeyBinding::new("tab", RootTab, Some("Root")),
            KeyBinding::new("tab", TerminalTab, Some(KEY_CONTEXT)),
        ]);
        let contexts = [
            KeyContext::parse("Root").unwrap(),
            KeyContext::parse(KEY_CONTEXT).unwrap(),
        ];
        let input = [Keystroke::parse("tab").unwrap()];

        let (bindings, pending) = keymap.bindings_for_input(&input, &contexts);

        assert!(!pending);
        assert!(bindings[0].action().as_any().is::<TerminalTab>());
    }
}
