//! 应用命令目录：请求、上下文快照与统一执行入口。

use std::collections::BTreeSet;

use gpui::{
    Action, App, Context, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Pixels,
    Point, Window,
};
use slterm_path::CanonicalPath;
use slterm_sidebar::ButtonMovement;
use slterm_split::SplitDirection;
use slterm_workspace::{
    LogicalSize, PanelId, ProjectId, SideViewId, TerminalId, TerminalRecord, ViewKey,
};

use super::{
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, ActivateSideButton,
    AdjustSidebarRatioDown, AdjustSidebarRatioDownLarge, AdjustSidebarRatioUp,
    AdjustSidebarRatioUpLarge, AdjustSidebarWidthLeft, AdjustSidebarWidthLeftLarge,
    AdjustSidebarWidthRight, AdjustSidebarWidthRightLarge, AppRoot, CloseActiveTerminal,
    FocusActivityBar, FocusFirstSideButton, FocusLastSideButton, FocusNextSideButton,
    FocusPreviousSideButton, NewTerminal, OpenSideButtonMenu, ResetSidebarRatio, ResetSidebarWidth,
    SplitDown, SplitRight, ToggleSidebar,
    commands::AppRootError,
    focus_manager::FocusError,
    sidebar::{SideButtonCommand, SideButtonNavigation, SidebarGeometryCommand},
};

/// 统一目录中的封闭命令。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CommandId {
    NewTerminal,
    SplitRight,
    SplitDown,
    CloseActiveTerminal,
    RemoveProject,
    ActivateSideView,
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

const TERMINAL_DEFINITIONS: [CommandDefinition; 4] = [
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
        id: CommandId::ActivateSideView,
        target: CommandTarget::SideView(SideViewId::Nav),
        keystroke: "ctrl-shift-n",
        context: None,
        action: || Box::new(ActivateNav),
    },
    CommandDefinition {
        id: CommandId::ActivateSideView,
        target: CommandTarget::SideView(SideViewId::Explorer),
        keystroke: "ctrl-shift-e",
        context: None,
        action: || Box::new(ActivateExplorer),
    },
    CommandDefinition {
        id: CommandId::ActivateSideView,
        target: CommandTarget::SideView(SideViewId::Commit),
        keystroke: "ctrl-shift-g",
        context: None,
        action: || Box::new(ActivateCommit),
    },
    CommandDefinition {
        id: CommandId::ActivateSideView,
        target: CommandTarget::SideView(SideViewId::AgentFiles),
        keystroke: "ctrl-shift-a",
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

impl CommandRegistry {
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn terminal_definitions() -> &'static [CommandDefinition] {
        &TERMINAL_DEFINITIONS
    }

    pub(crate) fn register_bindings(cx: &mut App) {
        let mut definitions = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        for definition in TERMINAL_DEFINITIONS
            .iter()
            .chain(SIDEBAR_DEFINITIONS.iter())
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
        if root.close_in_progress {
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
        );
        if (terminal_command || id == CommandId::RemoveProject) && context.modal {
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
        if terminal_command
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
        if id == CommandId::ActivateSideView {
            let CommandTarget::SideView(view) = target else {
                return Err(AppRootError::UnexpectedParameters);
            };
            if parameters.is_some() {
                return Err(AppRootError::UnexpectedParameters);
            }
            root.activate_side_view(view, window, cx)?;
            return Ok(CommandOutcome::Completed);
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
