//! 设置中心后台任务页渲染。
//!
//! 任务元数据只来自 `background_tasks` 注册表；启停提交经 AppRoot 的
//! 唯一设置写通道，页面不缓存任务状态或另建配置通道。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, div, px,
};
use std::collections::BTreeMap;

use gpui::{AppContext as _, Entity, Focusable as _, Subscription, Window};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};

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
            let editable = def.schedule != crate::background_tasks::TaskSchedule::OnDemand;
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
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(schedule),
                                    )
                                    .when(editable, |row| {
                                        row.child(
                                            div()
                                                .id(gpui::ElementId::Name(
                                                    format!("settings-task-interval-{task_id}")
                                                        .into(),
                                                ))
                                                .debug_selector(move || {
                                                    format!("settings-task-interval-{task_id}")
                                                })
                                                .w(px(72.0))
                                                .child(
                                                    self.task_inputs
                                                        .get(task_id)
                                                        .cloned()
                                                        .map(|input| {
                                                            Input::new(&input).into_any_element()
                                                        })
                                                        .unwrap_or_else(|| {
                                                            div().into_any_element()
                                                        }),
                                                ),
                                        )
                                    })
                                    .when(self.task_interval_errors.contains(task_id), |row| {
                                        row.child(
                                            div().text_xs().text_color(cx.theme().danger).child(
                                                format!(
                                                    "{}–{}",
                                                    def.interval_min, def.interval_max
                                                ),
                                            ),
                                        )
                                    }),
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

    pub(super) fn build_task_inputs(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> (
        BTreeMap<&'static str, Entity<InputState>>,
        Vec<Subscription>,
    ) {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for def in crate::background_tasks::TASKS {
            if def.schedule == crate::background_tasks::TaskSchedule::OnDemand {
                continue;
            }
            let task_id = def.task_id;
            let interval = crate::background_tasks::interval_sec(task_id).to_string();
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_value(interval.clone(), window, cx);
                state
            });
            let subscription = cx.subscribe_in(
                &input,
                window,
                move |view, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        view.commit_task_interval(task_id, input.clone(), window, cx);
                    }
                },
            );
            inputs.insert(task_id, input);
            subscriptions.push(subscription);
        }
        (inputs, subscriptions)
    }

    /// 运行值回填：聚焦或存在非法草稿的输入不被覆盖。
    pub(super) fn sync_task_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        for def in crate::background_tasks::TASKS {
            if def.schedule == crate::background_tasks::TaskSchedule::OnDemand {
                continue;
            }
            let task_id = def.task_id;
            let Some(input) = self.task_inputs.get(task_id) else {
                continue;
            };
            if self.task_interval_errors.contains(task_id) {
                continue;
            }
            let text = crate::background_tasks::interval_sec(task_id).to_string();
            let current = input.read(cx).value().to_string();
            let focused = window
                .focused(cx)
                .is_some_and(|handle| handle == input.read(cx).focus_handle(cx));
            if current != text && !focused {
                input.update(cx, |state, cx| state.set_value(text, window, cx));
            }
        }
    }

    /// 任务周期提交：只接受整数且在注册表范围内；非法草稿保留并显示范围。
    fn commit_task_interval(
        &mut self,
        task_id: &'static str,
        input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(def) = crate::background_tasks::find(task_id) else {
            return;
        };
        let text = input.read(cx).value().to_string();
        let parsed = text.trim().parse::<u64>().ok();
        let valid = parsed.is_some_and(|sec| (def.interval_min..=def.interval_max).contains(&sec));
        if !valid {
            self.task_interval_errors.insert(task_id);
            cx.notify();
            return;
        }
        self.task_interval_errors.remove(task_id);
        let Some(root) = self.root.upgrade() else {
            return;
        };
        root.update(cx, |root, cx| {
            root.apply_background_task_field(task_id, None, parsed, cx);
        });
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn set_task_interval_for_test(
        &mut self,
        task_id: &'static str,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(input) = self.task_inputs.get(task_id).cloned() else {
            return;
        };
        input.update(cx, |state, cx| state.set_value(text, window, cx));
        self.commit_task_interval(task_id, input, window, cx);
    }
}
