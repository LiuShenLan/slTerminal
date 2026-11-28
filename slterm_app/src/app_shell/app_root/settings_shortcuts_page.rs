//! 设置中心快捷键页：目录行、录制、覆盖、解绑、冲突与坏值修复。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;

use super::command_registry::{CommandId, CommandRegistry, ShortcutGroup};
use super::keybind_config::{KeybindRecording, KeybindRuntime, RecordingError};
use super::settings_overlay::SettingsView;
use crate::i18n::Message;

/// 默认键位字符串转显示格式：修饰键与命名键固定写法，其余首字母大写。
pub(super) fn display_keystroke(raw: &str) -> String {
    raw.split('-')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_owned(),
            "shift" => "Shift".to_owned(),
            "alt" => "Alt".to_owned(),
            "meta" => "Meta".to_owned(),
            "up" => "Up".to_owned(),
            "down" => "Down".to_owned(),
            "left" => "Left".to_owned(),
            "right" => "Right".to_owned(),
            "home" => "Home".to_owned(),
            "end" => "End".to_owned(),
            "enter" => "Enter".to_owned(),
            "escape" => "Esc".to_owned(),
            "tab" => "Tab".to_owned(),
            "space" => "Space".to_owned(),
            "pageup" => "PageUp".to_owned(),
            "pagedown" => "PageDown".to_owned(),
            "backspace" => "Backspace".to_owned(),
            "delete" => "Delete".to_owned(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

const fn group_slug(group: ShortcutGroup) -> &'static str {
    match group {
        ShortcutGroup::Global => "global",
        ShortcutGroup::Terminal => "terminal",
        ShortcutGroup::Layout => "layout",
        ShortcutGroup::Sidebar => "sidebar",
    }
}

impl SettingsView {
    pub(super) fn render_shortcuts_page(&self, cx: &Context<'_, Self>) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        let recording = cx
            .try_global::<KeybindRecording>()
            .cloned()
            .unwrap_or_default();
        let theme = cx.theme().clone();
        let mut rows = div()
            .id("settings-shortcuts")
            .debug_selector(|| "settings-shortcuts".to_owned())
            .flex()
            .flex_col()
            .gap_3();

        let invalid = runtime.config.invalid();
        if !invalid.is_empty() {
            let mut repair = div()
                .id("settings-shortcut-repair")
                .debug_selector(|| "settings-shortcut-repair".to_owned())
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded(px(4.0))
                .border_1()
                .border_color(theme.danger)
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(language.text(Message::SettingsShortcutsRepairInvalid)),
                );
            for (index, item) in invalid.iter().enumerate() {
                let (combo, action) = (item.combo.clone(), item.action.clone());
                repair = repair.child(
                    div()
                        .id(("settings-shortcut-invalid", index))
                        .debug_selector(move || format!("settings-shortcut-invalid-{index}"))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(format!("{} · {}", display_keystroke(&combo), action))
                        .child(
                            div()
                                .px_1()
                                .cursor_pointer()
                                .text_color(theme.primary)
                                .child(language.text(Message::SettingsInteractionKeybindReset))
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(move |view, _, _, cx| {
                                        cx.stop_propagation();
                                        view.remove_invalid_shortcut(&combo, &action, cx);
                                    }),
                                ),
                        ),
                );
            }
            repair = repair.child(
                div()
                    .id("settings-shortcut-repair-all")
                    .debug_selector(|| "settings-shortcut-repair-all".to_owned())
                    .w_full()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme.primary)
                    .child(language.text(Message::SettingsShortcutsRepairInvalid))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            cx.stop_propagation();
                            view.repair_invalid_shortcuts(cx);
                        }),
                    ),
            );
            rows = rows.child(repair);
        }

        if !runtime.conflicts.is_empty() {
            let names = runtime
                .conflicts
                .iter()
                .filter_map(|command| command.label_message())
                .map(|message| language.text(message))
                .collect::<Vec<_>>()
                .join(" · ");
            rows = rows.child(
                div()
                    .id("settings-shortcut-conflict")
                    .debug_selector(|| "settings-shortcut-conflict".to_owned())
                    .text_xs()
                    .text_color(theme.warning)
                    .child(
                        language
                            .format(Message::SettingsShortcutsConflict, &[("commands", &names)]),
                    ),
            );
        }
        if let Some(error) = runtime.recording_error {
            let combo = display_keystroke(&runtime.recording_error_combo);
            let message = match error {
                RecordingError::Reserved => language.format(
                    Message::SettingsInteractionKeybindReservedError,
                    &[("combo", &combo)],
                ),
                RecordingError::Invalid => language.format(
                    Message::SettingsInteractionKeybindInvalidError,
                    &[("combo", &combo)],
                ),
            };
            rows = rows.child(
                div()
                    .id("settings-shortcut-recording-error")
                    .debug_selector(|| "settings-shortcut-recording-error".to_owned())
                    .text_xs()
                    .text_color(theme.danger)
                    .child(message),
            );
        }

        for group in ShortcutGroup::ALL {
            let slug = group_slug(group);
            let mut merged: Vec<(CommandId, Vec<&'static str>)> = Vec::new();
            for definition in CommandRegistry::shortcut_definitions(group) {
                match merged.iter_mut().find(|(id, _)| *id == definition.id) {
                    Some((_, keys)) => keys.push(definition.keystroke),
                    None => merged.push((definition.id, vec![definition.keystroke])),
                }
            }
            let mut section = div()
                .id(gpui::ElementId::Name(
                    format!("settings-shortcut-group-{slug}").into(),
                ))
                .debug_selector(move || format!("settings-shortcut-group-{slug}"))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(language.text(group.message())),
                );
            for (index, (id, defaults)) in merged.into_iter().enumerate() {
                let Some(label) = id.label_message() else {
                    continue;
                };
                let customized = runtime.config.is_customized(id);
                let unbound = runtime.config.is_unbound(id);
                let combos: Vec<String> = if customized {
                    runtime.config.effective_combos(id)
                } else {
                    defaults.iter().map(|combo| (*combo).to_owned()).collect()
                };
                let keys = if unbound {
                    language
                        .text(Message::SettingsInteractionKeybindUnbound)
                        .to_string()
                } else {
                    combos
                        .iter()
                        .map(|combo| display_keystroke(combo))
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                let is_recording = recording.command == Some(id);
                let row_selector = format!("settings-shortcut-{slug}-{index}");
                let mut key_row = div()
                    .id(gpui::ElementId::Name(row_selector.clone().into()))
                    .debug_selector(move || row_selector.clone())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .cursor_pointer()
                    .child(div().text_sm().child(language.text(label)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_recording {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .child(if is_recording {
                                language
                                    .text(Message::SettingsInteractionKeybindRecording)
                                    .to_string()
                            } else {
                                keys
                            }),
                    );
                if customized {
                    let reset_selector = format!("settings-shortcut-reset-{}", id.slug());
                    key_row = key_row.child(
                        div()
                            .id(SharedString::from(reset_selector.clone()))
                            .debug_selector(move || reset_selector.clone())
                            .px_1()
                            .text_xs()
                            .text_color(theme.primary)
                            .child(language.text(Message::SettingsInteractionKeybindReset))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    cx.stop_propagation();
                                    view.reset_shortcut(id, cx);
                                }),
                            ),
                    );
                }
                let add_selector = format!("settings-shortcut-add-{}", id.slug());
                key_row = key_row.child(
                    div()
                        .id(SharedString::from(add_selector.clone()))
                        .debug_selector(move || add_selector.clone())
                        .px_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("+")
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |view, _, _, cx| {
                                cx.stop_propagation();
                                view.start_shortcut_recording(id, true, cx);
                            }),
                        ),
                );
                key_row = key_row.on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.start_shortcut_recording(id, false, cx);
                    }),
                );
                let item_selector = format!("settings-shortcut-item-{}", id.slug());
                section = section.child(
                    div()
                        .debug_selector(move || item_selector.clone())
                        .child(key_row),
                );
            }
            rows = rows.child(section);
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .child(rows)
            .into_any_element()
    }
}

#[cfg(test)]
mod shortcut_display_tests {
    use super::display_keystroke;

    #[test]
    fn keystrokes_display_with_stable_modifier_order_and_names() {
        assert_eq!(display_keystroke("ctrl-shift-t"), "Ctrl+Shift+T");
        assert_eq!(display_keystroke("ctrl-,"), "Ctrl+,");
        assert_eq!(display_keystroke("shift-f10"), "Shift+F10");
        assert_eq!(display_keystroke("pageup"), "PageUp");
        assert_eq!(display_keystroke("escape"), "Esc");
        assert_eq!(display_keystroke("left"), "Left");
    }
}
