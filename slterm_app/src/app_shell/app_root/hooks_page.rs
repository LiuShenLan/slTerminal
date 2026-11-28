//! 设置页 Hooks 编辑：三层草稿的载入／保存／丢弃与只读认领组摘要。
//!
//! 文件 I/O 只在后台执行体运行；前台持有草稿文本与诊断，保存前由
//! `HooksDocument` 在锁内重读并比较基线指纹，外部修改零写入。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::Input;
use slterm_agents::AgentKind;
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::settings_overlay::SettingsView;
use super::task_service::{CancellationToken, TaskId};
use crate::ai_hook::draft::DraftParseError;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer, HooksDraftOwner};
use crate::ai_hook::editor::{HooksDocument, HooksEditError, validate_editable_all};
use crate::i18n::Message;

mod form;

pub(super) use form::HooksFormState;

/// Hooks 编辑模式：JSON 文本与结构化表单编辑同一草稿。
/// 瞬态界面状态，不进入 ViewState；重建视图恒回 JSON。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HooksEditMode {
    #[default]
    Json,
    Form,
}

impl HooksEditMode {
    const fn message(self) -> Message {
        match self {
            Self::Json => Message::SettingsAgentsModeJson,
            Self::Form => Message::SettingsAgentsModeForm,
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Form => "form",
        }
    }
}

/// Hooks 页失败类别：页面上下文失败或编辑域封闭失败。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HooksPageError {
    ProjectMissing,
    ConfigDirUnavailable,
    RootUnavailable,
    DocumentNotLoaded,
    BaselineMissing,
    Edit(HooksEditError),
}

fn hooks_edit_error_text(language: crate::i18n::UiLanguage, error: &HooksEditError) -> String {
    use Message::{
        SettingsAgentsHooksBackupFailed, SettingsAgentsHooksConfigDirMissing,
        SettingsAgentsHooksConfigDirUnavailable, SettingsAgentsHooksDuplicateKey,
        SettingsAgentsHooksEventNameInvalid, SettingsAgentsHooksEventNotArray,
        SettingsAgentsHooksExternalModified, SettingsAgentsHooksHandlerFieldEmpty,
        SettingsAgentsHooksHandlerNotObject, SettingsAgentsHooksHandlerTypeInvalid,
        SettingsAgentsHooksInvalidJson, SettingsAgentsHooksInvalidUtf8,
        SettingsAgentsHooksIoFailed, SettingsAgentsHooksLockBusy,
        SettingsAgentsHooksMatcherHooksMissing, SettingsAgentsHooksMatcherNotObject,
        SettingsAgentsHooksMatcherShapeInvalid, SettingsAgentsHooksRootNotObject,
        SettingsAgentsHooksSerializeFailed, SettingsAgentsHooksSubtreeNotObject,
        SettingsAgentsHooksUnauthorized,
    };
    match error {
        HooksEditError::ConfigDirUnavailable => language
            .text(SettingsAgentsHooksConfigDirUnavailable)
            .to_owned(),
        HooksEditError::ConfigDirMissing => language
            .text(SettingsAgentsHooksConfigDirMissing)
            .to_owned(),
        HooksEditError::RootNotObject => language.text(SettingsAgentsHooksRootNotObject).to_owned(),
        HooksEditError::HooksNotObject => language
            .text(SettingsAgentsHooksSubtreeNotObject)
            .to_owned(),
        HooksEditError::EventNotArray(event) => {
            language.format(SettingsAgentsHooksEventNotArray, &[("event", event)])
        }
        HooksEditError::MatcherShapeInvalid(event) => {
            language.format(SettingsAgentsHooksMatcherShapeInvalid, &[("event", event)])
        }
        HooksEditError::MatcherNotObject(event) => {
            language.format(SettingsAgentsHooksMatcherNotObject, &[("event", event)])
        }
        HooksEditError::MatcherHooksMissing(event) => {
            language.format(SettingsAgentsHooksMatcherHooksMissing, &[("event", event)])
        }
        HooksEditError::HandlerNotObject(event) => {
            language.format(SettingsAgentsHooksHandlerNotObject, &[("event", event)])
        }
        HooksEditError::HandlerTypeInvalid { event, allowed } => language.format(
            SettingsAgentsHooksHandlerTypeInvalid,
            &[("event", event), ("allowed", allowed)],
        ),
        HooksEditError::HandlerFieldEmpty { event, field } => language.format(
            SettingsAgentsHooksHandlerFieldEmpty,
            &[("event", event), ("field", field)],
        ),
        HooksEditError::EventNameInvalid { event, allowed } => language.format(
            SettingsAgentsHooksEventNameInvalid,
            &[("event", event), ("allowed", allowed)],
        ),
        HooksEditError::JsonSyntax(detail) => {
            language.format(SettingsAgentsHooksInvalidJson, &[("detail", detail)])
        }
        HooksEditError::InvalidUtf8 => language.text(SettingsAgentsHooksInvalidUtf8).to_owned(),
        HooksEditError::DuplicateKey(key) => {
            language.format(SettingsAgentsHooksDuplicateKey, &[("key", key)])
        }
        HooksEditError::ExternalModified => language
            .text(SettingsAgentsHooksExternalModified)
            .to_owned(),
        HooksEditError::LockBusy => language.text(SettingsAgentsHooksLockBusy).to_owned(),
        HooksEditError::BackupFailed(detail) => {
            language.format(SettingsAgentsHooksBackupFailed, &[("detail", detail)])
        }
        HooksEditError::Unauthorized => language.text(SettingsAgentsHooksUnauthorized).to_owned(),
        HooksEditError::Io(detail) => {
            language.format(SettingsAgentsHooksIoFailed, &[("detail", detail)])
        }
        HooksEditError::Serialize(detail) => {
            language.format(SettingsAgentsHooksSerializeFailed, &[("detail", detail)])
        }
    }
}

/// 封闭错误类别 → typed 文案；前台渲染只经此入口。
fn hooks_error_text(language: crate::i18n::UiLanguage, error: &HooksPageError) -> String {
    use Message::{
        SettingsAgentsHooksBaselineMissing, SettingsAgentsHooksConfigDirUnavailable,
        SettingsAgentsHooksDocumentNotLoaded, SettingsAgentsHooksProjectMissing,
        SettingsAgentsHooksRootUnavailable,
    };
    match error {
        HooksPageError::ProjectMissing => {
            language.text(SettingsAgentsHooksProjectMissing).to_owned()
        }
        HooksPageError::ConfigDirUnavailable => language
            .text(SettingsAgentsHooksConfigDirUnavailable)
            .to_owned(),
        HooksPageError::RootUnavailable => {
            language.text(SettingsAgentsHooksRootUnavailable).to_owned()
        }
        HooksPageError::DocumentNotLoaded => language
            .text(SettingsAgentsHooksDocumentNotLoaded)
            .to_owned(),
        HooksPageError::BaselineMissing => {
            language.text(SettingsAgentsHooksBaselineMissing).to_owned()
        }
        HooksPageError::Edit(error) => hooks_edit_error_text(language, error),
    }
}

/// Hooks 编辑器宿主状态：文档（带基线）、在途载入与失败诊断。
#[derive(Default)]
pub(super) struct HooksEditorState {
    documents: BTreeMap<HooksDraftKey, HooksDocument>,
    loading: BTreeSet<HooksDraftKey>,
    errors: BTreeMap<HooksDraftKey, HooksPageError>,
    /// 载入时真实目录不存在的项目根：Project／Local 层据此禁用。
    unavailable_roots: BTreeSet<ProjectId>,
    /// 最近一次成功保存的草稿键；用于「重启 CLI 生效」提示。
    saved_hint: Option<HooksDraftKey>,
    /// 后台复核发现磁盘已变化且草稿未保存的键。
    external_modified: BTreeSet<HooksDraftKey>,
    /// 在途外部修改复核，合并同一文件的重复事件。
    external_checks: BTreeSet<HooksDraftKey>,
}

fn layer_label(layer: HooksDraftLayer) -> Message {
    match layer {
        HooksDraftLayer::User => Message::SettingsAgentsScopeUser,
        HooksDraftLayer::Project(_) => Message::SettingsAgentsScopeProject,
        HooksDraftLayer::Local(_) => Message::SettingsAgentsScopeLocal,
    }
}

fn layer_slug(layer: HooksDraftLayer) -> &'static str {
    match layer {
        HooksDraftLayer::User => "user",
        HooksDraftLayer::Project(_) => "project",
        HooksDraftLayer::Local(_) => "local",
    }
}

fn diagnostic_text(language: crate::i18n::UiLanguage, error: &DraftParseError) -> String {
    match error {
        DraftParseError::InvalidUtf8 => language
            .text(Message::SettingsAgentsHooksInvalidUtf8)
            .to_owned(),
        DraftParseError::InvalidJson(detail) => language.format(
            Message::SettingsAgentsHooksInvalidJson,
            &[("detail", detail)],
        ),
        DraftParseError::DuplicateKey(key) => {
            language.format(Message::SettingsAgentsHooksDuplicateKey, &[("key", key)])
        }
    }
}

/// Hooks 页内容：层选择、JSON／GUI 模式、诊断与保存／丢弃。
pub(super) fn render_hooks_page(
    view: &mut SettingsView,
    window: &mut Window,
    cx: &mut Context<'_, SettingsView>,
) -> AnyElement {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let root_entity = view.root.upgrade();
    let key = view.hooks_key();
    let mut draft_text: Option<String> = None;
    let mut draft_diagnostic: Option<DraftParseError> = None;
    let mut schema_errors: Vec<HooksEditError> = Vec::new();
    let mut model: Option<serde_json::Value> = None;
    let mut model_source_available = false;
    let mut saveable = false;
    let mut dirty = false;
    let mut error: Option<HooksPageError> = None;
    let mut owned = 0usize;
    let mut generation = 0u64;
    let mut project_layers_available = true;
    let mut saved_just_now = false;
    if let (Some(root), Some(key)) = (root_entity.as_ref(), key) {
        let root = root.read(cx);
        generation = root.hooks_generation;
        if let Some(project) = view.hooks_project() {
            project_layers_available = !root.hooks_editor.unavailable_roots.contains(&project);
        }
        if let Some(draft) = root.hook_drafts.get(key) {
            draft_text = Some(draft.text().to_owned());
            draft_diagnostic = draft.diagnostic().cloned();
            schema_errors = draft.schema_errors().to_vec();
            model = Some(draft.model().clone());
            model_source_available = true;
            saveable = draft.saveable();
            dirty = draft.dirty();
        } else if let Some(document) = root.hooks_editor.documents.get(&key) {
            // 保存成功后草稿已清除；表单与 JSON 编辑器都按已重载的磁盘事实渲染。
            draft_text = Some(document.editable_json());
            let document_model = document.editable_value().clone();
            schema_errors = validate_editable_all(&document_model);
            model = Some(document_model);
            model_source_available = true;
        }
        error = root.hooks_editor.errors.get(&key).cloned();
        saved_just_now = root.hooks_editor.saved_hint == Some(key);
        owned = root
            .hooks_editor
            .documents
            .get(&key)
            .map(HooksDocument::owned_group_count)
            .unwrap_or(0);
    }

    let form_available =
        model_source_available && draft_diagnostic.is_none() && schema_errors.is_empty();
    if view.hooks_mode_value() == HooksEditMode::Form && !form_available {
        view.set_hooks_mode(HooksEditMode::Json, cx);
    }
    let mode = view.hooks_mode_value();

    // 草稿文本变化后（载入／丢弃／保存回执）在帧间把 JSON 输入同步到最新文本。
    if let (Some(key), Some(text)) = (key, draft_text.clone())
        && !view.hooks_sync_matches(key, generation)
    {
        cx.defer_in(window, move |view, window, cx| {
            view.apply_hooks_sync(key, text, generation, window, cx);
        });
    }

    let layers: Vec<HooksDraftLayer> = view.available_hooks_layers();
    let selected_layer = view.hooks_layer_value();
    let current_layer_enabled = selected_layer == HooksDraftLayer::User || project_layers_available;
    let layer_pills = layers
        .into_iter()
        .map(|layer| {
            let selected = layer == selected_layer;
            let enabled = layer == HooksDraftLayer::User || project_layers_available;
            let slug = layer_slug(layer);
            div()
                .id(SharedString::from(format!("settings-hooks-layer-{slug}")))
                .px_2()
                .h(px(22.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .text_xs()
                .border_color(if selected {
                    cx.theme().accent
                } else {
                    cx.theme().border
                })
                .text_color(if !enabled {
                    cx.theme().muted_foreground.opacity(0.4)
                } else if selected {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .debug_selector(move || format!("settings-hooks-layer-{slug}"))
                .when(enabled, |element| {
                    element.cursor_pointer().on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| view.set_hooks_layer(layer, cx)),
                    )
                })
                .child(language.text(layer_label(layer)))
        })
        .collect::<Vec<_>>();

    let mode_pills = [HooksEditMode::Json, HooksEditMode::Form]
        .into_iter()
        .map(|candidate| {
            let selected = candidate == mode;
            let enabled = candidate != HooksEditMode::Form || form_available;
            let slug = candidate.slug();
            div()
                .id(SharedString::from(format!("settings-hooks-mode-{slug}")))
                .px_2()
                .h(px(22.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .text_xs()
                .border_color(if selected {
                    cx.theme().accent
                } else {
                    cx.theme().border
                })
                .text_color(if !enabled {
                    cx.theme().muted_foreground.opacity(0.4)
                } else if selected {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .debug_selector(move || format!("settings-hooks-mode-{slug}"))
                .when(enabled, |element| {
                    element.cursor_pointer().on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| view.set_hooks_mode(candidate, cx)),
                    )
                })
                .child(language.text(candidate.message()))
        })
        .collect::<Vec<_>>();

    let mut diagnostics: Vec<String> = Vec::new();
    if let Some(diagnostic) = &draft_diagnostic {
        diagnostics.push(diagnostic_text(language, diagnostic));
    }
    diagnostics.extend(
        schema_errors
            .iter()
            .map(|error| hooks_edit_error_text(language, error)),
    );
    let diagnostics_count = diagnostics.len();
    let diagnostics_expanded = view.hooks_diagnostics_expanded();
    let first_diagnostic = diagnostics.first().cloned();
    let all_diagnostics = diagnostics
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .debug_selector(move || format!("settings-hooks-diagnostic-{index}"))
                .child(SharedString::from(text))
                .into_any_element()
        })
        .collect::<Vec<_>>();

    let input = view.hooks_input().cloned();
    let json_editor = (mode == HooksEditMode::Json)
        .then(|| {
            input.map(|input| {
                div()
                    .h(px(300.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .debug_selector(|| "settings-hooks-editor".to_owned())
                    .child(Input::new(&input))
                    .into_any_element()
            })
        })
        .flatten();
    let form_editor = (mode == HooksEditMode::Form).then(|| {
        form::render_hooks_form(view, model.as_ref(), key, generation, language, window, cx)
    });

    let save_root = view.root.clone();
    let discard_root = view.root.clone();
    let reload_root = view.root.clone();
    let save_key = key;
    let discard_key = key;
    let reload_key = key;

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::SettingsAgentsEditorDescription)),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .debug_selector(|| "settings-hooks-layers".to_owned())
                .children(layer_pills),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .debug_selector(|| "settings-hooks-modes".to_owned())
                .children(mode_pills),
        )
        .when(selected_layer == HooksDraftLayer::User, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "settings-hooks-user-warning".to_owned())
                    .child(language.text(Message::SettingsAgentsHooksUserWarning)),
            )
        })
        .when(!form_available && mode == HooksEditMode::Json, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .debug_selector(|| "settings-hooks-form-disabled".to_owned())
                    .child(language.text(Message::SettingsAgentsHooksFormDisabled)),
            )
        })
        .when(saved_just_now, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .debug_selector(|| "settings-hooks-saved-hint".to_owned())
                    .child(language.text(Message::SettingsAgentsHooksSavedRestart)),
            )
        })
        .when(
            view.hooks_project().is_some() && !project_layers_available,
            |element| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .debug_selector(|| "settings-hooks-root-unavailable".to_owned())
                        .child(language.text(Message::SettingsAgentsHooksRootUnavailable)),
                )
            },
        )
        .when(owned > 0, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .debug_selector(|| "settings-hooks-owned".to_owned())
                    .child(language.format(
                        Message::SettingsAgentsEditorOwned,
                        &[("count", &owned.to_string())],
                    )),
            )
        })
        .when_some(error, |element, error| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "settings-hooks-error".to_owned())
                    .child(hooks_error_text(language, &error)),
            )
        })
        .children(json_editor)
        .children(form_editor)
        .when_some(first_diagnostic, |element, first| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .debug_selector(|| "settings-hooks-diagnostics".to_owned())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .debug_selector(|| "settings-hooks-diagnostic".to_owned())
                            .child(SharedString::from(first)),
                    )
                    .when(diagnostics_count > 1, |element| {
                        element.child(
                            div()
                                .id("settings-hooks-diagnostics-toggle")
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().accent)
                                .debug_selector(|| "settings-hooks-diagnostics-toggle".to_owned())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _, _, cx| view.toggle_hooks_diagnostics(cx)),
                                )
                                .child(if diagnostics_expanded {
                                    language
                                        .text(Message::SettingsAgentsHooksDiagnosticsHide)
                                        .to_owned()
                                } else {
                                    language.format(
                                        Message::SettingsAgentsHooksDiagnosticsShowAll,
                                        &[("count", &diagnostics_count.to_string())],
                                    )
                                }),
                        )
                    })
                    .when(diagnostics_expanded, |element| {
                        element.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .debug_selector(|| "settings-hooks-diagnostics-list".to_owned())
                                .children(all_diagnostics),
                        )
                    }),
            )
        })
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .id("settings-hooks-save")
                        .px_2()
                        .h(px(24.0))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(if saveable && current_layer_enabled {
                            cx.theme().accent
                        } else {
                            cx.theme().border
                        })
                        .text_xs()
                        .cursor_pointer()
                        .debug_selector(|| "settings-hooks-save".to_owned())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_view, _, _, cx| {
                                let (Some(root), Some(key)) = (save_root.upgrade(), save_key)
                                else {
                                    return;
                                };
                                root.update(cx, |root, cx| root.save_hooks_draft_for(key, cx));
                            }),
                        )
                        .child(language.text(Message::SettingsAgentsHooksSave)),
                )
                .child(
                    div()
                        .id("settings-hooks-discard")
                        .px_2()
                        .h(px(24.0))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(if dirty && current_layer_enabled {
                            cx.theme().border
                        } else {
                            cx.theme().border.opacity(0.5)
                        })
                        .text_xs()
                        .cursor_pointer()
                        .debug_selector(|| "settings-hooks-discard".to_owned())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_view, _, _, cx| {
                                let (Some(root), Some(key)) = (discard_root.upgrade(), discard_key)
                                else {
                                    return;
                                };
                                root.update(cx, |root, cx| root.discard_hooks_draft_for(key, cx));
                            }),
                        )
                        .child(language.text(Message::SettingsAgentsHooksDiscard)),
                )
                .child(
                    div()
                        .id("settings-hooks-reload")
                        .px_2()
                        .h(px(24.0))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_xs()
                        .cursor_pointer()
                        .debug_selector(|| "settings-hooks-reload".to_owned())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_view, _, _, cx| {
                                let (Some(root), Some(key)) = (reload_root.upgrade(), reload_key)
                                else {
                                    return;
                                };
                                root.update(cx, |root, cx| root.reload_hooks_draft_for(key, cx));
                            }),
                        )
                        .child(language.text(Message::SettingsAgentsHooksReload)),
                ),
        )
        .into_any_element()
}

impl SettingsView {
    /// 当前 Hooks 草稿键：面板绑定后固定 Claude 适配。
    pub(super) fn hooks_key(&self) -> Option<HooksDraftKey> {
        let owner = match self.panel_id_value() {
            Some(panel) => HooksDraftOwner::Panel(panel),
            None if self.is_overlay() => HooksDraftOwner::Overlay,
            None => return None,
        };
        Some(HooksDraftKey {
            owner,
            agent: AgentKind::Claude,
            layer: self.hooks_layer_value(),
        })
    }

    /// 可选择的层：项目面板恒提供三层，应用级浮层只有 User。
    /// 可选集合只看视图身份，不随当前层收缩，保证可往返切换。
    pub(super) fn available_hooks_layers(&self) -> Vec<HooksDraftLayer> {
        match self.hooks_project() {
            Some(project) => vec![
                HooksDraftLayer::User,
                HooksDraftLayer::Project(project),
                HooksDraftLayer::Local(project),
            ],
            None => vec![HooksDraftLayer::User],
        }
    }

    /// 切层：保留 JSON 输入实体，等下一次同步把文本换成目标层草稿。
    /// 表单输入与待新增 handler 属于旧层上下文，必须一并释放。
    pub(super) fn set_hooks_layer(&mut self, layer: HooksDraftLayer, cx: &mut Context<'_, Self>) {
        if self.hooks_layer_value() == layer {
            return;
        }
        self.set_hooks_layer_value(layer);
        self.reset_hooks_sync();
        self.hooks_form_mut().reset();
        self.reset_hooks_diagnostics();
        self.request_hooks_draft(cx);
        cx.notify();
    }

    /// 请求载入当前键草稿；已载入或载入中时为空操作。
    pub(super) fn request_hooks_draft(&mut self, cx: &mut Context<'_, Self>) {
        let Some(key) = self.hooks_key() else {
            return;
        };
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| root.request_hooks_draft(key, cx));
        }
    }
}

impl AppRoot {
    /// 面板绑定：设置面板拿到稳定 PanelId 后才能构造 Hooks 草稿键。
    pub(super) fn bind_settings_panel(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        project_id: slterm_workspace::ProjectId,
        cx: &mut Context<'_, Self>,
    ) {
        let view = self.projects.get(&project_id).and_then(|project| {
            project
                .read(cx)
                .panels
                .get(&panel_id)
                .and_then(|panel| match &panel.content {
                    super::panel_host::PanelContent::Settings(view) => Some(view.clone()),
                    _ => None,
                })
        });
        let Some(view) = view else {
            return;
        };
        view.update(cx, |view, cx| view.bind_panel_id(panel_id, cx));
        if view.read(cx).current_page() == crate::settings_state::SettingsPageId::Hooks {
            let layer = view.read(cx).hooks_layer_value();
            self.request_hooks_draft(
                HooksDraftKey {
                    owner: HooksDraftOwner::Panel(panel_id),
                    agent: AgentKind::Claude,
                    layer,
                },
                cx,
            );
        }
    }

    fn hooks_claude_dir(&self) -> Result<PathBuf, HooksPageError> {
        #[cfg(test)]
        if let Some(dir) = &self.hooks_claude_dir_for_test {
            return Ok(dir.clone());
        }
        crate::ai_hook::claude_config_dir().ok_or(HooksPageError::ConfigDirUnavailable)
    }

    /// 层写入根：Project／Local 为项目根，User 为 Claude 配置目录；
    /// 目录真实存在是读写前置条件。
    fn hooks_layer_root(&self, layer: HooksDraftLayer) -> Result<PathBuf, HooksPageError> {
        match layer {
            HooksDraftLayer::User => self.hooks_claude_dir(),
            HooksDraftLayer::Project(project) | HooksDraftLayer::Local(project) => self
                .snapshot
                .registry()
                .project(project)
                .map(|project| project.root().as_path().to_path_buf())
                .ok_or(HooksPageError::ProjectMissing),
        }
    }

    /// 记录层根不可用：Project／Local pill 据此禁用，成功载入后恢复。
    fn note_hooks_layer_failure(&mut self, key: HooksDraftKey, error: &HooksPageError) {
        if let Some(project) = key.layer.project()
            && matches!(error, HooksPageError::RootUnavailable)
        {
            self.hooks_editor.unavailable_roots.insert(project);
        }
    }

    fn hooks_layer_path(&self, layer: HooksDraftLayer) -> Result<PathBuf, HooksPageError> {
        match layer {
            HooksDraftLayer::User => Ok(self.hooks_claude_dir()?.join("settings.json")),
            HooksDraftLayer::Project(project) => {
                let project = self
                    .snapshot
                    .registry()
                    .project(project)
                    .ok_or(HooksPageError::ProjectMissing)?;
                Ok(project.root().as_path().join(".claude/settings.json"))
            }
            HooksDraftLayer::Local(project) => {
                let project = self
                    .snapshot
                    .registry()
                    .project(project)
                    .ok_or(HooksPageError::ProjectMissing)?;
                Ok(project.root().as_path().join(".claude/settings.local.json"))
            }
        }
    }

    /// 已载入 Hooks 层的真实文件路径；watcher 只登记存在的文件。
    pub(super) fn hooks_watch_files(&self) -> Vec<PathBuf> {
        self.hooks_editor
            .documents
            .keys()
            .filter_map(|key| self.hooks_layer_path(key.layer).ok())
            .filter(|path| path.is_file())
            .collect()
    }

    /// 外部事件只在 Hooks 页可见且所属项目活跃时处理；隐藏期间保留登记。
    pub(super) fn hooks_key_visible(&self, key: HooksDraftKey, cx: &App) -> bool {
        match key.owner {
            HooksDraftOwner::Overlay => self.settings_overlay.as_ref().is_some_and(|host| {
                host.view.read(cx).current_page() == crate::settings_state::SettingsPageId::Hooks
            }),
            HooksDraftOwner::Panel(panel) => {
                let active = self.snapshot.registry().active();
                self.projects.iter().any(|(project_id, project)| {
                    Some(*project_id) == active
                        && project.read(cx).panels.get(&panel).is_some_and(|host| {
                            match &host.content {
                                super::panel_host::PanelContent::Settings(view) => {
                                    view.read(cx).current_page()
                                        == crate::settings_state::SettingsPageId::Hooks
                                }
                                _ => false,
                            }
                        })
                })
            }
        }
    }

    pub(super) fn hooks_keys_for_path(&self, path: &Path) -> Vec<HooksDraftKey> {
        self.hooks_editor
            .documents
            .keys()
            .filter(|key| {
                self.hooks_layer_path(key.layer)
                    .is_ok_and(|layer| layer == path)
            })
            .copied()
            .collect()
    }

    /// watcher 命中：后台重读真实文件，只有基线变化才标记外部修改。
    pub(super) fn check_hooks_external(&mut self, key: HooksDraftKey, cx: &mut Context<'_, Self>) {
        let Some(document) = self.hooks_editor.documents.get(&key).cloned() else {
            return;
        };
        if !self.hooks_editor.external_checks.insert(key) {
            return;
        }
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let reloaded = cx
                .background_executor()
                .spawn(async move { document.reload() })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::HooksDraftExternalCheck(key));
                root.hooks_editor.external_checks.remove(&key);
                if !root.hooks_draft_owner_alive(key.owner, cx) {
                    return;
                }
                let current = root
                    .hooks_editor
                    .documents
                    .get(&key)
                    .and_then(HooksDocument::baseline);
                let changed = match &reloaded {
                    Ok(reloaded) => reloaded.baseline() != current,
                    Err(_) => true,
                };
                if changed {
                    root.hooks_editor.external_modified.insert(key);
                    root.hooks_editor
                        .errors
                        .insert(key, HooksPageError::Edit(HooksEditError::ExternalModified));
                } else {
                    root.hooks_editor.external_modified.remove(&key);
                    if matches!(
                        root.hooks_editor.errors.get(&key),
                        Some(HooksPageError::Edit(HooksEditError::ExternalModified))
                    ) {
                        root.hooks_editor.errors.remove(&key);
                    }
                }
                cx.notify();
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::HooksDraftExternalCheck(key), cancellation, task)
        {
            self.hooks_editor.external_checks.remove(&key);
            log::warn!("Hooks 外部修改复核注册失败: {error:?}");
        }
    }

    /// 非破坏性重新载入：只有干净草稿才按磁盘重建，脏草稿保留。
    pub(super) fn reload_hooks_draft_for(
        &mut self,
        key: HooksDraftKey,
        cx: &mut Context<'_, Self>,
    ) {
        if self.hook_drafts.get(key).is_some_and(|draft| draft.dirty()) {
            return;
        }
        self.hooks_editor.external_modified.remove(&key);
        self.forget_hooks_draft_keys(&[key]);
        self.hook_drafts.discard(key);
        self.request_hooks_draft(key, cx);
        cx.notify();
    }

    /// 安装/卸载完成后：User 层已载入文档重读；脏草稿只标记外部修改。
    pub(super) fn refresh_user_hooks_after_install(&mut self, cx: &mut Context<'_, Self>) {
        let keys = self
            .hooks_editor
            .documents
            .keys()
            .filter(|key| key.layer == HooksDraftLayer::User)
            .copied()
            .collect::<Vec<_>>();
        for key in keys {
            if self.hook_drafts.get(key).is_some_and(|draft| draft.dirty()) {
                self.hooks_editor.external_modified.insert(key);
                self.hooks_editor
                    .errors
                    .insert(key, HooksPageError::Edit(HooksEditError::ExternalModified));
            } else {
                self.reload_hooks_draft_for(key, cx);
            }
        }
        cx.notify();
    }

    /// 后台载入层文件；已载入或载入中为空操作。
    pub(super) fn request_hooks_draft(&mut self, key: HooksDraftKey, cx: &mut Context<'_, Self>) {
        if self.hook_drafts.get(key).is_some() || self.hooks_editor.loading.contains(&key) {
            return;
        }
        let path = match self.hooks_layer_path(key.layer) {
            Ok(path) => path,
            Err(message) => {
                self.hooks_editor.errors.insert(key, message);
                cx.notify();
                return;
            }
        };
        let guard_root = match self.hooks_layer_root(key.layer) {
            Ok(root) => root,
            Err(message) => {
                self.hooks_editor.errors.insert(key, message);
                cx.notify();
                return;
            }
        };
        let user_layer = key.layer == HooksDraftLayer::User;
        self.hooks_editor.loading.insert(key);
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    // 目录真实存在才允许读写：缺失根下不得重建配置。
                    if !guard_root.is_dir() {
                        return Err(if user_layer {
                            HooksPageError::Edit(HooksEditError::ConfigDirMissing)
                        } else {
                            HooksPageError::RootUnavailable
                        });
                    }
                    HooksDocument::load_at(&path).map_err(HooksPageError::Edit)
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.hooks_editor.loading.remove(&key);
                root.tasks.complete(TaskId::HooksDraftLoad(key));
                if !root.hooks_draft_owner_alive(key.owner, cx) {
                    return;
                }
                match loaded {
                    Ok(document) => {
                        let text = document.editable_json();
                        let model = document.editable_value().clone();
                        let baseline = document.baseline();
                        root.hooks_editor.documents.insert(key, document);
                        root.hooks_editor.errors.remove(&key);
                        root.hooks_editor.external_modified.remove(&key);
                        if root.hook_drafts.get(key).is_none() {
                            root.hook_drafts
                                .insert(key, HooksDraft::loaded(baseline, text, model));
                        }
                        root.hooks_generation = root.hooks_generation.wrapping_add(1);
                        root.reconcile_fs_watch();
                    }
                    Err(error) => {
                        root.note_hooks_layer_failure(key, &error);
                        root.hooks_editor.errors.insert(key, error);
                    }
                }
                cx.notify();
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::HooksDraftLoad(key), cancellation, task)
        {
            self.hooks_editor.loading.remove(&key);
            log::warn!("Hooks 载入任务注册失败: {error:?}");
        }
    }

    /// 编辑文本只更新内存草稿；诊断由草稿解析给出。
    pub(super) fn update_hooks_draft_text(
        &mut self,
        key: HooksDraftKey,
        text: &str,
        cx: &mut Context<'_, Self>,
    ) {
        if self.hooks_editor.saved_hint == Some(key) {
            self.hooks_editor.saved_hint = None;
        }
        if self.hook_drafts.get(key).is_none()
            && let Some(document) = self.hooks_editor.documents.get(&key)
        {
            // 保存成功后草稿已清除；继续编辑时从磁盘基线重建草稿。
            self.hook_drafts.insert(
                key,
                HooksDraft::loaded(
                    document.baseline(),
                    document.editable_json(),
                    document.editable_value().clone(),
                ),
            );
        }
        if let Some(draft) = self.hook_drafts.get_mut(key) {
            draft.set_text(text);
            cx.notify();
        }
    }

    /// 后台保存：写前重读守卫在 `HooksDocument` 内；成功后重载远端事实。
    pub(super) fn save_hooks_draft_for(&mut self, key: HooksDraftKey, cx: &mut Context<'_, Self>) {
        let Some(draft) = self.hook_drafts.get(key) else {
            return;
        };
        if !draft.saveable() {
            return;
        }
        let Some(document) = self.hooks_editor.documents.get(&key).cloned() else {
            self.hooks_editor
                .errors
                .insert(key, HooksPageError::DocumentNotLoaded);
            cx.notify();
            return;
        };
        let submitted = draft.text().to_owned();
        let guard_root = match self.hooks_layer_root(key.layer) {
            Ok(root) => root,
            Err(message) => {
                self.hooks_editor.errors.insert(key, message);
                cx.notify();
                return;
            }
        };
        let user_layer = key.layer == HooksDraftLayer::User;
        let text = submitted.clone();
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let document = document.clone();
                    let text = text.clone();
                    async move {
                        // 保存前复核目录仍然存在：根被删除后不得重建配置。
                        if !guard_root.is_dir() {
                            return Err(if user_layer {
                                HooksPageError::Edit(HooksEditError::ConfigDirMissing)
                            } else {
                                HooksPageError::RootUnavailable
                            });
                        }
                        document
                            .save_editable(&text, &guard_root)
                            .map_err(HooksPageError::Edit)?;
                        document.reload().map_err(HooksPageError::Edit)
                    }
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::HooksDraftSave(key));
                if !root.hooks_draft_owner_alive(key.owner, cx) {
                    return;
                }
                match result {
                    Ok(reloaded) => match reloaded.baseline() {
                        Some(baseline) => {
                            let saved_text = reloaded.editable_json();
                            let saved_model = reloaded.editable_value().clone();
                            root.hooks_editor.documents.insert(key, reloaded);
                            let applied = root.hook_drafts.get_mut(key).is_some_and(|draft| {
                                draft.finish_save(&submitted, baseline, saved_text, saved_model)
                            });
                            root.hooks_editor.errors.remove(&key);
                            root.hooks_editor.external_modified.remove(&key);
                            if applied {
                                // 保存成功即清除草稿；继续编辑会从磁盘基线重建。
                                root.hook_drafts.discard(key);
                                root.hooks_editor.saved_hint = Some(key);
                                root.hooks_generation = root.hooks_generation.wrapping_add(1);
                            }
                        }
                        None => {
                            root.hooks_editor
                                .errors
                                .insert(key, HooksPageError::BaselineMissing);
                        }
                    },
                    Err(error) => {
                        root.note_hooks_layer_failure(key, &error);
                        root.hooks_editor.errors.insert(key, error);
                    }
                }
                cx.notify();
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::HooksDraftSave(key), cancellation, task)
        {
            log::warn!("Hooks 保存任务注册失败: {error:?}");
        }
    }

    /// 清理草稿键对应的文档与诊断；调用方负责移除草稿本身。
    pub(super) fn forget_hooks_draft_keys(&mut self, keys: &[HooksDraftKey]) {
        for key in keys {
            // 面板或所有者销毁后，在途载入／保存不得再改动其草稿或文档。
            self.tasks.cancel(TaskId::HooksDraftLoad(*key));
            self.tasks.cancel(TaskId::HooksDraftSave(*key));
            self.tasks.cancel(TaskId::HooksDraftExternalCheck(*key));
            self.hooks_editor.documents.remove(key);
            self.hooks_editor.errors.remove(key);
            self.hooks_editor.external_modified.remove(key);
            self.hooks_editor.external_checks.remove(key);
        }
        if !keys.is_empty() {
            self.hooks_generation = self.hooks_generation.wrapping_add(1);
            self.reconcile_fs_watch();
        }
    }

    /// 面板或浮层销毁后，迟到回执不得复活草稿。
    fn hooks_draft_owner_alive(&self, owner: HooksDraftOwner, cx: &App) -> bool {
        match owner {
            HooksDraftOwner::Overlay => self.settings_overlay.is_some(),
            HooksDraftOwner::Panel(panel) => self
                .projects
                .values()
                .any(|project| project.read(cx).panels.contains_key(&panel)),
        }
    }

    /// 所有者销毁：释放其全部层级草稿与文档，避免孤儿键。
    pub(super) fn discard_hooks_owner_for_close(
        &mut self,
        owner: HooksDraftOwner,
        cx: &mut Context<'_, Self>,
    ) {
        let keys = self.hook_drafts.clear_owner(owner);
        self.forget_hooks_draft_keys(&keys);
        if !keys.is_empty() {
            cx.notify();
        }
    }

    /// 面板销毁：释放该面板全部层级草稿与文档，避免孤儿键指向失效 PanelId。
    pub(super) fn discard_panel_hooks_for_close(
        &mut self,
        panel: slterm_workspace::PanelId,
        cx: &mut Context<'_, Self>,
    ) {
        self.discard_hooks_owner_for_close(HooksDraftOwner::Panel(panel), cx);
    }

    /// 丢弃：移除草稿与文档后重新载入磁盘现状。
    /// 非交互项目移除前丢弃该项目草稿：只清内存，不再发起重载。
    pub(super) fn discard_project_hooks_drafts_for_close(
        &mut self,
        project: ProjectId,
        cx: &mut Context<'_, Self>,
    ) {
        let keys = self.hook_drafts.clear_project(project);
        self.forget_hooks_draft_keys(&keys);
        cx.notify();
    }

    /// 非交互应用关闭前丢弃全部草稿：只清内存，不再发起重载。
    pub(super) fn discard_all_hooks_drafts_for_close(&mut self, cx: &mut Context<'_, Self>) {
        for key in self.hook_drafts.keys() {
            self.hook_drafts.discard(key);
        }
        self.hooks_editor.documents.clear();
        self.hooks_editor.errors.clear();
        self.hooks_editor.external_modified.clear();
        self.hooks_editor.external_checks.clear();
        self.hooks_generation = self.hooks_generation.wrapping_add(1);
        self.reconcile_fs_watch();
        cx.notify();
    }

    pub(super) fn discard_hooks_draft_for(
        &mut self,
        key: HooksDraftKey,
        cx: &mut Context<'_, Self>,
    ) {
        self.hook_drafts.discard(key);
        self.hooks_editor.documents.remove(&key);
        self.hooks_editor.errors.remove(&key);
        self.hooks_editor.external_modified.remove(&key);
        self.hooks_editor.external_checks.remove(&key);
        if self.hooks_editor.saved_hint == Some(key) {
            self.hooks_editor.saved_hint = None;
        }
        self.hooks_generation = self.hooks_generation.wrapping_add(1);
        self.reconcile_fs_watch();
        self.request_hooks_draft(key, cx);
        cx.notify();
    }

    /// 测试观测：该草稿键是否已被 watcher 标记为外部修改。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn hooks_external_modified_for_test(&self, key: HooksDraftKey) -> bool {
        self.hooks_editor.external_modified.contains(&key)
    }

    /// 测试观测：该面板是否仍持有 Hooks 文档或诊断。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn hooks_state_has_panel_entries(&self, panel: slterm_workspace::PanelId) -> bool {
        self.hooks_editor
            .documents
            .keys()
            .chain(self.hooks_editor.errors.keys())
            .any(|key| key.panel() == Some(panel))
    }

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn replace_hooks_claude_dir_for_test(&mut self, dir: PathBuf) {
        self.hooks_claude_dir_for_test = Some(dir);
    }
}
