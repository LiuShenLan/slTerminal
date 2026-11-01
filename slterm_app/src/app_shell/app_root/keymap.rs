//! AppRoot 的默认键位。

use gpui::{App, KeyBinding};

use super::{
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, ActivateSideButton,
    CloseActiveTerminal, FocusActivityBar, FocusFirstSideButton, FocusLastSideButton,
    FocusNextSideButton, FocusPreviousSideButton, NewTerminal, OpenSideButtonMenu, SplitDown,
    SplitRight, ToggleSidebar,
};

/// 注册运行根默认键位；用户绑定后续统一接入 CommandRegistry。
pub(crate) fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-t", NewTerminal, None),
        KeyBinding::new("ctrl-shift-d", SplitRight, None),
        KeyBinding::new("ctrl-shift-s", SplitDown, None),
        KeyBinding::new("ctrl-shift-w", CloseActiveTerminal, None),
        KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-shift-n", ActivateNav, None),
        KeyBinding::new("ctrl-shift-e", ActivateExplorer, None),
        KeyBinding::new("ctrl-shift-g", ActivateCommit, None),
        KeyBinding::new("ctrl-shift-a", ActivateAgentFiles, None),
        KeyBinding::new("ctrl-shift-l", FocusActivityBar, None),
        KeyBinding::new(
            "down",
            FocusNextSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "up",
            FocusPreviousSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "home",
            FocusFirstSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "end",
            FocusLastSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "shift-f10",
            OpenSideButtonMenu,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "enter",
            ActivateSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "space",
            ActivateSideButton,
            Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        ),
    ]);
}
