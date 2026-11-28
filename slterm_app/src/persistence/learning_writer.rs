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
    /// 写线程是否持有可写数据库；重试/重置在写线程内更新。
    persistent: Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) type LearningClearResponse = mpsc::Receiver<Result<(), LearningWriterError>>;

/// 后台重试/重置的终态回执；成功值即写线程当前状态。
pub(crate) type LearningPersistenceResponse =
    mpsc::Receiver<Result<LearningWriterStatus, LearningWriterError>>;

impl LearningWriterClient {
    /// 无后台线程的内存学习端点；清空/重试会明确报告持久层不可用。
    pub(crate) fn memory_only() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        Self {
            sender,
            generation: Arc::new(AtomicI64::new(0)),
            status: Arc::new(AtomicU8::new(STATUS_MEMORY_ONLY)),
            persistent: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(crate) fn record_command(
        &self,
        observation: CommandObservation,
    ) -> Result<(), LearningWriterError> {
        if !self.persistent() {
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
        if !self.persistent() {
            return Ok(());
        }
        self.sender.try_send(WriterCommand::Directory {
            generation: self.generation(),
            observation,
        })?;
        Ok(())
    }

    /// 清空请求入队成功后才推进共享代次，使旧待写失去资格。
    ///
    /// 入队失败不得推进代次或让调用方看到成功；每个入队的清空都由
    /// 写线程按 FIFO 真实执行并返回自己的回执，不做“被更新代次取代”的伪成功。
    pub(crate) fn clear(&self) -> Result<LearningClearResponse, LearningWriterError> {
        // 内存学习没有可清空的持久事实；入队只会得到 MemoryOnly 回执，
        // 调用方还会误清内存。直接保留内存与错误。
        if self.status() == LearningWriterStatus::MemoryOnly {
            return Err(LearningWriterError::MemoryOnly);
        }
        let (completion, response) = mpsc::sync_channel(1);
        self.sender.try_send(WriterCommand::Clear { completion })?;
        self.generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
                generation.checked_add(1)
            })
            .map_err(|_| LearningWriterError::GenerationExhausted)?;
        Ok(response)
    }

    /// 后台重开学习库；失败保留当前连接或内存状态。
    pub(crate) fn retry(&self) -> Result<LearningPersistenceResponse, LearningWriterError> {
        let (completion, response) = mpsc::sync_channel(1);
        self.sender.try_send(WriterCommand::Retry { completion })?;
        Ok(response)
    }

    /// 显式重置：归档原库后重建，绝不在原损坏文件上覆盖。
    pub(crate) fn reset(&self) -> Result<LearningPersistenceResponse, LearningWriterError> {
        let (completion, response) = mpsc::sync_channel(1);
        self.sender.try_send(WriterCommand::Reset { completion })?;
        Ok(response)
    }

    fn persistent(&self) -> bool {
        self.persistent.load(Ordering::Acquire)
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
    ///
    /// 打开失败只降级为内存学习并保留原库，写线程仍然存活以承载设置页的
    /// 显式重试/重置；只有线程创建失败才返回错误。
    pub(crate) fn start(
        paths: &AppPaths,
    ) -> Result<(LearningWriterClient, LearningSnapshot, Self), DatabaseError> {
        let opened = match LearningDatabase::open(paths) {
            Ok(database) => Some(database),
            Err(error) => {
                log::warn!("learning database unavailable; using memory only: {error}");
                None
            }
        };
        let persistent = opened
            .as_ref()
            .is_some_and(|database| database.access() == DatabaseAccess::ReadWrite);
        let snapshot = match opened.as_ref() {
            Some(database) if persistent => match database.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    log::warn!("learning snapshot unavailable; using memory only: {error}");
                    return Self::start_memory_only(paths);
                }
            },
            _ => LearningSnapshot::default(),
        };
        let generation = Arc::new(AtomicI64::new(snapshot.generation));
        let status = Arc::new(AtomicU8::new(if persistent {
            STATUS_READY
        } else {
            STATUS_MEMORY_ONLY
        }));
        let persistent = Arc::new(std::sync::atomic::AtomicBool::new(persistent));
        Self::spawn(paths, opened, snapshot, persistent, generation, status)
    }

    /// 线程内没有任何可写数据库时的启动形态；重试仍可把它提升为持久学习。
    fn start_memory_only(
        paths: &AppPaths,
    ) -> Result<(LearningWriterClient, LearningSnapshot, Self), DatabaseError> {
        Self::spawn(
            paths,
            None,
            LearningSnapshot::default(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(AtomicI64::new(0)),
            Arc::new(AtomicU8::new(STATUS_MEMORY_ONLY)),
        )
    }

    fn spawn(
        paths: &AppPaths,
        database: Option<LearningDatabase>,
        snapshot: LearningSnapshot,
        persistent: Arc<std::sync::atomic::AtomicBool>,
        generation: Arc<AtomicI64>,
        status: Arc<AtomicU8>,
    ) -> Result<(LearningWriterClient, LearningSnapshot, Self), DatabaseError> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let worker_generation = Arc::clone(&generation);
        let worker_status = Arc::clone(&status);
        let worker_persistent = Arc::clone(&persistent);
        let worker_state = WriterState {
            paths: paths.clone(),
            database,
            persistent: persistent.load(Ordering::Acquire),
            failed: false,
        };
        let worker = std::thread::Builder::new()
            .name("learning-writer".to_owned())
            .spawn(move || {
                run_writer(
                    worker_state,
                    receiver,
                    worker_generation,
                    worker_status,
                    worker_persistent,
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
        completion: mpsc::SyncSender<Result<(), LearningWriterError>>,
    },
    Retry {
        completion: mpsc::SyncSender<Result<LearningWriterStatus, LearningWriterError>>,
    },
    Reset {
        completion: mpsc::SyncSender<Result<LearningWriterStatus, LearningWriterError>>,
    },
    Shutdown,
    #[cfg(test)]
    Pause {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
    },
}

/// 写线程独占的可变状态；重试/重置只在这里替换连接。
struct WriterState {
    paths: AppPaths,
    database: Option<LearningDatabase>,
    persistent: bool,
    /// 一次写入失败后暂停后续写，直到清空/重试/重置成功。
    failed: bool,
}

fn run_writer(
    mut state: WriterState,
    receiver: mpsc::Receiver<WriterCommand>,
    generation: Arc<AtomicI64>,
    status: Arc<AtomicU8>,
    persistent: Arc<std::sync::atomic::AtomicBool>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            WriterCommand::Command {
                generation: command_generation,
                observation,
            } => {
                if state.failed || command_generation != generation.load(Ordering::Acquire) {
                    continue;
                }
                let Some(database) = state.database.as_mut() else {
                    continue;
                };
                if let Err(error) = database.record_command(observation) {
                    state.failed = true;
                    status.store(STATUS_FAILED, Ordering::Release);
                    log::warn!("learning command write failed: {error}");
                }
            }
            WriterCommand::Directory {
                generation: command_generation,
                observation,
            } => {
                if state.failed || command_generation != generation.load(Ordering::Acquire) {
                    continue;
                }
                let Some(database) = state.database.as_mut() else {
                    continue;
                };
                if let Err(error) = database.record_directory(observation) {
                    state.failed = true;
                    status.store(STATUS_FAILED, Ordering::Release);
                    log::warn!("learning directory write failed: {error}");
                }
            }
            WriterCommand::Clear { completion } => {
                // 每个清空请求都真实执行；使用入队后推进的当前代次。
                let clear_generation = generation.load(Ordering::Acquire);
                let result = match (state.persistent, state.database.as_mut()) {
                    (true, Some(database)) => database.clear(clear_generation).map_err(Into::into),
                    _ => Err(LearningWriterError::MemoryOnly),
                };
                match &result {
                    Ok(()) => {
                        state.failed = false;
                        if state.persistent {
                            status.store(STATUS_READY, Ordering::Release);
                        }
                    }
                    Err(LearningWriterError::MemoryOnly) => {}
                    Err(_) => {
                        state.failed = true;
                        status.store(STATUS_FAILED, Ordering::Release);
                    }
                }
                let _ = completion.send(result);
            }
            WriterCommand::Retry { completion } => {
                let result = retry_persistence(&mut state, &generation, &status, &persistent);
                let _ = completion.send(result);
            }
            WriterCommand::Reset { completion } => {
                let result = reset_persistence(&mut state, &generation, &status, &persistent);
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

/// 后台重开数据库；只替换连接，不重放失败期间的丢失写入。
fn retry_persistence(
    state: &mut WriterState,
    generation: &AtomicI64,
    status: &AtomicU8,
    persistent: &std::sync::atomic::AtomicBool,
) -> Result<LearningWriterStatus, LearningWriterError> {
    match LearningDatabase::open(&state.paths) {
        Ok(database) if database.access() == DatabaseAccess::ReadWrite => {
            let snapshot = database.snapshot()?;
            state.database = Some(database);
            state.persistent = true;
            state.failed = false;
            persistent.store(true, Ordering::Release);
            generation.fetch_max(snapshot.generation, Ordering::AcqRel);
            status.store(STATUS_READY, Ordering::Release);
            Ok(LearningWriterStatus::Ready)
        }
        Ok(_) => {
            // 更高版本只读：保留既有连接与失败事实，报告不可写。
            status.store(
                if state.persistent {
                    STATUS_FAILED
                } else {
                    STATUS_MEMORY_ONLY
                },
                Ordering::Release,
            );
            Err(DatabaseError::IncompatibleSchema.into())
        }
        Err(error) => {
            status.store(
                if state.persistent {
                    STATUS_FAILED
                } else {
                    STATUS_MEMORY_ONLY
                },
                Ordering::Release,
            );
            Err(error.into())
        }
    }
}

/// 显式重置：先关闭连接并把原库（含 WAL/SHM）归档，再建新库。
fn reset_persistence(
    state: &mut WriterState,
    generation: &AtomicI64,
    status: &AtomicU8,
    persistent: &std::sync::atomic::AtomicBool,
) -> Result<LearningWriterStatus, LearningWriterError> {
    // 关闭现有连接后才能安全重命名主库与旁文件。
    state.database = None;
    state.persistent = false;
    persistent.store(false, Ordering::Release);
    let result = (|| -> Result<LearningSnapshot, DatabaseError> {
        archive_learning_database(&state.paths)?;
        let database = LearningDatabase::open(&state.paths)?;
        if database.access() != DatabaseAccess::ReadWrite {
            return Err(DatabaseError::IncompatibleSchema);
        }
        let snapshot = database.snapshot()?;
        state.database = Some(database);
        Ok(snapshot)
    })();
    match result {
        Ok(snapshot) => {
            state.persistent = true;
            state.failed = false;
            persistent.store(true, Ordering::Release);
            generation.fetch_max(snapshot.generation, Ordering::AcqRel);
            status.store(STATUS_READY, Ordering::Release);
            Ok(LearningWriterStatus::Ready)
        }
        Err(error) => {
            state.failed = false;
            status.store(STATUS_MEMORY_ONLY, Ordering::Release);
            Err(error.into())
        }
    }
}

/// 把原学习库整套移入同目录归档名；主库重命名失败时不触碰旁文件。
fn archive_learning_database(paths: &AppPaths) -> Result<(), DatabaseError> {
    let path = paths.learning_db_path();
    if !path.exists() {
        return Ok(());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    let archive = path.with_file_name(format!(
        "{}.reset-{timestamp}",
        path.file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| "learning.db".into())
    ));
    std::fs::rename(&path, &archive)?;
    for suffix in ["-wal", "-shm"] {
        let side = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
        if side.exists() {
            std::fs::rename(
                &side,
                std::path::PathBuf::from(format!("{}{suffix}", archive.display())),
            )?;
        }
    }
    Ok(())
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
    fn learning_queue_full_keeps_generation_and_pending_writes() {
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
        assert_eq!(client.generation(), 0, "清空未入队不得推进代次");
        release_tx.send(()).unwrap();
        runtime.shutdown().unwrap();

        let raw = Connection::open(paths.learning_db_path()).unwrap();
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM command_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            QUEUE_CAPACITY as i64,
            "清空失败后既有待写仍按原队列落盘，不得伪装成已清空"
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
