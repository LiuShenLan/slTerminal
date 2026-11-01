//! AppRoot 的壳事件落点。

use std::time::Duration;

use gpui::{Context, Window};
use slterm_path::CanonicalPath;

use super::AppRoot;
use crate::app_shell::SltermShellEvent;

impl AppRoot {
    /// 后台只负责定时唤醒；通道排空与状态变更都在窗口前台完成。
    pub(crate) fn start_shell_event_pump(
        &mut self,
        receiver: std::sync::mpsc::Receiver<SltermShellEvent>,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(120)).await;
                let mut events = Vec::new();
                while let Ok(event) = receiver.try_recv() {
                    events.push(event);
                }
                if events.is_empty() {
                    continue;
                }
                cx.update(|cx| {
                    let _ = window_handle.update(cx, |_root, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            for event in events {
                                root.handle_shell_event(event, window, cx);
                            }
                        });
                    });
                });
            }
        })
        .detach();
    }

    pub(crate) fn handle_shell_event(
        &mut self,
        event: SltermShellEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            SltermShellEvent::NotificationFocus(pane) => {
                crate::windowing::focus::focus_window(window);
                if let Some(pane) = pane
                    && let Err(error) = self.focus_notification_pane(pane, window, cx)
                {
                    log::warn!("通知目标终端不存在: {error}");
                }
                cx.notify();
            }
            SltermShellEvent::NotificationChoice { pane_id } => {
                crate::windowing::focus::focus_window(window);
                if let Err(error) = self.focus_notification_pane(pane_id, window, cx) {
                    log::warn!("通知动作目标终端不存在: {error}");
                }
                crate::audit::record(crate::audit::AuditEvent::new(
                    "notification_choice",
                    "app_shell::app_root::events::handle_shell_event",
                    crate::audit::AuditResult::Acknowledged,
                ));
                cx.notify();
            }
            SltermShellEvent::TrayQuit => {
                // 新根尚无脏 Document；统一关闭协调器接管后仍只能由该事件入口触发。
                #[cfg(not(all(test, feature = "gpui-test-support")))]
                window.remove_window();
            }
            SltermShellEvent::WorkspaceWriteFailed => {
                let language = cx.global::<crate::app_shell::config::Settings>().language;
                crate::notify::deliver_startup_summary(
                    &crate::notify::Notification::ProcessAlert {
                        text: language
                            .text(crate::i18n::Message::NotifyWorkspaceWriteFailed)
                            .to_owned(),
                    },
                    cx,
                );
                cx.notify();
            }
            SltermShellEvent::OpenDirectories(paths) => {
                for path in paths {
                    match CanonicalPath::new(path) {
                        Ok(path) => {
                            if let Err(error) = self.open_project(path, window, cx) {
                                log::warn!("打开外部目录失败: {error}");
                            }
                        }
                        Err(error) => log::warn!("外部目录路径无效: {error}"),
                    }
                }
            }
        }
    }
}
