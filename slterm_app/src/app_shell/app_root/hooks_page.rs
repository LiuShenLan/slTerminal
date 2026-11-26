//! 设置页 Hooks 编辑：三层草稿的载入／保存／丢弃与只读认领组摘要。
//!
//! 文件 I/O 只在后台执行体运行；前台持有草稿文本与诊断，保存前由
//! `HooksDocument` 在锁内重读并比较基线指纹，外部修改零写入。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::Input;
use slterm_agents::AgentKind;
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::settings_overlay::SettingsView;
use super::task_service::{CancellationToken, TaskId};
use crate::ai_hook::draft::DraftParseError;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer};
use crate::ai_hook::editor::{HooksDocument, HooksEditError};
use crate::i18n::Message;

/// Hooks 页失败类别：页面上下文失败或编辑域封闭失败。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HooksPageError {
    ProjectMissing,
    ConfigDirUnavailable,
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
    };
    match error {
        HooksPageError::ProjectMissing => {
            language.text(SettingsAgentsHooksProjectMissing).to_owned()
        }
        HooksPageError::ConfigDirUnavailable => language
            .text(SettingsAgentsHooksConfigDirUnavailable)
            .to_owned(),
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

/// Hooks 页内容：层选择、JSON 编辑区、诊断与保存／丢弃。
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
    let mut saveable = false;
    let mut dirty = false;
    let mut error: Option<HooksPageError> = None;
    let mut owned = 0usize;
    let mut generation = 0u64;
    if let (Some(root), Some(key)) = (root_entity.as_ref(), key) {
        let root = root.read(cx);
        generation = root.hooks_generation;
        if let Some(draft) = root.hook_drafts.get(key) {
            draft_text = Some(draft.text().to_owned());
            draft_diagnostic = draft.diagnostic().cloned();
            saveable = draft.saveable();
            dirty = draft.dirty();
        }
        error = root.hooks_editor.errors.get(&key).cloned();
        owned = root
            .hooks_editor
            .documents
            .get(&key)
            .map(HooksDocument::owned_group_count)
            .unwrap_or(0);
    }

    // 草稿文本变化后（载入／丢弃／保存回执）在帧间把输入框同步到最新文本。
    if let (Some(key), Some(text)) = (key, draft_text.clone())
        && !view.hooks_sync_matches(key, generation)
    {
        cx.defer_in(window, move |view, window, cx| {
            view.apply_hooks_sync(key, text, generation, window, cx);
        });
    }

    let layers: Vec<HooksDraftLayer> = view.available_hooks_layers();
    let selected_layer = view.hooks_layer_value();
    let layer_pills = layers.into_iter().map(|layer| {
        let selected = layer == selected_layer;
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
            .cursor_pointer()
            .border_color(if selected {
                cx.theme().accent
            } else {
                cx.theme().border
            })
            .text_color(if selected {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            })
            .debug_selector(move || format!("settings-hooks-layer-{slug}"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| view.set_hooks_layer(layer, cx)),
            )
            .child(language.text(layer_label(layer)))
    });

    let Some(input) = view.hooks_input() else {
        return div().into_any_element();
    };
    let input = input.clone();
    let save_root = view.root.clone();
    let discard_root = view.root.clone();
    let save_key = key;
    let discard_key = key;

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
        .when(owned > 0, |element| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
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
        .child(
            div()
                .h(px(300.0))
                .border_1()
                .border_color(cx.theme().border)
                .debug_selector(|| "settings-hooks-editor".to_owned())
                .child(Input::new(&input)),
        )
        .when_some(draft_diagnostic, |element, error| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "settings-hooks-diagnostic".to_owned())
                    .child(diagnostic_text(language, &error)),
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
                        .border_color(if saveable {
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
                        .border_color(if dirty {
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
                ),
        )
        .into_any_element()
}

impl SettingsView {
    /// 当前 Hooks 草稿键：面板绑定后固定 Claude 适配。
    pub(super) fn hooks_key(&self) -> Option<HooksDraftKey> {
        Some(HooksDraftKey {
            panel: self.panel_id_value()?,
            agent: AgentKind::Claude,
            layer: self.hooks_layer_value(),
        })
    }

    /// 可选择的层：项目面板提供三层，应用级浮层只有 User。
    pub(super) fn available_hooks_layers(&self) -> Vec<HooksDraftLayer> {
        match self.hooks_layer_value() {
            HooksDraftLayer::User => vec![HooksDraftLayer::User],
            HooksDraftLayer::Project(project) => vec![
                HooksDraftLayer::User,
                HooksDraftLayer::Project(project),
                HooksDraftLayer::Local(project),
            ],
            HooksDraftLayer::Local(project) => vec![
                HooksDraftLayer::User,
                HooksDraftLayer::Project(project),
                HooksDraftLayer::Local(project),
            ],
        }
    }

    /// 切层：保留输入实体，等下一次同步把文本换成目标层草稿。
    pub(super) fn set_hooks_layer(&mut self, layer: HooksDraftLayer, cx: &mut Context<'_, Self>) {
        if self.hooks_layer_value() == layer {
            return;
        }
        self.set_hooks_layer_value(layer);
        self.reset_hooks_sync();
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
                    panel: panel_id,
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
        self.hooks_editor.loading.insert(key);
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move { HooksDocument::load_at(&path) })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.hooks_editor.loading.remove(&key);
                root.tasks.complete(TaskId::HooksDraftLoad(key));
                match loaded {
                    Ok(document) => {
                        let text = document.editable_json();
                        let model = document.editable_value().clone();
                        let baseline = document.baseline();
                        root.hooks_editor.documents.insert(key, document);
                        root.hooks_editor.errors.remove(&key);
                        if root.hook_drafts.get(key).is_none() {
                            root.hook_drafts
                                .insert(key, HooksDraft::loaded(baseline, text, model));
                        }
                        root.hooks_generation = root.hooks_generation.wrapping_add(1);
                    }
                    Err(message) => {
                        root.hooks_editor
                            .errors
                            .insert(key, HooksPageError::Edit(message));
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
        let text = submitted.clone();
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let document = document.clone();
                    let text = text.clone();
                    async move {
                        document.save_editable(&text)?;
                        document.reload()
                    }
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::HooksDraftSave(key));
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
                            if applied {
                                root.hooks_generation = root.hooks_generation.wrapping_add(1);
                            }
                        }
                        None => {
                            root.hooks_editor
                                .errors
                                .insert(key, HooksPageError::BaselineMissing);
                        }
                    },
                    Err(message) => {
                        root.hooks_editor
                            .errors
                            .insert(key, HooksPageError::Edit(message));
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

    /// 丢弃：移除草稿与文档后重新载入磁盘现状。
    /// 非交互项目移除前丢弃该项目草稿：只清内存，不再发起重载。
    pub(super) fn discard_project_hooks_drafts_for_close(
        &mut self,
        project: ProjectId,
        cx: &mut Context<'_, Self>,
    ) {
        for key in self.hook_drafts.clear_project(project) {
            self.hooks_editor.documents.remove(&key);
            self.hooks_editor.errors.remove(&key);
        }
        self.hooks_generation = self.hooks_generation.wrapping_add(1);
        cx.notify();
    }

    /// 非交互应用关闭前丢弃全部草稿：只清内存，不再发起重载。
    pub(super) fn discard_all_hooks_drafts_for_close(&mut self, cx: &mut Context<'_, Self>) {
        for key in self.hook_drafts.keys() {
            self.hook_drafts.discard(key);
        }
        self.hooks_editor.documents.clear();
        self.hooks_editor.errors.clear();
        self.hooks_generation = self.hooks_generation.wrapping_add(1);
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
        self.hooks_generation = self.hooks_generation.wrapping_add(1);
        self.request_hooks_draft(key, cx);
        cx.notify();
    }

    #[cfg(test)]
    #[allow(dead_code)] // 消费点 = gpui 功能测试；非 UI 测试构建不引用注入端口。
    pub(crate) fn replace_hooks_claude_dir_for_test(&mut self, dir: PathBuf) {
        self.hooks_claude_dir_for_test = Some(dir);
    }
}
