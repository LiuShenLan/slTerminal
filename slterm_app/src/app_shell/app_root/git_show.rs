//! GitShow 固定版本面板与后台对象读取接线。

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window, div,
};
use gpui_component::{ActiveTheme as _, button::Button};
use slterm_document::{
    DocumentId, DocumentPageCache, DocumentTextRequest, LanguageId, LanguageRegistry, TextStateId,
};

use super::panel_host::PanelContent;
use crate::app_shell::config::Settings;
use crate::app_shell::paged_document::PageReader;
use crate::app_shell::text_view::{DocumentTextSnapshot, DocumentTextView, PagedTextView};
use crate::git::{GitShowContent, GitShowOpenMaterial, GitShowSnapshot};
use crate::i18n::Message;

pub(crate) enum GitShowEvent {
    OpenInSystem,
}

impl EventEmitter<GitShowEvent> for GitShowView {}

enum GitShowBody {
    Loading,
    Text(Entity<DocumentTextView>),
    Paged(Entity<PagedTextView>),
    Binary,
    InvalidUtf8,
    Error,
}

/// 固定对象只拥有只读展示；没有 Registry、保存或 Editor 持久租约。
pub(crate) struct GitShowView {
    focus: FocusHandle,
    body: GitShowBody,
    snapshot: Option<GitShowSnapshot>,
    cache: Rc<RefCell<DocumentPageCache>>,
    document_id: Option<DocumentId>,
    open_material: Option<GitShowOpenMaterial>,
    subscriptions: Vec<Subscription>,
    generation: u64,
}

impl GitShowView {
    pub(crate) fn loading(
        cache: Rc<RefCell<DocumentPageCache>>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(true),
            body: GitShowBody::Loading,
            snapshot: None,
            cache,
            document_id: None,
            open_material: None,
            subscriptions: Vec::new(),
            generation: 0,
        }
    }

    pub(crate) fn begin_load(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    pub(crate) fn apply_result(
        &mut self,
        generation: u64,
        result: Result<GitShowSnapshot, crate::git::GitError>,
        cx: &mut Context<'_, Self>,
    ) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(snapshot) => self.apply_snapshot(snapshot, cx),
            Err(_) => {
                self.close_body(cx);
                self.snapshot = None;
                self.body = GitShowBody::Error;
                cx.notify();
            }
        }
    }

    fn apply_snapshot(&mut self, snapshot: GitShowSnapshot, cx: &mut Context<'_, Self>) {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|current| current.blob == snapshot.blob)
        {
            self.snapshot = Some(snapshot);
            cx.notify();
            return;
        }
        self.close_body(cx);
        let document_id = DocumentId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        let text_state = TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap();
        self.document_id = Some(document_id);
        let automatic_language = LanguageRegistry::new()
            .for_path(Path::new(snapshot.path.as_str()))
            .unwrap_or(LanguageId::PlainText);
        self.subscriptions.clear();
        self.body = match snapshot.content() {
            GitShowContent::Text { text } => {
                let editor = cx.new(|cx| {
                    DocumentTextView::fixed(
                        DocumentTextSnapshot {
                            text: ropey::Rope::from_str(text),
                            revision: 0,
                            text_state,
                            read_only: true,
                        },
                        document_id,
                        automatic_language,
                        cx,
                    )
                });
                editor.update(cx, |editor, _| editor.share_focus(self.focus.clone()));
                self.subscriptions.push(cx.subscribe(
                    &editor,
                    |_, _, event: &crate::app_shell::text_view::DocumentTextEvent, cx| {
                        if matches!(
                            event,
                            crate::app_shell::text_view::DocumentTextEvent::OpenInSystem
                        ) {
                            cx.emit(GitShowEvent::OpenInSystem);
                        }
                    },
                ));
                GitShowBody::Text(editor)
            }
            GitShowContent::Paged(source) => {
                let fixed = snapshot.clone();
                let reader: PageReader = Arc::new(move |page| {
                    fixed.page_text(page).ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "固定版本页号越界")
                    })
                });
                let request = DocumentTextRequest::for_paged_snapshot(
                    document_id,
                    source.clone(),
                    snapshot.generation,
                );
                let cache = self.cache.clone();
                let paged = cx.new(|cx| {
                    PagedTextView::fixed(
                        source.clone(),
                        document_id,
                        text_state,
                        cache,
                        reader,
                        request,
                        cx,
                    )
                });
                paged.update(cx, |paged, _| paged.share_focus(self.focus.clone()));
                GitShowBody::Paged(paged)
            }
            GitShowContent::Binary => GitShowBody::Binary,
            GitShowContent::InvalidUtf8 => GitShowBody::InvalidUtf8,
        };
        self.snapshot = Some(snapshot);
        cx.notify();
    }

    pub(crate) fn snapshot(&self) -> Option<GitShowSnapshot> {
        self.snapshot.clone()
    }

    pub(crate) fn set_open_material(&mut self, material: GitShowOpenMaterial) {
        self.open_material = Some(material);
    }

    fn close_body(&mut self, cx: &mut App) {
        if let GitShowBody::Paged(view) = &self.body {
            view.update(cx, |view, _| view.close());
        }
        if let Some(document_id) = self.document_id.take() {
            self.cache.borrow_mut().remove_document(document_id);
        }
        self.subscriptions.clear();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn is_error_for_test(&self) -> bool {
        matches!(self.body, GitShowBody::Error)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_transients_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let GitShowBody::Text(editor) = &self.body else {
            return;
        };
        editor.update(cx, |editor, cx| {
            gpui::EntityInputHandler::set_selected_text_range(editor, 0..5, window, cx);
            editor.set_scroll_for_test(3, 0);
        });
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn transients_for_test(&self, cx: &App) -> (usize, std::ops::Range<usize>) {
        let GitShowBody::Text(editor) = &self.body else {
            return (0, 0..0);
        };
        let state = editor.read(cx).scroll_state_for_test();
        (state.0, state.3)
    }

    fn message(&self, message: Message, open: bool, cx: &mut Context<'_, Self>) -> AnyElement {
        let language = cx.global::<Settings>().language;
        let mut view = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .track_focus(&self.focus)
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .child(language.text(message));
        if open {
            view = view.child(
                Button::new("gitshow-open-system")
                    .debug_selector(|| "gitshow-open-system".to_owned())
                    .label(language.text(Message::GitshowOpenSystem))
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(GitShowEvent::OpenInSystem);
                    })),
            );
        }
        view.into_any_element()
    }
}

impl Focusable for GitShowView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GitShowView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let content = match &self.body {
            GitShowBody::Loading => self.message(Message::GitshowLoading, false, cx),
            GitShowBody::Text(view) => view.clone().into_any_element(),
            GitShowBody::Paged(view) => view.clone().into_any_element(),
            GitShowBody::Binary => self.message(Message::GitshowBinary, true, cx),
            GitShowBody::InvalidUtf8 => self.message(Message::GitshowInvalidEncoding, true, cx),
            GitShowBody::Error => self.message(Message::GitshowReadFailed, false, cx),
        };
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.requires_size_notice())
        {
            let language = cx.global::<Settings>().language;
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .debug_selector(|| "gitshow-size-notice".to_owned())
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::GitshowLarge)),
                )
                .child(div().flex_1().min_h_0().child(content))
                .into_any_element()
        } else {
            content
        }
    }
}

impl Drop for GitShowView {
    fn drop(&mut self) {
        if let Some(document_id) = self.document_id.take() {
            self.cache.borrow_mut().remove_document(document_id);
        }
        // PagedTextView::drop 会关闭在途任务；中转文件由 open_material Drop 清理。
    }
}

pub(crate) struct GitShowOpenRequest {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) target_group: Option<slterm_workspace::PanelGroupId>,
    pub(crate) binding: slterm_document::RepositoryBinding,
    pub(crate) path: slterm_document::RepoPath,
    pub(crate) focus: bool,
    pub(crate) activate_project: bool,
}

impl super::AppRoot {
    pub(crate) fn open_gitshow(
        &mut self,
        request: GitShowOpenRequest,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<slterm_workspace::PanelId, super::commands::AppRootError> {
        let GitShowOpenRequest {
            project_id,
            target_group,
            binding,
            path,
            focus,
            activate_project,
        } = request;
        self.ensure_not_closing()?;
        let commit_id = binding.head().clone();
        let view_key = slterm_workspace::ViewKey::GitShow {
            project_id,
            repository_binding: Box::new(binding),
            commit_id,
            repo_path: path,
        };
        if let Ok(panel_id) = self.panel_for_view(project_id, &view_key, cx) {
            self.activate_view(view_key, window, cx)?;
            return Ok(panel_id);
        }
        let already_in_layout = self
            .snapshot
            .registry()
            .project(project_id)
            .is_some_and(|project| project.layout().view_keys().contains(&view_key));
        let candidate = if already_in_layout {
            None
        } else {
            Some(self.snapshot.with_opened_view(
                project_id,
                target_group.ok_or(super::commands::AppRootError::MissingPanel)?,
                view_key.clone(),
            )?)
        };
        let cache = self.page_cache.clone();
        let installed = Rc::new(RefCell::new(None));
        let installed_in_build = installed.clone();
        let panel_id = self.install_panel(
            super::documents::DocumentPanelTarget {
                project_id,
                view: view_key.clone(),
                focus,
                activate_project,
            },
            window,
            cx,
            move |_, cx| {
                let view = cx.new(|cx| GitShowView::loading(cache, cx));
                let focus = view.read(cx).focus_handle(cx);
                *installed_in_build.borrow_mut() = Some(view.clone());
                (panel_content(view), focus)
            },
        )?;
        let view = installed
            .borrow()
            .clone()
            .expect("面板安装必须构造固定视图");
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |root, view, event: &GitShowEvent, _, cx| match event {
                GitShowEvent::OpenInSystem => root.open_gitshow_system(panel_id, view.clone(), cx),
            },
        );
        self.panel_subscriptions.insert(panel_id, subscription);
        if let Some(candidate) = candidate {
            self.commit_project_snapshot(candidate, cx);
        }
        let slterm_workspace::ViewKey::GitShow {
            repository_binding,
            repo_path,
            ..
        } = &view_key
        else {
            unreachable!()
        };
        self.schedule_gitshow_load(
            panel_id,
            view,
            (**repository_binding).clone(),
            repo_path.clone(),
            cx,
        );
        Ok(panel_id)
    }

    fn schedule_gitshow_load(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        view: Entity<GitShowView>,
        binding: slterm_document::RepositoryBinding,
        path: slterm_document::RepoPath,
        cx: &mut Context<'_, Self>,
    ) {
        let task_id = super::task_service::TaskId::GitShowLoad(panel_id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let generation = view.update(cx, |view, _| view.begin_load());
        let grants = self.gitshow_grants(&binding);
        let git = self.git.clone();
        let executor = cx.background_executor().clone();
        let load = executor.spawn(async move { git.show(&binding, &path, &grants, 0, generation) });
        let task_view = view.downgrade();
        let registration = self.tasks.register_with(
            task_id,
            super::task_service::CancellationToken::new(),
            move || {
                cx.spawn(async move |root, cx| {
                    let result = load.await;
                    let _ = root.update(cx, |root, cx| {
                        let _ = task_view
                            .update(cx, |view, cx| view.apply_result(generation, result, cx));
                        root.tasks.complete(task_id);
                    });
                })
            },
        );
        if registration.is_err() {
            log::debug!("gitshow load already scheduled");
        }
    }

    fn gitshow_grants(
        &self,
        binding: &slterm_document::RepositoryBinding,
    ) -> Vec<slterm_path::PathGrant> {
        let mut grants = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .filter(|project| project.root_status() == slterm_workspace::ProjectRootStatus::Ready)
            .map(|project| slterm_path::PathGrant::project_root(project.root().clone()))
            .collect::<Vec<_>>();
        grants.extend(
            [
                binding.worktree_root(),
                binding.git_dir(),
                binding.common_dir(),
            ]
            .into_iter()
            .map(|path| slterm_path::PathGrant::repo_root(path.canonical_path().clone())),
        );
        grants
    }

    fn open_gitshow_system(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        view: Entity<GitShowView>,
        cx: &mut Context<'_, Self>,
    ) {
        let task_id = super::task_service::TaskId::GitShowSystemOpen(panel_id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let Some(snapshot) = view.read(cx).snapshot() else {
            return;
        };
        let executor = cx.background_executor().clone();
        let open = executor.spawn(async move { crate::git::open_gitshow_snapshot(&snapshot) });
        let task_view = view.downgrade();
        let registration = self.tasks.register_with(
            task_id,
            super::task_service::CancellationToken::new(),
            move || {
                cx.spawn(async move |root, cx| {
                    let material = open.await;
                    let _ = root.update(cx, |root, cx| {
                        if let Ok(material) = material {
                            let _ =
                                task_view.update(cx, |view, _| view.set_open_material(material));
                        }
                        root.tasks.complete(task_id);
                    });
                })
            },
        );
        if registration.is_err() {
            log::debug!("gitshow system open already scheduled");
        }
    }
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct GitShowViewState {}

fn decode_view_state(
    key: &slterm_workspace::ViewKey,
    text: &str,
) -> Result<serde_json::Value, slterm_workspace::InvalidViewState> {
    if !matches!(key, slterm_workspace::ViewKey::GitShow { .. }) {
        return Err(slterm_workspace::InvalidViewState);
    }
    let state: GitShowViewState =
        serde_json::from_str(text).map_err(|_| slterm_workspace::InvalidViewState)?;
    serde_json::to_value(state).map_err(|_| slterm_workspace::InvalidViewState)
}

pub(super) fn view_state_schema() -> slterm_workspace::ViewStateSchema {
    slterm_workspace::ViewStateSchema::new(
        slterm_workspace::ViewKeyKind::GitShow,
        decode_view_state,
    )
}

pub(crate) fn panel_content(view: Entity<GitShowView>) -> PanelContent {
    PanelContent::GitShow(view)
}
