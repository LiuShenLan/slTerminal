//! 可信命令与目录的统一学习服务。

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use slterm_path::AppPaths;
use slterm_terminal::tty::{ShellKind, shell_kind_of};

use crate::directory_history::DirectoryHistory;
use crate::history::{HistoryScope, SltermHistory};
use crate::persistence::database::DatabaseError;
use crate::persistence::learning::{CommandObservation, DirectoryObservation};
pub(crate) use crate::persistence::learning_writer::LearningWriterStatus;
use crate::persistence::learning_writer::{
    LearningClearResponse, LearningPersistenceResponse, LearningWriterClient, LearningWriterError,
    LearningWriterRuntime,
};

#[derive(Clone)]
pub(crate) struct LearningService {
    memory: Arc<Mutex<LearningMemory>>,
    writer: LearningWriterClient,
}

struct LearningMemory {
    commands: Arc<SltermHistory>,
    directories: Arc<DirectoryHistory>,
}

impl Default for LearningMemory {
    fn default() -> Self {
        Self {
            commands: Arc::new(SltermHistory::default()),
            directories: Arc::new(DirectoryHistory::from_learning(Vec::new())),
        }
    }
}

/// 后台补全持有的不可变学习快照；清空后旧快照不能重新进入服务。
#[derive(Clone)]
pub(crate) struct LearningDataSnapshot {
    pub(crate) commands: Arc<SltermHistory>,
    pub(crate) directories: Arc<DirectoryHistory>,
    pub(crate) generation: i64,
}

impl LearningService {
    /// 在 GPUI 外启动独立写线程并恢复唯一学习快照。
    ///
    /// 数据库损坏或高版本不阻断启动：写线程保持内存学习状态，设置页可经
    /// 重试恢复后续写入，或经显式重置归档原库后重建。
    pub(crate) fn start(paths: &AppPaths) -> Result<(Self, LearningWriterRuntime), DatabaseError> {
        let (writer, snapshot, runtime) = LearningWriterRuntime::start(paths)?;
        let mut commands = SltermHistory::default();
        let mut records = snapshot.commands;
        records.sort_by(|left, right| {
            left.last_used_at_ms
                .cmp(&right.last_used_at_ms)
                .then_with(|| left.shell.cmp(&right.shell))
                .then_with(|| left.command.cmp(&right.command))
        });
        for record in records {
            commands.record_memory(&HistoryScope::Local, &record.command);
        }
        let memory = LearningMemory {
            commands: Arc::new(commands),
            directories: Arc::new(DirectoryHistory::from_learning(snapshot.directories)),
        };
        Ok((
            Self {
                memory: Arc::new(Mutex::new(memory)),
                writer,
            },
            runtime,
        ))
    }

    pub(crate) fn memory_only() -> Self {
        Self {
            memory: Arc::new(Mutex::new(LearningMemory::default())),
            writer: LearningWriterClient::memory_only(),
        }
    }

    pub(crate) fn snapshot(&self) -> LearningDataSnapshot {
        let memory = self
            .memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        LearningDataSnapshot {
            commands: Arc::clone(&memory.commands),
            directories: Arc::clone(&memory.directories),
            generation: self.writer.generation(),
        }
    }

    pub(crate) fn record_command(
        &self,
        shell_program: &str,
        command: &str,
        cwd: Option<&str>,
    ) -> Result<(), LearningWriterError> {
        self.record_command_inner(shell_program, command, cwd, || {})
    }

    fn record_command_inner(
        &self,
        shell_program: &str,
        command: &str,
        cwd: Option<&str>,
        before_write: impl FnOnce(),
    ) -> Result<(), LearningWriterError> {
        let shell = match shell_kind_of(shell_program) {
            ShellKind::Pwsh => "pwsh",
            ShellKind::PowerShell => "powershell",
            ShellKind::Cmd => "cmd",
            ShellKind::Other => return Ok(()),
        };
        let observation = CommandObservation::new(shell, command, cwd, current_time_ms())?;
        let mut memory = self
            .memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::make_mut(&mut memory.commands)
            .record_memory(&HistoryScope::Local, &observation.command);
        before_write();
        let result = self.writer.record_command(observation);
        drop(memory);
        result
    }

    pub(crate) fn record_directory(&self, path: &str) -> Result<(), LearningWriterError> {
        let at_ms = current_time_ms();
        let Some(normalized) = DirectoryHistory::normalize_observation(path) else {
            return Ok(());
        };
        // 在改变内存前完成全部字段校验，超限路径不得只落入内存副本。
        let observation = DirectoryObservation::new(&normalized, at_ms)?;
        let mut memory = self
            .memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Arc::make_mut(&mut memory.directories)
            .record_memory_at(&normalized, (at_ms / 1_000) as u64)
            .is_none()
        {
            return Ok(());
        }
        let result = self.writer.record_directory(observation);
        drop(memory);
        result
    }

    /// 后台重开学习库；成功只恢复后续写入，不重放失败期间的丢失事实。
    pub(crate) fn retry_persistence(
        &self,
    ) -> Result<LearningPersistenceResponse, LearningWriterError> {
        self.writer.retry()
    }

    /// 显式重置：归档原库后重建；内存学习数据保持不变。
    pub(crate) fn reset_persistence(
        &self,
    ) -> Result<LearningPersistenceResponse, LearningWriterError> {
        self.writer.reset()
    }

    /// 清空请求成功入队后才使旧内存失效；队列拒绝时保留内存与可见错误。
    pub(crate) fn clear(&self) -> Result<LearningClearResponse, LearningWriterError> {
        let mut memory = self
            .memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = self.writer.clear();
        if result.is_ok() {
            *memory = LearningMemory::default();
        }
        drop(memory);
        result
    }

    pub(crate) fn status(&self) -> LearningWriterStatus {
        self.writer.status()
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    use super::*;
    use crate::history::HistoryScope;

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    #[test]
    fn integration_local_learning_data() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        std::fs::write(
            paths.local_root().join("history.jsonl"),
            r#"{"cmd":"legacy-json-command"}"#,
        )
        .unwrap();
        std::fs::write(
            paths.local_root().join("directory_history.json"),
            b"not migrated",
        )
        .unwrap();

        let (service, runtime) = LearningService::start(&paths).unwrap();
        assert!(
            service
                .snapshot()
                .commands
                .hint(&HistoryScope::Local, "legacy-json")
                .is_none(),
            "旧 JSONL 不得迁入或参与补全"
        );
        service
            .record_command("pwsh.exe", " git status ", Some("D:/repo"))
            .unwrap();
        service
            .record_directory(temp.path().to_str().unwrap())
            .unwrap();
        runtime.shutdown().unwrap();

        let (service, runtime) = LearningService::start(&paths).unwrap();
        let restored = service.snapshot();
        assert_eq!(
            restored.commands.hint(&HistoryScope::Local, "git "),
            Some("status")
        );
        assert!(restored.directories.score(temp.path()).is_some());

        let cleared = service.clear().unwrap();
        assert!(
            service
                .snapshot()
                .commands
                .hint(&HistoryScope::Local, "git ")
                .is_none()
        );
        assert!(service.snapshot().directories.score(temp.path()).is_none());
        cleared
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        runtime.shutdown().unwrap();

        let raw = rusqlite::Connection::open(paths.learning_db_path()).unwrap();
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM command_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM directory_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    /// 真实 SQL 失败后状态转 Failed;重试在后台重开连接并恢复后续写入。
    #[test]
    fn persistence_retry_recovers_after_a_real_write_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let (service, runtime) = LearningService::start(&paths).unwrap();
        let raw = rusqlite::Connection::open(paths.learning_db_path()).unwrap();
        raw.execute_batch(
            "CREATE TRIGGER reject_command BEFORE INSERT ON command_history \
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();

        service
            .record_command("pwsh.exe", "blocked command", None)
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while service.status() != LearningWriterStatus::Failed
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            service.status(),
            LearningWriterStatus::Failed,
            "真实写入失败必须暴露封闭状态"
        );

        raw.execute_batch("DROP TRIGGER reject_command;").unwrap();
        let response = service.retry_persistence().unwrap();
        assert_eq!(
            response
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            LearningWriterStatus::Ready,
            "后台重开必须恢复可写状态"
        );
        drop(raw);

        service
            .record_command("pwsh.exe", "recovered command", None)
            .unwrap();
        runtime.shutdown().unwrap();

        let raw = rusqlite::Connection::open(paths.learning_db_path()).unwrap();
        let recovered: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM command_history WHERE command = 'recovered command'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recovered, 1, "重试后的新写入必须持久化");
        let blocked: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM command_history WHERE command = 'blocked command'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blocked, 0, "失败期间的记录不得伪装成已写入");
    }

    /// 损坏库启动只降级内存学习;显式重置归档原库并重建可写学习库。
    #[test]
    fn persistence_reset_archives_a_corrupt_database_and_reopens() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        std::fs::create_dir_all(paths.local_root()).unwrap();
        let original = b"not-a-sqlite-database".to_vec();
        std::fs::write(paths.learning_db_path(), &original).unwrap();

        let (service, runtime) = LearningService::start(&paths).unwrap();
        assert_eq!(
            service.status(),
            LearningWriterStatus::MemoryOnly,
            "损坏库必须以内存学习继续"
        );
        assert_eq!(
            std::fs::read(paths.learning_db_path()).unwrap(),
            original,
            "启动降级不得覆盖原库"
        );
        let retry = service
            .retry_persistence()
            .expect("损坏库仍应有可等待的重试入口");
        assert!(
            retry
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err(),
            "损坏库重试必须报告失败"
        );
        assert_eq!(
            service.status(),
            LearningWriterStatus::MemoryOnly,
            "未修复前重试仍保持内存状态"
        );

        let response = service.reset_persistence().unwrap();
        assert_eq!(
            response
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            LearningWriterStatus::Ready,
            "重置必须建立新的可写学习库"
        );
        let archive = std::fs::read_dir(paths.local_root())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("learning.db.reset-"))
            })
            .expect("原库必须保留为归档副本");
        assert_eq!(std::fs::read(archive).unwrap(), original);

        service
            .record_command("pwsh.exe", "after reset", None)
            .unwrap();
        runtime.shutdown().unwrap();
        let raw = rusqlite::Connection::open(paths.learning_db_path()).unwrap();
        let count: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM command_history WHERE command = 'after reset'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn failed_clear_keeps_memory_and_does_not_advance_generation() {
        let service = LearningService::memory_only();
        service.record_command("cmd.exe", "keep me", None).unwrap();
        assert!(service.clear().is_err(), "无持久写线程时清空必须失败");
        assert!(
            service
                .snapshot()
                .commands
                .contains_for_test(&HistoryScope::Local, "keep me"),
            "清空未入队时不得丢弃内存学习数据"
        );
        assert_eq!(service.snapshot().generation, 0, "失败清空不得推进代次");
    }

    #[test]
    fn learning_clear_serializes_with_an_inflight_recording() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let (service, runtime) = LearningService::start(&paths).unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let recorder = {
            let service = service.clone();
            std::thread::spawn(move || {
                service
                    .record_command_inner("cmd.exe", "old command", None, || {
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    })
                    .unwrap();
            })
        };
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let (cleared_tx, cleared_rx) = std::sync::mpsc::sync_channel(1);
        let clearer = {
            let service = service.clone();
            std::thread::spawn(move || {
                let response = service.clear().unwrap();
                response
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap()
                    .unwrap();
                cleared_tx.send(()).unwrap();
            })
        };

        // 旧实现会让 Clear 在记录写出前完成；新实现必须等待记录临界区。
        let clear_finished_first = cleared_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_ok();
        release_tx.send(()).unwrap();
        if !clear_finished_first {
            cleared_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        }
        recorder.join().unwrap();
        clearer.join().unwrap();
        runtime.shutdown().unwrap();

        let raw = rusqlite::Connection::open(paths.learning_db_path()).unwrap();
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM command_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0,
            "清空前开始的旧记录不得借新代次复活"
        );
    }
}
