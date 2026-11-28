//! 应用命令目录：请求、上下文快照与统一执行入口。

use std::collections::{BTreeMap, BTreeSet};

use gpui::{
    Action, App, Context, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Pixels,
    Point, Window,
};
use slterm_document::DocumentId;
use slterm_path::CanonicalPath;
use slterm_sidebar::ButtonMovement;
use slterm_split::SplitDirection;
use slterm_workspace::{
    CloseDecision, LogicalSize, PanelGroupId, PanelId, ProjectId, SideViewId, TerminalId,
    TerminalRecord, ViewKey,
};

use super::super::text_view::{
    CompleteEditor, FindEditor, FindNextEditor, FindPreviousEditor, GoToLineEditor, SaveAsFile,
    SaveFile, TEXT_VIEW_CONTEXT, TEXT_VIEW_SEARCH_CONTEXT, ToggleFoldEditor,
};
use super::explorer::{ExplorerCancelInlineEdit, ExplorerOpenSelected, ExplorerRenameSelected};
use super::{
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, ActivateSideButton,
    AdjustSidebarRatioDown, AdjustSidebarRatioDownLarge, AdjustSidebarRatioUp,
    AdjustSidebarRatioUpLarge, AdjustSidebarWidthLeft, AdjustSidebarWidthLeftLarge,
    AdjustSidebarWidthRight, AdjustSidebarWidthRightLarge, AdjustSplitDown, AdjustSplitLeft,
    AdjustSplitRight, AdjustSplitUp, AppRoot, CloseActivePanel, CloseActiveTerminal,
    FocusActivityBar, FocusFirstSideButton, FocusLastSideButton, FocusNextPanel,
    FocusNextSideButton, FocusPreviousPanel, FocusPreviousSideButton, NewTerminal, OpenLayoutMenu,
    OpenSettings, OpenSideButtonMenu, ReconnectTerminal, ResetSidebarRatio, ResetSidebarWidth,
    SplitDown, SplitRight, ToggleSidebar,
    commands::AppRootError,
    focus_manager::FocusError,
    sidebar::{SideButtonCommand, SideButtonNavigation, SidebarGeometryCommand},
};
use gpui_component::input::{
    Copy as InputCopy, Paste as InputPaste, Redo as InputRedo, SelectAll as InputSelectAll,
    Undo as InputUndo,
};

/// 统一目录中的封闭命令。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CommandId {
    NewTerminal,
    SplitRight,
    SplitDown,
    CloseActiveTerminal,
    CloseActivePanel,
    ReconnectTerminal,
    CloseOtherViews,
    CloseAllViews,
    FocusNextPanel,
    FocusPreviousPanel,
    OpenLayoutMenu,
    OpenSettings,
    MoveView,
    MergeGroups,
    SplitView,
    ResizeSplitLeft,
    ResizeSplitRight,
    ResizeSplitUp,
    ResizeSplitDown,
    FocusSplitDivider,
    RemoveProject,
    #[allow(dead_code)] // 导航树选中项入口由导航视图接线。
    RenameProject,
    #[allow(dead_code)] // 导航树选中项入口由导航视图接线。
    MoveProject,
    ActivateNavView,
    ActivateExplorerView,
    ActivateCommitView,
    ActivateAgentFilesView,
    ToggleSidebar,
    FocusActivityBar,
    FocusNextSideButton,
    FocusPreviousSideButton,
    FocusFirstSideButton,
    FocusLastSideButton,
    ActivateSideButton,
    OpenSideButtonMenu,
    AdjustSidebarWidthLeft,
    AdjustSidebarWidthRight,
    AdjustSidebarWidthLeftLarge,
    AdjustSidebarWidthRightLarge,
    ResetSidebarWidth,
    AdjustSidebarRatioUp,
    AdjustSidebarRatioDown,
    AdjustSidebarRatioUpLarge,
    AdjustSidebarRatioDownLarge,
    ResetSidebarRatio,
    ReorderSideButton,
    MoveSideButtonToOtherSlot,
    HideSideButton,
    ResetSideButtonOrder,
    EditorSave,
    EditorSaveAs,
    EditorFind,
    EditorFindNext,
    EditorFindPrevious,
    EditorCopy,
    EditorPaste,
    EditorUndo,
    EditorRedo,
    EditorSelectAll,
    EditorGoToLine,
    EditorToggleFold,
    EditorComplete,
    ExplorerRename,
    ExplorerOpen,
    ExplorerCancelEdit,
}

impl CommandId {
    /// 持久键位使用的稳定 slug；不得随枚举顺序变化。
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::NewTerminal => "command.new-terminal",
            Self::SplitRight => "command.split-right",
            Self::SplitDown => "command.split-down",
            Self::CloseActiveTerminal => "command.close-active-terminal",
            Self::CloseActivePanel => "command.close-active-panel",
            Self::ReconnectTerminal => "command.reconnect-terminal",
            Self::CloseOtherViews => "command.close-other-views",
            Self::CloseAllViews => "command.close-all-views",
            Self::FocusNextPanel => "command.focus-next-panel",
            Self::FocusPreviousPanel => "command.focus-previous-panel",
            Self::OpenLayoutMenu => "command.open-layout-menu",
            Self::OpenSettings => "command.open-settings",
            Self::MoveView => "command.move-view",
            Self::MergeGroups => "command.merge-groups",
            Self::SplitView => "command.split-view",
            Self::ResizeSplitLeft => "command.resize-split-left",
            Self::ResizeSplitRight => "command.resize-split-right",
            Self::ResizeSplitUp => "command.resize-split-up",
            Self::ResizeSplitDown => "command.resize-split-down",
            Self::FocusSplitDivider => "command.focus-split-divider",
            Self::RemoveProject => "command.remove-project",
            Self::RenameProject => "command.rename-project",
            Self::MoveProject => "command.move-project",
            Self::ActivateNavView => "command.activate-nav-view",
            Self::ActivateExplorerView => "command.activate-explorer-view",
            Self::ActivateCommitView => "command.activate-commit-view",
            Self::ActivateAgentFilesView => "command.activate-agent-files-view",
            Self::ToggleSidebar => "command.toggle-sidebar",
            Self::FocusActivityBar => "command.focus-activity-bar",
            Self::FocusNextSideButton => "command.focus-next-side-button",
            Self::FocusPreviousSideButton => "command.focus-previous-side-button",
            Self::FocusFirstSideButton => "command.focus-first-side-button",
            Self::FocusLastSideButton => "command.focus-last-side-button",
            Self::ActivateSideButton => "command.activate-side-button",
            Self::OpenSideButtonMenu => "command.open-side-button-menu",
            Self::AdjustSidebarWidthLeft => "command.adjust-sidebar-width-left",
            Self::AdjustSidebarWidthRight => "command.adjust-sidebar-width-right",
            Self::AdjustSidebarWidthLeftLarge => "command.adjust-sidebar-width-left-large",
            Self::AdjustSidebarWidthRightLarge => "command.adjust-sidebar-width-right-large",
            Self::ResetSidebarWidth => "command.reset-sidebar-width",
            Self::AdjustSidebarRatioUp => "command.adjust-sidebar-ratio-up",
            Self::AdjustSidebarRatioDown => "command.adjust-sidebar-ratio-down",
            Self::AdjustSidebarRatioUpLarge => "command.adjust-sidebar-ratio-up-large",
            Self::AdjustSidebarRatioDownLarge => "command.adjust-sidebar-ratio-down-large",
            Self::ResetSidebarRatio => "command.reset-sidebar-ratio",
            Self::ReorderSideButton => "command.reorder-side-button",
            Self::MoveSideButtonToOtherSlot => "command.move-side-button-to-other-slot",
            Self::HideSideButton => "command.hide-side-button",
            Self::ResetSideButtonOrder => "command.reset-side-button-order",
            Self::EditorSave => "command.editor-save",
            Self::EditorSaveAs => "command.editor-save-as",
            Self::EditorFind => "command.editor-find",
            Self::EditorFindNext => "command.editor-find-next",
            Self::EditorFindPrevious => "command.editor-find-previous",
            Self::EditorCopy => "command.editor-copy",
            Self::EditorPaste => "command.editor-paste",
            Self::EditorUndo => "command.editor-undo",
            Self::EditorRedo => "command.editor-redo",
            Self::EditorSelectAll => "command.editor-select-all",
            Self::EditorGoToLine => "command.editor-go-to-line",
            Self::EditorToggleFold => "command.editor-toggle-fold",
            Self::EditorComplete => "command.editor-complete",
            Self::ExplorerRename => "command.explorer-rename",
            Self::ExplorerOpen => "command.explorer-open",
            Self::ExplorerCancelEdit => "command.explorer-cancel-edit",
        }
    }

    pub(crate) fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "command.new-terminal" => Some(Self::NewTerminal),
            "command.split-right" => Some(Self::SplitRight),
            "command.split-down" => Some(Self::SplitDown),
            "command.close-active-terminal" => Some(Self::CloseActiveTerminal),
            "command.close-active-panel" => Some(Self::CloseActivePanel),
            "command.reconnect-terminal" => Some(Self::ReconnectTerminal),
            "command.close-other-views" => Some(Self::CloseOtherViews),
            "command.close-all-views" => Some(Self::CloseAllViews),
            "command.focus-next-panel" => Some(Self::FocusNextPanel),
            "command.focus-previous-panel" => Some(Self::FocusPreviousPanel),
            "command.open-layout-menu" => Some(Self::OpenLayoutMenu),
            "command.open-settings" => Some(Self::OpenSettings),
            "command.move-view" => Some(Self::MoveView),
            "command.merge-groups" => Some(Self::MergeGroups),
            "command.split-view" => Some(Self::SplitView),
            "command.resize-split-left" => Some(Self::ResizeSplitLeft),
            "command.resize-split-right" => Some(Self::ResizeSplitRight),
            "command.resize-split-up" => Some(Self::ResizeSplitUp),
            "command.resize-split-down" => Some(Self::ResizeSplitDown),
            "command.focus-split-divider" => Some(Self::FocusSplitDivider),
            "command.remove-project" => Some(Self::RemoveProject),
            "command.rename-project" => Some(Self::RenameProject),
            "command.move-project" => Some(Self::MoveProject),
            "command.activate-nav-view" => Some(Self::ActivateNavView),
            "command.activate-explorer-view" => Some(Self::ActivateExplorerView),
            "command.activate-commit-view" => Some(Self::ActivateCommitView),
            "command.activate-agent-files-view" => Some(Self::ActivateAgentFilesView),
            "command.toggle-sidebar" => Some(Self::ToggleSidebar),
            "command.focus-activity-bar" => Some(Self::FocusActivityBar),
            "command.focus-next-side-button" => Some(Self::FocusNextSideButton),
            "command.focus-previous-side-button" => Some(Self::FocusPreviousSideButton),
            "command.focus-first-side-button" => Some(Self::FocusFirstSideButton),
            "command.focus-last-side-button" => Some(Self::FocusLastSideButton),
            "command.activate-side-button" => Some(Self::ActivateSideButton),
            "command.open-side-button-menu" => Some(Self::OpenSideButtonMenu),
            "command.adjust-sidebar-width-left" => Some(Self::AdjustSidebarWidthLeft),
            "command.adjust-sidebar-width-right" => Some(Self::AdjustSidebarWidthRight),
            "command.adjust-sidebar-width-left-large" => Some(Self::AdjustSidebarWidthLeftLarge),
            "command.adjust-sidebar-width-right-large" => Some(Self::AdjustSidebarWidthRightLarge),
            "command.reset-sidebar-width" => Some(Self::ResetSidebarWidth),
            "command.adjust-sidebar-ratio-up" => Some(Self::AdjustSidebarRatioUp),
            "command.adjust-sidebar-ratio-down" => Some(Self::AdjustSidebarRatioDown),
            "command.adjust-sidebar-ratio-up-large" => Some(Self::AdjustSidebarRatioUpLarge),
            "command.adjust-sidebar-ratio-down-large" => Some(Self::AdjustSidebarRatioDownLarge),
            "command.reset-sidebar-ratio" => Some(Self::ResetSidebarRatio),
            "command.reorder-side-button" => Some(Self::ReorderSideButton),
            "command.move-side-button-to-other-slot" => Some(Self::MoveSideButtonToOtherSlot),
            "command.hide-side-button" => Some(Self::HideSideButton),
            "command.reset-side-button-order" => Some(Self::ResetSideButtonOrder),
            "command.editor-save" => Some(Self::EditorSave),
            "command.editor-save-as" => Some(Self::EditorSaveAs),
            "command.editor-find" => Some(Self::EditorFind),
            "command.editor-find-next" => Some(Self::EditorFindNext),
            "command.editor-find-previous" => Some(Self::EditorFindPrevious),
            "command.editor-copy" => Some(Self::EditorCopy),
            "command.editor-paste" => Some(Self::EditorPaste),
            "command.editor-undo" => Some(Self::EditorUndo),
            "command.editor-redo" => Some(Self::EditorRedo),
            "command.editor-select-all" => Some(Self::EditorSelectAll),
            "command.editor-go-to-line" => Some(Self::EditorGoToLine),
            "command.editor-toggle-fold" => Some(Self::EditorToggleFold),
            "command.editor-complete" => Some(Self::EditorComplete),
            "command.explorer-rename" => Some(Self::ExplorerRename),
            "command.explorer-open" => Some(Self::ExplorerOpen),
            "command.explorer-cancel-edit" => Some(Self::ExplorerCancelEdit),
            _ => None,
        }
    }
}

/// 命令来源只用于表达入口，不改变领域规则。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandSource {
    Keyboard,
    Application,
    Pointer,
    Menu,
}

/// 当前终端命令都作用于执行时的聚焦上下文。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CommandTarget {
    Focused,
    #[allow(dead_code)] // 导航与 Runtime 的项目入口按稳定身份构造该目标。
    Project(ProjectId),
    SideView(SideViewId),
    SideButton(SideViewId),
    SidebarWidthDivider,
    SidebarRatioDivider,
}

/// 执行前冻结的上下文；不作为第二份布局或焦点事实源。
#[derive(Clone, Debug)]
pub(crate) struct CommandContext {
    pub(crate) active_project: Option<ProjectId>,
    pub(crate) focused_view: Option<ViewKey>,
    pub(crate) focused_terminal: Option<TerminalId>,
    pub(crate) focused_side_button: Option<SideViewId>,
    pub(crate) modal: bool,
}

/// 可选命令参数；无参数命令必须拒绝非空载荷。
#[derive(Clone, Debug)]
pub(crate) enum CommandParameters {
    NewTerminal {
        shell: CanonicalPath,
        project_root: Option<CanonicalPath>,
        cwd: Option<CanonicalPath>,
    },
    Split {
        ratio: f32,
        viewport: LogicalSize,
        shell: CanonicalPath,
    },
    MoveView {
        view: ViewKey,
        target_group: PanelGroupId,
        target_index: usize,
    },
    MergeGroups {
        source_group: PanelGroupId,
        target_group: PanelGroupId,
    },
    SplitView {
        view: ViewKey,
        target_group: PanelGroupId,
        direction: SplitDirection,
        ratio: f32,
        viewport: LogicalSize,
    },
    ResizeSplit {
        path: Vec<bool>,
        ratio: f32,
        viewport: LogicalSize,
    },
    FocusSplitDivider {
        path: Vec<bool>,
    },
    CloseGroup {
        group_id: PanelGroupId,
        document_decisions: BTreeMap<DocumentId, CloseDecision>,
    },
    #[allow(dead_code)] // 导航树选中项入口由导航视图接线。
    RenameProject {
        name: String,
    },
    #[allow(dead_code)] // 导航树选中项入口由导航视图接线。
    MoveProject {
        new_index: usize,
    },
    SideButtonMovement(ButtonMovement),
    SideButtonMenuPosition(Point<Pixels>),
}

/// 菜单、键盘、设置和 Runtime 共用的命令请求。
#[derive(Clone, Debug)]
pub(crate) struct CommandRequest {
    pub(crate) id: CommandId,
    pub(crate) source: CommandSource,
    pub(crate) target: CommandTarget,
    pub(crate) context: CommandContext,
    pub(crate) parameters: Option<CommandParameters>,
}

/// 命令执行结果保留各命令的既有返回身份。
#[derive(Clone, Debug)]
pub(crate) enum CommandOutcome {
    TerminalPanel(PanelId),
    TerminalClosed(TerminalRecord),
    Completed,
}

pub(crate) struct CommandRegistry;

/// 命令的默认绑定与 GPUI action 映射只在目录登记一次。
pub(crate) struct CommandDefinition {
    pub(crate) id: CommandId,
    pub(crate) target: CommandTarget,
    pub(crate) keystroke: &'static str,
    pub(crate) context: Option<&'static str>,
    action: fn() -> Box<dyn Action>,
}

const APPLICATION_DEFINITIONS: [CommandDefinition; 1] = [CommandDefinition {
    id: CommandId::OpenSettings,
    target: CommandTarget::Focused,
    keystroke: "ctrl-,",
    context: None,
    action: || Box::new(OpenSettings),
}];

const TERMINAL_DEFINITIONS: [CommandDefinition; 6] = [
    CommandDefinition {
        id: CommandId::NewTerminal,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-t",
        context: None,
        action: || Box::new(NewTerminal),
    },
    CommandDefinition {
        id: CommandId::SplitRight,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-d",
        context: None,
        action: || Box::new(SplitRight),
    },
    CommandDefinition {
        id: CommandId::SplitDown,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-s",
        context: None,
        action: || Box::new(SplitDown),
    },
    CommandDefinition {
        id: CommandId::CloseActiveTerminal,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-w",
        context: None,
        action: || Box::new(CloseActiveTerminal),
    },
    CommandDefinition {
        id: CommandId::CloseActivePanel,
        target: CommandTarget::Focused,
        keystroke: "ctrl-w",
        context: None,
        action: || Box::new(CloseActivePanel),
    },
    CommandDefinition {
        id: CommandId::ReconnectTerminal,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-r",
        context: None,
        action: || Box::new(ReconnectTerminal),
    },
];

const PANEL_DEFINITIONS: [CommandDefinition; 2] = [
    CommandDefinition {
        id: CommandId::FocusNextPanel,
        target: CommandTarget::Focused,
        keystroke: "ctrl-tab",
        context: None,
        action: || Box::new(FocusNextPanel),
    },
    CommandDefinition {
        id: CommandId::FocusPreviousPanel,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-tab",
        context: None,
        action: || Box::new(FocusPreviousPanel),
    },
];

const LAYOUT_DEFINITIONS: [CommandDefinition; 5] = [
    CommandDefinition {
        id: CommandId::OpenLayoutMenu,
        target: CommandTarget::Focused,
        keystroke: "shift-f10",
        context: Some(super::layout::PROJECT_LAYOUT_KEY_CONTEXT),
        action: || Box::new(OpenLayoutMenu),
    },
    CommandDefinition {
        id: CommandId::ResizeSplitLeft,
        target: CommandTarget::Focused,
        keystroke: "left",
        context: Some(super::layout::SPLIT_DIVIDER_KEY_CONTEXT),
        action: || Box::new(AdjustSplitLeft),
    },
    CommandDefinition {
        id: CommandId::ResizeSplitRight,
        target: CommandTarget::Focused,
        keystroke: "right",
        context: Some(super::layout::SPLIT_DIVIDER_KEY_CONTEXT),
        action: || Box::new(AdjustSplitRight),
    },
    CommandDefinition {
        id: CommandId::ResizeSplitUp,
        target: CommandTarget::Focused,
        keystroke: "up",
        context: Some(super::layout::SPLIT_DIVIDER_KEY_CONTEXT),
        action: || Box::new(AdjustSplitUp),
    },
    CommandDefinition {
        id: CommandId::ResizeSplitDown,
        target: CommandTarget::Focused,
        keystroke: "down",
        context: Some(super::layout::SPLIT_DIVIDER_KEY_CONTEXT),
        action: || Box::new(AdjustSplitDown),
    },
];

const SIDEBAR_DEFINITIONS: [CommandDefinition; 23] = [
    CommandDefinition {
        id: CommandId::ToggleSidebar,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-b",
        context: None,
        action: || Box::new(ToggleSidebar),
    },
    CommandDefinition {
        id: CommandId::ActivateNavView,
        target: CommandTarget::SideView(SideViewId::Nav),
        keystroke: "ctrl-shift-n",
        context: None,
        action: || Box::new(ActivateNav),
    },
    CommandDefinition {
        id: CommandId::ActivateExplorerView,
        target: CommandTarget::SideView(SideViewId::Explorer),
        keystroke: "ctrl-shift-e",
        context: None,
        action: || Box::new(ActivateExplorer),
    },
    CommandDefinition {
        id: CommandId::ActivateCommitView,
        target: CommandTarget::SideView(SideViewId::Commit),
        keystroke: "ctrl-shift-g",
        context: None,
        action: || Box::new(ActivateCommit),
    },
    CommandDefinition {
        id: CommandId::ActivateAgentFilesView,
        target: CommandTarget::SideView(SideViewId::AgentFiles),
        keystroke: "ctrl-shift-i",
        context: None,
        action: || Box::new(ActivateAgentFiles),
    },
    CommandDefinition {
        id: CommandId::FocusActivityBar,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-l",
        context: None,
        action: || Box::new(FocusActivityBar),
    },
    CommandDefinition {
        id: CommandId::FocusNextSideButton,
        target: CommandTarget::Focused,
        keystroke: "down",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(FocusNextSideButton),
    },
    CommandDefinition {
        id: CommandId::FocusPreviousSideButton,
        target: CommandTarget::Focused,
        keystroke: "up",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(FocusPreviousSideButton),
    },
    CommandDefinition {
        id: CommandId::FocusFirstSideButton,
        target: CommandTarget::Focused,
        keystroke: "home",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(FocusFirstSideButton),
    },
    CommandDefinition {
        id: CommandId::FocusLastSideButton,
        target: CommandTarget::Focused,
        keystroke: "end",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(FocusLastSideButton),
    },
    CommandDefinition {
        id: CommandId::OpenSideButtonMenu,
        target: CommandTarget::Focused,
        keystroke: "shift-f10",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(OpenSideButtonMenu),
    },
    CommandDefinition {
        id: CommandId::ActivateSideButton,
        target: CommandTarget::Focused,
        keystroke: "enter",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(ActivateSideButton),
    },
    CommandDefinition {
        id: CommandId::ActivateSideButton,
        target: CommandTarget::Focused,
        keystroke: "space",
        context: Some(super::sidebar::ACTIVITY_BAR_KEY_CONTEXT),
        action: || Box::new(ActivateSideButton),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarWidthLeft,
        target: CommandTarget::SidebarWidthDivider,
        keystroke: "left",
        context: Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarWidthLeft),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarWidthRight,
        target: CommandTarget::SidebarWidthDivider,
        keystroke: "right",
        context: Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarWidthRight),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarWidthLeftLarge,
        target: CommandTarget::SidebarWidthDivider,
        keystroke: "shift-left",
        context: Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarWidthLeftLarge),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarWidthRightLarge,
        target: CommandTarget::SidebarWidthDivider,
        keystroke: "shift-right",
        context: Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarWidthRightLarge),
    },
    CommandDefinition {
        id: CommandId::ResetSidebarWidth,
        target: CommandTarget::SidebarWidthDivider,
        keystroke: "enter",
        context: Some(super::sidebar::SIDEBAR_WIDTH_KEY_CONTEXT),
        action: || Box::new(ResetSidebarWidth),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarRatioUp,
        target: CommandTarget::SidebarRatioDivider,
        keystroke: "up",
        context: Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarRatioUp),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarRatioDown,
        target: CommandTarget::SidebarRatioDivider,
        keystroke: "down",
        context: Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarRatioDown),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarRatioUpLarge,
        target: CommandTarget::SidebarRatioDivider,
        keystroke: "shift-up",
        context: Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarRatioUpLarge),
    },
    CommandDefinition {
        id: CommandId::AdjustSidebarRatioDownLarge,
        target: CommandTarget::SidebarRatioDivider,
        keystroke: "shift-down",
        context: Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        action: || Box::new(AdjustSidebarRatioDownLarge),
    },
    CommandDefinition {
        id: CommandId::ResetSidebarRatio,
        target: CommandTarget::SidebarRatioDivider,
        keystroke: "enter",
        context: Some(super::sidebar::SIDEBAR_RATIO_KEY_CONTEXT),
        action: || Box::new(ResetSidebarRatio),
    },
];

/// 其他模块已注册的键位：只进入快捷键页与覆盖应用，不重复注册默认绑定。
const EXTERNAL_DEFINITIONS: [CommandDefinition; 20] = [
    CommandDefinition {
        id: CommandId::EditorSave,
        target: CommandTarget::Focused,
        keystroke: "ctrl-s",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(SaveFile),
    },
    CommandDefinition {
        id: CommandId::EditorSaveAs,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-s",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(SaveAsFile),
    },
    CommandDefinition {
        id: CommandId::EditorCopy,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-c",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputCopy),
    },
    CommandDefinition {
        id: CommandId::EditorPaste,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-v",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputPaste),
    },
    CommandDefinition {
        id: CommandId::EditorUndo,
        target: CommandTarget::Focused,
        keystroke: "ctrl-z",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputUndo),
    },
    CommandDefinition {
        id: CommandId::EditorRedo,
        target: CommandTarget::Focused,
        keystroke: "ctrl-y",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputRedo),
    },
    CommandDefinition {
        id: CommandId::EditorRedo,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-z",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputRedo),
    },
    CommandDefinition {
        id: CommandId::EditorSelectAll,
        target: CommandTarget::Focused,
        keystroke: "ctrl-a",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(InputSelectAll),
    },
    CommandDefinition {
        id: CommandId::EditorFind,
        target: CommandTarget::Focused,
        keystroke: "ctrl-f",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(FindEditor),
    },
    CommandDefinition {
        id: CommandId::EditorFind,
        target: CommandTarget::Focused,
        keystroke: "ctrl-f",
        context: Some(TEXT_VIEW_SEARCH_CONTEXT),
        action: || Box::new(FindEditor),
    },
    CommandDefinition {
        id: CommandId::EditorFindNext,
        target: CommandTarget::Focused,
        keystroke: "f3",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(FindNextEditor),
    },
    CommandDefinition {
        id: CommandId::EditorFindNext,
        target: CommandTarget::Focused,
        keystroke: "f3",
        context: Some(TEXT_VIEW_SEARCH_CONTEXT),
        action: || Box::new(FindNextEditor),
    },
    CommandDefinition {
        id: CommandId::EditorFindPrevious,
        target: CommandTarget::Focused,
        keystroke: "shift-f3",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(FindPreviousEditor),
    },
    CommandDefinition {
        id: CommandId::EditorFindPrevious,
        target: CommandTarget::Focused,
        keystroke: "shift-f3",
        context: Some(TEXT_VIEW_SEARCH_CONTEXT),
        action: || Box::new(FindPreviousEditor),
    },
    CommandDefinition {
        id: CommandId::EditorGoToLine,
        target: CommandTarget::Focused,
        keystroke: "ctrl-alt-g",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(GoToLineEditor),
    },
    CommandDefinition {
        id: CommandId::EditorToggleFold,
        target: CommandTarget::Focused,
        keystroke: "ctrl-shift-[",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(ToggleFoldEditor),
    },
    CommandDefinition {
        id: CommandId::EditorComplete,
        target: CommandTarget::Focused,
        keystroke: "ctrl-space",
        context: Some(TEXT_VIEW_CONTEXT),
        action: || Box::new(CompleteEditor),
    },
    CommandDefinition {
        id: CommandId::ExplorerRename,
        target: CommandTarget::Focused,
        keystroke: "f2",
        context: Some(super::EXPLORER_KEY_CONTEXT),
        action: || Box::new(ExplorerRenameSelected),
    },
    CommandDefinition {
        id: CommandId::ExplorerOpen,
        target: CommandTarget::Focused,
        keystroke: "enter",
        context: Some(super::EXPLORER_KEY_CONTEXT),
        action: || Box::new(ExplorerOpenSelected),
    },
    CommandDefinition {
        id: CommandId::ExplorerCancelEdit,
        target: CommandTarget::Focused,
        keystroke: "escape",
        context: Some(super::EXPLORER_KEY_CONTEXT),
        action: || Box::new(ExplorerCancelInlineEdit),
    },
];

/// 快捷键页分组；只表达目录来源，不是持久身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutGroup {
    Global,
    Terminal,
    Layout,
    Sidebar,
    Editor,
    Files,
    Completion,
}

impl ShortcutGroup {
    pub(crate) const ALL: [Self; 7] = [
        Self::Global,
        Self::Terminal,
        Self::Layout,
        Self::Sidebar,
        Self::Editor,
        Self::Files,
        Self::Completion,
    ];

    pub(crate) const fn message(self) -> crate::i18n::Message {
        use crate::i18n::Message;
        match self {
            Self::Global => Message::SettingsShortcutsGroupGlobal,
            Self::Terminal => Message::SettingsShortcutsGroupTerminal,
            Self::Layout => Message::SettingsShortcutsGroupLayout,
            Self::Sidebar => Message::SettingsShortcutsGroupSidebar,
            Self::Editor => Message::SettingsShortcutsGroupEditor,
            Self::Files => Message::SettingsShortcutsGroupFiles,
            Self::Completion => Message::SettingsShortcutsGroupCompletion,
        }
    }
}

impl CommandId {
    /// 命令显示名经 typed i18n；快捷键页与后续录制共用同一标签。
    pub(crate) const fn label_message(self) -> Option<crate::i18n::Message> {
        use crate::i18n::Message;
        match self {
            Self::OpenSettings => Some(Message::SettingsShortcutsOpenSettings),
            Self::NewTerminal => Some(Message::SettingsShortcutsNewTerminal),
            Self::SplitRight => Some(Message::SettingsShortcutsSplitRight),
            Self::SplitDown => Some(Message::SettingsShortcutsSplitDown),
            Self::CloseActiveTerminal => Some(Message::SettingsShortcutsCloseActiveTerminal),
            Self::CloseActivePanel => Some(Message::SettingsShortcutsCloseActivePanel),
            Self::ReconnectTerminal => Some(Message::SettingsShortcutsReconnectTerminal),
            Self::FocusNextPanel => Some(Message::SettingsShortcutsFocusNextPanel),
            Self::FocusPreviousPanel => Some(Message::SettingsShortcutsFocusPreviousPanel),
            Self::OpenLayoutMenu => Some(Message::SettingsShortcutsOpenLayoutMenu),
            Self::ResizeSplitLeft => Some(Message::SettingsShortcutsResizeSplitLeft),
            Self::ResizeSplitRight => Some(Message::SettingsShortcutsResizeSplitRight),
            Self::ResizeSplitUp => Some(Message::SettingsShortcutsResizeSplitUp),
            Self::ResizeSplitDown => Some(Message::SettingsShortcutsResizeSplitDown),
            Self::ToggleSidebar => Some(Message::SettingsShortcutsToggleSidebar),
            Self::ActivateNavView => Some(Message::SidebarViewNav),
            Self::ActivateExplorerView => Some(Message::SidebarViewExplorer),
            Self::ActivateCommitView => Some(Message::SidebarViewCommit),
            Self::ActivateAgentFilesView => Some(Message::SidebarViewAgentFiles),
            Self::FocusActivityBar => Some(Message::SettingsShortcutsFocusActivityBar),
            Self::FocusNextSideButton => Some(Message::SettingsShortcutsFocusNextSideButton),
            Self::FocusPreviousSideButton => {
                Some(Message::SettingsShortcutsFocusPreviousSideButton)
            }
            Self::FocusFirstSideButton => Some(Message::SettingsShortcutsFocusFirstSideButton),
            Self::FocusLastSideButton => Some(Message::SettingsShortcutsFocusLastSideButton),
            Self::OpenSideButtonMenu => Some(Message::SettingsShortcutsOpenSideButtonMenu),
            Self::ActivateSideButton => Some(Message::SettingsShortcutsActivateSideButton),
            Self::AdjustSidebarWidthLeft => Some(Message::SettingsShortcutsAdjustSidebarWidthLeft),
            Self::AdjustSidebarWidthRight => {
                Some(Message::SettingsShortcutsAdjustSidebarWidthRight)
            }
            Self::AdjustSidebarWidthLeftLarge => {
                Some(Message::SettingsShortcutsAdjustSidebarWidthLeftLarge)
            }
            Self::AdjustSidebarWidthRightLarge => {
                Some(Message::SettingsShortcutsAdjustSidebarWidthRightLarge)
            }
            Self::ResetSidebarWidth => Some(Message::SettingsShortcutsResetSidebarWidth),
            Self::AdjustSidebarRatioUp => Some(Message::SettingsShortcutsAdjustSidebarRatioUp),
            Self::AdjustSidebarRatioDown => Some(Message::SettingsShortcutsAdjustSidebarRatioDown),
            Self::AdjustSidebarRatioUpLarge => {
                Some(Message::SettingsShortcutsAdjustSidebarRatioUpLarge)
            }
            Self::AdjustSidebarRatioDownLarge => {
                Some(Message::SettingsShortcutsAdjustSidebarRatioDownLarge)
            }
            Self::ResetSidebarRatio => Some(Message::SettingsShortcutsResetSidebarRatio),
            Self::EditorSave => Some(Message::SettingsShortcutsEditorSave),
            Self::EditorSaveAs => Some(Message::SettingsShortcutsEditorSaveAs),
            Self::EditorFind => Some(Message::SettingsShortcutsEditorFind),
            Self::EditorFindNext => Some(Message::SettingsShortcutsEditorFindNext),
            Self::EditorFindPrevious => Some(Message::SettingsShortcutsEditorFindPrevious),
            Self::EditorCopy => Some(Message::SettingsShortcutsEditorCopy),
            Self::EditorPaste => Some(Message::SettingsShortcutsEditorPaste),
            Self::EditorUndo => Some(Message::SettingsShortcutsEditorUndo),
            Self::EditorRedo => Some(Message::SettingsShortcutsEditorRedo),
            Self::EditorSelectAll => Some(Message::SettingsShortcutsEditorSelectAll),
            Self::EditorGoToLine => Some(Message::SettingsShortcutsEditorGoToLine),
            Self::EditorToggleFold => Some(Message::SettingsShortcutsEditorToggleFold),
            Self::EditorComplete => Some(Message::SettingsShortcutsEditorComplete),
            Self::ExplorerRename => Some(Message::SettingsShortcutsExplorerRename),
            Self::ExplorerOpen => Some(Message::SettingsShortcutsExplorerOpen),
            Self::ExplorerCancelEdit => Some(Message::SettingsShortcutsExplorerCancelEdit),
            // 其余命令尚未并入快捷键目录；不得借用其他命令的标签。
            _ => None,
        }
    }
}

#[cfg(test)]
mod shortcut_label_tests {
    use super::{CommandRegistry, ShortcutGroup};

    /// 目录内每个命令都必须有 typed 标签，页面才可能完整渲染。
    #[test]
    fn every_shortcut_directory_command_has_a_label() {
        for group in ShortcutGroup::ALL {
            for definition in CommandRegistry::shortcut_definitions(group) {
                assert!(
                    definition.id.label_message().is_some(),
                    "快捷键目录命令缺少标签: {:?}",
                    definition.id
                );
            }
        }
    }
}

impl CommandRegistry {
    /// 某一分组的默认定义；快捷键页只读消费，不复制键位表。
    pub(crate) fn shortcut_definitions(group: ShortcutGroup) -> Vec<&'static CommandDefinition> {
        match group {
            ShortcutGroup::Global => APPLICATION_DEFINITIONS.iter().collect(),
            ShortcutGroup::Terminal => TERMINAL_DEFINITIONS
                .iter()
                .chain(PANEL_DEFINITIONS.iter())
                .collect(),
            ShortcutGroup::Layout => LAYOUT_DEFINITIONS.iter().collect(),
            ShortcutGroup::Sidebar => SIDEBAR_DEFINITIONS.iter().collect(),
            ShortcutGroup::Editor => EXTERNAL_DEFINITIONS
                .iter()
                .filter(|definition| {
                    matches!(
                        definition.id,
                        CommandId::EditorSave
                            | CommandId::EditorSaveAs
                            | CommandId::EditorCopy
                            | CommandId::EditorPaste
                            | CommandId::EditorUndo
                            | CommandId::EditorRedo
                            | CommandId::EditorSelectAll
                            | CommandId::EditorFind
                            | CommandId::EditorFindNext
                            | CommandId::EditorFindPrevious
                            | CommandId::EditorGoToLine
                            | CommandId::EditorToggleFold
                            | CommandId::EditorComplete
                    )
                })
                .collect(),
            ShortcutGroup::Files => EXTERNAL_DEFINITIONS
                .iter()
                .filter(|definition| {
                    matches!(
                        definition.id,
                        CommandId::ExplorerRename
                            | CommandId::ExplorerOpen
                            | CommandId::ExplorerCancelEdit
                    )
                })
                .collect(),
            ShortcutGroup::Completion => Vec::new(),
        }
    }

    /// 全部命令定义（含未显示标签的目录项）；键位覆盖与派发共用同一来源。
    pub(crate) fn all_definitions() -> impl Iterator<Item = &'static CommandDefinition> {
        APPLICATION_DEFINITIONS
            .iter()
            .chain(TERMINAL_DEFINITIONS.iter())
            .chain(PANEL_DEFINITIONS.iter())
            .chain(LAYOUT_DEFINITIONS.iter())
            .chain(SIDEBAR_DEFINITIONS.iter())
            .chain(EXTERNAL_DEFINITIONS.iter())
    }

    /// 构造命令的 GPUI action，用于用户覆盖绑定。
    pub(crate) fn action_for(id: CommandId) -> Option<Box<dyn Action>> {
        Self::all_definitions()
            .find(|definition| definition.id == id)
            .map(|definition| (definition.action)())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn terminal_definitions() -> &'static [CommandDefinition] {
        &TERMINAL_DEFINITIONS
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn panel_cycle_bindings_for_test() -> Vec<KeyBinding> {
        PANEL_DEFINITIONS
            .iter()
            .map(|definition| {
                KeyBinding::load(
                    definition.keystroke,
                    (definition.action)(),
                    definition
                        .context
                        .map(|context| KeyBindingContextPredicate::parse(context).unwrap().into()),
                    false,
                    None,
                    &DummyKeyboardMapper,
                )
                .expect("面板循环键位必须有效")
            })
            .collect()
    }

    pub(crate) fn register_bindings(cx: &mut App) {
        let mut definitions = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        for definition in APPLICATION_DEFINITIONS
            .iter()
            .chain(TERMINAL_DEFINITIONS.iter())
            .chain(PANEL_DEFINITIONS.iter())
            .chain(LAYOUT_DEFINITIONS.iter())
            .chain(SIDEBAR_DEFINITIONS.iter())
            .chain(EXTERNAL_DEFINITIONS.iter())
        {
            assert!(
                definitions.insert((
                    definition.id,
                    definition.target,
                    definition.keystroke,
                    definition.context,
                )),
                "命令定义不得重复"
            );
            assert!(
                bindings.insert((definition.keystroke, definition.context)),
                "默认键位与上下文不得重复"
            );
            cx.bind_keys([KeyBinding::load(
                definition.keystroke,
                (definition.action)(),
                definition
                    .context
                    .map(|context| KeyBindingContextPredicate::parse(context).unwrap().into()),
                false,
                None,
                &DummyKeyboardMapper,
            )
            .expect("命令目录键位必须有效")]);
        }
    }

    pub(crate) fn execute(
        request: CommandRequest,
        root: &mut AppRoot,
        window: &mut Window,
        cx: &mut Context<'_, AppRoot>,
    ) -> Result<CommandOutcome, AppRootError> {
        if root.close_in_progress || root.application_close.is_some() {
            return Err(AppRootError::Closing);
        }
        let CommandRequest {
            id,
            source,
            target,
            context,
            parameters,
        } = request;
        debug_assert!(matches!(
            source,
            CommandSource::Keyboard
                | CommandSource::Application
                | CommandSource::Pointer
                | CommandSource::Menu
        ));
        let terminal_command = matches!(
            id,
            CommandId::NewTerminal
                | CommandId::SplitRight
                | CommandId::SplitDown
                | CommandId::CloseActiveTerminal
                | CommandId::CloseActivePanel
                | CommandId::ReconnectTerminal
                | CommandId::CloseOtherViews
                | CommandId::CloseAllViews
                | CommandId::FocusNextPanel
                | CommandId::FocusPreviousPanel
        );
        let layout_command = matches!(
            id,
            CommandId::OpenLayoutMenu
                | CommandId::MoveView
                | CommandId::MergeGroups
                | CommandId::SplitView
                | CommandId::ResizeSplitLeft
                | CommandId::ResizeSplitRight
                | CommandId::ResizeSplitUp
                | CommandId::ResizeSplitDown
                | CommandId::FocusSplitDivider
        );
        let project_metadata_command =
            matches!(id, CommandId::RenameProject | CommandId::MoveProject);
        if (terminal_command
            || layout_command
            || project_metadata_command
            || id == CommandId::RemoveProject)
            && context.modal
        {
            return Err(FocusError::ModalActive.into());
        }
        let failed_terminal_close_pending = id == CommandId::CloseActiveTerminal
            && root.oldest_unresolved_terminal_close_failure().is_some();
        let targeted_new_terminal = matches!(
            parameters.as_ref(),
            Some(CommandParameters::NewTerminal {
                project_root,
                cwd,
                ..
            }) if project_root.is_some() || cwd.is_some()
        );
        if (terminal_command || layout_command)
            && !targeted_new_terminal
            && context.active_project.is_none()
            && !failed_terminal_close_pending
        {
            return Err(AppRootError::ProjectNotActive);
        }
        if id == CommandId::CloseActiveTerminal
            && !failed_terminal_close_pending
            && !matches!(
                (&context.focused_view, context.focused_terminal),
                (Some(ViewKey::Terminal { .. }), Some(_))
            )
        {
            return Err(AppRootError::MissingPanel);
        }
        if project_metadata_command {
            let CommandTarget::Project(project_id) = target else {
                return Err(AppRootError::UnexpectedParameters);
            };
            return match (id, parameters) {
                (CommandId::RenameProject, Some(CommandParameters::RenameProject { name })) => root
                    .rename_project_from_request(project_id, name, cx)
                    .map(|()| CommandOutcome::Completed),
                (CommandId::MoveProject, Some(CommandParameters::MoveProject { new_index })) => {
                    root.move_project_from_request(project_id, new_index, cx)
                        .map(|()| CommandOutcome::Completed)
                }
                _ => Err(AppRootError::UnexpectedParameters),
            };
        }
        if id == CommandId::RemoveProject {
            let CommandTarget::Project(project_id) = target else {
                return Err(AppRootError::UnexpectedParameters);
            };
            if parameters.is_some() {
                return Err(AppRootError::UnexpectedParameters);
            }
            return root
                .remove_project(project_id, window, cx)
                .map(|()| CommandOutcome::Completed);
        }
        if id == CommandId::OpenSettings {
            if !matches!(target, CommandTarget::Focused) || parameters.is_some() {
                return Err(AppRootError::UnexpectedParameters);
            }
            return root
                .open_settings_from_request(window, cx)
                .map(|()| CommandOutcome::Completed);
        }
        if matches!(
            id,
            CommandId::ActivateNavView
                | CommandId::ActivateExplorerView
                | CommandId::ActivateCommitView
                | CommandId::ActivateAgentFilesView
        ) {
            let CommandTarget::SideView(view) = target else {
                return Err(AppRootError::UnexpectedParameters);
            };
            if parameters.is_some() {
                return Err(AppRootError::UnexpectedParameters);
            }
            root.activate_side_view(view, window, cx)?;
            return Ok(CommandOutcome::Completed);
        }
        if id == CommandId::OpenLayoutMenu {
            if parameters.is_some() {
                return Err(AppRootError::UnexpectedParameters);
            }
            return root
                .open_layout_menu_from_request(window, cx)
                .map(|()| CommandOutcome::Completed);
        }
        if layout_command {
            let CommandTarget::Focused = target else {
                return Err(AppRootError::UnexpectedParameters);
            };
            return match (id, parameters) {
                (
                    CommandId::MoveView,
                    Some(CommandParameters::MoveView {
                        view,
                        target_group,
                        target_index,
                    }),
                ) => root
                    .move_view_from_request(view, target_group, target_index, window, cx)
                    .map(|()| CommandOutcome::Completed),
                (
                    CommandId::MergeGroups,
                    Some(CommandParameters::MergeGroups {
                        source_group,
                        target_group,
                    }),
                ) => root
                    .merge_groups_from_request(source_group, target_group, window, cx)
                    .map(|()| CommandOutcome::Completed),
                (
                    CommandId::SplitView,
                    Some(CommandParameters::SplitView {
                        view,
                        target_group,
                        direction,
                        ratio,
                        viewport,
                    }),
                ) => root
                    .split_view_from_request(
                        super::layout::SplitViewOptions {
                            view,
                            target_group,
                            direction,
                            ratio,
                            viewport,
                        },
                        window,
                        cx,
                    )
                    .map(|()| CommandOutcome::Completed),
                (
                    CommandId::ResizeSplitLeft
                    | CommandId::ResizeSplitRight
                    | CommandId::ResizeSplitUp
                    | CommandId::ResizeSplitDown,
                    Some(CommandParameters::ResizeSplit {
                        path,
                        ratio,
                        viewport,
                    }),
                ) => root
                    .resize_split_from_request(path, ratio, viewport, window, cx)
                    .map(|()| CommandOutcome::Completed),
                (
                    CommandId::FocusSplitDivider,
                    Some(CommandParameters::FocusSplitDivider { path }),
                ) => root
                    .focus_split_divider_from_request(path, window, cx)
                    .map(|()| CommandOutcome::Completed),
                _ => Err(AppRootError::UnexpectedParameters),
            };
        }
        if !terminal_command {
            return execute_sidebar_command(id, target, parameters, context, root, window, cx);
        }
        let CommandTarget::Focused = target else {
            return Err(AppRootError::UnexpectedParameters);
        };
        match (id, parameters) {
            (
                CommandId::NewTerminal,
                Some(CommandParameters::NewTerminal {
                    shell,
                    project_root,
                    cwd,
                }),
            ) => root
                .new_terminal_from_request(shell, project_root, cwd, window, cx)
                .map(CommandOutcome::TerminalPanel),
            (CommandId::SplitRight, parameters) => {
                let (ratio, viewport, shell) = split_options(parameters)?;
                root.split_terminal_from_request(
                    SplitDirection::LeftRight,
                    ratio,
                    viewport,
                    shell,
                    window,
                    cx,
                )
                .map(CommandOutcome::TerminalPanel)
            }
            (CommandId::SplitDown, parameters) => {
                let (ratio, viewport, shell) = split_options(parameters)?;
                root.split_terminal_from_request(
                    SplitDirection::TopBottom,
                    ratio,
                    viewport,
                    shell,
                    window,
                    cx,
                )
                .map(CommandOutcome::TerminalPanel)
            }
            (CommandId::CloseActiveTerminal, None) => root
                .close_focused_terminal_from_request(window, cx)
                .map(CommandOutcome::TerminalClosed),
            (CommandId::CloseActivePanel, None) => {
                let view = context
                    .focused_view
                    .clone()
                    .ok_or(AppRootError::MissingPanel)?;
                root.close_view_from_request(view, BTreeMap::new(), window, cx)?;
                Ok(CommandOutcome::Completed)
            }
            (CommandId::ReconnectTerminal, None) => root
                .reconnect_focused_terminal_from_request(window, cx)
                .map(|()| CommandOutcome::Completed),
            (CommandId::FocusNextPanel, None) => root
                .cycle_panel_from_request(super::navigation::PanelCycleDirection::Next, window, cx)
                .map(|()| CommandOutcome::Completed),
            (CommandId::FocusPreviousPanel, None) => root
                .cycle_panel_from_request(
                    super::navigation::PanelCycleDirection::Previous,
                    window,
                    cx,
                )
                .map(|()| CommandOutcome::Completed),
            (
                CommandId::CloseOtherViews,
                Some(CommandParameters::CloseGroup {
                    group_id,
                    document_decisions,
                }),
            ) => root
                .close_other_views_from_request(group_id, document_decisions, window, cx)
                .map(|()| CommandOutcome::Completed),
            (
                CommandId::CloseAllViews,
                Some(CommandParameters::CloseGroup {
                    group_id,
                    document_decisions,
                }),
            ) => root
                .close_all_views_from_request(group_id, document_decisions, window, cx)
                .map(|()| CommandOutcome::Completed),
            _ => Err(AppRootError::UnexpectedParameters),
        }
    }
}

fn execute_sidebar_command(
    id: CommandId,
    target: CommandTarget,
    parameters: Option<CommandParameters>,
    context: CommandContext,
    root: &mut AppRoot,
    window: &mut Window,
    cx: &mut Context<'_, AppRoot>,
) -> Result<CommandOutcome, AppRootError> {
    match (id, target, parameters) {
        (CommandId::ToggleSidebar, CommandTarget::Focused, None) => {
            root.toggle_sidebar(window, cx)?;
        }
        (CommandId::FocusActivityBar, CommandTarget::Focused, None) => {
            root.focus_activity_bar(window, cx)?;
        }
        (CommandId::FocusNextSideButton, CommandTarget::Focused, None) => {
            root.navigate_side_buttons(SideButtonNavigation::Next, window, cx)?;
        }
        (CommandId::FocusPreviousSideButton, CommandTarget::Focused, None) => {
            root.navigate_side_buttons(SideButtonNavigation::Previous, window, cx)?;
        }
        (CommandId::FocusFirstSideButton, CommandTarget::Focused, None) => {
            root.navigate_side_buttons(SideButtonNavigation::First, window, cx)?;
        }
        (CommandId::FocusLastSideButton, CommandTarget::Focused, None) => {
            root.navigate_side_buttons(SideButtonNavigation::Last, window, cx)?;
        }
        (CommandId::ActivateSideButton, target, None) => {
            let view = match target {
                CommandTarget::Focused => context.focused_side_button,
                CommandTarget::SideButton(view) => Some(view),
                _ => return Err(AppRootError::UnexpectedParameters),
            };
            if let Some(view) = view {
                root.click_side_view(view, window, cx)?;
            }
        }
        (CommandId::OpenSideButtonMenu, target, parameters) => {
            let view = match target {
                CommandTarget::Focused => context.focused_side_button,
                CommandTarget::SideButton(view) => Some(view),
                _ => return Err(AppRootError::UnexpectedParameters),
            };
            let position = match parameters {
                None => None,
                Some(CommandParameters::SideButtonMenuPosition(position)) => Some(position),
                _ => return Err(AppRootError::UnexpectedParameters),
            };
            if let Some(view) = view {
                root.open_side_button_menu(view, position, window, cx)?;
            }
        }
        (CommandId::AdjustSidebarWidthLeft, CommandTarget::SidebarWidthDivider, None) => {
            root.adjust_sidebar_width(-16.0, window, cx)?;
        }
        (CommandId::AdjustSidebarWidthRight, CommandTarget::SidebarWidthDivider, None) => {
            root.adjust_sidebar_width(16.0, window, cx)?;
        }
        (CommandId::AdjustSidebarWidthLeftLarge, CommandTarget::SidebarWidthDivider, None) => {
            root.adjust_sidebar_width(-64.0, window, cx)?;
        }
        (CommandId::AdjustSidebarWidthRightLarge, CommandTarget::SidebarWidthDivider, None) => {
            root.adjust_sidebar_width(64.0, window, cx)?;
        }
        (CommandId::ResetSidebarWidth, CommandTarget::SidebarWidthDivider, None) => {
            root.change_sidebar_geometry(SidebarGeometryCommand::ResetWidth, cx)?;
        }
        (CommandId::AdjustSidebarRatioUp, CommandTarget::SidebarRatioDivider, None) => {
            root.adjust_sidebar_ratio(-0.02, window, cx)?;
        }
        (CommandId::AdjustSidebarRatioDown, CommandTarget::SidebarRatioDivider, None) => {
            root.adjust_sidebar_ratio(0.02, window, cx)?;
        }
        (CommandId::AdjustSidebarRatioUpLarge, CommandTarget::SidebarRatioDivider, None) => {
            root.adjust_sidebar_ratio(-0.1, window, cx)?;
        }
        (CommandId::AdjustSidebarRatioDownLarge, CommandTarget::SidebarRatioDivider, None) => {
            root.adjust_sidebar_ratio(0.1, window, cx)?;
        }
        (CommandId::ResetSidebarRatio, CommandTarget::SidebarRatioDivider, None) => {
            root.change_sidebar_geometry(SidebarGeometryCommand::ResetRatio, cx)?;
        }
        (
            CommandId::ReorderSideButton,
            CommandTarget::SideButton(view),
            Some(CommandParameters::SideButtonMovement(movement)),
        ) => {
            root.organize_side_button(view, SideButtonCommand::Reorder(movement), window, cx)?;
        }
        (CommandId::MoveSideButtonToOtherSlot, CommandTarget::SideButton(view), None) => {
            root.organize_side_button(view, SideButtonCommand::MoveToOtherSlot, window, cx)?;
        }
        (CommandId::HideSideButton, CommandTarget::SideButton(view), None) => {
            root.organize_side_button(view, SideButtonCommand::Hide, window, cx)?;
        }
        (CommandId::ResetSideButtonOrder, CommandTarget::SideButton(view), None) => {
            root.organize_side_button(view, SideButtonCommand::ResetOrder, window, cx)?;
        }
        _ => return Err(AppRootError::UnexpectedParameters),
    }
    Ok(CommandOutcome::Completed)
}

fn split_options(
    parameters: Option<CommandParameters>,
) -> Result<(f32, LogicalSize, CanonicalPath), AppRootError> {
    match parameters {
        Some(CommandParameters::Split {
            ratio,
            viewport,
            shell,
        }) => Ok((ratio, viewport, shell)),
        _ => Err(AppRootError::UnexpectedParameters),
    }
}

impl AppRoot {
    pub(crate) fn execute_command(
        &mut self,
        id: CommandId,
        source: CommandSource,
        target: CommandTarget,
        parameters: Option<CommandParameters>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<CommandOutcome, AppRootError> {
        let active_project = self.snapshot.registry().active();
        let focused_view = active_project.and_then(|project_id| {
            let panel_id = self.focused_panel(project_id, cx)?;
            self.projects
                .get(&project_id)?
                .read(cx)
                .panels
                .get(&panel_id)
                .map(|panel| panel.view_key.clone())
        });
        let focused_terminal = match &focused_view {
            Some(ViewKey::Terminal { terminal_id, .. }) => Some(*terminal_id),
            _ => None,
        };
        let context = CommandContext {
            active_project,
            focused_view,
            focused_terminal,
            focused_side_button: self.focused_side_button(window, cx),
            modal: self.focus_manager.modal_is_active(),
        };
        CommandRegistry::execute(
            CommandRequest {
                id,
                source,
                target,
                context,
                parameters,
            },
            self,
            window,
            cx,
        )
    }
}

#[cfg(test)]
mod command_registry_tests {
    use super::*;

    /// 命令目录不得占用 Ctrl+C 或 Ctrl+Shift+C：中断直通 PTY，复制归终端视图。
    #[test]
    fn command_registry_reserved_terminal_interrupt() {
        for definition in APPLICATION_DEFINITIONS
            .iter()
            .chain(TERMINAL_DEFINITIONS.iter())
            .chain(PANEL_DEFINITIONS.iter())
            .chain(LAYOUT_DEFINITIONS.iter())
            .chain(SIDEBAR_DEFINITIONS.iter())
        {
            let keystroke = definition.keystroke.to_ascii_lowercase();
            assert_ne!(keystroke, "ctrl-c", "命令目录不得占用终端中断键");
            assert_ne!(
                keystroke, "ctrl-shift-c",
                "复制语义归终端视图，不得进入命令目录"
            );
            assert_ne!(
                keystroke, "ctrl-shift-a",
                "全选语义归终端视图，不得进入命令目录"
            );
        }
        assert!(!TERMINAL_DEFINITIONS.is_empty());
    }
}
