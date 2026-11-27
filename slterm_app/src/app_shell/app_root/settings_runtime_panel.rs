//! 设置「应用」页的 Runtime 状态只读面板与后台刷新。
//!
//! 端点元数据只取自非敏感发现记录；token 与凭据域永不进入设置页。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    WeakEntity, div,
};
use gpui_component::ActiveTheme as _;

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

/// Runtime 只读状态：可用性来自控制面 client，端点来自发现记录。
#[derive(Clone, Debug, Default)]
pub(super) struct RuntimeStatus {
    pub(super) loaded: bool,
    pub(super) available: bool,
    pub(super) endpoint: Option<String>,
}

impl AppRoot {
    /// 后台读取发现记录；切到应用页时刷新一次。
    pub(super) fn refresh_runtime_status(&mut self, cx: &mut Context<'_, Self>) {
        let available = self.global_close.is_some();
        let task = TaskId::RuntimeStatus;
        if self.tasks.is_registered(task) {
            return;
        }
        let paths = crate::platform::dirs::app_paths();
        let executor = cx.background_executor().clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let loaded = executor.spawn(async move {
                    crate::local_control::read_discovery_for(paths)
                        .ok()
                        .map(|record| record.endpoint.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let endpoint = loaded.await;
                    let _ = this.update(cx, |root, cx| {
                        root.tasks.complete(task);
                        root.runtime_status = RuntimeStatus {
                            loaded: true,
                            available,
                            endpoint,
                        };
                        cx.notify();
                    });
                })
            });
        if registration.is_err() {
            log::debug!("Runtime 状态刷新任务未登记");
        }
    }
}

/// 应用页面板：可用性与非敏感端点；未加载时保持上一状态。
pub(super) fn render_runtime_panel(
    root: &WeakEntity<AppRoot>,
    cx: &Context<'_, super::settings_overlay::SettingsView>,
) -> AnyElement {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let status = root
        .upgrade()
        .map(|root| root.read(cx).runtime_status.clone())
        .unwrap_or_default();
    let state = if status.available {
        language.text(Message::RuntimeAvailable)
    } else {
        language.text(Message::RuntimeUnavailable)
    };
    let mut panel = div()
        .debug_selector(|| "settings-runtime".to_owned())
        .flex()
        .flex_col()
        .gap_1()
        .pt_3()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::RuntimeTitle)),
        )
        .child(div().text_sm().child(state));
    if status.loaded {
        panel = panel.child(
            div()
                .debug_selector(|| "settings-runtime-endpoint".to_owned())
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(match &status.endpoint {
                    Some(endpoint) => language
                        .format(Message::RuntimeEndpoint, &[("endpoint", endpoint.as_str())]),
                    None => language.text(Message::RuntimeNoEndpoint).to_owned(),
                }),
        );
    }
    panel.into_any_element()
}
