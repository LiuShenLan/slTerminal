//! Commit 视图：状态分组、共享 Document Diff 与固定版本打开。

use super::*;

use super::super::panel_host::PanelContent;

fn open_commit(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.simulate_keystrokes("ctrl-shift-g");
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Commit)
            .expect("Commit 必须已打开")
            .clone()
    })
}

fn activate_commit_row(
    commit: &Entity<super::super::side_view::SideView>,
    path: &CanonicalPath,
    window: &mut VisualTestContext,
) {
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_activate_row(path, window, cx));
    });
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
}

#[gpui::test]
fn commit_opens_shared_document_diff(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("first.txt"), "changed\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "dirty draft\n",
                window,
                cx,
            );
        });
    });

    let commit = open_commit(&root, &mut window, cx);
    activate_commit_row(&commit, &file, &mut window);

    let diff = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .expect("modified 行必须打开 Diff")
    });
    diff.read_with(cx, |view, app| {
        assert_eq!(
            view.shared_identity_for_test(),
            document,
            "必须复用共享 Document"
        );
        assert_eq!(view.shared_text_for_test(app), "dirty draft\n");
        assert_eq!(view.head_bytes_for_test().as_ref(), b"fixed first\n");
    });
    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        1,
        "不得为 Diff 另建第二份 Document"
    );
}
