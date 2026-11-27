//! 设置中心后台任务页渲染。
//!
//! 任务元数据只来自 `background_tasks` 注册表；启停提交经 AppRoot 的
//! 唯一设置写通道，页面不缓存任务状态或另建配置通道。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, div, px,
};
use gpui_component::ActiveTheme as _;

use super::settings_overlay::SettingsView;

impl SettingsView {
    /// 后台任务页：任务清单来自注册表，启停立即提交，周期显示当前运行值。
    pub(super) fn render_background_tasks_page(&self, cx: &Context<'_, Self>) -> AnyElement {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut rows = div()
            .id("settings-background-tasks")
            .debug_selector(|| "settings-background-tasks".to_owned())
            .flex()
            .flex_col()
            .gap_2();
        for def in crate::background_tasks::TASKS {
            let task_id = def.task_id;
            let enabled = crate::background_tasks::enabled(task_id);
            let interval = crate::background_tasks::interval_sec(task_id);
            let title = match task_id {
                crate::background_tasks::AGENT_SCREEN_WATCHDOG => {
                    crate::i18n::Message::SettingsTasksAgentScreenWatchdog
                }
                crate::background_tasks::AUTO_UPDATE_CHECK => {
                    crate::i18n::Message::SettingsTasksAutoUpdateCheck
                }
                crate::background_tasks::AUTO_UPDATE_DOWNLOAD => {
                    crate::i18n::Message::SettingsTasksAutoUpdateDownload
                }
                crate::background_tasks::PLAN_BALANCE => {
                    crate::i18n::Message::SettingsTasksPlanBalance
                }
                _ => continue,
            };
            let schedule = match def.schedule {
                crate::background_tasks::TaskSchedule::Interval => language.format(
                    crate::i18n::Message::SettingsTasksInterval,
                    &[("seconds", &interval.to_string())],
                ),
                crate::background_tasks::TaskSchedule::Once => language.format(
                    crate::i18n::Message::SettingsTasksStartupDelay,
                    &[("seconds", &interval.to_string())],
                ),
                crate::background_tasks::TaskSchedule::OnDemand => language
                    .text(crate::i18n::Message::SettingsTasksOnDemand)
                    .to_owned(),
            };
            let root = self.root.clone();
            rows = rows.child(
                div()
                    .id(gpui::ElementId::Name(
                        format!("settings-task-{task_id}").into(),
                    ))
                    .debug_selector(move || format!("settings-task-{task_id}"))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().child(language.text(title)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(schedule),
                            ),
                    )
                    .child(
                        div()
                            .id(gpui::ElementId::Name(
                                format!("settings-task-toggle-{task_id}").into(),
                            ))
                            .debug_selector(move || format!("settings-task-toggle-{task_id}"))
                            .w(px(40.0))
                            .h(px(20.0))
                            .rounded_full()
                            .cursor_pointer()
                            .bg(if enabled {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_view, _, _, cx| {
                                    let _ = root.update(cx, |root, cx| {
                                        root.apply_background_task_field(
                                            task_id,
                                            Some(!enabled),
                                            None,
                                            cx,
                                        );
                                    });
                                }),
                            ),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(rows)
            .into_any_element()
    }
}
