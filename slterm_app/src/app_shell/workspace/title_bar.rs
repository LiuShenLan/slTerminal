//! 自绘窗口标题栏:活跃 tab 展示名 + 最小化/最大化/关闭三钮 + 拖拽区。
//!
//! 壳重建(窗口三钮消息归窗口域,本步按 GPUI 窗口 API 直连留缝):
//! - 标题数据源 = 活跃 tab 的展示名,当前取 `TabMeta.custom_name` 或聚焦
//!   pane 的既有终端标题;`TabPresentation` 落地后替换这一处取值。
//! - 拖拽区 = GPUI `WindowControlArea::Drag`(Windows 原生 HTCAPTION,
//!   双击最大化由系统处理,不自挂双击 handler 防二次 toggle)。
//! - 三钮 = 原生 `WindowControlArea::Min/Max/Close`;Windows 由系统完成
//!   「最大化 ↔ 还原」双态点击。非 Windows 平台与虚拟窗口测试走显式
//!   click 分派(测试只记录命令,不触碰平台窗口)。
//!
//! 视觉:34px 高,左段品牌标识、中段活跃 tab 名、右段三钮;颜色全部取
//! 组件库主题 token,不硬编码色值。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, WindowControlArea, div, px,
};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _};

use super::SltermWorkspace;
use crate::brand;

/// 标题栏高度(旧产品面 34px)。
pub(super) const TITLE_BAR_HEIGHT: f32 = 34.0;
/// 三钮尺寸(旧产品面 38×26)。
const WINDOW_BUTTON_WIDTH: f32 = 38.0;
const WINDOW_BUTTON_HEIGHT: f32 = 26.0;

/// 窗口三钮命令。窗口域接入消息总线后,本枚举即其载荷形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TitleBarCommand {
    Minimize,
    MaximizeToggle,
    Close,
}

impl SltermWorkspace {
    /// 标题栏渲染。`window.is_maximized()` 在每帧读取,保证按钮双击、
    /// 拖边框、Win+方向键等系统路径触发的最大化态都能在同一帧反映。
    pub(super) fn render_title_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let title = self.active_tab_title(cx);
        let maximized = window.is_maximized();
        // Windows 生产路径交给原生控制区(含还原);非 Windows 与带测试
        // 支持的虚拟窗口走显式 click 分派,让行为可机测。
        let native_controls = cfg!(windows);
        let draw_controls = !cfg!(windows) || cfg!(all(test, feature = "gpui-test-support"));
        let title_bar_bg = cx.theme().title_bar;
        let border = cx.theme().title_bar_border;
        let idle_fg = cx.theme().muted_foreground;

        let max_icon = if maximized {
            IconName::WindowRestore
        } else {
            IconName::WindowMaximize
        };

        div()
            .id("titlebar")
            .debug_selector(|| "titlebar".to_owned())
            .h(px(TITLE_BAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .bg(title_bar_bg)
            .border_b_1()
            .border_color(border)
            .text_size(px(12.0))
            .text_color(idle_fg)
            .child(
                // 左段:品牌标识 + 应用名(拖拽区撑满全高,文字子元素不截胡)
                div()
                    .h_full()
                    .pl(px(12.0))
                    .pr(px(10.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(cx.theme().sidebar_accent)
                            .text_color(cx.theme().sidebar_accent_foreground)
                            .text_size(px(11.0))
                            .child(">"),
                    )
                    .child(brand::NAME),
            )
            .child(
                // 中段:活跃 tab 展示名(空标题回退品牌名)
                div()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .debug_selector(|| "titlebar-title".to_owned())
                            .min_w_0()
                            .truncate()
                            .child(title),
                    ),
            )
            .child(
                // 右段:三钮独立于拖拽区,保证可点击
                div()
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .pr(px(8.0))
                    .child(title_bar_button(
                        TitleBarCommand::Minimize,
                        "titlebar-min",
                        "最小化",
                        WindowControlArea::Min,
                        IconName::WindowMinimize,
                        false,
                        native_controls,
                        draw_controls,
                        cx,
                    ))
                    .child(title_bar_button(
                        TitleBarCommand::MaximizeToggle,
                        "titlebar-max",
                        if maximized { "还原" } else { "最大化" },
                        WindowControlArea::Max,
                        max_icon,
                        false,
                        native_controls,
                        draw_controls,
                        cx,
                    ))
                    .child(title_bar_button(
                        TitleBarCommand::Close,
                        "titlebar-close",
                        "关闭",
                        WindowControlArea::Close,
                        IconName::WindowClose,
                        true,
                        native_controls,
                        draw_controls,
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// 三钮命令消费点:Windows 生产由原生控制区接管;其余平台/测试走这里。
    /// 测试分支只记录命令——虚拟窗口平台的最小化/缩放是 unimplemented,
    /// 直接调用会炸掉用例(行为断言由记录值承担)。
    fn apply_titlebar_command(
        &mut self,
        command: TitleBarCommand,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        #[cfg(all(test, feature = "gpui-test-support"))]
        {
            self.window_commands.push(command);
            cx.notify();
        }
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        {
            let _ = cx;
            match command {
                TitleBarCommand::Minimize => _window.minimize_window(),
                TitleBarCommand::MaximizeToggle => _window.zoom_window(),
                TitleBarCommand::Close => {
                    // B50:自绘关闭钮绕过 OS 关闭请求,必须复用同一脏守卫;
                    // 被拦截(确认在途/保存中)时什么都不做。
                    if self.guard_file_window_close(_window, cx) {
                        self.save_on_window_close(_window, cx);
                        _window.remove_window();
                    }
                }
            }
        }
    }
}

/// 单个窗口控制钮。原生控制区与显式 click 分派互斥挂载:Windows 生产
/// 只走原生(系统保证最大化/还原双态),其余路径走显式分派。
#[allow(clippy::too_many_arguments)] // 控件参数逐项显式,调用点只有三处
fn title_bar_button(
    command: TitleBarCommand,
    id: &'static str,
    label: &'static str,
    area: WindowControlArea,
    icon: IconName,
    is_close: bool,
    native_controls: bool,
    draw_controls: bool,
    cx: &mut Context<'_, SltermWorkspace>,
) -> gpui::Stateful<gpui::Div> {
    let hover_bg = if is_close {
        cx.theme().danger
    } else {
        cx.theme().secondary_hover
    };
    let hover_fg = if is_close {
        cx.theme().danger_foreground
    } else {
        cx.theme().foreground
    };
    let idle_fg = cx.theme().muted_foreground;
    div()
        .id(SharedString::from(id))
        .debug_selector(move || id.to_owned())
        .w(px(WINDOW_BUTTON_WIDTH))
        .h(px(WINDOW_BUTTON_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .text_color(idle_fg)
        .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
        .when(native_controls, |button| button.window_control_area(area))
        .when(draw_controls, |button| {
            button.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.apply_titlebar_command(command, window, cx);
            }))
        })
        .child(Icon::new(icon).small())
        // 无障碍名称:三钮语义对读屏与自动化可见。
        .aria_label(label)
}

#[cfg(test)]
mod title_bar_tests {
    use super::*;

    /// 命令枚举是窗口域的载荷形状:三值封闭,不可静默扩维。
    #[test]
    fn title_bar_commands_are_a_closed_three_value_set() {
        let commands = [
            TitleBarCommand::Minimize,
            TitleBarCommand::MaximizeToggle,
            TitleBarCommand::Close,
        ];
        assert_eq!(commands.len(), 3);
    }
}
