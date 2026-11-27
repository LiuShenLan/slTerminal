//! 更新提示：启动检查结果、下载进度与安装预约的唯一交互面。
//!
//! 检查与下载都在后台；提示只读取封闭结果与下载状态，安装经既有
//! `begin_update_install` 统一关闭守卫。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;

use super::AppRoot;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;
use crate::update_check::{UpdateAsset, UpdateCheckResult};
use crate::update_download::DownloadStatus;

#[derive(Clone, Debug)]
pub(super) enum UpdateStage {
    Available,
    Downloading,
    Ready,
    Failed(String),
}

pub(super) struct UpdatePrompt {
    result: UpdateCheckResult,
    stage: UpdateStage,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn on_update_available(
        &mut self,
        result: UpdateCheckResult,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !result.update_available {
            return;
        }
        let version = result.latest.clone();
        // 提示状态写盘是阻塞 I/O：只放后台，失败只记日志。
        let executor = cx.background_executor().clone();
        executor
            .spawn(async move {
                if let Err(error) = crate::update_check::mark_prompted(&version) {
                    log::warn!("更新提示状态写入失败: {error}");
                }
            })
            .detach();
        let stage = result
            .asset
            .as_ref()
            .map(|asset| match crate::update_download::status(asset) {
                DownloadStatus::Ready { .. } => UpdateStage::Ready,
                DownloadStatus::Downloading { .. } => UpdateStage::Downloading,
                DownloadStatus::Failed(error) | DownloadStatus::InstallFailed(error) => {
                    UpdateStage::Failed(error)
                }
                DownloadStatus::Absent => UpdateStage::Available,
            })
            .unwrap_or(UpdateStage::Available);
        if let Err(error) = self.open_update_prompt(result.clone(), stage, window, cx) {
            log::warn!("更新提示打开失败: {error:?}");
            return;
        }
        let auto_download = cx
            .try_global::<crate::app_shell::config::Settings>()
            .is_some_and(|settings| settings.auto_download_updates);
        if auto_download
            && matches!(
                self.update_prompt.as_ref().map(|prompt| &prompt.stage),
                Some(UpdateStage::Available)
            )
            && self.update_prompt.as_ref().is_some_and(|prompt| {
                prompt.result.asset.is_some() && !prompt.result.manual_reinstall
            })
        {
            self.start_update_download(window, cx);
        }
    }

    fn open_update_prompt(
        &mut self,
        result: UpdateCheckResult,
        stage: UpdateStage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.update_prompt.is_some() {
            return Ok(());
        }
        let focus_handle = cx.focus_handle();
        let registration = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self.focus_manager.open_modal(
            self.focus_manager.workspace_token(),
            self.focus_manager.workspace_token(),
            registration.token(),
            window,
            cx,
        ) {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.update_prompt = Some(UpdatePrompt {
            result,
            stage,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    fn close_update_prompt(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(prompt) = self.update_prompt.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(prompt.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    fn update_asset(prompt: &UpdatePrompt) -> Option<UpdateAsset> {
        prompt.result.asset.clone()
    }

    /// 下载入口：会话建立与后台下载任务都只在前台发起。
    pub(super) fn start_update_download(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(prompt) = self.update_prompt.as_ref() else {
            return;
        };
        let Some(asset) = Self::update_asset(prompt) else {
            return;
        };
        if matches!(prompt.stage, UpdateStage::Downloading | UpdateStage::Ready) {
            return;
        }
        let task = TaskId::UpdateDownload;
        if self.tasks.is_registered(task) {
            return;
        }
        let job = match crate::update_download::begin(&asset) {
            Ok(Some(job)) => job,
            Ok(None) => return,
            Err(error) => {
                log::warn!("更新下载无法开始: {error}");
                if let Some(prompt) = self.update_prompt.as_mut() {
                    prompt.stage = UpdateStage::Failed(error);
                    cx.notify();
                }
                return;
            }
        };
        if let Some(prompt) = self.update_prompt.as_mut() {
            prompt.stage = UpdateStage::Downloading;
            cx.notify();
        }
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let finished = executor.spawn(async move {
                    crate::update_download::run(job);
                });
                cx.spawn(async move |this, cx| {
                    finished.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_update_download(asset, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("更新下载任务未登记");
        }
    }

    fn finish_update_download(
        &mut self,
        asset: UpdateAsset,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let status = crate::update_download::status(&asset);
        let Some(prompt) = self.update_prompt.as_mut() else {
            return;
        };
        prompt.stage = match status {
            DownloadStatus::Ready { .. } => UpdateStage::Ready,
            DownloadStatus::Failed(error) | DownloadStatus::InstallFailed(error) => {
                UpdateStage::Failed(error)
            }
            DownloadStatus::Downloading { .. } => UpdateStage::Downloading,
            DownloadStatus::Absent => UpdateStage::Available,
        };
        cx.notify();
    }

    pub(super) fn install_downloaded_update(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(prompt) = self.update_prompt.as_ref() else {
            return Ok(());
        };
        if !matches!(prompt.stage, UpdateStage::Ready) {
            return Ok(());
        }
        let Some(asset) = Self::update_asset(prompt) else {
            return Ok(());
        };
        self.close_update_prompt(window, cx)?;
        if self.begin_update_install(asset, window, cx)? {
            Ok(())
        } else {
            // 关闭被取消：安装预约由用户显式放弃，恢复提示。
            self.cancel_update_install();
            Ok(())
        }
    }

    pub(super) fn remind_update_later(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(prompt) = self.update_prompt.as_ref() else {
            return Ok(());
        };
        let version = prompt.result.latest.clone();
        self.close_update_prompt(window, cx)?;
        let executor = cx.background_executor().clone();
        executor
            .spawn(async move {
                if let Err(error) = crate::update_check::remind_later(&version) {
                    log::warn!("更新稍后提醒写入失败: {error}");
                }
            })
            .detach();
        Ok(())
    }

    pub(super) fn skip_update_version(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(prompt) = self.update_prompt.as_ref() else {
            return Ok(());
        };
        let version = prompt.result.latest.clone();
        self.close_update_prompt(window, cx)?;
        let executor = cx.background_executor().clone();
        executor
            .spawn(async move {
                if let Err(error) = crate::update_check::skip_version(&version) {
                    log::warn!("更新跳过写入失败: {error}");
                }
            })
            .detach();
        Ok(())
    }

    pub(super) fn open_update_page(&mut self, cx: &mut Context<'_, Self>) {
        let url = crate::update_check::RELEASES_PAGE;
        let executor = cx.background_executor().clone();
        executor
            .spawn(async move {
                if let Err(error) = crate::platform::file_operations::open_url_in_system(url) {
                    log::warn!("打开发布页失败: {error}");
                }
            })
            .detach();
    }

    pub(super) fn render_update_prompt(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let prompt = self.update_prompt.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let app = crate::brand::NAME;
        let version = &prompt.result.latest;
        let current = &prompt.result.current;
        let body = if prompt.result.manual_reinstall {
            language.format(
                Message::UpdateBodyManualReinstall,
                &[("app", app), ("version", version), ("current", current)],
            )
        } else if prompt.result.asset.is_none() {
            language.format(
                Message::UpdateBodyManualDownload,
                &[("app", app), ("version", version), ("current", current)],
            )
        } else {
            language.format(
                Message::UpdateBodyAvailable,
                &[("app", app), ("version", version), ("current", current)],
            )
        };
        let title = language.format(Message::UpdateTitle, &[("app", app), ("version", version)]);
        let mut buttons = div().flex().flex_row().justify_end().gap_2();
        match &prompt.stage {
            UpdateStage::Available
                if prompt.result.asset.is_some() && !prompt.result.manual_reinstall =>
            {
                buttons = buttons.child(prompt_button(
                    "update-download",
                    language.text(Message::UpdateDownloadAndInstall),
                    cx,
                    cx.listener(|root, _, window, cx| {
                        root.start_update_download(window, cx);
                    }),
                ));
            }
            UpdateStage::Available => {
                buttons = buttons.child(prompt_button(
                    "update-open-page",
                    language.text(Message::UpdateOpenDownloadPage),
                    cx,
                    cx.listener(|root, _, _, cx| root.open_update_page(cx)),
                ));
            }
            UpdateStage::Downloading => {
                buttons = buttons.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::UpdateDownloadIncomplete)),
                );
            }
            UpdateStage::Ready => {
                buttons = buttons.child(prompt_button(
                    "update-install",
                    language.text(Message::UpdateDownloadAndInstall),
                    cx,
                    cx.listener(|root, _, window, cx| {
                        if let Err(error) = root.install_downloaded_update(window, cx) {
                            log::warn!("更新安装失败: {error:?}");
                        }
                    }),
                ));
            }
            UpdateStage::Failed(error) => {
                buttons = buttons.child(div().text_xs().text_color(cx.theme().danger).child(
                    language.format(
                        Message::UpdateDownloadFailedDetail,
                        &[("error", error.as_str())],
                    ),
                ));
                buttons = buttons.child(prompt_button(
                    "update-retry",
                    language.text(Message::UpdateDownloadAndInstall),
                    cx,
                    cx.listener(|root, _, window, cx| {
                        root.start_update_download(window, cx);
                    }),
                ));
            }
        }
        buttons = buttons.child(prompt_button(
            "update-remind",
            language.text(Message::UpdateRemindLater),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) = root.remind_update_later(window, cx) {
                    log::warn!("更新稍后提醒失败: {error:?}");
                }
            }),
        ));
        buttons = buttons.child(prompt_button(
            "update-skip",
            language.text(Message::UpdateSkipVersion),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) = root.skip_update_version(window, cx) {
                    log::warn!("更新跳过失败: {error:?}");
                }
            }),
        ));
        let dialog = div()
            .debug_selector(|| "update-prompt".to_owned())
            .track_focus(&prompt.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.remind_update_later(window, cx)
                {
                    log::warn!("更新提示关闭失败: {error:?}");
                }
            }))
            .w(px(520.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(div().text_sm().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(body),
            )
            .child(buttons);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(110.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog),
                )
                .into_any_element(),
        )
    }
}

fn prompt_button(
    selector: &'static str,
    label: &'static str,
    cx: &Context<'_, AppRoot>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(selector)
        .debug_selector(move || selector.to_owned())
        .cursor_pointer()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().foreground)
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.to_owned())
        .into_any_element()
}
