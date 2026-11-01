//! AppRoot 的 Terminal-only 默认键位。

use gpui::{App, KeyBinding};

use super::{CloseActiveTerminal, NewTerminal, SplitDown, SplitRight};

/// 注册运行根默认键位；用户绑定后续统一接入 CommandRegistry。
#[allow(dead_code)] // 生产窗口切换时消费。
pub(crate) fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-t", NewTerminal, None),
        KeyBinding::new("ctrl-shift-d", SplitRight, None),
        KeyBinding::new("ctrl-shift-s", SplitDown, None),
        KeyBinding::new("ctrl-shift-w", CloseActiveTerminal, None),
    ]);
}
