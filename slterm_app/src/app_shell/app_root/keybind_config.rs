//! 快捷键用户覆盖的唯一模型：默认目录 + 持久键位段的行为等价映射。
//!
//! 本模块只做纯值转换；GPUI 绑定应用与设置写盘由 AppRoot 负责。

use gpui::{
    App, DummyKeyboardMapper, Global, KeyBinding, KeyBindingContextPredicate, Keystroke, Unbind,
    WeakEntity,
};
use slterm_settings::json::Value;
use slterm_settings::keys::{KEYBIND_SEGMENT, keybind_pairs_from_json, keybind_value};

use super::AppRoot;
use super::command_registry::{CommandId, CommandRegistry};
use super::settings_overlay::SettingsView;

/// 持久动作字符串的解除绑定前缀。
pub(crate) const UNBIND_PREFIX: &str = "unbind:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvalidKeybindReason {
    UnknownCommand,
    InvalidCombo,
    Reserved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InvalidKeybind {
    pub(crate) combo: String,
    pub(crate) action: String,
    pub(crate) reason: InvalidKeybindReason,
}

/// `keybind` 段的运行期视图；所有行保持原顺序，坏行不静默删除。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeybindConfig {
    rows: Vec<(String, String)>,
}

impl KeybindConfig {
    pub(crate) fn from_pairs(pairs: &[(String, String)]) -> Self {
        Self {
            rows: pairs.to_vec(),
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn pairs(&self) -> Vec<(String, String)> {
        self.rows.clone()
    }

    pub(crate) fn value(&self) -> Value {
        keybind_value(&self.rows)
    }

    fn unbind_slug(action: &str) -> Option<&str> {
        action.strip_prefix(UNBIND_PREFIX)
    }

    fn slug_rows<'a>(&'a self, slug: &'a str) -> impl Iterator<Item = &'a (String, String)> {
        self.rows
            .iter()
            .filter(move |(_, action)| action == slug || Self::unbind_slug(action) == Some(slug))
    }

    pub(crate) fn is_unbound(&self, id: CommandId) -> bool {
        self.rows
            .iter()
            .any(|(_, action)| Self::unbind_slug(action) == Some(id.slug()))
    }

    /// 生效组合：用户覆盖优先；没有合法覆盖时回退目录默认。
    pub(crate) fn effective_combos(&self, id: CommandId) -> Vec<String> {
        if self.is_unbound(id) {
            return Vec::new();
        }
        let combos = self
            .rows
            .iter()
            .filter(|(_, action)| action == id.slug())
            .filter(|(combo, _)| valid_combo(combo) && !self.reserved(id, combo))
            .map(|(combo, _)| combo.to_ascii_lowercase())
            .collect::<Vec<_>>();
        if combos.is_empty() {
            default_combos(id)
        } else {
            combos
        }
    }

    /// 是否存在生效的用户覆盖（含解绑）。
    pub(crate) fn is_customized(&self, id: CommandId) -> bool {
        self.slug_rows(id.slug()).next().is_some()
    }

    pub(crate) fn invalid(&self) -> Vec<InvalidKeybind> {
        let mut invalid = Vec::new();
        for (combo, action) in &self.rows {
            if let Some(slug) = Self::unbind_slug(action) {
                if CommandId::from_slug(slug).is_none() {
                    invalid.push(InvalidKeybind {
                        combo: combo.clone(),
                        action: action.clone(),
                        reason: InvalidKeybindReason::UnknownCommand,
                    });
                }
                continue;
            }
            let Some(id) = CommandId::from_slug(action) else {
                invalid.push(InvalidKeybind {
                    combo: combo.clone(),
                    action: action.clone(),
                    reason: InvalidKeybindReason::UnknownCommand,
                });
                continue;
            };
            if !valid_combo(combo) {
                invalid.push(InvalidKeybind {
                    combo: combo.clone(),
                    action: action.clone(),
                    reason: InvalidKeybindReason::InvalidCombo,
                });
                continue;
            }
            if self.reserved(id, combo) {
                invalid.push(InvalidKeybind {
                    combo: combo.clone(),
                    action: action.clone(),
                    reason: InvalidKeybindReason::Reserved,
                });
            }
        }
        invalid
    }

    pub(crate) fn without_invalid_rows(&self) -> Self {
        let invalid = self.invalid();
        Self {
            rows: self
                .rows
                .iter()
                .filter(|row| {
                    !invalid
                        .iter()
                        .any(|item| item.combo == row.0 && item.action == row.1)
                })
                .cloned()
                .collect(),
        }
    }

    /// 同上下文、同组合的其他命令；冲突允许保存但必须展示。
    pub(crate) fn conflicts(&self, id: CommandId, combo: &str) -> Vec<CommandId> {
        let combo = combo.to_ascii_lowercase();
        let command_contexts = contexts(id);
        CommandRegistry::all_definitions()
            .map(|definition| definition.id)
            .filter(|other| *other != id)
            .filter(|other| {
                command_contexts
                    .iter()
                    .any(|context| contexts(*other).contains(context))
            })
            .filter(|other| {
                self.effective_combos(*other)
                    .iter()
                    .any(|candidate| candidate == &combo)
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// 终端及全局上下文永久保留 Ctrl+C 为中断。
    pub(crate) fn reserved(&self, id: CommandId, combo: &str) -> bool {
        primary_context(id).is_none() && combo.eq_ignore_ascii_case("ctrl-c")
    }

    /// 录制覆盖：替换该命令全部用户行（含解绑）。
    pub(crate) fn record(&mut self, id: CommandId, combo: &str) {
        self.reset(id);
        self.rows
            .push((combo.to_ascii_lowercase(), id.slug().to_owned()));
    }

    /// 追加额外绑定：首次定制先物化默认组合，解绑行被移除。
    pub(crate) fn add_extra(&mut self, id: CommandId, combo: &str) {
        if !self.is_customized(id) {
            for default in default_combos(id) {
                self.rows.push((default, id.slug().to_owned()));
            }
        }
        self.rows
            .retain(|(_, action)| Self::unbind_slug(action) != Some(id.slug()));
        self.rows
            .push((combo.to_ascii_lowercase(), id.slug().to_owned()));
    }

    pub(crate) fn unbind(&mut self, id: CommandId) {
        self.reset(id);
        // 空 combo 会被键位段宽容解析丢弃；解绑行用固定哨兵占据组合位置
        self.rows.push((
            "unbound".to_owned(),
            format!("{UNBIND_PREFIX}{}", id.slug()),
        ));
    }

    pub(crate) fn reset(&mut self, id: CommandId) {
        self.rows.retain(|(_, action)| {
            action != id.slug() && Self::unbind_slug(action) != Some(id.slug())
        });
    }

    pub(crate) fn remove_row(&mut self, combo: &str, action: &str) {
        self.rows
            .retain(|(row_combo, row_action)| row_combo != combo || row_action != action);
    }
}

/// 组合必须是非空键名结尾的完整键位；修饰键单独结尾不算。
fn valid_combo(combo: &str) -> bool {
    let last = combo.rsplit('-').next().unwrap_or_default();
    !last.is_empty()
        && !matches!(
            last.to_ascii_lowercase().as_str(),
            "ctrl" | "control" | "shift" | "alt" | "meta" | "super" | "win"
        )
        && Keystroke::parse(combo).is_ok()
}

/// 目录默认组合，按定义顺序去重。
pub(crate) fn default_combos(id: CommandId) -> Vec<String> {
    let mut combos = Vec::new();
    for definition in CommandRegistry::all_definitions() {
        if definition.id != id {
            continue;
        }
        let combo = definition.keystroke.to_ascii_lowercase();
        if !combos.contains(&combo) {
            combos.push(combo);
        }
    }
    combos
}

/// 命令的全部派发上下文；同一命令可同时服务正文与查找栏。
pub(crate) fn contexts(id: CommandId) -> Vec<Option<&'static str>> {
    let mut contexts = Vec::new();
    for definition in CommandRegistry::all_definitions() {
        if definition.id == id && !contexts.contains(&definition.context) {
            contexts.push(definition.context);
        }
    }
    contexts
}

/// 命令的首个派发上下文；保留键与页面提示使用。
pub(crate) fn primary_context(id: CommandId) -> Option<&'static str> {
    contexts(id).first().copied().flatten()
}

/// 快捷键页读取的生效配置与最近一次冲突/录制错误。
#[derive(Clone, Debug, Default)]
pub(crate) struct KeybindRuntime {
    pub(crate) config: KeybindConfig,
    pub(crate) conflicts: Vec<CommandId>,
    pub(crate) recording_error: Option<RecordingError>,
    pub(crate) recording_error_combo: String,
}

impl Global for KeybindRuntime {}

/// 录制目标由 AppRoot 的全局按键拦截器消费；视图只负责发起与重绘。
#[derive(Clone, Debug, Default)]
pub(crate) struct KeybindRecording {
    pub(crate) command: Option<CommandId>,
    pub(crate) append: bool,
    pub(in crate::app_shell::app_root) owner: Option<WeakEntity<SettingsView>>,
}

impl Global for KeybindRecording {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordingError {
    Reserved,
    Invalid,
}

/// 把配置差异写入 GPUI 键位：旧组合先解绑，新组合后绑定。
///
/// 仅追加绑定与 `Unbind`，不清空其他模块键位；重置时在更高位置重新绑定默认动作，
/// 使其重新胜过更早的解绑记录。
pub(crate) fn apply_bindings(old: &KeybindConfig, new: &KeybindConfig, cx: &mut App) {
    let mut changed = std::collections::BTreeSet::new();
    for definition in CommandRegistry::all_definitions() {
        let id = definition.id;
        if old.effective_combos(id) != new.effective_combos(id)
            || old.is_customized(id) != new.is_customized(id)
        {
            changed.insert(id);
        }
    }
    for id in changed {
        let Some(action) = CommandRegistry::action_for(id) else {
            continue;
        };
        let action_name = action.name();
        let context = primary_context(id);
        for combo in old.effective_combos(id) {
            bind(cx, &combo, Box::new(Unbind(action_name.into())), context);
        }
        let combos = if new.is_customized(id) {
            new.effective_combos(id)
        } else {
            default_combos(id)
        };
        for combo in combos {
            if let Some(action) = CommandRegistry::action_for(id) {
                bind(cx, &combo, action, context);
            }
        }
    }
}

fn bind(cx: &mut App, combo: &str, action: Box<dyn gpui::Action>, context: Option<&'static str>) {
    let predicate = context.map(|context| {
        KeyBindingContextPredicate::parse(context)
            .expect("命令上下文必须可解析")
            .into()
    });
    let binding = KeyBinding::load(combo, action, predicate, false, None, &DummyKeyboardMapper)
        .expect("键位组合必须有效");
    cx.bind_keys([binding]);
}

impl AppRoot {
    /// 启动装载持久键位段并把差异应用到默认键位之上。
    pub(crate) fn initialize_keybind_config(&mut self, cx: &mut App) {
        let pairs = self
            .settings
            .values()
            .external_value(KEYBIND_SEGMENT)
            .map(keybind_pairs_from_json)
            .unwrap_or_default();
        let config = KeybindConfig::from_pairs(&pairs);
        let old = std::mem::take(&mut self.keybind_config);
        apply_bindings(&old, &config, cx);
        self.keybind_config = config.clone();
        cx.set_global(KeybindRuntime {
            config,
            conflicts: Vec::new(),
            recording_error: None,
            recording_error_combo: String::new(),
        });
        cx.set_global(KeybindRecording::default());
    }

    /// 录制或修复后的配置：热应用键位、写入键位段并广播运行值。
    pub(crate) fn apply_keybind_config(
        &mut self,
        config: KeybindConfig,
        conflicts: Vec<CommandId>,
        recording_error: Option<RecordingError>,
        cx: &mut App,
    ) {
        let old = std::mem::replace(&mut self.keybind_config, config.clone());
        apply_bindings(&old, &config, cx);
        let now = std::time::Instant::now();
        let _ = self
            .settings
            .queue_explicit(&[(KEYBIND_SEGMENT, config.value())], now);
        self.bump_settings_revision();
        cx.set_global(KeybindRuntime {
            config,
            conflicts,
            recording_error,
            recording_error_combo: String::new(),
        });
    }
}
