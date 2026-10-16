//! 暗色单主题装载(仅暗色定位约束;13 篇:主题系统归 06 篇,M3 只保一份内置暗色)。
//!
//! 裁剪口径:上游 theme.rs 的 ResolvedTheme/Skin token 覆写族、亮暗跟随、
//! 壁纸/透明度联动全部归 06 篇;M3.1 只把组件库切到暗色模式垫底。

use gpui::App;
use gpui_component::{Theme, ThemeMode};

/// 装载内置暗色主题:组件库 token 全量落暗色底色系。
/// 无主题切换、无亮暗跟随;终端 palette 的暗色默认在 `gpui_terminal::colors`。
pub(crate) fn apply_dark_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
}
