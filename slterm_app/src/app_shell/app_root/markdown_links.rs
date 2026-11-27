//! Markdown 点击链接的后台分派与统一打开落点。

use gpui::{App, Context, Window};
use slterm_document::{DocumentId, MarkdownLink};

use super::AppRoot;
use super::events::notify_markdown_link_failure;
use super::panel_host::PanelContent;
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::markdown::link::{
    MarkdownLinkDispatcher, MarkdownLinkFailure, MarkdownLinkOutcome,
};

impl AppRoot {
    pub(super) fn open_markdown_link(
        &mut self,
        document_id: DocumentId,
        link: MarkdownLink,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(session) = self.documents.session(document_id) else {
            return;
        };
        let location = session.location().clone();
        let revision = session.text_revision();
        let grants = self.document_read_grants(document_id);
        let task_id = TaskId::MarkdownLink(uuid::Uuid::now_v7());
        let dispatcher = MarkdownLinkDispatcher::default();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let dispatch = executor
                    .spawn(async move { dispatcher.dispatch(&location, &link, &grants, 0) });
                cx.spawn(async move |this, cx| {
                    let outcome = dispatch.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task_id);
                            match outcome {
                                Ok(MarkdownLinkOutcome::Anchor(target)) => {
                                    root.scroll_markdown_anchor(document_id, target, revision, cx);
                                }
                                Ok(MarkdownLinkOutcome::Local {
                                    document: resolved,
                                    fragment,
                                }) => {
                                    root.open_markdown_resolved(
                                        document_id,
                                        resolved,
                                        fragment,
                                        window,
                                        cx,
                                    );
                                }
                                Ok(MarkdownLinkOutcome::System) => {}
                                Ok(MarkdownLinkOutcome::Rejected(failure)) => {
                                    notify_markdown_link_failure(failure, cx);
                                }
                                Err(error) => {
                                    log::warn!("Markdown 链接分派失败: {error:?}");
                                    notify_markdown_link_failure(
                                        MarkdownLinkFailure::UnreadableTarget,
                                        cx,
                                    );
                                }
                            }
                        });
                    });
                })
            });
        if let Err(error) = registration {
            log::warn!("Markdown 链接任务未登记: {error:?}");
        }
    }

    fn markdown_views(
        &self,
        document_id: DocumentId,
        cx: &App,
    ) -> Vec<gpui::Entity<crate::app_shell::markdown::MarkdownView>> {
        self.projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| match &panel.content {
                        PanelContent::Markdown(markdown)
                            if matches!(&panel.view_key, slterm_workspace::ViewKey::Markdown { document_id: id, .. } if *id == document_id) =>
                        {
                            Some(markdown.view.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn scroll_markdown_anchor(
        &mut self,
        document_id: DocumentId,
        target: Option<slterm_document::BlockId>,
        revision: u64,
        cx: &mut Context<'_, Self>,
    ) {
        for view in self.markdown_views(document_id, cx) {
            view.update(cx, |view, cx| view.scroll_to_anchor(target, revision, cx));
        }
    }

    pub(super) fn scroll_markdown_fragment(
        &mut self,
        document_id: DocumentId,
        fragment: Option<&str>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(fragment) = fragment else {
            return;
        };
        let Some(revision) = self
            .documents
            .session(document_id)
            .map(|session| session.text_revision())
        else {
            return;
        };
        for view in self.markdown_views(document_id, cx) {
            view.update(cx, |view, cx| {
                view.scroll_to_fragment(fragment, revision, cx)
            });
        }
    }

    fn open_markdown_resolved(
        &mut self,
        source_document: DocumentId,
        resolved: crate::app_shell::documents::ResolvedDocument,
        fragment: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let project_id = self.snapshot.registry().active().or_else(|| {
            self.snapshot.registry().projects().iter().find_map(|project| {
                project
                    .layout()
                    .view_keys()
                    .iter()
                    .any(|view| {
                        matches!(view, slterm_workspace::ViewKey::Markdown { document_id, .. } if *document_id == source_document)
                    })
                    .then_some(project.id())
            })
        });
        let Some(project_id) = project_id else {
            notify_markdown_link_failure(MarkdownLinkFailure::UnreadableTarget, cx);
            return;
        };
        let target = self.focused_group(project_id, cx).ok().flatten();
        let path = resolved.location().canonical_path().clone();
        let request = match self.prepare_document_open(project_id, target, path, false) {
            Ok(request) => request,
            Err(error) => {
                log::warn!("Markdown 本地链接落点无效: {error}");
                notify_markdown_link_failure(MarkdownLinkFailure::UnreadableTarget, cx);
                return;
            }
        };
        self.queue_resolved_document_open(request, resolved, fragment, window, cx);
    }
}
