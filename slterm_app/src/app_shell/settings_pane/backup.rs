//! C33 加密备份导出:前台只做本地校验与选路径,收集/密封/写盘全部在
//! 后台执行器完成。`collect` 任一类失败即整单失败,目标文件零写入。
//!
//! 口令随后台闭包移动并在作用域结束丢弃;不落日志、不进状态快照。

use super::*;

impl SettingsPane {
    /// C33:口令占位符随语言热切换刷新(实体保留,编辑值/焦点不丢)。
    pub(super) fn sync_backup_passphrase_placeholder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.backup_passphrase_language == self.language {
            return;
        }
        self.backup_passphrase_language = self.language;
        let placeholder = localized_input_placeholder("backup_passphrase", self.language);
        self.backup_passphrase.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx)
        });
    }

    /// C33:导出加密备份入口。前台只做本地校验与选路径;收集/密封/写盘
    /// 全部后台执行,结果回前台。取消对话框 = 零写入、零状态变化。
    pub(super) fn export_backup(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.backup_busy {
            return;
        }
        let passphrase = self.backup_passphrase.read(cx).value().to_string();
        if let Some(message) = self.backup_validation_error(&passphrase, cx) {
            self.status = Some(SettingsStatus::BackupFailed(message));
            cx.notify();
            return;
        }
        let directory = self.settings_target.settings_directory();
        let picked = cx.prompt_for_new_path(&directory, Some(BACKUP_FILE_NAME));
        self.backup_busy = true;
        cx.notify();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(path))) => path,
                // 取消:零写入、零状态变化(忙态复位)。
                _ => {
                    let _ = this.update(cx, |pane, cx| {
                        pane.backup_busy = false;
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |pane, cx| {
                    // 对话框阶段结束;交给统一执行入口重新走忙态门与校验。
                    pane.backup_busy = false;
                    pane.start_backup_export(path, window, cx);
                });
            });
        })
        .detach();
    }

    /// C33:选定路径后的导出执行(测试可直接调用绕过系统对话框)。
    ///
    /// 前台零阻塞:校验只看内存态;`collect`/`seal`/原子写进后台执行器。
    /// `collect` 任一类失败即整单失败,目标文件一行不写。口令随后台闭包
    /// 移动并在作用域结束丢弃,不落日志、不存字段。
    pub(super) fn start_backup_export(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.backup_busy {
            return;
        }
        let selection = self.backup_selection;
        let passphrase = self.backup_passphrase.read(cx).value().to_string();
        if let Some(message) = self.backup_validation_error(&passphrase, cx) {
            self.status = Some(SettingsStatus::BackupFailed(message));
            cx.notify();
            return;
        }
        self.backup_busy = true;
        self.status = None;
        cx.notify();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let task = cx.background_executor().spawn(async move {
                // 整单语义:collect 任一类解析失败直接 Err,不产生部分备份;
                // 目标文件只在密封成功后才原子写入。
                let archive = crate::encrypted_backup::collect(selection)?;
                let packet = crate::encrypted_backup::seal(&archive, &passphrase)?;
                crate::atomic_file::write(&path, &packet)
                    .map_err(|error| format!("写入备份文件失败: {error}"))?;
                Ok::<PathBuf, String>(path)
            });
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |pane, cx| pane.finish_backup_export(result, window, cx));
            });
        })
        .detach();
    }

    /// 前台前置校验:与引擎同一口令下限(单源常量),避免后台往返。
    fn backup_validation_error(&self, passphrase: &str, cx: &Context<'_, Self>) -> Option<String> {
        let language = self.language(cx);
        if self.backup_selection.is_empty() {
            return Some(language.text(Message::SettingsBackupNoCategory).to_owned());
        }
        if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            return Some(
                language
                    .text(Message::SettingsBackupPassphraseTooShort)
                    .to_owned(),
            );
        }
        None
    }

    /// 结果回前台:成功给非错误态状态条;失败给错误态状态条 + 窗口通知
    /// (标题明示「备份未完成」,用户不会误以为成功)。
    fn finish_backup_export(
        &mut self,
        result: Result<PathBuf, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.backup_busy = false;
        match result {
            Ok(path) => self.status = Some(SettingsStatus::BackupExported { path }),
            Err(error) => {
                self.status = Some(SettingsStatus::BackupFailed(error.clone()));
                let language = self.language(cx);
                window.push_notification(
                    gpui_component::notification::Notification::error(
                        language.text(Message::SettingsBackupFailed),
                    )
                    .message(error),
                    cx,
                );
            }
        }
        cx.notify();
    }
}
