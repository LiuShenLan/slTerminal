//! 运行面板宿主：稳定视图键绑定封闭的终端或编辑器实体。

use gpui::{App, Entity, FocusHandle};
use slterm_workspace::{PanelId, ViewKey};

use crate::app_shell::file_editor::TextFileView;
use crate::gpui_terminal::view::TerminalView;

pub(crate) enum PanelContent {
    Terminal(Entity<TerminalView>),
    #[allow(dead_code)] // 消费点 = Document 真实打开与后续编辑器视图集成。
    Editor(Entity<TextFileView>),
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
            PanelContent::Editor(_) => None,
        }
    }

    #[allow(dead_code)] // 消费点 = Document 跨项目已保存快照复用。
    pub(crate) fn editor(&self) -> Option<&Entity<TextFileView>> {
        match &self.content {
            PanelContent::Editor(editor) => Some(editor),
            PanelContent::Terminal(_) => None,
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.content {
            PanelContent::Terminal(terminal) => terminal.read(cx).focus_handle.clone(),
            PanelContent::Editor(editor) => editor.read(cx).focus_handle(cx),
        }
    }
}
