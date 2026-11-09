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

use super::config::Settings;
use crate::i18n::{Message, UiLanguage};
use crate::persistence::startup::MaintenanceStage;
use crate::persistence::workspace_writer::WorkspaceWriterError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShellRunOutcome {
    #[default]
    Exit,
    RetryStartup,
}

pub(crate) struct StartupDiagnosticView {
    error: WorkspaceWriterError,
    outcome: Rc<Cell<ShellRunOutcome>>,
}

impl StartupDiagnosticView {
    fn new(
        error: WorkspaceWriterError,
        outcome: Rc<Cell<ShellRunOutcome>>,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        Self { error, outcome }
    }

    fn retry(&self, window: &mut Window) {
        self.outcome.set(ShellRunOutcome::RetryStartup);
        window.remove_window();
    }
}

impl Render for StartupDiagnosticView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<Settings>().language;
        div()
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
                    .child(diagnostic_message(&self.error, language)),
            )
            .child(
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

fn diagnostic_message(error: &WorkspaceWriterError, language: UiLanguage) -> String {
    if let WorkspaceWriterError::MaintenanceNeedsAttention(stage) = error {
        let stage = match stage {
            MaintenanceStage::Journal => Message::StartupDiagnosticsStageJournal,
            MaintenanceStage::MaintenanceRequests => {
                Message::StartupDiagnosticsStageMaintenanceRequests
            }
            MaintenanceStage::CloseIntents => Message::StartupDiagnosticsStageCloseIntents,
            MaintenanceStage::Snapshots => Message::StartupDiagnosticsStageSnapshots,
            MaintenanceStage::HistoryQuarantine => {
                Message::StartupDiagnosticsStageHistoryQuarantine
            }
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
            crate::platform::startup::first_frame::present_then_show(handle, cx);
        }
        Ok(_) => {}
        Err(error) => {
            log::error!("failed to open startup diagnostics: {error}");
            quit(cx);
        }
    }
}

/// 诊断窗不创建 writer、控制面或 PTY；重试只关闭本窗，由外层重新进入启动链。
pub(crate) fn run(error: WorkspaceWriterError) -> ShellRunOutcome {
    let outcome = Rc::new(Cell::new(ShellRunOutcome::Exit));
    let view_outcome = outcome.clone();
    let application = gpui_platform::application()
        .with_assets(super::assets::SltermAssets)
        .with_quit_mode(gpui::QuitMode::LastWindowClosed);
    application.run(move |cx| {
        init_diagnostic(cx);
        let mut options = super::windowing::workspace_window_options(cx, true);
        let deferred_show = crate::platform::startup::first_frame::defer_show(&mut options);
        let result = cx.open_window(options, move |window, cx| {
            window.set_window_title(crate::brand::NAME);
            install_window_close(window, cx);
            let view = cx.new(|cx| StartupDiagnosticView::new(error, view_outcome.clone(), cx));
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

    use super::{ShellRunOutcome, StartupDiagnosticView, diagnostic_message};
    use crate::i18n::UiLanguage;
    use crate::persistence::startup::MaintenanceStage;
    use crate::persistence::workspace_writer::WorkspaceWriterError;

    fn open_diagnostic(
        cx: &mut TestAppContext,
        error: WorkspaceWriterError,
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
            let view = cx.new(|cx| StartupDiagnosticView::new(error, outcome.clone(), cx));
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
        let attention =
            WorkspaceWriterError::MaintenanceNeedsAttention(MaintenanceStage::CloseIntents);
        assert_eq!(
            diagnostic_message(&attention, UiLanguage::ZhCn),
            "启动维护需要人工处理：关闭意图"
        );
        assert_eq!(
            diagnostic_message(&attention, UiLanguage::EnUs),
            "Startup maintenance needs attention: close intents"
        );
        assert_eq!(
            diagnostic_message(&WorkspaceWriterError::Stopped, UiLanguage::ZhCn),
            "启动维护未完成，未启动终端或后台写入。"
        );
    }

    #[gpui::test]
    fn retry_button_returns_retry_without_starting_workspace(cx: &mut TestAppContext) {
        let (view, mut window, outcome) = open_diagnostic(
            cx,
            WorkspaceWriterError::MaintenanceNeedsAttention(MaintenanceStage::Journal),
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
        let (view, mut window, outcome) = open_diagnostic(cx, WorkspaceWriterError::Stopped);
        let weak = view.downgrade();

        assert!(window.simulate_close());
        window.update(|window, _| window.remove_window());
        window.run_until_parked();

        assert_eq!(outcome.get(), ShellRunOutcome::Exit);
        drop(view);
        cx.update(|_| {});
        weak.assert_released();
    }
}
