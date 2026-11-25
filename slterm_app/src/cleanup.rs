//! 清理服务：所有者提供候选与活跃代次，计划／执行两阶段。
//!
//! 只清理显式登记的候选文件；不枚举目录、不猜测删除。活跃恢复材料、
//! 设置、项目与进行中任务占用文件由所有者以活跃代次排除在计划之外。

use std::path::PathBuf;

/// 可清理类别；只有这些类别能进入计划。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CleanupCategory {
    /// 可安全重建的旧日志。
    OldLogs,
    /// 已完成且不再需要的更新下载与交接材料。
    FinishedUpdateDownloads,
    /// 不再承担恢复职责的崩溃现场。
    StaleCrashSites,
    /// 审计日志：必须与普通缓存分开显式确认。
    AuditLogs,
}

/// 所有者提供的候选；活跃代次由所有者给出，计划不得自行猜测。
#[derive(Clone, Debug)]
pub(crate) struct CleanupCandidate {
    pub(crate) category: CleanupCategory,
    pub(crate) owner: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) active_generation: Option<u64>,
    pub(crate) bytes: u64,
}

/// 两类确认分开：普通缓存与审计日志。
#[derive(Clone, Copy, Debug)]
pub(crate) struct CleanupRequest {
    pub(crate) caches: bool,
    pub(crate) audit_logs: bool,
}

/// 逐项结果；部分成功必须完整呈现。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CleanupOutcome {
    Removed {
        path: PathBuf,
        bytes: u64,
    },
    ExcludedActive {
        path: PathBuf,
    },
    SkippedUnsafe {
        path: PathBuf,
    },
    Failed {
        path: PathBuf,
        kind: std::io::ErrorKind,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CleanupReport {
    pub(crate) outcomes: Vec<CleanupOutcome>,
}

impl CleanupReport {
    pub(crate) fn removed_bytes(&self) -> u64 {
        self.outcomes
            .iter()
            .map(|outcome| match outcome {
                CleanupOutcome::Removed { bytes, .. } => *bytes,
                _ => 0,
            })
            .sum()
    }
}

pub(crate) struct CleanupService;

impl CleanupService {
    /// 计划阶段：只按类别开关过滤所有者候选，不做目录枚举。
    pub(crate) fn plan(
        candidates: Vec<CleanupCandidate>,
        request: CleanupRequest,
    ) -> Vec<CleanupCandidate> {
        candidates
            .into_iter()
            .filter(|candidate| match candidate.category {
                CleanupCategory::AuditLogs => request.audit_logs,
                _ => request.caches,
            })
            .collect()
    }

    /// 执行阶段：活跃代次匹配即排除；目录与未知 IO 失败逐项报告。
    pub(crate) fn execute(
        candidates: &[CleanupCandidate],
        current_generation: &dyn Fn(&CleanupCandidate) -> u64,
    ) -> CleanupReport {
        let mut report = CleanupReport::default();
        for candidate in candidates {
            if candidate
                .active_generation
                .is_some_and(|generation| generation == current_generation(candidate))
            {
                record(candidate, crate::audit::AuditResult::Suspended);
                report.outcomes.push(CleanupOutcome::ExcludedActive {
                    path: candidate.path.clone(),
                });
                continue;
            }
            if candidate.path.is_dir() {
                record(candidate, crate::audit::AuditResult::Suspended);
                report.outcomes.push(CleanupOutcome::SkippedUnsafe {
                    path: candidate.path.clone(),
                });
                continue;
            }
            match std::fs::remove_file(&candidate.path) {
                Ok(()) => {
                    record(candidate, crate::audit::AuditResult::Ok);
                    report.outcomes.push(CleanupOutcome::Removed {
                        path: candidate.path.clone(),
                        bytes: candidate.bytes,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    record(candidate, crate::audit::AuditResult::Acknowledged);
                    report.outcomes.push(CleanupOutcome::Removed {
                        path: candidate.path.clone(),
                        bytes: 0,
                    });
                }
                Err(error) => {
                    record(candidate, crate::audit::AuditResult::Error);
                    report.outcomes.push(CleanupOutcome::Failed {
                        path: candidate.path.clone(),
                        kind: error.kind(),
                    });
                }
            }
        }
        report
    }
}

fn record(candidate: &CleanupCandidate, result: crate::audit::AuditResult) {
    crate::audit::record(
        crate::audit::AuditEvent::new("cleanup", candidate.owner, result)
            .with_path(&candidate.path),
    );
}

#[cfg(test)]
mod cleanup_tests {
    use std::path::Path;

    use super::*;
    use crate::audit::test_support::capture_events;

    fn candidate(
        path: &Path,
        category: CleanupCategory,
        owner: &'static str,
        active_generation: Option<u64>,
    ) -> CleanupCandidate {
        CleanupCandidate {
            category,
            owner,
            path: path.to_owned(),
            active_generation,
            bytes: std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        }
    }

    fn write(path: &Path, body: &[u8]) {
        std::fs::write(path, body).unwrap();
    }

    /// 锚：清理排除活跃恢复材料且审计独立确认。
    #[test]
    fn cleanup_excludes_active_recovery_material() {
        let temp = tempfile::tempdir().unwrap();
        let active = temp.path().join("snapshot-active.bin");
        let stale = temp.path().join("app-old.log");
        let audit_log = temp.path().join("audit-old.log");
        write(&active, b"active recovery");
        write(&stale, b"old log");
        write(&audit_log, b"audit");

        let candidates = vec![
            candidate(
                &active,
                CleanupCategory::StaleCrashSites,
                "snapshot",
                Some(7),
            ),
            candidate(&stale, CleanupCategory::OldLogs, "logging", None),
            candidate(&audit_log, CleanupCategory::AuditLogs, "logging", None),
        ];
        let generations = |_: &CleanupCandidate| 7u64;

        let capture = capture_events();
        let planned = CleanupService::plan(
            candidates.clone(),
            CleanupRequest {
                caches: true,
                audit_logs: false,
            },
        );
        let report = CleanupService::execute(&planned, &generations);
        assert!(active.exists(), "活跃恢复材料必须排除");
        assert!(!stale.exists(), "非活跃旧日志应删除");
        assert!(audit_log.exists(), "审计日志必须独立确认");
        assert!(report.outcomes.iter().any(|outcome| matches!(
            outcome,
            CleanupOutcome::ExcludedActive { path } if path == &active
        )));
        assert!(report.outcomes.iter().all(|outcome| !matches!(
            outcome,
            CleanupOutcome::Removed { path, .. } if path == &audit_log
        )));
        let text = capture.text();
        assert!(text.contains("operation=\"cleanup\""), "{text}");
        assert!(text.contains("result=\"suspended\""), "{text}");
        drop(capture);

        // 显式确认审计日志后才删除。
        let report = CleanupService::execute(
            &CleanupService::plan(
                vec![candidate(
                    &audit_log,
                    CleanupCategory::AuditLogs,
                    "logging",
                    None,
                )],
                CleanupRequest {
                    caches: true,
                    audit_logs: true,
                },
            ),
            &generations,
        );
        assert!(!audit_log.exists());
        assert_eq!(report.removed_bytes(), 5);
    }

    /// 集成锚：诊断材料与清理互通，目录与活跃恢复材料不被删除。
    #[test]
    fn integration_diagnostics_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let app_log = logs.join("app.log");
        write(&app_log, b"old application log");
        let updates = temp.path().join("updates");
        std::fs::create_dir_all(&updates).unwrap();
        write(&updates.join("old-setup.exe"), b"MZold installer");
        let crash = temp.path().join("crash-site");
        std::fs::create_dir_all(&crash).unwrap();
        let recovery = temp.path().join("recovery-point.current");
        write(&recovery, b"active recovery");

        let candidates = vec![
            candidate(&app_log, CleanupCategory::OldLogs, "logging", None),
            candidate(
                &updates.join("old-setup.exe"),
                CleanupCategory::FinishedUpdateDownloads,
                "update",
                None,
            ),
            candidate(&crash, CleanupCategory::StaleCrashSites, "crash", None),
            candidate(
                &recovery,
                CleanupCategory::StaleCrashSites,
                "recovery",
                Some(2),
            ),
        ];
        let generations = |_: &CleanupCandidate| 2u64;
        let report = CleanupService::execute(
            &CleanupService::plan(
                candidates,
                CleanupRequest {
                    caches: true,
                    audit_logs: true,
                },
            ),
            &generations,
        );

        assert!(!app_log.exists());
        assert!(!updates.join("old-setup.exe").exists());
        assert!(crash.exists(), "目录候选不得被递归删除");
        assert!(!updates.exists() || updates.read_dir().unwrap().next().is_none());
        assert!(recovery.exists(), "活跃恢复材料保留");
        assert_eq!(report.outcomes.len(), 4, "逐项结果完整");

        // 诊断导出仍可用，清理不触诊断白名单。
        let mut diagnostics = crate::diagnostics::DiagnosticReport::new(
            crate::diagnostics::DiagnosticSnapshot::current(false, false),
        );
        diagnostics.push_event("cleanup finished");
        assert!(diagnostics.to_json().contains("cleanup finished"));
    }
}
