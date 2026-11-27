//! Markdown 链接拒绝与打开失败必须显示封闭原因，而不是只写日志。

use super::*;
use crate::i18n::Message;

/// 危险协议被拒绝时进入应用内通知，且不携带 URL 或路径。
#[gpui::test]
fn rejected_markdown_link_shows_a_closed_reason(cx: &mut TestAppContext) {
    crate::notify::reset_in_app_notices_for_test();
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("note.md");
    std::fs::write(&source, "[bad](javascript:alert(1))").unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    super::documents::open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let document_id = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        project
            .panels
            .values()
            .find_map(|panel| match panel.view_key {
                ViewKey::Markdown { document_id, .. } => Some(document_id),
                _ => None,
            })
            .expect("Markdown 文档必须打开为渲染面板")
    });
    let link = slterm_document::MarkdownDocument::parse("[bad](javascript:alert(1))")
        .links()
        .first()
        .expect("危险协议必须被投影为链接")
        .clone();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_markdown_link(document_id, link, window, cx);
        });
    });

    let language = cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language);
    let expected = language
        .text(Message::NotifyMarkdownLinkUnsupported)
        .to_owned();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        window.run_until_parked();
        root.update(cx, |root, cx| root.poll_in_app_notices(cx));
        found = root.read_with(cx, |root, _| {
            root.in_app_notices
                .iter()
                .any(|notice| notice.text == expected)
        });
        if found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(found, "链接拒绝必须在应用内显示原因");
    let texts = root.read_with(cx, |root, _| {
        root.in_app_notices
            .iter()
            .map(|notice| notice.text.clone())
            .collect::<Vec<_>>()
    });
    assert!(
        texts.iter().all(|text| !text.contains("javascript")
            && !text.contains(&source.to_string_lossy().to_string())),
        "通知不得携带 URL 或路径: {texts:?}"
    );
}
