//! 壳事件类型与有界投递 helper。
//!
//! 事件只携带稳定身份与一次性回执；领域消费者在 AppRoot 事件泵中实现。
//! 所有投递都是非阻塞 `try_send`，队列满或断连映射结构化错误。

use std::sync::mpsc;

use slterm_runtime_protocol::{
    DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError, JsonRpcErrorData,
    SnapshotResources,
};

use crate::gpui_terminal::PaneId;

/// 壳跨线程唤醒事件；生产者与消费归各领域模块。
pub(crate) enum SltermShellEvent {
    /// 消费(右键「在此处打开」/on_open_urls 落盘)归窗口/平台域;本模块只登记发送端。
    #[allow(dead_code)] // 打开目录载荷由平台消费，壳事件本身只负责转发。
    OpenDirectories(Vec<String>),
    /// toast 点击回窗(统一走窗口 facade)。
    /// 托盘左键/菜单「显示」同样走这条单窗回窗语义。
    NotificationFocus(Option<PaneId>),
    /// 权限确认 toast 的动作按钮回传(只携回窗目标;不回写权限决定)。
    NotificationChoice { pane_id: PaneId },
    /// 托盘「退出」:与关窗同一条退出链(终局快照 + 退出)。
    TrayQuit,
    /// 系统关机/注销守卫投递：复用与托盘退出相同的统一关闭链。
    SessionEndRequested,
    /// 工作区状态后台写失败；不携带路径、SQL 或状态正文。
    WorkspaceWriteFailed,
    /// 更新检查结果；只在前台决定提示、下载或跳过。
    UpdateAvailable(crate::update_check::UpdateCheckResult),
    /// Hook 安装自愈失败；只携带封闭类别，前台按语言呈现。
    AiHookAlert(crate::ai_hook::HookFailureKind),
    /// 命名管道 hook 事件；前台按 pane 所有权门路由到 AgentHub。
    AiHookEvents(Vec<crate::ai_hook::AiHookEvent>),
    /// 启动时副本恢复或双损隔离已完成；只携带封闭通知类型。
    WorkspaceRecovered(crate::persistence::startup::WorkspaceRecoveryNotice),
    /// 第二实例意图与一次性完成回执；AppRoot 前台消费，真实完成后答复。
    Handover {
        params: HandoverParams,
        completion: mpsc::SyncSender<Result<HandoverResponse, JsonRpcError>>,
    },
    /// Runtime 快照请求与一次性回执；资源只在前台同一时刻读取。
    RuntimeSnapshot {
        completion: mpsc::SyncSender<Result<SnapshotResources, JsonRpcError>>,
    },
    /// Runtime 写意图与一次性回执；AppRoot 复用既有命令执行。
    ///
    /// Git 破坏写只在前台解析目标与授权，阻塞读写由回执接收方在后台执行。
    RuntimeMutation {
        mutation: crate::local_control::RuntimeMutation,
        completion: mpsc::SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
    },
    /// Runtime 终端读取目标与一次性回执；回执携带 worker 文本快照接收端。
    RuntimeRead {
        terminal_id: slterm_workspace::TerminalId,
        generation: u64,
        /// `Some` 表示 agent.read，必须再匹配该终端当前对外身份。
        agent_id: Option<String>,
        completion: mpsc::SyncSender<
            Result<mpsc::Receiver<slterm_terminal::term::TextSnapshot>, JsonRpcError>,
        >,
    },
    /// Runtime 终端等待状态查询与一次性回执；回执返回最近帧 revision。
    RuntimeWait {
        terminal_id: slterm_workspace::TerminalId,
        generation: u64,
        /// `Some` 表示 agent.wait，必须再匹配该终端当前对外身份。
        agent_id: Option<String>,
        completion: mpsc::SyncSender<Result<u64, JsonRpcError>>,
    },
    /// Runtime Agent 资源请求与一次性回执；资源只在前台同一时刻读取。
    RuntimeAgents {
        completion:
            mpsc::SyncSender<Result<Vec<slterm_runtime_protocol::AgentResource>, JsonRpcError>>,
    },
    /// Runtime Git 状态目标请求与一次性回执；仓库读取由后台执行体完成。
    RuntimeGitStatus {
        project_id: Option<slterm_workspace::ProjectId>,
        completion: mpsc::SyncSender<Result<RuntimeGitTarget, JsonRpcError>>,
    },
    /// Runtime Git diff 目标请求与一次性回执；路径与仓库读取分离。
    RuntimeGitDiff {
        project_id: Option<slterm_workspace::ProjectId>,
        path: String,
        completion: mpsc::SyncSender<Result<RuntimeGitDiffTarget, JsonRpcError>>,
    },
    /// Runtime Git 打开请求与一次性回执；仓库绑定已在后台捕获，前台只执行打开链。
    RuntimeGitOpen {
        project_id: slterm_workspace::ProjectId,
        /// 绑定体积较大；事件走有界队列，用 Box 保持枚举紧凑。
        binding: Box<slterm_document::RepositoryBinding>,
        path: slterm_path::CanonicalPath,
        kind: RuntimeGitOpenKind,
        completion: mpsc::SyncSender<Result<(), JsonRpcError>>,
    },
    /// Git 写已完成后通知前台刷新对应项目的视图；尽力投递，不携带结果。
    RuntimeGitRefresh {
        project_id: slterm_workspace::ProjectId,
    },
}

/// Runtime Git 读取目标：前台只解析项目与仓库入口，阻塞读取归后台。
pub(crate) struct RuntimeGitTarget {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) root: slterm_path::CanonicalPath,
    pub(crate) git: std::sync::Arc<crate::git::GitFacade>,
}

/// Runtime Git diff 目标：仓库入口与已校验的仓库相对路径。
pub(crate) struct RuntimeGitDiffTarget {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) root: slterm_path::CanonicalPath,
    pub(crate) path: String,
    pub(crate) git: std::sync::Arc<crate::git::GitFacade>,
}

/// 写意图在前台的处理结果：当场完成，或只解析了 Git 写/打开目标。
pub(crate) enum RuntimeMutationReply {
    /// 前台已完成全部副作用或已给出结构化失败。
    Done(Result<(), JsonRpcError>),
    /// 前台只解析项目与授权；阻塞读写由调用方在后台执行。
    Git(GitWriteTarget),
    /// 前台只解析项目与仓库入口；绑定捕获与打开链由调用方在后台发起。
    GitOpen(GitOpenTarget),
}

/// Git 打开目标：项目根只用于状态与仓库解析，不授予额外写能力。
pub(crate) struct GitOpenTarget {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) root: slterm_path::CanonicalPath,
    pub(crate) git: std::sync::Arc<crate::git::GitFacade>,
}

/// Runtime Git 打开方式：与 Commit 行激活的打开映射一一对应。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeGitOpenKind {
    Diff,
    Deleted,
}

/// Git 破坏写目标：项目根是唯一写授权来源；阻塞执行归后台。
pub(crate) struct GitWriteTarget {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) root: slterm_path::CanonicalPath,
    pub(crate) git: std::sync::Arc<crate::git::GitFacade>,
    pub(crate) file_operations:
        std::sync::Arc<crate::app_shell::file_operation_service::FileOperationService>,
    /// 前台时刻存在未保存修改的 Document；Runtime 无法交互，必须拒绝写。
    pub(crate) dirty_documents: Vec<slterm_path::CanonicalPath>,
}

pub(crate) const SHELL_EVENT_CAPACITY: usize = 64;

pub(crate) fn shell_event_channel() -> (
    mpsc::SyncSender<SltermShellEvent>,
    mpsc::Receiver<SltermShellEvent>,
) {
    mpsc::sync_channel(SHELL_EVENT_CAPACITY)
}

pub(crate) fn try_send_shell_event(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    event: SltermShellEvent,
    queue: &'static str,
) {
    match sender.try_send(event) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(_)) => log::warn!("{queue} event queue full"),
        Err(mpsc::TrySendError::Disconnected(_)) => log::debug!("{queue} event channel closed"),
    }
}

pub(crate) fn handover_error(
    domain: DomainErrorCode,
    numeric: i32,
    message: &'static str,
) -> JsonRpcError {
    control_error(domain, numeric, message)
}

/// 控制面结构化错误构造；handover 与 Runtime 共用同一数值/领域码映射。
pub(crate) fn control_error(
    domain: DomainErrorCode,
    numeric: i32,
    message: &'static str,
) -> JsonRpcError {
    JsonRpcError::new(numeric, message, Some(JsonRpcErrorData { code: domain }))
}

/// 非阻塞投递第二实例意图；调用方只在后台执行体等待完成回执。
pub(crate) fn try_send_handover(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    params: HandoverParams,
) -> Result<mpsc::Receiver<Result<HandoverResponse, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::Handover { params, completion }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => {
            log::warn!("handover event queue full");
            Err(handover_error(
                DomainErrorCode::QueueFull,
                -32003,
                "handover queue full",
            ))
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {
            log::debug!("handover event channel closed");
            Err(handover_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime 快照请求；调用方只在后台执行体等待完成回执。
pub(crate) fn try_send_runtime_snapshot(
    sender: &mpsc::SyncSender<SltermShellEvent>,
) -> Result<mpsc::Receiver<Result<SnapshotResources, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeSnapshot { completion }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "snapshot queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime 写意图；调用方只在后台执行体等待完成回执。
pub(crate) fn try_send_runtime_mutation(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    mutation: crate::local_control::RuntimeMutation,
) -> Result<mpsc::Receiver<Result<RuntimeMutationReply, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeMutation {
        mutation,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "mutation queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime 终端读取请求；调用方只在后台执行体等待 worker 回执。
pub(crate) fn try_send_runtime_read(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    terminal_id: slterm_workspace::TerminalId,
    generation: u64,
    agent_id: Option<String>,
) -> Result<
    mpsc::Receiver<Result<mpsc::Receiver<slterm_terminal::term::TextSnapshot>, JsonRpcError>>,
    JsonRpcError,
> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeRead {
        terminal_id,
        generation,
        agent_id,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "read queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime 终端等待状态查询；调用方只在后台执行体等待回执。
pub(crate) fn try_send_runtime_wait(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    terminal_id: slterm_workspace::TerminalId,
    generation: u64,
    agent_id: Option<String>,
) -> Result<mpsc::Receiver<Result<u64, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeWait {
        terminal_id,
        generation,
        agent_id,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "wait queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime Agent 资源请求；调用方只在后台执行体等待回执。
pub(crate) fn try_send_runtime_agents(
    sender: &mpsc::SyncSender<SltermShellEvent>,
) -> Result<
    mpsc::Receiver<Result<Vec<slterm_runtime_protocol::AgentResource>, JsonRpcError>>,
    JsonRpcError,
> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeAgents { completion }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "agent list queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime Git 状态目标请求；调用方只在后台执行体等待回执。
pub(crate) fn try_send_runtime_git_status(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    project_id: Option<slterm_workspace::ProjectId>,
) -> Result<mpsc::Receiver<Result<RuntimeGitTarget, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeGitStatus {
        project_id,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "git status queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime Git diff 目标请求；调用方只在后台执行体等待回执。
pub(crate) fn try_send_runtime_git_diff(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    project_id: Option<slterm_workspace::ProjectId>,
    path: String,
) -> Result<mpsc::Receiver<Result<RuntimeGitDiffTarget, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeGitDiff {
        project_id,
        path,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "git diff queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 非阻塞投递 Runtime Git 打开请求；调用方在后台执行体等待打开回执。
pub(crate) fn try_send_runtime_git_open(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    project_id: slterm_workspace::ProjectId,
    binding: slterm_document::RepositoryBinding,
    path: slterm_path::CanonicalPath,
    kind: RuntimeGitOpenKind,
) -> Result<mpsc::Receiver<Result<(), JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::RuntimeGitOpen {
        project_id,
        binding: Box::new(binding),
        path,
        kind,
        completion,
    }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => Err(control_error(
            DomainErrorCode::QueueFull,
            -32003,
            "git open queue full",
        )),
        Err(mpsc::TrySendError::Disconnected(_)) => {
            Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
}

/// 尽力通知前台刷新 Git 视图；队列满或通道关闭都不阻断已完成的写。
pub(crate) fn try_send_runtime_git_refresh(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    project_id: slterm_workspace::ProjectId,
) {
    try_send_shell_event(
        sender,
        SltermShellEvent::RuntimeGitRefresh { project_id },
        "git refresh",
    );
}
