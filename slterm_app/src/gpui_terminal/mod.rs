//! 终端垂直切片根(模块名映射:上游 gpui_shell/terminal → gpui_terminal,
//! 层级压平一格)。M3.1 仅立骨架与边界注释;session/element/view 归 M3.2,
//! keymap/IME/鼠标族归 M3.3。
//!
//! 边界(13 篇不可变更项 2):本模块之外不出现 `slterm_terminal` 类型;
//! `slterm_terminal` 之内不出现 GPUI 类型。

pub mod colors;

/// Tab/Shift-Tab 绑定与 `KEY_CONTEXT`("SltermTerminal")归 M3.3 随 keymap 随迁
/// (上游 terminal/mod.rs `init` + 遮蔽测试:终端必须先于组件库 Root 接住 Tab,
/// 否则补齐后光标失焦);冒烟不需要,先立骨架。
pub fn init(_cx: &mut gpui::App) {}
