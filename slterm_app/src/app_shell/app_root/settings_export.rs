//! 普通设置导出：前台生成白名单快照，后台选择目标并原子写。
//!
//! 导出只含已登记的非敏感应用字段；主题包、Hooks、凭据与工作区不经此通道。

use gpui::{Context, Window};

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};

impl AppRoot {
    /// 导出入口：目标选择与写盘都在后台，前台只结算回执。
    pub(super) fn start_export_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let task = TaskId::SettingsExport;
        if self.tasks.is_registered(task) {
            return;
        }
        let export = crate::settings_transfer::export_settings(self.settings.values());
        let json = crate::settings_transfer::export_json(&export);
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let written = executor.spawn(async move {
                    let Some(target) = windows
                        .pick_save_target(owner, std::path::Path::new("slterm-settings.json"))?
                    else {
                        return Ok::<bool, std::io::Error>(false);
                    };
                    crate::atomic_file::write(&target, json.as_bytes())?;
                    Ok(true)
                });
                cx.spawn(async move |this, cx| {
                    let result = written.await;
                    let _ = window_handle.update(cx, |_, _window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            match result {
                                Ok(true) => super::events::notify_settings_exported(cx),
                                Ok(false) => {}
                                Err(error) => {
                                    log::warn!("导出设置失败: {error}");
                                    super::events::notify_settings_export_failed(cx);
                                }
                            }
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("设置导出任务未登记");
        }
    }
}
