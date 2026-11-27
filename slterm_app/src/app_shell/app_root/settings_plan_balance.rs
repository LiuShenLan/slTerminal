//! 设置页套餐余量：专用凭据、后台自动调度与只读结果展示。
//!
//! 查询状态机在 `plan_balance::service`，网络在 `plan_balance::client`。
//! 本模块只负责运行期协调：注册表订阅驱动、凭据读写、迟到回执代次与
//! 封闭文案。凭据值只在保存任务内以即用即焚句柄存在，不进入日志或 DTO。

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;
use crate::plan_balance::client::{PlanBalanceClient, PlanBalanceError};
use crate::plan_balance::service::{
    Clock, FetchToken, PlanBalanceService, PlanBalanceView, SystemClock,
};
use crate::plan_balance::{PlanBalanceCredentials, PlanBalanceInfo, PlanBalanceSource};
use crate::secrets::CredentialStore;

/// 运行期协调器：状态机、凭据端口与注册表订阅守卫。
pub(super) struct PlanBalanceRuntime {
    service: PlanBalanceService,
    credentials: PlanBalanceCredentials,
    store: Arc<dyn CredentialStore + Send + Sync>,
    local_root: std::path::PathBuf,
    configured: [bool; 2],
    configured_refresh_pending: bool,
    clock: Arc<dyn Clock>,
    _subscription: crate::background_tasks::TaskSubscription,
}

/// 套餐查询端：生产走固定端点客户端，测试可注入罐装结果。
pub(crate) type PlanBalanceFetchHook =
    Arc<dyn Fn(PlanBalanceSource) -> Result<PlanBalanceInfo, PlanBalanceError> + Send + Sync>;

pub(super) struct PlanBalanceDialog {
    selected: PlanBalanceSource,
    token: Entity<InputState>,
    busy: bool,
    error: Option<Message>,
    notice: Option<Message>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

#[cfg(test)]
struct NullCredentialStore;

#[cfg(test)]
impl CredentialStore for NullCredentialStore {
    fn load(&self, _target: &str) -> std::io::Result<Option<crate::secrets::SecretBytes>> {
        Ok(None)
    }

    fn store(&self, _target: &str, _secret: &[u8]) -> std::io::Result<()> {
        Ok(())
    }

    fn delete(&self, _target: &str) -> std::io::Result<()> {
        Ok(())
    }
}

fn source_index(source: PlanBalanceSource) -> usize {
    match source {
        PlanBalanceSource::DeepSeek => 0,
        PlanBalanceSource::Kimi => 1,
    }
}

fn source_label(source: PlanBalanceSource) -> Message {
    match source {
        PlanBalanceSource::DeepSeek => Message::SettingsPlanBalanceSourceDeepseek,
        PlanBalanceSource::Kimi => Message::SettingsPlanBalanceSourceKimi,
    }
}

fn error_message(error: PlanBalanceError) -> Message {
    match error {
        PlanBalanceError::MissingCredential => Message::SettingsPlanBalanceErrorMissingCredential,
        PlanBalanceError::Credential => Message::SettingsPlanBalanceErrorCredential,
        PlanBalanceError::Request => Message::SettingsPlanBalanceErrorRequest,
        PlanBalanceError::Status(_) => Message::SettingsPlanBalanceErrorStatus,
        PlanBalanceError::ResponseTooLarge => Message::SettingsPlanBalanceErrorTooLarge,
        PlanBalanceError::Parse => Message::SettingsPlanBalanceErrorParse,
    }
}

impl AppRoot {
    /// 装配运行期协调器：订阅注册表节拍并在后台到期时查询。
    pub(super) fn start_plan_balance_runtime(&mut self, cx: &mut Context<'_, Self>) {
        if self.plan_balance.is_some() {
            return;
        }
        let Some(subscription) =
            crate::background_tasks::subscribe(crate::background_tasks::PLAN_BALANCE)
        else {
            return;
        };
        #[cfg(all(test, feature = "gpui-test-support"))]
        let store: Arc<dyn CredentialStore + Send + Sync> = self
            .plan_balance_store_for_test
            .clone()
            .unwrap_or_else(|| Arc::new(NullCredentialStore));
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let store: Arc<dyn CredentialStore + Send + Sync> =
            Arc::new(crate::secrets::WindowsCredentialStore);
        let local_root = {
            #[cfg(all(test, feature = "gpui-test-support"))]
            {
                std::path::PathBuf::new()
            }
            #[cfg(not(all(test, feature = "gpui-test-support")))]
            {
                crate::platform::dirs::app_paths()
                    .local_root()
                    .to_path_buf()
            }
        };
        #[cfg(all(test, feature = "gpui-test-support"))]
        let clock = self
            .plan_balance_clock_for_test
            .clone()
            .unwrap_or_else(|| Arc::new(SystemClock));
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        self.plan_balance = Some(PlanBalanceRuntime {
            service: PlanBalanceService::new(clock.clone()),
            clock,
            credentials: PlanBalanceCredentials::new(store.clone(), local_root.clone()),
            store,
            local_root,
            configured: [false; 2],
            configured_refresh_pending: false,
            _subscription: subscription,
        });
        self.refresh_plan_balance_configured(cx);
        let cancellation = CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update(cx, |root, cx| root.poll_plan_balance(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        if let Err(error) = self.tasks.register(TaskId::PlanBalance, cancellation, task) {
            log::warn!("套餐余量调度任务登记失败: {error:?}");
        }
    }

    /// 凭据存在性只在后台读取；结果按来源写入运行期状态。
    fn refresh_plan_balance_configured(&mut self, cx: &mut Context<'_, Self>) {
        let task = TaskId::PlanBalanceCredentials;
        if self.tasks.is_registered(task) {
            // 在途检查完成后自动重跑一次，避免保存/删除后的状态停留在旧代次。
            if let Some(runtime) = self.plan_balance.as_mut() {
                runtime.configured_refresh_pending = true;
            }
            return;
        }
        let Some(runtime) = self.plan_balance.as_ref() else {
            return;
        };
        let store = runtime.store.clone();
        let local_root = runtime.local_root.clone();
        let targets = PlanBalanceSource::ALL.map(|source| source.credential_target(&local_root));
        let checked = cx.background_spawn(async move {
            targets.map(|target| store.exists(&target).unwrap_or(false))
        });
        let spawned = cx.spawn(async move |this, cx| {
            let configured = checked.await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(task);
                let mut rerun = false;
                if let Some(runtime) = root.plan_balance.as_mut() {
                    runtime.configured = configured;
                    if runtime.configured_refresh_pending {
                        runtime.configured_refresh_pending = false;
                        rerun = true;
                    }
                }
                if rerun {
                    root.refresh_plan_balance_configured(cx);
                }
                cx.notify();
            });
        });
        if let Err(error) = self.tasks.register(task, CancellationToken::new(), spawned) {
            log::debug!("套餐凭据状态任务登记跳过: {error:?}");
        }
    }

    /// 单次调度拍：提交任务配置，按到期且已配置的来源发起后台查询。
    fn poll_plan_balance(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = self.plan_balance.as_mut() else {
            return;
        };
        let enabled = crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE);
        let period = Duration::from_secs(crate::background_tasks::interval_sec(
            crate::background_tasks::PLAN_BALANCE,
        ));
        runtime.service.apply_settings(enabled, Some(period));
        let now = runtime.clock.now();
        let due = runtime.service.due(now);
        let configured = runtime.configured;
        let mut tokens = Vec::new();
        for source in due {
            if !configured[source_index(source)] {
                continue;
            }
            if let Some(token) = runtime.service.begin_auto(source, now) {
                tokens.push(token);
            }
        }
        for token in tokens {
            self.start_plan_balance_fetch(token, cx);
        }
    }

    fn start_plan_balance_fetch(&mut self, token: FetchToken, cx: &mut Context<'_, Self>) {
        let Some(runtime) = self.plan_balance.as_ref() else {
            return;
        };
        let credentials = runtime.credentials.clone();
        let source = token.source();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let fetch_override = self.plan_balance_fetch_for_test.clone();
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let fetch_override: Option<PlanBalanceFetchHook> = None;
        let clock = self
            .plan_balance
            .as_ref()
            .map(|runtime| runtime.clock.clone());
        let fetched = cx.background_spawn(async move {
            match fetch_override {
                Some(fetch) => fetch(source),
                None => PlanBalanceClient::new(credentials).fetch(source),
            }
        });
        let id = TaskId::PlanBalanceFetch(source);
        let task = cx.spawn(async move |this, cx| {
            let result = fetched.await;
            let now = clock.map_or_else(SystemTime::now, |clock| clock.now());
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(id);
                if let Some(runtime) = root.plan_balance.as_mut() {
                    runtime.service.complete(token, result, now);
                }
                cx.notify();
            });
        });
        if let Err(error) = self.tasks.register(id, CancellationToken::new(), task) {
            log::debug!("套餐查询任务登记跳过: {error:?}");
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_plan_balance_store_for_test(
        &mut self,
        store: Arc<dyn CredentialStore + Send + Sync>,
    ) {
        self.plan_balance_store_for_test = Some(store);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_plan_balance_fetch_for_test(&mut self, fetch: PlanBalanceFetchHook) {
        self.plan_balance_fetch_for_test = Some(fetch);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_plan_balance_clock_for_test(&mut self, clock: Arc<dyn Clock>) {
        self.plan_balance_clock_for_test = Some(clock);
    }

    /// 测试：替换凭据/查询端后重建协调器（旧订阅与调度任务一并回收）。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn restart_plan_balance_runtime_for_test(&mut self, cx: &mut Context<'_, Self>) {
        self.tasks.complete(TaskId::PlanBalance);
        self.tasks.complete(TaskId::PlanBalanceCredentials);
        self.tasks
            .complete(TaskId::PlanBalanceFetch(PlanBalanceSource::DeepSeek));
        self.tasks
            .complete(TaskId::PlanBalanceFetch(PlanBalanceSource::Kimi));
        self.tasks.complete(TaskId::PlanBalanceTokenSave);
        self.tasks.complete(TaskId::PlanBalanceTokenDelete);
        self.plan_balance = None;
        self.plan_balance_dialog = None;
        self.start_plan_balance_runtime(cx);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_plan_balance_token_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.plan_balance_dialog.as_ref() {
            let text = text.to_owned();
            dialog
                .token
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    pub(super) fn open_plan_balance_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.plan_balance_dialog.is_some() {
            return Ok(());
        }
        self.start_plan_balance_runtime(cx);
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
        let placeholder = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(Message::SettingsPlanBalanceTokenPlaceholder);
        let token = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder);
            state.set_masked(true, window, cx);
            state
        });
        self.plan_balance_dialog = Some(PlanBalanceDialog {
            selected: PlanBalanceSource::DeepSeek,
            token,
            busy: false,
            error: None,
            notice: None,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_plan_balance_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.plan_balance_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn select_plan_balance_source(
        &mut self,
        source: PlanBalanceSource,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(dialog) = self.plan_balance_dialog.as_mut() {
            dialog.selected = source;
            dialog.error = None;
            dialog.notice = None;
            cx.notify();
        }
    }

    /// 保存凭据：明文只存在于即用即焚句柄内，成功后递增代次清空旧结果。
    pub(super) fn save_plan_balance_token(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.plan_balance_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        let source = dialog.selected;
        let text = dialog.token.read(cx).value().trim().to_owned();
        if text.is_empty() {
            if let Some(dialog) = self.plan_balance_dialog.as_mut() {
                dialog.error = Some(Message::SettingsPlanBalanceErrorMissingCredential);
            }
            cx.notify();
            return;
        }
        let Some(runtime) = self.plan_balance.as_ref() else {
            return;
        };
        let store = runtime.store.clone();
        let target = source.credential_target(&runtime.local_root);
        let secret = crate::secrets::SecretBytes::new(text.into_bytes());
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::PlanBalanceTokenSave;
        if let Some(dialog) = self.plan_balance_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
            dialog.notice = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let saved = executor.spawn(async move {
                    store
                        .store(&target, secret.as_bytes())
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = saved.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_plan_balance_credential(source, result, true, window, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.plan_balance_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    pub(super) fn delete_plan_balance_token(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.plan_balance_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        let source = dialog.selected;
        let Some(runtime) = self.plan_balance.as_ref() else {
            return;
        };
        let store = runtime.store.clone();
        let target = source.credential_target(&runtime.local_root);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::PlanBalanceTokenDelete;
        if let Some(dialog) = self.plan_balance_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
            dialog.notice = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let deleted = executor
                    .spawn(async move { store.delete(&target).map_err(|error| error.to_string()) });
                cx.spawn(async move |this, cx| {
                    let result = deleted.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_plan_balance_credential(source, result, false, window, cx);
                        });
                    });
                })
            });
        if registration.is_err()
            && let Some(dialog) = self.plan_balance_dialog.as_mut()
        {
            dialog.busy = false;
        }
    }

    fn finish_plan_balance_credential(
        &mut self,
        source: PlanBalanceSource,
        result: Result<(), String>,
        saved: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match result {
            Ok(()) => {
                if let Some(runtime) = self.plan_balance.as_mut() {
                    runtime.service.invalidate(source);
                }
                if let Some(dialog) = self.plan_balance_dialog.as_mut() {
                    dialog.busy = false;
                    dialog.notice = Some(if saved {
                        Message::SettingsPlanBalanceSaved
                    } else {
                        Message::SettingsPlanBalanceDeleted
                    });
                    if saved {
                        dialog
                            .token
                            .update(cx, |state, cx| state.set_value("", window, cx));
                    }
                }
                self.refresh_plan_balance_configured(cx);
            }
            Err(error) => {
                log::warn!("套餐凭据写入失败: {error}");
                if let Some(dialog) = self.plan_balance_dialog.as_mut() {
                    dialog.busy = false;
                    dialog.error = Some(if saved {
                        Message::SettingsPlanBalanceSaveFailed
                    } else {
                        Message::SettingsPlanBalanceDeleteFailed
                    });
                }
            }
        }
        cx.notify();
    }

    /// 手动全量刷新：5 秒冷却由状态机裁定，只查询已配置来源。
    pub(super) fn manual_plan_balance_refresh(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = self.plan_balance.as_mut() else {
            return;
        };
        let now = SystemTime::now();
        let configured = runtime.configured;
        let tokens = runtime
            .service
            .begin_manual_refresh(now)
            .into_iter()
            .filter(|token| configured[source_index(token.source())])
            .collect::<Vec<_>>();
        for token in tokens {
            self.start_plan_balance_fetch(token, cx);
        }
        cx.notify();
    }

    /// 单来源只读视图；未启动运行期时返回 None。
    pub(super) fn plan_balance_view(&self, source: PlanBalanceSource) -> Option<PlanBalanceView> {
        let runtime = self.plan_balance.as_ref()?;
        Some(runtime.service.view(source, runtime.clock.now()))
    }

    pub(super) fn plan_balance_configured(&self, source: PlanBalanceSource) -> bool {
        self.plan_balance
            .as_ref()
            .is_some_and(|runtime| runtime.configured[source_index(source)])
    }

    /// 应用页只读汇总：两来源的已配置/结果/错误状态。
    pub(super) fn render_plan_balance_summary(&self, cx: &gpui::App) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut panel = div()
            .debug_selector(|| "settings-plan-balance".to_owned())
            .flex()
            .flex_col()
            .gap_1()
            .pt_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsPlanBalanceTitle)),
            );
        for source in PlanBalanceSource::ALL {
            let index = source_index(source);
            let configured = self.plan_balance_configured(source);
            let status = plan_balance_status_text(
                language,
                self.plan_balance_view(source).as_ref(),
                configured,
            );
            panel = panel.child(
                div()
                    .debug_selector(move || format!("settings-plan-balance-{index}"))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().child(language.text(source_label(source))))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            );
        }
        panel.into_any_element()
    }

    pub(super) fn render_plan_balance_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.plan_balance_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut body = div().flex().flex_col().gap_3().child(
            div()
                .text_sm()
                .child(language.text(Message::SettingsPlanBalanceTitle)),
        );
        let mut rows = div().flex().flex_col().gap_2();
        for source in PlanBalanceSource::ALL {
            let index = source_index(source);
            let selected = dialog.selected == source;
            let configured = self.plan_balance_configured(source);
            let view = self.plan_balance_view(source);
            let status = plan_balance_status_text(language, view.as_ref(), configured);
            let row = div()
                .id(gpui::ElementId::Name(
                    format!("plan-balance-source-{index}").into(),
                ))
                .debug_selector(move || format!("plan-balance-source-{index}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(if selected {
                    cx.theme().accent
                } else {
                    cx.theme().border
                })
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |root, _, _, cx| {
                        root.select_plan_balance_source(source, cx);
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().text_sm().child(language.text(source_label(source))))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text(if configured {
                                    Message::SettingsPlanBalanceConfigured
                                } else {
                                    Message::SettingsPlanBalanceUnconfigured
                                })),
                        ),
                )
                .child(
                    div()
                        .debug_selector(move || format!("plan-balance-status-{index}"))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(status),
                );
            rows = rows.child(row);
        }
        body = body.child(rows).child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::SettingsPlanBalanceToken)),
                )
                .child(Input::new(&dialog.token)),
        );
        if let Some(message) = dialog.notice {
            body = body.child(
                div()
                    .debug_selector(|| "plan-balance-notice".to_owned())
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(message)),
            );
        }
        if let Some(message) = dialog.error {
            body = body.child(
                div()
                    .debug_selector(|| "plan-balance-error".to_owned())
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(message)),
            );
        }
        let mut actions = div().flex().flex_row().justify_end().items_center().gap_3();
        if dialog.busy {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::SettingsPlanBalanceLoading)),
            );
        } else {
            actions = actions
                .child(plan_balance_action(
                    cx,
                    "plan-balance-save-token",
                    language.text(Message::SettingsPlanBalanceSave),
                    cx.theme().accent,
                    |root, window, cx| root.save_plan_balance_token(window, cx),
                ))
                .child(plan_balance_action(
                    cx,
                    "plan-balance-delete-token",
                    language.text(Message::SettingsPlanBalanceDelete),
                    cx.theme().danger,
                    |root, window, cx| root.delete_plan_balance_token(window, cx),
                ))
                .child(plan_balance_action(
                    cx,
                    "plan-balance-refresh",
                    language.text(Message::SettingsPlanBalanceRefresh),
                    cx.theme().accent,
                    |root, _window, cx| root.manual_plan_balance_refresh(cx),
                ));
        }
        actions = actions.child(plan_balance_action(
            cx,
            "plan-balance-close",
            language.text(Message::EditorCancel),
            cx.theme().foreground,
            |root, window, cx| {
                if let Err(error) = root.close_plan_balance_dialog(window, cx) {
                    log::warn!("套餐余量对话框关闭失败: {error:?}");
                }
            },
        ));
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "plan-balance-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_plan_balance_dialog(window, cx)
                {
                    log::warn!("套餐余量对话框关闭失败: {error:?}");
                }
            }))
            .w(px(560.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(body);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(90.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog_element),
                )
                .into_any_element(),
        )
    }
}

fn plan_balance_action(
    cx: &Context<'_, AppRoot>,
    id: &'static str,
    label: &'static str,
    color: gpui::Hsla,
    action: impl Fn(&mut AppRoot, &mut Window, &mut Context<'_, AppRoot>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .px_2()
        .text_xs()
        .cursor_pointer()
        .text_color(color)
        .child(label)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |root, _, window, cx| {
                cx.stop_propagation();
                action(root, window, cx);
            }),
        )
        .into_any_element()
}

fn plan_balance_status_text(
    language: crate::i18n::UiLanguage,
    view: Option<&PlanBalanceView>,
    configured: bool,
) -> String {
    if !configured {
        return language
            .text(Message::SettingsPlanBalanceUnconfigured)
            .to_owned();
    }
    let Some(view) = view else {
        return language
            .text(Message::SettingsPlanBalanceUnconfigured)
            .to_owned();
    };
    if view.loading {
        return language
            .text(Message::SettingsPlanBalanceLoading)
            .to_owned();
    }
    let mut parts = Vec::new();
    if let Some(error) = view.error {
        parts.push(language.text(error_message(error)).to_owned());
    }
    if let Some(info) = &view.result {
        parts.extend(plan_balance_info_text(language, info));
    }
    if view.stale {
        parts.push(language.text(Message::SettingsPlanBalanceStale).to_owned());
    }
    parts.join(" · ")
}

fn plan_balance_info_text(
    language: crate::i18n::UiLanguage,
    info: &PlanBalanceInfo,
) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(amount) = &info.amount {
        parts.push(format!(
            "{}: {} {}",
            language.text(Message::SettingsPlanBalanceAmount),
            amount.value,
            amount.currency
        ));
    }
    if let Some(windows) = &info.windows {
        parts.push(format!(
            "{}: {}%",
            language.text(Message::SettingsPlanBalanceFiveHour),
            windows.five_hour.used_percent
        ));
        parts.push(format!(
            "{}: {}%",
            language.text(Message::SettingsPlanBalanceSevenDay),
            windows.seven_day.used_percent
        ));
    }
    parts
}
