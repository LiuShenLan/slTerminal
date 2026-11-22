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
    LearningClearResponse, LearningWriterClient, LearningWriterError, LearningWriterRuntime,
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

    /// 无论排队是否成功都先使旧内存失效；持久化结果由独立回执报告。
    pub(crate) fn clear(&self) -> Result<LearningClearResponse, LearningWriterError> {
        let mut memory = self
            .memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = self.writer.clear();
        *memory = LearningMemory::default();
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
