//! 设置中心恢复默认：页级与全局共用字段表、缺省值与热应用链。

use std::time::Instant;

use gpui::Context;
use slterm_settings::SettingsRegistry;
use slterm_settings::json::Value;

use super::AppRoot;
use super::settings_controls::fields_for_page;
use crate::settings_state::SettingsPageId;

impl AppRoot {
    /// 页级恢复默认：字段回到注册表缺省，平台键同步落地。
    pub(super) fn restore_settings_page_defaults(
        &mut self,
        page: SettingsPageId,
        cx: &mut Context<'_, Self>,
    ) {
        let now = Instant::now();
        let mut changed = false;
        let registry = SettingsRegistry::new();
        for field in fields_for_page(page) {
            if registry
                .lookup(field.key)
                .is_some_and(|def| !def.resettable)
            {
                continue;
            }
            changed |= self.settings.clear_auto(field.key, now);
            if field.key == "launch_at_login" {
                let default = registry
                    .lookup(field.key)
                    .and_then(|def| def.default_value());
                let enabled = default.is_some_and(|value| matches!(value, Value::Bool(true)));
                if let Err(error) = self.windows.set_login_start(enabled) {
                    log::warn!("恢复开机启动默认值失败: {error}");
                }
            }
        }
        if !changed {
            return;
        }
        self.reload_settings_snapshot(cx);
        self.bump_settings_revision();
        self.notify_settings_views(cx);
        cx.notify();
    }

    /// 恢复所有设置默认：全部字段页与后台任务段回到缺省，平台键同步落地。
    pub(super) fn restore_all_settings_defaults(&mut self, cx: &mut Context<'_, Self>) {
        let now = Instant::now();
        let registry = SettingsRegistry::new();
        let mut changed = false;
        for page in [
            SettingsPageId::Appearance,
            SettingsPageId::Terminal,
            SettingsPageId::Application,
        ] {
            for field in fields_for_page(page) {
                if registry
                    .lookup(field.key)
                    .is_some_and(|def| !def.resettable)
                {
                    continue;
                }
                changed |= self.settings.clear_auto(field.key, now);
            }
        }
        if let Some(login) = registry.lookup("launch_at_login") {
            let enabled = login
                .default_value()
                .is_some_and(|value| matches!(value, Value::Bool(true)));
            if let Err(error) = self.windows.set_login_start(enabled) {
                log::warn!("恢复开机启动默认值失败: {error}");
            }
        }
        changed |= self.settings.queue_explicit(
            &[(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT, Value::Null)],
            now,
        );
        crate::background_tasks::reset_defaults();
        if !changed {
            return;
        }
        self.reload_settings_snapshot(cx);
        self.bump_settings_revision();
        self.notify_settings_views(cx);
        cx.notify();
    }
}
