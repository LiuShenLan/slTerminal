use std::sync::Arc;

use crate::app_shell::text_view::DocumentTextView;
use crate::platform::windows_integration::FakeWindowsIntegration;

fn editor_for(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: &str,
    document: slterm_document::DocumentId,
) -> Entity<DocumentTextView> {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(project)]
            .read(cx)
            .panels
            .values()
            .find(|panel| {
                matches!(
                    panel.view_key,
                    ViewKey::Editor { document_id, .. } if document_id == document
                )
            })
            .and_then(|panel| panel.editor().cloned())
            .unwrap()
    })
}

fn type_local_draft(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    project: &str,
    document: slterm_document::DocumentId,
) {
    let editor = editor_for(root, cx, project, document);
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..0),
                "local ",
                window,
                cx,
            );
        });
    });
}

/// 显式保存遇到磁盘变化必须进入唯一冲突决定面；覆盖后正文落盘并清脏。
#[gpui::test]
fn explicit_save_conflict_opens_decision_and_overwrite_writes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("draft.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    type_local_draft(&root, &mut window, cx, PROJECT_A, document);
    std::fs::write(file.as_path(), "outside").unwrap();

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.save_document_explicit(document, window, cx);
        });
    });
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(
        root.read_with(cx, |root, _| root.save_conflict.is_some()),
        "磁盘冲突必须打开保存决定面"
    );
    assert!(window.debug_bounds("save-conflict-dialog").is_some());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.overwrite_conflicted_document(window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(std::fs::read_to_string(file.as_path()).unwrap(), "local old");
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert!(!session.is_dirty(), "覆盖保存必须清脏");
        assert!(session.external_conflict().is_none());
        assert!(root.save_conflict.is_none());
    });
}

/// 取消只收合决定面，磁盘与本地草稿都不变。
#[gpui::test]
fn save_conflict_cancel_keeps_draft_and_disk(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("draft.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    type_local_draft(&root, &mut window, cx, PROJECT_A, document);
    std::fs::write(file.as_path(), "outside").unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.save_document_explicit(document, window, cx);
        });
    });
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.save_conflict.is_some()));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.cancel_save_conflict(window, cx).unwrap();
        });
    });
    window.run_until_parked();

    assert!(root.read_with(cx, |root, _| root.save_conflict.is_none()));
    assert_eq!(std::fs::read_to_string(file.as_path()).unwrap(), "outside");
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert!(session.is_dirty());
        assert_eq!(session.text().unwrap().to_string(), "local old");
    });
}

/// 「重载磁盘」丢弃本地草稿并广播真实磁盘正文。
#[gpui::test]
fn save_conflict_reload_discards_local_draft(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("draft.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    type_local_draft(&root, &mut window, cx, PROJECT_A, document);
    std::fs::write(file.as_path(), "outside").unwrap();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.save_document_explicit(document, window, cx);
        });
    });
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.save_conflict.is_some()));

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.reload_conflicted_document(window, cx);
        });
    });
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.text().unwrap().to_string(), "outside");
        assert!(!session.is_dirty());
        assert!(session.external_conflict().is_none());
        assert!(root.save_conflict.is_none());
    });
    assert_eq!(editor_text(&root, cx, PROJECT_A, document), "outside");
}

/// 另存为把正文写到用户选定目标，并在同一 Document 上重绑位置。
#[gpui::test]
fn save_as_writes_selected_target_and_rebinds(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let source = project.join("source.txt");
    let target = project.join("copy.txt");
    std::fs::write(&source, "source").unwrap();
    let source = CanonicalPath::new(dunce::canonicalize(&source).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(target.clone()));
    root.update(cx, |root, _| root.replace_windows_for_test(windows));
    open_document(&root, &mut window, PROJECT_A, GROUP_A, source.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    type_local_draft(&root, &mut window, cx, PROJECT_A, document);
    draw_frame(&mut window);
    let editor = editor_for(&root, cx, PROJECT_A, document);
    let selector: &'static str = Box::leak(
        format!("document-save-as-{}", editor.entity_id()).into_boxed_str(),
    );
    assert!(
        window.debug_bounds(selector).is_some(),
        "Editor 底栏必须提供另存为按钮"
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_save_document_as(document, window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "local source");
    assert_eq!(std::fs::read_to_string(source.as_path()).unwrap(), "source");
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.location().canonical_path().as_path(), target);
        assert!(!session.is_dirty());
    });
}

/// 目标已关联其他打开 Document 时另存为必须在写前拒绝。
#[gpui::test]
fn save_as_rejects_target_owned_by_open_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let source = project.join("source.txt");
    let target = project.join("opened.txt");
    std::fs::write(&source, "source").unwrap();
    std::fs::write(&target, "target").unwrap();
    let source = CanonicalPath::new(dunce::canonicalize(&source).unwrap()).unwrap();
    let target = CanonicalPath::new(dunce::canonicalize(&target).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(target.as_path().to_path_buf()));
    root.update(cx, |root, _| root.replace_windows_for_test(windows));
    open_document(&root, &mut window, PROJECT_A, GROUP_A, source.clone());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, target.clone());
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find(|(_, location)| location.canonical_path() == &source)
            .map(|(id, _)| id)
            .unwrap()
    });
    type_local_draft(&root, &mut window, cx, PROJECT_A, document);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_save_document_as(document, window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(std::fs::read_to_string(target.as_path()).unwrap(), "target");
    assert_eq!(std::fs::read_to_string(source.as_path()).unwrap(), "source");
    root.read_with(cx, |root, _| {
        let session = root.documents.session(document).unwrap();
        assert_eq!(session.location().canonical_path(), &source);
        assert!(session.is_dirty(), "拒绝后必须保留本地草稿与脏态");
    });
}
