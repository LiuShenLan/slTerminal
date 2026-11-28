//! 快照专用有界写队列、去抖冲刷、失败通知与线程生命周期。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use slterm_document::{DocumentId, DocumentSession};
use slterm_path::AppPaths;

use super::envelope::{DocumentSnapshot, SnapshotStoreError};
use super::store::DocumentSnapshotStore;
use crate::persistence::close::PersistedCloseIntent;

const COMMAND_CAPACITY: usize = 64;
const FAILURE_CAPACITY: usize = 64;
const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SnapshotFlushReport {
    written: usize,
    deleted: usize,
    failures: Vec<(DocumentId, SnapshotStoreError)>,
}

impl SnapshotFlushReport {
    pub(crate) fn failures(&self) -> &[(DocumentId, SnapshotStoreError)] {
        &self.failures
    }
}

#[derive(Debug)]
pub(crate) enum SnapshotWriterError {
    Spawn(io::Error),
    QueueFull,
    Stopped,
    FlushFailed,
}

impl fmt::Display for SnapshotWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Spawn(_) => "快照写线程启动失败",
            Self::QueueFull => "快照写队列已满",
            Self::Stopped => "快照写线程已停止",
            Self::FlushFailed => "快照冲刷存在失败，待写内容仍保留",
        })
    }
}

impl std::error::Error for SnapshotWriterError {}

enum SnapshotCommand {
    Write {
        snapshot: Box<DocumentSnapshot>,
        protected: BTreeSet<DocumentId>,
    },
    Reconcile(Box<DocumentSession>),
    Delete {
        id: DocumentId,
        close: Box<PersistedCloseIntent>,
    },
    Flush(SyncSender<SnapshotFlushReport>),
    Shutdown {
        response: SyncSender<SnapshotFlushReport>,
        force: bool,
    },
}

enum PendingSnapshot {
    Write {
        snapshot: Box<DocumentSnapshot>,
        protected: BTreeSet<DocumentId>,
        due: Option<Instant>,
    },
    Delete {
        close: Option<Box<PersistedCloseIntent>>,
        due: Option<Instant>,
    },
}

#[derive(Clone)]
pub(crate) struct DocumentSnapshotClient {
    commands: SyncSender<SnapshotCommand>,
}

impl DocumentSnapshotClient {
    /// 编辑事件非阻塞提交；同 Document 仅最新正文等待去抖写入。
    pub(crate) fn write(
        &self,
        snapshot: &DocumentSnapshot,
        protected: &BTreeSet<DocumentId>,
    ) -> Result<(), SnapshotWriterError> {
        self.try_send(SnapshotCommand::Write {
            snapshot: Box::new(snapshot.clone()),
            protected: protected.clone(),
        })
    }

    /// 保存完成或撤销清脏转删除；新脏状态会取消尚未执行的删除。
    pub(crate) fn reconcile(&self, session: &DocumentSession) -> Result<(), SnapshotWriterError> {
        self.try_send(SnapshotCommand::Reconcile(Box::new(session.clone())))
    }

    pub(crate) fn delete_closed(
        &self,
        id: DocumentId,
        close: &PersistedCloseIntent,
    ) -> Result<(), SnapshotWriterError> {
        self.try_send(SnapshotCommand::Delete {
            id,
            close: Box::new(close.clone()),
        })
    }

    /// 返回独立容量 1 回执；等待回执只能发生在后台执行体。
    pub(crate) fn flush(&self) -> Result<Receiver<SnapshotFlushReport>, SnapshotWriterError> {
        let (response, result) = mpsc::sync_channel(1);
        self.try_send(SnapshotCommand::Flush(response))?;
        Ok(result)
    }

    fn try_send(&self, command: SnapshotCommand) -> Result<(), SnapshotWriterError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => SnapshotWriterError::QueueFull,
                TrySendError::Disconnected(_) => SnapshotWriterError::Stopped,
            })
    }
}

/// 快照目录的专用后台所有者；GPUI 前台只持 client，不调用 shutdown。
pub(crate) struct DocumentSnapshotWriter {
    client: Option<DocumentSnapshotClient>,
    worker: Option<JoinHandle<()>>,
    failures: Option<Receiver<(DocumentId, SnapshotStoreError)>>,
}

impl DocumentSnapshotWriter {
    pub(crate) fn start(paths: &AppPaths) -> Result<Self, SnapshotWriterError> {
        Self::start_with_store(DocumentSnapshotStore::new(paths))
    }

    pub(super) fn start_with_store(
        mut store: DocumentSnapshotStore,
    ) -> Result<Self, SnapshotWriterError> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (failures, failure_receiver) = mpsc::sync_channel(FAILURE_CAPACITY);
        let worker = std::thread::Builder::new()
            .name("slterm-document-snapshots".to_owned())
            .spawn(move || {
                let mut pending = BTreeMap::new();
                loop {
                    let timeout = pending
                        .values()
                        .filter_map(|entry| match entry {
                            PendingSnapshot::Write { due, .. } => *due,
                            PendingSnapshot::Delete { due, .. } => *due,
                        })
                        .min()
                        .map(|due| due.saturating_duration_since(Instant::now()))
                        .unwrap_or(Duration::from_secs(3600));
                    match receiver.recv_timeout(timeout) {
                        Ok(command) => {
                            if handle_command(command, &mut pending, &mut store) {
                                break;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            flush_due(&mut pending, &mut store, &failures);
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            let _ = flush_pending(&mut pending, &mut store);
                            break;
                        }
                    }
                }
            })
            .map_err(SnapshotWriterError::Spawn)?;
        Ok(Self {
            client: Some(DocumentSnapshotClient { commands }),
            worker: Some(worker),
            failures: Some(failure_receiver),
        })
    }

    pub(crate) fn client(&self) -> DocumentSnapshotClient {
        self.client
            .as_ref()
            .expect("writer client 仅在退出时移除")
            .clone()
    }

    /// 去抖写失败经有界通道独立上报；flush 回执仍聚合本次全部失败。
    pub(crate) fn take_failures(&mut self) -> Option<Receiver<(DocumentId, SnapshotStoreError)>> {
        self.failures.take()
    }

    /// 后台最终冲刷；失败保留线程与待写集合，修复原因后可由原 owner 重试。
    pub(crate) fn shutdown(&mut self) -> Result<SnapshotFlushReport, SnapshotWriterError> {
        self.stop(false)
    }

    fn stop(&mut self, force: bool) -> Result<SnapshotFlushReport, SnapshotWriterError> {
        let Some(client) = self.client.as_ref().cloned() else {
            return Err(SnapshotWriterError::Stopped);
        };
        let (response, result) = mpsc::sync_channel(1);
        // 这是专用后台退出路径，允许等待队列腾出位置，不在 GPUI 前台调用。
        if client
            .commands
            .send(SnapshotCommand::Shutdown { response, force })
            .is_err()
        {
            self.reap_after_disconnect();
            return Err(SnapshotWriterError::Stopped);
        }
        let report = match result.recv() {
            Ok(report) => report,
            Err(_) => {
                self.reap_after_disconnect();
                return Err(SnapshotWriterError::Stopped);
            }
        };
        if !report.failures.is_empty() && !force {
            return Err(SnapshotWriterError::FlushFailed);
        }
        self.client.take();
        let worker = self.worker.take().expect("client 存活时 worker 必须存在");
        worker.join().map_err(|_| SnapshotWriterError::Stopped)?;
        if report.failures.is_empty() {
            Ok(report)
        } else {
            Err(SnapshotWriterError::FlushFailed)
        }
    }

    fn reap_after_disconnect(&mut self) {
        self.client.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for DocumentSnapshotWriter {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.stop(true);
        }
    }
}

/// 返回 true 表示 Shutdown 已冲刷并要求退出。
fn handle_command(
    command: SnapshotCommand,
    pending: &mut BTreeMap<DocumentId, PendingSnapshot>,
    store: &mut DocumentSnapshotStore,
) -> bool {
    match command {
        SnapshotCommand::Write {
            snapshot,
            protected,
        } => {
            pending.insert(
                snapshot.document_id(),
                PendingSnapshot::Write {
                    snapshot,
                    protected,
                    due: Some(Instant::now() + SNAPSHOT_DEBOUNCE),
                },
            );
        }
        SnapshotCommand::Reconcile(session) => {
            if session.is_dirty() {
                if matches!(
                    pending.get(&session.id()),
                    Some(PendingSnapshot::Delete { .. })
                ) {
                    pending.remove(&session.id());
                }
            } else {
                pending.insert(
                    session.id(),
                    PendingSnapshot::Delete {
                        close: None,
                        due: Some(Instant::now()),
                    },
                );
            }
        }
        SnapshotCommand::Delete { id, close } => {
            pending.insert(
                id,
                PendingSnapshot::Delete {
                    close: Some(close),
                    due: Some(Instant::now()),
                },
            );
        }
        SnapshotCommand::Flush(response) => {
            let _ = response.send(flush_pending(pending, store));
        }
        SnapshotCommand::Shutdown { response, force } => {
            let report = flush_pending(pending, store);
            let complete = report.failures.is_empty() || force;
            let _ = response.send(report);
            return complete;
        }
    }
    false
}

fn flush_due(
    pending: &mut BTreeMap<DocumentId, PendingSnapshot>,
    store: &mut DocumentSnapshotStore,
    failures: &SyncSender<(DocumentId, SnapshotStoreError)>,
) {
    let now = Instant::now();
    let due = pending
        .iter()
        .filter_map(|(id, entry)| match entry {
            PendingSnapshot::Write { due: Some(due), .. } if *due <= now => Some(*id),
            PendingSnapshot::Delete { due: Some(due), .. } if *due <= now => Some(*id),
            _ => None,
        })
        .collect::<Vec<_>>();
    for id in due {
        let Some(entry) = pending.remove(&id) else {
            continue;
        };
        match execute_pending(id, entry, store) {
            Ok(()) => {}
            Err((mut entry, error)) => {
                match &mut entry {
                    PendingSnapshot::Write { due, .. } => *due = None,
                    PendingSnapshot::Delete { due, .. } => *due = None,
                }
                pending.insert(id, entry);
                let _ = failures.try_send((id, error));
            }
        }
    }
}

fn flush_pending(
    pending: &mut BTreeMap<DocumentId, PendingSnapshot>,
    store: &mut DocumentSnapshotStore,
) -> SnapshotFlushReport {
    let mut report = SnapshotFlushReport::default();
    for (id, entry) in std::mem::take(pending) {
        let is_delete = matches!(entry, PendingSnapshot::Delete { .. });
        match execute_pending(id, entry, store) {
            Ok(()) if is_delete => report.deleted += 1,
            Ok(()) => report.written += 1,
            Err((mut entry, error)) => {
                match &mut entry {
                    PendingSnapshot::Write { due, .. } => *due = None,
                    PendingSnapshot::Delete { due, .. } => *due = None,
                }
                pending.insert(id, entry);
                report.failures.push((id, error));
            }
        }
    }
    report
}

fn execute_pending(
    id: DocumentId,
    entry: PendingSnapshot,
    store: &mut DocumentSnapshotStore,
) -> Result<(), (PendingSnapshot, SnapshotStoreError)> {
    match entry {
        PendingSnapshot::Write {
            snapshot,
            protected,
            due,
        } => match store.write(&snapshot, &protected) {
            Ok(()) => Ok(()),
            Err(error) => Err((
                PendingSnapshot::Write {
                    snapshot,
                    protected,
                    due,
                },
                error,
            )),
        },
        PendingSnapshot::Delete { close, due } => {
            let result = match &close {
                Some(close) => store.delete_closed(id, close),
                None => store.delete_validated(id),
            };
            result.map_err(|error| (PendingSnapshot::Delete { close, due }, error))
        }
    }
}
