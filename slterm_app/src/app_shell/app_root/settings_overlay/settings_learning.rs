//! 设置终端页的本地学习面板：披露、清空确认与后台回执。

use gpui::{
    AnyElement, AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;

use super::SettingsView;
use crate::app_shell::app_root::AppRoot;
use crate::i18n::Message;
use crate::persistence::learning_writer::{LearningClearResponse, LearningWriterError};

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

impl SettingsView {
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn learning_clear_state_for_test(&self) -> LearningClearState {
        self.learning_clear_state
    }

    /// 终端页底部面板：本地学习披露与带确认的清空入口。
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
        panel.into_any_element()
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
}
