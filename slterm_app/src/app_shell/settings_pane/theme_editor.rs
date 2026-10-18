//! 原生主题编辑器:草稿/基线双快照 + 首次保存恒 fork。
//!
//! 承重语义(06 篇):
//! - 编辑器持有具体 `draft`(编辑中的值)与 `baseline`(最近已持久化快照,
//!   回滚/脏判定基准);模板切换只替换 draft,源主题与已持久化运行时不动。
//! - 首次保存恒 fork:无论模板是内置还是他人自定义主题,都经
//!   `ThemeLibraryStore::import` 生成新库快照;后续保存才用快照修订做
//!   乐观并发。
//! - 保存与应用是两个动作:保存只落主题库,不碰应用键;应用先条件写入
//!   应用键(携带编辑前观察到的设置字节快照),再热应用并关闭。
//! - 取色推荐:`foreground_recommendations` 原色恒第一,候选只供点选,
//!   用户颜色绝不静默改写;保存路径对不达标组合直接拒绝(WCAG)。

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px, rgb,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::{ActiveTheme as _, Disableable as _, IndexPath, h_flex, v_flex};
use slterm_settings::{
    BlurMode, Rgb8, ThemeDefinition, ThemeName, foreground_recommendations, format_hex_rgb,
    parse_hex_rgb,
};

use crate::settings_store::SettingsRevision;
use crate::theme_library::{
    RevisionPrecondition, ThemeDocument, builtin_documents, from_definition, preferences,
};

use crate::i18n::Message;

use super::SettingsPane;
use super::design::SETTINGS_SELECT_WIDTH;
use super::localization::UiLanguage;

/// 编辑器输入槽(无效文本与最后一次有效草稿值分离)。
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum EditorInput {
    Name,
    Background,
    Foreground,
    Accent,
    Cursor,
    FontFamily,
    FontSize,
    LineHeight,
    LetterSpacing,
    FontWeight,
    Opacity,
    LayoutRadius,
    LayoutGutter,
    LayoutDivider,
    PaddingX,
    PaddingY,
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl EditorInput {
    /// 测试辅助:按稳定字段名取输入槽,避免把内部枚举暴露到 `ui_tests`。
    fn from_test_name(name: &str) -> Option<Self> {
        Some(match name {
            "name" => Self::Name,
            "background" => Self::Background,
            "foreground" => Self::Foreground,
            "accent" => Self::Accent,
            "cursor" => Self::Cursor,
            "font_family" => Self::FontFamily,
            "font_size" => Self::FontSize,
            "line_height" => Self::LineHeight,
            "letter_spacing" => Self::LetterSpacing,
            "font_weight" => Self::FontWeight,
            "opacity" => Self::Opacity,
            "layout_radius" => Self::LayoutRadius,
            "layout_gutter" => Self::LayoutGutter,
            "layout_divider" => Self::LayoutDivider,
            "padding_x" => Self::PaddingX,
            "padding_y" => Self::PaddingY,
            _ => return None,
        })
    }
}

/// 颜色槽(常用四色)。生产路径当前只从取色推荐点选前景;其余槽由输入框
/// 编辑,枚举保留给测试与后续取色面板统一入口。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum ThemeColorSlot {
    Background,
    Foreground,
    Accent,
    Cursor,
}

/// 一个模板条目:内置或库内自定义主题。
#[derive(Clone)]
struct ThemeTemplate {
    definition: ThemeDefinition,
    document: Option<ThemeDocument>,
    label: SharedString,
}

/// 主题编辑会话。
pub(super) struct ThemeEditor {
    pub(super) draft: ThemeDefinition,
    baseline: ThemeDefinition,
    /// 编辑器打开时载入的源文档(不可变);首次保存恒 fork 的依据。
    pub(super) fork_source: Option<ThemeDocument>,
    /// 最近一次已保存库快照;None = 尚未保存,下一次保存必须 fork。
    saved_document: Option<ThemeDocument>,
    /// 导入文档保留 vendor 字段,保存仍是新库快照。
    source_is_import: bool,
    pub(super) template: ThemeName,
    templates: Vec<ThemeTemplate>,
    selected_template: usize,
    pub(super) template_select: Entity<SelectState<Vec<SharedString>>>,
    /// 背景模糊选择器:空选择 = 跟随全局设置(`effects.blur` 为 None)。
    blur_select: Entity<SelectState<Vec<SharedString>>>,
    name_input: Entity<InputState>,
    background_input: Entity<InputState>,
    foreground_input: Entity<InputState>,
    accent_input: Entity<InputState>,
    cursor_input: Entity<InputState>,
    font_input: Entity<InputState>,
    font_size_input: Entity<InputState>,
    line_height_input: Entity<InputState>,
    letter_spacing_input: Entity<InputState>,
    font_weight_input: Entity<InputState>,
    opacity_input: Entity<InputState>,
    layout_radius_input: Entity<InputState>,
    layout_gutter_input: Entity<InputState>,
    layout_divider_input: Entity<InputState>,
    padding_x_input: Entity<InputState>,
    padding_y_input: Entity<InputState>,
    /// 最后一次写入输入框的显示值,防程序回填触发 Change 循环。
    input_values: HashMap<EditorInput, String>,
    /// 当前无效的输入槽;非空时保存被拒。
    invalid_inputs: HashSet<EditorInput>,
    /// 打开编辑器时观察到的设置字节快照(应用路径乐观并发)。
    preference_revision: Result<SettingsRevision, String>,
    pub(super) error: Option<String>,
    pub(super) saved: bool,
    pub(super) save_busy: bool,
    _subscriptions: Vec<gpui::Subscription>,
}

impl ThemeEditor {
    fn dirty(&self) -> bool {
        self.source_is_import || self.draft != self.baseline || !self.invalid_inputs.is_empty()
    }

    fn selected_index(&self) -> usize {
        self.selected_template
            .min(self.templates.len().saturating_sub(1))
    }
}

impl SettingsPane {
    /// 打开编辑器:草稿 = 基线 = 源快照;不写盘、不改运行时。
    pub(super) fn open_theme_editor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme_editor.is_some() {
            return;
        }
        let store = self.settings_target.theme_store();
        let snapshot = store.list().unwrap_or_default();
        let active_custom = self.runtime.custom_theme.as_deref().and_then(|id| {
            snapshot
                .custom
                .iter()
                .find(|doc| doc.id() == Some(id))
                .cloned()
        });
        let template = active_custom
            .as_ref()
            .and_then(|document| document.definition().ok())
            .map(|definition| definition.base)
            .unwrap_or(self.runtime.theme);
        let mut baseline = active_custom
            .as_ref()
            .and_then(|document| document.definition().ok())
            .unwrap_or_else(|| ThemeDefinition::from_builtin(template));
        let original_name = baseline.name.clone();
        let language = self.language(cx);
        baseline.name = language.format_new_theme_name(&original_name);
        if baseline.validate_wcag().is_err() || baseline.validate().is_err() {
            baseline.name = original_name;
        }
        let draft = baseline.clone();
        let templates = self.theme_templates(&snapshot, active_custom.as_ref());
        let selected = active_custom
            .as_ref()
            .and_then(|document| document.id())
            .and_then(|id| {
                templates.iter().position(|template| {
                    template.document.as_ref().and_then(ThemeDocument::id) == Some(id)
                })
            })
            .unwrap_or_else(|| {
                templates
                    .iter()
                    .position(|candidate| candidate.definition.base == template)
                    .unwrap_or(0)
            });
        let labels = templates
            .iter()
            .map(|template| template.label.clone())
            .collect::<Vec<_>>();
        let template_select = cx.new(|cx| {
            SelectState::new(labels, Some(IndexPath::default().row(selected)), window, cx)
        });
        // 模糊选项顺序与 `BlurMode::VALUES` 全等;None 选择 = 未设置。
        let blur_labels = BlurMode::VALUES
            .iter()
            .filter_map(|value| BlurMode::from_settings(value))
            .map(|mode| SharedString::from(blur_option_label(mode, language)))
            .collect::<Vec<_>>();
        let blur_selected = draft.effects.blur.and_then(|mode| {
            BlurMode::VALUES
                .iter()
                .position(|value| *value == mode.settings_value())
        });
        let blur_select = cx.new(|cx| {
            SelectState::new(
                blur_labels,
                blur_selected.map(|row| IndexPath::default().row(row)),
                window,
                cx,
            )
        });
        let mut make_input = |value: String, placeholder: &'static str| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let name_input = make_input(draft.name.clone(), "Theme name");
        let background_input = make_input(format_hex_rgb(draft.terminal.background), "#rrggbb");
        let foreground_input = make_input(format_hex_rgb(draft.terminal.foreground), "#rrggbb");
        let accent_input = make_input(format_hex_rgb(draft.resolved_ui().accent), "#rrggbb");
        let cursor_value = draft.terminal.cursor.unwrap_or(draft.terminal.foreground);
        let cursor_input = make_input(format_hex_rgb(cursor_value), "#rrggbb");
        let font_input = make_input(
            draft.typography.font_family.clone().unwrap_or_default(),
            "Cascadia Code",
        );
        let font_size_input = make_input(
            draft
                .typography
                .font_size
                .map_or_else(String::new, |value| format!("{value:.1}")),
            "15",
        );
        let line_height_input = make_input(
            draft
                .typography
                .line_height
                .map_or_else(String::new, |value| format!("{value:.2}")),
            "1.60",
        );
        let letter_spacing_input = make_input(
            draft
                .typography
                .letter_spacing
                .map_or_else(String::new, |value| format!("{value:.2}")),
            "0",
        );
        let font_weight_input = make_input(
            draft
                .typography
                .font_weight
                .map_or_else(String::new, |value| value.to_string()),
            "400",
        );
        let opacity_input = make_input(
            draft.effects.opacity.map_or_else(String::new, |value| {
                format!("{}%", (value * 100.0).round() as u16)
            }),
            "100%",
        );
        let layout_radius_input = make_input(format!("{:.1}", draft.layout.card.radius), "14");
        let layout_gutter_input = make_input(format!("{:.1}", draft.layout.card.gutter), "8");
        let layout_divider_input = make_input(format!("{:.2}", draft.layout.card.divider), "1");
        let padding_x_input = make_input(
            draft
                .layout
                .padding_x
                .map_or_else(String::new, |value| format!("{value:.1}")),
            "0",
        );
        let padding_y_input = make_input(
            draft
                .layout
                .padding_y
                .map_or_else(String::new, |value| format!("{value:.1}")),
            "0",
        );
        let preference_revision = self
            .settings_target
            .revision()
            .map_err(|error| error.to_string());
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &template_select,
            window,
            |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if !matches!(event, SelectEvent::Confirm(Some(_))) {
                    return;
                }
                let Some(index) = entity.read(cx).selected_index(cx).map(|path| path.row) else {
                    return;
                };
                this.switch_theme_template(index, window, cx);
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &blur_select,
            window,
            |this, entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                // `Confirm(None)` = 清除选择(跟随全局设置)。
                if !matches!(event, SelectEvent::Confirm(_)) {
                    return;
                }
                let row = entity.read(cx).selected_index(cx).map(|path| path.row);
                this.on_theme_editor_blur_selected(row, window, cx);
            },
        ));
        let mut register_input =
            |input: &Entity<InputState>,
             field: EditorInput,
             subscriptions: &mut Vec<gpui::Subscription>| {
                subscriptions.push(cx.subscribe_in(
                    input,
                    window,
                    move |this, _, event: &InputEvent, window, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.on_theme_editor_input(field, window, cx);
                        }
                    },
                ));
            };
        register_input(&name_input, EditorInput::Name, &mut subscriptions);
        register_input(
            &background_input,
            EditorInput::Background,
            &mut subscriptions,
        );
        register_input(
            &foreground_input,
            EditorInput::Foreground,
            &mut subscriptions,
        );
        register_input(&accent_input, EditorInput::Accent, &mut subscriptions);
        register_input(&cursor_input, EditorInput::Cursor, &mut subscriptions);
        register_input(&font_input, EditorInput::FontFamily, &mut subscriptions);
        register_input(&font_size_input, EditorInput::FontSize, &mut subscriptions);
        register_input(
            &line_height_input,
            EditorInput::LineHeight,
            &mut subscriptions,
        );
        register_input(
            &letter_spacing_input,
            EditorInput::LetterSpacing,
            &mut subscriptions,
        );
        register_input(
            &font_weight_input,
            EditorInput::FontWeight,
            &mut subscriptions,
        );
        register_input(&opacity_input, EditorInput::Opacity, &mut subscriptions);
        register_input(
            &layout_radius_input,
            EditorInput::LayoutRadius,
            &mut subscriptions,
        );
        register_input(
            &layout_gutter_input,
            EditorInput::LayoutGutter,
            &mut subscriptions,
        );
        register_input(
            &layout_divider_input,
            EditorInput::LayoutDivider,
            &mut subscriptions,
        );
        register_input(&padding_x_input, EditorInput::PaddingX, &mut subscriptions);
        register_input(&padding_y_input, EditorInput::PaddingY, &mut subscriptions);
        let mut input_values = HashMap::new();
        for (field, input) in [
            (EditorInput::Name, &name_input),
            (EditorInput::Background, &background_input),
            (EditorInput::Foreground, &foreground_input),
            (EditorInput::Accent, &accent_input),
            (EditorInput::Cursor, &cursor_input),
            (EditorInput::FontFamily, &font_input),
            (EditorInput::FontSize, &font_size_input),
            (EditorInput::LineHeight, &line_height_input),
            (EditorInput::LetterSpacing, &letter_spacing_input),
            (EditorInput::FontWeight, &font_weight_input),
            (EditorInput::Opacity, &opacity_input),
            (EditorInput::LayoutRadius, &layout_radius_input),
            (EditorInput::LayoutGutter, &layout_gutter_input),
            (EditorInput::LayoutDivider, &layout_divider_input),
            (EditorInput::PaddingX, &padding_x_input),
            (EditorInput::PaddingY, &padding_y_input),
        ] {
            input_values.insert(field, input.read(cx).value().to_string());
        }
        let focus = cx.focus_handle();
        self.theme_editor = Some(ThemeEditor {
            draft,
            baseline,
            fork_source: active_custom.clone(),
            saved_document: None,
            source_is_import: false,
            template,
            templates,
            selected_template: selected,
            template_select,
            blur_select,
            name_input,
            background_input,
            foreground_input,
            accent_input,
            cursor_input,
            font_input,
            font_size_input,
            line_height_input,
            letter_spacing_input,
            font_weight_input,
            opacity_input,
            layout_radius_input,
            layout_gutter_input,
            layout_divider_input,
            padding_x_input,
            padding_y_input,
            input_values,
            invalid_inputs: HashSet::new(),
            preference_revision,
            error: None,
            saved: false,
            save_busy: false,
            _subscriptions: subscriptions,
        });
        window.focus(&focus, cx);
        cx.notify();
    }

    fn theme_templates(
        &self,
        snapshot: &crate::theme_library::ThemeLibrarySnapshot,
        active_custom: Option<&ThemeDocument>,
    ) -> Vec<ThemeTemplate> {
        let mut templates = builtin_documents()
            .into_iter()
            .filter_map(|document| {
                let definition = document.definition().ok()?;
                Some(ThemeTemplate {
                    label: SharedString::from(document.name().to_owned()),
                    definition,
                    document: None,
                })
            })
            .collect::<Vec<_>>();
        for document in &snapshot.custom {
            let Ok(definition) = document.definition() else {
                continue;
            };
            templates.push(ThemeTemplate {
                label: SharedString::from(document.name().to_owned()),
                definition,
                document: Some(document.clone()),
            });
        }
        if let Some(document) = active_custom {
            let present = document.id().is_some_and(|id| {
                templates.iter().any(|template| {
                    template.document.as_ref().and_then(ThemeDocument::id) == Some(id)
                })
            });
            if !present && let Ok(definition) = document.definition() {
                templates.push(ThemeTemplate {
                    label: SharedString::from(document.name().to_owned()),
                    definition,
                    document: Some(document.clone()),
                });
            }
        }
        templates
    }

    /// 模板切换只替换 draft/baseline;源主题与已持久化运行时不动。
    pub(super) fn switch_theme_template(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self.theme_editor.as_ref() else {
            return;
        };
        if editor.save_busy || index >= editor.templates.len() || index == editor.selected_index() {
            return;
        }
        let template = editor.templates[index].clone();
        let name = editor.draft.name.clone();
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        let mut draft = template.definition.clone();
        // 编辑器命名的是自定义结果,换模板不改用户正在编辑的名字。
        draft.name = name;
        editor.template = draft.base;
        editor.selected_template = index;
        editor.fork_source = template.document.clone();
        editor.saved_document = None;
        editor.source_is_import = false;
        editor.baseline = draft.clone();
        editor.draft = draft;
        editor.saved = false;
        editor.invalid_inputs.clear();
        editor.error = None;
        editor.template_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::default().row(index)), window, cx);
        });
        self.sync_theme_editor_inputs(window, cx);
        cx.notify();
    }

    fn on_theme_editor_input(
        &mut self,
        field: EditorInput,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self.theme_editor.as_ref() else {
            return;
        };
        if editor.save_busy {
            return;
        }
        let Some(input) = editor_input(editor, field) else {
            return;
        };
        let value = input.read(cx).value().to_string();
        let invalid_message = self
            .language(cx)
            .text(Message::SettingsThemeEditorInvalidInputs)
            .to_owned();
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        if editor.input_values.get(&field) == Some(&value) {
            return;
        }
        editor.input_values.insert(field, value.clone());
        let mut candidate = editor.draft.clone();
        let parsed = match field {
            EditorInput::Name => {
                candidate.name = value.clone();
                true
            }
            EditorInput::Background => parse_hex_rgb(&value)
                .map(|color| candidate.terminal.background = color)
                .is_some(),
            EditorInput::Foreground => parse_hex_rgb(&value)
                .map(|color| candidate.terminal.foreground = color)
                .is_some(),
            EditorInput::Accent => parse_hex_rgb(&value)
                .map(|color| {
                    candidate.ui = candidate.resolved_ui();
                    candidate.ui.derive = false;
                    candidate.ui.accent = color;
                })
                .is_some(),
            EditorInput::Cursor => parse_hex_rgb(&value)
                .map(|color| candidate.terminal.cursor = Some(color))
                .is_some(),
            EditorInput::FontFamily => {
                candidate.typography.font_family =
                    (!value.trim().is_empty()).then(|| value.trim().to_owned());
                true
            }
            EditorInput::FontSize => value
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.typography.font_size = Some(number))
                .is_some(),
            EditorInput::LineHeight => value
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.typography.line_height = Some(number))
                .is_some(),
            EditorInput::LetterSpacing => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    // 空 = 跟随全局设置(与字体族同一语义)。
                    candidate.typography.letter_spacing = None;
                    true
                } else {
                    trimmed
                        .parse::<f32>()
                        .ok()
                        .map(|number| candidate.typography.letter_spacing = Some(number))
                        .is_some()
                }
            }
            EditorInput::FontWeight => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    candidate.typography.font_weight = None;
                    true
                } else {
                    trimmed
                        .parse::<u16>()
                        .ok()
                        .map(|weight| candidate.typography.font_weight = Some(weight))
                        .is_some()
                }
            }
            EditorInput::Opacity => value
                .trim()
                .trim_end_matches('%')
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.effects.opacity = Some(number / 100.0))
                .is_some(),
            EditorInput::LayoutRadius => value
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.layout.card.radius = number)
                .is_some(),
            EditorInput::LayoutGutter => value
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.layout.card.gutter = number)
                .is_some(),
            EditorInput::LayoutDivider => value
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| candidate.layout.card.divider = number)
                .is_some(),
            EditorInput::PaddingX => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    candidate.layout.padding_x = None;
                    true
                } else {
                    trimmed
                        .parse::<f32>()
                        .ok()
                        .map(|number| candidate.layout.padding_x = Some(number))
                        .is_some()
                }
            }
            EditorInput::PaddingY => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    candidate.layout.padding_y = None;
                    true
                } else {
                    trimmed
                        .parse::<f32>()
                        .ok()
                        .map(|number| candidate.layout.padding_y = Some(number))
                        .is_some()
                }
            }
        };
        // 共享值模型是标量范围权威;只有完整合法的候选进入草稿。
        if parsed && candidate.validate().is_ok() {
            editor.draft = candidate;
            editor.invalid_inputs.remove(&field);
        } else {
            editor.invalid_inputs.insert(field);
        }
        editor.saved = false;
        editor.error = (!editor.invalid_inputs.is_empty()).then_some(invalid_message);
        cx.notify();
    }

    /// 模糊选择确认;`None` = 清除为跟随全局设置。
    fn on_theme_editor_blur_selected(
        &mut self,
        row: Option<usize>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let next = match row {
            None => None,
            Some(row) => match BlurMode::VALUES
                .get(row)
                .and_then(|value| BlurMode::from_settings(value))
            {
                Some(mode) => Some(mode),
                None => return,
            },
        };
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        if editor.save_busy || editor.draft.effects.blur == next {
            return;
        }
        editor.draft.effects.blur = next;
        editor.saved = false;
        cx.notify();
    }

    /// 取色推荐点选(原色恒第一;候选只供建议)。
    pub(super) fn set_theme_color(
        &mut self,
        slot: ThemeColorSlot,
        color: Rgb8,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        match slot {
            ThemeColorSlot::Background => editor.draft.terminal.background = color,
            ThemeColorSlot::Foreground => editor.draft.terminal.foreground = color,
            ThemeColorSlot::Accent => {
                editor.draft.ui = editor.draft.resolved_ui();
                editor.draft.ui.derive = false;
                editor.draft.ui.accent = color;
            }
            ThemeColorSlot::Cursor => editor.draft.terminal.cursor = Some(color),
        }
        editor.saved = false;
        editor.invalid_inputs.remove(&match slot {
            ThemeColorSlot::Background => EditorInput::Background,
            ThemeColorSlot::Foreground => EditorInput::Foreground,
            ThemeColorSlot::Accent => EditorInput::Accent,
            ThemeColorSlot::Cursor => EditorInput::Cursor,
        });
        self.sync_theme_editor_inputs(window, cx);
        cx.notify();
    }

    fn sync_theme_editor_inputs(&mut self, window: &mut Window, cx: &mut App) {
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        let values = [
            (EditorInput::Name, editor.draft.name.clone()),
            (
                EditorInput::Background,
                format_hex_rgb(editor.draft.terminal.background),
            ),
            (
                EditorInput::Foreground,
                format_hex_rgb(editor.draft.terminal.foreground),
            ),
            (
                EditorInput::Accent,
                format_hex_rgb(editor.draft.resolved_ui().accent),
            ),
            (
                EditorInput::Cursor,
                format_hex_rgb(
                    editor
                        .draft
                        .terminal
                        .cursor
                        .unwrap_or(editor.draft.terminal.foreground),
                ),
            ),
            (
                EditorInput::FontFamily,
                editor
                    .draft
                    .typography
                    .font_family
                    .clone()
                    .unwrap_or_default(),
            ),
            (
                EditorInput::FontSize,
                editor
                    .draft
                    .typography
                    .font_size
                    .map_or_else(String::new, |value| format!("{value:.1}")),
            ),
            (
                EditorInput::LineHeight,
                editor
                    .draft
                    .typography
                    .line_height
                    .map_or_else(String::new, |value| format!("{value:.2}")),
            ),
            (
                EditorInput::LetterSpacing,
                editor
                    .draft
                    .typography
                    .letter_spacing
                    .map_or_else(String::new, |value| format!("{value:.2}")),
            ),
            (
                EditorInput::FontWeight,
                editor
                    .draft
                    .typography
                    .font_weight
                    .map_or_else(String::new, |value| value.to_string()),
            ),
            (
                EditorInput::Opacity,
                editor
                    .draft
                    .effects
                    .opacity
                    .map_or_else(String::new, |value| {
                        format!("{}%", (value * 100.0).round() as u16)
                    }),
            ),
            (
                EditorInput::LayoutRadius,
                format!("{:.1}", editor.draft.layout.card.radius),
            ),
            (
                EditorInput::LayoutGutter,
                format!("{:.1}", editor.draft.layout.card.gutter),
            ),
            (
                EditorInput::LayoutDivider,
                format!("{:.2}", editor.draft.layout.card.divider),
            ),
            (
                EditorInput::PaddingX,
                editor
                    .draft
                    .layout
                    .padding_x
                    .map_or_else(String::new, |value| format!("{value:.1}")),
            ),
            (
                EditorInput::PaddingY,
                editor
                    .draft
                    .layout
                    .padding_y
                    .map_or_else(String::new, |value| format!("{value:.1}")),
            ),
        ];
        for (field, value) in values {
            editor.input_values.insert(field, value.clone());
            if let Some(input) = editor_input(editor, field) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        let blur_row = editor.draft.effects.blur.and_then(|mode| {
            BlurMode::VALUES
                .iter()
                .position(|value| *value == mode.settings_value())
        });
        editor.blur_select.update(cx, |state, cx| {
            state.set_selected_index(
                blur_row.map(|row| IndexPath::default().row(row)),
                window,
                cx,
            );
        });
    }

    /// 保存:首次恒 fork(import),后续用快照修订做乐观并发。
    /// `apply` = 保存成功后条件写入应用键并热应用(两个动作可分开)。
    pub(super) fn save_theme_editor(
        &mut self,
        apply: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(editor) = self.theme_editor.as_ref() else {
            return false;
        };
        if editor.save_busy {
            return false;
        }
        if !editor.invalid_inputs.is_empty() {
            let message = self
                .language(cx)
                .text(Message::SettingsThemeEditorInvalidInputs)
                .to_owned();
            if let Some(editor) = self.theme_editor.as_mut() {
                editor.error = Some(message);
            }
            cx.notify();
            return false;
        }
        if let Err(error) = editor.draft.validate() {
            if let Some(editor) = self.theme_editor.as_mut() {
                editor.error = Some(error.to_string());
            }
            cx.notify();
            return false;
        }
        // WCAG 保存路径:不达标组合拒绝,用户颜色不被静默改写。
        if let Err(error) = editor.draft.validate_wcag() {
            if let Some(editor) = self.theme_editor.as_mut() {
                editor.error = Some(error.to_string());
            }
            cx.notify();
            return false;
        }
        let draft = editor.draft.clone();
        let fork_source = editor.fork_source.clone();
        let saved_document = editor.saved_document.clone();
        let candidate = match fork_source.as_ref() {
            Some(source) => source.with_definition(&draft),
            None => from_definition(&draft),
        };
        let candidate = match candidate {
            Ok(document) => document,
            Err(error) => {
                if let Some(editor) = self.theme_editor.as_mut() {
                    editor.error = Some(error.to_string());
                }
                cx.notify();
                return false;
            }
        };
        let store = self.settings_target.theme_store();
        let stored = match saved_document {
            // 首次保存恒 fork:不覆盖内置/他人主题,也不改激活快照。
            None => store.import(&candidate),
            Some(current) => store.save(&candidate, RevisionPrecondition::Unchanged(current)),
        };
        let stored = match stored {
            Ok(document) => document,
            Err(error) => {
                if let Some(editor) = self.theme_editor.as_mut() {
                    editor.error = Some(error.to_string());
                }
                cx.notify();
                return false;
            }
        };
        let stored_definition = stored
            .definition()
            .expect("stored theme documents are validated");
        if let Some(editor) = self.theme_editor.as_mut() {
            editor.saved_document = Some(stored.clone());
            editor.fork_source = Some(stored.clone());
            editor.source_is_import = false;
            editor.baseline = stored_definition.clone();
            editor.draft = stored_definition;
            editor.saved = true;
            editor.error = None;
            editor.invalid_inputs.clear();
        }
        self.sync_theme_editor_inputs(window, cx);
        if !apply {
            self.status = None;
            cx.notify();
            return true;
        }
        self.apply_saved_theme(&stored, window, cx)
    }

    /// 应用已保存主题:条件写应用键 → 重载单一事实源 → 热应用 → 关闭。
    fn apply_saved_theme(
        &mut self,
        document: &ThemeDocument,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let revision = match self
            .theme_editor
            .as_ref()
            .map(|editor| editor.preference_revision.clone())
        {
            Some(Ok(revision)) => revision,
            Some(Err(error)) => {
                if let Some(editor) = self.theme_editor.as_mut() {
                    editor.error = Some(error);
                }
                cx.notify();
                return false;
            }
            None => return false,
        };
        let updates = match preferences::custom_theme_updates(document, None) {
            Ok(updates) => updates,
            Err(error) => {
                if let Some(editor) = self.theme_editor.as_mut() {
                    editor.error = Some(error.to_string());
                }
                cx.notify();
                return false;
            }
        };
        // A2:条件写盘入队后台执行;成功后由 `finish_persist` 关闭编辑器并
        // 应用回读(语言变化时刷新显示项)。
        self.try_persist_checked(&updates, &revision, true, cx);
        self.status = None;
        cx.notify();
        true
    }

    fn request_close_theme_editor(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        if self
            .theme_editor
            .as_ref()
            .is_some_and(|editor| editor.save_busy)
        {
            return;
        }
        self.theme_editor = None;
        cx.notify();
    }

    fn reset_theme_editor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        if editor.save_busy {
            return;
        }
        editor.draft = editor.baseline.clone();
        editor.saved = false;
        editor.invalid_inputs.clear();
        editor.error = None;
        self.sync_theme_editor_inputs(window, cx);
        cx.notify();
    }

    /// 编辑器主视图(外观页的子视图)。
    pub(super) fn page_theme_editor(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let Some(editor) = self.theme_editor.as_ref() else {
            return v_flex();
        };
        let draft = editor.draft.clone();
        let template_select = editor.template_select.clone();
        let blur_select = editor.blur_select.clone();
        let name_input = editor.name_input.clone();
        let background_input = editor.background_input.clone();
        let foreground_input = editor.foreground_input.clone();
        let accent_input = editor.accent_input.clone();
        let cursor_input = editor.cursor_input.clone();
        let font_input = editor.font_input.clone();
        let font_size_input = editor.font_size_input.clone();
        let line_height_input = editor.line_height_input.clone();
        let letter_spacing_input = editor.letter_spacing_input.clone();
        let font_weight_input = editor.font_weight_input.clone();
        let opacity_input = editor.opacity_input.clone();
        let layout_radius_input = editor.layout_radius_input.clone();
        let layout_gutter_input = editor.layout_gutter_input.clone();
        let layout_divider_input = editor.layout_divider_input.clone();
        let padding_x_input = editor.padding_x_input.clone();
        let padding_y_input = editor.padding_y_input.clone();
        let error = editor.error.clone();
        let saved = editor.saved;
        let dirty = editor.dirty();
        let busy = editor.save_busy;
        let recommendations =
            foreground_recommendations(draft.terminal.background, draft.terminal.foreground);
        let original_foreground = draft.terminal.foreground;
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let border = cx.theme().border;

        let mut page = v_flex()
            .w_full()
            .gap(px(super::design::SETTINGS_GROUP_GAP))
            .child(
                self.group(language.text(Message::SettingsThemeEditorGroup), cx)
                    .child(
                        self.row(
                            language.text(Message::SettingsThemeEditorTemplate),
                            language.text(Message::SettingsThemeEditorTemplateDescription),
                            div()
                                .debug_selector(|| "settings-theme-template".to_owned())
                                .w(px(SETTINGS_SELECT_WIDTH))
                                .child(Select::new(&template_select).appearance(false)),
                            cx,
                        ),
                    )
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorName),
                        language.text(Message::SettingsThemeEditorNameDescription),
                        editor_input_box("settings-theme-name", &name_input, false, border),
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsThemeEditorColors), cx)
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorBackground),
                        "#RRGGBB",
                        editor_input_box(
                            "settings-theme-background",
                            &background_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(
                        self.row(
                            language.text(Message::SettingsThemeEditorForeground),
                            language.text(Message::SettingsThemeEditorForegroundDescription),
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(editor_input_box(
                                    "settings-theme-foreground",
                                    &foreground_input,
                                    false,
                                    border,
                                ))
                                .children(recommendations.into_iter().enumerate().map(
                                    |(index, color)| {
                                        let selected = color == original_foreground;
                                        div()
                                            .id(SharedString::from(format!(
                                                "settings-theme-recommendation-{index}"
                                            )))
                                            .debug_selector(move || {
                                                format!("settings-theme-recommendation-{index}")
                                            })
                                            .size(px(24.0))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .bg(rgb(u32::from_be_bytes([
                                                0, color[0], color[1], color[2],
                                            ])))
                                            .border_1()
                                            .border_color(if selected {
                                                cx.theme().foreground
                                            } else {
                                                border
                                            })
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.set_theme_color(
                                                    ThemeColorSlot::Foreground,
                                                    color,
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .into_any_element()
                                    },
                                )),
                            cx,
                        ),
                    )
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorAccent),
                        "#RRGGBB",
                        editor_input_box("settings-theme-accent", &accent_input, false, border),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorCursor),
                        "#RRGGBB",
                        editor_input_box("settings-theme-cursor", &cursor_input, false, border),
                        cx,
                    )),
            )
            .child(
                self.group(language.text(Message::SettingsThemeEditorTypography), cx)
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorFontFamily),
                        language.text(Message::SettingsThemeEditorFontFamilyDescription),
                        editor_input_box("settings-theme-font", &font_input, false, border),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorFontSize),
                        "4–96",
                        editor_input_box(
                            "settings-theme-font-size",
                            &font_size_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorLineHeight),
                        "0.5–3.0",
                        editor_input_box(
                            "settings-theme-line-height",
                            &line_height_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorLetterSpacing),
                        language.text(Message::SettingsThemeEditorLetterSpacingDescription),
                        editor_input_box(
                            "settings-theme-letter-spacing",
                            &letter_spacing_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorFontWeight),
                        language.text(Message::SettingsThemeEditorFontWeightDescription),
                        editor_input_box(
                            "settings-theme-font-weight",
                            &font_weight_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorOpacity),
                        "0–100%",
                        editor_input_box("settings-theme-opacity", &opacity_input, false, border),
                        cx,
                    ))
                    .child(
                        self.row(
                            language.text(Message::SettingsThemeEditorBlur),
                            language.text(Message::SettingsThemeEditorBlurDescription),
                            div()
                                .debug_selector(|| "settings-theme-blur".to_owned())
                                .w(px(SETTINGS_SELECT_WIDTH))
                                .child(
                                    Select::new(&blur_select)
                                        .appearance(false)
                                        .placeholder(
                                            language
                                                .text(Message::SettingsThemeEditorFollowSettings),
                                        )
                                        .cleanable(true),
                                ),
                            cx,
                        ),
                    ),
            )
            .child(
                self.group(language.text(Message::SettingsThemeEditorLayout), cx)
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorLayoutRadius),
                        "0–28",
                        editor_input_box(
                            "settings-theme-layout-radius",
                            &layout_radius_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorLayoutGutter),
                        "0–32",
                        editor_input_box(
                            "settings-theme-layout-gutter",
                            &layout_gutter_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorLayoutDivider),
                        "0–4",
                        editor_input_box(
                            "settings-theme-layout-divider",
                            &layout_divider_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorPaddingX),
                        "0–48",
                        editor_input_box(
                            "settings-theme-padding-x",
                            &padding_x_input,
                            false,
                            border,
                        ),
                        cx,
                    ))
                    .child(self.row(
                        language.text(Message::SettingsThemeEditorPaddingY),
                        "0–40",
                        editor_input_box(
                            "settings-theme-padding-y",
                            &padding_y_input,
                            false,
                            border,
                        ),
                        cx,
                    )),
            );
        if let Some(error) = error {
            page = page.child(
                div()
                    .debug_selector(|| "settings-theme-error".to_owned())
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
        page.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .text_color(muted)
                .child(
                    Button::new("settings-theme-back")
                        .label(language.text(Message::SettingsThemeEditorBack))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.request_close_theme_editor(window, cx);
                        })),
                )
                .child(
                    Button::new("settings-theme-reset")
                        .label(language.text(Message::SettingsThemeEditorResetDraft))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.reset_theme_editor(window, cx);
                        })),
                )
                .child(
                    Button::new("settings-theme-save")
                        .label(language.text(Message::CommonSave))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.save_theme_editor(false, window, cx);
                        })),
                )
                .child(
                    Button::new("settings-theme-apply")
                        .label(language.text(Message::SettingsThemeEditorSaveAndApply))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.save_theme_editor(true, window, cx);
                        })),
                )
                .child(if saved {
                    div()
                        .debug_selector(|| "settings-theme-saved".to_owned())
                        .text_sm()
                        .child(language.text(Message::SettingsThemeEditorSaved))
                        .into_any_element()
                } else if dirty {
                    div()
                        .debug_selector(|| "settings-theme-dirty".to_owned())
                        .text_sm()
                        .child(language.text(Message::SettingsThemeEditorUnsaved))
                        .into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
    }

    #[allow(dead_code)] // 消费点 = 取色面板/测试读取;推荐本身在视图内直接渲染。
    fn theme_editor_recommendations(&self) -> Option<[Rgb8; 3]> {
        let editor = self.theme_editor.as_ref()?;
        Some(foreground_recommendations(
            editor.draft.terminal.background,
            editor.draft.terminal.foreground,
        ))
    }
}

/// 输入框控件:固定宽度 + 无效槽视觉由错误条统一承载。
fn editor_input_box(
    selector: &'static str,
    input: &Entity<InputState>,
    disabled: bool,
    border: gpui::Hsla,
) -> AnyElement {
    div()
        .debug_selector(move || selector.to_owned())
        .w(px(SETTINGS_SELECT_WIDTH))
        .h(px(32.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .child(
            Input::new(input)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .disabled(disabled),
        )
        .into_any_element()
}

fn editor_input(editor: &ThemeEditor, field: EditorInput) -> Option<&Entity<InputState>> {
    Some(match field {
        EditorInput::Name => &editor.name_input,
        EditorInput::Background => &editor.background_input,
        EditorInput::Foreground => &editor.foreground_input,
        EditorInput::Accent => &editor.accent_input,
        EditorInput::Cursor => &editor.cursor_input,
        EditorInput::FontFamily => &editor.font_input,
        EditorInput::FontSize => &editor.font_size_input,
        EditorInput::LineHeight => &editor.line_height_input,
        EditorInput::LetterSpacing => &editor.letter_spacing_input,
        EditorInput::FontWeight => &editor.font_weight_input,
        EditorInput::Opacity => &editor.opacity_input,
        EditorInput::LayoutRadius => &editor.layout_radius_input,
        EditorInput::LayoutGutter => &editor.layout_gutter_input,
        EditorInput::LayoutDivider => &editor.layout_divider_input,
        EditorInput::PaddingX => &editor.padding_x_input,
        EditorInput::PaddingY => &editor.padding_y_input,
    })
}

/// 模糊选项文案;调用方按 `BlurMode::VALUES` 顺序取用。
fn blur_option_label(mode: BlurMode, language: UiLanguage) -> &'static str {
    match mode {
        BlurMode::None => language.text(Message::SettingsOptionsBlurNone),
        BlurMode::Mica => language.text(Message::SettingsOptionsMica),
        BlurMode::MicaAlt => language.text(Message::SettingsOptionsMicaAlt),
        BlurMode::Aero => language.text(Message::SettingsOptionsAero),
        BlurMode::Acrylic => language.text(Message::SettingsOptionsAcrylic),
    }
}

/// 语言侧的新主题名(值/显示分离:命名模板随语言,保存值不受影响)。
trait NewThemeName {
    fn format_new_theme_name(&self, name: &str) -> String;
}

impl NewThemeName for UiLanguage {
    fn format_new_theme_name(&self, name: &str) -> String {
        format!("{} {}", name, self.text(Message::CommonCopySuffix))
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl SettingsPane {
    pub(crate) fn theme_editor_open_for_test(&self) -> bool {
        self.theme_editor.is_some()
    }

    pub(crate) fn theme_editor_draft_for_test(&self) -> Option<&ThemeDefinition> {
        self.theme_editor.as_ref().map(|editor| &editor.draft)
    }

    pub(crate) fn theme_editor_baseline_for_test(&self) -> Option<&ThemeDefinition> {
        self.theme_editor.as_ref().map(|editor| &editor.baseline)
    }

    pub(crate) fn theme_editor_source_id_for_test(&self) -> Option<Option<String>> {
        use crate::theme_library::ThemeDocument;
        self.theme_editor.as_ref().map(|editor| {
            editor
                .fork_source
                .as_ref()
                .and_then(ThemeDocument::id)
                .map(str::to_owned)
        })
    }

    pub(crate) fn theme_editor_saved_id_for_test(&self) -> Option<Option<String>> {
        use crate::theme_library::ThemeDocument;
        self.theme_editor.as_ref().map(|editor| {
            editor
                .saved_document
                .as_ref()
                .and_then(ThemeDocument::id)
                .map(str::to_owned)
        })
    }

    pub(crate) fn theme_editor_saved_revision_for_test(&self) -> Option<u64> {
        use crate::theme_library::ThemeDocument;
        self.theme_editor
            .as_ref()
            .and_then(|editor| editor.saved_document.as_ref())
            .map(ThemeDocument::revision)
    }

    pub(crate) fn theme_editor_error_for_test(&self) -> Option<&str> {
        self.theme_editor
            .as_ref()
            .and_then(|editor| editor.error.as_deref())
    }

    pub(crate) fn theme_editor_recommendations_for_test(&self) -> Option<[Rgb8; 3]> {
        self.theme_editor_recommendations()
    }

    pub(crate) fn switch_theme_template_for_test(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.switch_theme_template(index, window, cx);
    }

    pub(crate) fn set_theme_color_for_test(
        &mut self,
        slot: ThemeColorSlot,
        color: Rgb8,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.set_theme_color(slot, color, window, cx);
    }

    pub(crate) fn save_theme_editor_for_test(
        &mut self,
        apply: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        self.save_theme_editor(apply, window, cx)
    }

    pub(crate) fn reset_theme_editor_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.reset_theme_editor(window, cx);
    }

    pub(crate) fn theme_editor_template_count_for_test(&self) -> usize {
        self.theme_editor
            .as_ref()
            .map_or(0, |editor| editor.templates.len())
    }

    /// 模拟输入框内容变更并跑与真实 Change 事件相同的解析路径。
    pub(crate) fn set_theme_editor_input_for_test(
        &mut self,
        field: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(field) = EditorInput::from_test_name(field) else {
            return;
        };
        let Some(input) = self
            .theme_editor
            .as_ref()
            .and_then(|editor| editor_input(editor, field))
        else {
            return;
        };
        input.update(cx, |state, cx| {
            state.set_value(SharedString::from(value.to_owned()), window, cx)
        });
        self.on_theme_editor_input(field, window, cx);
    }

    pub(crate) fn theme_editor_input_value_for_test(
        &self,
        field: &str,
        cx: &App,
    ) -> Option<String> {
        let field = EditorInput::from_test_name(field)?;
        let editor = self.theme_editor.as_ref()?;
        editor_input(editor, field).map(|input| input.read(cx).value().to_string())
    }

    /// 模拟模糊下拉确认(含清除);`row = None` 对应 `Confirm(None)`。
    pub(crate) fn set_theme_editor_blur_for_test(
        &mut self,
        row: Option<usize>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        editor.blur_select.update(cx, |state, cx| {
            state.set_selected_index(row.map(|row| IndexPath::default().row(row)), window, cx);
        });
        self.on_theme_editor_blur_selected(row, window, cx);
    }

    pub(crate) fn theme_editor_blur_index_for_test(&self, cx: &App) -> Option<usize> {
        self.theme_editor
            .as_ref()?
            .blur_select
            .read(cx)
            .selected_index(cx)
            .map(|path| path.row)
    }
}
