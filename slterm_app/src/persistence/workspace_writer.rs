//! `workspace.db` 的专用写线程与 ViewState 去抖调度。
//!
//! 前台只提交不可变状态；SQLite 连接、失败待写副本与退出冲刷均由写线程独占。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use slterm_path::AppPaths;
use slterm_workspace::{ViewKey, ViewKeyError, ViewState, ViewStateError};

use super::database::DatabaseError;
use super::view_state::ViewStateStoreError;
use super::workspace::WorkspaceDatabase;

const VIEW_STATE_DEBOUNCE: Duration = Duration::from_millis(500);
const COMMAND_CAPACITY: usize = 64;

/// 一次显式或退出前冲刷的实际写入结果。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkspaceFlushReport {
    pub(crate) written: usize,
    pub(crate) failed: usize,
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
    ViewKey(ViewKeyError),
    ViewState(ViewStateError),
    QueueFull,
    FlushFailed,
    Stopped,
}

impl fmt::Display for WorkspaceWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(_) => formatter.write_str("工作区数据库打开失败"),
            Self::Spawn(_) => formatter.write_str("工作区写线程启动失败"),
            Self::ViewKey(_) => formatter.write_str("ViewState 的 ViewKey 编码失败"),
            Self::ViewState(error) => fmt::Display::fmt(error, formatter),
            Self::QueueFull => formatter.write_str("工作区写队列已满"),
            Self::FlushFailed => formatter.write_str("工作区视图状态最终冲刷失败"),
            Self::Stopped => formatter.write_str("工作区写线程已停止"),
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
    /// 实体退出时移交的最终状态；只在 owner shutdown 时由后台排空。
    final_states: SyncSender<(ViewKey, ViewState)>,
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
    final_states: Receiver<(ViewKey, ViewState)>,
    failures: Option<Receiver<ViewStateWriteFailure>>,
    worker: Option<JoinHandle<()>>,
}

impl WorkspaceWriter {
    /// 在调用线程等待数据库打开与线程启动；生产只由启动协调器后台调用。
    pub(crate) fn start(paths: AppPaths) -> Result<Self, WorkspaceWriterError> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (final_states, final_state_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (failure_sender, failures) = mpsc::sync_channel(1);
        let (startup, started) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("slterm-workspace-writer".to_owned())
            .spawn(move || run(paths, receiver, failure_sender, startup))
            .map_err(WorkspaceWriterError::Spawn)?;
        match started.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                final_state_sender: final_states,
                final_states: final_state_receiver,
                failures: Some(failures),
                worker: Some(worker),
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

    pub(crate) fn client(&self) -> WorkspaceWriterClient {
        WorkspaceWriterClient {
            commands: self.commands.clone(),
            final_states: self.final_state_sender.clone(),
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

    /// 将唯一失败接收端移交壳事件转发器；只能取走一次。
    pub(crate) fn take_failure_receiver(&mut self) -> Option<Receiver<ViewStateWriteFailure>> {
        self.failures.take()
    }

    /// 非阻塞收割一条失败通知；容量为一时，已有通知即代表仍有失败待处理。
    pub(crate) fn try_recv_view_state_failure(&self) -> Option<ViewStateWriteFailure> {
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
        // App 已释放，终态发送者不再增长；后台可用阻塞 send 等队列排水。
        let final_states = self.submit_final_states();
        let (response, result) = mpsc::sync_channel(1);
        let report = self
            .commands
            .send(Command::Shutdown { response })
            .map_err(|_| WorkspaceWriterError::Stopped)
            .and_then(|()| result.recv().map_err(|_| WorkspaceWriterError::Stopped));
        let _ = worker.join();
        final_states.and(report)
    }

    fn submit_final_states(&self) -> Result<(), WorkspaceWriterError> {
        for (key, state) in self.final_states.try_iter() {
            let command = view_state_command(&key, &state)?;
            self.commands
                .send(command)
                .map_err(|_| WorkspaceWriterError::Stopped)?;
        }
        Ok(())
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

fn run(
    paths: AppPaths,
    commands: Receiver<Command>,
    failures: SyncSender<ViewStateWriteFailure>,
    startup: SyncSender<Result<(), DatabaseError>>,
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
    let mut pending = BTreeMap::<String, PendingViewState>::new();
    loop {
        let wait = next_wait(&pending);
        let command = match wait {
            Some(wait) => match commands.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    write_pending(&mut store, &mut pending, &failures, true);
                    return;
                }
            },
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => {
                    write_pending(&mut store, &mut pending, &failures, true);
                    return;
                }
            },
        };
        let Some(command) = command else {
            write_pending(&mut store, &mut pending, &failures, false);
            continue;
        };
        match command {
            Command::SaveViewState {
                key_json,
                key,
                state,
            } => {
                pending.insert(
                    key_json,
                    PendingViewState {
                        key,
                        state: *state,
                        due: Some(Instant::now() + VIEW_STATE_DEBOUNCE),
                    },
                );
            }
            Command::Flush { response } => {
                let report = write_pending(&mut store, &mut pending, &failures, true);
                let _ = response.send(report);
                continue;
            }
            Command::Shutdown { response } => {
                let report = write_pending(&mut store, &mut pending, &failures, true);
                let _ = response.send(report);
                return;
            }
        }
        write_pending(&mut store, &mut pending, &failures, false);
    }
}

fn next_wait(pending: &BTreeMap<String, PendingViewState>) -> Option<Duration> {
    let now = Instant::now();
    pending
        .values()
        .filter_map(|pending| pending.due)
        .map(|due| due.saturating_duration_since(now))
        .min()
}

/// 写到期项或全部项；失败项保留最新副本并取消定时器，等后续修改或显式冲刷重试。
fn write_pending(
    store: &mut WorkspaceDatabase,
    pending: &mut BTreeMap<String, PendingViewState>,
    failures: &SyncSender<ViewStateWriteFailure>,
    all: bool,
) -> WorkspaceFlushReport {
    let now = Instant::now();
    let ready = pending
        .iter()
        .filter(|(_, pending)| all || pending.due.is_some_and(|due| due <= now))
        .map(|(key_json, _)| key_json.clone())
        .collect::<Vec<_>>();
    let mut report = WorkspaceFlushReport::default();
    for key_json in ready {
        let result = {
            let pending = pending
                .get(&key_json)
                .expect("待写键在单线程写循环中不得消失");
            store.save_view_state(&pending.key, &pending.state)
        };
        match result {
            Ok(_) => {
                pending.remove(&key_json);
                report.written += 1;
            }
            Err(error) => {
                let pending = pending.get_mut(&key_json).expect("失败待写键必须保留");
                pending.due = None;
                let key = pending.key.clone();
                report.failed += 1;
                // 容量为一时，已有信号已代表失败；待写副本不随通知丢弃。
                let _ = failures.try_send(ViewStateWriteFailure { key, error });
            }
        }
    }
    report
}

/// 将写失败逐条转换为非阻塞壳通知；失败通道关闭后线程自然退出。
pub(crate) fn spawn_view_state_failure_forwarder(
    failures: Receiver<ViewStateWriteFailure>,
    mut notify: impl FnMut() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("slterm-view-state-events".into())
        .spawn(move || {
            while failures.recv().is_ok() {
                notify();
            }
        })
}

#[cfg(test)]
mod workspace_writer_tests {
    use std::time::Instant;

    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use slterm_workspace::{
        InvalidViewState, SideViewId, ViewKeyKind, ViewStateRegistry, ViewStateSchema,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PersistentState {
        position: u64,
    }

    fn decode_persistent_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
        let state: PersistentState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
        serde_json::to_value(state).map_err(|_| InvalidViewState)
    }

    fn registry() -> ViewStateRegistry {
        ViewStateRegistry::new(
            &[ViewKeyKind::SideView, ViewKeyKind::AppSettingsOverlay]
                .map(|kind| ViewStateSchema::new(kind, decode_persistent_state)),
        )
        .unwrap()
    }

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn key_for(side_view_id: SideViewId) -> ViewKey {
        ViewKey::SideView { side_view_id }
    }

    fn key() -> ViewKey {
        key_for(SideViewId::Nav)
    }

    fn state_for(key: &ViewKey, position: u64) -> ViewState {
        ViewState::new(key, json!({"position": position}), &registry()).unwrap()
    }

    fn state(position: u64) -> ViewState {
        state_for(&key(), position)
    }

    fn read_position_for(store: &WorkspaceDatabase, key: &ViewKey) -> Option<(u64, u64)> {
        store.view_state(key, &registry()).unwrap().map(|stored| {
            let position = stored.state.state()["position"].as_u64().unwrap();
            (position, stored.revision)
        })
    }

    fn read_position(store: &WorkspaceDatabase) -> Option<(u64, u64)> {
        read_position_for(store, &key())
    }

    fn wait_for_position_for(store: &WorkspaceDatabase, key: &ViewKey) -> Option<(u64, u64)> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let current = read_position_for(store, key);
            if current.is_some() || Instant::now() >= deadline {
                return current;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_position(store: &WorkspaceDatabase) -> Option<(u64, u64)> {
        wait_for_position_for(store, &key())
    }

    fn wait_for_failure(writer: &WorkspaceWriter) -> Option<ViewStateWriteFailure> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(failure) = writer.try_recv_view_state_failure() {
                return Some(failure);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 每个 ViewKey 只写最新状态，且在 500 毫秒去抖到期前不落盘。
    #[test]
    fn workspace_writer_debounces_view_state_to_latest() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();

        assert_eq!(VIEW_STATE_DEBOUNCE, Duration::from_millis(500));
        for position in 1..=3 {
            writer.save_view_state(&key(), &state(position)).unwrap();
        }
        assert_eq!(read_position(&observer), None, "去抖到期前不得写盘");

        assert_eq!(wait_for_position(&observer), Some((3, 0)));
        assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
        writer.shutdown().unwrap();
    }

    /// UI client 与外部生命周期 owner 共用同一有界命令队列。
    #[test]
    fn workspace_writer_client_shares_command_channel() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        let client = writer.client();

        client.save_view_state(&key(), &state(7)).unwrap();

        assert_eq!(
            writer.flush().unwrap(),
            WorkspaceFlushReport {
                written: 1,
                failed: 0,
            }
        );
        assert_eq!(read_position(&observer), Some((7, 0)));
        writer.shutdown().unwrap();
    }

    /// 转发器逐条发出通知，并在失败通道关闭后退出。
    #[test]
    fn view_state_failure_forwarder_relays_and_stops() {
        let (failure_tx, failure_rx) = mpsc::sync_channel(2);
        let (event_tx, event_rx) = mpsc::channel();
        let forwarder = spawn_view_state_failure_forwarder(failure_rx, move || {
            let _ = event_tx.send(());
        })
        .unwrap();

        for _ in 0..2 {
            failure_tx
                .send(ViewStateWriteFailure {
                    key: key(),
                    error: ViewStateStoreError::CorruptRecord,
                })
                .unwrap();
            event_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        drop(failure_tx);

        assert!(event_rx.try_recv().is_err());
        forwarder.join().unwrap();
    }

    /// 失败接收端可移交壳事件转发器，writer owner 仍负责退出冲刷。
    #[test]
    fn workspace_writer_failure_receiver_is_transferable() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer.save_view_state(&key(), &state(7)).unwrap();
        let mut writer = WorkspaceWriter::start(paths).unwrap();
        observer
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();
        let failures = writer.take_failure_receiver().unwrap();
        assert!(writer.try_recv_view_state_failure().is_none());

        writer.save_view_state(&key(), &state(8)).unwrap();

        let failure = failures.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(failure.key(), &key());
        assert!(matches!(failure.error(), ViewStateStoreError::Database(_)));
        drop(failures);
        observer
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_view_state_write")
            .unwrap();
        writer.shutdown().unwrap();
        assert_eq!(read_position(&observer), Some((8, 1)));
    }

    /// 写失败保留最新待写副本；后续修改重试，shutdown 在退出前再次冲刷。
    #[test]
    fn view_state_save_failure_retains_memory() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer.save_view_state(&key(), &state(7)).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        observer
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();

        writer.save_view_state(&key(), &state(8)).unwrap();
        let failure = wait_for_failure(&writer);
        assert!(failure.is_some(), "后台写失败必须产生非阻塞通知");
        let failure = failure.unwrap();
        assert_eq!(failure.key(), &key());
        assert!(matches!(failure.error(), ViewStateStoreError::Database(_)));
        assert_eq!(read_position(&observer), Some((7, 0)));

        writer.save_view_state(&key(), &state(9)).unwrap();
        assert!(
            wait_for_failure(&writer).is_some(),
            "后续修改必须重试仍待写的最新状态"
        );
        assert_eq!(read_position(&observer), Some((7, 0)));
        assert_eq!(
            writer.flush().unwrap(),
            WorkspaceFlushReport {
                written: 0,
                failed: 1,
            }
        );
        assert_eq!(read_position(&observer), Some((7, 0)));

        observer
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_view_state_write")
            .unwrap();
        assert_eq!(
            writer.shutdown().unwrap(),
            WorkspaceFlushReport {
                written: 1,
                failed: 0,
            }
        );
        assert_eq!(read_position(&observer), Some((9, 1)));
    }

    /// 键与 DTO 类型不匹配时同步拒绝，不能进入后台待写队列。
    #[test]
    fn workspace_writer_rejects_mismatched_state_before_queueing() {
        let temp = tempfile::tempdir().unwrap();
        let writer = WorkspaceWriter::start(fixture(&temp)).unwrap();
        let mismatched = state_for(&ViewKey::AppSettingsOverlay, 1);

        assert!(matches!(
            writer.save_view_state(&key(), &mismatched),
            Err(WorkspaceWriterError::ViewState(ViewStateError::KeyMismatch))
        ));
        assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
        writer.shutdown().unwrap();
    }

    /// 数据库在写线程内打开失败时，启动方收到分类错误且线程被回收。
    #[test]
    fn workspace_writer_start_reports_open_failure() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        std::fs::write(&local, "not a directory").unwrap();
        let paths = AppPaths::new(temp.path().join("config"), local).unwrap();

        assert!(matches!(
            WorkspaceWriter::start(paths),
            Err(WorkspaceWriterError::Open(DatabaseError::Io(_)))
        ));
    }

    /// 有界队列满时非阻塞拒收；调用方可保留原状态并在排水后重试。
    #[test]
    fn workspace_writer_queue_full_allows_later_retry() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer.save_view_state(&key(), &state(1)).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        observer
            .database
            .connection
            .execute_batch("BEGIN EXCLUSIVE")
            .unwrap();
        writer.save_view_state(&key(), &state(2)).unwrap();
        std::thread::sleep(VIEW_STATE_DEBOUNCE + Duration::from_millis(100));

        for position in 3..3 + COMMAND_CAPACITY as u64 {
            writer.save_view_state(&key(), &state(position)).unwrap();
        }
        assert!(matches!(
            writer.save_view_state(&key(), &state(999)),
            Err(WorkspaceWriterError::QueueFull)
        ));

        observer
            .database
            .connection
            .execute_batch("ROLLBACK")
            .unwrap();
        assert_eq!(
            writer.flush().unwrap(),
            WorkspaceFlushReport {
                written: 1,
                failed: 0,
            }
        );
        assert_eq!(read_position(&observer), Some((66, 2)));
        assert!(writer.try_recv_view_state_failure().is_none());

        writer.save_view_state(&key(), &state(999)).unwrap();
        assert_eq!(
            writer.shutdown().unwrap(),
            WorkspaceFlushReport {
                written: 1,
                failed: 0,
            }
        );
        assert_eq!(read_position(&observer), Some((999, 3)));
    }

    /// 单键写失败保留自身待写副本，不阻塞或驱逐其他 ViewKey 的保存。
    #[test]
    fn workspace_writer_failure_isolated_to_one_view_key() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        let explorer = key_for(SideViewId::Explorer);
        observer.save_view_state(&key(), &state(7)).unwrap();
        observer
            .save_view_state(&explorer, &state_for(&explorer, 20))
            .unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        let trigger = format!(
            "CREATE TRIGGER reject_nav_write BEFORE UPDATE ON view_states \
             WHEN OLD.view_key = '{}' \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            key().to_canonical_json().unwrap()
        );
        observer
            .database
            .connection
            .execute_batch(&trigger)
            .unwrap();

        writer.save_view_state(&key(), &state(8)).unwrap();
        writer
            .save_view_state(&explorer, &state_for(&explorer, 21))
            .unwrap();
        let failure = wait_for_failure(&writer);
        assert!(failure.is_some(), "失败视图必须产生通知");
        assert_eq!(failure.unwrap().key(), &key());
        assert_eq!(wait_for_position_for(&observer, &explorer), Some((21, 1)));
        assert_eq!(read_position(&observer), Some((7, 0)));

        observer
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_nav_write")
            .unwrap();
        assert_eq!(
            writer.shutdown().unwrap(),
            WorkspaceFlushReport {
                written: 1,
                failed: 0,
            }
        );
        assert_eq!(read_position(&observer), Some((8, 1)));
        assert_eq!(read_position_for(&observer, &explorer), Some((21, 1)));
    }

    /// 未显式 shutdown 时，Drop 仍先冲刷待写状态并回收线程。
    #[test]
    fn workspace_writer_drop_flushes_pending_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        writer.save_view_state(&key(), &state(11)).unwrap();

        drop(writer);

        assert_eq!(read_position(&observer), Some((11, 0)));
    }
}
