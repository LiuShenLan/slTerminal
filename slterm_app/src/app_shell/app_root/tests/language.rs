//! 运行期语言切换：只替换文案目录并重绘，不重建工作区。

use super::*;
use crate::i18n::UiLanguage;
use crate::notify::Notification;

/// 锚：切换语言不重建项目、布局或终端，通知文案即时使用新语言。
#[gpui::test]
fn i18n_switch_preserves_workspace(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("note.txt");
    std::fs::write(&source, "old").unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let _panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let (editor, document) = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        project
            .panels
            .values()
            .find_map(|panel| {
                let editor = panel.editor()?.clone();
                match panel.view_key {
                    ViewKey::Editor { document_id, .. } => Some((editor, document_id)),
                    _ => None,
                }
            })
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });

    let before = root.read_with(cx, |root, cx| {
        (
            root.projects[&project_id(PROJECT_A)].entity_id(),
            root.snapshot_generation,
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .layout()
                .view_keys(),
            root.snapshot().terminals().count(),
            editor.entity_id(),
            document,
            editor.read(cx).input_text_for_test(),
            root.documents
                .session(document)
                .map(|session| session.text().unwrap().to_string()),
            root.panel_count(project_id(PROJECT_A), cx),
        )
    });
    let start_language = cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language);
    let (target_setting, target_language) = match start_language {
        UiLanguage::EnUs => ("zh-CN", UiLanguage::ZhCn),
        _ => ("en-US", UiLanguage::EnUs),
    };
    // 模拟启动链：投递语言与全局快照同源。
    crate::notify::set_language(start_language);
    let notification = Notification::AiTurn {
        program: "claude".to_owned(),
        message: None,
        attention: true,
    };
    let before_text = notification.toast_text();

    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "language",
            slterm_settings::json::Value::String(target_setting.to_owned()),
            cx,
        );
    });
    window.run_until_parked();

    assert_eq!(
        cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language),
        target_language,
        "全局语言快照必须即时更新"
    );
    let after = root.read_with(cx, |root, cx| {
        (
            root.projects[&project_id(PROJECT_A)].entity_id(),
            root.snapshot_generation,
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .layout()
                .view_keys(),
            root.snapshot().terminals().count(),
            editor.entity_id(),
            document,
            editor.read(cx).input_text_for_test(),
            root.documents
                .session(document)
                .map(|session| session.text().unwrap().to_string()),
            root.panel_count(project_id(PROJECT_A), cx),
        )
    });
    assert_eq!(
        before, after,
        "切换语言不得重建项目、布局、终端或未保存 Document"
    );
    assert_eq!(
        before.6.as_str(),
        "local old",
        "语言切换前必须存在未保存草稿"
    );

    let after_text = notification.toast_text();
    assert_ne!(before_text.1, after_text.1, "通知文案必须即时切换语言");
    assert!(
        root.read_with(cx, |root, _| root.settings.pending_len() >= 1),
        "语言修改必须进入唯一写队列"
    );
    crate::notify::set_language(start_language);
}

/// 集成锚：运行期双向切换语言，文案即时往返且两语言目录都完整。
#[gpui::test]
fn integration_language(cx: &mut TestAppContext) {
    use std::time::Duration;

    let (root, _window, _state) = open_root(cx);
    let start = cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language);
    crate::notify::set_language(start);
    let notification = Notification::CommandDone {
        duration: Duration::from_secs(12),
        program: Some("cargo".to_owned()),
    };
    let before = notification.toast_text();

    let (other_setting, other_language) = match start {
        UiLanguage::EnUs => ("zh-CN", UiLanguage::ZhCn),
        _ => ("en-US", UiLanguage::EnUs),
    };
    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "language",
            slterm_settings::json::Value::String(other_setting.to_owned()),
            cx,
        );
    });
    assert_eq!(
        cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language),
        other_language
    );
    let switched = notification.toast_text();
    assert_ne!(before.1, switched.1);

    let back_setting = match start {
        UiLanguage::EnUs => "en-US",
        _ => "zh-CN",
    };
    root.update(cx, |root, cx| {
        root.apply_settings_field(
            "language",
            slterm_settings::json::Value::String(back_setting.to_owned()),
            cx,
        );
    });
    assert_eq!(notification.toast_text(), before, "切回原语言文案必须恢复");

    // 构建期目录强等保证无缺译；运行期两种语言都有非空文案。
    assert!(
        !UiLanguage::EnUs
            .text(crate::i18n::Message::NotifyBell)
            .is_empty()
    );
    assert!(
        !UiLanguage::ZhCn
            .text(crate::i18n::Message::NotifyBell)
            .is_empty()
    );
    crate::notify::set_language(start);
}
