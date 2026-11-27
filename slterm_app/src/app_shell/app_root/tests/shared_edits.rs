//! 共享编辑的可见性门控：隐藏 Markdown／Diff 不重算，重新可见只同步一次。

use super::documents::open_document;
use super::*;

fn markdown_view(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: ProjectId,
) -> Entity<crate::app_shell::markdown::MarkdownView> {
    root.read_with(cx, |root, cx| {
        root.projects[&project]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.markdown().cloned())
            .expect("项目必须存在 Markdown 面板")
    })
}

/// 隐藏项目的 Markdown 不消费共享编辑；激活时用最新正文刷新一次。
#[gpui::test]
fn hidden_markdown_defers_shared_edits_until_visible(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let root_dir = temp.path().join("project");
    std::fs::create_dir(&root_dir).unwrap();
    let markdown_file = root_dir.join("note.md");
    std::fs::write(
        &markdown_file,
        "# before
",
    )
    .unwrap();
    let other_file = root_dir.join("notes.txt");
    std::fs::write(
        &other_file,
        "other
",
    )
    .unwrap();
    let markdown_file = CanonicalPath::new(dunce::canonicalize(&markdown_file).unwrap()).unwrap();
    let other_file = CanonicalPath::new(dunce::canonicalize(&other_file).unwrap()).unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, root_dir.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, markdown_file);
    draw_frame(&mut window);
    let view = markdown_view(&root, cx, project_id(PROJECT_A));
    let markdown_key = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.markdown().map(|_| panel.view_key.clone()))
            .expect("项目必须存在 Markdown 面板")
    });
    // 同组打开另一个文件后 Markdown 变为非激活视图，显示派生重算必须暂停。
    open_document(&root, &mut window, PROJECT_A, GROUP_A, other_file);
    draw_frame(&mut window);
    let revision_before = view.read_with(cx, |view, _| view.revision_for_test());
    let source = view.read_with(cx, |view, _| view.source_for_test());
    window.update(|window, cx| {
        source.update(cx, |source, cx| {
            let length = source.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                source,
                Some(0..length),
                "# after
",
                window,
                cx,
            );
        });
    });
    draw_frame(&mut window);
    assert_eq!(
        view.read_with(cx, |view, _| view.revision_for_test()),
        revision_before,
        "非激活 Markdown 不得消费共享编辑"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.text_for_test()),
        "# before
",
        "非激活 Markdown 保持最后可见正文"
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(markdown_key.clone(), window, cx)
                .expect("激活 Markdown 面板必须成功")
        });
    });
    draw_frame(&mut window);
    assert_ne!(
        view.read_with(cx, |view, _| view.revision_for_test()),
        revision_before,
        "重新可见必须采纳最新正文"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.text_for_test()),
        "# after
",
        "重新可见必须显示最新正文"
    );
}
