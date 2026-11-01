//! 应用命令目录：请求、上下文快照与统一执行入口。

use std::collections::BTreeSet;

use gpui::{
    Action, App, Context, DummyKeyboardMapper, KeyBinding, KeyBindingContextPredicate, Window,
};
use slterm_path::CanonicalPath;
use slterm_split::SplitDirection;
use slterm_workspace::{LogicalSize, PanelId, ProjectId, TerminalId, TerminalRecord, ViewKey};

use super::{
    AppRoot, CloseActiveTerminal, NewTerminal, SplitDown, SplitRight, commands::AppRootError,
    focus_manager::FocusError,
};

/// 当前已接入统一目录的终端命令。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CommandId {
    NewTerminal,
    SplitRight,
    SplitDown,
    CloseActiveTerminal,
}

/// 命令来源只用于表达入口，不改变领域规则。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandSource {
    Keyboard,
    Application,
}

/// 当前终端命令都作用于执行时的聚焦上下文。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandTarget {
    Focused,
}

/// 执行前冻结的上下文；不作为第二份布局或焦点事实源。
#[derive(Clone, Debug)]
pub(crate) struct CommandContext {
    pub(crate) active_project: Option<ProjectId>,
    pub(crate) focused_view: Option<ViewKey>,
    pub(crate) focused_terminal: Option<TerminalId>,
    pub(crate) modal: bool,
}

/// 可选命令参数；无参数命令必须拒绝非空载荷。
#[derive(Clone, Debug)]
pub(crate) enum CommandParameters {
    NewTerminal {
        shell: CanonicalPath,
    },
    Split {
        ratio: f32,
        viewport: LogicalSize,
        shell: CanonicalPath,
    },
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
}

pub(crate) struct CommandRegistry;

/// 终端命令的默认绑定与 GPUI action 映射只在目录登记一次。
pub(crate) struct CommandDefinition {
    pub(crate) id: CommandId,
    pub(crate) keystroke: &'static str,
    pub(crate) context: Option<&'static str>,
    action: fn() -> Box<dyn Action>,
}

const TERMINAL_DEFINITIONS: [CommandDefinition; 4] = [
    CommandDefinition {
        id: CommandId::NewTerminal,
        keystroke: "ctrl-shift-t",
        context: None,
        action: || Box::new(NewTerminal),
    },
    CommandDefinition {
        id: CommandId::SplitRight,
        keystroke: "ctrl-shift-d",
        context: None,
        action: || Box::new(SplitRight),
    },
    CommandDefinition {
        id: CommandId::SplitDown,
        keystroke: "ctrl-shift-s",
        context: None,
        action: || Box::new(SplitDown),
    },
    CommandDefinition {
        id: CommandId::CloseActiveTerminal,
        keystroke: "ctrl-shift-w",
        context: None,
        action: || Box::new(CloseActiveTerminal),
    },
];

impl CommandRegistry {
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn terminal_definitions() -> &'static [CommandDefinition] {
        &TERMINAL_DEFINITIONS
    }

    pub(crate) fn register_terminal_bindings(cx: &mut App) {
        let mut ids = BTreeSet::new();
        for definition in &TERMINAL_DEFINITIONS {
            assert!(ids.insert(definition.id), "终端命令 ID 不得重复");
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
        let CommandRequest {
            id,
            source,
            target,
            context,
            parameters,
        } = request;
        debug_assert!(matches!(
            source,
            CommandSource::Keyboard | CommandSource::Application
        ));
        if context.modal {
            return Err(FocusError::ModalActive.into());
        }
        if context.active_project.is_none() {
            return Err(AppRootError::ProjectNotActive);
        }
        if id == CommandId::CloseActiveTerminal
            && !matches!(
                (&context.focused_view, context.focused_terminal),
                (Some(ViewKey::Terminal { .. }), Some(_))
            )
        {
            return Err(AppRootError::MissingPanel);
        }
        let CommandTarget::Focused = target;
        match (id, parameters) {
            (CommandId::NewTerminal, Some(CommandParameters::NewTerminal { shell })) => root
                .new_terminal_from_request(shell, window, cx)
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
    pub(crate) fn execute_terminal_command(
        &mut self,
        id: CommandId,
        source: CommandSource,
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
            modal: self.focus_manager.modal_is_active(),
        };
        CommandRegistry::execute(
            CommandRequest {
                id,
                source,
                target: CommandTarget::Focused,
                context,
                parameters,
            },
            self,
            window,
            cx,
        )
    }
}
