//! 应用内通知保留面：进程级错误与警告独立于系统 toast 与桌面开关。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;

use super::AppRoot;
use crate::notify::{AlertSeverity, IN_APP_NOTICE_LIMIT};

impl AppRoot {
    /// 事件泵排出新增通知；同一文本与严重度只保留一份，超出上限淘汰最旧。
    pub(super) fn poll_in_app_notices(&mut self, cx: &mut Context<'_, Self>) {
        let incoming = crate::notify::drain_in_app_notices();
        if incoming.is_empty() {
            return;
        }
        for notice in incoming {
            if self.in_app_notices.iter().any(|existing| {
                existing.text == notice.text && existing.severity == notice.severity
            }) {
                continue;
            }
            self.in_app_notices.push(notice);
        }
        while self.in_app_notices.len() > IN_APP_NOTICE_LIMIT {
            self.in_app_notices.remove(0);
        }
        cx.notify();
    }

    /// 用户关闭一条通知；这是错误态的唯一清除入口。
    pub(super) fn dismiss_in_app_notice(&mut self, id: u64, cx: &mut Context<'_, Self>) {
        let before = self.in_app_notices.len();
        self.in_app_notices.retain(|notice| notice.id != id);
        if self.in_app_notices.len() != before {
            cx.notify();
        }
    }

    /// 顶部居中通知条；模态面与关闭决定面仍在其上。
    pub(super) fn render_in_app_notices(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        if self.in_app_notices.is_empty() {
            return None;
        }
        Some(
            div()
                .absolute()
                .top(px(8.0))
                .left(px(0.0))
                .right(px(0.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .debug_selector(|| "in-app-notices".to_owned())
                .children(self.in_app_notices.iter().map(|notice| {
                    let (color, background) = match notice.severity {
                        AlertSeverity::Error => {
                            (cx.theme().danger, cx.theme().danger.opacity(0.12))
                        }
                        AlertSeverity::Warning => {
                            (cx.theme().warning, cx.theme().warning.opacity(0.12))
                        }
                        AlertSeverity::Info => {
                            (cx.theme().foreground, cx.theme().border.opacity(0.5))
                        }
                    };
                    let id = notice.id;
                    div()
                        .id(("in-app-notice", id))
                        .max_w(px(560.0))
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(color.opacity(0.5))
                        .bg(background)
                        .debug_selector(|| "in-app-notice".to_owned())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_color(color)
                                .child(notice.text.clone()),
                        )
                        .child(
                            div()
                                .id(("in-app-notice-dismiss", id))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .debug_selector(|| "in-app-notice-dismiss".to_owned())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |root, _, _, cx| {
                                        root.dismiss_in_app_notice(id, cx)
                                    }),
                                )
                                .child("×"),
                        )
                }))
                .into_any_element(),
        )
    }
}
