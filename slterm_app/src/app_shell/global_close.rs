//! GPUI 外的全局退出所有者：停止控制面、回收 writer 并提交正常退出事实。

use std::sync::mpsc;

use super::{
    ControlPlaneCleanup, FinalCommitDecision, FinalShutdownOutcome, WorkspaceWriterRuntime,
    shutdown_local_control_service,
};
use crate::local_control::LocalControlService;
use crate::persistence::workspace_writer::WorkspaceWriterError;

const GLOBAL_CLOSE_CAPACITY: usize = 1;

/// 窗口只提交请求；控制面与 writer 始终由工作线程独占。
#[derive(Clone)]
pub(crate) struct GlobalCloseClient {
    requests: mpsc::SyncSender<GlobalCloseRequest>,
}

pub(crate) type GlobalCloseResponse =
    mpsc::Receiver<Result<FinalShutdownOutcome, GlobalCloseError>>;

struct GlobalCloseRequest {
    completion: mpsc::SyncSender<Result<FinalShutdownOutcome, GlobalCloseError>>,
    owner: Option<isize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlobalCloseRequestError {
    Full,
    Disconnected,
}

#[derive(Debug)]
pub(crate) enum GlobalCloseError {
    ControlPlaneCleanupFailed,
    Writer(WorkspaceWriterError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlobalCloseSummary {
    Completed,
    Abandoned,
    Failed,
}

pub(crate) struct GlobalCloseRuntime {
    client: GlobalCloseClient,
    worker: std::thread::JoinHandle<GlobalCloseSummary>,
}

impl GlobalCloseClient {
    /// 每次请求使用独立容量一回执，避免壳事件洪峰丢失退出结果。
    pub(crate) fn request(
        &self,
        owner: Option<isize>,
    ) -> Result<GlobalCloseResponse, GlobalCloseRequestError> {
        let (completion, response) = mpsc::sync_channel(1);
        self.requests
            .try_send(GlobalCloseRequest { completion, owner })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => GlobalCloseRequestError::Full,
                mpsc::TrySendError::Disconnected(_) => GlobalCloseRequestError::Disconnected,
            })?;
        Ok(response)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn disconnected_for_test() -> Self {
        let (requests, receiver) = mpsc::sync_channel(GLOBAL_CLOSE_CAPACITY);
        drop(receiver);
        Self { requests }
    }
}

type WorkerControl = Box<dyn FnMut() -> ControlPlaneCleanup + Send>;
type WorkerDecision =
    Box<dyn FnMut(Option<isize>) -> Result<FinalCommitDecision, WorkspaceWriterError> + Send>;
type WorkerParts = (WorkspaceWriterRuntime, WorkerControl, WorkerDecision);
type WorkerMain = Box<dyn FnOnce() -> GlobalCloseSummary + Send>;

impl GlobalCloseRuntime {
    pub(crate) fn start(
        runtime: WorkspaceWriterRuntime,
        mut local_control: Option<LocalControlService>,
    ) -> std::io::Result<Self> {
        let control: WorkerControl =
            Box::new(move || shutdown_local_control_service(local_control.as_mut()));
        let decide: WorkerDecision =
            Box::new(
                |owner| match crate::platform::close_dialog::retry_final_commit(owner) {
                    Ok(true) => Ok(FinalCommitDecision::Retry),
                    Ok(false) => Ok(FinalCommitDecision::Abandon),
                    Err(_) => Err(WorkspaceWriterError::FinalCommitDecisionFailed),
                },
            );
        Self::start_with_spawn((runtime, control, decide), spawn_worker)
    }

    #[cfg(test)]
    pub(crate) fn start_with_control(
        runtime: WorkspaceWriterRuntime,
        control: impl FnMut() -> ControlPlaneCleanup + Send + 'static,
        decide: impl FnMut() -> FinalCommitDecision + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::start_with_control_and_spawn(runtime, control, decide, spawn_worker)
    }

    #[cfg(test)]
    fn start_with_control_and_spawn(
        runtime: WorkspaceWriterRuntime,
        control: impl FnMut() -> ControlPlaneCleanup + Send + 'static,
        mut decide: impl FnMut() -> FinalCommitDecision + Send + 'static,
        spawn: impl FnOnce(WorkerMain) -> std::io::Result<std::thread::JoinHandle<GlobalCloseSummary>>,
    ) -> std::io::Result<Self> {
        let control: WorkerControl = Box::new(control);
        let decide: WorkerDecision = Box::new(move |_| Ok(decide()));
        Self::start_with_spawn((runtime, control, decide), spawn)
    }

    fn start_with_spawn(
        parts: WorkerParts,
        spawn: impl FnOnce(WorkerMain) -> std::io::Result<std::thread::JoinHandle<GlobalCloseSummary>>,
    ) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel(GLOBAL_CLOSE_CAPACITY);
        let (handoff, handoff_receiver) = mpsc::sync_channel(1);
        let main: WorkerMain = Box::new(move || match handoff_receiver.recv() {
            Ok((runtime, control, decide)) => run_worker(runtime, receiver, control, decide),
            Err(_) => GlobalCloseSummary::Failed,
        });
        let worker = match spawn(main) {
            Ok(worker) => worker,
            Err(error) => {
                cleanup_unstarted(parts);
                return Err(error);
            }
        };
        if let Err(error) = handoff.send(parts) {
            cleanup_unstarted(error.0);
            let _ = worker.join();
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "global close worker handoff failed",
            ));
        }
        Ok(Self {
            client: GlobalCloseClient { requests },
            worker,
        })
    }

    pub(crate) fn client(&self) -> GlobalCloseClient {
        self.client.clone()
    }

    /// 正常路径工作线程已完成；异常窗口退出时在断开后执行兜底回收。
    pub(crate) fn join(self) -> std::thread::Result<GlobalCloseSummary> {
        let Self { client, worker } = self;
        drop(client);
        worker.join()
    }
}

fn spawn_worker(main: WorkerMain) -> std::io::Result<std::thread::JoinHandle<GlobalCloseSummary>> {
    std::thread::Builder::new()
        .name("slterm-global-close".to_owned())
        .spawn(main)
}

fn cleanup_unstarted(parts: WorkerParts) {
    let (runtime, mut control, mut decide) = parts;
    let cleanup = control();
    match runtime.shutdown_with_fallible_decision(cleanup, || decide(None)) {
        Ok(_) => {}
        Err(error) => log::warn!("unstarted global close cleanup failed: {error}"),
    }
}

fn run_worker(
    runtime: WorkspaceWriterRuntime,
    requests: mpsc::Receiver<GlobalCloseRequest>,
    mut control: WorkerControl,
    mut decide: WorkerDecision,
) -> GlobalCloseSummary {
    let mut runtime = Some(runtime);
    while let Ok(request) = requests.recv() {
        if control() == ControlPlaneCleanup::Failed {
            let _ = request
                .completion
                .send(Err(GlobalCloseError::ControlPlaneCleanupFailed));
            continue;
        }
        let owner = request.owner;
        let outcome = runtime
            .take()
            .expect("writer 只能由全局关闭工作线程消费一次")
            .shutdown_with_fallible_decision(ControlPlaneCleanup::Succeeded, || decide(owner));
        let summary = summarize(&outcome);
        if let Err(error) = &outcome {
            log::warn!("workspace writer shutdown failed: {error}");
        }
        let _ = request
            .completion
            .send(outcome.map_err(GlobalCloseError::Writer));
        return summary;
    }

    // 异常窗口退出也兜底回收；只有控制面、writer 与最终提交全部成功才写正常退出。
    let control_plane_cleanup = control();
    let outcome = runtime
        .take()
        .expect("兜底关闭前 writer 必须仍由工作线程持有")
        .shutdown_with_fallible_decision(control_plane_cleanup, || decide(None));
    let summary = summarize(&outcome);
    match outcome {
        Ok(FinalShutdownOutcome::Abandoned) => {
            log::warn!("user abandoned final workspace state commit after explicit warning")
        }
        Err(error) => log::warn!("workspace writer fallback shutdown failed: {error}"),
        Ok(FinalShutdownOutcome::Completed) => {}
    }
    summary
}

fn summarize(outcome: &Result<FinalShutdownOutcome, WorkspaceWriterError>) -> GlobalCloseSummary {
    match outcome {
        Ok(FinalShutdownOutcome::Completed) => GlobalCloseSummary::Completed,
        Ok(FinalShutdownOutcome::Abandoned) => GlobalCloseSummary::Abandoned,
        Err(_) => GlobalCloseSummary::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_shell::{ControlPlaneCleanup, WorkspaceWriterRuntime};
    use crate::persistence::workspace::WorkspaceDatabase;
    use slterm_path::AppPaths;

    #[test]
    fn control_cleanup_failure_retries_before_writer_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let mut attempts = 0;
        let global = GlobalCloseRuntime::start_with_control(
            runtime,
            move || {
                attempts += 1;
                if attempts == 1 {
                    ControlPlaneCleanup::Failed
                } else {
                    ControlPlaneCleanup::Succeeded
                }
            },
            || FinalCommitDecision::Retry,
        )
        .unwrap();
        let client = global.client();

        let first = client.request(None).unwrap();
        assert!(matches!(
            first.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(Err(GlobalCloseError::ControlPlaneCleanupFailed))
        ));
        let second = client.request(None).unwrap();
        assert_eq!(
            second
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            FinalShutdownOutcome::Completed
        );
        drop(client);
        assert_eq!(global.join().unwrap(), GlobalCloseSummary::Completed);
        assert!(
            WorkspaceDatabase::open(&paths)
                .unwrap()
                .metadata()
                .unwrap()
                .unwrap()
                .clean_shutdown
        );
    }

    #[test]
    fn disconnected_window_still_runs_fallback_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let global = GlobalCloseRuntime::start_with_control(
            runtime,
            || ControlPlaneCleanup::Succeeded,
            || FinalCommitDecision::Abandon,
        )
        .unwrap();

        drop(global.client());
        assert_eq!(global.join().unwrap(), GlobalCloseSummary::Completed);
        assert!(
            WorkspaceDatabase::open(&paths)
                .unwrap()
                .metadata()
                .unwrap()
                .unwrap()
                .clean_shutdown
        );
    }

    #[test]
    fn request_channel_full_and_disconnect_are_explicit() {
        let (requests, receiver) = mpsc::sync_channel(GLOBAL_CLOSE_CAPACITY);
        let client = GlobalCloseClient { requests };
        let _first = client.request(None).unwrap();
        assert_eq!(
            client.request(None).unwrap_err(),
            GlobalCloseRequestError::Full
        );
        drop(receiver);
        assert_eq!(
            client.request(None).unwrap_err(),
            GlobalCloseRequestError::Disconnected
        );
    }

    #[test]
    fn writer_flush_failure_does_not_commit_clean_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_final_write BEFORE INSERT ON app_state \
                 BEGIN SELECT RAISE(ABORT, 'injected final write failure'); END;",
            )
            .unwrap();
        runtime
            .client()
            .save_sidebar_state(&slterm_sidebar::SidebarState::default())
            .unwrap();
        let global = GlobalCloseRuntime::start_with_control(
            runtime,
            || ControlPlaneCleanup::Succeeded,
            || FinalCommitDecision::Retry,
        )
        .unwrap();
        let client = global.client();

        let response = client.request(None).unwrap();
        assert!(matches!(
            response.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(Err(GlobalCloseError::Writer(_)))
        ));
        drop(client);
        assert_eq!(global.join().unwrap(), GlobalCloseSummary::Failed);
        fault
            .execute_batch("DROP TRIGGER reject_final_write")
            .unwrap();
        assert!(
            !WorkspaceDatabase::open(&paths)
                .unwrap()
                .metadata()
                .unwrap()
                .unwrap()
                .clean_shutdown
        );
    }

    #[test]
    fn spawn_failure_shuts_down_writer_and_failure_forwarder() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();

        let error = match GlobalCloseRuntime::start_with_control_and_spawn(
            runtime,
            || ControlPlaneCleanup::Succeeded,
            || FinalCommitDecision::Retry,
            |_| Err(std::io::Error::other("injected spawn failure")),
        ) {
            Ok(_) => panic!("注入的 spawn 失败不得启动全局所有者"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert!(
            WorkspaceDatabase::open(&paths)
                .unwrap()
                .metadata()
                .unwrap()
                .unwrap()
                .clean_shutdown
        );
        assert!(matches!(
            event_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn dialog_failure_is_not_abandoned_and_owner_reaches_dialog() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_clean_shutdown BEFORE UPDATE ON workspace_meta \
                 BEGIN SELECT RAISE(ABORT, 'injected metadata failure'); END;",
            )
            .unwrap();
        let control: WorkerControl = Box::new(|| ControlPlaneCleanup::Succeeded);
        let decide: WorkerDecision = Box::new(|owner| {
            assert_eq!(owner, Some(1234));
            Err(WorkspaceWriterError::FinalCommitDecisionFailed)
        });
        let global =
            GlobalCloseRuntime::start_with_spawn((runtime, control, decide), spawn_worker).unwrap();
        let client = global.client();

        let response = client.request(Some(1234)).unwrap();
        assert!(matches!(
            response.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(Err(GlobalCloseError::Writer(
                WorkspaceWriterError::FinalCommitDecisionFailed
            )))
        ));
        drop(client);
        assert_eq!(global.join().unwrap(), GlobalCloseSummary::Failed);
        fault
            .execute_batch("DROP TRIGGER reject_clean_shutdown")
            .unwrap();
        assert!(
            !WorkspaceDatabase::open(&paths)
                .unwrap()
                .metadata()
                .unwrap()
                .unwrap()
                .clean_shutdown
        );
    }
}
