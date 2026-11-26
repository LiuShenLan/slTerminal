/// 编辑产生崩溃快照，保存清脏后同一快照被删除；写线程由 AppRoot 外持有。
#[gpui::test]
fn document_edits_write_and_clear_crash_snapshots(cx: &mut TestAppContext) {
    use crate::persistence::document_snapshot::writer::DocumentSnapshotWriter;

    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("snapshot.txt");
    std::fs::write(&file, "old").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut writer = DocumentSnapshotWriter::start(&paths).unwrap();
    let client = writer.client();

    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.install_snapshot_client(Some(client.clone()), "run-snapshot-test".to_owned());
    });
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let (document, editor) = root.read_with(cx, |root, cx| {
        let document = root
            .snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == &file).then_some(id))
            .unwrap();
        let editor = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap();
        (document, editor)
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, Some(0..0), "draft ", window, cx);
        });
    });
    window.run_until_parked();
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    let snapshot_path = paths
        .local_root()
        .join("snapshots")
        .join(format!("{document}.snap"));
    assert!(snapshot_path.exists(), "脏编辑必须生成崩溃快照");

    // 最后引用关闭越过 StateCommitted 后，携带关闭意图的删除必须生效。
    let persisted = root.read_with(cx, |root, _| {
        let revision = root.documents.session(document).unwrap().text_revision();
        let objects = vec![slterm_workspace::CloseObject {
            id: slterm_workspace::CloseTarget::Document(document),
            generation: revision,
            decision: slterm_workspace::CloseDecision::Discard,
        }];
        crate::persistence::close::PersistedCloseIntent {
            close_id: "snapshot-close".to_owned(),
            scope: slterm_workspace::CloseScope::ApplicationExit,
            store_epoch: "epoch".to_owned(),
            intent: slterm_workspace::CloseIntent::new(objects, [0; 32]).unwrap(),
            commit_status: crate::persistence::close::CloseCommitStatus::StateCommitted,
        }
    });
    root.read_with(cx, |root, _| {
        root.delete_closed_document_snapshots(&persisted)
    });
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    assert!(!snapshot_path.exists(), "已提交关闭的最后引用必须删除快照");

    // 再次编辑恢复快照，保存清脏后同样删除。
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, Some(0..1), "D", window, cx);
        });
    });
    window.run_until_parked();
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    assert!(snapshot_path.exists(), "再次编辑必须重新生成快照");
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    window.run_until_parked();
    let report = client
        .flush()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(report.failures().is_empty(), "{report:?}");
    assert!(!snapshot_path.exists(), "保存清脏后冗余快照必须删除");
    assert_eq!(std::fs::read_to_string(file.as_path()).unwrap(), "Draft old");

    drop(root);
    drop(window);
    cx.run_until_parked();
    writer.shutdown().unwrap();
}
