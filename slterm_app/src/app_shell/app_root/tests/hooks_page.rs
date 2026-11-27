//! Hooks 编辑页的草稿、监听与面板释放测试。

use std::time::Instant;

use super::settings_overlay::{focus_workspace, open_settings, settings_panel};
use super::*;
use crate::ai_hook::draft_host::HooksDraftOwner;

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
        assert!(
            root.fs_watch
                .as_ref()
                .is_some_and(|watch| watch.tracks_file(&project_settings)),
            "已载入的 Hooks 层文件必须进入 watcher"
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

    // 外部修改守卫：watcher 先给出持续状态，脏草稿的重载不得丢正文。
    root.update(cx, |root, cx| {
        root.update_hooks_draft_text(key, r#"{"Stop":[]}"#, cx)
    });
    std::fs::write(&project_settings, br#"{"keep":false,"hooks":{}}"#).unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.fs_watch
                .as_ref()
                .expect("fs watcher")
                .inject_file_event_for_test(project_settings.clone());
            root.poll_fs_watch(window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(
            root.hooks_external_modified_for_test(key),
            "watcher 必须标记外部修改"
        );
        assert_eq!(
            root.hook_drafts.get(key).map(|draft| draft.text()),
            Some(r#"{"Stop":[]}"#),
            "外部修改不得静默改写草稿"
        );
    });
    let reload = window.debug_bounds("settings-hooks-reload").unwrap();
    window.simulate_click(reload.center(), Modifiers::default());
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.hook_drafts.get(key).map(|draft| draft.text()),
            Some(r#"{"Stop":[]}"#),
            "脏草稿的重新加载必须零副作用"
        );
    });

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

    // 干净草稿遇外部修改：重新加载入口非破坏性地采纳磁盘新内容。
    std::fs::write(
        &project_settings,
        br#"{"keep":true,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo reload"}]}]}}"#,
    )
    .unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.fs_watch
                .as_ref()
                .expect("fs watcher")
                .inject_file_event_for_test(project_settings.clone());
            root.poll_fs_watch(window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(root.hooks_external_modified_for_test(key));
    });
    let reload = window.debug_bounds("settings-hooks-reload").unwrap();
    window.simulate_click(reload.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("重新加载后必须重新载入");
        assert!(
            draft.text().contains("echo reload"),
            "重新加载必须采纳磁盘内容: {}",
            draft.text()
        );
        assert!(!draft.dirty());
        assert!(
            !root.hooks_external_modified_for_test(key),
            "成功重载后外部修改状态必须清除"
        );
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
