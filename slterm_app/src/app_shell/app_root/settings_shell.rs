//! 默认 Shell 设置热应用：变更时后台解析真实探测链，成功才替换。
//!
//! 解析涉及进程创建，只在后台执行；新设置无效时保持当前 Shell。

use gpui::Context;

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};

impl AppRoot {
    /// 默认 Shell 设置热应用：值变化时后台解析真实程序，成功才替换。
    pub(super) fn refresh_shell_from_settings(
        &mut self,
        settings: &crate::app_shell::config::Settings,
        cx: &mut Context<'_, Self>,
    ) {
        if self.resolved_shell_setting == settings.shell {
            return;
        }
        let requested = settings.shell.clone();
        let requested_for_update = requested.clone();
        let executor = cx.background_executor().clone();
        let task = TaskId::ShellResolve;
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                cx.spawn(async move |this, cx| {
                    let resolved = executor
                        .spawn(async move {
                            slterm_terminal::tty::resolve_shell(requested.as_deref())
                                .map(|shell| shell.program().to_owned())
                                .map_err(|error| error.to_string())
                        })
                        .await;
                    let _ = this.update(cx, |root, cx| {
                        root.tasks.complete(task);
                        match resolved {
                            Ok(program) => {
                                if let Ok(path) = slterm_path::CanonicalPath::new(
                                    std::path::PathBuf::from(&program),
                                ) {
                                    root.shell = path;
                                    root.resolved_shell_setting = requested_for_update;
                                    cx.notify();
                                }
                            }
                            Err(error) => {
                                log::warn!("设置中的 Shell 不可用，保持原 Shell: {error}");
                            }
                        }
                    });
                })
            });
        if registration.is_err() {
            log::debug!("Shell 解析任务未登记");
        }
    }
}
