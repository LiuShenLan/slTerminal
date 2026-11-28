//! Hooks 编辑页的草稿、监听、面板释放与 JSON／GUI 表单测试。

use std::time::Instant;

use super::settings_overlay::{focus_workspace, open_settings, settings_panel};
use super::*;
use crate::ai_hook::draft::DraftParseError;
use crate::ai_hook::draft_host::{HooksDraftKey, HooksDraftLayer, HooksDraftOwner};

fn click_selector(window: &mut VisualTestContext, selector: &'static str) {
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少 debug selector: {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
}

fn open_hooks_page(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    focus_workspace(root, window);
    open_settings(window);
    draw_frame(window);
    click_selector(window, "settings-page-hooks");
    window.run_until_parked();
    draw_frame(window);
}

fn user_hooks_key(root: &Entity<AppRoot>, cx: &TestAppContext) -> HooksDraftKey {
    let panel = settings_panel(root, cx, project_id(PROJECT_A)).expect("设置面板必须存在");
    HooksDraftKey {
        owner: HooksDraftOwner::Panel(panel),
        agent: slterm_agents::AgentKind::Claude,
        layer: HooksDraftLayer::User,
    }
}

fn settings_view(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
) -> Entity<super::super::settings_overlay::SettingsView> {
    root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find(|panel| matches!(panel.view_key, ViewKey::Settings { .. }))
            .expect("设置面板必须存在");
        let super::super::panel_host::PanelContent::Settings(view) = &panel.content else {
            panic!("设置面板内容形态错误");
        };
        view.clone()
    })
}

fn focus_form_field(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    selector: &str,
) {
    let view = settings_view(root, cx);
    let selector = selector.to_owned();
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            assert!(
                view.focus_form_input_for_test(&selector, window, cx),
                "表单输入 {selector} 必须存在"
            );
        });
    });
}

fn focus_json_editor(root: &Entity<AppRoot>, window: &mut VisualTestContext, cx: &TestAppContext) {
    let view = settings_view(root, cx);
    window.update(|window, cx| {
        view.update(cx, |view, cx| view.focus_hooks_input_for_test(window, cx));
    });
}

fn write_claude_settings(directory: &std::path::Path, contents: &str) {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(directory.join("settings.json"), contents).unwrap();
}

fn switch_to_form(window: &mut VisualTestContext) {
    click_selector(window, "settings-hooks-mode-form");
}

/// JSON／GUI 共用同一草稿；事件树只列未配置事件，matcher 只随事件能力展示。
#[gpui::test]
fn hooks_page_json_and_form_share_draft_and_event_tree(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(
        &claude_dir,
        r#"{"hooks":{
            "PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo pre"}]}],
            "Stop":[{"hooks":[{"type":"command","command":"echo stop"}]}]
        }}"#,
    );
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);

    let key = user_hooks_key(&root, cx);
    let before = root.read_with(cx, |root, _| {
        root.hook_drafts
            .get(key)
            .expect("User 层草稿必须载入")
            .text()
            .to_owned()
    });
    assert!(before.contains("echo pre"));
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "默认必须是 JSON 模式"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "默认不得渲染 GUI 表单"
    );

    switch_to_form(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-form").is_some(),
        "切到 GUI 后必须渲染事件树"
    );
    assert!(
        window.debug_bounds("settings-hooks-editor").is_none(),
        "GUI 模式不得同时渲染 JSON 编辑器"
    );
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.hook_drafts.get(key).map(|draft| draft.text()),
            Some(before.as_str()),
            "JSON→GUI 不得改写草稿文本"
        );
    });

    for selector in [
        "settings-hooks-event-PreToolUse",
        "settings-hooks-event-Stop",
        "settings-hooks-matcher-PreToolUse-0",
        "settings-hooks-all-matches-Stop-0",
        "settings-hooks-add-event-PostToolUse",
        "settings-hooks-add-event-SessionEnd",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "GUI 事件树必须渲染 {selector}"
        );
    }
    assert!(
        window
            .debug_bounds("settings-hooks-matcher-Stop-0")
            .is_none(),
        "不支持 matcher 的事件不得渲染 matcher 输入"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-add-event-PreToolUse")
            .is_none(),
        "已配置事件不得出现在添加列表"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-add-event-Stop")
            .is_none(),
        "已配置事件不得重复添加"
    );

    click_selector(
        &mut window,
        Box::leak(
            "settings-hooks-add-event-PostToolUse"
                .to_owned()
                .into_boxed_str(),
        ),
    );
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-hooks-event-PostToolUse")
            .is_some(),
        "点击添加后必须出现新事件"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-add-event-PostToolUse")
            .is_none(),
        "已添加事件必须从添加列表消失"
    );
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("添加事件后草稿必须存在");
        assert!(draft.text().contains("PostToolUse"));
        assert!(draft.diagnostic().is_none(), "表单写入必须保持 JSON 合法");
    });

    click_selector(&mut window, "settings-hooks-mode-json");
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "切回 JSON 必须恢复编辑器"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "切回 JSON 不得残留表单"
    );
    root.read_with(cx, |root, _| {
        assert!(
            root.hook_drafts
                .get(key)
                .is_some_and(|draft| draft.text().contains("PostToolUse")),
            "GUI 编辑结果必须留在同一草稿"
        );
    });
}

/// 五种 handler 类型按共享字段目录渲染各自字段，不展示其他类型专属字段。
#[gpui::test]
fn hooks_page_form_renders_type_specific_fields(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(
        &claude_dir,
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[
            {"type":"command","command":"echo","args":["x"],"async":true},
            {"type":"http","url":"https://example.test","headers":{"A":"B"},"allowedEnvVars":["TOKEN"]},
            {"type":"mcp_tool","server":"fs","tool":"read","input":{"path":"x"}},
            {"type":"prompt","prompt":"hi","model":"haiku","continueOnBlock":true},
            {"type":"agent","prompt":"go","model":"sonnet"}
        ]}]}}"#,
    );
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    switch_to_form(&mut window);

    for selector in [
        "settings-hooks-field-PreToolUse-0-0-command",
        "settings-hooks-field-PreToolUse-0-0-args",
        "settings-hooks-field-PreToolUse-0-0-async",
        "settings-hooks-field-PreToolUse-0-1-url",
        "settings-hooks-field-PreToolUse-0-1-headers",
        "settings-hooks-field-PreToolUse-0-1-allowedEnvVars",
        "settings-hooks-field-PreToolUse-0-2-server",
        "settings-hooks-field-PreToolUse-0-2-tool",
        "settings-hooks-field-PreToolUse-0-2-input",
        "settings-hooks-field-PreToolUse-0-3-prompt",
        "settings-hooks-field-PreToolUse-0-3-model",
        "settings-hooks-field-PreToolUse-0-3-continueOnBlock",
        "settings-hooks-field-PreToolUse-0-4-prompt",
        "settings-hooks-field-PreToolUse-0-4-model",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "handler 表单必须渲染共享目录字段 {selector}"
        );
    }
    for selector in [
        "settings-hooks-field-PreToolUse-0-0-url",
        "settings-hooks-field-PreToolUse-0-1-command",
        "settings-hooks-field-PreToolUse-0-2-prompt",
    ] {
        assert!(
            window.debug_bounds(selector).is_none(),
            "不得渲染其他 handler 类型的专属字段 {selector}"
        );
    }
}

/// 表单保存只经同一草稿与唯一写通道；JSON 文本与文件 hooks 子树一致。
#[gpui::test]
fn hooks_page_form_save_updates_json_and_file(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(
        &claude_dir,
        r#"{"keep":true,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo old"}]}]}}"#,
    );
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir.clone())
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    switch_to_form(&mut window);

    click_selector(&mut window, "settings-hooks-field-Stop-0-0-command-input");
    focus_form_field(
        &root,
        &mut window,
        cx,
        "settings-hooks-field-Stop-0-0-command",
    );
    window.simulate_keystrokes("ctrl-a");
    window.simulate_input("echo form");
    draw_frame(&mut window);
    let key = user_hooks_key(&root, cx);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("编辑后草稿必须存在");
        assert!(
            draft.text().contains("echo form"),
            "表单编辑必须写回同一草稿: {}",
            draft.text()
        );
        assert!(draft.diagnostic().is_none());
    });
    assert!(
        window
            .debug_bounds("settings-hooks-field-Stop-0-0-command-error")
            .is_none(),
        "合法命令不得显示行内错误"
    );

    click_selector(&mut window, "settings-hooks-save");
    window.run_until_parked();
    draw_frame(&mut window);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(claude_dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        saved["keep"],
        serde_json::json!(true),
        "hooks 外字段必须保留"
    );
    assert_eq!(
        saved["hooks"]["Stop"][0]["hooks"][0]["command"],
        serde_json::json!("echo form")
    );
    assert!(
        window.debug_bounds("settings-hooks-saved-hint").is_some(),
        "保存成功后必须提示重启 CLI"
    );

    click_selector(&mut window, "settings-hooks-mode-json");
    window.run_until_parked();
    draw_frame(&mut window);
    let settings = root.read_with(cx, |root, cx| {
        let panel = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find(|panel| matches!(panel.view_key, ViewKey::Settings { .. }))
            .expect("设置面板必须存在");
        let super::super::panel_host::PanelContent::Settings(view) = &panel.content else {
            panic!("设置面板内容形态错误");
        };
        view.clone()
    });
    let json_text = settings.read_with(cx, |view, cx| {
        view.hooks_input()
            .expect("JSON 输入实体必须存在")
            .read(cx)
            .value()
            .to_string()
    });
    let json_model: serde_json::Value = serde_json::from_str(&json_text).unwrap();
    assert_eq!(
        json_model, saved["hooks"],
        "JSON 模式必须与文件 hooks 子树一致"
    );
    assert!(json_text.contains("echo form"));
}

/// 表单非法字段只留行内诊断，不写回草稿；修正后恢复可保存。
#[gpui::test]
fn hooks_page_form_reports_invalid_field_inline(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(
        &claude_dir,
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo old"}]}]}}"#,
    );
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    switch_to_form(&mut window);
    let key = user_hooks_key(&root, cx);
    let before = root.read_with(cx, |root, _| {
        root.hook_drafts.get(key).unwrap().text().to_owned()
    });

    click_selector(&mut window, "settings-hooks-field-Stop-0-0-args-input");
    focus_form_field(&root, &mut window, cx, "settings-hooks-field-Stop-0-0-args");
    window.simulate_keystrokes("ctrl-a");
    window.simulate_input("not-json");
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-hooks-field-Stop-0-0-args-error")
            .is_some(),
        "非法 JSON 数组必须显示行内诊断"
    );
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).unwrap();
        assert_eq!(draft.text(), before, "非法字段不得写回草稿");
        assert!(!draft.saveable(), "非法字段不得启用保存");
    });

    click_selector(&mut window, "settings-hooks-field-Stop-0-0-args-input");
    focus_form_field(&root, &mut window, cx, "settings-hooks-field-Stop-0-0-args");
    window.simulate_keystrokes("ctrl-a");
    window.simulate_input("[\"x\"]");
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-hooks-field-Stop-0-0-args-error")
            .is_none(),
        "修正后行内诊断必须消失"
    );
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).unwrap();
        assert_eq!(
            draft.model()["Stop"][0]["hooks"][0]["args"],
            serde_json::json!(["x"]),
            "合法修正必须写回草稿: {}",
            draft.text()
        );
        assert!(draft.saveable(), "合法修正后必须恢复可保存");
        assert!(!draft.text().contains("not-json"), "非法文本不得写回草稿");
    });
}

/// schema 多错误必须全部可见；非法文档禁用 GUI 与保存且零写入。
#[gpui::test]
fn hooks_page_shows_complete_diagnostics_and_disables_form(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    let contents = r#"{"hooks":{
        "NotAnEvent":[{"hooks":[{"type":"command","command":"x"}]}],
        "Stop":[{"hooks":[
            {"type":"webhook","url":"https://x"},
            {"type":"command","command":"   "}
        ]}]
    }}"#;
    write_claude_settings(&claude_dir, contents);
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir.clone())
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);

    let key = user_hooks_key(&root, cx);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("非法 schema 草稿仍须载入");
        assert_eq!(
            draft.schema_errors().len(),
            3,
            "未知事件、非法 type 与空必填字段必须各自成项"
        );
        assert!(!draft.saveable(), "schema 非法必须禁用保存");
    });
    assert!(
        window.debug_bounds("settings-hooks-diagnostic").is_some(),
        "必须显示首条诊断"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-diagnostics-toggle")
            .is_some(),
        "多条诊断必须提供完整列表入口"
    );
    click_selector(&mut window, "settings-hooks-diagnostics-toggle");
    assert!(
        window
            .debug_bounds("settings-hooks-diagnostics-list")
            .is_some(),
        "展开后必须显示完整诊断列表"
    );
    for selector in [
        "settings-hooks-diagnostic-0",
        "settings-hooks-diagnostic-1",
        "settings-hooks-diagnostic-2",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "完整列表必须包含 {selector}"
        );
    }
    assert!(
        window.debug_bounds("settings-hooks-diagnostic-3").is_none(),
        "诊断列表不得超出真实错误数"
    );

    click_selector(&mut window, "settings-hooks-mode-form");
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "schema 非法时 GUI 必须禁用并停留 JSON"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "schema 非法时不得渲染表单"
    );

    click_selector(&mut window, "settings-hooks-save");
    window.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(claude_dir.join("settings.json")).unwrap(),
        contents,
        "schema 非法必须零写入"
    );
}

/// 重复键仍走严格解析：JSON 模式行内诊断，GUI 模式保持禁用。
#[gpui::test]
fn hooks_page_duplicate_json_diagnostic_keeps_gui_disabled(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(&claude_dir, r#"{"hooks":{}}"#);
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);

    click_selector(&mut window, "settings-hooks-editor");
    focus_json_editor(&root, &mut window, cx);
    window.simulate_keystrokes("ctrl-a");
    window.simulate_input(r#"{"Stop":[{"hooks":[{"type":"command","command":"a"}]}],"Stop":[]}"#);
    draw_frame(&mut window);
    let key = user_hooks_key(&root, cx);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("输入后草稿必须存在");
        assert!(
            matches!(draft.diagnostic(), Some(DraftParseError::DuplicateKey(key)) if key == "Stop"),
            "重复键必须给出严格解析诊断: {:?}",
            draft.diagnostic()
        );
    });
    assert!(
        window.debug_bounds("settings-hooks-diagnostic").is_some(),
        "重复键必须在页面上可见"
    );
    click_selector(&mut window, "settings-hooks-mode-form");
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "重复键未修复时 GUI 必须禁用"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "重复键未修复时不得渲染表单"
    );
}

/// 缺失根不被 GUI 模式破坏：不得进入表单、不得凭空创建草稿或目录。
#[gpui::test]
fn hooks_page_form_preserves_missing_root(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing-claude");
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(missing.clone())
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    assert!(
        window.debug_bounds("settings-hooks-error").is_some(),
        "缺失配置目录必须给出可见错误"
    );
    switch_to_form(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "无文档时不得进入表单"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "无文档时 GUI 必须禁用"
    );
    root.read_with(cx, |root, _| {
        assert!(
            root.hook_drafts.keys().is_empty(),
            "无文档时不得凭空创建草稿"
        );
    });
    assert!(!missing.exists(), "缺失配置目录不得被创建");
}

/// 新增 handler 先收集完整必填字段；缺失时只留诊断，不写入非法模型。
#[gpui::test]
fn hooks_page_form_adds_complete_handler_and_rejects_missing_required(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(&claude_dir, r#"{"hooks":{}}"#);
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    switch_to_form(&mut window);

    click_selector(&mut window, "settings-hooks-add-event-Stop");
    click_selector(&mut window, "settings-hooks-add-group-Stop");
    click_selector(&mut window, "settings-hooks-add-handler-Stop-0-command");
    assert!(
        window
            .debug_bounds("settings-hooks-pending-Stop-0")
            .is_some(),
        "添加 handler 必须先渲染待提交表单"
    );
    click_selector(&mut window, "settings-hooks-pending-add");
    assert!(
        window
            .debug_bounds("settings-hooks-pending-error")
            .is_some(),
        "必填字段缺失必须显示行内诊断"
    );
    let key = user_hooks_key(&root, cx);
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).unwrap();
        assert_eq!(
            draft.model()["Stop"][0]["hooks"].as_array().map(Vec::len),
            Some(0),
            "缺失必填字段不得写入非法 handler"
        );
        assert!(draft.saveable(), "缺失字段时尚未产生可保存改动");
    });

    click_selector(&mut window, "settings-hooks-pending-field-command-input");
    focus_form_field(
        &root,
        &mut window,
        cx,
        "settings-hooks-pending-field-command",
    );
    window.simulate_keystrokes("ctrl-a");
    window.simulate_input("echo pending");
    draw_frame(&mut window);
    click_selector(&mut window, "settings-hooks-pending-add");
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).unwrap();
        assert_eq!(
            draft.model()["Stop"][0]["hooks"][0]["command"],
            serde_json::json!("echo pending"),
            "完整 handler 必须写入同一草稿"
        );
        assert!(draft.diagnostic().is_none());
        assert!(draft.saveable());
    });
    assert!(
        window
            .debug_bounds("settings-hooks-handler-Stop-0-0")
            .is_some(),
        "提交后事件树必须显示新 handler"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-pending-Stop-0")
            .is_none(),
        "提交后待新增表单必须关闭"
    );
}

/// 托管只读摘要与可编辑事件树并存；GUI 不提供删除托管内容的入口。
#[gpui::test]
fn hooks_page_form_keeps_managed_groups_read_only(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let claude_dir = temp.path().join("claude");
    write_claude_settings(
        &claude_dir,
        r#"{"hooks":{
            "Stop":[{"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]}],
            "SessionStart":[{"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]}]
        }}"#,
    );
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_hooks_claude_dir_for_test(claude_dir)
    });
    add_project(&root, &mut window, PROJECT_A, project_dir.to_str().unwrap());
    open_hooks_page(&root, &mut window);
    assert!(
        window.debug_bounds("settings-hooks-owned").is_some(),
        "托管只读摘要必须保留"
    );
    switch_to_form(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-form").is_some(),
        "仅有托管组时表单仍须渲染"
    );
    assert!(
        window.debug_bounds("settings-hooks-event-Stop").is_none(),
        "托管组不得进入可编辑事件树"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-add-event-Stop")
            .is_some(),
        "托管组不占用可编辑事件列表"
    );
    assert!(
        window
            .debug_bounds("settings-hooks-remove-event-Stop")
            .is_none(),
        "GUI 不得提供删除托管事件的入口"
    );
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
