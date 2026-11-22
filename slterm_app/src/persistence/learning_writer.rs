//! `learning.db` 的有界写队列与代次取消。

use std::fmt;
use std::sync::atomic::{AtomicI64, AtomicU8, Ordering};
use std::sync::{Arc, mpsc};

use slterm_path::AppPaths;

use super::database::{DatabaseAccess, DatabaseError};
use super::learning::{
    CommandObservation, DirectoryObservation, LearningDatabase, LearningSnapshot,
};

const QUEUE_CAPACITY: usize = 256;
const STATUS_READY: u8 = 0;
const STATUS_MEMORY_ONLY: u8 = 1;
const STATUS_FAILED: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LearningWriterStatus {
    Ready,
    MemoryOnly,
    Failed,
}

#[derive(Debug)]
pub(crate) enum LearningWriterError {
    QueueFull,
    Disconnected,
    GenerationExhausted,
    MemoryOnly,
    Database(DatabaseError),
    WorkerPanic,
}

impl fmt::Display for LearningWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::QueueFull => "学习写队列已满",
            Self::Disconnected => "学习写线程已断开",
            Self::GenerationExhausted => "学习代次已耗尽",
            Self::MemoryOnly => "学习库当前只允许内存学习",
            Self::Database(_) => "学习库操作失败",
            Self::WorkerPanic => "学习写线程异常结束",
        })
    }
}

impl std::error::Error for LearningWriterError {}

impl From<DatabaseError> for LearningWriterError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

#[derive(Clone)]
pub(crate) struct LearningWriterClient {
    sender: mpsc::SyncSender<WriterCommand>,
    generation: Arc<AtomicI64>,
    status: Arc<AtomicU8>,
    persistent: bool,
}

pub(crate) type LearningClearResponse = mpsc::Receiver<Result<(), LearningWriterError>>;

impl LearningWriterClient {
    /// 无后台线程的内存学习端点；清空会明确报告持久层不可用。
    pub(crate) fn memory_only() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        Self {
            sender,
            generation: Arc::new(AtomicI64::new(0)),
            status: Arc::new(AtomicU8::new(STATUS_MEMORY_ONLY)),
            persistent: false,
        }
    }

    pub(crate) fn record_command(
        &self,
        observation: CommandObservation,
    ) -> Result<(), LearningWriterError> {
        if !self.persistent {
            return Ok(());
        }
        self.sender.try_send(WriterCommand::Command {
            generation: self.generation(),
            observation,
        })?;
        Ok(())
    }

    pub(crate) fn record_directory(
        &self,
        observation: DirectoryObservation,
    ) -> Result<(), LearningWriterError> {
        if !self.persistent {
            return Ok(());
        }
        self.sender.try_send(WriterCommand::Directory {
            generation: self.generation(),
            observation,
        })?;
        Ok(())
    }

    /// 先推进共享代次，使已排队旧写立即失去资格，再提交清空事务。
    pub(crate) fn clear(&self) -> Result<LearningClearResponse, LearningWriterError> {
        let previous = self
            .generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
                generation.checked_add(1)
            })
            .map_err(|_| LearningWriterError::GenerationExhausted)?;
        let generation = previous + 1;
        let (completion, response) = mpsc::sync_channel(1);
        self.sender.try_send(WriterCommand::Clear {
            generation,
            completion,
        })?;
        Ok(response)
    }

    pub(crate) fn generation(&self) -> i64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn status(&self) -> LearningWriterStatus {
        match self.status.load(Ordering::Acquire) {
            STATUS_READY => LearningWriterStatus::Ready,
            STATUS_MEMORY_ONLY => LearningWriterStatus::MemoryOnly,
            STATUS_FAILED => LearningWriterStatus::Failed,
            _ => unreachable!("学习写状态是封闭枚举"),
        }
    }
}

impl<T> From<mpsc::TrySendError<T>> for LearningWriterError {
    fn from(error: mpsc::TrySendError<T>) -> Self {
        match error {
            mpsc::TrySendError::Full(_) => Self::QueueFull,
            mpsc::TrySendError::Disconnected(_) => Self::Disconnected,
        }
    }
}

pub(crate) struct LearningWriterRuntime {
    sender: mpsc::SyncSender<WriterCommand>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl LearningWriterRuntime {
    /// 启动前完成数据库打开与快照读取；调用方必须处于 GPUI 外的启动协调器。
    pub(crate) fn start(
        paths: &AppPaths,
    ) -> Result<(LearningWriterClient, LearningSnapshot, Self), DatabaseError> {
        let database = LearningDatabase::open(paths)?;
        let snapshot = database.snapshot()?;
        let persistent = database.access() == DatabaseAccess::ReadWrite;
        let generation = Arc::new(AtomicI64::new(snapshot.generation));
        let status = Arc::new(AtomicU8::new(if persistent {
            STATUS_READY
        } else {
            STATUS_MEMORY_ONLY
        }));
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let worker_generation = Arc::clone(&generation);
        let worker_status = Arc::clone(&status);
        let worker = std::thread::Builder::new()
            .name("learning-writer".to_owned())
            .spawn(move || {
                run_writer(
                    database,
                    receiver,
                    persistent,
                    worker_generation,
                    worker_status,
                )
            })?;
        let client = LearningWriterClient {
            sender: sender.clone(),
            generation,
            status,
            persistent,
        };
        Ok((
            client,
            snapshot,
            Self {
                sender,
                worker: Some(worker),
            },
        ))
    }

    /// 在后台关闭体排空当前代次并 join；不得在 GPUI 前台调用。
    pub(crate) fn shutdown(mut self) -> Result<(), LearningWriterError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), LearningWriterError> {
        let send = self
            .sender
            .send(WriterCommand::Shutdown)
            .map_err(|_| LearningWriterError::Disconnected);
        let join = self.worker.take().map_or(Ok(()), |worker| {
            worker.join().map_err(|_| LearningWriterError::WorkerPanic)
        });
        send.and(join)
    }
}

impl Drop for LearningWriterRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_inner() {
            log::warn!("learning writer shutdown failed: {error}");
        }
    }
}

enum WriterCommand {
    Command {
        generation: i64,
        observation: CommandObservation,
    },
    Directory {
        generation: i64,
        observation: DirectoryObservation,
    },
    Clear {
        generation: i64,
        completion: mpsc::SyncSender<Result<(), LearningWriterError>>,
    },
    Shutdown,
    #[cfg(test)]
    Pause {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
    },
}

fn run_writer(
    mut database: LearningDatabase,
    receiver: mpsc::Receiver<WriterCommand>,
    persistent: bool,
    generation: Arc<AtomicI64>,
    status: Arc<AtomicU8>,
) {
    let mut failed = false;
    while let Ok(command) = receiver.recv() {
        match command {
            WriterCommand::Command {
                generation: command_generation,
                observation,
            } => {
                if failed || command_generation != generation.load(Ordering::Acquire) {
                    continue;
                }
                if let Err(error) = database.record_command(observation) {
                    failed = true;
                    status.store(STATUS_FAILED, Ordering::Release);
                    log::warn!("learning command write failed: {error}");
                }
            }
            WriterCommand::Directory {
                generation: command_generation,
                observation,
            } => {
                if failed || command_generation != generation.load(Ordering::Acquire) {
                    continue;
                }
                if let Err(error) = database.record_directory(observation) {
                    failed = true;
                    status.store(STATUS_FAILED, Ordering::Release);
                    log::warn!("learning directory write failed: {error}");
                }
            }
            WriterCommand::Clear {
                generation: clear_generation,
                completion,
            } => {
                // 更新的清空已经取代本请求；旧请求无需再碰数据库。
                let result = if clear_generation != generation.load(Ordering::Acquire) {
                    Ok(())
                } else if persistent {
                    database.clear(clear_generation).map_err(Into::into)
                } else {
                    Err(LearningWriterError::MemoryOnly)
                };
                match &result {
                    Ok(()) => {
                        failed = false;
                        if persistent {
                            status.store(STATUS_READY, Ordering::Release);
                        }
                    }
                    Err(LearningWriterError::MemoryOnly) => {}
                    Err(_) => {
                        failed = true;
                        status.store(STATUS_FAILED, Ordering::Release);
                    }
                }
                let _ = completion.send(result);
            }
            WriterCommand::Shutdown => break,
            #[cfg(test)]
            WriterCommand::Pause { entered, release } => {
                let _ = entered.send(());
                let _ = release.recv();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use rusqlite::Connection;
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::learning::{
        CommandObservation, DirectoryObservation, LearningDatabase,
    };

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    #[test]
    fn learning_queue_full_is_nonblocking_and_invalidates_old_writes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let (client, _, runtime) = LearningWriterRuntime::start(&paths).unwrap();
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        client
            .sender
            .try_send(WriterCommand::Pause {
                entered: entered_tx,
                release: release_rx,
            })
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        for index in 0..QUEUE_CAPACITY {
            client
                .record_command(
                    CommandObservation::new("pwsh", &format!("old {index}"), None, 1).unwrap(),
                )
                .unwrap();
        }
        assert!(matches!(
            client.record_command(CommandObservation::new("pwsh", "overflow", None, 1).unwrap()),
            Err(LearningWriterError::QueueFull)
        ));
        assert!(matches!(
            client.clear(),
            Err(LearningWriterError::QueueFull)
        ));
        assert_eq!(client.generation(), 1);
        release_tx.send(()).unwrap();
        runtime.shutdown().unwrap();

        let raw = Connection::open(paths.learning_db_path()).unwrap();
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM command_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0,
            "即使 Clear 入队失败，旧代次待写也不得落盘"
        );
    }

    #[test]
    fn learning_clear_cancels_pending_writes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let (client, snapshot, runtime) = LearningWriterRuntime::start(&paths).unwrap();
        assert_eq!(snapshot.generation, 0);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        client
            .sender
            .try_send(WriterCommand::Pause {
                entered: entered_tx,
                release: release_rx,
            })
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        for command in ["old one", "old two"] {
            client
                .record_command(CommandObservation::new("pwsh", command, None, 1).unwrap())
                .unwrap();
        }
        client
            .record_directory(DirectoryObservation::new("D:/old", 1).unwrap())
            .unwrap();

        let cleared = client.clear().unwrap();
        assert_eq!(client.generation(), 1);
        release_tx.send(()).unwrap();
        cleared
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();

        let raw = Connection::open(paths.learning_db_path()).unwrap();
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM command_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        assert_eq!(
            raw.query_row("SELECT generation FROM learning_meta", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM directory_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        drop(raw);

        client
            .record_command(CommandObservation::new("pwsh", "new fact", None, 2).unwrap())
            .unwrap();
        client
            .record_directory(DirectoryObservation::new("D:/new", 2).unwrap())
            .unwrap();
        runtime.shutdown().unwrap();
        let reopened = LearningDatabase::open(&paths).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        assert_eq!(snapshot.generation, 1);
        assert_eq!(snapshot.commands.len(), 1);
        assert_eq!(snapshot.commands[0].command, "new fact");
        assert_eq!(snapshot.directories.len(), 1);
        assert_eq!(snapshot.directories[0].path, "D:/new");
    }
}
