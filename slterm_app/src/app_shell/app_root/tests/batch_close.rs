//! 组内批量关闭的原子提交、空组塌缩与失败重试测试。

use super::super::task_service::TaskId;
use super::close::flush_when_ready;
use super::*;
use crate::persistence::close::CloseCommitStatus;
use slterm_workspace::{CloseDecision, ClosePhase, CloseStage, CloseTarget};

fn execute_batch(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: CommandId,
    group: PanelGroupId,
) -> Result<CommandOutcome, AppRootError> {
    execute_batch_with_decisions(root, window, id, group, BTreeMap::new())
}

fn execute_batch_with_decisions(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: CommandId,
    group: PanelGroupId,
    document_decisions: BTreeMap<slterm_document::DocumentId, CloseDecision>,
) -> Result<CommandOutcome, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                id,
                CommandSource::Menu,
                CommandTarget::Focused,
                Some(CommandParameters::CloseGroup {
                    group_id: group,
                    document_decisions,
                }),
                window,
                cx,
            )
        })
    })
}

fn close_active(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
) -> Result<TerminalRecord, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
            .map(|outcome| match outcome {
                CommandOutcome::TerminalClosed(record) => record,
                _ => panic!("关闭命令必须返回终端记录"),
            })
        })
    })
}

fn group_views(root: &Entity<AppRoot>, cx: &TestAppContext, group: PanelGroupId) -> Vec<ViewKey> {
    root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .and_then(|project| project.layout().group(group))
            .map(|group| group.views().to_vec())
            .unwrap_or_default()
    })
}

fn open_editor(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    path: CanonicalPath,
) -> (
    slterm_document::DocumentId,
    Entity<crate::app_shell::text_view::DocumentTextView>,
) {
    super::documents::open_document(root, window, PROJECT_A, GROUP_A, path.clone());
    root.read_with(cx, |root, cx| {
        let document = root
            .snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == &path).then_some(id))
            .unwrap();
        let editor = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find(|panel| {
                panel.view_key
                    == ViewKey::Editor {
                        project_id: project_id(PROJECT_A),
                        document_id: document,
                    }
            })
            .and_then(|panel| panel.editor().cloned())
            .unwrap();
        (document, editor)
    })
}

#[gpui::test]
fn integration_batch_close(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let kept = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let neighbor = window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.split_terminal_with_identity(
                    SplitTerminalSpec {
                        new_terminal: terminal_spec(PROJECT_A, GROUP_A, TERMINAL_C),
                        new_group_id: group_id(GROUP_B),
                        direction: SplitDirection::LeftRight,
                        ratio: 0.5,
                        viewport: LogicalSize::new(1200.0, 800.0),
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.activate_terminal(terminal_id(TERMINAL_B), window, cx)
            })
        })
        .unwrap();
    let first_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let kept_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_B);
    let neighbor_entity = panel_entity(&root, cx, PROJECT_A, TERMINAL_C);
    let kept_focus = window.update(|window, cx| window.focused(cx));

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseOtherViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    window.run_until_parked();

    assert_eq!(
        group_views(&root, cx, group_id(GROUP_A)),
        vec![ViewKey::Terminal {
            project_id: project_id(PROJECT_A),
            terminal_id: terminal_id(TERMINAL_B),
        }]
    );
    assert_eq!(
        group_views(&root, cx, group_id(GROUP_B)),
        vec![ViewKey::Terminal {
            project_id: project_id(PROJECT_A),
            terminal_id: terminal_id(TERMINAL_C),
        }]
    );
    assert!(first_entity.2.upgrade().is_none());
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_B).1,
        kept_entity.1
    );
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_C).1,
        neighbor_entity.1
    );
    assert_eq!(first_entity.0, first);
    assert_eq!(kept_entity.0, kept);
    assert_eq!(neighbor_entity.0, neighbor);
    assert!(matches!(
        state.borrow().sessions[&first.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert!(matches!(
        state.borrow().sessions[&neighbor.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(window.update(|window, cx| window.focused(cx)), kept_focus);

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    window.run_until_parked();

    assert!(group_views(&root, cx, group_id(GROUP_A)).is_empty());
    assert_eq!(group_views(&root, cx, group_id(GROUP_B)).len(), 1);
    assert_eq!(
        root.read_with(cx, |root, _| root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group_ids()),
        vec![group_id(GROUP_B)]
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        1
    );
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(matches!(
        state.borrow().sessions[&neighbor.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(root.read_with(cx, |root, _| root.terminal_closes.len()), 0);
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
    assert_eq!(state.borrow().attempts, 3);
}

#[gpui::test]
fn save_close_rechecks_revision_after_background_writes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("late.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path.clone());
    let (document, editor) = root.read_with(cx, |root, cx| {
        let document = root.snapshot().documents().next().unwrap().0;
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
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
    });
    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Save)]),
    )
    .unwrap();
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "late ", window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(file_path.as_path()).unwrap(),
        "savedold"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .documents
            .session(document)
            .unwrap()
            .text()
            .unwrap()
            .to_string()),
        "savedlate old"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .terminal_close_batches
            .values()
            .next()
            .unwrap()
            .coordinator
            .phase()),
        ClosePhase::Failed(CloseStage::Save)
    );

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Save)]),
    )
    .unwrap();
    window.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(file_path.as_path()).unwrap(),
        "savedlate old"
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
}

#[gpui::test]
fn save_close_rechecks_a_clean_sibling_edited_during_background_writes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let saved_path = temp.path().join("saved.txt");
    let sibling_path = temp.path().join("sibling.txt");
    std::fs::write(&saved_path, "old").unwrap();
    std::fs::write(&sibling_path, "clean").unwrap();
    let saved_path = CanonicalPath::new(dunce::canonicalize(&saved_path).unwrap()).unwrap();
    let sibling_path = CanonicalPath::new(dunce::canonicalize(&sibling_path).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (saved, saved_editor) = open_editor(&root, &mut window, cx, saved_path.clone());
    let (sibling, sibling_editor) = open_editor(&root, &mut window, cx, sibling_path.clone());
    window.update(|window, cx| {
        saved_editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
    });

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(saved, CloseDecision::Save)]),
    )
    .unwrap();
    window.update(|window, cx| {
        sibling_editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "late ", window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(saved_path.as_path()).unwrap(),
        "savedold"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .documents
            .session(sibling)
            .unwrap()
            .text()
            .unwrap()
            .to_string()),
        "late clean"
    );
    assert!(root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .is_some_and(|close| {
                close.needs_recollect
                    && close.coordinator.phase() == ClosePhase::Failed(CloseStage::Save)
            })
    }));
}

#[gpui::test]
fn discard_close_waits_for_inflight_save_before_detaching_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("discard.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path.clone());
    let (document, editor) = root.read_with(cx, |root, cx| {
        let document = root.snapshot().documents().next().unwrap().0;
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
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
        root.update(cx, |root, cx| root.save_document(document, cx));
    });

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Discard)]),
    )
    .unwrap();
    assert!(root.read_with(cx, |root, _| root.documents.session(document).is_some()));
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(file_path.as_path()).unwrap(),
        "savedold"
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
}

#[gpui::test]
fn discard_close_rechecks_late_edits_and_recollects_on_retry(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("discard-late.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path.clone());
    let (document, editor) = root.read_with(cx, |root, cx| {
        let document = root.snapshot().documents().next().unwrap().0;
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
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
        root.update(cx, |root, cx| root.save_document(document, cx));
    });
    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Discard)]),
    )
    .unwrap();
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "late ", window, cx);
        });
    });
    window.run_until_parked();

    assert_eq!(
        root.read_with(cx, |root, _| root
            .documents
            .session(document)
            .unwrap()
            .text()
            .unwrap()
            .to_string()),
        "savedlate old"
    );
    assert!(root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .is_some_and(|close| {
                close.coordinator.phase() == ClosePhase::Failed(CloseStage::CommitState)
            })
    }));
    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Discard)]),
    )
    .unwrap();
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
}

#[gpui::test]
fn recollect_requires_a_decision_for_newly_dirty_document_and_persists_new_intent(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let discarded_path = temp.path().join("discarded.txt");
    let clean_path = temp.path().join("newly-dirty.txt");
    std::fs::write(&discarded_path, "old").unwrap();
    std::fs::write(&clean_path, "clean").unwrap();
    let discarded_path = CanonicalPath::new(dunce::canonicalize(&discarded_path).unwrap()).unwrap();
    let clean_path = CanonicalPath::new(dunce::canonicalize(&clean_path).unwrap()).unwrap();
    let (discarded, discarded_editor) = open_editor(&root, &mut window, cx, discarded_path.clone());
    let (clean, clean_editor) = open_editor(&root, &mut window, cx, clean_path.clone());
    flush_when_ready(&runtime.client());
    window.update(|window, cx| {
        discarded_editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
        root.update(cx, |root, cx| root.save_document(discarded, cx));
    });

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(discarded, CloseDecision::Discard)]),
    )
    .unwrap();
    window.update(|window, cx| {
        clean_editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "late ", window, cx);
        });
    });
    window.run_until_parked();
    let previous_fingerprint = root.read_with(cx, |root, _| {
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::CommitState)
        );
        *close.persisted.as_ref().unwrap().intent.fingerprint()
    });

    // 原干净 Document 已变脏，重试必须重新收集并要求新的内容决定。
    assert!(matches!(
        execute_batch(&root, &mut window, CommandId::CloseAllViews, group_id(GROUP_A)),
        Err(AppRootError::DocumentClose(
            slterm_workspace::DocumentCloseError::MissingDecision(id)
        )) if id == clean
    ));
    assert!(root.read_with(cx, |root, _| root.documents.session(clean).is_some()));

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(clean, CloseDecision::Save)]),
    )
    .unwrap();
    root.read_with(cx, |root, _| {
        let close = root.terminal_close_batches.values().next().unwrap();
        let persisted = close.persisted.as_ref().unwrap();
        assert_eq!(close.coordinator.intent(), Some(&persisted.intent));
        assert_ne!(persisted.intent.fingerprint(), &previous_fingerprint);
        let objects = close
            .coordinator
            .intent()
            .unwrap()
            .objects()
            .filter_map(|object| match object.id {
                CloseTarget::Document(id) => Some((id, object.decision)),
                CloseTarget::Terminal(_) => None,
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(objects[&discarded], CloseDecision::NotRequired);
        assert_eq!(objects[&clean], CloseDecision::Save);
    });
    window.run_until_parked();
    flush_when_ready(&runtime.client());

    assert_eq!(
        std::fs::read_to_string(discarded_path.as_path()).unwrap(),
        "savedold"
    );
    assert_eq!(
        std::fs::read_to_string(clean_path.as_path()).unwrap(),
        "late clean"
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
    let database = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    assert_eq!(
        database
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    drop(database);
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn pending_discard_resume_is_idempotent(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("resume.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path.clone());
    let (document, editor) = root.read_with(cx, |root, cx| {
        let document = root.snapshot().documents().next().unwrap().0;
        let editor = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap();
        (document, editor)
    });
    let request = window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "saved", window, cx);
        });
        root.update(cx, |root, _| {
            root.documents.begin_save(document, 0).unwrap()
        })
    });
    let receipt = crate::app_shell::documents::DocumentSaveService::default()
        .save(
            request,
            &[slterm_path::PathGrant::project_root(
                CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap(),
            )],
            0,
        )
        .unwrap();
    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Discard)]),
    )
    .unwrap();
    root.update(cx, |root, cx| {
        root.documents.complete_save(receipt).unwrap();
        root.resume_blocked_batch_saves(cx);
        root.resume_blocked_batch_saves(cx);
    });
    assert!(root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .is_some_and(|close| !close.outcome_unknown)
    }));
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
}

#[gpui::test]
fn document_close_waits_for_and_follows_an_inflight_save(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("shared.txt");
    std::fs::write(&file, "old").unwrap();
    let file_path = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let terminal = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file_path.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    root.update(cx, |root, cx| {
        let session = root.documents.session(document).unwrap();
        let end = session.text().unwrap().len();
        root.documents
            .apply(
                slterm_document::EditTransaction {
                    document_id: document,
                    expected_text_revision: session.text_revision(),
                    edits: vec![slterm_document::TextEdit::new(0..end, "saved")],
                    origin: slterm_document::EditOrigin::ReplaceAll,
                    merge_policy: slterm_document::MergePolicy::Disallowed,
                },
                slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
                std::time::Instant::now(),
            )
            .unwrap();
        root.save_document(document, cx);
    });

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        BTreeMap::from([(document, CloseDecision::Save)]),
    )
    .unwrap();
    assert!(root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .unwrap()
            .coordinator
            .phase()
            == ClosePhase::Running(CloseStage::Save)
    }));
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(file_path.as_path()).unwrap(),
        "saved"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.terminal_close_batches.len()),
        0
    );
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert!(matches!(
        state.borrow().sessions[&terminal.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
}

#[gpui::test]
fn document_batch_close_partial_save(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let first_path = temp.path().join("first.txt");
    let second_path = temp.path().join("second.txt");
    std::fs::write(&first_path, "first").unwrap();
    std::fs::write(&second_path, "second").unwrap();
    let first_path = CanonicalPath::new(dunce::canonicalize(&first_path).unwrap()).unwrap();
    let second_path = CanonicalPath::new(dunce::canonicalize(&second_path).unwrap()).unwrap();
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    let terminal = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, first_path.clone());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, second_path.clone());
    let ids = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .map(|(id, location)| {
                if location.canonical_path() == &first_path {
                    ("first", id)
                } else if location.canonical_path() == &second_path {
                    ("second", id)
                } else {
                    panic!("未知 Document 位置")
                }
            })
            .collect::<BTreeMap<_, _>>()
    });
    root.update(cx, |root, _| {
        for (name, replacement) in [("first", "first saved"), ("second", "second local")] {
            let id = ids[name];
            let session = root.documents.session(id).unwrap();
            let revision = session.text_revision();
            let end = session.text().unwrap().len();
            root.documents
                .apply(
                    slterm_document::EditTransaction {
                        document_id: id,
                        expected_text_revision: revision,
                        edits: vec![slterm_document::TextEdit::new(0..end, replacement)],
                        origin: slterm_document::EditOrigin::ReplaceAll,
                        merge_policy: slterm_document::MergePolicy::Disallowed,
                    },
                    slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
                    std::time::Instant::now(),
                )
                .unwrap();
        }
    });
    std::fs::write(second_path.as_path(), "second external").unwrap();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let decisions = ids.values().map(|id| (*id, CloseDecision::Save)).collect();

    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
        decisions,
    )
    .unwrap();
    assert!(
        root.read_with(cx, |root, _| root.documents.session(ids["first"]).is_some()),
        "后台保存期间 Registry 必须仍可供前台对账"
    );
    window.run_until_parked();

    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        3
    );
    assert_eq!(
        std::fs::read_to_string(first_path.as_path()).unwrap(),
        "first saved"
    );
    assert_eq!(
        std::fs::read_to_string(second_path.as_path()).unwrap(),
        "second external"
    );
    root.read_with(cx, |root, cx| {
        assert!(!root.documents.session(ids["first"]).unwrap().is_dirty());
        let second = root.documents.session(ids["second"]).unwrap();
        assert!(second.is_dirty());
        assert_eq!(second.text().unwrap().to_string(), "second local");
        assert!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .read(cx)
                .session
                .is_some()
        );
        assert_eq!(root.terminal_close_batches.len(), 1);
        assert_eq!(
            root.terminal_close_batches
                .values()
                .next()
                .unwrap()
                .coordinator
                .phase(),
            ClosePhase::Failed(CloseStage::Save)
        );
    });
    assert!(matches!(
        state.borrow().sessions[&terminal.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));

    // 冲突解除后重试同一 Save 意图，全部保存成功才停止终端并销毁面板。
    std::fs::write(second_path.as_path(), "second").unwrap();
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert!(root.read_with(cx, |root, _| root.terminal_close_batches.is_empty()));
    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(
        std::fs::read_to_string(second_path.as_path()).unwrap(),
        "second local"
    );
    assert!(matches!(
        state.borrow().sessions[&terminal.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
}

#[gpui::test]
fn integration_last_reference_close(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_a = temp.path().join("a");
    std::fs::create_dir(&project_a).unwrap();
    let file = project_a.join("shared.txt");
    std::fs::write(&file, "disk").unwrap();
    let file = CanonicalPath::new(dunce::canonicalize(file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project_a.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    add_project(&root, &mut window, PROJECT_B, temp.path().to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_B, GROUP_B, file.clone());
    let (id, shared_panel) = root.read_with(cx, |root, cx| {
        let (id, _) = root.snapshot().documents().next().unwrap();
        let view = ViewKey::Editor {
            project_id: project_id(PROJECT_B),
            document_id: id,
        };
        (
            id,
            root.panel_for_view(project_id(PROJECT_B), &view, cx)
                .unwrap(),
        )
    });
    root.update(cx, |root, _| {
        let session = root.documents.session(id).unwrap();
        let revision = session.text_revision();
        let end = session.text().unwrap().len();
        root.documents
            .apply(
                slterm_document::EditTransaction {
                    document_id: id,
                    expected_text_revision: revision,
                    edits: vec![slterm_document::TextEdit::new(0..end, "draft")],
                    origin: slterm_document::EditOrigin::ReplaceAll,
                    merge_policy: slterm_document::MergePolicy::Disallowed,
                },
                slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
                std::time::Instant::now(),
            )
            .unwrap();
    });
    activate_project(&root, &mut window, PROJECT_A);

    // 集合外仍有引用，不需要也不允许内容决定；共享正文与既有面板不变。
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();

    assert!(root.read_with(cx, |root, _| root.documents.session(id).unwrap().is_dirty()));
    assert_eq!(
        root.read_with(cx, |root, cx| {
            let view = ViewKey::Editor {
                project_id: project_id(PROJECT_B),
                document_id: id,
            };
            root.panel_for_view(project_id(PROJECT_B), &view, cx)
                .unwrap()
        }),
        shared_panel
    );
    activate_project(&root, &mut window, PROJECT_B);
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_B),
        ),
        Err(AppRootError::DocumentClose(
            slterm_workspace::DocumentCloseError::MissingDecision(missing)
        )) if missing == id
    ));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        root.read_with(cx, |root, cx| {
            let view = ViewKey::Editor {
                project_id: project_id(PROJECT_B),
                document_id: id,
            };
            root.panel_for_view(project_id(PROJECT_B), &view, cx)
                .unwrap()
        }),
        shared_panel
    );
    std::fs::write(file.as_path(), "external").unwrap();
    execute_batch_with_decisions(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_B),
        BTreeMap::from([(id, CloseDecision::Save)]),
    )
    .unwrap();
    window.run_until_parked();
    assert_eq!(std::fs::read_to_string(file.as_path()).unwrap(), "external");
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.terminal_close_batches
                .values()
                .next()
                .unwrap()
                .coordinator
                .phase()
        }),
        ClosePhase::Failed(CloseStage::Save)
    );

    // 纯 Document 批次同样必须能重试，不依赖任何终端成员。
    std::fs::write(file.as_path(), "disk").unwrap();
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_B),
    )
    .unwrap();
    window.run_until_parked();

    assert!(root.read_with(cx, |root, _| root.documents.is_empty()));
    assert_eq!(std::fs::read_to_string(file.as_path()).unwrap(), "draft");
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_B), cx)),
        0
    );
}

#[gpui::test]
fn incomplete_batch_stop_result_is_outcome_unknown_without_partial_facts(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseOtherViews,
        group_id(GROUP_A),
    )
    .unwrap();
    let (batch_id, attempt, terminal_id) = root.read_with(cx, |root, _| {
        let (id, close) = root.terminal_close_batches.iter().next().unwrap();
        (
            *id,
            close.coordinator.current_attempt().unwrap(),
            *close.members.keys().next().unwrap(),
        )
    });

    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.finish_batch_stop_for_test(batch_id, attempt, vec![(terminal_id, Ok(()))], cx);
        });
    });

    root.read_with(cx, |root, _| {
        let close = &root.terminal_close_batches[&batch_id];
        assert!(close.outcome_unknown);
        assert!(close.stop_observations.is_empty());
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
    });
    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        ),
        Err(AppRootError::TerminalCloseOutcomeUnknown)
    ));
}

#[gpui::test]
fn unknown_batch_stop_member_is_outcome_unknown_even_with_full_valid_prefix(
    cx: &mut TestAppContext,
) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseOtherViews,
        group_id(GROUP_A),
    )
    .unwrap();
    let (batch_id, attempt, result) = root.read_with(cx, |root, _| {
        let (id, close) = root.terminal_close_batches.iter().next().unwrap();
        let mut result = close
            .members
            .keys()
            .map(|terminal_id| (*terminal_id, Ok(())))
            .collect::<Vec<_>>();
        result.push((terminal_id(TERMINAL_C), Ok(())));
        (*id, close.coordinator.current_attempt().unwrap(), result)
    });

    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.finish_batch_stop_for_test(batch_id, attempt, result, cx);
        });
    });

    root.read_with(cx, |root, _| {
        let close = &root.terminal_close_batches[&batch_id];
        assert!(close.outcome_unknown);
        assert!(close.stop_observations.is_empty());
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
    });
}

#[gpui::test]
fn batch_join_panic_requires_explicit_retry_without_rejoin(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    execute_batch(
        &root,
        &mut window,
        CommandId::CloseOtherViews,
        group_id(GROUP_A),
    )
    .unwrap();
    let (batch_id, attempt, result) = root.read_with(cx, |root, _| {
        let (id, close) = root.terminal_close_batches.iter().next().unwrap();
        let mut result = close
            .members
            .keys()
            .map(|terminal_id| (*terminal_id, Ok(())))
            .collect::<Vec<_>>();
        result[1].1 = Err(Box::new("injected join panic") as Box<dyn std::any::Any + Send>);
        (*id, close.coordinator.current_attempt().unwrap(), result)
    });
    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.finish_batch_stop_for_test(batch_id, attempt, result, cx);
        });
    });
    root.read_with(cx, |root, _| {
        let close = &root.terminal_close_batches[&batch_id];
        assert!(!close.outcome_unknown);
        assert_eq!(close.stop_observations.len(), close.members.len());
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert!(
            !root
                .tasks
                .is_registered(TaskId::TerminalCloseBatch(batch_id))
        );
    });

    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
    )
    .unwrap();

    assert!(root.read_with(cx, |root, _| root.terminal_close_batches.is_empty()));
    assert_eq!(group_views(&root, cx, group_id(GROUP_A)).len(), 1);
}

#[gpui::test]
fn batch_commit_failure_preserves_every_panel_and_pty(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_close BEFORE INSERT ON close_intents \
             BEGIN SELECT RAISE(ABORT, 'injected batch close failure'); END;",
        )
        .unwrap();
    let before = root.read_with(cx, |root, _| root.snapshot().clone());

    assert!(matches!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.terminal_close_batches.len(), 1);
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Running(CloseStage::CommitState)
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.snapshot(), &before);
        for id in [TERMINAL_A, TERMINAL_B] {
            assert!(
                root.terminal_panel(project_id(PROJECT_A), terminal_id(id), cx)
                    .unwrap()
                    .1
                    .read(cx)
                    .session
                    .is_some()
            );
        }
    });
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ));
    }
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
        for id in [TERMINAL_A, TERMINAL_B] {
            assert!(
                root.terminal_panel(project_id(PROJECT_A), terminal_id(id), cx)
                    .unwrap()
                    .1
                    .read(cx)
                    .session
                    .is_some()
            );
        }
    });
    fault
        .execute_batch("DROP TRIGGER reject_batch_close")
        .unwrap();
    assert_eq!(
        WorkspaceDatabase::open(&paths)
            .unwrap()
            .load_project_snapshot()
            .unwrap(),
        before
    );
    assert_eq!(
        fault
            .query_row("SELECT COUNT(*) FROM close_intents", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        execute_batch(
            &root,
            &mut window,
            CommandId::CloseAllViews,
            group_id(GROUP_A),
        )
        .is_ok()
    );
    window.run_until_parked();
    flush_when_ready(&runtime.client());
    assert_eq!(
        root.read_with(cx, |root, cx| root.panel_count(project_id(PROJECT_A), cx)),
        0
    );
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn batch_stop_failure_retries_original_batch_without_rejoin_or_new_target(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    let kept = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_C);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_pty_stopped BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'pty_stopped' \
             BEGIN SELECT RAISE(ABORT, 'injected batch pty failure'); END;",
        )
        .unwrap();

    execute_batch(
        &root,
        &mut window,
        CommandId::CloseOtherViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 3);
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_C)).is_some());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_A)).is_none());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_B)).is_none());
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::StopPty)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
        assert!(close.unresolved_failure);
    });
    let close_id = root.read_with(cx, |root, _| {
        root.terminal_close_batches
            .values()
            .next()
            .unwrap()
            .persisted
            .as_ref()
            .unwrap()
            .close_id
            .clone()
    });
    let object_keys = fault
        .prepare("SELECT object_key FROM close_objects WHERE close_id = ?1 ORDER BY object_key")
        .unwrap()
        .query_map([close_id], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        object_keys,
        vec![
            format!("terminal:{TERMINAL_A}"),
            format!("terminal:{TERMINAL_B}")
        ]
    );
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    fault
        .execute_batch("DROP TRIGGER reject_batch_pty_stopped")
        .unwrap();

    let retried = close_active(&root, &mut window).unwrap();
    assert_eq!(retried.id(), terminal_id(TERMINAL_A));
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
        assert!(root.snapshot().terminal(terminal_id(TERMINAL_C)).is_some());
    });
    for panel in [first, second] {
        assert!(
            !matches!(
                state.borrow().sessions[&panel.sequence()].try_recv(),
                Ok(Msg::Shutdown)
            ),
            "显式重试不得再次 join 已停止的成员"
        );
    }
    assert!(matches!(
        state.borrow().sessions[&kept.sequence()].try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}

#[gpui::test]
fn batch_cleanup_failure_retries_without_rejoining_members(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/batch");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let second = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    flush_when_ready(&runtime.client());
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER reject_batch_cleanup BEFORE UPDATE OF commit_status ON close_intents \
             WHEN NEW.commit_status = 'cleanup_committed' \
             BEGIN SELECT RAISE(ABORT, 'injected batch cleanup failure'); END;",
        )
        .unwrap();

    execute_batch(
        &root,
        &mut window,
        CommandId::CloseAllViews,
        group_id(GROUP_A),
    )
    .unwrap();
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        let close = root.terminal_close_batches.values().next().unwrap();
        assert_eq!(
            close.coordinator.phase(),
            ClosePhase::Failed(CloseStage::Cleanup)
        );
        assert_eq!(
            close.persisted.as_ref().unwrap().commit_status,
            CloseCommitStatus::PtyStopped
        );
    });
    for panel in [first, second] {
        assert!(matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    fault
        .execute_batch("DROP TRIGGER reject_batch_cleanup")
        .unwrap();

    assert_eq!(
        close_active(&root, &mut window).unwrap().id(),
        terminal_id(TERMINAL_A)
    );
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
        assert!(root.terminal_close_batches.is_empty());
        assert!(root.terminal_close_batch_members.is_empty());
    });
    for panel in [first, second] {
        assert!(!matches!(
            state.borrow().sessions[&panel.sequence()].try_recv(),
            Ok(Msg::Shutdown)
        ));
    }
    assert_eq!(
        fault
            .query_row("SELECT commit_status FROM close_intents", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "completed"
    );
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
