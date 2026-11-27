//! 无项目设置浮层的应用级所有权与焦点测试。

use std::time::Instant;

use slterm_agents::AgentKind;
use slterm_settings::SettingsRegistry;
use slterm_settings::json::Value;

use super::*;
use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey, HooksDraftLayer, HooksDraftOwner};
use crate::settings_state::{SettingsPageId, SettingsViewState};

fn install_user_hooks_draft(root: &Entity<AppRoot>, cx: &mut TestAppContext) -> HooksDraftKey {
    let panel = PanelIdAllocator::new(31).allocate().unwrap();
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
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

/// 读取失败提供显式重试入口；重读成功后采用磁盘值并清除警示。
#[gpui::test]
fn settings_read_failure_offers_retry_and_adopts_loaded_values(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.settings_load_warning = Some(crate::settings_state::SettingsLoadWarning::ReadFailed);
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-retry-load").is_some(),
        "读取失败必须提供重试入口"
    );

    root.update(cx, |root, cx| {
        root.finish_settings_load_retry(
            Ok(crate::settings_store::LoadResult {
                data: slterm_settings::RawSettings::from_json_bytes(
                    br#"{"appearance":{"theme":"Nord"}}"#,
                ),
                corrupted: false,
                unsupported_version: None,
            }),
            cx,
        );
    });
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(root.settings_load_warning, None);
        assert_eq!(
            root.settings.runtime().theme,
            slterm_settings::ThemeName::Nord
        );
    });
    assert!(
        window.debug_bounds("settings-retry-load").is_none(),
        "重读成功后不得继续显示重试入口"
    );
}

/// 加载降级、单字段回退与写盘失败都必须在设置页出现可见状态条。
#[gpui::test]
fn settings_status_surfaces_load_fallback_and_save_failure(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.settings_load_warning =
            Some(crate::settings_state::SettingsLoadWarning::UnsupportedVersion(9));
        root.settings.set_auto(
            "ui_font_size",
            slterm_settings::json::Value::Number(16.0),
            Instant::now(),
        );
        root.settings.finish_save(
            &[],
            Err(std::io::Error::other("injected settings write failure")),
            Instant::now(),
        );
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("settings-status").is_some(),
        "设置页必须显示加载降级与写失败状态"
    );
}

/// 无项目浮层 Hooks 页使用 Overlay 草稿键，关闭浮层即释放草稿。
#[gpui::test]
fn app_overlay_hooks_page_uses_overlay_draft_and_releases_it_on_close(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        br#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo overlay"}]}]}}"#,
    )
    .unwrap();
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let key = HooksDraftKey {
        owner: HooksDraftOwner::Overlay,
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.read_with(cx, |root, _| {
        let draft = root
            .hook_drafts
            .get(key)
            .expect("浮层 Hooks 页必须载入 Overlay 草稿");
        assert!(
            draft.text().contains("echo overlay"),
            "Overlay 草稿必须来自注入的 User 层文件: {}",
            draft.text()
        );
    });

    let focus = root.read_with(cx, |root, cx| {
        root.settings_overlay
            .as_ref()
            .expect("浮层必须存在")
            .view
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| focus.focus(window, cx));
    window.simulate_keystrokes("escape");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_none()));
    root.read_with(cx, |root, _| {
        assert!(
            root.hook_drafts.get(key).is_none(),
            "关闭浮层必须释放 Overlay 草稿"
        );
    });
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
    assert!(
        settings_panel(&root, cx, project_id(PROJECT_A)).is_none(),
        "浮层打开期间不得改开项目设置面板"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .settings_overlay
            .as_ref()
            .unwrap()
            .view
            .entity_id()),
        overlay_id,
        "浮层存在时重复入口只能还焦既有浮层"
    );
    // 关闭浮层后的下一次入口才使用项目 Settings 面板。
    let focus = root.read_with(cx, |root, cx| {
        root.settings_overlay
            .as_ref()
            .expect("浮层必须存在")
            .view
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| focus.focus(window, cx));
    window.simulate_keystrokes("escape");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_none()));
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let project_a =
        settings_panel(&root, cx, project_id(PROJECT_A)).expect("浮层关闭后打开项目设置面板");
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
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "项目设置面板入口不得重建应用级浮层"
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
    for page in crate::settings_state::SettingsPageId::ALL {
        let selector = Box::leak(format!("settings-page-{}", page.slug()).into_boxed_str());
        assert!(
            window.debug_bounds(selector).is_some(),
            "设置页列表必须渲染 {}",
            page.slug()
        );
    }
    let target = window.debug_bounds("settings-page-terminal").unwrap();
    window.simulate_click(target.center(), Modifiers::default());
    draw_frame(&mut window);
    assert_eq!(
        settings.read_with(cx, |view, _| view.selected_page_for_test()),
        crate::settings_state::SettingsPageId::Terminal,
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
    let page = window.debug_bounds("settings-page-terminal").unwrap();
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
    let page = window.debug_bounds("settings-page-terminal").unwrap();
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

/// IME 组合期间全局键绑定在派发前被拦截，组合结束才恢复命令。
#[gpui::test]
fn ime_composition_blocks_global_command_dispatch(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    cx.update(|cx| {
        cx.set_global(crate::gpui_terminal::ImeComposition { active: true });
    });
    window.simulate_keystrokes("ctrl-,");
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "组合期间不得派发全局命令"
    );
    cx.update(|cx| {
        cx.set_global(crate::gpui_terminal::ImeComposition { active: false });
    });
    focus_workspace(&root, &mut window);
    window.simulate_keystrokes("ctrl-,");
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "组合结束后命令必须恢复"
    );
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

/// 设置快照提交必须扇出到既有终端视图：字号与主题立即生效。
#[gpui::test]
fn terminal_views_hot_apply_settings_snapshot(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    activate_project(&root, &mut window, PROJECT_A);
    let terminals = root.read_with(cx, |root, cx| {
        root.projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| panel.terminal().cloned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(terminals.len(), 2, "两个项目各有一个终端");
    let background_before = terminals[0].read_with(cx, |view, _| view.palette.background);
    for terminal in &terminals {
        terminal.read_with(cx, |view, _| {
            assert_ne!(view.font_size, gpui::px(20.0));
        });
    }

    root.update(cx, |root, cx| {
        root.apply_settings_field("font_size", slterm_settings::json::Value::Number(20.0), cx);
        root.apply_settings_field(
            "theme",
            slterm_settings::json::Value::String("Nord".to_owned()),
            cx,
        );
    });
    window.run_until_parked();

    for terminal in &terminals {
        terminal.read_with(cx, |view, _| {
            assert_eq!(
                view.font_size,
                gpui::px(20.0),
                "既有终端（含隐藏项目）必须热应用字号"
            );
            assert_ne!(
                view.palette.background, background_before,
                "既有终端必须热应用主题调色板"
            );
            assert_eq!(
                view.font.family.as_ref(),
                crate::app_shell::assets::REQUIRED_FONT_FAMILY
            );
        });
    }
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

    let page = window.debug_bounds("settings-page-agent-basic").unwrap();
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

    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let panel = settings_panel(&root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
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
        "settings-hooks-reload",
        "settings-hooks-layer-user",
        "settings-hooks-layer-project",
        "settings-hooks-layer-local",
    ] {
        assert!(
            window
                .debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
                .is_some(),
            "Hooks 页必须渲染 {selector}"
        );
    }
    // 项目面板重建视图默认回 User 层；User 草稿来自注入的 Claude 目录。
    let user_key = HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
        agent: slterm_agents::AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(user_key).expect("User 层草稿必须载入");
        assert!(
            draft.text().contains("echo user"),
            "默认层必须是 User 层文件: {}",
            draft.text()
        );
    });

    assert!(
        window.debug_bounds("settings-hooks-user-warning").is_some(),
        "User 层必须显示任意命令执行警告"
    );

    // 切到项目层后使用项目文件草稿。
    let project_layer = window.debug_bounds("settings-hooks-layer-project").unwrap();
    window.simulate_click(project_layer.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-user-warning").is_none(),
        "Project 层不显示 User 专属警告"
    );
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
            root.hook_drafts.get(key).is_none(),
            "保存成功后草稿必须清除"
        );
    });
    assert!(
        window.debug_bounds("settings-hooks-saved-hint").is_some(),
        "保存成功后必须提示重启 CLI 会话"
    );

    // 保存期间继续编辑：迟到回执不得覆盖新文本。
    let in_flight = r#"{"Stop":[{"hooks":[{"type":"command","command":"echo slow"}]}]}"#;
    root.update(cx, |root, cx| {
        root.update_hooks_draft_text(key, in_flight, cx)
    });
    root.update(cx, |root, cx| root.save_hooks_draft_for(key, cx));
    let newer = r#"{"Stop":[{"hooks":[{"type":"command","command":"echo newer"}]}]}"#;
    root.update(cx, |root, cx| root.update_hooks_draft_text(key, newer, cx));
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).unwrap();
        assert_eq!(
            draft.text(),
            newer,
            "保存回执不得覆盖保存期间继续输入的文本"
        );
        assert!(draft.dirty(), "保存期间的编辑必须保持脏态以待再次保存");
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
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(user_key).expect("User 层草稿必须载入");
        assert!(
            draft.text().contains("echo user"),
            "User 层草稿必须来自注入的 Claude 目录: {}",
            draft.text()
        );
    });

    // 切到 User 后项目层入口必须仍在，且可切回项目层。
    let project_layer = window
        .debug_bounds("settings-hooks-layer-project")
        .expect("切到 User 后 Project 层 pill 必须仍然可用");
    window.simulate_click(project_layer.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("项目层草稿必须仍可恢复");
        assert_eq!(draft.text(), "{}", "项目层丢弃后重新载入磁盘现状");
    });
}

/// 关闭 Settings 面板必须释放该面板的 Hooks 草稿与文档，不留下孤儿键。
#[gpui::test]
fn closing_settings_panel_releases_hooks_drafts(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), br#"{"hooks":{}}"#).unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let panel = settings_panel(&root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    root.read_with(cx, |root, _| {
        assert!(
            root.hook_drafts
                .keys()
                .iter()
                .any(|key| key.panel() == Some(panel)),
            "前置：Hooks 页必须已载入草稿"
        );
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_view_from_request(
                ViewKey::Settings {
                    project_id: project_id(PROJECT_A),
                },
                BTreeMap::new(),
                window,
                cx,
            )
            .expect("设置面板必须可关闭");
        });
    });
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while settings_panel(&root, cx, project_id(PROJECT_A)).is_some() {
        assert!(Instant::now() < deadline, "设置面板必须完成关闭");
        window.run_until_parked();
    }
    root.read_with(cx, |root, _| {
        assert!(
            !root
                .hook_drafts
                .keys()
                .iter()
                .any(|key| key.panel() == Some(panel)),
            "面板销毁后不得保留孤儿草稿"
        );
        assert!(
            !root.hooks_state_has_panel_entries(panel),
            "面板销毁后不得保留 Hooks 文档或诊断"
        );
    });
}

/// 项目根在编辑期间被删除：Project 层禁用、保存拒绝且不得重建配置目录。
#[gpui::test]
fn hooks_project_root_loss_disables_layers_and_never_recreates_config(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), br#"{"hooks":{}}"#).unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
    let project_settings = project_dir.join(".claude/settings.json");
    std::fs::write(&project_settings, br#"{"hooks":{}}"#).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let panel = settings_panel(&root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::Project(project_id(PROJECT_A)),
    };
    let project_layer = window.debug_bounds("settings-hooks-layer-project").unwrap();
    window.simulate_click(project_layer.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.get(key).is_some(), "项目层草稿必须载入")
    });

    root.update(cx, |root, cx| {
        root.update_hooks_draft_text(key, r#"{"Stop":[]}"#, cx)
    });
    std::fs::remove_dir_all(&project_dir).unwrap();
    root.update(cx, |root, cx| root.save_hooks_draft_for(key, cx));
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(!project_settings.exists(), "缺失项目根下不得重建配置文件");
    assert!(!project_dir.exists(), "缺失项目根不得被重新创建");
    assert!(
        window.debug_bounds("settings-hooks-error").is_some(),
        "保存失败必须可见"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-root-unavailable")
            .is_some(),
        "Project／Local 层必须标明已禁用"
    );
}

/// User 层配置目录不存在：载入失败可见且不得创建目录。
#[gpui::test]
fn hooks_missing_user_config_dir_is_reported_without_creating_it(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-claude");
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(missing.clone())
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("settings-hooks-error").is_some(),
        "缺失配置目录必须给出可见错误"
    );
    assert!(!missing.exists(), "缺失配置目录不得被创建");
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

/// 浮层选中页关闭重开与重启恢复共用同一应用级 ViewState。
#[gpui::test]
fn settings_overlay_page_survives_close_reopen_and_restart(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let page = window
        .debug_bounds("settings-page-terminal")
        .expect("Session 页入口必须存在");
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    root.read_with(cx, |root, cx| {
        let host = root.settings_overlay.as_ref().expect("浮层必须存在");
        assert_eq!(host.view.read(cx).current_page(), SettingsPageId::Terminal);
    });

    window.simulate_keystrokes("escape");
    window.run_until_parked();
    open_settings(&mut window);
    draw_frame(&mut window);
    root.read_with(cx, |root, cx| {
        let host = root.settings_overlay.as_ref().expect("浮层必须重新打开");
        assert_eq!(
            host.view.read(cx).current_page(),
            SettingsPageId::Terminal,
            "关闭重开必须恢复上次选中的页"
        );
    });

    // 重启：启动恢复把同一 ViewKey 的状态预载进设置视图存储。
    let persistence = AppRootPersistence::empty(None);
    persistence.settings_view_states().restore(
        ViewKey::AppSettingsOverlay,
        SettingsViewState::new(SettingsPageId::Terminal),
    );
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    root.read_with(cx, |root, cx| {
        let host = root.settings_overlay.as_ref().expect("浮层必须存在");
        assert_eq!(
            host.view.read(cx).current_page(),
            SettingsPageId::Terminal,
            "重启后必须恢复持久选中页"
        );
    });
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
        .debug_bounds("settings-page-terminal")
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
        .debug_bounds("settings-page-terminal")
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
        .debug_bounds("settings-page-application")
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

/// 项目设置面板存在脏 Hooks 草稿时必须拒绝关闭并保留现场。
#[gpui::test]
fn dirty_hooks_drafts_block_panel_close(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), br#"{"hooks":{}}"#).unwrap();
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-hooks").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    let panel = settings_panel(&root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    let panel_key = root.update(cx, |root, _| {
        let key = root
            .hook_drafts
            .keys()
            .into_iter()
            .find(|key| key.panel() == Some(panel))
            .expect("Hooks 页必须载入草稿");
        root.hook_drafts
            .get_mut(key)
            .expect("草稿必须存在")
            .set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        key
    });
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_view_from_request(
                ViewKey::Settings {
                    project_id: project_id(PROJECT_A),
                },
                BTreeMap::new(),
                window,
                cx,
            )
        })
    });
    assert!(result.is_ok(), "脏草稿必须先进入统一决定面");
    assert!(root.read_with(cx, |root, _| root.close_decision.is_some()));
    assert!(settings_panel(&root, cx, project_id(PROJECT_A)).is_some());
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.get(panel_key).is_some());
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.resolve_close_decision(
                crate::app_shell::app_root::close_decision::CloseDecisionChoice::Discard,
                window,
                cx,
            )
            .expect("丢弃草稿后面板必须可关闭");
        });
    });
    window.run_until_parked();
    assert!(settings_panel(&root, cx, project_id(PROJECT_A)).is_none());
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.get(panel_key).is_none());
    });
}

/// 应用级设置浮层存在脏 Hooks 草稿时，Escape 关闭必须被拒绝。
#[gpui::test]
fn dirty_hooks_drafts_block_overlay_close(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_some()));
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Overlay,
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.update(cx, |root, _| {
        let mut draft = HooksDraft::loaded(None, "{}".to_owned(), serde_json::json!({}));
        draft.set_text(r#"{"Stop":[]}"#);
        root.hook_drafts.insert(key, draft);
    });
    window.simulate_keystrokes("escape");
    window.run_until_parked();
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "脏 Hooks 草稿不得被浮层关闭静默丢弃"
    );
    assert!(root.read_with(cx, |root, _| root.close_decision.is_some()));
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.resolve_close_decision(
                crate::app_shell::app_root::close_decision::CloseDecisionChoice::Discard,
                window,
                cx,
            )
            .unwrap();
        });
    });
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_none()));
    assert!(root.read_with(cx, |root, _| root.hook_drafts.get(key).is_none()));
}

/// 应用关闭遇到任何脏 Hooks 草稿必须拒绝并保持窗口。
#[gpui::test]
fn dirty_hooks_drafts_block_application_close(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Overlay,
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.update(cx, |root, _| {
        let mut draft = HooksDraft::loaded(None, "{}".to_owned(), serde_json::json!({}));
        draft.set_text(r#"{"Stop":[]}"#);
        root.hook_drafts.insert(key, draft);
    });
    assert!(!window.simulate_close());
    assert!(!root.read_with(cx, |root, _| root.close_completed));
    assert!(!root.read_with(cx, |root, _| root.close_in_progress));
}

/// 所有者已销毁时不存在的面板不得由迟到载入回执复活草稿。
#[gpui::test]
fn hooks_load_receipt_never_resurrects_a_destroyed_owner(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), br#"{"hooks":{}}"#).unwrap();
    let (root, window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    let key = HooksDraftKey {
        owner: HooksDraftOwner::Panel(PanelIdAllocator::new(77).allocate().unwrap()),
        agent: AgentKind::Claude,
        layer: HooksDraftLayer::User,
    };
    root.update(cx, |root, cx| root.request_hooks_draft(key, cx));
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert!(root.hook_drafts.get(key).is_none(), "迟到载入不得复活草稿");
        assert!(!root.hooks_state_has_panel_entries(key.panel().unwrap()));
    });
}

/// 保存状态可见：待写批次显示保存中，成功回执后显示已保存，失败保留失败提示。
#[gpui::test]
fn settings_status_reports_saving_then_saved(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let view = root.read_with(cx, |root, _| {
        root.settings_overlay
            .as_ref()
            .expect("浮层必须存在")
            .view
            .clone()
    });
    let language = cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language);
    let saving = language
        .text(crate::i18n::Message::SettingsStatusSaving)
        .to_owned();
    let saved = language
        .text(crate::i18n::Message::SettingsStatusSaved)
        .to_owned();
    let failed = language
        .text(crate::i18n::Message::SettingsStatusSaveFailed)
        .to_owned();

    root.update(cx, |root, cx| {
        root.apply_settings_field("font_family", Value::String("StatusFont".to_owned()), cx);
    });
    draw_frame(&mut window);
    let messages = view.read_with(cx, |view, cx| view.status_messages_for_test(cx));
    assert_eq!(messages, vec![saving], "待写批次必须显示保存中");

    root.update(cx, |root, cx| {
        let now = Instant::now();
        let due = now + std::time::Duration::from_secs(5);
        let entries = root.settings.begin_save(due).expect("到期批次必须启动");
        root.settings.finish_save(&entries, Ok(()), due);
        root.notify_settings_views(cx);
    });
    draw_frame(&mut window);
    let messages = view.read_with(cx, |view, cx| view.status_messages_for_test(cx));
    assert_eq!(messages, vec![saved], "成功回执必须显示已保存");

    root.update(cx, |root, cx| {
        root.apply_settings_field("font_family", Value::String("FailFont".to_owned()), cx);
        let now = Instant::now();
        let due = now + std::time::Duration::from_secs(5);
        let entries = root.settings.begin_save(due).expect("到期批次必须启动");
        root.settings
            .finish_save(&entries, Err(std::io::Error::other("disk full")), due);
        root.notify_settings_views(cx);
    });
    draw_frame(&mut window);
    let messages = view.read_with(cx, |view, cx| view.status_messages_for_test(cx));
    assert_eq!(
        messages,
        vec![failed],
        "失败回执必须显示保存失败，不得伪造已保存"
    );
}

/// 文本控件跟随全局运行值；切页清除上一页未提交的非法文本。
#[gpui::test]
fn settings_text_inputs_follow_runtime_and_page_switch_clears_invalid_text(
    cx: &mut TestAppContext,
) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let view = root.read_with(cx, |root, _| {
        root.settings_overlay
            .as_ref()
            .expect("浮层必须存在")
            .view
            .clone()
    });
    let original = view
        .read_with(cx, |view, cx| view.input_value_for_test("background", cx))
        .expect("颜色字段必须有文本输入");

    // 其他提交入口写入运行值后，浮层文本控件必须回填同一值。
    root.update(cx, |root, cx| {
        root.apply_settings_field("font_family", Value::String("RuntimeFont".to_owned()), cx);
    });
    draw_frame(&mut window);
    assert_eq!(
        view.read_with(cx, |view, cx| view.input_value_for_test("font_family", cx)),
        Some("RuntimeFont".to_owned()),
        "程序化变更后文本控件必须回填运行值"
    );

    // 在颜色输入框键入非法文本：控件标记非法，运行值不变。
    let runtime_before = root.read_with(cx, |root, _| {
        SettingsRegistry::new()
            .recover(root.settings.values(), "background")
            .and_then(|recovery| recovery.value)
    });
    assert!(
        window
            .debug_bounds("settings-field-background-input")
            .is_some(),
        "颜色输入框必须渲染"
    );
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.focus_input_for_test("background", window, cx);
            assert!(
                view.input_is_focused_for_test("background", window, cx),
                "字段输入框必须获得键盘焦点"
            );
        });
    });
    draw_frame(&mut window);
    assert!(
        window.update(|window, cx| {
            view.read(cx)
                .input_is_focused_for_test("background", window, cx)
        }),
        "绘制后焦点不得被焦点管理器误恢复"
    );
    window.simulate_input("zzz");
    draw_frame(&mut window);
    let invalid_text = view
        .read_with(cx, |view, cx| view.input_value_for_test("background", cx))
        .expect("颜色字段必须有文本输入");
    assert_ne!(invalid_text, original, "键入必须改变输入框文本");
    assert!(
        view.read_with(cx, |view, _| view.field_invalid_for_test("background")),
        "非法文本必须标记字段无效"
    );
    let runtime = root.read_with(cx, |root, _| {
        SettingsRegistry::new()
            .recover(root.settings.values(), "background")
            .and_then(|recovery| recovery.value)
    });
    assert_eq!(runtime, runtime_before, "非法文本不得改写运行值");

    // 切走再切回：强制回填运行值并清除非法标记。
    focus_workspace(&root, &mut window);
    view.update(cx, |view, cx| {
        view.select_page_for_test(SettingsPageId::Terminal, cx)
    });
    draw_frame(&mut window);
    view.update(cx, |view, cx| {
        view.select_page_for_test(SettingsPageId::Appearance, cx)
    });
    draw_frame(&mut window);
    assert_eq!(
        view.read_with(cx, |view, cx| view.input_value_for_test("background", cx)),
        Some(original),
        "切页必须从运行值回填控件"
    );
    assert!(
        !view.read_with(cx, |view, _| view.field_invalid_for_test("background")),
        "切页必须清除上一页的非法标记"
    );
}

/// 路径类设置只接受绝对路径，并须真实存在且类型匹配后才写入运行值。
#[gpui::test]
fn settings_path_fields_require_existing_absolute_targets(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let exe = temp.path().join("fake-shell.exe");
    std::fs::write(&exe, b"MZ").unwrap();
    let startup = temp.path().join("startup");
    std::fs::create_dir(&startup).unwrap();
    let image = temp.path().join("wallpaper.png");
    std::fs::write(&image, b"png").unwrap();
    let plain_file = temp.path().join("plain.txt");
    std::fs::write(&plain_file, b"text").unwrap();
    let missing = temp.path().join("missing.exe");
    let text = |path: &std::path::Path| path.to_string_lossy().into_owned();

    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let view = root.read_with(cx, |root, _| {
        root.settings_overlay
            .as_ref()
            .expect("浮层必须存在")
            .view
            .clone()
    });
    let runtime_value = |key: &str| {
        root.read_with(cx, |root, _| {
            SettingsRegistry::new()
                .recover(root.settings.values(), key)
                .and_then(|recovery| recovery.value)
        })
    };
    let commit = |window: &mut VisualTestContext, key: &'static str, value: &str| {
        window.update(|_, cx| {
            view.update(cx, |view, cx| view.commit_text_for_test(key, value, cx));
        });
    };
    let invalid = || view.read_with(cx, |view, _| view.field_invalid_for_test("shell"));

    commit(&mut window, "shell", "pwsh.exe");
    assert!(invalid(), "相对路径必须立即标记非法");
    assert_eq!(runtime_value("shell"), None, "相对路径不得写入运行值");

    commit(&mut window, "shell", &text(&missing));
    draw_frame(&mut window);
    assert!(invalid(), "不存在的路径必须在真实检查后标记非法");
    assert_eq!(runtime_value("shell"), None, "不存在的路径不得写入运行值");

    commit(&mut window, "shell", &text(&exe));
    draw_frame(&mut window);
    assert!(!invalid(), "存在的可执行文件必须清除非法标记");
    assert_eq!(
        runtime_value("shell"),
        Some(Value::String(text(&exe))),
        "存在的可执行文件必须写入运行值"
    );

    let invalid_startup = || {
        view.read_with(cx, |view, _| {
            view.field_invalid_for_test("startup_directory")
        })
    };
    commit(&mut window, "startup_directory", &text(&plain_file));
    draw_frame(&mut window);
    assert!(invalid_startup(), "普通文件不得作为启动目录");
    assert_eq!(runtime_value("startup_directory"), None);
    commit(&mut window, "startup_directory", &text(&startup));
    draw_frame(&mut window);
    assert!(!invalid_startup(), "存在的目录必须清除非法标记");
    assert_eq!(
        runtime_value("startup_directory"),
        Some(Value::String(text(&startup))),
        "存在的目录必须写入运行值"
    );

    let invalid_image = || {
        view.read_with(cx, |view, _| {
            view.field_invalid_for_test("background_image")
        })
    };
    commit(&mut window, "background_image", &text(&startup));
    draw_frame(&mut window);
    assert!(invalid_image(), "目录不得作为背景图片");
    assert_eq!(runtime_value("background_image"), None);
    commit(&mut window, "background_image", &text(&image));
    draw_frame(&mut window);
    assert!(!invalid_image(), "存在的图片文件必须清除非法标记");
    assert_eq!(
        runtime_value("background_image"),
        Some(Value::String(text(&image))),
        "存在的图片文件必须写入运行值"
    );
}

/// 保存失败必须给出显式重试入口，重试把同一待写批次立即重新到期。
#[gpui::test]
fn settings_save_failure_offers_explicit_retry(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    root.update(cx, |root, cx| {
        root.apply_settings_field("font_family", Value::String("RetryFont".to_owned()), cx);
        let now = Instant::now();
        let due = now + std::time::Duration::from_secs(5);
        let entries = root.settings.begin_save(due).expect("到期批次必须启动");
        root.settings
            .finish_save(&entries, Err(std::io::Error::other("disk full")), due);
        root.notify_settings_views(cx);
    });
    draw_frame(&mut window);
    let retry = window
        .debug_bounds("settings-retry-save")
        .expect("保存失败必须提供重试入口");
    window.simulate_click(retry.center(), Modifiers::default());
    draw_frame(&mut window);
    let entries = root.update(cx, |root, _| {
        root.settings
            .begin_save(Instant::now())
            .expect("点击重试后同一批次必须立即到期")
    });
    root.update(cx, |root, _| {
        root.settings.finish_save(&entries, Ok(()), Instant::now());
    });
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-retry-save").is_none(),
        "成功回执后不得继续显示保存失败重试"
    );
}

/// 导航按全局／Agent 分组，过滤只影响导航且 Esc 先清空查询。
#[gpui::test]
fn settings_navigation_groups_and_filters_pages(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-nav-group-global").is_some(),
        "全局组标题必须渲染"
    );
    assert!(
        window.debug_bounds("settings-nav-group-agent").is_some(),
        "Agent 组标题必须渲染"
    );
    assert!(window.debug_bounds("settings-page-shortcuts").is_some());

    let settings = root.read_with(cx, |root, _| {
        root.settings_overlay.as_ref().unwrap().view.clone()
    });
    settings.update(cx, |view, cx| {
        view.set_nav_query_for_test("hooks", cx);
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-page-hooks").is_some());
    assert!(
        window.debug_bounds("settings-page-appearance").is_none(),
        "不匹配的全局页必须被过滤"
    );
    assert!(
        window.debug_bounds("settings-nav-group-global").is_none(),
        "无匹配页的组标题必须隐藏"
    );
    assert!(window.debug_bounds("settings-nav-group-agent").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "Esc 清空搜索时不得关闭浮层"
    );
    assert!(
        settings
            .read_with(cx, |view, _| view.nav_query_for_test())
            .is_empty()
    );
    assert!(window.debug_bounds("settings-page-appearance").is_some());
}

/// 后台任务页：任务行来自注册表，启停立即生效并写入外部段。
#[gpui::test]
fn settings_background_tasks_toggle_applies_and_persists(cx: &mut TestAppContext) {
    crate::background_tasks::reset_for_test();
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window
        .debug_bounds("settings-page-background-tasks")
        .unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-background-tasks").is_some());
    assert!(
        window
            .debug_bounds("settings-task-toggle-planBalance")
            .is_some(),
        "注册表任务必须逐行渲染"
    );

    let before = crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE);
    let toggle = window
        .debug_bounds("settings-task-toggle-planBalance")
        .unwrap();
    window.simulate_click(toggle.center(), Modifiers::default());
    draw_frame(&mut window);

    assert_eq!(
        crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE),
        !before,
        "启停必须立即热应用"
    );
    root.read_with(cx, |root, _| {
        let segment = root
            .settings
            .values()
            .external_value(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT)
            .expect("外部段必须写入");
        let plan = segment
            .get(crate::background_tasks::PLAN_BALANCE)
            .expect("目标任务必须存在");
        assert_eq!(
            plan.get("enabled").and_then(|value| value.as_bool()),
            Some(!before)
        );
    });
}
