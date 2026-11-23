//! 运行面板宿主：稳定视图键绑定封闭的终端或文档实体。

use gpui::{App, Entity, FocusHandle, Focusable as _, Subscription};
use slterm_workspace::{PanelId, ViewKey};

use crate::app_shell::markdown::MarkdownView;
use crate::app_shell::text_view::{DocumentTextView, PagedTextView};
use crate::gpui_terminal::view::TerminalView;

use super::document_recovery::DocumentRecoveryView;

pub(crate) struct EditorContent {
    pub(crate) view: Entity<DocumentTextView>,
    _save_subscription: Subscription,
}

impl EditorContent {
    #[allow(dead_code)] // 消费点归 Diff 复合视图保留保存订阅。
    pub(crate) fn into_parts(self) -> (Entity<DocumentTextView>, Subscription) {
        (self.view, self._save_subscription)
    }

    pub(crate) fn new(view: Entity<DocumentTextView>, save_subscription: Subscription) -> Self {
        Self {
            view,
            _save_subscription: save_subscription,
        }
    }
}

pub(crate) struct MarkdownContent {
    pub(crate) view: Entity<MarkdownView>,
    _save_subscription: Subscription,
}

impl MarkdownContent {
    pub(crate) fn new(view: Entity<MarkdownView>, save_subscription: Subscription) -> Self {
        Self {
            view,
            _save_subscription: save_subscription,
        }
    }
}

pub(crate) enum PanelContent {
    Terminal(Entity<TerminalView>),
    Editor(EditorContent),
    Markdown(MarkdownContent),
    #[allow(dead_code)] // 消费点归共享 Document Diff 面板。
    Diff(Entity<super::diff::DiffView>),
    Paged(Entity<PagedTextView>),
    GitShow(Entity<super::git_show::GitShowView>),
    Recovery(Entity<DocumentRecoveryView>),
}

pub(crate) struct PanelHost {
    pub(crate) id: PanelId,
    pub(crate) view_key: ViewKey,
    pub(crate) content: PanelContent,
}

impl PanelHost {
    pub(crate) fn terminal(&self) -> Option<&Entity<TerminalView>> {
        match &self.content {
            PanelContent::Terminal(terminal) => Some(terminal),
            _ => None,
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn editor(&self) -> Option<&Entity<DocumentTextView>> {
        match &self.content {
            PanelContent::Editor(editor) => Some(&editor.view),
            _ => None,
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn markdown(&self) -> Option<&Entity<MarkdownView>> {
        match &self.content {
            PanelContent::Markdown(markdown) => Some(&markdown.view),
            _ => None,
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.content {
            PanelContent::Terminal(terminal) => terminal.read(cx).focus_handle.clone(),
            PanelContent::Editor(editor) => editor.view.read(cx).focus_handle(cx),
            PanelContent::Markdown(markdown) => markdown.view.read(cx).focus_handle(cx),
            PanelContent::Diff(view) => view.read(cx).focus_handle(cx),
            PanelContent::Paged(paged) => paged.read(cx).focus_handle(cx),
            PanelContent::GitShow(view) => view.read(cx).focus_handle(cx),
            PanelContent::Recovery(recovery) => recovery.read(cx).focus_handle.clone(),
        }
    }
}
