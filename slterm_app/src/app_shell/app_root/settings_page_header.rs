//! 设置页头：恢复本页默认与手动检查更新入口。
//!
//! 页头只构造命令请求；恢复默认与更新检查的联网、结算都归 AppRoot。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, div,
};
use gpui_component::ActiveTheme as _;

use super::settings_overlay::SettingsView;
use crate::settings_state::SettingsPageId;

impl SettingsView {
    /// 页头按页选择动作：检查更新只出现在「应用」页。
    pub(super) fn render_page_header(
        &self,
        page: SettingsPageId,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let reset_root = self.root.clone();
        let reset = div()
            .id("settings-reset-page")
            .debug_selector(|| "settings-reset-page".to_owned())
            .pb_1()
            .px_2()
            .text_xs()
            .cursor_pointer()
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_view, _, _, cx| {
                    let _ = reset_root.update(cx, |root, cx| {
                        root.restore_settings_page_defaults(page, cx);
                    });
                }),
            )
            .child(language.text(crate::i18n::Message::SettingsResetPage));
        let mut row = div().flex().flex_row().items_center().gap_3().child(reset);
        if page == SettingsPageId::Application {
            let backup_root = self.root.clone();
            row = row.child(
                div()
                    .id("settings-open-backup")
                    .debug_selector(|| "settings-open-backup".to_owned())
                    .pb_1()
                    .px_2()
                    .text_xs()
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_view, _, window, cx| {
                            let _ = backup_root.update(cx, |root, cx| {
                                if let Err(error) = root.open_backup_dialog(window, cx) {
                                    log::warn!("备份对话框打开失败: {error:?}");
                                }
                            });
                        }),
                    )
                    .child(language.text(crate::i18n::Message::BackupOpen)),
            );
            let check_root = self.root.clone();
            row = row.child(
                div()
                    .id("settings-check-updates")
                    .debug_selector(|| "settings-check-updates".to_owned())
                    .pb_1()
                    .px_2()
                    .text_xs()
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_view, _, window, cx| {
                            let _ = check_root.update(cx, |root, cx| {
                                root.check_for_updates_now(window, cx);
                            });
                        }),
                    )
                    .child(language.text(crate::i18n::Message::UpdateCheckNow)),
            );
        }
        row.into_any_element()
    }
}
