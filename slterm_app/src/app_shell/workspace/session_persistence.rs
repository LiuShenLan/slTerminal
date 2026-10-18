//! 单窗口会话写盘状态机(上游 `SessionPersistence` 裁剪)。
//!
//! 机制照抄、语义单窗化:
//! - `latest` / `saved` 双快照:1 Hz `Checkpoint` 只在内容变化时落盘;
//! - `Checkpoint` 的 `clean_exit` 恒 `false`,只有关窗/最后 tab 关闭的
//!   最后一笔写 `true`;
//! - 首次成功快照来自 [`crate::session::Session::new`],天然
//!   `boot_attempts = 0`(断路器只看「活到首次自动保存没有」);
//! - 写失败只返回 `Err` 给调用方 warn,不更新 `saved`,下一拍自动重试;
//! - `isolated`(提权进程)不写共享会话文件,见
//!   [`crate::platform::elevation`];
//! - 单窗口「关窗即退出」:最后一笔成功后冻结,迟到的 1 Hz 快照不能再
//!   把 `clean_exit=true` 盖回 `false`。
//!
//! 多窗时代的 `Quit` 状态与逐窗快照组合没有存在前提,已裁;`WindowClose`
//! 在本世界即进程终点,承担其冻结职责。

use std::io;
use std::rc::Rc;

use crate::session::Session;

/// 一次写盘的触发原因。`Checkpoint` = 1 Hz 自动保存;其余为终局笔。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveReason {
    Checkpoint,
    WindowClose,
    TabsClosed,
}

impl SaveReason {
    /// 终局笔:成功后冻结状态机,且写 `clean_exit = true`。
    const fn is_final(self) -> bool {
        matches!(self, Self::WindowClose | Self::TabsClosed)
    }
}

/// 注入式写函数:生产 = `session::try_save`,测试 = 临时目录/计数器。
pub(crate) type SessionWriter = Rc<dyn Fn(&Session) -> io::Result<()>>;

pub(crate) struct SessionPersistence {
    latest: Option<Session>,
    saved: Option<Session>,
    finalized: bool,
    isolated: bool,
    writer: SessionWriter,
}

impl Default for SessionPersistence {
    fn default() -> Self {
        Self {
            latest: None,
            saved: None,
            finalized: false,
            isolated: crate::platform::elevation::requires_isolation(),
            writer: Rc::new(crate::session::try_save),
        }
    }
}

impl SessionPersistence {
    /// 测试用:显式写函数与提权隔离位(生产走 [`Default`])。
    pub(crate) fn with_writer(writer: SessionWriter, isolated: bool) -> Self {
        Self {
            latest: None,
            saved: None,
            finalized: false,
            isolated,
            writer,
        }
    }

    /// 关窗/最后 tab 关闭后是否已落下终局快照(1 Hz 泵的停写判据)。
    #[cfg(test)]
    pub(crate) fn finalized(&self) -> bool {
        self.finalized
    }

    /// 生产写入口:写函数取自构造时的注入值。
    pub(crate) fn save(&mut self, current: Option<Session>, reason: SaveReason) -> io::Result<()> {
        let writer = Rc::clone(&self.writer);
        self.save_with(current, reason, writer.as_ref())
    }

    /// 一次快照提交。`current = None` 表示「本拍没有新快照」(如关窗后
    /// 的收尾重试),由状态机决定是否复用 `latest`。
    fn save_with(
        &mut self,
        current: Option<Session>,
        reason: SaveReason,
        write: &dyn Fn(&Session) -> io::Result<()>,
    ) -> io::Result<()> {
        // 提权进程绝不触碰共享会话:管理员窗口的 shell 命令不能被普通
        // 启动恢复出来。
        if self.isolated || self.finalized {
            return Ok(());
        }
        // Checkpoint 在「本拍没有快照」或「空会话」时保留重试语义:上一次
        // 终局/空快照写失败留下的 `latest` 仍可在下一拍补写,但空快照本身
        // 不能抹掉已保存的 tab(启动初期 workspace 尚未建 tab)。
        let retry_checkpoint = reason == SaveReason::Checkpoint
            && current
                .as_ref()
                .is_none_or(|session| session.tabs.is_empty());
        let candidate =
            match reason {
                SaveReason::Checkpoint => current
                    .filter(|session| !session.tabs.is_empty())
                    .or_else(|| {
                        self.latest
                            .as_ref()
                            .filter(|latest| self.saved.as_ref() != Some(latest))
                            .cloned()
                    }),
                SaveReason::WindowClose | SaveReason::TabsClosed => current,
            };
        let Some(mut session) = candidate else {
            return Ok(());
        };
        // 重试笔保留 `latest` 里的 clean_exit(它已经是决定好的终局形态);
        // 新快照按原因重写。
        if !retry_checkpoint {
            session.clean_exit = reason.is_final();
        }
        self.latest = Some(session.clone());
        if self.saved.as_ref() == Some(&session) {
            self.finalized |= reason.is_final();
            return Ok(());
        }
        write(&session)?;
        self.saved = Some(session);
        self.finalized |= reason.is_final();
        Ok(())
    }
}

#[cfg(test)]
mod session_persistence_tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::session::TabSession;

    fn sample() -> Session {
        Session::new(
            0,
            vec![TabSession {
                cwd: "D:/work".into(),
                custom_name: Some("后端".into()),
                color: None,
                launch: None,
                layout: None,
                active_pane: 0,
            }],
        )
    }

    fn writer(log: &Rc<RefCell<Vec<Session>>>) -> SessionWriter {
        let log = Rc::clone(log);
        Rc::new(move |session: &Session| {
            log.borrow_mut().push(session.clone());
            Ok(())
        })
    }

    fn ordinary(log: &Rc<RefCell<Vec<Session>>>) -> SessionPersistence {
        SessionPersistence::with_writer(writer(log), false)
    }

    /// 1 Hz 无变化跳过:第二次相同快照不得再写盘。
    #[test]
    fn unchanged_checkpoints_do_not_rewrite_storage() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state.save(Some(sample()), SaveReason::Checkpoint).unwrap();
        state.save(Some(sample()), SaveReason::Checkpoint).unwrap();
        assert_eq!(log.borrow().len(), 1);
    }

    /// 提权进程四种入口都不触碰共享存储。
    #[test]
    fn isolated_processes_never_write_shared_session_storage() {
        let writes = Rc::new(Cell::new(0));
        let counter = Rc::clone(&writes);
        let mut state = SessionPersistence::with_writer(
            Rc::new(move |_| {
                counter.set(counter.get() + 1);
                Ok(())
            }),
            true,
        );
        for reason in [
            SaveReason::Checkpoint,
            SaveReason::TabsClosed,
            SaveReason::WindowClose,
            SaveReason::Checkpoint,
        ] {
            state.save(Some(sample()), reason).unwrap();
            state.save(None, reason).unwrap();
        }
        assert_eq!(writes.get(), 0);
    }

    /// 写失败不更新 saved,下一拍原快照重试成功。
    #[test]
    fn failed_checkpoints_are_retried_on_the_next_tick() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let attempts = Rc::new(Cell::new(0));
        let mut state = SessionPersistence::with_writer(
            {
                let log = Rc::clone(&log);
                Rc::new(move |session: &Session| {
                    attempts.set(attempts.get() + 1);
                    if attempts.get() == 1 {
                        return Err(io::Error::other("storage unavailable"));
                    }
                    log.borrow_mut().push(session.clone());
                    Ok(())
                })
            },
            false,
        );
        let session = sample();
        assert!(
            state
                .save(Some(session.clone()), SaveReason::Checkpoint)
                .is_err()
        );
        state.save(Some(session), SaveReason::Checkpoint).unwrap();
        assert_eq!(log.borrow().len(), 1);
    }

    /// 1 Hz 快照恒 `clean_exit=false`;关窗终局笔才写 true。
    #[test]
    fn only_the_final_write_marks_a_clean_exit() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state.save(Some(sample()), SaveReason::Checkpoint).unwrap();
        assert!(!log.borrow()[0].clean_exit);
        state.save(Some(sample()), SaveReason::WindowClose).unwrap();
        assert!(log.borrow().last().unwrap().clean_exit);
        assert!(state.finalized());
    }

    /// 首拍快照的规范形态 = `Session::new`(断路器归零);恢复路径的
    /// `mark_boot_attempt` 计数只存在于启动前那一笔。
    #[test]
    fn the_first_successful_snapshot_uses_the_zeroed_session_shape() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state
            .save(
                Some(crate::session::Session::new(0, sample().tabs)),
                SaveReason::Checkpoint,
            )
            .unwrap();
        assert_eq!(log.borrow()[0].boot_attempts, 0);
    }

    /// tabs 全关:空快照干净落盘,`should_restore` 为假。
    #[test]
    fn closing_every_tab_writes_an_empty_clean_snapshot() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state.save(Some(sample()), SaveReason::Checkpoint).unwrap();
        state
            .save(Some(Session::new(0, Vec::new())), SaveReason::TabsClosed)
            .unwrap();
        let saved = log.borrow().last().unwrap().clone();
        assert!(saved.tabs.is_empty());
        assert!(saved.clean_exit);
        assert!(!crate::session::should_restore(&saved));
        assert!(state.finalized());
    }

    /// 启动初期的空 Checkpoint 不能抹掉上一份已存 tab(例如恢复尚未建 tab)。
    #[test]
    fn an_empty_checkpoint_cannot_erase_saved_tabs() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state.save(Some(sample()), SaveReason::Checkpoint).unwrap();
        state
            .save(Some(Session::new(0, Vec::new())), SaveReason::Checkpoint)
            .unwrap();
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(log.borrow()[0].tabs.len(), 1);
    }

    /// 终局快照成功后迟到拍不得覆盖(单窗「关窗即退出」的冻结语义)。
    #[test]
    fn the_final_snapshot_cannot_be_overwritten_by_a_late_tick() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut state = ordinary(&log);
        state.save(Some(sample()), SaveReason::WindowClose).unwrap();
        let mut different = sample();
        different.tabs[0].cwd = "D:/after-close".into();
        state
            .save(Some(different), SaveReason::Checkpoint)
            .expect("冻结后迟到拍是空操作");
        assert_eq!(log.borrow().len(), 1);
    }

    /// 终局写失败可重试;成功后冻结。
    #[test]
    fn a_failed_final_write_is_retried_and_only_then_frozen() {
        let attempts = Cell::new(0);
        let mut state = SessionPersistence::with_writer(
            Rc::new(move |session: &Session| {
                attempts.set(attempts.get() + 1);
                if attempts.get() == 1 {
                    return Err(io::Error::other("temporary write failure"));
                }
                let _ = session;
                Ok(())
            }),
            false,
        );
        assert!(state.save(Some(sample()), SaveReason::WindowClose).is_err());
        assert!(!state.finalized());
        state.save(Some(sample()), SaveReason::WindowClose).unwrap();
        assert!(state.finalized());
    }
}
