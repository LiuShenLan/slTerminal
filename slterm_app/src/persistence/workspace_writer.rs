//! `workspace.db` 的专用写线程、ViewState 与项目聚合去抖调度。
//!
//! 前台只提交不可变状态；SQLite 连接、失败待写副本与退出冲刷均由写线程独占。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use slterm_path::AppPaths;
use slterm_sidebar::SidebarState;
use slterm_workspace::{
    CloseScope, ProjectId, ProjectSnapshot, SideViewId, ViewKey, ViewKeyError, ViewState,
    ViewStateError, ViewStateRegistry,
};

use super::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent, ProjectCleanupError,
};
use super::database::DatabaseError;
use super::metadata::MetadataError;
use super::project::ProjectStoreError;
use super::sidebar::SidebarStateStoreError;
use super::startup::{MaintenanceError, MaintenanceStage};
use super::view_state::{ViewStateStoreError, remove_project_from_side_view_state};
use super::workspace::WorkspaceDatabase;

const VIEW_STATE_DEBOUNCE: Duration = Duration::from_millis(500);
const SIDEBAR_STATE_DEBOUNCE: Duration = Duration::from_millis(500);
const PROJECT_SNAPSHOT_DEBOUNCE: Duration = Duration::from_secs(2);
const COMMAND_CAPACITY: usize = 64;

/// 一次显式或退出前冲刷的实际写入结果。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkspaceFlushReport {
    pub(crate) written: usize,
    pub(crate) failed: usize,
}

/// 后台工作区状态写失败；分类信息不携带路径、SQL 或状态正文。
pub(crate) enum WorkspaceWriteFailure {
    ViewState(ViewStateWriteFailure),
    SidebarState(SidebarStateStoreError),
    ProjectSnapshot(ProjectStoreError),
}

/// 后台 ViewState 写失败；保留键与分类错误，不携带状态正文。
pub(crate) struct ViewStateWriteFailure {
    key: ViewKey,
    error: ViewStateStoreError,
}

impl ViewStateWriteFailure {
    pub(crate) const fn key(&self) -> &ViewKey {
        &self.key
    }

    pub(crate) const fn error(&self) -> &ViewStateStoreError {
        &self.error
    }
}

/// 写线程请求或启动失败；展示文本不包含 SQL、路径或状态内容。
#[derive(Debug)]
pub(crate) enum WorkspaceWriterError {
    Open(DatabaseError),
    Spawn(std::io::Error),
    Metadata(MetadataError),
    ProcessCreated(std::io::Error),
    MaintenanceNeedsAttention(MaintenanceStage),
    Maintenance(MaintenanceError),
    ViewKey(ViewKeyError),
    ViewState(ViewStateError),
    QueueFull,
    FlushFailed,
    Stopped,
    ControlPlaneCleanupFailed,
    FinalCommitDecisionFailed,
}

impl fmt::Display for WorkspaceWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(_) => formatter.write_str("工作区数据库打开失败"),
            Self::Spawn(_) => formatter.write_str("工作区写线程启动失败"),
            Self::Metadata(_) => formatter.write_str("工作区运行标记提交失败"),
            Self::ProcessCreated(_) => formatter.write_str("进程创建时间读取失败"),
            Self::MaintenanceNeedsAttention(stage) => {
                let stage = match stage {
                    MaintenanceStage::Journal => "Journal",
                    MaintenanceStage::MaintenanceRequests => "维护请求／恢复点",
                    MaintenanceStage::CloseIntents => "关闭意图",
                    MaintenanceStage::Snapshots => "快照索引",
                    MaintenanceStage::HistoryQuarantine => "历史隔离",
                };
                write!(formatter, "工作区启动维护需要人工处理：{stage}")
            }
            Self::Maintenance(_) => formatter.write_str("工作区启动维护执行失败"),
            Self::ViewKey(_) => formatter.write_str("ViewState 的 ViewKey 编码失败"),
            Self::ViewState(error) => fmt::Display::fmt(error, formatter),
            Self::QueueFull => formatter.write_str("工作区写队列已满"),
            Self::FlushFailed => formatter.write_str("工作区状态最终冲刷失败"),
            Self::Stopped => formatter.write_str("工作区写线程已停止"),
            Self::ControlPlaneCleanupFailed => {
                formatter.write_str("控制面清理失败，保持未正常退出")
            }
            Self::FinalCommitDecisionFailed => {
                formatter.write_str("最终退出确认失败，保持未正常退出")
            }
        }
    }
}

impl std::error::Error for WorkspaceWriterError {}

enum Command {
    SaveViewState {
        key_json: String,
        key: ViewKey,
        state: Box<ViewState>,
    },
    SaveProjectSnapshot {
        snapshot: Box<ProjectSnapshot>,
    },
    CommitCloseIntent {
        snapshot: Box<ProjectSnapshot>,
        registration: Box<PersistedCloseIntent>,
        response: SyncSender<Result<PersistedCloseIntent, CloseCommitError>>,
    },
    CommitProjectCloseIntent {
        snapshot: Box<ProjectSnapshot>,
        registration: Box<PersistedCloseIntent>,
        response: SyncSender<Result<PersistedCloseIntent, CloseCommitError>>,
    },
    CleanupProjectClose {
        current: Box<PersistedCloseIntent>,
        registry: Arc<ViewStateRegistry>,
        response: SyncSender<Result<PersistedCloseIntent, ProjectCleanupError>>,
    },
    AdvanceCloseIntent {
        current: Box<PersistedCloseIntent>,
        next: CloseCommitStatus,
        response: SyncSender<Result<PersistedCloseIntent, CloseStoreError>>,
    },
    SaveSidebarState {
        state: Box<SidebarState>,
    },
    Flush {
        response: SyncSender<WorkspaceFlushReport>,
    },
    Shutdown {
        response: SyncSender<WorkspaceFlushReport>,
    },
}

/// UI 持有的非阻塞写入口；生命周期与退出冲刷由外部 owner 独占。
#[derive(Clone, Debug)]
pub(crate) struct WorkspaceWriterClient {
    commands: SyncSender<Command>,
    /// 实体退出时移交的最终状态；Flush 或 owner shutdown 时由后台排空。
    final_states: SyncSender<(ViewKey, ViewState)>,
    /// 应用级侧栏只有一份待终态，独立于普通命令队列。
    final_sidebar_state: SyncSender<SidebarState>,
    /// 项目聚合只有一份待终态，独立于普通命令队列。
    final_project_snapshot: SyncSender<ProjectSnapshot>,
}

impl WorkspaceWriterClient {
    /// 非阻塞提交最新 ViewState；调用方保留自己的内存状态，队列满时可稍后重试。
    pub(crate) fn save_view_state(
        &self,
        key: &ViewKey,
        state: &ViewState,
    ) -> Result<(), WorkspaceWriterError> {
        let command = view_state_command(key, state)?;
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }

    /// 非阻塞提交最新项目聚合快照；同一时刻只保留一个待写版本。
    pub(crate) fn save_project_snapshot(
        &self,
        snapshot: &ProjectSnapshot,
    ) -> Result<(), WorkspaceWriterError> {
        self.commands
            .try_send(Command::SaveProjectSnapshot {
                snapshot: Box::new(snapshot.clone()),
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }

    /// 非阻塞提交项目快照与关闭意图的原子组合事务，实际结果只经独立回执返回。
    pub(crate) fn commit_close_intent(
        &self,
        snapshot: &ProjectSnapshot,
        registration: &PersistedCloseIntent,
    ) -> Result<Receiver<Result<PersistedCloseIntent, CloseCommitError>>, WorkspaceWriterError>
    {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::CommitCloseIntent {
                snapshot: Box::new(snapshot.clone()),
                registration: Box::new(registration.clone()),
                response,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })?;
        Ok(result)
    }

    /// 非阻塞提交项目关闭初始事务，实际结果只经独立回执返回。
    pub(crate) fn commit_project_close_intent(
        &self,
        snapshot: &ProjectSnapshot,
        registration: &PersistedCloseIntent,
    ) -> Result<Receiver<Result<PersistedCloseIntent, CloseCommitError>>, WorkspaceWriterError>
    {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::CommitProjectCloseIntent {
                snapshot: Box::new(snapshot.clone()),
                registration: Box::new(registration.clone()),
                response,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })?;
        Ok(result)
    }

    /// 非阻塞清理项目关闭事实；ViewState schema 注册表随命令移交唯一写线程。
    pub(crate) fn cleanup_project_close(
        &self,
        current: &PersistedCloseIntent,
        registry: Arc<ViewStateRegistry>,
    ) -> Result<Receiver<Result<PersistedCloseIntent, ProjectCleanupError>>, WorkspaceWriterError>
    {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::CleanupProjectClose {
                current: Box::new(current.clone()),
                registry,
                response,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })?;
        Ok(result)
    }

    /// 非阻塞推进关闭头状态；旧状态、指纹与当前世代由唯一写线程复核。
    pub(crate) fn advance_close_intent(
        &self,
        current: &PersistedCloseIntent,
        next: CloseCommitStatus,
    ) -> Result<Receiver<Result<PersistedCloseIntent, CloseStoreError>>, WorkspaceWriterError> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::AdvanceCloseIntent {
                current: Box::new(current.clone()),
                next,
                response,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })?;
        Ok(result)
    }

    /// 非阻塞提交最新侧栏组织状态；同一时刻只保留一个待写版本。
    pub(crate) fn save_sidebar_state(
        &self,
        state: &SidebarState,
    ) -> Result<(), WorkspaceWriterError> {
        self.commands
            .try_send(Command::SaveSidebarState {
                state: Box::new(state.clone()),
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }

    /// 非阻塞移交项目快照最终状态；只由 AppRoot 在退出时调用。
    pub(crate) fn save_final_project_snapshot(
        &self,
        snapshot: &ProjectSnapshot,
    ) -> Result<(), WorkspaceWriterError> {
        self.final_project_snapshot
            .try_send(snapshot.clone())
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }

    /// 非阻塞请求一次实际冲刷；结果由后台线程等待，失败可重试。
    pub(crate) fn flush(&self) -> Result<Receiver<WorkspaceFlushReport>, WorkspaceWriterError> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(Command::Flush { response })
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })?;
        Ok(result)
    }

    /// 非阻塞移交侧栏最终组织状态；只由持久化 owner 在退出时调用。
    pub(crate) fn save_final_sidebar_state(
        &self,
        state: &SidebarState,
    ) -> Result<(), WorkspaceWriterError> {
        self.final_sidebar_state
            .try_send(state.clone())
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }

    /// 非阻塞移交退出终态；正常命令队列满不丢弃这份最后恢复事实。
    pub(crate) fn save_final_view_state(
        &self,
        key: &ViewKey,
        state: &ViewState,
    ) -> Result<(), WorkspaceWriterError> {
        // 终态无需去抖键，但仍执行同一身份与作用域校验。
        let _ = view_state_command(key, state)?;
        self.final_states
            .try_send((key.clone(), state.clone()))
            .map_err(|error| match error {
                TrySendError::Full(_) => WorkspaceWriterError::QueueFull,
                TrySendError::Disconnected(_) => WorkspaceWriterError::Stopped,
            })
    }
}

fn view_state_command(key: &ViewKey, state: &ViewState) -> Result<Command, WorkspaceWriterError> {
    let key_json = key
        .to_canonical_json()
        .map_err(WorkspaceWriterError::ViewKey)?;
    state.scope(key).map_err(WorkspaceWriterError::ViewState)?;
    Ok(Command::SaveViewState {
        key_json,
        key: key.clone(),
        state: Box::new(state.clone()),
    })
}

/// 工作区库唯一写线程句柄；显式 shutdown 与 Drop 共用退出冲刷路径。
pub(crate) struct WorkspaceWriter {
    commands: SyncSender<Command>,
    final_state_sender: SyncSender<(ViewKey, ViewState)>,
    final_sidebar_sender: SyncSender<SidebarState>,
    final_project_sender: SyncSender<ProjectSnapshot>,
    failures: Option<Receiver<WorkspaceWriteFailure>>,
    worker: Option<JoinHandle<()>>,
    #[cfg(test)]
    join_completion_probe: Option<SyncSender<()>>,
}

impl WorkspaceWriter {
    /// 在调用线程等待数据库打开与线程启动；生产只由启动协调器后台调用。
    pub(crate) fn start(paths: AppPaths) -> Result<Self, WorkspaceWriterError> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (final_states, final_state_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (final_sidebar_state, final_sidebar_receiver) = mpsc::sync_channel(1);
        let (final_project_snapshot, final_project_receiver) = mpsc::sync_channel(1);
        let (failure_sender, failures) = mpsc::sync_channel(1);
        let (startup, started) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("slterm-workspace-writer".to_owned())
            .spawn(move || {
                run(
                    paths,
                    receiver,
                    failure_sender,
                    startup,
                    FinalStateReceivers {
                        view_states: final_state_receiver,
                        sidebar_state: final_sidebar_receiver,
                        project_snapshot: final_project_receiver,
                    },
                )
            })
            .map_err(WorkspaceWriterError::Spawn)?;
        match started.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                final_state_sender: final_states,
                final_sidebar_sender: final_sidebar_state,
                final_project_sender: final_project_snapshot,
                failures: Some(failures),
                worker: Some(worker),
                #[cfg(test)]
                join_completion_probe: None,
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(WorkspaceWriterError::Open(error))
            }
            Err(_) => {
                let _ = worker.join();
                Err(WorkspaceWriterError::Stopped)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn set_join_completion_probe(&mut self, probe: SyncSender<()>) {
        self.join_completion_probe = Some(probe);
    }

    pub(crate) fn client(&self) -> WorkspaceWriterClient {
        WorkspaceWriterClient {
            commands: self.commands.clone(),
            final_states: self.final_state_sender.clone(),
            final_sidebar_state: self.final_sidebar_sender.clone(),
            final_project_snapshot: self.final_project_sender.clone(),
        }
    }

    /// 外部 owner 的维护写与 UI client 共用同一提交逻辑。
    pub(crate) fn save_view_state(
        &self,
        key: &ViewKey,
        state: &ViewState,
    ) -> Result<(), WorkspaceWriterError> {
        if self.worker.is_none() {
            return Err(WorkspaceWriterError::Stopped);
        }
        self.client().save_view_state(key, state)
    }

    /// 外部 owner 与 UI client 共用同一项目聚合快照写队列。
    pub(crate) fn save_project_snapshot(
        &self,
        snapshot: &ProjectSnapshot,
    ) -> Result<(), WorkspaceWriterError> {
        if self.worker.is_none() {
            return Err(WorkspaceWriterError::Stopped);
        }
        self.client().save_project_snapshot(snapshot)
    }

    /// 将唯一失败接收端移交壳事件转发器；只能取走一次。
    pub(crate) fn take_failure_receiver(&mut self) -> Option<Receiver<WorkspaceWriteFailure>> {
        self.failures.take()
    }

    /// 非阻塞收割一条失败通知；容量为一时，已有通知即代表仍有失败待处理。
    pub(crate) fn try_recv_failure(&self) -> Option<WorkspaceWriteFailure> {
        self.failures.as_ref()?.try_recv().ok()
    }

    /// 立即重试全部待写状态并等待结果；生产只在后台执行体调用。
    pub(crate) fn flush(&self) -> Result<WorkspaceFlushReport, WorkspaceWriterError> {
        if self.worker.is_none() {
            return Err(WorkspaceWriterError::Stopped);
        }
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Flush { response })
            .map_err(|_| WorkspaceWriterError::Stopped)?;
        result.recv().map_err(|_| WorkspaceWriterError::Stopped)
    }

    /// 退出前重试全部待写状态，再关闭连接并回收线程。
    pub(crate) fn shutdown(mut self) -> Result<WorkspaceFlushReport, WorkspaceWriterError> {
        self.finish()
    }

    fn finish(&mut self) -> Result<WorkspaceFlushReport, WorkspaceWriterError> {
        let Some(worker) = self.worker.take() else {
            return Err(WorkspaceWriterError::Stopped);
        };
        let (response, result) = mpsc::sync_channel(1);
        let report = self
            .commands
            .send(Command::Shutdown { response })
            .map_err(|_| WorkspaceWriterError::Stopped)
            .and_then(|()| result.recv().map_err(|_| WorkspaceWriterError::Stopped));
        let _ = worker.join();
        #[cfg(test)]
        if let Some(probe) = self.join_completion_probe.take() {
            // 测试回执只在真实 JoinHandle::join 返回后发出。
            let _ = probe.try_send(());
        }
        report
    }
}

impl Drop for WorkspaceWriter {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.finish();
        }
    }
}

struct PendingViewState {
    key: ViewKey,
    state: ViewState,
    due: Option<Instant>,
}

struct PendingProjectSnapshot {
    snapshot: ProjectSnapshot,
    due: Option<Instant>,
}

struct PendingSidebarState {
    state: SidebarState,
    due: Option<Instant>,
}

struct FinalStateReceivers {
    view_states: Receiver<(ViewKey, ViewState)>,
    sidebar_state: Receiver<SidebarState>,
    project_snapshot: Receiver<ProjectSnapshot>,
}

#[derive(Default)]
struct PendingWrites {
    view_states: BTreeMap<String, PendingViewState>,
    sidebar_state: Option<PendingSidebarState>,
    project_snapshot: Option<PendingProjectSnapshot>,
}

#[derive(Default)]
struct PendingProjectCleanup {
    discarded_view_keys: Vec<String>,
    changed_side_views: Vec<(String, ViewState)>,
}

impl PendingProjectCleanup {
    fn apply(self, pending: &mut PendingWrites) {
        pending.project_snapshot = None;
        for key_json in self.discarded_view_keys {
            pending.view_states.remove(&key_json);
        }
        for (key_json, state) in self.changed_side_views {
            if let Some(pending) = pending.view_states.get_mut(&key_json) {
                pending.state = state;
            }
        }
    }
}

/// 预先计算待写副本清理，确保数据库提交后不再发生可能失败的转换。
fn prepare_pending_project_cleanup(
    pending: &PendingWrites,
    project_id: ProjectId,
    registry: &ViewStateRegistry,
) -> Result<PendingProjectCleanup, ProjectCleanupError> {
    let mut cleanup = PendingProjectCleanup::default();
    for (key_json, pending_view) in &pending.view_states {
        if pending_view.key.project_id() == Some(project_id) {
            cleanup.discarded_view_keys.push(key_json.clone());
            continue;
        }
        let ViewKey::SideView { side_view_id } = &pending_view.key else {
            continue;
        };
        if !matches!(side_view_id, SideViewId::Nav | SideViewId::Explorer) {
            continue;
        }
        if let Some(state) = remove_project_from_side_view_state(
            *side_view_id,
            &pending_view.state,
            project_id,
            registry,
        )
        .map_err(ProjectCleanupError::ViewState)?
        {
            cleanup.changed_side_views.push((key_json.clone(), state));
        }
    }
    Ok(cleanup)
}

fn run(
    paths: AppPaths,
    commands: Receiver<Command>,
    failures: SyncSender<WorkspaceWriteFailure>,
    startup: SyncSender<Result<(), DatabaseError>>,
    final_states: FinalStateReceivers,
) {
    let mut store = match WorkspaceDatabase::open(&paths) {
        Ok(store) => {
            let _ = startup.send(Ok(()));
            store
        }
        Err(error) => {
            let _ = startup.send(Err(error));
            return;
        }
    };
    let mut pending = PendingWrites::default();
    loop {
        let wait = next_wait(&pending);
        let command = match wait {
            Some(wait) => match commands.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    write_pending(&mut store, &mut pending, &failures, true, &final_states);
                    return;
                }
            },
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => {
                    write_pending(&mut store, &mut pending, &failures, true, &final_states);
                    return;
                }
            },
        };
        let Some(command) = command else {
            write_pending(&mut store, &mut pending, &failures, false, &final_states);
            continue;
        };
        match command {
            Command::SaveViewState {
                key_json,
                key,
                state,
            } => {
                pending.view_states.insert(
                    key_json,
                    PendingViewState {
                        key,
                        state: *state,
                        due: Some(Instant::now() + VIEW_STATE_DEBOUNCE),
                    },
                );
            }
            Command::SaveProjectSnapshot { snapshot } => {
                pending.project_snapshot = Some(PendingProjectSnapshot {
                    snapshot: *snapshot,
                    due: Some(Instant::now() + PROJECT_SNAPSHOT_DEBOUNCE),
                });
            }
            Command::CommitCloseIntent {
                snapshot,
                registration,
                response,
            } => {
                drain_final_states(&final_states, &mut pending);
                let result = store.commit_close_intent(&snapshot, &registration);
                if result.is_ok() {
                    // 关闭快照已成为最新事实，旧普通与终态副本不得随后反写。
                    pending.project_snapshot = None;
                }
                let _ = response.send(result);
            }
            Command::CommitProjectCloseIntent {
                snapshot,
                registration,
                response,
            } => {
                drain_final_states(&final_states, &mut pending);
                let result = store.commit_project_close_intent(&snapshot, &registration);
                if result.is_ok() {
                    // 中间保留事实同样不能被旧项目快照覆盖。
                    pending.project_snapshot = None;
                }
                let _ = response.send(result);
            }
            Command::CleanupProjectClose {
                current,
                registry,
                response,
            } => {
                drain_final_states(&final_states, &mut pending);
                let prepared = match current.scope {
                    CloseScope::Project { project_id } => {
                        prepare_pending_project_cleanup(&pending, project_id, &registry)
                    }
                    _ => Ok(PendingProjectCleanup::default()),
                };
                let result = match prepared {
                    Ok(prepared) => {
                        let result = store.cleanup_project_close(&current, &registry);
                        if result.is_ok() {
                            // 磁盘与待写副本同步删除，旧 SideView 分区不得复活。
                            prepared.apply(&mut pending);
                        }
                        result
                    }
                    Err(error) => Err(error),
                };
                let _ = response.send(result);
            }
            Command::AdvanceCloseIntent {
                current,
                next,
                response,
            } => {
                let result = store.advance_close_intent(
                    &current.close_id,
                    current.commit_status,
                    next,
                    current.intent.fingerprint(),
                    &current.store_epoch,
                );
                let _ = response.send(result);
            }
            Command::SaveSidebarState { state } => {
                pending.sidebar_state = Some(PendingSidebarState {
                    state: *state,
                    due: Some(Instant::now() + SIDEBAR_STATE_DEBOUNCE),
                });
            }
            Command::Flush { response } => {
                let report =
                    write_pending(&mut store, &mut pending, &failures, true, &final_states);
                let _ = response.send(report);
                continue;
            }
            Command::Shutdown { response } => {
                let report =
                    write_pending(&mut store, &mut pending, &failures, true, &final_states);
                let _ = response.send(report);
                return;
            }
        }
        write_pending(&mut store, &mut pending, &failures, false, &final_states);
    }
}

fn next_wait(pending: &PendingWrites) -> Option<Duration> {
    let now = Instant::now();
    pending
        .view_states
        .values()
        .filter_map(|pending| pending.due)
        .chain(
            pending
                .sidebar_state
                .as_ref()
                .and_then(|pending| pending.due),
        )
        .chain(
            pending
                .project_snapshot
                .as_ref()
                .and_then(|pending| pending.due),
        )
        .map(|due| due.saturating_duration_since(now))
        .min()
}

/// 终态队列与 flush 共用同一待写集合，最后提交覆盖普通队列旧副本。
fn drain_final_states(final_states: &FinalStateReceivers, pending: &mut PendingWrites) {
    for (key, state) in final_states.view_states.try_iter() {
        let Command::SaveViewState {
            key_json,
            key,
            state,
        } = view_state_command(&key, &state).expect("终态 ViewState 入队前已完成校验")
        else {
            unreachable!()
        };
        pending.view_states.insert(
            key_json,
            PendingViewState {
                key,
                state: *state,
                due: Some(Instant::now()),
            },
        );
    }
    for state in final_states.sidebar_state.try_iter() {
        pending.sidebar_state = Some(PendingSidebarState {
            state,
            due: Some(Instant::now()),
        });
    }
    for snapshot in final_states.project_snapshot.try_iter() {
        pending.project_snapshot = Some(PendingProjectSnapshot {
            snapshot,
            due: Some(Instant::now()),
        });
    }
}

/// 写到期项或全部项；失败项保留最新副本并取消定时器，等后续修改或显式冲刷重试。
fn write_pending(
    store: &mut WorkspaceDatabase,
    pending: &mut PendingWrites,
    failures: &SyncSender<WorkspaceWriteFailure>,
    all: bool,
    final_states: &FinalStateReceivers,
) -> WorkspaceFlushReport {
    if all {
        drain_final_states(final_states, pending);
    }
    let now = Instant::now();
    let ready = pending
        .view_states
        .iter()
        .filter(|(_, pending)| all || pending.due.is_some_and(|due| due <= now))
        .map(|(key_json, _)| key_json.clone())
        .collect::<Vec<_>>();
    let mut report = WorkspaceFlushReport::default();
    for key_json in ready {
        let result = {
            let pending = pending
                .view_states
                .get(&key_json)
                .expect("待写键在单线程写循环中不得消失");
            store.save_view_state(&pending.key, &pending.state)
        };
        match result {
            Ok(_) => {
                pending.view_states.remove(&key_json);
                report.written += 1;
            }
            Err(error) => {
                let pending = pending
                    .view_states
                    .get_mut(&key_json)
                    .expect("失败待写键必须保留");
                pending.due = None;
                let key = pending.key.clone();
                report.failed += 1;
                // 容量为一时，已有信号已代表失败；待写副本不随通知丢弃。
                let _ =
                    failures.try_send(WorkspaceWriteFailure::ViewState(ViewStateWriteFailure {
                        key,
                        error,
                    }));
            }
        }
    }

    let sidebar_ready = pending
        .sidebar_state
        .as_ref()
        .is_some_and(|pending| all || pending.due.is_some_and(|due| due <= now));
    if sidebar_ready {
        let result = {
            let sidebar = pending
                .sidebar_state
                .as_ref()
                .expect("就绪侧栏状态必须存在");
            store.save_sidebar_state(&sidebar.state)
        };
        match result {
            Ok(()) => {
                pending.sidebar_state = None;
                report.written += 1;
            }
            Err(error) => {
                let sidebar = pending
                    .sidebar_state
                    .as_mut()
                    .expect("失败侧栏状态必须保留");
                sidebar.due = None;
                report.failed += 1;
                let _ = failures.try_send(WorkspaceWriteFailure::SidebarState(error));
            }
        }
    }

    let project_ready = pending
        .project_snapshot
        .as_ref()
        .is_some_and(|pending| all || pending.due.is_some_and(|due| due <= now));
    if project_ready {
        let result = {
            let project = pending
                .project_snapshot
                .as_ref()
                .expect("就绪项目快照必须存在");
            store.save_project_snapshot(&project.snapshot)
        };
        match result {
            Ok(()) => {
                pending.project_snapshot = None;
                report.written += 1;
            }
            Err(error) => {
                let project = pending
                    .project_snapshot
                    .as_mut()
                    .expect("失败项目快照必须保留");
                project.due = None;
                report.failed += 1;
                let _ = failures.try_send(WorkspaceWriteFailure::ProjectSnapshot(error));
            }
        }
    }
    report
}

/// 将写失败逐条转换为非阻塞壳通知；失败通道关闭后线程自然退出。
pub(crate) fn spawn_workspace_failure_forwarder(
    failures: Receiver<WorkspaceWriteFailure>,
    mut notify: impl FnMut() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("slterm-workspace-events".into())
        .spawn(move || {
            while failures.recv().is_ok() {
                notify();
            }
        })
}

#[cfg(test)]
#[path = "workspace_writer_close_advance_tests.rs"]
mod workspace_writer_close_advance_tests;

#[cfg(test)]
#[path = "workspace_writer_tests.rs"]
mod workspace_writer_tests;
