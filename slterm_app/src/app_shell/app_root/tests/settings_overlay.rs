//! 无项目设置浮层的应用级所有权与焦点测试。

use std::time::Instant;

use slterm_agents::AgentKind;

use super::*;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer};

fn install_user_hooks_draft(root: &Entity<AppRoot>, cx: &mut TestAppContext) -> HooksDraftKey {
    let panel = PanelIdAllocator::new(31).allocate().unwrap();
    let key = HooksDraftKey {
        panel,
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.update(cx, |root, _| {
        let text = r#"{"Stop":[]}"#;
        root.hook_drafts.insert(
            key,
            HooksDraft::loaded(
                None,
                text.to_owned(),
                crate::ai_hook::draft::parse_hooks_json(text).unwrap(),
            ),
        );
    });
    key
}

fn settings_panel(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: ProjectId,
) -> Option<PanelId> {
    root.read_with(cx, |root, cx| {
        root.projects.get(&project).and_then(|view| {
            view.read(cx)
                .panels
                .values()
                .find(|panel| {
                    matches!(
                        panel.view_key,
                        ViewKey::Settings { project_id } if project_id == project
                    )
                })
                .map(|panel| panel.id)
        })
    })
}

/// 设置单例：无项目复用浮层，有项目按项目复用面板，浮层与草稿保留。
#[gpui::test]
fn integration_settings_singleton(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let overlay_id = root.read_with(cx, |root, _| {
        root.settings_overlay
            .as_ref()
            .expect("无项目时创建设置浮层")
            .view
            .entity_id()
    });
    open_settings(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root
            .settings_overlay
            .as_ref()
            .unwrap()
            .view
            .entity_id()),
        overlay_id,
        "无项目重复入口必须复用同一浮层"
    );

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    activate_project(&root, &mut window, PROJECT_A);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let project_a =
        settings_panel(&root, cx, project_id(PROJECT_A)).expect("有活跃项目时打开项目设置面板");
    open_settings(&mut window);
    assert_eq!(
        settings_panel(&root, cx, project_id(PROJECT_A)),
        Some(project_a),
        "同项目重复入口必须复用同一设置面板"
    );

    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let project_b =
        settings_panel(&root, cx, project_id(PROJECT_B)).expect("新活跃项目必须有自己的设置面板");
    assert_ne!(project_a, project_b, "不同项目不得共享设置面板");
    assert!(
        settings_panel(&root, cx, project_id(PROJECT_A)).is_some(),
        "隐藏项目的设置面板必须保活"
    );
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "项目设置面板不得销毁应用级浮层"
    );
}

/// 重启恢复：持久布局中的设置面板按 ViewKey 重建，不启动任何 PTY。
#[gpui::test]
fn settings_panel_restores_from_snapshot(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("settings-restore");
    std::fs::create_dir(&project).unwrap();
    let mut snapshot = ProjectSnapshot::default();
    snapshot
        .registry_mut()
        .add_project(project_id(PROJECT_A), CanonicalPath::new(project).unwrap())
        .unwrap();
    snapshot = snapshot
        .with_opened_view(
            project_id(PROJECT_A),
            group_id(GROUP_A),
            ViewKey::Settings {
                project_id: project_id(PROJECT_A),
            },
        )
        .unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, window, state) = open_root_with_persistence(cx, persistence);
    window.run_until_parked();
    assert_eq!(state.borrow().attempts, 0);
    assert!(
        settings_panel(&root, cx, project_id(PROJECT_A)).is_some(),
        "持久布局中的设置面板必须重建"
    );
}

/// 设置面板渲染完整页面列表，点击切换选中页。
#[gpui::test]
fn settings_panel_lists_pages_and_switches(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/settings");
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let settings = root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find(|panel| matches!(panel.view_key, ViewKey::Settings { .. }))
            .unwrap();
        let super::super::panel_host::PanelContent::Settings(view) = &panel.content else {
            panic!("设置面板内容形态错误");
        };
        view.clone()
    });
    assert_eq!(
        settings.read_with(cx, |view, _| view.selected_page_for_test()),
        crate::settings_state::SettingsPageId::Appearance
    );
    for index in 0..crate::settings_state::SettingsPageId::ALL.len() {
        let selector = Box::leak(format!("settings-page-{index}").into_boxed_str());
        assert!(
            window.debug_bounds(selector).is_some(),
            "设置页列表必须渲染第 {index} 项"
        );
    }
    let target = window.debug_bounds("settings-page-2").unwrap();
    window.simulate_click(target.center(), Modifiers::default());
    draw_frame(&mut window);
    assert_eq!(
        settings.read_with(cx, |view, _| view.selected_page_for_test()),
        crate::settings_state::SettingsPageId::Interaction,
        "点击必须切换选中页"
    );
}

/// 浮层重复入口不重置 Hooks 草稿与全局待写；项目激活不销毁浮层。
#[gpui::test]
fn app_settings_overlay_keeps_hooks_draft(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let key = install_user_hooks_draft(&root, cx);
    root.update(cx, |root, _| {
        assert!(root.settings.set_auto(
            "font_size",
            slterm_settings::json::Value::Number(15.0),
            Instant::now(),
        ));
    });

    open_settings(&mut window);
    root.read_with(cx, |root, _| {
        assert!(root.settings_overlay.is_some(), "重复入口必须复用同一浮层");
        assert!(
            root.hook_drafts.get(key).is_some(),
            "重复入口不得重置浮层 Hooks 草稿"
        );
        assert_eq!(root.settings.pending_len(), 1);
    });
}

/// 项目激活与切换后浮层、Hooks 草稿与全局写队列全部保留。
#[gpui::test]
fn settings_overlay_keeps_draft_and_queue(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let key = install_user_hooks_draft(&root, cx);
    root.update(cx, |root, _| {
        assert!(root.settings.set_auto(
            "font_size",
            slterm_settings::json::Value::Number(15.0),
            Instant::now(),
        ));
    });
    let entity_id = root.read_with(&*cx, |root, _| {
        root.settings_overlay.as_ref().unwrap().view.entity_id()
    });

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    activate_project(&root, &mut window, PROJECT_A);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings_overlay
                .as_ref()
                .map(|host| host.view.entity_id()),
            Some(entity_id),
            "激活或切换项目不得销毁无项目浮层"
        );
        assert_eq!(
            root.hook_drafts.get(key).map(|draft| draft.text()),
            Some(r#"{"Stop":[]}"#),
            "项目激活不得丢失浮层 Hooks 草稿"
        );
        assert_eq!(
            root.settings.pending_len(),
            1,
            "关闭或切换视图不得取消全局待写"
        );
    });
}

/// 字段控件真实写入运行值、全局快照与唯一写队列；页面切换与滚动容器渲染。
#[gpui::test]
fn settings_overlay_controls_write_runtime_values_and_queue(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let before = root.read_with(cx, |root, _| root.settings.runtime().copy_on_select);

    // 切到 Interaction 页后点击「选中即复制」开关。
    let page = window.debug_bounds("settings-page-2").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-content").is_some(),
        "页面内容必须渲染"
    );
    let toggle = window
        .debug_bounds("settings-field-copy_on_select-toggle")
        .expect("Interaction 页必须渲染 copy_on_select 开关");
    window.simulate_click(toggle.center(), Modifiers::default());
    draw_frame(&mut window);

    let runtime_value = root.read_with(cx, |root, _| root.settings.runtime().copy_on_select);
    let global_value = cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .copy_on_select
    });
    assert_ne!(runtime_value, before, "点击开关必须翻转运行值");
    assert_eq!(runtime_value, global_value, "全局快照必须与运行值同源");
    root.read_with(cx, |root, _| {
        assert_eq!(root.settings.pending_len(), 1, "修改必须进入唯一写队列");
    });

    // Scrolling 页的步进控件：点击 + 必须按步长推进运行值。
    let page = window.debug_bounds("settings-page-1").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let before = root.read_with(cx, |root, _| root.settings.runtime().scroll_speed);
    let up = window
        .debug_bounds("settings-field-scroll_speed-up")
        .expect("Scrolling 页必须渲染 scroll_speed 步进控件");
    window.simulate_click(up.center(), Modifiers::default());
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        let after = root.settings.runtime().scroll_speed;
        assert!(
            after > before || (before - slterm_settings::MAX_SCROLL_SPEED).abs() < 1e-6,
            "步进必须推进或已在上限: {before} -> {after}"
        );
    });
    let runtime_speed = root.read_with(cx, |root, _| root.settings.runtime().scroll_speed);
    let global_speed = cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .scroll_speed
    });
    assert_eq!(runtime_speed, global_speed, "全局快照必须与运行值同源");
}

/// 活动栏固定底部配置按钮与 Ctrl+, 共用同一设置入口。
#[gpui::test]
fn activity_bar_settings_button_opens_the_settings_overlay(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    draw_frame(&mut window);
    let button = window
        .debug_bounds("app-root-settings-button")
        .expect("活动栏必须有固定底部配置按钮");
    window.simulate_click(button.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-content").is_some(),
        "点击配置按钮必须打开设置浮层"
    );
    // 键盘入口复用同一浮层：再次触发不创建第二个实体。
    focus_workspace(&root, &mut window);
    window.simulate_keystrokes("ctrl-,");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-page-content").is_some(),
        "Ctrl+, 必须聚焦既有设置浮层"
    );
}

/// 自定义主题提交在后台预载；后续无关字段热应用只查缓存，不读主题文件。
#[gpui::test]
fn custom_theme_hot_apply_uses_the_background_preloaded_cache(cx: &mut TestAppContext) {
    let (root, window, _state) = open_root(cx);
    let directory = tempfile::tempdir().unwrap();
    let themes = directory.path().join("themes");
    let store = crate::theme_library::ThemeLibraryStore::new(&themes);
    let mut definition =
        slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
    definition.name = "Cached".to_owned();
    definition.terminal.background = [0x11, 0x22, 0x33];
    let document = store
        .import(&crate::theme_library::from_definition(&definition).unwrap())
        .unwrap();
    let id = document.id().unwrap().to_owned();
    let theme_id = document.theme_id().unwrap();
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "custom_theme",
            slterm_settings::json::Value::String(id.clone()),
            cx,
        );
    });
    window.run_until_parked();
    let background = cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .palette
            .background
    });
    assert_eq!(
        background,
        crate::gpui_terminal::colors::from_rgb8([0x11, 0x22, 0x33]),
        "后台预载完成后快照必须采用自定义主题"
    );

    // 主题文件在外部消失后，改无关字段不得回读文件，缓存继续生效。
    std::fs::remove_file(store.path_for(theme_id).unwrap()).unwrap();
    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "scroll_speed",
            slterm_settings::json::Value::Number(1.5),
            cx,
        );
    });
    let background = cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .palette
            .background
    });
    assert_eq!(
        background,
        crate::gpui_terminal::colors::from_rgb8([0x11, 0x22, 0x33]),
        "热应用不得重新读取主题文件"
    );
}

/// AgentHooks 页：状态快照与授权开关只经注入端口，不触碰真实用户配置。
#[gpui::test]
fn agent_hooks_page_renders_status_and_toggles_through_injected_ports(cx: &mut TestAppContext) {
    static APPLIED: std::sync::Mutex<Vec<(slterm_settings::AgentHook, bool)>> =
        std::sync::Mutex::new(Vec::new());
    fn inspect() -> Vec<crate::ai_hook::integrations::AgentIntegration> {
        use crate::ai_hook::integrations::{AgentIntegration, HookInspection};
        vec![
            AgentIntegration {
                agent: slterm_settings::AgentHook::Claude,
                executable: Some(std::path::PathBuf::from("claude")),
                inspection: HookInspection {
                    available: true,
                    installed: true,
                    enabled: true,
                    ..HookInspection::default()
                },
            },
            AgentIntegration {
                agent: slterm_settings::AgentHook::Codex,
                executable: None,
                inspection: HookInspection::default(),
            },
        ]
    }
    fn apply(hook: slterm_settings::AgentHook, enabled: bool) -> Result<(), String> {
        APPLIED.lock().unwrap().push((hook, enabled));
        Ok(())
    }
    APPLIED.lock().unwrap().clear();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_agent_hooks_for_test(inspect, apply)
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window.debug_bounds("settings-page-5").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    for slug in ["claude", "codex"] {
        assert!(
            window
                .debug_bounds(Box::leak(format!("settings-agent-{slug}").into_boxed_str()))
                .is_some(),
            "AgentHooks 页必须渲染 {slug} 行"
        );
        assert!(
            window
                .debug_bounds(Box::leak(
                    format!("settings-agent-{slug}-state").into_boxed_str()
                ))
                .is_some(),
            "AgentHooks 页必须渲染 {slug} 状态"
        );
    }

    let toggle = window.debug_bounds("settings-agent-claude-toggle").unwrap();
    window.simulate_click(toggle.center(), Modifiers::default());
    draw_frame(&mut window);

    assert_eq!(
        APPLIED.lock().unwrap().as_slice(),
        [(slterm_settings::AgentHook::Claude, false)],
        "开关必须经注入的安装端口执行一次卸载"
    );
    root.read_with(cx, |root, _| {
        assert_eq!(root.settings.pending_len(), 1, "显式字段必须立即入队");
        assert_eq!(
            root.settings
                .values()
                .bool_on(slterm_settings::AgentHook::Claude.settings_key()),
            Some(false),
            "授权键必须写入显式设置值"
        );
    });
}

/// Hooks 页：三层草稿载入、编辑保存、外部修改守卫与丢弃全部在临时目录。
#[gpui::test]
fn hooks_page_loads_saves_external_guard_and_discards(cx: &mut TestAppContext) {
    use crate::ai_hook::draft_host::{HooksDraftKey, HooksDraftLayer};

    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        br#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo user"}]}]}}"#,
    )
    .unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
    let project_settings = project_dir.join(".claude/settings.json");
    std::fs::write(
        &project_settings,
        br#"{"keep":true,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo old"}]}]}}"#,
    )
    .unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window.debug_bounds("settings-page-6").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let panel = settings_panel(&root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    let key = HooksDraftKey {
        panel,
        agent: slterm_agents::AgentKind::Claude,
        layer: HooksDraftLayer::Project(project_id(PROJECT_A)),
    };
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "Hooks 页必须渲染编辑区"
    );
    for selector in [
        "settings-hooks-save",
        "settings-hooks-discard",
        "settings-hooks-layer-user",
    ] {
        assert!(
            window
                .debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
                .is_some(),
            "Hooks 页必须渲染 {selector}"
        );
    }
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("Project 层草稿必须载入");
        assert!(
            draft.text().contains("echo old"),
            "草稿必须来自项目层文件: {}",
            draft.text()
        );
    });

    // 编辑并保存：未知顶层字段保留，钩子子树替换为新内容。
    let edited = r#"{"Stop":[{"hooks":[{"type":"command","command":"echo hi"}]}]}"#;
    root.update(cx, |root, cx| root.update_hooks_draft_text(key, edited, cx));
    draw_frame(&mut window);
    let save = window.debug_bounds("settings-hooks-save").unwrap();
    window.simulate_click(save.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&project_settings).unwrap()).unwrap();
    assert_eq!(
        saved["keep"],
        serde_json::json!(true),
        "未知顶层字段必须保留"
    );
    assert_eq!(
        saved["hooks"]["Stop"][0]["hooks"][0]["command"],
        serde_json::json!("echo hi")
    );
    root.read_with(cx, |root, _| {
        assert!(
            !root.hook_drafts.get(key).unwrap().dirty(),
            "保存成功后草稿必须回到干净态"
        );
    });

    // 外部修改守卫：保存前重读，文件保持外部内容且草稿保留。
    root.update(cx, |root, cx| {
        root.update_hooks_draft_text(key, r#"{"Stop":[]}"#, cx)
    });
    std::fs::write(&project_settings, br#"{"keep":false,"hooks":{}}"#).unwrap();
    root.update(cx, |root, cx| root.save_hooks_draft_for(key, cx));
    window.run_until_parked();
    assert_eq!(
        std::fs::read(&project_settings).unwrap(),
        br#"{"keep":false,"hooks":{}}"#,
        "外部修改必须零写入"
    );
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-error").is_some(),
        "外部修改必须给出可见错误"
    );
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.get(key).unwrap().dirty());
    });

    // 丢弃：移除草稿后按磁盘现状重新载入。
    root.update(cx, |root, cx| root.discard_hooks_draft_for(key, cx));
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("丢弃后必须重新载入");
        assert!(!draft.dirty());
        assert_eq!(draft.text(), "{}");
    });

    // 切 User 层：独立草稿来自注入的 Claude 配置目录。
    let user_layer = window.debug_bounds("settings-hooks-layer-user").unwrap();
    window.simulate_click(user_layer.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    let user_key = HooksDraftKey {
        panel,
        agent: slterm_agents::AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(user_key).expect("User 层草稿必须载入");
        assert!(
            draft.text().contains("echo user"),
            "User 层草稿必须来自注入的 Claude 目录: {}",
            draft.text()
        );
    });
}

fn open_settings(window: &mut VisualTestContext) {
    window.simulate_keystrokes("ctrl-,");
    draw_frame(window);
}

fn focus_workspace(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    window.update(|window, cx| {
        let focus = root.read(cx).workspace_focus.clone();
        focus.focus(window, cx);
    });
}

#[gpui::test]
fn settings_overlay_explicit_close_releases_instance_and_allows_reopen(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let (entity, focus) = root.read_with(cx, |root, cx| {
        let view = &root.settings_overlay.as_ref().unwrap().view;
        (view.downgrade(), view.read(cx).focus_handle.clone())
    });

    window.simulate_keystrokes("escape");
    window.run_until_parked();

    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_none()));
    assert!(entity.upgrade().is_none());
    assert_ne!(window.update(|window, cx| window.focused(cx)), Some(focus));

    open_settings(&mut window);
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_some()));
}

#[gpui::test]
fn app_overlay_does_not_close_on_project_activation(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    assert!(root.read_with(cx, |root, _| root.snapshot().registry().active().is_none()));

    focus_workspace(&root, &mut window);
    open_settings(&mut window);

    let (entity_id, focus, key) = root.read_with(cx, |root, cx| {
        let host = root.settings_overlay.as_ref().expect("必须创建设置浮层");
        (
            host.view.entity_id(),
            host.view.read(cx).focus_handle.clone(),
            host.view.read(cx).view_key(),
        )
    });
    assert_eq!(key, ViewKey::AppSettingsOverlay);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(focus.clone())
    );

    open_settings(&mut window);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings_overlay.as_ref().unwrap().view.entity_id(),
            entity_id,
            "重复入口不得重置既有设置视图"
        );
    });
    assert_eq!(window.update(|window, cx| window.focused(cx)), Some(focus));

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");

    root.read_with(cx, |root, cx| {
        let host = root
            .settings_overlay
            .as_ref()
            .expect("项目激活不得销毁浮层");
        assert_eq!(host.view.entity_id(), entity_id);
        assert_eq!(host.view.read(cx).view_key(), ViewKey::AppSettingsOverlay);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B))
        );
    });
}

/// 设置页对账：启动项被外部改成其他程序时显示实际状态，不拿设置值冒充。
#[gpui::test]
fn settings_overlay_reports_the_actual_login_start_state(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let windows = std::sync::Arc::new(
        crate::platform::windows_integration::FakeWindowsIntegration::default(),
    );
    windows.set_actual_state(crate::platform::startup::LoginStartState::OtherTarget(
        "other.exe".into(),
    ));
    root.update(cx, |root, _| root.replace_windows_for_test(windows.clone()));
    focus_workspace(&root, &mut window);
    open_settings(&mut window);

    let page = window
        .debug_bounds("settings-page-3")
        .expect("Session 页入口必须存在");
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-field-launch_at_login-actual")
            .is_some(),
        "启动项被外部修改后必须显示实际状态说明"
    );
}

/// 托盘不可用：静默启动与托盘字段都必须说明回退到可见窗口。
#[gpui::test]
fn settings_overlay_explains_the_unusable_tray_fallback(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let windows = std::sync::Arc::new(
        crate::platform::windows_integration::FakeWindowsIntegration::default(),
    );
    windows.set_tray_available(false);
    root.update(cx, |root, cx| {
        root.replace_windows_for_test(windows.clone());
        root.apply_settings_field("silent_start", slterm_settings::json::Value::Bool(true), cx);
        root.apply_settings_field("tray", slterm_settings::json::Value::Bool(true), cx);
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);

    let page = window
        .debug_bounds("settings-page-3")
        .expect("Session 页入口必须存在");
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-field-silent_start-actual")
            .is_some(),
        "托盘不可用时静默启动必须说明回退"
    );

    let page = window
        .debug_bounds("settings-page-4")
        .expect("System 页入口必须存在");
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-field-tray-actual").is_some(),
        "托盘不可用时托盘字段必须说明"
    );
}

/// 材质不支持：档位置灰不可点，当前不支持档位显示回退说明。
#[gpui::test]
fn settings_overlay_disables_unsupported_window_material(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let windows = std::sync::Arc::new(
        crate::platform::windows_integration::FakeWindowsIntegration::default(),
    );
    windows.set_blur_support(crate::platform::material::BlurSupport::NoneFallback);
    root.update(cx, |root, cx| {
        root.replace_windows_for_test(windows.clone());
        root.apply_settings_field(
            "blur",
            slterm_settings::json::Value::String("mica".to_owned()),
            cx,
        );
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("settings-field-blur-actual").is_some(),
        "不支持档位必须显示说明"
    );
    let mica = window
        .debug_bounds("settings-choice-blur-mica")
        .expect("mica 档位必须渲染");
    window.simulate_click(mica.center(), Modifiers::default());
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.settings.runtime().blur),
        slterm_settings::BlurMode::Mica,
        "禁用档位不得被点击改写"
    );

    // 可用档位仍可正常切换。
    let aero = window
        .debug_bounds("settings-choice-blur-aero")
        .expect("aero 档位必须渲染");
    window.simulate_click(aero.center(), Modifiers::default());
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.settings.runtime().blur),
        slterm_settings::BlurMode::Aero
    );
}

/// 字段写入经 WindowsIntegration 端口：启动项与托盘副作用同源。
#[gpui::test]
fn settings_field_writes_use_the_windows_integration_port(cx: &mut TestAppContext) {
    let (root, _window, _state) = open_root(cx);
    let windows = std::sync::Arc::new(
        crate::platform::windows_integration::FakeWindowsIntegration::default(),
    );
    root.update(cx, |root, _| root.replace_windows_for_test(windows.clone()));

    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "launch_at_login",
            slterm_settings::json::Value::Bool(true),
            cx,
        );
    });
    assert_eq!(
        windows.current_login_start_state(),
        crate::platform::startup::LoginStartState::Enabled,
        "开机启动写入必须经端口"
    );

    root.update(cx, |root, cx| {
        root.apply_settings_field("tray", slterm_settings::json::Value::Bool(true), cx);
    });
    assert!(windows.tray_enabled(), "托盘启用必须经端口");
}
