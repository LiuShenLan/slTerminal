//! 设置页视觉 token 与行原语:左列标签/说明,右列控件垂直居中。
//!
//! 分组靠留白与标题层级表达,不画横线;行宽上限由页面容器给,控件列
//! 固定宽度并在内部右对齐,标签与值不会随窗口宽度拉开成两行。

use gpui::prelude::FluentBuilder as _;
use gpui::{Context, FontWeight, IntoElement, ParentElement as _, Styled as _, div, px};

use super::SettingsPane;
use gpui_component::ActiveTheme as _;

/// 导航列宽度(逻辑像素)。
pub(crate) const SETTINGS_NAV_WIDTH: f32 = 248.0;
/// 导航行高。
pub(crate) const SETTINGS_NAV_ROW_HEIGHT: f32 = 34.0;
/// 导航图标尺寸。
pub(crate) const SETTINGS_NAV_ICON_SIZE: f32 = 16.0;
/// 正文分组间距。
pub(crate) const SETTINGS_GROUP_GAP: f32 = 32.0;
/// 设置行垂直内边距。
pub(crate) const SETTINGS_ROW_PAD_Y: f32 = 12.0;
/// 标准选择器宽度。
pub(crate) const SETTINGS_SELECT_WIDTH: f32 = 220.0;
/// 文字列最小宽度(控件列右缘对齐的前提)。
pub(crate) const SETTINGS_TEXT_COL_MIN_W: f32 = 320.0;
/// 控件列宽(容纳 220 的下拉,开关在列内右对齐)。
pub(crate) const SETTINGS_CTRL_COL_W: f32 = 232.0;
/// 说明文字相对正文字号的比例。
const SETTINGS_DESC_SCALE: f32 = 0.82;

impl SettingsPane {
    /// 一组设置的开头:标题行。组间距归页面容器的 `gap`,组不自带
    /// 上留白(否则首组会拿到双份、非分组页会贴页头)。
    pub(crate) fn group(&self, title: &'static str, cx: &Context<'_, Self>) -> gpui::Div {
        div().w_full().child(
            div()
                .pb(px(12.0))
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
    }

    /// 设置行:左列名称 + 说明,右列控件。
    pub(crate) fn row(
        &self,
        label: &'static str,
        desc: &'static str,
        control: impl IntoElement,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_4()
            .py(px(SETTINGS_ROW_PAD_Y))
            .child(
                div()
                    .flex_1()
                    .min_w(px(SETTINGS_TEXT_COL_MIN_W))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(label),
                    )
                    .when(!desc.is_empty(), |column| {
                        column.child(
                            div()
                                .mt(px(4.0))
                                .text_size(px(13.0 * SETTINGS_DESC_SCALE))
                                .text_color(theme.muted_foreground)
                                .child(desc),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(SETTINGS_CTRL_COL_W))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .items_center()
                    .child(control),
            )
    }
}
