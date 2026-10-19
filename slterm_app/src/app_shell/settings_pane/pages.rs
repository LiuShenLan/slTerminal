//! 各分区页:只摆控件,不定义设置语义。所有改动经统一写通道
//! (`SettingsPane::try_persist`)落盘并重载。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, KeyDownEvent, ObjectFit,
    ParentElement as _, SharedString, Styled as _, StyledImage as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::select::Select;
use gpui_component::slider::Slider;
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};

use crate::app_shell::workspace::keyboard_bindings;
use crate::encrypted_backup::BackupCategory;
use crate::i18n::Message;

use super::SettingsPane;
use super::design::{SETTINGS_GROUP_GAP, SETTINGS_SELECT_WIDTH};
use super::localization::UiLanguage;

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

    /// 加密备份类别开关行:选中态只存设置页内存,不写设置键。
    fn backup_category_row(
        &self,
        category: BackupCategory,
        debug_id: &'static str,
        label: &'static str,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let checked = self.backup_selection.get(category);
        let control = div()
            .debug_selector(move || debug_id.to_owned())
            .child(Switch::new(debug_id).checked(checked).on_click(cx.listener(
                move |this, checked: &bool, _, cx| {
                    this.backup_selection.set(category, *checked);
                    this.status = None;
                    cx.notify();
                },
            )))
            .into_any_element();
        self.row(label, "", control, cx).into_any_element()
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

    /// 独立下拉行(实体不进入通用 `selects`:壁纸键的值映射与本地化在
    /// `settings_pane` 本模块完成)。
    fn bound_select_row(
        &self,
        debug_id: &'static str,
        label: &'static str,
        desc: &'static str,
        select: &gpui::Entity<gpui_component::select::SelectState<Vec<SharedString>>>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = div()
            .debug_selector(move || debug_id.to_owned())
            .w(px(SETTINGS_SELECT_WIDTH))
            .child(
                Select::new(select)
                    .appearance(false)
                    .h_full()
                    .rounded(px(6.0)),
            )
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    /// 壁纸缩略预览:固定盒内按 fit/alignment 落点绘制后台解码的纹理。
    fn wallpaper_preview_element(&self, cx: &Context<'_, Self>) -> AnyElement {
        // 与控件列同宽(220),高度按 2:1 留出 fit/alignment 观察面。
        const PREVIEW_WIDTH: f32 = SETTINGS_SELECT_WIDTH;
        const PREVIEW_HEIGHT: f32 = 110.0;
        let language = self.language(cx);
        let Some(preview) = self.wallpaper_preview.as_ref() else {
            let label = if self.wallpaper_preview_loading {
                language.text(Message::SettingsAppearanceWallpaperPreviewLoading)
            } else {
                language.text(Message::SettingsAppearanceWallpaperPreviewEmpty)
            };
            return div()
                .debug_selector(|| "settings-wallpaper-preview".to_owned())
                .w(px(PREVIEW_WIDTH))
                .h(px(PREVIEW_HEIGHT))
                .rounded_md()
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .into_any_element();
        };
        let (x, y, width, height) = crate::app_shell::wallpaper::preview_rect(
            PREVIEW_WIDTH,
            PREVIEW_HEIGHT,
            preview.width,
            preview.height,
            self.runtime.background_image_fit.as_deref(),
            self.runtime.background_image_alignment.as_deref(),
        );
        div()
            .debug_selector(|| "settings-wallpaper-preview".to_owned())
            .relative()
            .w(px(PREVIEW_WIDTH))
            .h(px(PREVIEW_HEIGHT))
            .rounded_md()
            .overflow_hidden()
            .bg(cx.theme().muted)
            .child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    .child(
                        gpui::img(preview.image.clone())
                            // 落点矩形已编码 fit/alignment;Fill 让纹理铺满该矩形。
                            .object_fit(ObjectFit::Fill),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn page_appearance(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        // 语言热切换只换显示项;壁纸下拉不走通用 `selects` 刷新路径。
        self.sync_wallpaper_select_language(window, cx);
        // 启动/回读/恢复路径:运行时路径变化时后台重载缩略图。
        self.sync_wallpaper_preview(cx);
        let language = self.language(cx);
        let wallpaper_path = self.runtime.background_image.clone();
        let wallpaper_path_label = wallpaper_path.clone().unwrap_or_else(|| {
            language
                .text(Message::SettingsAppearanceWallpaperNone)
                .to_owned()
        });
        let wallpaper_path_control = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                div()
                    .debug_selector(|| "settings-wallpaper-path".to_owned())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(wallpaper_path_label),
            )
            .child(
                Button::new("settings-wallpaper-choose")
                    .label(language.text(Message::CommonChoose))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_wallpaper(window, cx);
                    })),
            )
            .child(
                Button::new("settings-wallpaper-clear")
                    .label(language.text(Message::SettingsAppearanceWallpaperClear))
                    .disabled(wallpaper_path.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_wallpaper(window, cx);
                    })),
            )
            .into_any_element();
        let mut wallpaper_group = self
            .group(language.text(Message::SettingsAppearanceWallpaper), cx)
            .child(self.row(
                language.text(Message::SettingsAppearanceWallpaperPath),
                language.text(Message::SettingsAppearanceWallpaperPathDescription),
                wallpaper_path_control,
                cx,
            ))
            .child(self.row(
                language.text(Message::SettingsAppearanceWallpaperPreview),
                "",
                self.wallpaper_preview_element(cx),
                cx,
            ))
            .child(self.bound_select_row(
                "settings-wallpaper-fit",
                language.text(Message::SettingsAppearanceWallpaperFit),
                "",
                &self.wallpaper_fit_select,
                cx,
            ))
            .child(self.bound_select_row(
                "settings-wallpaper-alignment",
                language.text(Message::SettingsAppearanceWallpaperAlignment),
                "",
                &self.wallpaper_alignment_select,
                cx,
            ))
            .child(self.switch_row(
                "background_image_cover_chrome",
                language.text(Message::SettingsAppearanceWallpaperCoverChrome),
                language.text(Message::SettingsAppearanceWallpaperCoverChromeDescription),
                self.runtime.background_image_cover_chrome,
                cx,
            ))
            .child(self.slider_row(
                language.text(Message::SettingsAppearanceWallpaperOpacity),
                language.text(Message::SettingsAppearanceWallpaperOpacityDescription),
                "settings-slider-wallpaper-opacity",
                &self.wallpaper_opacity_slider,
                SharedString::from(format!(
                    "{:.0}%",
                    self.runtime.background_image_opacity * 100.0
                )),
                cx,
            ));
        if let Some(error) = self.wallpaper_error.clone() {
            let danger = cx.theme().danger;
            wallpaper_group = wallpaper_group.child(
                div()
                    .debug_selector(|| "settings-wallpaper-error".to_owned())
                    .w_full()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded_md()
                    .bg(danger.opacity(0.08))
                    .text_color(danger)
                    .text_sm()
                    .child(error),
            );
        }
        let import_busy = self.theme_import_busy;
        let export_busy = self.theme_export_busy;
        let has_custom_theme = self.runtime.custom_theme.is_some();
        let import_label = if import_busy {
            language.text(Message::SettingsAppearanceThemeImportBusy)
        } else {
            language.text(Message::SettingsAppearanceThemeImportAction)
        };
        let export_label = if export_busy {
            language.text(Message::SettingsAppearanceThemeExportBusy)
        } else {
            language.text(Message::SettingsAppearanceThemeExportAction)
        };
        let export_description = if has_custom_theme {
            language.text(Message::SettingsAppearanceThemeExportDescription)
        } else {
            language.text(Message::SettingsAppearanceThemeExportNeedsCustom)
        };
        let theme_package_group = self
            .group(language.text(Message::SettingsAppearanceThemePackage), cx)
            .child(
                self.row(
                    language.text(Message::SettingsAppearanceThemeImport),
                    language.text(Message::SettingsAppearanceThemeImportDescription),
                    Button::new("settings-theme-import")
                        .label(import_label)
                        .disabled(import_busy)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.import_theme_package(window, cx);
                        }))
                        .into_any_element(),
                    cx,
                ),
            )
            .child(
                self.row(
                    language.text(Message::SettingsAppearanceThemeExport),
                    export_description,
                    Button::new("settings-theme-export")
                        .label(export_label)
                        .disabled(export_busy || !has_custom_theme)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.export_theme_package(window, cx);
                        }))
                        .into_any_element(),
                    cx,
                ),
            );
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
            .child(theme_package_group)
            .child(wallpaper_group)
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
            // 键位组置顶:动作表行数多,置顶保证首屏可见与可发现性。
            .child(self.keybind_editor(language, cx))
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

    /// 键位编辑组:可重绑动作表(录制/清除/恢复默认)+ 保留键只读区。
    fn keybind_editor(&self, language: UiLanguage, cx: &Context<'_, Self>) -> AnyElement {
        // 录制焦点挂在本组容器:进入录制态后下一次 KeyDown 由此消费。
        let mut table = v_flex()
            .w_full()
            .track_focus(&self.keybind_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_keybind_key_down(event, window, cx);
            }));
        for action in keyboard_bindings::EDITABLE_ACTIONS {
            table = table.child(self.keybind_editor_row(action, language, cx));
        }
        table = table.child(
            div()
                .w_full()
                .pt(px(12.0))
                .pb(px(4.0))
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsInteractionKeybindsReserved)),
        );
        for (combo, label_zh, label_en) in keyboard_bindings::reserved_display_rows() {
            let label = if language == UiLanguage::ZhCn {
                label_zh
            } else {
                label_en
            };
            table = table.child(
                h_flex()
                    .w_full()
                    .py(px(6.0))
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .w(px(150.0))
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(combo),
                    ),
            );
        }
        if let Some(error) = self.keybind_error.clone() {
            table = table.child(
                div()
                    .debug_selector(|| "settings-keybind-error".to_owned())
                    .w_full()
                    .pt(px(6.0))
                    .text_color(cx.theme().danger)
                    .child(error),
            );
        }
        self.group(language.text(Message::SettingsInteractionKeybinds), cx)
            .child(table)
            .into_any_element()
    }

    /// 单个可重绑动作行:名称 + 当前生效组合 + 录制/清除/恢复默认。
    fn keybind_editor_row(
        &self,
        action: &keyboard_bindings::EditableAction,
        language: UiLanguage,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let label = if language == UiLanguage::ZhCn {
            action.label_zh
        } else {
            action.label_en
        };
        let action_name = action.name;
        let recording = self.keybind_recording.as_deref() == Some(action_name);
        let combos = keyboard_bindings::effective_combos(&self.keybind_pairs, action_name);
        let combo_text = if recording {
            language
                .text(Message::SettingsInteractionKeybindRecording)
                .to_owned()
        } else if combos.is_empty() {
            language
                .text(Message::SettingsInteractionKeybindUnbound)
                .to_owned()
        } else {
            combos.join(" / ")
        };
        let record_label = if recording {
            language.text(Message::SettingsInteractionKeybindRecording)
        } else {
            language.text(Message::SettingsInteractionKeybindRecord)
        };
        h_flex()
            .w_full()
            .py(px(8.0))
            .gap_3()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.0))
                    .text_color(cx.theme().foreground)
                    .child(label),
            )
            .child(
                div()
                    .debug_selector(move || format!("settings-keybind-combo-{action_name}"))
                    .w(px(150.0))
                    .flex_shrink_0()
                    .truncate()
                    .text_color(if recording {
                        cx.theme().accent
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(combo_text),
            )
            .child(
                div()
                    .debug_selector(move || format!("settings-keybind-record-{action_name}"))
                    .child(
                        Button::new(SharedString::from(format!(
                            "settings-keybind-record-btn-{action_name}"
                        )))
                        .small()
                        .label(record_label)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.begin_keybind_recording(action_name, window, cx);
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .debug_selector(move || format!("settings-keybind-clear-{action_name}"))
                    .child(
                        Button::new(SharedString::from(format!(
                            "settings-keybind-clear-btn-{action_name}"
                        )))
                        .small()
                        .label(language.text(Message::SettingsInteractionKeybindClear))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.clear_keybind_action(action_name, cx);
                        })),
                    ),
            )
            .child(
                div()
                    .debug_selector(move || format!("settings-keybind-reset-{action_name}"))
                    .child(
                        Button::new(SharedString::from(format!(
                            "settings-keybind-reset-btn-{action_name}"
                        )))
                        .small()
                        .label(language.text(Message::SettingsInteractionKeybindReset))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.reset_keybind_action(action_name, cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn page_advanced(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        self.sync_backup_passphrase_placeholder(window, cx);
        let language = self.language(cx);
        let export_workspace_label = language.text(Message::SettingsAdvancedExportWorkspaceAction);
        let export_backup_label = if self.backup_busy {
            language.text(Message::SettingsBackupExportBusy)
        } else {
            language.text(Message::SettingsBackupExport)
        };
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
                    ))
                    .child(
                        self.row(
                            language.text(Message::SettingsAdvancedExportWorkspace),
                            language.text(Message::SettingsAdvancedExportWorkspaceDescription),
                            div()
                                .debug_selector(|| "settings-export-workspace".to_owned())
                                .child(
                                    Button::new("settings-export-workspace-btn")
                                        .label(export_workspace_label)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.prompt_export_workspace(window, cx);
                                        })),
                                ),
                            cx,
                        ),
                    ),
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
            // 加密备份导出(恢复/撤回入口不在本模块范围)。
            .child(
                self.group(language.text(Message::SettingsBackupGroup), cx)
                    .child(self.backup_category_row(
                        BackupCategory::Settings,
                        "settings-backup-category-settings",
                        language.text(Message::SettingsBackupCategorySettings),
                        cx,
                    ))
                    .child(self.backup_category_row(
                        BackupCategory::Session,
                        "settings-backup-category-session",
                        language.text(Message::SettingsBackupCategorySession),
                        cx,
                    ))
                    .child(self.backup_category_row(
                        BackupCategory::Themes,
                        "settings-backup-category-themes",
                        language.text(Message::SettingsBackupCategoryThemes),
                        cx,
                    ))
                    .child(self.backup_category_row(
                        BackupCategory::CommandHistory,
                        "settings-backup-category-command_history",
                        language.text(Message::SettingsBackupCategoryCommandHistory),
                        cx,
                    ))
                    .child(self.backup_category_row(
                        BackupCategory::DirectoryHistory,
                        "settings-backup-category-directory_history",
                        language.text(Message::SettingsBackupCategoryDirectoryHistory),
                        cx,
                    ))
                    .child(self.backup_category_row(
                        BackupCategory::Providers,
                        "settings-backup-category-providers",
                        language.text(Message::SettingsBackupCategoryProviders),
                        cx,
                    ))
                    .child(
                        self.row(
                            language.text(Message::SettingsBackupPassphrase),
                            language.text(Message::SettingsBackupPassphraseDescription),
                            div()
                                .debug_selector(|| "settings-backup-passphrase".to_owned())
                                .w(px(SETTINGS_SELECT_WIDTH))
                                .child(Input::new(&self.backup_passphrase)),
                            cx,
                        ),
                    )
                    .child(
                        self.row(
                            language.text(Message::SettingsBackupExport),
                            language.text(Message::SettingsBackupExportDescription),
                            div()
                                .debug_selector(|| "settings-backup-export".to_owned())
                                .child(
                                    Button::new("settings-backup-export-btn")
                                        .label(export_backup_label)
                                        .disabled(self.backup_busy)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.export_backup(window, cx);
                                        })),
                                ),
                            cx,
                        ),
                    ),
            )
    }
}
