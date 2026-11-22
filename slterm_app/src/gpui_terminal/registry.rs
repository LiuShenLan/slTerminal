//! 逻辑终端到单 worker、PTY 代次与进程身份的唯一运行期映射。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

use slterm_terminal::event_loop::{EventLoopSendError, Msg};
use slterm_terminal::tty::ProcessIdentity;
use slterm_workspace::TerminalId;

use super::session::{SharedTerminal, TerminalSession};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PtyGeneration(u64);

#[allow(dead_code)] // 代次观测与重连接入由后续 Runtime／恢复切片消费。
impl PtyGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// 一次 PTY 生命的完整命令目标；不能只凭 TerminalId 写入重连后的进程。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalTarget {
    terminal_id: TerminalId,
    generation: PtyGeneration,
}

#[allow(dead_code)] // 外部目标观测由 Runtime 与恢复测试消费。
impl TerminalTarget {
    pub const fn terminal_id(self) -> TerminalId {
        self.terminal_id
    }

    pub const fn generation(self) -> PtyGeneration {
        self.generation
    }
}

#[derive(Debug)]
pub enum RegistryError {
    DuplicateTerminal,
    TerminalNotFound,
    StaleGeneration,
    #[allow(dead_code)] // 由重连耗尽路径构造；当前已有定向测试。
    GenerationExhausted,
    PtyLimitReached {
        limit: usize,
    },
    ProcessExited,
    ProcessStillRunning,
    Launch(std::io::Error),
    Send(EventLoopSendError),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTerminal => formatter.write_str("终端已有存活 worker"),
            Self::TerminalNotFound => formatter.write_str("终端 worker 不存在"),
            Self::StaleGeneration => formatter.write_str("PTY 代次已失效"),
            Self::GenerationExhausted => formatter.write_str("PTY 代次已耗尽"),
            Self::PtyLimitReached { limit } => {
                write!(
                    formatter,
                    "存活 PTY 数已达上限 {limit}，请等待进程退出或关闭终端后重试"
                )
            }
            Self::ProcessExited => formatter.write_str("PTY 进程已退出"),
            Self::ProcessStillRunning => formatter.write_str("只有进程退出后才能重新连接"),
            Self::Launch(error) => write!(formatter, "终端 worker 启动失败: {error}"),
            Self::Send(error) => write!(formatter, "终端命令发送失败: {error}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Launch(error) => Some(error),
            Self::Send(error) => Some(error),
            _ => None,
        }
    }
}

struct TerminalEntry {
    generation: PtyGeneration,
    #[allow(dead_code)] // Runtime 与 Agent 经 process_identity 读取该固定身份。
    process: ProcessIdentity,
    process_live: bool,
    session: TerminalSession,
}

/// AppRoot 独占的运行期注册表；不复制逻辑终端记录或持久元数据。
#[derive(Default)]
pub struct TerminalRegistry {
    entries: BTreeMap<TerminalId, TerminalEntry>,
}

impl TerminalRegistry {
    fn ensure_live_capacity(&self) -> Result<(), RegistryError> {
        let live = self
            .entries
            .values()
            .filter(|entry| entry.process_live)
            .count();
        if live >= slterm_terminal::tty::MAX_PTY_SESSIONS {
            return Err(RegistryError::PtyLimitReached {
                limit: slterm_terminal::tty::MAX_PTY_SESSIONS,
            });
        }
        Ok(())
    }

    /// 只有工厂成功后才公布新代次；重复 TerminalId 不执行工厂。
    pub fn launch(
        &mut self,
        terminal_id: TerminalId,
        factory: impl FnOnce() -> std::io::Result<TerminalSession>,
    ) -> Result<TerminalTarget, RegistryError> {
        if self.entries.contains_key(&terminal_id) {
            return Err(RegistryError::DuplicateTerminal);
        }
        self.ensure_live_capacity()?;
        let session = factory().map_err(RegistryError::Launch)?;
        let generation = PtyGeneration(1);
        let process = session.process_identity();
        self.entries.insert(
            terminal_id,
            TerminalEntry {
                generation,
                process,
                process_live: true,
                session,
            },
        );
        Ok(TerminalTarget {
            terminal_id,
            generation,
        })
    }

    /// 重连先证明代次可推进，再启动新 worker；旧 worker 交由关闭链显式回收。
    pub fn reconnect(
        &mut self,
        terminal_id: TerminalId,
        factory: impl FnOnce(SharedTerminal) -> std::io::Result<super::session::SpawnedSession>,
    ) -> Result<TerminalReplacement, RegistryError> {
        let entry = self
            .entries
            .get(&terminal_id)
            .ok_or(RegistryError::TerminalNotFound)?;
        let generation = entry
            .generation
            .next()
            .ok_or(RegistryError::GenerationExhausted)?;
        if entry.process_live {
            return Err(RegistryError::ProcessStillRunning);
        }
        let terminal = entry.session.shared_term();
        self.ensure_live_capacity()?;
        let (session, events) = factory(terminal).map_err(RegistryError::Launch)?;
        let frame = session.capture_frame(0);
        let process = session.process_identity();
        let previous = self
            .entries
            .insert(
                terminal_id,
                TerminalEntry {
                    generation,
                    process,
                    process_live: true,
                    session,
                },
            )
            .expect("重连前已证明旧 worker 存在")
            .session;
        Ok(TerminalReplacement {
            target: TerminalTarget {
                terminal_id,
                generation,
            },
            previous,
            events,
            frame,
        })
    }

    pub fn target(&self, terminal_id: TerminalId) -> Option<TerminalTarget> {
        self.entries.get(&terminal_id).map(|entry| TerminalTarget {
            terminal_id,
            generation: entry.generation,
        })
    }

    fn entry(&self, target: TerminalTarget) -> Result<&TerminalEntry, RegistryError> {
        let entry = self
            .entries
            .get(&target.terminal_id)
            .ok_or(RegistryError::TerminalNotFound)?;
        if entry.generation != target.generation {
            return Err(RegistryError::StaleGeneration);
        }
        Ok(entry)
    }

    #[allow(dead_code)] // Runtime 与 Agent 的固定进程身份查询由后续切片消费。
    pub fn process_identity(
        &self,
        target: TerminalTarget,
    ) -> Result<ProcessIdentity, RegistryError> {
        Ok(self.entry(target)?.process)
    }

    /// Registry 先校验完整目标，再把命令交给固定 worker 的有界端点。
    pub fn send(&self, target: TerminalTarget, command: Msg) -> Result<(), RegistryError> {
        let entry = self.entry(target)?;
        if matches!(&command, Msg::Input(_)) && !entry.process_live {
            return Err(RegistryError::ProcessExited);
        }
        entry
            .session
            .notifier
            .0
            .try_send(command)
            .map_err(RegistryError::Send)
    }

    /// 进程退出只退休输入能力，不移除显示 worker 或最后画面。
    pub fn mark_process_exited(&mut self, target: TerminalTarget) -> Result<(), RegistryError> {
        let entry = self
            .entries
            .get_mut(&target.terminal_id)
            .ok_or(RegistryError::TerminalNotFound)?;
        if entry.generation != target.generation {
            return Err(RegistryError::StaleGeneration);
        }
        entry.process_live = false;
        Ok(())
    }

    /// 启动移交时捕获首帧；运行期更新只消费 worker 发布的帧。
    pub fn capture_frame(
        &self,
        target: TerminalTarget,
        revision: u64,
    ) -> Result<slterm_terminal::render::TerminalFrame, RegistryError> {
        Ok(self.entry(target)?.session.capture_frame(revision))
    }

    /// 退休完整目标并移交 worker；调用方必须在后台显式 shutdown。
    pub fn remove(&mut self, target: TerminalTarget) -> Result<TerminalSession, RegistryError> {
        self.entry(target)?;
        Ok(self
            .entries
            .remove(&target.terminal_id)
            .expect("已校验的 worker 必须存在")
            .session)
    }

    fn observe_input(
        &self,
        target: TerminalTarget,
        preserves_prompt: bool,
    ) -> Result<(), RegistryError> {
        let prompt = &self.entry(target)?.session.native_prompt;
        let epoch = prompt.snapshot().input_epoch.wrapping_add(1);
        prompt.observe_input(epoch, preserves_prompt);
        Ok(())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    fn apply_test_command(
        &self,
        target: TerminalTarget,
        command: &Msg,
    ) -> Result<bool, RegistryError> {
        Ok(self.entry(target)?.session.apply_test_command(command))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    fn feed_test_bytes(&self, target: TerminalTarget, bytes: &[u8]) -> Result<(), RegistryError> {
        self.entry(target)?.session.feed_test_bytes(bytes);
        Ok(())
    }
}

/// View 的固定代次弱绑定；升级失败或代次失效均不能写其他 worker。
#[derive(Clone)]
pub struct TerminalBinding {
    target: TerminalTarget,
    registry: Weak<RefCell<TerminalRegistry>>,
}

impl TerminalBinding {
    pub fn new(registry: &Rc<RefCell<TerminalRegistry>>, target: TerminalTarget) -> Self {
        Self {
            target,
            registry: Rc::downgrade(registry),
        }
    }

    pub const fn target(&self) -> TerminalTarget {
        self.target
    }

    pub fn send(&self, command: Msg) -> Result<(), RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow()
            .send(self.target, command)
    }

    pub fn observe_input(&self, preserves_prompt: bool) -> Result<(), RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow()
            .observe_input(self.target, preserves_prompt)
    }

    pub fn mark_process_exited(&self) -> Result<(), RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow_mut()
            .mark_process_exited(self.target)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn is_current(&self) -> bool {
        self.registry.upgrade().is_some_and(|registry| {
            registry.borrow().target(self.target.terminal_id) == Some(self.target)
        })
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn capture_test_frame(
        &self,
        revision: u64,
    ) -> Result<slterm_terminal::render::TerminalFrame, RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow()
            .capture_frame(self.target, revision)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn apply_test_command(&self, command: &Msg) -> Result<bool, RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow()
            .apply_test_command(self.target, command)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn feed_test_bytes(&self, bytes: &[u8]) -> Result<(), RegistryError> {
        self.registry
            .upgrade()
            .ok_or(RegistryError::TerminalNotFound)?
            .borrow()
            .feed_test_bytes(self.target, bytes)
    }
}

pub struct TerminalReplacement {
    target: TerminalTarget,
    previous: TerminalSession,
    events: super::event_mailbox::EventReceiver,
    frame: slterm_terminal::render::TerminalFrame,
}

impl TerminalReplacement {
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub const fn target(&self) -> TerminalTarget {
        self.target
    }

    /// 旧 worker 只能移交给后台关闭路径，不能在 GPUI 前台析构 join。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn into_previous(self) -> TerminalSession {
        self.previous
    }

    pub fn into_parts(
        self,
    ) -> (
        TerminalTarget,
        TerminalSession,
        super::event_mailbox::EventReceiver,
        slterm_terminal::render::TerminalFrame,
    ) {
        (self.target, self.previous, self.events, self.frame)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod registry_tests {
    use std::borrow::Cow;
    use std::str::FromStr as _;

    use slterm_terminal::event_loop::Msg;
    use slterm_workspace::TerminalId;

    use super::*;
    use crate::gpui_terminal::session;

    fn id(value: &str) -> TerminalId {
        TerminalId::from_str(value).unwrap()
    }

    #[test]
    fn registry_assigns_generation_only_after_successful_launch() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e01");
        let (session, _receiver) = session::test_session();
        let process = session.process_identity();

        let target = registry
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))
            .unwrap();

        assert_eq!(target.generation().get(), 1);
        assert_eq!(registry.process_identity(target).unwrap(), process);
    }

    #[test]
    fn duplicate_launch_does_not_invoke_worker_factory() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e02");
        let (session, _receiver) = session::test_session();
        registry
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))
            .unwrap();
        let mut invoked = false;

        let result = registry.launch(terminal_id, || {
            invoked = true;
            Ok::<_, std::io::Error>(session::test_session().0)
        });

        assert!(matches!(result, Err(RegistryError::DuplicateTerminal)));
        assert!(!invoked);
    }

    #[test]
    fn reconnect_requires_an_exited_process() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e0b");
        registry
            .launch(terminal_id, || {
                Ok::<_, std::io::Error>(session::test_session().0)
            })
            .unwrap();
        let current = registry.target(terminal_id);
        let mut invoked = false;
        let result = registry.reconnect(terminal_id, |_| {
            invoked = true;
            Err(std::io::Error::other("存活进程重连不应执行工厂"))
        });
        assert!(result.is_err(), "存活进程不得被重连替换");
        assert!(!invoked);
        assert_eq!(registry.target(terminal_id), current);
    }

    #[test]
    fn reconnect_invalidates_the_previous_target_and_worker() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e03");
        let (first, _first_receiver) = session::test_session();
        let previous_target = registry
            .launch(terminal_id, || Ok::<_, std::io::Error>(first))
            .unwrap();
        registry.mark_process_exited(previous_target).unwrap();
        let mut second_receiver = None;

        let replacement = registry
            .reconnect(terminal_id, |terminal| {
                let (session, receiver, events) = session::test_reconnect_session(Some(terminal));
                second_receiver = Some(receiver);
                Ok::<_, std::io::Error>((session, events))
            })
            .unwrap();

        assert_eq!(replacement.target().generation().get(), 2);
        assert!(matches!(
            registry.send(previous_target, Msg::Input(Cow::Owned(b"stale".to_vec()))),
            Err(RegistryError::StaleGeneration)
        ));
        replacement
            .into_previous()
            .shutdown()
            .expect("旧 worker 必须显式关闭");
        registry
            .send(
                replacement_target(&registry, terminal_id),
                Msg::Input(Cow::Owned(b"current".to_vec())),
            )
            .unwrap();
        assert!(matches!(
            second_receiver.unwrap().recv(),
            Ok(Msg::Input(bytes)) if bytes.as_ref() == b"current"
        ));
    }

    fn replacement_target(registry: &TerminalRegistry, id: TerminalId) -> TerminalTarget {
        registry.target(id).expect("重连后必须存在当前目标")
    }

    #[test]
    fn exited_process_rejects_input_but_keeps_display_commands() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e0a");
        let (session, _receiver) = session::test_session();
        let target = registry
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))
            .unwrap();

        registry.mark_process_exited(target).unwrap();

        assert!(matches!(
            registry.send(target, Msg::Input(Cow::Owned(b"late".to_vec()))),
            Err(RegistryError::ProcessExited)
        ));
        registry
            .send(target, Msg::Scroll(slterm_terminal::grid::Scroll::Bottom))
            .expect("退出后仍须允许显示命令");
    }

    #[test]
    fn terminal_live_pty_limit() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = |index: usize| id(&format!("018f2f7c-8f5a-7c3d-9e2b-{index:012x}"));
        for index in 0..slterm_terminal::tty::MAX_PTY_SESSIONS {
            registry
                .launch(terminal_id(index), || {
                    Ok::<_, std::io::Error>(session::test_session().0)
                })
                .unwrap();
        }
        let rejected = terminal_id(slterm_terminal::tty::MAX_PTY_SESSIONS);
        let mut invoked = false;
        let result = registry.launch(rejected, || {
            invoked = true;
            Ok::<_, std::io::Error>(session::test_session().0)
        });
        assert!(result.is_err(), "第 65 个存活 PTY 必须被拒绝");
        assert!(!invoked, "达到存活上限时不得执行 worker 工厂");
        assert!(registry.target(rejected).is_none());
        assert!(
            registry.target(terminal_id(0)).is_some(),
            "既有记录不得丢失"
        );

        registry
            .mark_process_exited(registry.target(terminal_id(0)).unwrap())
            .unwrap();
        registry
            .launch(rejected, || {
                Ok::<_, std::io::Error>(session::test_session().0)
            })
            .expect("一个进程退出后必须释放重试额度");
    }

    #[test]
    fn reconnect_live_pty_limit_preserves_worker() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = |index: usize| id(&format!("018f2f7c-8f5a-7c3d-9e2b-{index:012x}"));
        for index in 0..slterm_terminal::tty::MAX_PTY_SESSIONS {
            registry
                .launch(terminal_id(index), || {
                    Ok::<_, std::io::Error>(session::test_session().0)
                })
                .unwrap();
        }
        let current = registry.target(terminal_id(0)).unwrap();
        registry.mark_process_exited(current).unwrap();
        registry
            .launch(terminal_id(slterm_terminal::tty::MAX_PTY_SESSIONS), || {
                Ok::<_, std::io::Error>(session::test_session().0)
            })
            .unwrap();
        let mut invoked = false;
        let result = registry.reconnect(terminal_id(0), |_| {
            invoked = true;
            Err(std::io::Error::other("达到上限时不应执行重连工厂"))
        });
        assert!(result.is_err(), "达到上限时不得让重连临时超发");
        assert!(!invoked);
        assert_eq!(registry.target(terminal_id(0)), Some(current));
    }

    #[test]
    fn generation_exhaustion_does_not_invoke_worker_factory() {
        let mut registry = TerminalRegistry::default();
        let terminal_id = id("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e04");
        let (session, _receiver) = session::test_session();
        registry
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))
            .unwrap();
        registry.entries.get_mut(&terminal_id).unwrap().generation = PtyGeneration(u64::MAX);
        let mut invoked = false;

        let result = registry.reconnect(terminal_id, |_| {
            invoked = true;
            Err(std::io::Error::other("存活进程重连不应执行工厂"))
        });

        assert!(matches!(result, Err(RegistryError::GenerationExhausted)));
        assert!(!invoked);
    }
}
