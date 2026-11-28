//! 工作区 writer 的退出最终提交：非消费式冲刷、显式重试／知情放弃决定与
//! 正常退出标记。只在 GPUI 外的全局关闭工作线程调用。

use super::{
    ControlPlaneCleanup, FinalCommitDecision, FinalShutdownOutcome, WorkspaceWriterRuntime,
};
use crate::persistence::workspace_writer::WorkspaceWriterError;

/// 最终提交失败后的用户决定；`Fail` 只承载决策面自身失败，不得被当作知情放弃。
enum FinalCommitRecovery {
    Retry,
    Abandon,
    Fail(WorkspaceWriterError),
}

impl WorkspaceWriterRuntime {
    #[cfg(test)]
    pub(super) fn shutdown(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
    ) -> Result<(), WorkspaceWriterError> {
        match self.shutdown_internal(control_plane_cleanup, |error| {
            FinalCommitRecovery::Fail(error)
        })? {
            FinalShutdownOutcome::Completed => Ok(()),
            FinalShutdownOutcome::Abandoned => {
                unreachable!("默认最终提交失败路径不得转为知情放弃")
            }
        }
    }

    #[cfg(test)]
    /// 最终提交失败由生产调用方决定重试或承担旧状态恢复后果。
    pub(super) fn shutdown_with_decision(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut decide: impl FnMut() -> FinalCommitDecision,
    ) -> Result<FinalShutdownOutcome, WorkspaceWriterError> {
        self.shutdown_with_fallible_decision(control_plane_cleanup, move || Ok(decide()))
    }

    /// 确认失败不是用户放弃；显式上抛并保持未正常退出。
    pub(super) fn shutdown_with_fallible_decision(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut decide: impl FnMut() -> Result<FinalCommitDecision, WorkspaceWriterError>,
    ) -> Result<FinalShutdownOutcome, WorkspaceWriterError> {
        self.shutdown_internal(control_plane_cleanup, move |_| match decide() {
            Ok(FinalCommitDecision::Retry) => FinalCommitRecovery::Retry,
            Ok(FinalCommitDecision::Abandon) => FinalCommitRecovery::Abandon,
            Err(error) => FinalCommitRecovery::Fail(error),
        })
    }

    fn shutdown_internal(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut on_final_error: impl FnMut(WorkspaceWriterError) -> FinalCommitRecovery,
    ) -> Result<FinalShutdownOutcome, WorkspaceWriterError> {
        let Self {
            writer,
            client: _,
            failure_forwarder,
            paths,
            metadata,
            previous_run: _,
        } = self;
        // 第一段是非消费式最终冲刷：失败时 writer 仍由本线程持有，可经同一
        // 显式重试／知情放弃决定重试；不得在此消费最后的重试入口。
        loop {
            match writer.flush() {
                Ok(report) if report.failed == 0 => break,
                Ok(_) => match on_final_error(WorkspaceWriterError::FlushFailed) {
                    FinalCommitRecovery::Retry => continue,
                    FinalCommitRecovery::Abandon => {
                        // 知情放弃：显式回收写线程，但不提交正常退出事实。
                        let _ = writer.shutdown();
                        let _ = failure_forwarder.join();
                        return Ok(FinalShutdownOutcome::Abandoned);
                    }
                    FinalCommitRecovery::Fail(error) => {
                        let _ = writer.shutdown();
                        let _ = failure_forwarder.join();
                        return Err(error);
                    }
                },
                Err(error) => {
                    // 写线程已不可用，无法保留可重试入口；按终态上抛。
                    let _ = failure_forwarder.join();
                    return Err(error);
                }
            }
        }
        let shutdown = writer.shutdown();
        let forwarder = failure_forwarder.join();
        let report = shutdown?;
        if report.failed > 0 {
            // 仅当预冲刷成功后仍有迟到写入失败才可达；writer 已消费，
            // 不能伪造成功或伪造可重试入口。
            return Err(WorkspaceWriterError::FlushFailed);
        }
        forwarder.map_err(|_| WorkspaceWriterError::Stopped)?;
        if control_plane_cleanup == ControlPlaneCleanup::Failed {
            return Err(WorkspaceWriterError::ControlPlaneCleanupFailed);
        }
        loop {
            let result = (|| {
                let mut store = crate::persistence::workspace::WorkspaceDatabase::open(&paths)
                    .map_err(WorkspaceWriterError::Open)?;
                store
                    .mark_clean_shutdown(&metadata.run.run_id)
                    .map_err(WorkspaceWriterError::Metadata)
            })();
            match result {
                Ok(()) => return Ok(FinalShutdownOutcome::Completed),
                Err(error) => match on_final_error(error) {
                    FinalCommitRecovery::Retry => {}
                    FinalCommitRecovery::Abandon => return Ok(FinalShutdownOutcome::Abandoned),
                    FinalCommitRecovery::Fail(error) => return Err(error),
                },
            }
        }
    }
}
