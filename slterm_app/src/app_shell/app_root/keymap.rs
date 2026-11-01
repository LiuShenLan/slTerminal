//! AppRoot 的默认键位。

use gpui::{App, KeyBinding};

use super::command_registry::CommandRegistry;
use super::{
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, ActivateSideButton,
    AdjustSidebarRatioDown, AdjustSidebarRatioDownLarge, AdjustSidebarRatioUp,
    AdjustSidebarRatioUpLarge, AdjustSidebarWidthLeft, AdjustSidebarWidthLeftLarge,
    AdjustSidebarWidthRight, AdjustSidebarWidthRightLarge, FocusActivityBar, FocusFirstSideButton,
    FocusLastSideButton, FocusNextSideButton, FocusPreviousSideButton, OpenSideButtonMenu,
    ResetSidebarRatio, ResetSidebarWidth, ToggleSidebar,
};

/// 注册运行根默认键位；终端命令与其执行目录共用定义。
pub(crate) fn register(cx: &mut App) {
    CommandRegistry::register_terminal_bindings(cx);
    cx.bind_keys([
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
        KeyBinding::new(
            "left",
            AdjustSidebarWidthLeft,
            Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "right",
            AdjustSidebarWidthRight,
            Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "shift-left",
            AdjustSidebarWidthLeftLarge,
            Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "shift-right",
            AdjustSidebarWidthRightLarge,
            Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "enter",
            ResetSidebarWidth,
            Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "up",
            AdjustSidebarRatioUp,
            Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "down",
            AdjustSidebarRatioDown,
            Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "shift-up",
            AdjustSidebarRatioUpLarge,
            Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "shift-down",
            AdjustSidebarRatioDownLarge,
            Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        ),
        KeyBinding::new(
            "enter",
            ResetSidebarRatio,
            Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        ),
    ]);
}
