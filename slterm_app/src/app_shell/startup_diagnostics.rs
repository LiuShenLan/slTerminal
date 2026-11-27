//! 启动维护失败的只读诊断与重试窗口。

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, Root,
    button::{Button, ButtonVariants as _},
};

use slterm_path::AppPaths;

use super::config::Settings;
use crate::app_shell::app_root::DomainRecoveryFailure;
use crate::i18n::{Message, UiLanguage};
use crate::persistence::startup::MaintenanceStage;
use crate::persistence::workspace_writer::WorkspaceWriterError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShellRunOutcome {
    #[default]
    Exit,
    RetryStartup,
}

/// 诊断窗的唯一失败源；领域恢复失败与维护失败共用只读分支。
#[derive(Debug)]
pub(crate) enum StartupFailure {
    Writer(WorkspaceWriterError),
    Domain(DomainRecoveryFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbandonState {
    Idle,
    Confirm,
    Running,
    Failed,
}

pub(crate) struct StartupDiagnosticView {
    failure: StartupFailure,
    paths: Option<AppPaths>,
    outcome: Rc<Cell<ShellRunOutcome>>,
    abandon: AbandonState,
    abandon_task: Option<gpui::Task<()>>,
}

impl StartupDiagnosticView {
    fn new(
        failure: StartupFailure,
        paths: Option<AppPaths>,
        outcome: Rc<Cell<ShellRunOutcome>>,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        Self {
            failure,
            paths,
            outcome,
            abandon: AbandonState::Idle,
            abandon_task: None,
        }
    }

    fn retry(&self, window: &mut Window) {
        self.outcome.set(ShellRunOutcome::RetryStartup);
        window.remove_window();
    }

    fn abandon_stage(&self) -> Option<MaintenanceStage> {
        match &self.failure {
            StartupFailure::Writer(WorkspaceWriterError::MaintenanceNeedsAttention(stage)) => {
                Some(*stage)
            }
            _ => None,
        }
    }

    fn abandon_clicked(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        match self.abandon {
            AbandonState::Idle | AbandonState::Failed => {
                self.abandon = AbandonState::Confirm;
                cx.notify();
            }
            AbandonState::Confirm => self.start_abandon(window, cx),
            AbandonState::Running => {}
        }
    }

    /// 归档与删除在后台执行；成功后关闭诊断窗，由外层重走同一启动链。
    fn start_abandon(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let (Some(paths), Some(stage)) = (self.paths.clone(), self.abandon_stage()) else {
            return;
        };
        self.abandon = AbandonState::Running;
        cx.notify();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.abandon_task = Some(cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    crate::persistence::startup::abandon_maintenance_blockers(&paths, stage)
                })
                .await;
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| match result {
                    Ok(_) => {
                        view.outcome.set(ShellRunOutcome::RetryStartup);
                        window.remove_window();
                    }
                    Err(_) => {
                        view.abandon = AbandonState::Failed;
                        cx.notify();
                    }
                });
            });
        }));
    }
}

impl StartupDiagnosticView {
    fn abandon_controls(
        &mut self,
        language: &UiLanguage,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        match self.abandon {
            AbandonState::Idle => Button::new("startup-diagnostics-abandon")
                .debug_selector(|| "startup-diagnostics-abandon".to_owned())
                .label(language.text(Message::StartupDiagnosticsAbandon))
                .ghost()
                .on_click(cx.listener(|this, _, window, cx| this.abandon_clicked(window, cx)))
                .into_any_element(),
            AbandonState::Confirm => div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .max_w(px(560.0))
                        .text_center()
                        .text_color(cx.theme().danger)
                        .debug_selector(|| "startup-diagnostics-abandon-warning".to_owned())
                        .child(language.text(Message::StartupDiagnosticsAbandonWarning)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.0))
                        .child(
                            Button::new("startup-diagnostics-abandon-confirm")
                                .debug_selector(|| "startup-diagnostics-abandon-confirm".to_owned())
                                .label(language.text(Message::StartupDiagnosticsAbandonConfirm))
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.abandon_clicked(window, cx)
                                })),
                        )
                        .child(
                            Button::new("startup-diagnostics-abandon-cancel")
                                .debug_selector(|| "startup-diagnostics-abandon-cancel".to_owned())
                                .label(language.text(Message::StartupDiagnosticsCancel))
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.abandon = AbandonState::Idle;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
            AbandonState::Running => div()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "startup-diagnostics-abandon-running".to_owned())
                .child(language.text(Message::StartupDiagnosticsAbandonRunning))
                .into_any_element(),
            AbandonState::Failed => div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .max_w(px(560.0))
                        .text_center()
                        .text_color(cx.theme().danger)
                        .debug_selector(|| "startup-diagnostics-abandon-failed".to_owned())
                        .child(language.text(Message::StartupDiagnosticsAbandonFailed)),
                )
                .child(
                    Button::new("startup-diagnostics-abandon")
                        .debug_selector(|| "startup-diagnostics-abandon".to_owned())
                        .label(language.text(Message::StartupDiagnosticsAbandon))
                        .ghost()
                        .on_click(
                            cx.listener(|this, _, window, cx| this.abandon_clicked(window, cx)),
                        ),
                )
                .into_any_element(),
        }
    }
}

impl Render for StartupDiagnosticView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<Settings>().language;
        let running = self.abandon == AbandonState::Running;
        let mut view = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(18.0))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .debug_selector(|| "startup-diagnostics-title".to_owned())
                    .text_xl()
                    .child(language.text(Message::StartupDiagnosticsTitle)),
            )
            .child(
                div()
                    .debug_selector(|| "startup-diagnostics-message".to_owned())
                    .max_w(px(560.0))
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(diagnostic_message(&self.failure, language)),
            );
        if self.abandon_stage().is_some() && self.paths.is_some() {
            view = view.child(self.abandon_controls(&language, cx));
        }
        if running {
            return view;
        }
        view.child(
            div()
                .flex()
                .gap(px(12.0))
                .child(
                    Button::new("startup-diagnostics-retry")
                        .debug_selector(|| "startup-diagnostics-retry".to_owned())
                        .label(language.text(Message::StartupDiagnosticsRetry))
                        .primary()
                        .on_click(cx.listener(|this, _, window, _| this.retry(window))),
                )
                .child(
                    Button::new("startup-diagnostics-exit")
                        .debug_selector(|| "startup-diagnostics-exit".to_owned())
                        .label(language.text(Message::StartupDiagnosticsExit))
                        .ghost()
                        .on_click(cx.listener(|_, _, window, _| window.remove_window())),
                ),
        )
    }
}

fn install_window_close(window: &mut Window, cx: &mut App) {
    window.on_window_should_close(cx, |_, _| true);
}

fn diagnostic_message(failure: &StartupFailure, language: UiLanguage) -> String {
    let error = match failure {
        StartupFailure::Writer(error) => error,
        StartupFailure::Domain(DomainRecoveryFailure::Database) => {
            return language
                .text(Message::StartupDiagnosticsDatabaseUnavailable)
                .to_owned();
        }
        StartupFailure::Domain(DomainRecoveryFailure::Project) => {
            return language
                .text(Message::StartupDiagnosticsProjectRecoveryFailed)
                .to_owned();
        }
        StartupFailure::Domain(DomainRecoveryFailure::Snapshot) => {
            return language
                .text(Message::StartupDiagnosticsSnapshotRecoveryFailed)
                .to_owned();
        }
    };
    if let WorkspaceWriterError::MaintenanceNeedsAttention(stage) = error {
        let stage = match stage {
            MaintenanceStage::Journal => Message::StartupDiagnosticsStageJournal,
            MaintenanceStage::MaintenanceRequests => {
                Message::StartupDiagnosticsStageMaintenanceRequests
            }
            MaintenanceStage::CloseIntents => Message::StartupDiagnosticsStageCloseIntents,
        };
        language.format(
            Message::StartupDiagnosticsAttention,
            &[("stage", language.text(stage))],
        )
    } else {
        language
            .text(Message::StartupDiagnosticsDescription)
            .to_owned()
    }
}

/// 诊断初始化不得读取并修复设置，也不得进入壁纸或终端后台链。
fn init_diagnostic(cx: &mut App) {
    super::register_bundled_fonts(cx);
    gpui_component::init(cx);
    let settings = Settings::default();
    gpui_component::set_locale(settings.language.gpui_component_locale());
    cx.set_global(settings);
    gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);
}

fn handle_open_result<E: std::fmt::Display>(
    result: Result<gpui::AnyWindowHandle, E>,
    deferred_show: bool,
    cx: &mut App,
    quit: impl FnOnce(&mut App),
) {
    match result {
        Ok(handle) if deferred_show => {
            crate::platform::startup::first_frame::present_then_show(handle, cx, || {});
        }
        Ok(_) => {}
        Err(error) => {
            log::error!("failed to open startup diagnostics: {error}");
            quit(cx);
        }
    }
}

/// 诊断窗不创建 writer、控制面或 PTY；重试只关闭本窗，由外层重新进入启动链。
pub(crate) fn run(error: WorkspaceWriterError, paths: AppPaths) -> ShellRunOutcome {
    run_failure(StartupFailure::Writer(error), paths)
}

/// 领域恢复失败与维护失败共用同一只读诊断与重试链。
pub(crate) fn run_domain(failure: DomainRecoveryFailure, paths: AppPaths) -> ShellRunOutcome {
    run_failure(StartupFailure::Domain(failure), paths)
}

fn run_failure(failure: StartupFailure, paths: AppPaths) -> ShellRunOutcome {
    let outcome = Rc::new(Cell::new(ShellRunOutcome::Exit));
    let view_outcome = outcome.clone();
    let application = gpui_platform::application()
        .with_assets(super::assets::SltermAssets)
        .with_quit_mode(gpui::QuitMode::LastWindowClosed);
    application.run(move |cx| {
        init_diagnostic(cx);
        let mut options = super::windowing::workspace_window_options(cx, true, None);
        let deferred_show = crate::platform::startup::first_frame::defer_show(&mut options);
        let result = cx.open_window(options, move |window, cx| {
            window.set_window_title(crate::brand::NAME);
            install_window_close(window, cx);
            let view = cx.new(|cx| {
                StartupDiagnosticView::new(failure, Some(paths), view_outcome.clone(), cx)
            });
            cx.new(|cx| Root::new(view, window, cx))
        });
        handle_open_result(
            result.map(|handle| handle.into()),
            deferred_show,
            cx,
            |cx| cx.quit(),
        );
    });
    outcome.get()
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext,
    };
    use gpui_component::Root;

    use super::{ShellRunOutcome, StartupDiagnosticView, StartupFailure, diagnostic_message};
    use crate::app_shell::app_root::DomainRecoveryFailure;
    use crate::i18n::UiLanguage;
    use crate::persistence::startup::MaintenanceStage;
    use crate::persistence::workspace_writer::WorkspaceWriterError;

    fn open_diagnostic(
        cx: &mut TestAppContext,
        failure: StartupFailure,
    ) -> (
        Entity<StartupDiagnosticView>,
        VisualTestContext,
        Rc<Cell<ShellRunOutcome>>,
    ) {
        open_diagnostic_with(cx, failure, None)
    }

    fn open_diagnostic_with(
        cx: &mut TestAppContext,
        failure: StartupFailure,
        paths: Option<slterm_path::AppPaths>,
    ) -> (
        Entity<StartupDiagnosticView>,
        VisualTestContext,
        Rc<Cell<ShellRunOutcome>>,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(crate::app_shell::config::Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let outcome = Rc::new(Cell::new(ShellRunOutcome::Exit));
        let mut slot = None;
        let window_size = cx.update(|cx| gpui::Bounds::maximized(None, cx).size);
        let window = cx.open_window(window_size, |window, cx| {
            super::install_window_close(window, cx);
            let view = cx.new(|cx| StartupDiagnosticView::new(failure, paths, outcome.clone(), cx));
            slot = Some(view.clone());
            Root::new(view, window, cx)
        });
        let mut window = VisualTestContext::from_window(window.into(), cx);
        window.run_until_parked();
        window.refresh().unwrap();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        (slot.unwrap(), window, outcome)
    }

    #[test]
    fn diagnostic_messages_are_localized_and_sanitized() {
        let attention = StartupFailure::Writer(WorkspaceWriterError::MaintenanceNeedsAttention(
            MaintenanceStage::CloseIntents,
        ));
        assert_eq!(
            diagnostic_message(&attention, UiLanguage::ZhCn),
            "启动维护需要人工处理：关闭意图"
        );
        assert_eq!(
            diagnostic_message(&attention, UiLanguage::EnUs),
            "Startup maintenance needs attention: close intents"
        );
        assert_eq!(
            diagnostic_message(
                &StartupFailure::Writer(WorkspaceWriterError::Stopped),
                UiLanguage::ZhCn
            ),
            "启动维护未完成，未启动终端或后台写入。"
        );
        assert_eq!(
            diagnostic_message(
                &StartupFailure::Domain(DomainRecoveryFailure::Snapshot),
                UiLanguage::ZhCn
            ),
            "文档恢复未完成；记录与草稿保持原样。请处理损坏数据后重试。"
        );
        assert_eq!(
            diagnostic_message(
                &StartupFailure::Domain(DomainRecoveryFailure::Project),
                UiLanguage::EnUs
            ),
            "The saved layout could not be restored. Records are unchanged; retry after fixing the corrupt data."
        );
    }

    #[gpui::test]
    fn retry_button_returns_retry_without_starting_workspace(cx: &mut TestAppContext) {
        let (view, mut window, outcome) = open_diagnostic(
            cx,
            StartupFailure::Writer(WorkspaceWriterError::MaintenanceNeedsAttention(
                MaintenanceStage::Journal,
            )),
        );
        let weak = view.downgrade();
        let retry = window.debug_bounds("startup-diagnostics-retry").unwrap();

        window.simulate_mouse_down(retry.center(), MouseButton::Left, Modifiers::default());
        window.simulate_mouse_up(retry.center(), MouseButton::Left, Modifiers::default());
        window.run_until_parked();

        assert_eq!(outcome.get(), ShellRunOutcome::RetryStartup);
        drop(view);
        cx.update(|_| {});
        weak.assert_released();
    }

    #[test]
    fn retry_loop_reenters_same_startup_with_original_cwd() {
        let mut attempts = Vec::new();
        crate::app_shell::run_shell_loop(Some(std::path::PathBuf::from("D:/work/a")), |cwd| {
            attempts.push(cwd);
            if attempts.len() == 1 {
                ShellRunOutcome::RetryStartup
            } else {
                ShellRunOutcome::Exit
            }
        });

        assert_eq!(
            attempts,
            vec![
                Some(std::path::PathBuf::from("D:/work/a")),
                Some(std::path::PathBuf::from("D:/work/a"))
            ]
        );
    }

    #[test]
    fn diagnostic_init_excludes_settings_recovery_and_background_effects() {
        let source = include_str!("startup_diagnostics.rs");
        let start = source.find("fn init_diagnostic").unwrap();
        let end = source[start..].find("\nfn handle_open_result").unwrap();
        let init = &source[start..start + end];
        for forbidden in [
            "Settings::load",
            "apply_dark_theme",
            "wallpaper",
            "gpui_terminal::init",
            "app_root::keymap",
        ] {
            assert!(!init.contains(forbidden), "诊断初始化不得包含 {forbidden}");
        }
        let run_start = source.find("pub(crate) fn run(").unwrap();
        let run_end = source[run_start..].find("#[cfg(all(test").unwrap();
        let run = &source[run_start..run_start + run_end];
        assert!(run.contains("init_diagnostic(cx)"));
        assert!(!run.contains("super::init(cx)"));
    }

    #[gpui::test]
    fn open_failure_requests_quit(cx: &mut TestAppContext) {
        let quit = Cell::new(false);
        cx.update(|cx| {
            super::handle_open_result(Err("injected open failure"), false, cx, |_| quit.set(true));
        });
        assert!(quit.get());
    }

    #[gpui::test]
    fn closing_diagnostic_returns_exit(cx: &mut TestAppContext) {
        let (view, mut window, outcome) =
            open_diagnostic(cx, StartupFailure::Writer(WorkspaceWriterError::Stopped));
        let weak = view.downgrade();

        assert!(window.simulate_close());
        window.update(|window, _| window.remove_window());
        window.run_until_parked();

        assert_eq!(outcome.get(), ShellRunOutcome::Exit);
        drop(view);
        cx.update(|_| {});
        weak.assert_released();
    }

    /// 显式放弃必须二次确认、先归档再删除，并在完成后请求重试启动。
    #[gpui::test]
    fn abandon_unfinished_close_intent_archives_then_retries(cx: &mut TestAppContext) {
        use crate::persistence::close::{CloseCommitStatus, PersistedCloseIntent};
        use crate::persistence::metadata::RunMarker;
        use crate::persistence::workspace::WorkspaceDatabase;
        use slterm_workspace::{CloseIntent, CloseScope};

        const EPOCH: &str = "epoch-diagnostic-abandon";
        let temp = tempfile::tempdir().unwrap();
        let paths =
            slterm_path::AppPaths::new(temp.path().join("config"), temp.path().join("local"))
                .unwrap();
        {
            let mut store = WorkspaceDatabase::open(&paths).unwrap();
            store
                .begin_run(
                    EPOCH,
                    RunMarker {
                        run_id: "run-diagnostic".into(),
                        process_id: 1,
                        process_created_at_ms: 1,
                    },
                )
                .unwrap();
            store
                .register_close_intent(&PersistedCloseIntent {
                    close_id: "close-diagnostic".into(),
                    scope: CloseScope::ApplicationExit,
                    store_epoch: EPOCH.into(),
                    intent: CloseIntent::new(Vec::new(), [7; 32]).unwrap(),
                    commit_status: CloseCommitStatus::Collected,
                })
                .unwrap();
        }
        let (view, mut window, outcome) = open_diagnostic_with(
            cx,
            StartupFailure::Writer(WorkspaceWriterError::MaintenanceNeedsAttention(
                MaintenanceStage::CloseIntents,
            )),
            Some(paths.clone()),
        );

        let abandon = window
            .debug_bounds("startup-diagnostics-abandon")
            .expect("维护阻断必须提供放弃入口");
        window.simulate_click(abandon.center(), Modifiers::default());
        window.run_until_parked();
        window.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        assert!(
            window
                .debug_bounds("startup-diagnostics-abandon-warning")
                .is_some(),
            "放弃前必须展示不可撤销警告"
        );
        let confirm = window
            .debug_bounds("startup-diagnostics-abandon-confirm")
            .unwrap();
        window.simulate_click(confirm.center(), Modifiers::default());
        window.run_until_parked();

        assert_eq!(outcome.get(), ShellRunOutcome::RetryStartup);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.unfinished_close_intents().unwrap().is_empty());
        let archive = std::fs::read_dir(paths.local_root().join("recovery-abandon"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
            .count();
        assert_eq!(archive, 1, "删除前必须留有整库诊断副本");
        drop(view);
    }
}
