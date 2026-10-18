//! 暗色单主题装载(仅暗色定位约束;主题系统归设置主题体系,本模块只保一份内置暗色)。
//!
//! 裁剪口径:上游 theme.rs 的 ResolvedTheme/Skin token 覆写族、亮暗跟随全部
//! 归设置主题体系;本模块只把组件库切到暗色模式垫底,并落壳色
//! 透明度纪律——透明度只乘壳底色,文字前景一个字节都不改。

use gpui::{App, Hsla};
use gpui_component::{Theme, ThemeMode};

/// 装载内置暗色主题,并同步刷新视效(模糊/透明度/壁纸)。
///
/// 顺序固定:先 `Theme::change` 重置组件库 token,再 `wallpaper::refresh`
/// 落视效与壳色 alpha。`refresh` 不触碰窗口可见性,静默启动形态不受影响。
pub(crate) fn apply_dark_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    crate::app_shell::wallpaper::refresh(cx);
}

/// 按当前不透明度重算壳色 token。
///
/// 只改「壳底色」五项(背景/侧栏/标题栏及其分隔线);`foreground` 与其余
/// 文字 token 不动——全窗 alpha 会让文字对比度塌掉(壳色透明度纪律)。
///
/// 每次重算都把基线 alpha 归一到 1 再乘用户值,因此高频滑块拖动与异步
/// 壁纸就绪回调可以重复调用而不会层层相乘。
pub(crate) fn reapply_shell_opacity(cx: &mut App) {
    if !cx.has_global::<Theme>() {
        return;
    }
    let opacity = crate::app_shell::wallpaper::chrome_surface_opacity(cx);
    let theme = Theme::global_mut(cx);
    let dim = |color: Hsla| {
        let mut color = color;
        color.a = 1.0;
        color.a *= opacity;
        color
    };
    let background = dim(theme.background);
    let sidebar = dim(theme.sidebar);
    let sidebar_border = dim(theme.sidebar_border);
    let title_bar = dim(theme.title_bar);
    let title_bar_border = dim(theme.title_bar_border);
    theme.background = background;
    theme.sidebar = sidebar;
    theme.sidebar_border = sidebar_border;
    theme.title_bar = title_bar;
    theme.title_bar_border = title_bar_border;
    // gpui-component 1.16 的壳组件读 resolved tokens;直接改 ThemeColor
    // 不会自动同步,故五个壳 token 必须同步写。
    theme.tokens.background = background.into();
    theme.tokens.sidebar = sidebar.into();
    theme.tokens.sidebar_border = sidebar_border.into();
    theme.tokens.title_bar = title_bar.into();
    theme.tokens.title_bar_border = title_bar_border.into();
}
