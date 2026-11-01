//! AppRoot 的默认键位。

use gpui::App;

use super::command_registry::CommandRegistry;

/// 注册运行根默认键位；所有命令与执行目录共用定义。
pub(crate) fn register(cx: &mut App) {
    CommandRegistry::register_bindings(cx);
}
