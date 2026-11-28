//! 设置终端页的本地学习面板：披露、清空确认、持久化状态与后台回执。

use gpui::{
    AnyElement, AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;

use super::SettingsView;
use crate::app_shell::app_root::AppRoot;
use crate::i18n::Message;
use crate::persistence::learning_writer::{
    LearningClearResponse, LearningPersistenceResponse, LearningWriterError, LearningWriterStatus,
};

/// 清空学习的页面瞬态；不进入持久状态。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum LearningClearState {
    #[default]
    Idle,
    Confirm,
    Pending,
    Done,
    Failed,
}

/// 学习持久化重试/重置的页面瞬态；不进入持久状态。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum LearningPersistenceState {
    #[default]
    Idle,
    RetryPending,
    RetryDone,
    RetryFailed,
    ResetConfirm,
    ResetPending,
    ResetDone,
    ResetFailed,
}

impl SettingsView {
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn learning_clear_state_for_test(&self) -> LearningClearState {
        self.learning_clear_state
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn learning_persistence_state_for_test(&self) -> LearningPersistenceState {
        self.learning_persistence_state
    }

    /// 终端页底部面板：本地学习披露、清空入口与持久化恢复入口。
    pub(super) fn render_learning_panel(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let theme = cx.theme().clone();
        let state = self.learning_clear_state;
        let button_text = match state {
            LearningClearState::Confirm => Message::SettingsProfilesClearLearningConfirm,
            LearningClearState::Pending => Message::SettingsProfilesClearLearningPending,
            LearningClearState::Idle | LearningClearState::Done | LearningClearState::Failed => {
                Message::SettingsProfilesClearLearning
            }
        };
        let status = match state {
            LearningClearState::Done => Some((
                Message::SettingsProfilesClearLearningDone,
                theme.muted_foreground,
            )),
            LearningClearState::Failed => {
                Some((Message::SettingsProfilesClearLearningFailed, theme.danger))
            }
            LearningClearState::Idle
            | LearningClearState::Confirm
            | LearningClearState::Pending => None,
        };
        let mut panel = div()
            .id("settings-learning")
            .debug_selector(|| "settings-learning".to_owned())
            .flex()
            .flex_col()
            .gap_1()
            .pt_3()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(language.text(Message::SettingsProfilesLearning)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(language.text(Message::SettingsProfilesLearningDisclosure)),
            )
            .child(
                div()
                    .id("settings-learning-clear")
                    .debug_selector(|| "settings-learning-clear".to_owned())
                    .w_full()
                    .cursor_pointer()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(if state == LearningClearState::Confirm {
                        theme.danger
                    } else {
                        theme.border
                    })
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(if state == LearningClearState::Confirm {
                        theme.danger
                    } else {
                        theme.foreground
                    })
                    .child(language.text(button_text))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|view, _, _, cx| view.request_learning_clear(cx)),
                    ),
            );
        if state == LearningClearState::Confirm {
            panel = panel.child(
                div()
                    .id("settings-learning-confirm")
                    .debug_selector(|| "settings-learning-confirm".to_owned())
                    .text_xs()
                    .text_color(theme.danger)
                    .child(language.text(Message::SettingsProfilesClearLearningConfirm)),
            );
        }
        if let Some((message, color)) = status {
            panel = panel.child(
                div()
                    .id("settings-learning-status")
                    .debug_selector(|| "settings-learning-status".to_owned())
                    .text_xs()
                    .text_color(color)
                    .child(language.text(message)),
            );
        }
        panel = self.render_learning_persistence(panel, language, theme, cx);
        panel.into_any_element()
    }

    /// 持久化状态只在非 Ready 或最近一次动作有结果时展示。
    fn render_learning_persistence(
        &mut self,
        panel: gpui::Stateful<gpui::Div>,
        language: crate::i18n::UiLanguage,
        theme: gpui_component::Theme,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let writer_status = self
            .root
            .upgrade()
            .map(|root| root.read(cx).learning.status())
            .unwrap_or(LearningWriterStatus::MemoryOnly);
        let state = self.learning_persistence_state;
        let failure = match writer_status {
            LearningWriterStatus::Ready => None,
            LearningWriterStatus::MemoryOnly => Some(Message::SettingsProfilesLearningMemoryOnly),
            LearningWriterStatus::Failed => Some(Message::SettingsProfilesLearningFailed),
        };
        let action_status = match state {
            LearningPersistenceState::RetryDone => {
                Some((Message::SettingsProfilesLearningRetryDone, theme.foreground))
            }
            LearningPersistenceState::RetryFailed => {
                Some((Message::SettingsProfilesLearningRetryFailed, theme.danger))
            }
            LearningPersistenceState::ResetDone => {
                Some((Message::SettingsProfilesLearningResetDone, theme.foreground))
            }
            LearningPersistenceState::ResetFailed => {
                Some((Message::SettingsProfilesLearningResetFailed, theme.danger))
            }
            LearningPersistenceState::ResetConfirm => {
                Some((Message::SettingsProfilesLearningResetConfirm, theme.danger))
            }
            LearningPersistenceState::Idle
            | LearningPersistenceState::RetryPending
            | LearningPersistenceState::ResetPending => None,
        };
        if failure.is_none() && action_status.is_none() {
            return panel;
        }
        let mut section = div()
            .id("settings-learning-persistence")
            .debug_selector(|| "settings-learning-persistence".to_owned())
            .flex()
            .flex_col()
            .gap_1();
        if let Some(message) = failure {
            section = section.child(
                div()
                    .id("settings-learning-persistence-status")
                    .debug_selector(|| "settings-learning-persistence-status".to_owned())
                    .text_xs()
                    .text_color(theme.danger)
                    .child(language.text(message)),
            );
        }
        if let Some((message, color)) = action_status {
            section = section.child(
                div()
                    .id("settings-learning-persistence-action")
                    .debug_selector(|| "settings-learning-persistence-action".to_owned())
                    .text_xs()
                    .text_color(color)
                    .child(language.text(message)),
            );
        }
        if failure.is_some() {
            let retry_text = match state {
                LearningPersistenceState::RetryPending => {
                    Message::SettingsProfilesLearningRetryPending
                }
                _ => Message::SettingsProfilesLearningRetry,
            };
            let reset_text = match state {
                LearningPersistenceState::ResetPending => {
                    Message::SettingsProfilesLearningResetPending
                }
                LearningPersistenceState::ResetConfirm => {
                    Message::SettingsProfilesLearningResetConfirm
                }
                _ => Message::SettingsProfilesLearningReset,
            };
            section = section
                .child(
                    div()
                        .id("settings-learning-retry")
                        .debug_selector(|| "settings-learning-retry".to_owned())
                        .w_full()
                        .cursor_pointer()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(theme.border)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(theme.foreground)
                        .child(language.text(retry_text))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|view, _, _, cx| {
                                view.request_learning_persistence_retry(cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .id("settings-learning-reset")
                        .debug_selector(|| "settings-learning-reset".to_owned())
                        .w_full()
                        .cursor_pointer()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(if state == LearningPersistenceState::ResetConfirm {
                            theme.danger
                        } else {
                            theme.border
                        })
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(if state == LearningPersistenceState::ResetConfirm {
                            theme.danger
                        } else {
                            theme.foreground
                        })
                        .child(language.text(reset_text))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|view, _, _, cx| {
                                view.request_learning_persistence_reset(cx);
                            }),
                        ),
                );
        }
        panel.child(section)
    }

    /// 第一次点击只进入确认态；确认后经后台等待写线程真实回执。
    fn request_learning_clear(&mut self, cx: &mut Context<'_, Self>) {
        if self.learning_clear_state == LearningClearState::Pending {
            return;
        }
        if self.learning_clear_state != LearningClearState::Confirm {
            self.learning_clear_state = LearningClearState::Confirm;
            cx.notify();
            return;
        }
        let Some(root) = self.root.upgrade() else {
            self.learning_clear_state = LearningClearState::Failed;
            cx.notify();
            return;
        };
        let response = match root.update(cx, |root, _| root.clear_learning_data()) {
            Ok(response) => response,
            Err(error) => {
                log::warn!("学习数据清空请求失败: {error:?}");
                self.learning_clear_state = LearningClearState::Failed;
                cx.notify();
                return;
            }
        };
        self.learning_clear_state = LearningClearState::Pending;
        cx.notify();
        // 写线程回执是阻塞等待，只在后台执行体接收；超时按失败展示。
        let outcome =
            cx.background_spawn(
                async move { response.recv_timeout(std::time::Duration::from_secs(5)) },
            );
        let task = cx.spawn(async move |this, cx| {
            let outcome = outcome.await;
            let _ = this.update(cx, |view, cx| {
                view.learning_clear_state = LearningClearState::from_writer_outcome(&outcome);
                view.learning_clear_task = None;
                cx.notify();
            });
        });
        self.learning_clear_task = Some(task);
    }

    /// 重试持久化：请求只入队，真实重开与回执在写线程/后台等待。
    fn request_learning_persistence_retry(&mut self, cx: &mut Context<'_, Self>) {
        if matches!(
            self.learning_persistence_state,
            LearningPersistenceState::RetryPending | LearningPersistenceState::ResetPending
        ) {
            return;
        }
        let Some(root) = self.root.upgrade() else {
            self.learning_persistence_state = LearningPersistenceState::RetryFailed;
            cx.notify();
            return;
        };
        let response = match root.update(cx, |root, _| root.retry_learning_persistence()) {
            Ok(response) => response,
            Err(error) => {
                log::warn!("学习持久化重试请求失败: {error:?}");
                self.learning_persistence_state = LearningPersistenceState::RetryFailed;
                cx.notify();
                return;
            }
        };
        self.learning_persistence_state = LearningPersistenceState::RetryPending;
        self.spawn_learning_persistence_outcome(response, LearningPersistenceAction::Retry, cx);
    }

    /// 重置持久学习：先归档原库，失败保留归档材料与可见错误。
    fn request_learning_persistence_reset(&mut self, cx: &mut Context<'_, Self>) {
        if matches!(
            self.learning_persistence_state,
            LearningPersistenceState::RetryPending | LearningPersistenceState::ResetPending
        ) {
            return;
        }
        if self.learning_persistence_state != LearningPersistenceState::ResetConfirm {
            self.learning_persistence_state = LearningPersistenceState::ResetConfirm;
            cx.notify();
            return;
        }
        let Some(root) = self.root.upgrade() else {
            self.learning_persistence_state = LearningPersistenceState::ResetFailed;
            cx.notify();
            return;
        };
        let response = match root.update(cx, |root, _| root.reset_learning_persistence()) {
            Ok(response) => response,
            Err(error) => {
                log::warn!("学习持久化重置请求失败: {error:?}");
                self.learning_persistence_state = LearningPersistenceState::ResetFailed;
                cx.notify();
                return;
            }
        };
        self.learning_persistence_state = LearningPersistenceState::ResetPending;
        self.spawn_learning_persistence_outcome(response, LearningPersistenceAction::Reset, cx);
    }

    /// 等待写线程回执的后台任务；页面销毁即取消接收。
    fn spawn_learning_persistence_outcome(
        &mut self,
        response: LearningPersistenceResponse,
        action: LearningPersistenceAction,
        cx: &mut Context<'_, Self>,
    ) {
        cx.notify();
        let outcome =
            cx.background_spawn(
                async move { response.recv_timeout(std::time::Duration::from_secs(5)) },
            );
        let task = cx.spawn(async move |this, cx| {
            let outcome = outcome.await;
            let _ = this.update(cx, |view, cx| {
                view.learning_persistence_state =
                    LearningPersistenceState::from_outcome(action, &outcome);
                view.learning_persistence_task = None;
                cx.notify();
            });
        });
        self.learning_persistence_task = Some(task);
    }
}

#[derive(Clone, Copy)]
enum LearningPersistenceAction {
    Retry,
    Reset,
}

impl LearningPersistenceState {
    /// 写线程回执 → 页面终态；只有 Ready 才算恢复成功。
    fn from_outcome(
        action: LearningPersistenceAction,
        outcome: &Result<
            Result<LearningWriterStatus, LearningWriterError>,
            std::sync::mpsc::RecvTimeoutError,
        >,
    ) -> Self {
        let recovered = matches!(outcome, Ok(Ok(LearningWriterStatus::Ready)));
        match (action, recovered) {
            (LearningPersistenceAction::Retry, true) => Self::RetryDone,
            (LearningPersistenceAction::Retry, false) => Self::RetryFailed,
            (LearningPersistenceAction::Reset, true) => Self::ResetDone,
            (LearningPersistenceAction::Reset, false) => Self::ResetFailed,
        }
    }
}

impl LearningClearState {
    /// 写线程回执 → 页面终态；超时、断连与事务失败都是可见失败。
    fn from_writer_outcome(
        outcome: &Result<Result<(), LearningWriterError>, std::sync::mpsc::RecvTimeoutError>,
    ) -> Self {
        if matches!(outcome, Ok(Ok(()))) {
            Self::Done
        } else {
            Self::Failed
        }
    }
}

impl AppRoot {
    /// 清空命令与目录学习；内存立即失效，持久删除由写线程回执。
    pub(super) fn clear_learning_data(&self) -> Result<LearningClearResponse, LearningWriterError> {
        self.learning.clear()
    }

    /// 后台重开学习库；成功只恢复后续写入。
    pub(super) fn retry_learning_persistence(
        &self,
    ) -> Result<LearningPersistenceResponse, LearningWriterError> {
        self.learning.retry_persistence()
    }

    /// 归档原库并重建持久学习；内存数据保持不变。
    pub(super) fn reset_learning_persistence(
        &self,
    ) -> Result<LearningPersistenceResponse, LearningWriterError> {
        self.learning.reset_persistence()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_outcome_maps_only_a_real_writer_success_to_done() {
        assert_eq!(
            LearningClearState::from_writer_outcome(&Ok(Ok(()))),
            LearningClearState::Done
        );
        assert_eq!(
            LearningClearState::from_writer_outcome(&Ok(Err(LearningWriterError::MemoryOnly))),
            LearningClearState::Failed
        );
        assert_eq!(
            LearningClearState::from_writer_outcome(&Err(
                std::sync::mpsc::RecvTimeoutError::Timeout
            )),
            LearningClearState::Failed
        );
    }

    #[test]
    fn persistence_outcome_maps_only_ready_to_recovery() {
        assert_eq!(
            LearningPersistenceState::from_outcome(
                LearningPersistenceAction::Retry,
                &Ok(Ok(LearningWriterStatus::Ready))
            ),
            LearningPersistenceState::RetryDone
        );
        assert_eq!(
            LearningPersistenceState::from_outcome(
                LearningPersistenceAction::Retry,
                &Ok(Ok(LearningWriterStatus::MemoryOnly))
            ),
            LearningPersistenceState::RetryFailed
        );
        assert_eq!(
            LearningPersistenceState::from_outcome(
                LearningPersistenceAction::Reset,
                &Ok(Ok(LearningWriterStatus::Failed))
            ),
            LearningPersistenceState::ResetFailed
        );
        assert_eq!(
            LearningPersistenceState::from_outcome(
                LearningPersistenceAction::Reset,
                &Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
            ),
            LearningPersistenceState::ResetFailed
        );
    }
}
