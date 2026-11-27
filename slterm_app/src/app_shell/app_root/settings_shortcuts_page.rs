//! 设置中心快捷键页：只读展示统一命令目录的默认绑定。
//!
//! 本页不维护第二份键位表；录制、覆盖与冲突检测由后续切片接入同一目录。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div,
};
use gpui_component::ActiveTheme as _;

use super::command_registry::{CommandId, CommandRegistry, ShortcutGroup};
use super::settings_overlay::SettingsView;

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
        let mut rows = div()
            .id("settings-shortcuts")
            .debug_selector(|| "settings-shortcuts".to_owned())
            .flex()
            .flex_col()
            .gap_3();
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
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(group.message())),
                );
            for (index, (id, keys)) in merged.into_iter().enumerate() {
                let Some(label) = id.label_message() else {
                    continue;
                };
                let keys = keys
                    .into_iter()
                    .map(display_keystroke)
                    .collect::<Vec<_>>()
                    .join(" · ");
                section = section.child(
                    div()
                        .id(gpui::ElementId::Name(
                            format!("settings-shortcut-{slug}-{index}").into(),
                        ))
                        .debug_selector(move || format!("settings-shortcut-{slug}-{index}"))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(div().text_sm().child(language.text(label)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(keys),
                        ),
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
