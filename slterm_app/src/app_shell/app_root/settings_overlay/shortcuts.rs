//! SettingsView 的快捷键录制、覆盖、解绑与坏值修复入口。

use gpui::{Context, Keystroke};

use super::super::command_registry::CommandId;
use super::super::keybind_config::{
    KeybindConfig, KeybindRecording, KeybindRuntime, RecordingError,
};
use super::SettingsView;

impl SettingsView {
    pub(in crate::app_shell::app_root) fn start_shortcut_recording(
        &mut self,
        command: CommandId,
        append: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        cx.set_global(KeybindRuntime {
            config: runtime.config,
            conflicts: Vec::new(),
            recording_error: None,
            recording_error_combo: String::new(),
        });
        let owner = cx.entity().downgrade();
        cx.set_global(KeybindRecording {
            command: Some(command),
            append,
            owner: Some(owner),
        });
        cx.notify();
    }

    /// 全局拦截器在录制期间把每个键交到这里；只有完整组合才形成绑定。
    pub(in crate::app_shell::app_root) fn record_shortcut_keystroke(
        &mut self,
        keystroke: Keystroke,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(recording) = cx.try_global::<KeybindRecording>().cloned() else {
            return;
        };
        let Some(command) = recording.command else {
            return;
        };
        let key = keystroke.key.to_ascii_lowercase();
        if key.is_empty()
            || matches!(
                key.as_str(),
                "ctrl" | "control" | "shift" | "alt" | "meta" | "super" | "win"
            )
        {
            // 只按修饰键不形成绑定，录制继续
            return;
        }
        if key == "escape" && !keystroke.modifiers.modified() {
            cx.set_global(KeybindRecording::default());
            cx.notify();
            return;
        }
        if matches!(key.as_str(), "backspace" | "delete") && !keystroke.modifiers.modified() {
            let runtime = cx
                .try_global::<KeybindRuntime>()
                .cloned()
                .unwrap_or_default();
            let mut config = runtime.config;
            config.unbind(command);
            self.apply_shortcut_config(config, Vec::new(), None, cx);
            cx.set_global(KeybindRecording::default());
            cx.notify();
            return;
        }
        let Some(combo) = keystroke_combo(&keystroke) else {
            return;
        };
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        if runtime.config.reserved(command, &combo) {
            cx.set_global(KeybindRuntime {
                config: runtime.config,
                conflicts: Vec::new(),
                recording_error: Some(RecordingError::Reserved),
                recording_error_combo: combo.clone(),
            });
            cx.notify();
            return;
        }
        if Keystroke::parse(&combo).is_err() {
            cx.set_global(KeybindRuntime {
                config: runtime.config,
                conflicts: Vec::new(),
                recording_error: Some(RecordingError::Invalid),
                recording_error_combo: combo.clone(),
            });
            cx.notify();
            return;
        }
        let mut config = runtime.config;
        if recording.append {
            config.add_extra(command, &combo);
        } else {
            config.record(command, &combo);
        }
        let conflicts = config.conflicts(command, &combo);
        self.apply_shortcut_config(config, conflicts, None, cx);
        cx.set_global(KeybindRecording::default());
        cx.notify();
    }

    pub(in crate::app_shell::app_root) fn reset_shortcut(
        &mut self,
        command: CommandId,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        let mut config = runtime.config;
        config.reset(command);
        self.apply_shortcut_config(config, Vec::new(), None, cx);
    }

    pub(in crate::app_shell::app_root) fn repair_invalid_shortcuts(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        let config = runtime.config.without_invalid_rows();
        self.apply_shortcut_config(config, Vec::new(), None, cx);
    }

    pub(in crate::app_shell::app_root) fn remove_invalid_shortcut(
        &mut self,
        combo: &str,
        action: &str,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = cx
            .try_global::<KeybindRuntime>()
            .cloned()
            .unwrap_or_default();
        let mut config = runtime.config;
        config.remove_row(combo, action);
        self.apply_shortcut_config(config, Vec::new(), None, cx);
    }

    fn apply_shortcut_config(
        &mut self,
        config: KeybindConfig,
        conflicts: Vec<CommandId>,
        error: Option<RecordingError>,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| {
                root.apply_keybind_config(config, conflicts, error, cx);
                root.notify_settings_views(cx);
            });
        }
        cx.notify();
    }
}

fn keystroke_combo(keystroke: &Keystroke) -> Option<String> {
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("ctrl");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.platform {
        parts.push("meta");
    }
    let key = keystroke.key.to_ascii_lowercase();
    if key.is_empty() {
        return None;
    }
    parts.push(key.as_str());
    Some(parts.join("-"))
}
