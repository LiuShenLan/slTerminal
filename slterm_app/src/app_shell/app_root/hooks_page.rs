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

use super::AppRoot;
use super::settings_overlay::SettingsView;
use super::task_service::{CancellationToken, TaskId};
use crate::ai_hook::draft::DraftParseError;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer};
use crate::ai_hook::editor::HooksDocument;
use crate::i18n::Message;

/// Hooks 编辑器宿主状态：文档（带基线）、在途载入与失败诊断。
#[derive(Default)]
pub(super) struct HooksEditorState {
    documents: BTreeMap<HooksDraftKey, HooksDocument>,
    loading: BTreeSet<HooksDraftKey>,
    errors: BTreeMap<HooksDraftKey, String>,
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
    let mut error: Option<String> = None;
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
        .when_some(error, |element, message| {
            element.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "settings-hooks-error".to_owned())
                    .child(message),
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

    fn hooks_claude_dir(&self) -> Result<PathBuf, String> {
        #[cfg(test)]
        if let Some(dir) = &self.hooks_claude_dir_for_test {
            return Ok(dir.clone());
        }
        crate::ai_hook::claude_config_dir().ok_or_else(|| "无法解析 Claude 配置目录".to_owned())
    }

    fn hooks_layer_path(&self, layer: HooksDraftLayer) -> Result<PathBuf, String> {
        match layer {
            HooksDraftLayer::User => Ok(self.hooks_claude_dir()?.join("settings.json")),
            HooksDraftLayer::Project(project) => {
                let project = self
                    .snapshot
                    .registry()
                    .project(project)
                    .ok_or_else(|| "项目不存在".to_owned())?;
                Ok(project.root().as_path().join(".claude/settings.json"))
            }
            HooksDraftLayer::Local(project) => {
                let project = self
                    .snapshot
                    .registry()
                    .project(project)
                    .ok_or_else(|| "项目不存在".to_owned())?;
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
                root.tasks.complete(TaskId::HooksDraftLoad(key.panel));
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
                        root.hooks_editor.errors.insert(key, message);
                    }
                }
                cx.notify();
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::HooksDraftLoad(key.panel), cancellation, task)
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
                .insert(key, "Hooks 文档尚未加载".to_owned());
            cx.notify();
            return;
        };
        let text = draft.text().to_owned();
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
                root.tasks.complete(TaskId::HooksDraftSave(key.panel));
                match result {
                    Ok(reloaded) => match reloaded.baseline() {
                        Some(baseline) => {
                            let text = reloaded.editable_json();
                            let model = reloaded.editable_value().clone();
                            root.hooks_editor.documents.insert(key, reloaded);
                            if let Some(draft) = root.hook_drafts.get_mut(key) {
                                draft.mark_saved(baseline, text, model);
                            }
                            root.hooks_editor.errors.remove(&key);
                            root.hooks_generation = root.hooks_generation.wrapping_add(1);
                        }
                        None => {
                            root.hooks_editor
                                .errors
                                .insert(key, "保存后无法读取文件基线".to_owned());
                        }
                    },
                    Err(message) => {
                        root.hooks_editor.errors.insert(key, message);
                    }
                }
                cx.notify();
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::HooksDraftSave(key.panel), cancellation, task)
        {
            log::warn!("Hooks 保存任务注册失败: {error:?}");
        }
    }

    /// 丢弃：移除草稿与文档后重新载入磁盘现状。
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
    pub(crate) fn replace_hooks_claude_dir_for_test(&mut self, dir: PathBuf) {
        self.hooks_claude_dir_for_test = Some(dir);
    }
}
