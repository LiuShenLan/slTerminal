//! run 记账(OSC 133 缝合):run id 单调分配与完成结果。
//!
//! 只认 OSC 133;D 携带的真实 exit code;CommandStart 未观测则如实标
//! `Unavailable`,绝不伪造 0。本文件独立成模块,避免主协议文件越过行数预算。

use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    ExitCodeCapability, RuntimePaneRun, RuntimeRunOutcome, RuntimeRunPhase, RuntimeRunState,
};

impl RuntimeRunOutcome {
    /// OSC 133;D 的完成结果。
    pub(crate) fn command_done(run: RuntimePaneRun, exit_code: Option<i32>) -> Self {
        if run.phase != RuntimeRunPhase::Started {
            return Self::unavailable(run.run_id, "command_start_not_observed");
        }
        match exit_code {
            Some(0) => Self {
                run_id: run.run_id,
                state: RuntimeRunState::Finished,
                exit_code,
                exit_code_capability: ExitCodeCapability::Supported,
                unavailable_reason: None,
            },
            Some(_) => Self {
                run_id: run.run_id,
                state: RuntimeRunState::Failed,
                exit_code,
                exit_code_capability: ExitCodeCapability::Supported,
                unavailable_reason: None,
            },
            None => Self::unavailable(run.run_id, "exit_code_unavailable"),
        }
    }
}

/// 新 run 的进程级单调编号。OSC 133 的两条边只认这个 id,等待方不会把
/// 上一条命令的完成误配给新请求。
pub(crate) fn begin_runtime_run() -> RuntimePaneRun {
    static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
    RuntimePaneRun {
        run_id: NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed),
        phase: RuntimeRunPhase::Submitted,
    }
}
