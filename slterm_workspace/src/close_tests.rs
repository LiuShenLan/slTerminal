//! 关闭作用域、阶段转换与结果代次测试。

use std::str::FromStr;

use crate::ProjectId;
use crate::close::{
    CloseAbandonWarning, CloseAttempt, CloseCoordinator, CloseDecision, CloseEffect, CloseError,
    CloseIntent, CloseObject, CloseOutcome, ClosePhase, CloseScope, CloseStage,
};

fn project_id() -> ProjectId {
    ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap()
}

fn object(id: u32, decision: CloseDecision) -> CloseObject<u32> {
    CloseObject {
        id,
        generation: 100 + u64::from(id),
        decision,
    }
}

fn intent(decisions: &[CloseDecision]) -> CloseIntent<u32> {
    let objects = decisions
        .iter()
        .enumerate()
        .map(|(index, decision)| object(index as u32 + 1, *decision))
        .collect();
    CloseIntent::new(objects, [7; 32]).unwrap()
}

fn current(coordinator: &CloseCoordinator<u32>) -> CloseAttempt {
    coordinator.current_attempt().unwrap()
}

fn effect_attempt(effect: &CloseEffect<u32>) -> CloseAttempt {
    effect.attempt().unwrap()
}

fn begin(scope: CloseScope) -> CloseCoordinator<u32> {
    let mut coordinator = CloseCoordinator::new();
    let effect = coordinator.begin(scope).unwrap();
    let attempt = effect_attempt(&effect);
    assert_eq!(effect, CloseEffect::Collect { attempt });
    coordinator
}

fn collect_current(
    coordinator: &mut CloseCoordinator<u32>,
    intent: CloseIntent<u32>,
) -> CloseEffect<u32> {
    let attempt = current(coordinator);
    coordinator.collected(attempt, intent).unwrap()
}

fn drive_to_stage(scope: CloseScope, stage: CloseStage) -> CloseCoordinator<u32> {
    let mut coordinator = begin(scope);
    if stage == CloseStage::Collect {
        return coordinator;
    }
    collect_current(&mut coordinator, intent(&[CloseDecision::Save]));
    if stage == CloseStage::Save {
        return coordinator;
    }
    coordinator.save_succeeded(current(&coordinator)).unwrap();
    if stage == CloseStage::CommitState {
        return coordinator;
    }
    coordinator.state_committed(current(&coordinator)).unwrap();
    if stage == CloseStage::StopPty {
        return coordinator;
    }
    coordinator.pty_stopped(current(&coordinator)).unwrap();
    if stage == CloseStage::Cleanup {
        return coordinator;
    }
    coordinator.recovery_cleaned(current(&coordinator)).unwrap();
    if stage == CloseStage::StopServices {
        return coordinator;
    }
    coordinator.services_stopped(current(&coordinator)).unwrap();
    if stage == CloseStage::CommitCleanShutdown {
        return coordinator;
    }
    panic!("unsupported close stage: {stage:?}");
}

fn drive_to_clean_shutdown(scope: CloseScope) -> CloseCoordinator<u32> {
    drive_to_stage(scope, CloseStage::CommitCleanShutdown)
}

fn effect_for_stage(
    stage: CloseStage,
    attempt: CloseAttempt,
    intent: &CloseIntent<u32>,
) -> CloseEffect<u32> {
    match stage {
        CloseStage::Collect => CloseEffect::Collect { attempt },
        CloseStage::Save => CloseEffect::Save {
            attempt,
            intent: intent.clone(),
        },
        CloseStage::CommitState => CloseEffect::CommitState {
            attempt,
            intent: intent.clone(),
        },
        CloseStage::StopPty => CloseEffect::StopPty {
            attempt,
            intent: intent.clone(),
        },
        CloseStage::Cleanup => CloseEffect::Cleanup {
            attempt,
            intent: intent.clone(),
        },
        CloseStage::StopServices => CloseEffect::StopServices { attempt },
        CloseStage::CommitCleanShutdown => CloseEffect::CommitCleanShutdown { attempt },
    }
}

fn complete_current_attempt(
    coordinator: &mut CloseCoordinator<u32>,
    stage: CloseStage,
    attempt: CloseAttempt,
) -> Result<CloseEffect<u32>, CloseError> {
    match stage {
        CloseStage::Collect => coordinator.collected(attempt, intent(&[CloseDecision::Save])),
        CloseStage::Save => coordinator.save_succeeded(attempt),
        CloseStage::CommitState => coordinator.state_committed(attempt),
        CloseStage::StopPty => coordinator.pty_stopped(attempt),
        CloseStage::Cleanup => coordinator.recovery_cleaned(attempt),
        CloseStage::StopServices => coordinator.services_stopped(attempt),
        CloseStage::CommitCleanShutdown => coordinator.clean_shutdown_committed(attempt),
    }
}

#[test]
fn close_intent_rejects_duplicate_objects() {
    let error = CloseIntent::new(
        vec![
            object(1, CloseDecision::Save),
            object(1, CloseDecision::Discard),
        ],
        [9; 32],
    )
    .unwrap_err();

    assert_eq!(error, CloseError::DuplicateObject);
}

#[test]
fn close_intent_preserves_identity_generation_decisions_and_fingerprint() {
    let intent = intent(&[
        CloseDecision::Save,
        CloseDecision::Discard,
        CloseDecision::NotRequired,
    ]);

    assert_eq!(intent.objects().count(), 3);
    let second = intent.objects().nth(1).unwrap();
    assert_eq!(second.id, 2);
    assert_eq!(second.generation, 102);
    assert_eq!(second.decision, CloseDecision::Discard);
    assert_eq!(intent.fingerprint(), &[7; 32]);
}

#[test]
fn application_close_follows_global_sequence() {
    let mut coordinator = begin(CloseScope::ApplicationExit);
    assert_eq!(
        coordinator.phase(),
        ClosePhase::Running(CloseStage::Collect)
    );
    assert_eq!(coordinator.scope(), Some(&CloseScope::ApplicationExit));
    let collect_attempt = current(&coordinator);
    let intent = intent(&[CloseDecision::Save]);

    let save = coordinator
        .collected(collect_attempt, intent.clone())
        .unwrap();
    let save_attempt = effect_attempt(&save);
    assert_eq!(save_attempt.stage(), CloseStage::Save);
    assert!(save_attempt.sequence() > collect_attempt.sequence());
    assert_eq!(
        save,
        CloseEffect::Save {
            attempt: save_attempt,
            intent: intent.clone(),
        }
    );
    assert_eq!(coordinator.intent(), Some(&intent));

    let commit = coordinator.save_succeeded(save_attempt).unwrap();
    let commit_attempt = effect_attempt(&commit);
    assert_eq!(
        commit,
        CloseEffect::CommitState {
            attempt: commit_attempt,
            intent: intent.clone(),
        }
    );
    let stop = coordinator.state_committed(commit_attempt).unwrap();
    let stop_attempt = effect_attempt(&stop);
    assert_eq!(
        stop,
        CloseEffect::StopPty {
            attempt: stop_attempt,
            intent: intent.clone(),
        }
    );
    let cleanup = coordinator.pty_stopped(stop_attempt).unwrap();
    let cleanup_attempt = effect_attempt(&cleanup);
    assert_eq!(
        cleanup,
        CloseEffect::Cleanup {
            attempt: cleanup_attempt,
            intent,
        }
    );
    let services = coordinator.recovery_cleaned(cleanup_attempt).unwrap();
    let services_attempt = effect_attempt(&services);
    assert_eq!(
        services,
        CloseEffect::StopServices {
            attempt: services_attempt
        }
    );
    let clean = coordinator.services_stopped(services_attempt).unwrap();
    let clean_attempt = effect_attempt(&clean);
    assert_eq!(
        clean,
        CloseEffect::CommitCleanShutdown {
            attempt: clean_attempt
        }
    );

    let finish = coordinator.clean_shutdown_committed(clean_attempt).unwrap();
    assert_eq!(finish.attempt(), None);
    assert_eq!(finish, CloseEffect::Finish(CloseOutcome::Closed));
    assert_eq!(coordinator.phase(), ClosePhase::Completed);
    assert_eq!(coordinator.current_attempt(), None);
    assert_eq!(coordinator.outcome(), Some(CloseOutcome::Closed));
}

#[test]
fn project_close_skips_save_and_global_shutdown() {
    let mut coordinator = begin(CloseScope::Project {
        project_id: project_id(),
    });
    let intent = intent(&[CloseDecision::Discard, CloseDecision::NotRequired]);

    let commit = collect_current(&mut coordinator, intent.clone());
    let commit_attempt = effect_attempt(&commit);
    assert_eq!(
        commit,
        CloseEffect::CommitState {
            attempt: commit_attempt,
            intent: intent.clone(),
        }
    );
    let stop = coordinator.state_committed(commit_attempt).unwrap();
    let stop_attempt = effect_attempt(&stop);
    assert_eq!(
        stop,
        CloseEffect::StopPty {
            attempt: stop_attempt,
            intent: intent.clone(),
        }
    );
    let cleanup = coordinator.pty_stopped(stop_attempt).unwrap();
    let cleanup_attempt = effect_attempt(&cleanup);
    assert_eq!(
        cleanup,
        CloseEffect::Cleanup {
            attempt: cleanup_attempt,
            intent,
        }
    );
    assert_eq!(
        coordinator.recovery_cleaned(cleanup_attempt),
        Ok(CloseEffect::Finish(CloseOutcome::Closed))
    );
    assert_eq!(coordinator.outcome(), Some(CloseOutcome::Closed));
}

#[test]
fn mixed_close_decisions_still_require_save() {
    let mut coordinator = begin(CloseScope::ApplicationExit);
    let intent = intent(&[
        CloseDecision::NotRequired,
        CloseDecision::Discard,
        CloseDecision::Save,
    ]);

    let effect = collect_current(&mut coordinator, intent.clone());
    let attempt = effect_attempt(&effect);
    assert_eq!(effect, CloseEffect::Save { attempt, intent });
}

#[test]
fn empty_close_intent_commits_without_save() {
    let mut coordinator = begin(CloseScope::ApplicationExit);
    let intent = intent(&[]);

    let effect = collect_current(&mut coordinator, intent.clone());
    let attempt = effect_attempt(&effect);
    assert_eq!(effect, CloseEffect::CommitState { attempt, intent });
}

#[test]
fn panels_close_saves_without_global_shutdown() {
    let mut coordinator = begin(CloseScope::Panels {
        project_id: project_id(),
    });
    let intent = intent(&[CloseDecision::Save]);

    let save = collect_current(&mut coordinator, intent.clone());
    let save_attempt = effect_attempt(&save);
    assert_eq!(
        save,
        CloseEffect::Save {
            attempt: save_attempt,
            intent: intent.clone(),
        }
    );
    let commit = coordinator.save_succeeded(save_attempt).unwrap();
    let commit_attempt = effect_attempt(&commit);
    assert_eq!(
        commit,
        CloseEffect::CommitState {
            attempt: commit_attempt,
            intent: intent.clone(),
        }
    );
    let stop = coordinator.state_committed(commit_attempt).unwrap();
    let stop_attempt = effect_attempt(&stop);
    assert_eq!(
        stop,
        CloseEffect::StopPty {
            attempt: stop_attempt,
            intent: intent.clone(),
        }
    );
    let cleanup = coordinator.pty_stopped(stop_attempt).unwrap();
    let cleanup_attempt = effect_attempt(&cleanup);
    assert_eq!(
        cleanup,
        CloseEffect::Cleanup {
            attempt: cleanup_attempt,
            intent,
        }
    );
    assert_eq!(
        coordinator.recovery_cleaned(cleanup_attempt),
        Ok(CloseEffect::Finish(CloseOutcome::Closed))
    );
}

#[test]
fn close_failure_retries_every_stage() {
    for stage in [
        CloseStage::Collect,
        CloseStage::Save,
        CloseStage::CommitState,
        CloseStage::StopPty,
        CloseStage::Cleanup,
        CloseStage::StopServices,
        CloseStage::CommitCleanShutdown,
    ] {
        let mut coordinator = drive_to_stage(CloseScope::ApplicationExit, stage);
        let previous_attempt = current(&coordinator);
        coordinator.fail(previous_attempt).unwrap();
        assert_eq!(coordinator.phase(), ClosePhase::Failed(stage));
        assert_eq!(coordinator.current_attempt(), None);
        assert_eq!(coordinator.outcome(), None);

        let effect = coordinator.retry().unwrap();
        let attempt = effect_attempt(&effect);
        assert!(attempt.sequence() > previous_attempt.sequence());
        assert_eq!(
            effect,
            effect_for_stage(stage, attempt, &intent(&[CloseDecision::Save]))
        );
        assert_eq!(coordinator.phase(), ClosePhase::Running(stage));
    }
}

#[test]
fn application_close_rejects_abandon_before_final_commit_failure() {
    for stage in [
        CloseStage::Collect,
        CloseStage::Save,
        CloseStage::CommitState,
        CloseStage::StopPty,
        CloseStage::Cleanup,
        CloseStage::StopServices,
    ] {
        let mut coordinator = drive_to_stage(CloseScope::ApplicationExit, stage);
        coordinator.fail(current(&coordinator)).unwrap();
        assert_eq!(coordinator.abandon_warning(), None);
        assert_eq!(coordinator.abandon(), Err(CloseError::InvalidTransition));
        assert_eq!(coordinator.phase(), ClosePhase::Failed(stage));
    }
}

#[test]
fn close_cancel_restores_only_before_state_commit() {
    let mut collecting = begin(CloseScope::ApplicationExit);
    assert_eq!(
        collecting.cancel(),
        Ok(CloseEffect::Finish(CloseOutcome::Cancelled))
    );

    let mut saving = begin(CloseScope::ApplicationExit);
    collect_current(&mut saving, intent(&[CloseDecision::Save]));
    assert_eq!(
        saving.cancel(),
        Ok(CloseEffect::Finish(CloseOutcome::Cancelled))
    );

    let mut failed_commit = begin(CloseScope::ApplicationExit);
    collect_current(&mut failed_commit, intent(&[CloseDecision::Discard]));
    assert_eq!(failed_commit.cancel(), Err(CloseError::InvalidTransition));
    assert_eq!(
        failed_commit.phase(),
        ClosePhase::Running(CloseStage::CommitState)
    );
    assert_eq!(failed_commit.outcome(), None);
    failed_commit.fail(current(&failed_commit)).unwrap();
    assert_eq!(
        failed_commit.cancel(),
        Ok(CloseEffect::Finish(CloseOutcome::Cancelled))
    );

    let mut committed = begin(CloseScope::ApplicationExit);
    collect_current(&mut committed, intent(&[CloseDecision::Discard]));
    committed.state_committed(current(&committed)).unwrap();
    assert_eq!(committed.cancel(), Err(CloseError::InvalidTransition));
}

#[test]
fn close_cancel_covers_failure_stage_matrix() {
    for stage in [
        CloseStage::Collect,
        CloseStage::Save,
        CloseStage::CommitState,
    ] {
        let mut coordinator = drive_to_stage(CloseScope::ApplicationExit, stage);
        coordinator.fail(current(&coordinator)).unwrap();
        assert_eq!(
            coordinator.cancel(),
            Ok(CloseEffect::Finish(CloseOutcome::Cancelled))
        );
        assert_eq!(coordinator.outcome(), Some(CloseOutcome::Cancelled));
    }

    for stage in [
        CloseStage::StopPty,
        CloseStage::Cleanup,
        CloseStage::StopServices,
        CloseStage::CommitCleanShutdown,
    ] {
        let mut coordinator = drive_to_stage(CloseScope::ApplicationExit, stage);
        coordinator.fail(current(&coordinator)).unwrap();
        assert_eq!(coordinator.cancel(), Err(CloseError::InvalidTransition));
        assert_eq!(coordinator.phase(), ClosePhase::Failed(stage));
        assert_eq!(coordinator.outcome(), None);
    }
}

#[test]
fn final_commit_in_progress_rejects_abandon() {
    let mut coordinator = drive_to_clean_shutdown(CloseScope::ApplicationExit);

    assert_eq!(coordinator.abandon_warning(), None);
    assert_eq!(coordinator.abandon(), Err(CloseError::InvalidTransition));
    assert_eq!(
        coordinator.phase(),
        ClosePhase::Running(CloseStage::CommitCleanShutdown)
    );
    assert_eq!(coordinator.outcome(), None);
}

#[test]
fn close_final_commit_failure_allows_explicit_abandon() {
    let mut coordinator = drive_to_clean_shutdown(CloseScope::ApplicationExit);
    coordinator.fail(current(&coordinator)).unwrap();

    assert_eq!(
        coordinator.abandon(),
        Ok(CloseEffect::Finish(CloseOutcome::Abandoned(
            CloseAbandonWarning::OldLayoutAndSnapshotsMayRecover,
        )))
    );
    assert_eq!(
        coordinator.outcome(),
        Some(CloseOutcome::Abandoned(
            CloseAbandonWarning::OldLayoutAndSnapshotsMayRecover,
        ))
    );
}

#[test]
fn close_abandon_warns_about_old_state() {
    let mut coordinator = drive_to_clean_shutdown(CloseScope::ApplicationExit);
    coordinator.fail(current(&coordinator)).unwrap();

    assert_eq!(
        coordinator.abandon_warning(),
        Some(CloseAbandonWarning::OldLayoutAndSnapshotsMayRecover)
    );
}

#[test]
fn update_handoff_failure_requires_retry_instead_of_abandon() {
    let mut coordinator = drive_to_clean_shutdown(CloseScope::UpdateHandoff);
    coordinator.fail(current(&coordinator)).unwrap();

    assert_eq!(coordinator.abandon_warning(), None);
    assert_eq!(coordinator.abandon(), Err(CloseError::InvalidTransition));
    let retry = coordinator.retry().unwrap();
    let attempt = effect_attempt(&retry);
    assert_eq!(retry, CloseEffect::CommitCleanShutdown { attempt });
    assert_eq!(
        coordinator.clean_shutdown_committed(attempt),
        Ok(CloseEffect::Finish(CloseOutcome::Closed))
    );
}

#[test]
fn stale_save_failure_does_not_fail_commit_stage() {
    let mut coordinator = begin(CloseScope::ApplicationExit);
    let save = collect_current(&mut coordinator, intent(&[CloseDecision::Save]));
    let save_attempt = effect_attempt(&save);
    coordinator.save_succeeded(save_attempt).unwrap();

    assert_eq!(
        coordinator.fail(save_attempt),
        Err(CloseError::StaleAttempt)
    );
    assert_eq!(
        coordinator.phase(),
        ClosePhase::Running(CloseStage::CommitState)
    );
    assert_eq!(coordinator.outcome(), None);
    assert_eq!(coordinator.cancel(), Err(CloseError::InvalidTransition));
}

#[test]
fn stale_attempt_results_do_not_affect_retry() {
    let mut coordinator = begin(CloseScope::ApplicationExit);
    let save = collect_current(&mut coordinator, intent(&[CloseDecision::Save]));
    let first_attempt = effect_attempt(&save);
    coordinator.fail(first_attempt).unwrap();
    let retry = coordinator.retry().unwrap();
    let retry_attempt = effect_attempt(&retry);
    assert!(retry_attempt.sequence() > first_attempt.sequence());

    assert_eq!(
        coordinator.save_succeeded(first_attempt),
        Err(CloseError::StaleAttempt)
    );
    assert_eq!(
        coordinator.fail(first_attempt),
        Err(CloseError::StaleAttempt)
    );
    assert_eq!(coordinator.phase(), ClosePhase::Running(CloseStage::Save));
    assert!(matches!(
        coordinator.save_succeeded(retry_attempt),
        Ok(CloseEffect::CommitState { .. })
    ));
}

#[test]
fn stale_attempt_is_rejected_by_every_completion_entry() {
    for stage in [
        CloseStage::Collect,
        CloseStage::Save,
        CloseStage::CommitState,
        CloseStage::StopPty,
        CloseStage::Cleanup,
        CloseStage::StopServices,
        CloseStage::CommitCleanShutdown,
    ] {
        let mut coordinator = drive_to_stage(CloseScope::ApplicationExit, stage);
        let stale = current(&coordinator);
        coordinator.fail(stale).unwrap();
        coordinator.retry().unwrap();

        assert_eq!(coordinator.fail(stale), Err(CloseError::StaleAttempt));
        assert_eq!(
            complete_current_attempt(&mut coordinator, stage, stale),
            Err(CloseError::StaleAttempt)
        );
        assert_eq!(coordinator.phase(), ClosePhase::Running(stage));
        assert_eq!(coordinator.outcome(), None);
    }
}

#[test]
fn invalid_transitions_preserve_current_phase() {
    let mut coordinator = CloseCoordinator::<u32>::new();
    assert_eq!(
        coordinator.save_succeeded(CloseAttempt::new(CloseStage::Save, 0)),
        Err(CloseError::InvalidTransition)
    );
    assert_eq!(coordinator.phase(), ClosePhase::Idle);

    let mut coordinator = begin(CloseScope::ApplicationExit);
    assert_eq!(
        coordinator.begin(CloseScope::ApplicationExit),
        Err(CloseError::InvalidTransition)
    );
    assert_eq!(
        coordinator.state_committed(current(&coordinator)),
        Err(CloseError::InvalidTransition)
    );
    assert_eq!(coordinator.retry(), Err(CloseError::InvalidTransition));
    assert_eq!(
        coordinator.phase(),
        ClosePhase::Running(CloseStage::Collect)
    );

    let attempt = current(&coordinator);
    coordinator.fail(attempt).unwrap();
    assert_eq!(
        coordinator.fail(attempt),
        Err(CloseError::InvalidTransition)
    );
    assert_eq!(coordinator.phase(), ClosePhase::Failed(CloseStage::Collect));
}
