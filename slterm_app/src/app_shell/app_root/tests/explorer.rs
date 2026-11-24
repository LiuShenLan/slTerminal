//! Explorer 真实文件树、Document 复用与视图状态测试。

use super::*;

fn open_explorer(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.simulate_keystrokes("ctrl-shift-e");
    window.run_until_parked();
    draw_frame(window);
    root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Explorer)
            .expect("Explorer 必须已打开")
            .clone()
    })
}

fn activate_explorer_row(
    explorer: &Entity<super::super::side_view::SideView>,
    path: &CanonicalPath,
    window: &mut VisualTestContext,
) {
    window.update(|window, cx| {
        explorer.update(cx, |view, cx| view.explorer_activate_at(path, window, cx));
    });
    window.run_until_parked();
    draw_frame(window);
}

#[gpui::test]
fn explorer_open_reuses_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let docs_root = project_root.join("docs");
    std::fs::create_dir_all(&docs_root).unwrap();
    std::fs::write(docs_root.join("note.txt"), "one\ntwo\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let file =
        CanonicalPath::new(dunce::canonicalize(docs_root.join("note.txt")).unwrap()).unwrap();
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
                "draft\n",
                window,
                cx,
            );
        });
    });

    add_project(&root, &mut window, PROJECT_B, docs_root.to_str().unwrap());
    activate_project(&root, &mut window, PROJECT_B);
    let explorer = open_explorer(&root, &mut window, cx);
    activate_explorer_row(&explorer, &file, &mut window);

    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        1,
        "Explorer 打开必须复用另一项目已打开的同一 Document"
    );
    let editor_key = ViewKey::Editor {
        project_id: project_id(PROJECT_B),
        document_id: document,
    };
    assert!(root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_B))
            .unwrap()
            .layout()
            .view_keys()
            .contains(&editor_key)
    }));
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "draft\n",
        "复用必须保留原脏正文"
    );

    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    activate_explorer_row(&explorer, &file, &mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before,
        "同一项目重复打开只还焦既有视图"
    );
}
