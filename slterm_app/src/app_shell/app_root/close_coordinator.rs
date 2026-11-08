//! 应用关闭协调：终态移交、快照代次与窗口销毁的单一入口。

use gpui::{Context, Window};

use crate::persistence::workspace_writer::{
    WorkspaceFlushReport, WorkspaceWriterClient, WorkspaceWriterError,
};

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};

impl AppRoot {
    /// 关窗与托盘退出共用；最新快照代次成功前绝不销毁窗口。
    pub(crate) fn request_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        self.start_application_close(window, cx)
    }

    pub(super) fn start_close_flush(
        &mut self,
        client: WorkspaceWriterClient,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let reconcile_required = match self.side_view_persistence.submit_final_states(
            &self.side_views,
            &self.snapshot,
            cx,
        ) {
            Ok(reconcile_required) => reconcile_required,
            Err(error) => {
                self.close_in_progress = false;
                self.side_view_persistence
                    .resume_checkpoint_for_close_retry(&self.side_views, cx);
                log::warn!("关闭终态移交失败: {error}");
                super::events::notify_workspace_write_failed(cx);
                return false;
            }
        };
        let generation = self.snapshot_generation;
        let response = match client.flush() {
            Ok(response) => response,
            Err(error) => {
                self.close_in_progress = false;
                self.side_view_persistence
                    .resume_checkpoint_for_close_retry(&self.side_views, cx);
                log::warn!("关闭冲刷请求失败: {error}");
                super::events::notify_workspace_write_failed(cx);
                return false;
            }
        };
        let cancellation = CancellationToken::new();
        let flush = cx
            .background_executor()
            .spawn(async move { response.recv().map_err(|_| WorkspaceWriterError::Stopped) });
        let completion = cx.spawn(async move |this, cx| {
            let result = flush.await;
            let _ = this.update(cx, |root, cx| {
                root.finish_close(result, generation, reconcile_required, window, cx)
            });
        });
        if let Err(error) = self
            .tasks
            .register(TaskId::CloseFlush, cancellation, completion)
        {
            self.close_in_progress = false;
            self.side_view_persistence
                .resume_checkpoint_for_close_retry(&self.side_views, cx);
            log::warn!("关闭冲刷任务注册失败: {error:?}");
        }
        false
    }

    fn finish_close(
        &mut self,
        result: Result<WorkspaceFlushReport, WorkspaceWriterError>,
        generation: u64,
        reconcile_required: bool,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CloseFlush);
        match result {
            Ok(report)
                if report.failed == 0
                    && (reconcile_required || generation != self.snapshot_generation) =>
            {
                // 旧终态或旧项目代次已排空，必须重新提交最新状态并再次冲刷。
                if reconcile_required {
                    self.side_view_persistence
                        .resume_checkpoint_for_close_retry(&self.side_views, cx);
                }
                let Some(client) = self.project_writer.clone() else {
                    self.close_in_progress = false;
                    return;
                };
                let _ = self.start_close_flush(client, window, cx);
            }
            Ok(report) if report.failed == 0 => {
                let application_global = self.application_close.as_ref().is_some_and(|close| {
                    close.coordinator.phase()
                        == slterm_workspace::ClosePhase::Running(
                            slterm_workspace::CloseStage::StopServices,
                        )
                });
                let recovery_global = self.recovery_global_close.is_some();
                if self.global_close.is_some() && (application_global || recovery_global) {
                    if application_global {
                        self.start_application_global_close(cx);
                    } else {
                        self.start_recovery_global_close(cx);
                    }
                } else {
                    self.close_in_progress = false;
                    self.close_completed = true;
                    let _ = window.update(cx, |_, window, _| window.remove_window());
                }
            }
            _ => {
                self.close_in_progress = false;
                self.side_view_persistence
                    .resume_checkpoint_for_close_retry(&self.side_views, cx);
                let _ = window.update(cx, |_, _, cx| {
                    super::events::notify_workspace_write_failed(cx)
                });
            }
        }
        cx.notify();
    }
}
