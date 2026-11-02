//! 关闭作用域、关闭意图与纯阶段转换。

use std::collections::BTreeMap;
use std::fmt;

use crate::ProjectId;

/// 关闭请求冻结、提交和销毁的对象边界。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseScope {
    Panels { project_id: ProjectId },
    Project { project_id: ProjectId },
    ApplicationExit,
    UpdateHandoff,
}

impl CloseScope {
    fn is_global(&self) -> bool {
        matches!(self, Self::ApplicationExit | Self::UpdateHandoff)
    }
}

/// 收集阶段对单个对象确定的内容处置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDecision {
    Save,
    Discard,
    NotRequired,
}

/// 关闭对象的身份、输入代次与内容决定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseObject<O> {
    pub id: O,
    pub generation: u64,
    pub decision: CloseDecision,
}

/// 破坏动作前必须提交的完整关闭事实。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseIntent<O> {
    objects: BTreeMap<O, CloseObject<O>>,
    fingerprint: [u8; 32],
}

impl<O: Ord + Clone> CloseIntent<O> {
    pub fn new(objects: Vec<CloseObject<O>>, fingerprint: [u8; 32]) -> Result<Self, CloseError> {
        let mut collected = BTreeMap::new();
        for object in objects {
            if collected.insert(object.id.clone(), object).is_some() {
                return Err(CloseError::DuplicateObject);
            }
        }
        Ok(Self {
            objects: collected,
            fingerprint,
        })
    }

    pub fn objects(&self) -> impl Iterator<Item = &CloseObject<O>> {
        self.objects.values()
    }

    pub const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    fn requires_save(&self) -> bool {
        self.objects()
            .any(|object| object.decision == CloseDecision::Save)
    }
}

/// 运行或失败所在的关闭阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStage {
    Collect,
    Save,
    CommitState,
    StopPty,
    Cleanup,
    StopServices,
    CommitCleanShutdown,
}

/// 一次阶段执行的代次；端口结果必须原样回传，防止迟到结果污染新阶段或重试。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseAttempt {
    stage: CloseStage,
    sequence: u64,
}

impl CloseAttempt {
    pub const fn new(stage: CloseStage, sequence: u64) -> Self {
        Self { stage, sequence }
    }

    pub const fn stage(&self) -> CloseStage {
        self.stage
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// 可由 application 端口执行的阶段；对象相关阶段携带同一关闭意图。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseEffect<O> {
    Collect {
        attempt: CloseAttempt,
    },
    Save {
        attempt: CloseAttempt,
        intent: CloseIntent<O>,
    },
    CommitState {
        attempt: CloseAttempt,
        intent: CloseIntent<O>,
    },
    StopPty {
        attempt: CloseAttempt,
        intent: CloseIntent<O>,
    },
    Cleanup {
        attempt: CloseAttempt,
        intent: CloseIntent<O>,
    },
    StopServices {
        attempt: CloseAttempt,
    },
    CommitCleanShutdown {
        attempt: CloseAttempt,
    },
    Finish(CloseOutcome),
}

impl<O> CloseEffect<O> {
    /// 返回端口结果必须携带的执行代次；最终结果不再接受端口回执。
    pub const fn attempt(&self) -> Option<CloseAttempt> {
        match self {
            Self::Collect { attempt }
            | Self::Save { attempt, .. }
            | Self::CommitState { attempt, .. }
            | Self::StopPty { attempt, .. }
            | Self::Cleanup { attempt, .. }
            | Self::StopServices { attempt }
            | Self::CommitCleanShutdown { attempt } => Some(*attempt),
            Self::Finish(_) => None,
        }
    }
}

/// 关闭协调器的可观察阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePhase {
    Idle,
    Running(CloseStage),
    Failed(CloseStage),
    Completed,
}

/// 知情放弃旧窗口状态时必须展示的后果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAbandonWarning {
    OldLayoutAndSnapshotsMayRecover,
}

/// 关闭请求的最终结果；取消与放弃不得伪装成正常保存成功。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed,
    Cancelled,
    Abandoned(CloseAbandonWarning),
}

/// 关闭输入、结果代次或阶段转换错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseError {
    DuplicateObject,
    InvalidTransition,
    StaleAttempt,
    AttemptExhausted,
}

impl fmt::Display for CloseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateObject => formatter.write_str("关闭对象身份重复"),
            Self::InvalidTransition => formatter.write_str("关闭状态转换无效"),
            Self::StaleAttempt => formatter.write_str("关闭结果代次已过期"),
            Self::AttemptExhausted => formatter.write_str("关闭结果代次耗尽"),
        }
    }
}

impl std::error::Error for CloseError {}

/// 只管理关闭顺序、结果代次与失败恢复，不执行 I/O 或直接销毁进程资源。
pub struct CloseCoordinator<O> {
    scope: Option<CloseScope>,
    phase: ClosePhase,
    intent: Option<CloseIntent<O>>,
    outcome: Option<CloseOutcome>,
    attempt_sequence: u64,
}

impl<O> CloseCoordinator<O> {
    pub const fn new() -> Self {
        Self {
            scope: None,
            phase: ClosePhase::Idle,
            intent: None,
            outcome: None,
            attempt_sequence: 0,
        }
    }

    pub const fn phase(&self) -> ClosePhase {
        self.phase
    }

    pub const fn outcome(&self) -> Option<CloseOutcome> {
        self.outcome
    }

    pub fn scope(&self) -> Option<&CloseScope> {
        self.scope.as_ref()
    }

    pub fn intent(&self) -> Option<&CloseIntent<O>> {
        self.intent.as_ref()
    }

    /// 当前运行阶段接受的唯一结果代次；失败与完成状态不接受端口回执。
    pub const fn current_attempt(&self) -> Option<CloseAttempt> {
        match self.phase {
            ClosePhase::Running(stage) => Some(CloseAttempt {
                stage,
                sequence: self.attempt_sequence,
            }),
            _ => None,
        }
    }
}

impl<O> Default for CloseCoordinator<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Ord + Clone> CloseCoordinator<O> {
    /// 冻结关闭作用域并要求收集对象、代次与决定。
    pub fn begin(&mut self, scope: CloseScope) -> Result<CloseEffect<O>, CloseError> {
        if self.phase != ClosePhase::Idle {
            return Err(CloseError::InvalidTransition);
        }
        self.scope = Some(scope);
        self.phase = ClosePhase::Running(CloseStage::Collect);
        self.attempt_sequence = 0;
        self.effect_for(CloseStage::Collect)
    }

    /// 接收完整关闭意图；存在保存决定时先进入内容保存。
    pub fn collected(
        &mut self,
        attempt: CloseAttempt,
        intent: CloseIntent<O>,
    ) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::Collect, attempt)?;
        self.ensure_next_attempt()?;
        let next = if intent.requires_save() {
            CloseStage::Save
        } else {
            CloseStage::CommitState
        };
        self.intent = Some(intent);
        self.transition(next)
    }

    pub fn save_succeeded(&mut self, attempt: CloseAttempt) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::Save, attempt)?;
        self.transition(CloseStage::CommitState)
    }

    pub fn state_committed(&mut self, attempt: CloseAttempt) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::CommitState, attempt)?;
        self.transition(CloseStage::StopPty)
    }

    pub fn pty_stopped(&mut self, attempt: CloseAttempt) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::StopPty, attempt)?;
        self.transition(CloseStage::Cleanup)
    }

    pub fn recovery_cleaned(
        &mut self,
        attempt: CloseAttempt,
    ) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::Cleanup, attempt)?;
        if self.is_global() {
            self.transition(CloseStage::StopServices)
        } else {
            Ok(self.finish(CloseOutcome::Closed))
        }
    }

    pub fn services_stopped(
        &mut self,
        attempt: CloseAttempt,
    ) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::StopServices, attempt)?;
        if !self.is_global() {
            return Err(CloseError::InvalidTransition);
        }
        self.transition(CloseStage::CommitCleanShutdown)
    }

    pub fn clean_shutdown_committed(
        &mut self,
        attempt: CloseAttempt,
    ) -> Result<CloseEffect<O>, CloseError> {
        self.expect_attempt(CloseStage::CommitCleanShutdown, attempt)?;
        if !self.is_global() {
            return Err(CloseError::InvalidTransition);
        }
        Ok(self.finish(CloseOutcome::Closed))
    }

    /// 当前代次失败只保留现场，不推进到任何破坏动作；旧代次失败直接拒绝。
    pub fn fail(&mut self, attempt: CloseAttempt) -> Result<(), CloseError> {
        let ClosePhase::Running(stage) = self.phase else {
            return Err(CloseError::InvalidTransition);
        };
        self.expect_attempt(stage, attempt)?;
        self.phase = ClosePhase::Failed(stage);
        Ok(())
    }

    /// 从失败的同一阶段重试并签发新代次，关闭意图保持不变。
    pub fn retry(&mut self) -> Result<CloseEffect<O>, CloseError> {
        let ClosePhase::Failed(stage) = self.phase else {
            return Err(CloseError::InvalidTransition);
        };
        self.transition(stage)
    }

    /// 状态提交尚未发起，或提交已明确失败时，允许取消并恢复原运行状态。
    pub fn cancel(&mut self) -> Result<CloseEffect<O>, CloseError> {
        let allowed = matches!(
            self.phase,
            ClosePhase::Running(CloseStage::Collect | CloseStage::Save)
                | ClosePhase::Failed(
                    CloseStage::Collect | CloseStage::Save | CloseStage::CommitState,
                )
        );
        if !allowed {
            return Err(CloseError::InvalidTransition);
        }
        Ok(self.finish(CloseOutcome::Cancelled))
    }

    /// 应用退出的最终正常状态提交失败后，返回必须告知用户的恢复后果。
    pub fn abandon_warning(&self) -> Option<CloseAbandonWarning> {
        if self.phase == ClosePhase::Failed(CloseStage::CommitCleanShutdown)
            && self.scope.as_ref() == Some(&CloseScope::ApplicationExit)
        {
            Some(CloseAbandonWarning::OldLayoutAndSnapshotsMayRecover)
        } else {
            None
        }
    }

    /// 用户明确承担旧布局与快照恢复后果后，允许不提交正常退出状态而结束。
    pub fn abandon(&mut self) -> Result<CloseEffect<O>, CloseError> {
        let warning = self
            .abandon_warning()
            .ok_or(CloseError::InvalidTransition)?;
        Ok(self.finish(CloseOutcome::Abandoned(warning)))
    }

    fn expect_attempt(
        &self,
        expected_stage: CloseStage,
        attempt: CloseAttempt,
    ) -> Result<(), CloseError> {
        let Some(current) = self.current_attempt() else {
            return Err(CloseError::InvalidTransition);
        };
        if current != attempt {
            return Err(CloseError::StaleAttempt);
        }
        if current.stage() != expected_stage {
            return Err(CloseError::InvalidTransition);
        }
        Ok(())
    }

    fn ensure_next_attempt(&self) -> Result<u64, CloseError> {
        self.attempt_sequence
            .checked_add(1)
            .ok_or(CloseError::AttemptExhausted)
    }

    fn transition(&mut self, stage: CloseStage) -> Result<CloseEffect<O>, CloseError> {
        let sequence = self.ensure_next_attempt()?;
        self.phase = ClosePhase::Running(stage);
        self.attempt_sequence = sequence;
        self.effect_for(stage)
    }

    fn effect_for(&self, stage: CloseStage) -> Result<CloseEffect<O>, CloseError> {
        let attempt = self
            .current_attempt()
            .ok_or(CloseError::InvalidTransition)?;
        match stage {
            CloseStage::Collect => Ok(CloseEffect::Collect { attempt }),
            CloseStage::Save => Ok(CloseEffect::Save {
                attempt,
                intent: self.intent_clone()?,
            }),
            CloseStage::CommitState => Ok(CloseEffect::CommitState {
                attempt,
                intent: self.intent_clone()?,
            }),
            CloseStage::StopPty => Ok(CloseEffect::StopPty {
                attempt,
                intent: self.intent_clone()?,
            }),
            CloseStage::Cleanup => Ok(CloseEffect::Cleanup {
                attempt,
                intent: self.intent_clone()?,
            }),
            CloseStage::StopServices => Ok(CloseEffect::StopServices { attempt }),
            CloseStage::CommitCleanShutdown => Ok(CloseEffect::CommitCleanShutdown { attempt }),
        }
    }

    fn intent_clone(&self) -> Result<CloseIntent<O>, CloseError> {
        self.intent.clone().ok_or(CloseError::InvalidTransition)
    }

    fn is_global(&self) -> bool {
        self.scope.as_ref().is_some_and(CloseScope::is_global)
    }

    fn finish(&mut self, outcome: CloseOutcome) -> CloseEffect<O> {
        self.phase = ClosePhase::Completed;
        self.outcome = Some(outcome);
        CloseEffect::Finish(outcome)
    }
}

#[cfg(test)]
mod close_attempt_tests {
    use super::*;

    #[test]
    fn attempt_exhaustion_does_not_wrap_or_change_stage() {
        let mut coordinator = CloseCoordinator::<u32>::new();
        coordinator.begin(CloseScope::ApplicationExit).unwrap();
        coordinator.attempt_sequence = u64::MAX;
        let attempt = coordinator.current_attempt().unwrap();
        let intent = CloseIntent::new(Vec::new(), [0; 32]).unwrap();

        assert_eq!(
            coordinator.collected(attempt, intent),
            Err(CloseError::AttemptExhausted)
        );
        assert_eq!(
            coordinator.phase(),
            ClosePhase::Running(CloseStage::Collect)
        );
        assert_eq!(coordinator.current_attempt(), Some(attempt));
        assert!(coordinator.intent().is_none());
    }

    fn intent() -> CloseIntent<u32> {
        CloseIntent::new(
            vec![CloseObject {
                id: 1,
                generation: 1,
                decision: CloseDecision::Save,
            }],
            [1; 32],
        )
        .unwrap()
    }

    fn drive_to_stage(stage: CloseStage) -> CloseCoordinator<u32> {
        let mut coordinator = CloseCoordinator::new();
        coordinator.begin(CloseScope::ApplicationExit).unwrap();
        if stage == CloseStage::Collect {
            return coordinator;
        }
        let attempt = coordinator.current_attempt().unwrap();
        coordinator.collected(attempt, intent()).unwrap();
        if stage == CloseStage::Save {
            return coordinator;
        }
        coordinator
            .save_succeeded(coordinator.current_attempt().unwrap())
            .unwrap();
        if stage == CloseStage::CommitState {
            return coordinator;
        }
        coordinator
            .state_committed(coordinator.current_attempt().unwrap())
            .unwrap();
        if stage == CloseStage::StopPty {
            return coordinator;
        }
        coordinator
            .pty_stopped(coordinator.current_attempt().unwrap())
            .unwrap();
        if stage == CloseStage::Cleanup {
            return coordinator;
        }
        coordinator
            .recovery_cleaned(coordinator.current_attempt().unwrap())
            .unwrap();
        if stage == CloseStage::StopServices {
            return coordinator;
        }
        coordinator
            .services_stopped(coordinator.current_attempt().unwrap())
            .unwrap();
        coordinator
    }

    fn advance(
        coordinator: &mut CloseCoordinator<u32>,
        stage: CloseStage,
        attempt: CloseAttempt,
    ) -> Result<CloseEffect<u32>, CloseError> {
        match stage {
            CloseStage::Save => coordinator.save_succeeded(attempt),
            CloseStage::CommitState => coordinator.state_committed(attempt),
            CloseStage::StopPty => coordinator.pty_stopped(attempt),
            CloseStage::Cleanup => coordinator.recovery_cleaned(attempt),
            CloseStage::StopServices => coordinator.services_stopped(attempt),
            _ => panic!("stage does not advance: {stage:?}"),
        }
    }

    #[test]
    fn retry_exhaustion_preserves_failed_stage() {
        for stage in [
            CloseStage::Collect,
            CloseStage::Save,
            CloseStage::CommitState,
            CloseStage::StopPty,
            CloseStage::Cleanup,
            CloseStage::StopServices,
            CloseStage::CommitCleanShutdown,
        ] {
            let mut coordinator = drive_to_stage(stage);
            coordinator.attempt_sequence = u64::MAX;
            let attempt = coordinator.current_attempt().unwrap();
            coordinator.fail(attempt).unwrap();

            assert_eq!(coordinator.retry(), Err(CloseError::AttemptExhausted));
            assert_eq!(coordinator.phase(), ClosePhase::Failed(stage));
            assert_eq!(coordinator.attempt_sequence, u64::MAX);
            assert_eq!(coordinator.current_attempt(), None);
        }
    }

    #[test]
    fn transition_exhaustion_preserves_running_stage() {
        for stage in [
            CloseStage::Save,
            CloseStage::CommitState,
            CloseStage::StopPty,
            CloseStage::Cleanup,
            CloseStage::StopServices,
        ] {
            let mut coordinator = drive_to_stage(stage);
            coordinator.attempt_sequence = u64::MAX;
            let attempt = coordinator.current_attempt().unwrap();

            assert_eq!(
                advance(&mut coordinator, stage, attempt),
                Err(CloseError::AttemptExhausted)
            );
            assert_eq!(coordinator.phase(), ClosePhase::Running(stage));
            assert_eq!(coordinator.attempt_sequence, u64::MAX);
            assert_eq!(coordinator.current_attempt(), Some(attempt));
        }
    }
}
