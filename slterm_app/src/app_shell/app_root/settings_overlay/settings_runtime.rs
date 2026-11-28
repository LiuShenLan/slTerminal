//! 设置应用、快照刷新、加载/保存重试与设置入口生命周期。

use super::*;

impl AppRoot {
    /// 设置入口：有活跃项目时在聚焦组打开项目设置面板（同项目单例），
    /// 无项目时打开应用级浮层。重复入口只还焦既有视图。
    pub(crate) fn open_settings_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        // 浮层打开期间继续共享应用设置并保留草稿：入口先还焦既有浮层；
        // 项目 Settings 面板只在浮层关闭后的下一次入口使用。
        if self.settings_overlay.is_some() || self.snapshot.registry().active().is_none() {
            return self.open_settings_overlay_from_request(window, cx);
        }
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let view_key = ViewKey::Settings { project_id };
        if self.panel_for_view(project_id, &view_key, cx).is_ok() {
            return self.activate_view(view_key, window, cx);
        }
        let target_group = match self.focused_group(project_id, cx)? {
            Some(group_id) => group_id,
            None => {
                PanelGroupId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7")
            }
        };
        let candidate =
            self.snapshot
                .with_opened_view(project_id, target_group, view_key.clone())?;
        candidate.validate()?;
        let view_states = self.side_view_persistence.settings_view_states();
        let settings_key = view_key.clone();
        let root_handle = cx.entity().downgrade();
        let windows = self.windows.clone();
        let initial_values =
            super::super::settings_controls::initial_field_values(self.settings.values());
        let panel_id = self.install_panel(
            super::super::documents::DocumentPanelTarget {
                project_id,
                view: view_key,
                focus: true,
                activate_project: false,
            },
            window,
            cx,
            move |window, cx| {
                let view = cx.new(|cx| {
                    SettingsView::with_state(
                        window,
                        cx,
                        settings_key,
                        Some(view_states),
                        false,
                        SettingsViewRoots {
                            root: root_handle,
                            windows,
                        },
                        initial_values,
                    )
                });
                let focus = view.read(cx).focus_handle.clone();
                (PanelContent::Settings(view), focus)
            },
        )?;
        self.commit_project_snapshot(candidate, cx);
        self.bind_settings_panel(panel_id, project_id, cx);
        self.request_agent_hooks_if_selected(project_id, cx);
        cx.notify();
        Ok(())
    }

    pub(in crate::app_shell::app_root) fn open_settings_overlay_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        if let Some(host) = &self.settings_overlay {
            self.focus_manager
                .request_focus(host.registration.token(), window, cx)?;
            cx.notify();
            return Ok(());
        }
        // 项目 Settings 面板由设置阶段接入；此处不得把浮层伪装成项目面板。
        if self.snapshot.registry().active().is_some() {
            return Err(AppRootError::MissingPanel);
        }
        self.sync_focus(window, cx);
        let return_focus = self
            .focus_manager
            .observed_focus_token()
            .unwrap_or_else(|| self.focus_manager.workspace_token());
        let root_handle = cx.entity().downgrade();
        let windows = self.windows.clone();
        let view_states = self.side_view_persistence.settings_view_states();
        let initial_values =
            super::super::settings_controls::initial_field_values(self.settings.values());
        let view = cx.new(|cx| {
            SettingsView::new_overlay(
                window,
                cx,
                view_states,
                SettingsViewRoots {
                    root: root_handle,
                    windows,
                },
                initial_values,
            )
        });
        let registration = self.focus_manager.register(
            &view.read(cx).focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self
            .focus_manager
            .request_focus(registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let close = cx.subscribe_in(&view, window, |root, _, event, window, cx| match event {
            SettingsViewEvent::Close => {
                if let Err(error) = root.close_settings_overlay(window, cx) {
                    log::warn!("设置浮层关闭失败: {error:?}");
                }
            }
        });
        let restored_page = view.read(cx).current_page();
        self.settings_overlay = Some(SettingsOverlayHost {
            view,
            registration,
            return_focus,
            _close: close,
        });
        if restored_page == SettingsPageId::AgentBasic {
            self.request_agent_hooks_inspect(cx);
            self.ensure_agent_files_config_loading(cx);
        }
        cx.notify();
        Ok(())
    }

    pub(in crate::app_shell::app_root) fn close_settings_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.hook_drafts.owner_has_dirty(HooksDraftOwner::Overlay) {
            // 脏草稿先进入统一决定面；丢弃后再次关闭即正常释放浮层。
            self.open_close_decision(
                super::super::close_decision::CloseDecisionScope::SettingsOverlay,
                Vec::new(),
                Vec::new(),
                true,
                window,
                cx,
            )?;
            return Ok(());
        }
        let Some(host) = self.settings_overlay.take() else {
            return Ok(());
        };
        if let Err(error) = self.focus_manager.restore_focus(
            &[host.return_focus, self.focus_manager.workspace_token()],
            window,
            cx,
        ) {
            self.settings_overlay = Some(host);
            return Err(error.into());
        }
        self.focus_manager.unregister(host.registration.token())?;
        // 浮层关闭即释放其 Hooks 草稿与文档，不留下无所有者的应用级草稿。
        self.discard_hooks_owner_for_close(HooksDraftOwner::Overlay, cx);
        cx.notify();
        Ok(())
    }

    /// 字段修改的唯一入口：注册表校验 → 运行值与写队列 → 全局快照热应用。
    pub(in crate::app_shell::app_root) fn apply_settings_field(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<'_, Self>,
    ) {
        let registry = SettingsRegistry::new();
        let Some(def) = registry.lookup(key) else {
            return;
        };
        if registry.validate(key, &value) != Some(true) {
            return;
        }
        let now = Instant::now();
        let accepted = match def.save_mode {
            slterm_settings::SettingsSaveMode::Automatic => {
                self.settings.set_auto(key, value.clone(), now)
            }
            slterm_settings::SettingsSaveMode::Explicit => {
                self.settings.queue_explicit(&[(key, value.clone())], now)
            }
        };
        if !accepted {
            return;
        }
        // 联动清除：选择内置主题即撤销自定义主题覆盖，否则旧主题继续生效。
        for followup in theme_field_followups(key) {
            self.settings.clear_auto(followup, now);
        }
        match key {
            "launch_at_login" => {
                let enabled = matches!(value, Value::Bool(true));
                if let Err(error) = self.windows.set_login_start(enabled) {
                    log::warn!("更新开机启动失败: {error}");
                }
            }
            "tray" => self
                .windows
                .set_tray_enabled(matches!(value, Value::Bool(true))),
            _ => {}
        }
        let theme_commit = match (key, &value) {
            ("custom_theme", Value::String(id)) => Some(id.clone()),
            _ => None,
        };
        if let Some(id) = theme_commit {
            self.reload_theme_definition(id, cx);
        } else {
            self.reload_settings_snapshot(cx);
        }
        self.bump_settings_revision();
        self.notify_settings_views(cx);
        cx.notify();
    }

    /// 后台任务配置：只经外部段整段写回，并立即热应用到运行期原子量。
    pub(in crate::app_shell::app_root) fn apply_background_task_field(
        &mut self,
        task_id: &'static str,
        enabled: Option<bool>,
        interval_sec: Option<u64>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(def) = crate::background_tasks::find(task_id) else {
            return;
        };
        if enabled.is_none() && interval_sec.is_none() {
            return;
        }
        if let Some(sec) = interval_sec
            && !(def.interval_min..=def.interval_max).contains(&sec)
        {
            return;
        }
        let segment = crate::background_tasks::segment_with_task_config(
            self.settings
                .values()
                .external_value(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT),
            task_id,
            enabled,
            interval_sec,
        );
        let now = Instant::now();
        if !self.settings.queue_explicit(
            &[(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT, segment)],
            now,
        ) {
            return;
        }
        crate::background_tasks::apply_settings(self.settings.values());
        self.bump_settings_revision();
        self.notify_settings_views(cx);
        cx.notify();
    }

    /// 自定义主题的冷路径预载：读盘与解析只在后台执行，完成后安装缓存
    /// 并重建快照；前台提交路径不做任何主题文件 I/O。
    pub(in crate::app_shell::app_root) fn reload_theme_definition(
        &mut self,
        id: String,
        cx: &mut Context<'_, Self>,
    ) {
        let store = self.theme_store();
        let cancellation = super::super::task_service::CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let load_id = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    store.load(&load_id).and_then(|document| {
                        document
                            .resolve()
                            .map_err(crate::theme_library::StoreError::Document)
                    })
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.finish_theme_load(id, result, cx);
            });
        });
        if let Err(error) = self.tasks.register(
            super::super::task_service::TaskId::ThemeLoad,
            cancellation,
            task,
        ) {
            log::warn!("主题预载任务注册失败: {error:?}");
            self.reload_settings_snapshot(cx);
        }
    }

    fn finish_theme_load(
        &mut self,
        id: String,
        result: Result<slterm_settings::ThemeDefinition, crate::theme_library::StoreError>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks
            .complete(super::super::task_service::TaskId::ThemeLoad);
        match result {
            Ok(definition) => crate::theme_library::cache::install(&id, definition),
            Err(error) => log::warn!("主题 {id} 预载失败: {error}"),
        }
        self.reload_settings_snapshot(cx);
        cx.notify();
    }

    /// 主题库根：生产走默认数据目录；测试注入隔离根，不触碰真实 home。
    pub(in crate::app_shell::app_root) fn theme_store(
        &self,
    ) -> crate::theme_library::ThemeLibraryStore {
        #[cfg(test)]
        if let Some(root) = &self.theme_root_for_test {
            return crate::theme_library::ThemeLibraryStore::new(root);
        }
        crate::theme_library::ThemeLibraryStore::default()
    }

    /// 运行值 → 全局设置快照 + 主题/语言/托盘/后台任务即时生效。
    pub(in crate::app_shell::app_root) fn reload_settings_snapshot(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        let runtime = self.settings.runtime();
        let settings = crate::app_shell::config::Settings::from_runtime(&runtime);
        let language = settings.language;
        let tray = settings.tray;
        self.refresh_shell_from_settings(&settings, cx);
        self.apply_settings_to_terminal_views(&settings, cx);
        cx.set_global(settings);
        gpui_component::set_locale(language.gpui_component_locale());
        crate::notify::set_language(language);
        self.windows.set_tray_enabled(tray);
        crate::background_tasks::apply_settings(self.settings.values());
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.refresh_windows();
    }

    /// 运行值世代推进后通知全部设置视图重绘；控件回填发生在各自绘制帧。
    pub(in crate::app_shell::app_root) fn notify_settings_views(&self, cx: &mut Context<'_, Self>) {
        let mut views = Vec::new();
        if let Some(host) = &self.settings_overlay {
            views.push(host.view.clone());
        }
        for project in self.projects.values() {
            for panel in project.read(cx).panels.values() {
                if let PanelContent::Settings(view) = &panel.content {
                    views.push(view.clone());
                }
            }
        }
        // 延迟到效果周期：提交可能正发生在某个设置视图自身的更新中，
        // 同步重入该实体会触发 GPUI 的重复更新断言。
        cx.defer(move |cx| {
            for view in views {
                view.update(cx, |_, cx| cx.notify());
            }
        });
    }

    pub(in crate::app_shell::app_root) fn bump_settings_revision(&mut self) {
        self.settings_revision = self.settings_revision.wrapping_add(1);
    }

    /// 设置快照提交后向全部既有终端扇出一次，隐藏项目同样更新；
    /// 新建终端直接读取同一全局快照。
    fn apply_settings_to_terminal_views(
        &self,
        settings: &crate::app_shell::config::Settings,
        cx: &mut Context<'_, Self>,
    ) {
        let terminals = self
            .projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| panel.terminal().cloned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for terminal in terminals {
            terminal.update(cx, |view, cx| view.apply_settings(settings, cx));
        }
    }

    /// 读取失败后的显式重试：后台重读磁盘与冷路径主题，前台只结算结果。
    pub(in crate::app_shell::app_root) fn retry_settings_load(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        let cancellation = super::super::task_service::CancellationToken::new();
        let task = cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let result = crate::settings_store::load_settings();
                    if let Ok(loaded) = &result {
                        let runtime = slterm_settings::RuntimeSettings::from_raw(&loaded.data);
                        crate::app_shell::config::Settings::preload_theme_cold(&runtime);
                    }
                    result
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.finish_settings_load_retry(loaded, cx);
            });
        });
        if let Err(error) = self.tasks.register(
            super::super::task_service::TaskId::SettingsLoad,
            cancellation,
            task,
        ) {
            log::warn!("设置重读任务注册失败: {error:?}");
        }
    }

    pub(in crate::app_shell::app_root) fn finish_settings_load_retry(
        &mut self,
        result: std::io::Result<crate::settings_store::LoadResult>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks
            .complete(super::super::task_service::TaskId::SettingsLoad);
        match result {
            Ok(loaded) => {
                self.settings.adopt_loaded(loaded.data);
                self.settings_load_warning = if let Some(version) = loaded.unsupported_version {
                    log::warn!(
                        "settings.json 版本 {version} 高于当前程序支持；保持默认值且不会覆盖原文件"
                    );
                    Some(crate::settings_state::SettingsLoadWarning::UnsupportedVersion(version))
                } else if loaded.corrupted {
                    log::warn!("settings.json 损坏，重读已回退备份或默认值");
                    Some(crate::settings_state::SettingsLoadWarning::Corrupted)
                } else {
                    None
                };
                cx.set_global(crate::settings_state::SettingsLoadState {
                    warning: self.settings_load_warning,
                });
                self.reload_settings_snapshot(cx);
                self.bump_settings_revision();
                self.notify_settings_views(cx);
            }
            Err(error) => {
                log::warn!("重读设置失败: {error}");
                self.settings_load_warning =
                    Some(crate::settings_state::SettingsLoadWarning::ReadFailed);
            }
        }
        cx.notify();
    }

    /// 保存失败后的显式重试：同一待写批次立即到期，由下一节拍走同一后台写盘链。
    pub(in crate::app_shell::app_root) fn retry_settings_save(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        self.settings.retry_pending(Instant::now());
        self.notify_settings_views(cx);
        cx.notify();
    }

    /// 设置去抖到期后交给后台写盘；前台只提交快照与结算回执。
    pub(in crate::app_shell::app_root) fn poll_settings_save(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(entries) = self.settings.begin_save(Instant::now()) else {
            return;
        };
        let cancellation = super::super::task_service::CancellationToken::new();
        let written = entries.clone();
        let task = cx.spawn(async move |this, cx| {
            let to_write = written.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let pairs: Vec<(&str, Value)> = to_write
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.clone()))
                        .collect();
                    crate::settings_store::save_settings_blocking(&pairs)
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.settings.finish_save(&written, result, Instant::now());
                root.tasks
                    .complete(super::super::task_service::TaskId::SettingsSave);
                root.bump_settings_revision();
                root.notify_settings_views(cx);
                cx.notify();
            });
        });
        if let Err(error) = self.tasks.register(
            super::super::task_service::TaskId::SettingsSave,
            cancellation,
            task,
        ) {
            // 注册失败时恢复在途标记，下一节拍重试同一批快照。
            self.settings.abort_save();
            log::warn!("设置保存任务注册失败: {error:?}");
        }
    }

    /// 恢复或新建的项目设置面板若停在 AgentHooks 页，打开即探测安装状态。
    pub(in crate::app_shell::app_root) fn request_agent_hooks_if_selected(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<'_, Self>,
    ) {
        let view = self.projects.get(&project_id).and_then(|project| {
            project
                .read(cx)
                .panels
                .values()
                .find_map(|panel| match &panel.content {
                    PanelContent::Settings(view) => Some(view.clone()),
                    _ => None,
                })
        });
        let Some(view) = view else {
            return;
        };
        if view.read(cx).current_page() == SettingsPageId::AgentBasic {
            self.request_agent_hooks_inspect(cx);
            self.ensure_agent_files_config_loading(cx);
        }
    }

    pub(in crate::app_shell::app_root) fn render_settings_overlay(
        &self,
        cx: &App,
    ) -> Option<AnyElement> {
        let host = self.settings_overlay.as_ref()?;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().background.opacity(0.85))
                .debug_selector(|| "app-settings-overlay-backdrop".to_owned())
                .child(
                    div()
                        .w(relative(0.8))
                        .h(relative(0.8))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .p_4()
                        .debug_selector(|| "app-settings-overlay".to_owned())
                        .child(host.view.clone()),
                )
                .into_any_element(),
        )
    }
}
