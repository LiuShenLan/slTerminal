use std::fmt;

/// Runtime 写操作的持久生命周期状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperationStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    OutcomeUnknown,
}

impl OperationStatus {
    pub const ALL: [Self; 6] = [
        Self::Queued,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::Cancelled,
        Self::OutcomeUnknown,
    ];

    /// 终态不可再转换，结果不明同样是可查询终态。
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
        )
    }

    /// 只沿登记的向前边转换，禁止跳过执行或从终态复活。
    pub fn transition(self, next: Self) -> Result<Self, InvalidOperationTransition> {
        if matches!(
            (self, next),
            (Self::Queued, Self::Running)
                | (Self::Running, Self::Succeeded)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Running, Self::OutcomeUnknown)
        ) {
            Ok(next)
        } else {
            Err(InvalidOperationTransition {
                from: self,
                to: next,
            })
        }
    }
}

/// 请求的操作状态边不属于封闭状态机。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidOperationTransition {
    from: OperationStatus,
    to: OperationStatus,
}

impl InvalidOperationTransition {
    pub const fn from(self) -> OperationStatus {
        self.from
    }

    pub const fn to(self) -> OperationStatus {
        self.to
    }
}

impl fmt::Display for InvalidOperationTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("操作状态只允许从排队进入执行，或从执行进入终态")
    }
}

impl std::error::Error for InvalidOperationTransition {}

#[cfg(test)]
mod operation_tests {
    use super::*;

    /// 全矩阵只接受 Queued→Running 与 Running→四种终态。
    #[test]
    fn operation_status_accepts_only_the_declared_forward_edges() {
        for from in OperationStatus::ALL {
            for to in OperationStatus::ALL {
                let allowed = matches!(
                    (from, to),
                    (OperationStatus::Queued, OperationStatus::Running)
                        | (OperationStatus::Running, OperationStatus::Succeeded)
                        | (OperationStatus::Running, OperationStatus::Failed)
                        | (OperationStatus::Running, OperationStatus::Cancelled)
                        | (OperationStatus::Running, OperationStatus::OutcomeUnknown)
                );
                assert_eq!(
                    from.transition(to).ok(),
                    allowed.then_some(to),
                    "{from:?} → {to:?}"
                );
            }
        }
    }

    /// 成功、失败、取消和结果不明都是终态，排队与执行中不是。
    #[test]
    fn operation_status_marks_every_terminal_state() {
        for status in OperationStatus::ALL {
            assert_eq!(
                status.is_terminal(),
                matches!(
                    status,
                    OperationStatus::Succeeded
                        | OperationStatus::Failed
                        | OperationStatus::Cancelled
                        | OperationStatus::OutcomeUnknown
                ),
                "{status:?}"
            );
        }
    }

    /// 非法转换保留两端状态，错误文本不依赖调试格式。
    #[test]
    fn invalid_operation_transition_describes_the_rejection() {
        fn require_error<T: std::error::Error>() {}
        require_error::<InvalidOperationTransition>();
        let error = OperationStatus::Queued
            .transition(OperationStatus::Succeeded)
            .unwrap_err();
        assert_eq!(error.from(), OperationStatus::Queued);
        assert_eq!(error.to(), OperationStatus::Succeeded);
        assert_eq!(
            error.to_string(),
            "操作状态只允许从排队进入执行，或从执行进入终态"
        );
    }
}
