#[gpui::test]
fn markdown_local_link_opens_through_shared_document_pipeline(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, view, _source, _target) = open_watched_markdown(cx);
    let target = temp.path().join("outside.md");
    std::fs::write(&target, "# Target\n").unwrap();
    let document = slterm_document::MarkdownDocument::parse("[target](../outside.md)");
    let link = document.links()[0].clone();

    window.update(|_, cx| {
        view.update(cx, |_, cx| {
            cx.emit(crate::app_shell::markdown::MarkdownViewEvent::OpenLink(link));
        });
    });
    window.run_until_parked();

    let texts = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .filter_map(|panel| match &panel.content {
                super::super::panel_host::PanelContent::Markdown(markdown) => {
                    Some(markdown.view.read(cx).input_text_for_test(cx))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(texts.len(), 2, "本地链接必须复用统一面板打开链");
    assert!(
        texts.iter().any(|text| text == "# Target\n"),
        "本地链接必须安装真实读取的 Markdown 正文"
    );
}

#[gpui::test]
fn stale_markdown_anchor_receipt_does_not_move_new_revision(cx: &mut TestAppContext) {
    let (_temp, _root, mut window, _document, view, _source, _target) =
        open_watched_markdown(cx);
    view.update(cx, |view, _| view.set_reading_for_test(0, 12));
    let document = slterm_document::MarkdownDocument::parse("[top](#)");

    window.update(|_, cx| {
        view.update(cx, |_, cx| {
            cx.emit(crate::app_shell::markdown::MarkdownViewEvent::OpenLink(
                document.links()[0].clone(),
            ));
        });
    });
    edit_markdown_draft(&view, &mut window);
    window.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.rendered_revision_for_test(), 1);
        assert_eq!(view.reading_top_for_test(), (0, 12));
    });
}

struct DocumentReadGate {
    blocked: CanonicalPath,
}

impl DocumentReadGate {
    fn install(
        root: &Entity<AppRoot>,
        cx: &mut TestAppContext,
        blocked: &std::path::Path,
    ) -> Self {
        let blocked = CanonicalPath::new(dunce::canonicalize(blocked).unwrap()).unwrap();
        root.update(cx, |root, _| {
            root.test_document_read_hold = Some(blocked.clone());
        });
        Self { blocked }
    }

    fn wait_entered(&self, root: &Entity<AppRoot>, cx: &TestAppContext) {
        let held = root.read_with(cx, |root, _| {
            root.held_document_read
                .as_ref()
                .map(|(_, resolved)| resolved.location().canonical_path().clone())
        });
        assert_eq!(held, Some(self.blocked.clone()), "目标读取必须到达可控闸门");
    }

    fn release(self, root: &Entity<AppRoot>, window: &mut VisualTestContext) {
        window.update(|window, cx| {
            root.update(cx, |root, cx| root.release_held_document_read(window, cx));
        });
    }
}

fn emit_markdown_link(
    view: &Entity<crate::app_shell::markdown::MarkdownView>,
    window: &mut VisualTestContext,
    url: &str,
) {
    let document = slterm_document::MarkdownDocument::parse(&format!("[target]({url})"));
    window.update(|_, cx| {
        view.update(cx, |_, cx| {
            cx.emit(crate::app_shell::markdown::MarkdownViewEvent::OpenLink(
                document.links()[0].clone(),
            ));
        });
    });
}

fn open_paths(root: &Entity<AppRoot>, cx: &TestAppContext) -> Vec<std::path::PathBuf> {
    root.read_with(cx, |root, _| {
        root.snapshot
            .documents()
            .map(|(_, location)| location.canonical_path().as_path().to_path_buf())
            .collect()
    })
}

#[gpui::test]
fn resolved_markdown_link_waits_behind_inflight_document_open(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, view, _source, _target) = open_watched_markdown(cx);
    let ordinary = temp.path().join("project/ordinary.txt");
    let linked = temp.path().join("project/linked.md");
    std::fs::write(&ordinary, "ordinary").unwrap();
    std::fs::write(&linked, "# Linked\n").unwrap();
    let gate = DocumentReadGate::install(&root, cx, &ordinary);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_document(
                project_id(PROJECT_A),
                group_id(GROUP_A),
                CanonicalPath::new(&ordinary).unwrap(),
                window,
                cx,
            )
            .unwrap();
        });
    });
    window.run_until_parked();
    gate.wait_entered(&root, cx);
    emit_markdown_link(&view, &mut window, "linked.md");
    window.run_until_parked();
    let before_release = open_paths(&root, cx);
    let document_open_owned = root.read_with(cx, |root, _| {
        root.tasks
            .is_registered(super::super::task_service::TaskId::DocumentOpen)
    });
    gate.release(&root, &mut window);
    window.run_until_parked();
    let after_release = open_paths(&root, cx);

    assert!(
        document_open_owned,
        "链接不得完成或取消另一个 DocumentOpen 任务"
    );
    assert!(!before_release.contains(&ordinary));
    assert!(
        !before_release.contains(&linked),
        "已解析链接不得越过仍在读取的普通打开"
    );
    assert!(after_release.contains(&ordinary), "链接不得取消普通打开");
    assert!(after_release.contains(&linked));
}

#[gpui::test]
fn two_resolved_markdown_links_share_one_fifo(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, view, _source, _target) = open_watched_markdown(cx);
    let first = temp.path().join("project/first.md");
    let second = temp.path().join("project/second.md");
    std::fs::write(&first, "# First\n").unwrap();
    std::fs::write(&second, "# Second\n").unwrap();
    let gate = DocumentReadGate::install(&root, cx, &first);

    emit_markdown_link(&view, &mut window, "first.md");
    window.run_until_parked();
    gate.wait_entered(&root, cx);
    emit_markdown_link(&view, &mut window, "second.md");
    window.run_until_parked();
    let before_release = open_paths(&root, cx);
    let document_open_owned = root.read_with(cx, |root, _| {
        root.tasks
            .is_registered(super::super::task_service::TaskId::DocumentOpen)
    });
    gate.release(&root, &mut window);
    window.run_until_parked();
    let after_release = open_paths(&root, cx);

    assert!(
        document_open_owned,
        "第二个链接不得完成第一个 DocumentOpen 任务"
    );
    assert!(
        !before_release.contains(&second),
        "第二个链接必须等待第一个链接结算"
    );
    assert!(after_release.contains(&first), "第二个链接不得取消第一个");
    assert!(after_release.contains(&second));
}

#[gpui::test]
fn markdown_local_link_scrolls_to_current_target_anchor(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, source, _source_path, _target) =
        open_watched_markdown(cx);
    let target = temp.path().join("project/target.md");
    std::fs::write(&target, "intro\n\n# Target\n\nbody\n").unwrap();

    emit_markdown_link(&source, &mut window, "target.md#target");
    window.run_until_parked();

    let target_view = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .filter_map(|panel| panel.markdown().cloned())
            .find(|view| {
                *view != source && view.read(cx).input_text_for_test(cx) == "intro\n\n# Target\n\nbody\n"
            })
            .unwrap()
    });
    target_view.read_with(cx, |view, _| {
        assert_eq!(view.rendered_revision_for_test(), 0);
        assert_eq!(view.reading_top_for_test(), (1, 0));
    });
}

#[gpui::test]
fn missing_markdown_local_link_anchor_preserves_reading_position(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, source, _source_path, _target) =
        open_watched_markdown(cx);
    let target = temp.path().join("project/target.md");
    std::fs::write(&target, "# One\n\n# Two\n").unwrap();
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&target).unwrap(),
    );
    let target_view = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .filter_map(|panel| panel.markdown().cloned())
            .find(|view| *view != source)
            .unwrap()
    });
    target_view.update(cx, |view, _| view.set_reading_for_test(1, 7));

    emit_markdown_link(&source, &mut window, "target.md#missing");
    window.run_until_parked();

    target_view.read_with(cx, |view, _| {
        assert_eq!(view.reading_top_for_test(), (1, 7));
    });
}

#[gpui::test]
fn markdown_fragment_waits_for_current_comrak_revision(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, source, _source_path, _target) =
        open_watched_markdown(cx);
    let target = temp.path().join("project/target.md");
    std::fs::write(&target, "# Target\n").unwrap();
    open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&target).unwrap(),
    );
    let (document, target_view) = root.read_with(cx, |root, cx| {
        let view = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .filter_map(|panel| panel.markdown().cloned())
            .find(|view| *view != source)
            .unwrap();
        let document = root
            .snapshot
            .documents()
            .find_map(|(id, location)| {
                (location.canonical_path().as_path() == target).then_some(id)
            })
            .unwrap();
        (document, view)
    });
    assert_eq!(
        target_view.read_with(cx, |view, _| view.reading_top_for_test()),
        (0, 0)
    );

    window.update(|window, cx| {
        let input = target_view.read(cx).source_for_test();
        input.update(cx, |input, cx| {
            gpui::EntityInputHandler::replace_text_in_range(
                input,
                Some(0..0),
                "intro\n\n",
                window,
                cx,
            );
        });
        root.update(cx, |root, cx| {
            root.scroll_markdown_fragment(document, Some("target"), cx);
        });
    });
    window.run_until_parked();

    target_view.read_with(cx, |view, _| {
        assert_eq!(view.rendered_revision_for_test(), 1);
        assert_eq!(view.reading_top_for_test(), (1, 0));
    });
}

#[gpui::test]
fn outside_markdown_link_retains_session_read_for_watcher_reload(cx: &mut TestAppContext) {
    let (temp, root, mut window, _document, source, _source_path, _target) =
        open_watched_markdown(cx);
    let target = temp.path().join("outside.md");
    std::fs::write(&target, "# Before\n").unwrap();

    emit_markdown_link(&source, &mut window, "../outside.md");
    window.run_until_parked();

    let (document, target_view) = root.read_with(cx, |root, cx| {
        root.snapshot
            .documents()
            .find_map(|(id, location)| {
                (location.canonical_path().as_path() == target).then(|| {
                    let view = root.projects[&project_id(PROJECT_A)]
                        .read(cx)
                        .panels
                        .values()
                        .filter_map(|panel| panel.markdown().cloned())
                        .find(|view| *view != source)
                        .unwrap();
                    (id, view)
                })
            })
            .unwrap()
    });
    wait_for_document_watch(&root, cx, &target);
    std::fs::write(&target, "# After\n").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while target_view.read_with(cx, |view, cx| view.input_text_for_test(cx)) != "# After\n" {
        if root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .unwrap()
                .is_read_revoked()
        }) {
            break;
        }
        window
            .executor()
            .advance_clock(std::time::Duration::from_millis(121));
        window.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "根外链接正文未重载");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(!root.read_with(cx, |root, _| {
        root.documents
            .session(document)
            .unwrap()
            .is_read_revoked()
    }));
    assert_eq!(
        target_view.read_with(cx, |view, cx| view.input_text_for_test(cx)),
        "# After\n"
    );
    let grant = root.read_with(cx, |root, _| {
        root.document_session_read_grants[&document].clone()
    });
    assert_eq!(grant.persistence(), slterm_path::GrantPersistence::Session);
    assert_eq!(grant.capabilities(), slterm_path::Capabilities::READ);
    assert_eq!(
        grant.path().as_path(),
        dunce::canonicalize(&target).unwrap()
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_all_views_from_request(group_id(GROUP_A), BTreeMap::new(), window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();
    assert!(root.read_with(cx, |root, _| {
        !root.document_session_read_grants.contains_key(&document)
    }));
}
