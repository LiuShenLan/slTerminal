//! 各分区页:只摆控件,不定义设置语义。所有改动经统一写通道
//! (`SettingsPane::try_persist`)落盘并重载。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::select::Select;
use gpui_component::slider::Slider;
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

use crate::i18n::Message;

use super::SettingsPane;
use super::design::{SETTINGS_GROUP_GAP, SETTINGS_SELECT_WIDTH};

impl SettingsPane {
    pub(super) fn select_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = match self.selects.iter().find(|entry| entry.key == key) {
            Some(entry) => div()
                .debug_selector(move || format!("settings-select-{key}"))
                .w(px(SETTINGS_SELECT_WIDTH))
                .child(
                    Select::new(&entry.select)
                        .appearance(false)
                        .h_full()
                        .rounded(px(6.0)),
                )
                .into_any_element(),
            None => div().into_any_element(),
        };
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn switch_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        checked: bool,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = div()
            .debug_selector(move || format!("settings-switch-{key}"))
            .child(Switch::new(key).checked(checked).on_click(cx.listener(
                move |this, checked: &bool, window, cx| {
                    this.toggle(key, *checked, window, cx);
                },
            )))
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn input_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        input: &gpui::Entity<InputState>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let state = input.clone();
        let control = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                div()
                    .debug_selector(move || format!("settings-input-{key}"))
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(input)),
            )
            .child(
                Button::new(SharedString::from(format!("settings-save-{key}")))
                    .label(self.language(cx).text(Message::CommonSave))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let value = state.read(cx).value().to_string();
                        this.save_text_input(key, value, window, cx);
                    })),
            )
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn slider_row(
        &self,
        label: &'static str,
        desc: &'static str,
        debug_id: &'static str,
        slider: &gpui::Entity<gpui_component::slider::SliderState>,
        display: SharedString,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = h_flex()
            .w(px(SETTINGS_SELECT_WIDTH))
            .items_center()
            .gap_3()
            .child(
                div()
                    .debug_selector(move || debug_id.to_owned())
                    .flex_1()
                    .min_w_0()
                    .child(Slider::new(slider)),
            )
            .child(
                div()
                    .w(px(48.0))
                    .flex_shrink_0()
                    .text_color(cx.theme().muted_foreground)
                    .child(display),
            )
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn page_appearance(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.text(Message::SettingsAppearanceFonts), cx)
                    .child(self.input_row(
                        "font_family",
                        language.text(Message::SettingsAppearanceFontFamily),
                        language.text(Message::SettingsAppearanceFontFamilyDescription),
                        &self.font_family_input,
                        cx,
                    ))
                    .child(self.input_row(
                        "font_size",
                        language.text(Message::SettingsAppearanceFontSize),
                        language.text(Message::SettingsAppearanceFontSizeDescription),
                        &self.font_size_input,
                        cx,
                    ))
                    .child(self.select_row(
                        "ligatures",
                        language.text(Message::SettingsAppearanceLigatures),
                        language.text(Message::SettingsAppearanceLigaturesDescription),
                        cx,
                    ))
                    .child(self.select_row(
                        "cell_width_mode",
                        language.text(Message::SettingsAppearanceCellWidth),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "theme",
                        language.text(Message::SettingsAppearanceTheme),
                        language.text(Message::SettingsAppearanceThemeDescription),
                        cx,
                    ))
                    .child(
                        self.row(
                            language.text(Message::SettingsAppearanceCustomTheme),
                            language.text(Message::SettingsAppearanceCustomThemeDescription),
                            div()
                                .debug_selector(|| "settings-theme-edit".to_owned())
                                .child(
                                    Button::new("settings-theme-edit")
                                        .label(
                                            language
                                                .text(Message::SettingsAppearanceCustomThemeAction),
                                        )
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_theme_editor(window, cx);
                                        })),
                                ),
                            cx,
                        ),
                    ),
            )
            .child(
                self.group(language.text(Message::SettingsAppearanceDisplay), cx)
                    .child(self.slider_row(
                        language.text(Message::SettingsAppearanceOpacity),
                        language.text(Message::SettingsAppearanceOpacityDescription),
                        "settings-slider-opacity",
                        &self.opacity_slider,
                        SharedString::from(format!("{:.0}%", self.runtime.opacity * 100.0)),
                        cx,
                    ))
                    .child(self.slider_row(
                        language.text(Message::SettingsAppearanceScrollSpeed),
                        language.text(Message::SettingsAppearanceScrollSpeedDescription),
                        "settings-slider-scroll-speed",
                        &self.scroll_speed_slider,
                        SharedString::from(format!("{:.2}×", self.runtime.scroll_speed)),
                        cx,
                    ))
                    .child(self.select_row(
                        "scrollback_lines",
                        language.text(Message::SettingsAppearanceScrollback),
                        language.text(Message::SettingsAppearanceScrollbackDescription),
                        cx,
                    ))
                    .child(self.select_row(
                        "density",
                        language.text(Message::SettingsAppearanceDensity),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "tab_reveal",
                        language.text(Message::SettingsAppearanceTabReveal),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "tab_close_visible",
                        language.text(Message::SettingsAppearanceTabClose),
                        language.text(Message::SettingsAppearanceTabCloseDescription),
                        self.runtime.tab_close_visible,
                        cx,
                    ))
                    .child(self.switch_row(
                        "cjk_bold_regular",
                        language.text(Message::SettingsAppearanceCjkBold),
                        "",
                        self.runtime.cjk_bold_regular,
                        cx,
                    )),
            )
    }

    pub(super) fn page_profiles(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let startup_directory = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                div()
                    .debug_selector(|| "settings-input-startup_directory".to_owned())
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.startup_directory_input)),
            )
            .child(
                Button::new("settings-pick-startup-directory")
                    .label(language.text(Message::CommonChoose))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_startup_directory(window, cx);
                    })),
            )
            .into_any_element();
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.text(Message::SettingsProfilesStartup), cx)
                    .child(self.row(
                        language.text(Message::SettingsProfilesStartupDirectory),
                        language.text(Message::SettingsProfilesStartupDirectoryDescription),
                        startup_directory,
                        cx,
                    ))
                    .child(self.switch_row(
                        "refresh_environment",
                        language.text(Message::SettingsProfilesRefreshEnvironment),
                        language.text(Message::SettingsProfilesRefreshEnvironmentDescription),
                        self.runtime.refresh_environment,
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsProfilesCompletion), cx)
                    .child(self.switch_row(
                        "ghost",
                        language.text(Message::SettingsProfilesGhost),
                        "",
                        self.runtime.ghost,
                        cx,
                    ))
                    .child(self.select_row(
                        "completion_style",
                        language.text(Message::SettingsProfilesCompletionStyle),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "accept",
                        language.text(Message::SettingsProfilesAccept),
                        "",
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsProfilesAlerts), cx)
                    .child(self.switch_row(
                        "ai_toasts",
                        language.text(Message::SettingsProfilesAiToasts),
                        "",
                        self.runtime.ai_toasts,
                        cx,
                    ))
                    .child(self.select_row(
                        "notification_duration",
                        language.text(Message::SettingsProfilesNotificationDuration),
                        language.text(Message::SettingsProfilesNotificationDurationDescription),
                        cx,
                    ))
                    .child(self.select_row(
                        "bell",
                        language.text(Message::SettingsProfilesBell),
                        "",
                        cx,
                    )),
            )
    }

    pub(super) fn page_interaction(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.text(Message::SettingsInteractionMouse), cx)
                    .child(self.switch_row(
                        "focus_follows_mouse",
                        language.text(Message::SettingsInteractionFocusFollows),
                        "",
                        self.runtime.focus_follows_mouse.unwrap_or(false),
                        cx,
                    ))
                    .child(self.switch_row(
                        "copy_on_select",
                        language.text(Message::SettingsInteractionCopyOnSelect),
                        "",
                        self.runtime.copy_on_select,
                        cx,
                    ))
                    .child(self.switch_row(
                        "multiline_paste_confirm",
                        language.text(Message::SettingsInteractionPasteConfirm),
                        language.text(Message::SettingsInteractionPasteConfirmDescription),
                        self.runtime.multiline_paste_confirm,
                        cx,
                    ))
                    .child(self.switch_row(
                        "panel_resize",
                        language.text(Message::SettingsInteractionPanelResize),
                        "",
                        self.runtime.panel_resize,
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsInteractionCursor), cx)
                    .child(self.select_row(
                        "cursor_shape",
                        language.text(Message::SettingsInteractionCursorShape),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "cursor_blink",
                        language.text(Message::SettingsInteractionCursorBlink),
                        "",
                        self.runtime.cursor_blink.unwrap_or(true),
                        cx,
                    ))
                    .child(self.select_row(
                        "cursor_motion",
                        language.text(Message::SettingsInteractionCursorMotion),
                        "",
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsInteractionTabs), cx)
                    .child(self.select_row(
                        "new_tab_position",
                        language.text(Message::SettingsInteractionNewTabPosition),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "vcs_display",
                        language.text(Message::SettingsInteractionVcsDisplay),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "dim_inactive_panes",
                        language.text(Message::SettingsInteractionDimInactive),
                        "",
                        self.runtime.dim_inactive_panes,
                        cx,
                    )),
            )
    }

    pub(super) fn page_advanced(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.text(Message::SettingsAdvancedSession), cx)
                    .child(self.switch_row(
                        "restore_session",
                        language.text(Message::SettingsAdvancedRestoreSession),
                        "",
                        self.runtime.restore_session,
                        cx,
                    ))
                    .child(self.switch_row(
                        "resume_ai",
                        language.text(Message::SettingsAdvancedResumeAi),
                        "",
                        self.runtime.resume_ai,
                        cx,
                    ))
                    .child(self.switch_row(
                        "keep_session",
                        language.text(Message::SettingsAdvancedKeepSession),
                        "",
                        self.runtime.keep_session,
                        cx,
                    ))
                    .child(self.switch_row(
                        "silent_start",
                        language.text(Message::SettingsAdvancedSilentStart),
                        "",
                        self.runtime.silent_start,
                        cx,
                    ))
                    .child(self.switch_row(
                        "launch_at_login",
                        language.text(Message::SettingsAdvancedLaunchAtLogin),
                        "",
                        self.runtime.launch_at_login,
                        cx,
                    ))
                    .child(self.switch_row(
                        "tray",
                        language.text(Message::SettingsAdvancedTray),
                        "",
                        self.runtime.tray,
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsAdvancedUpdates), cx)
                    .child(self.switch_row(
                        "auto_check_updates",
                        language.text(Message::SettingsAdvancedCheckUpdates),
                        "",
                        self.runtime.auto_check_updates,
                        cx,
                    ))
                    .child(self.switch_row(
                        "auto_download_updates",
                        language.text(Message::SettingsAdvancedDownloadUpdates),
                        language.text(Message::SettingsAdvancedDownloadUpdatesDescription),
                        self.runtime.auto_download_updates,
                        cx,
                    ))
                    .child(self.switch_row(
                        "fetch",
                        language.text(Message::SettingsAdvancedWelcome),
                        "",
                        self.runtime.fetch,
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsAdvancedEnvironment), cx)
                    .child(self.switch_row(
                        "terminal_proxy",
                        language.text(Message::SettingsAdvancedProxy),
                        language.text(Message::SettingsAdvancedProxyDescription),
                        self.runtime.terminal_proxy,
                        cx,
                    )),
            )
    }
}
