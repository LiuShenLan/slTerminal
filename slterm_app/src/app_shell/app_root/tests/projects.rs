//! 项目元数据命令的候选提交与持久化集成测试。

use super::*;

fn execute_project(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: CommandId,
    project: &str,
    parameters: CommandParameters,
) -> Result<CommandOutcome, AppRootError> {
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                id,
                CommandSource::Application,
                CommandTarget::Project(project_id(project)),
                Some(parameters),
                window,
                cx,
            )
        })
    })
}

#[gpui::test]
fn project_rename_and_reorder_preserve_identity_layout_and_instances(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    for (project, root_path) in [
        (PROJECT_A, "D:/work/a"),
        (PROJECT_B, "D:/work/b"),
        (PROJECT_C, "D:/work/c"),
    ] {
        add_project(&root, &mut window, project, root_path);
    }
    activate_project(&root, &mut window, PROJECT_A);
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    activate_project(&root, &mut window, PROJECT_B);
    let (_, terminal_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let original_layout = root.read_with(cx, |root, _| {
        root.snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .clone()
    });

    assert!(matches!(
        execute_project(
            &root,
            &mut window,
            CommandId::RenameProject,
            PROJECT_A,
            CommandParameters::RenameProject {
                name: "  shared-name  ".to_owned(),
            },
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    assert!(matches!(
        execute_project(
            &root,
            &mut window,
            CommandId::RenameProject,
            PROJECT_B,
            CommandParameters::RenameProject {
                name: "shared-name".to_owned(),
            },
        )
        .unwrap(),
        CommandOutcome::Completed
    ));
    assert!(matches!(
        execute_project(
            &root,
            &mut window,
            CommandId::MoveProject,
            PROJECT_A,
            CommandParameters::MoveProject { new_index: 2 },
        )
        .unwrap(),
        CommandOutcome::Completed
    ));

    root.read_with(cx, |root, cx| {
        let registry = root.snapshot().registry();
        assert_eq!(registry.active(), Some(project_id(PROJECT_B)));
        assert_eq!(
            registry
                .projects()
                .iter()
                .map(|project| project.id())
                .collect::<Vec<_>>(),
            vec![
                project_id(PROJECT_B),
                project_id(PROJECT_C),
                project_id(PROJECT_A)
            ]
        );
        assert_eq!(
            registry
                .project(project_id(PROJECT_A))
                .unwrap()
                .display_name(),
            "shared-name"
        );
        assert_eq!(
            registry.project(project_id(PROJECT_A)).unwrap().layout(),
            &original_layout
        );
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), Some(panel));
    });
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1,
        terminal_entity
    );

    writer.flush().unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let restored = store.load_project_snapshot().unwrap();
    let current = root.read_with(cx, |root, _| root.snapshot().registry().clone());
    assert_eq!(restored.registry(), &current);

    let before_invalid = root.read_with(cx, |root, _| root.snapshot().clone());
    assert!(
        execute_project(
            &root,
            &mut window,
            CommandId::RenameProject,
            PROJECT_A,
            CommandParameters::RenameProject {
                name: " \t ".to_owned(),
            },
        )
        .is_err()
    );
    assert!(
        execute_project(
            &root,
            &mut window,
            CommandId::MoveProject,
            PROJECT_A,
            CommandParameters::MoveProject { new_index: 3 },
        )
        .is_err()
    );
    root.read_with(cx, |root, _| assert_eq!(root.snapshot(), &before_invalid));

    let wrong_target = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::RenameProject,
                CommandSource::Application,
                CommandTarget::Focused,
                Some(CommandParameters::RenameProject {
                    name: "wrong-target".to_owned(),
                }),
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        wrong_target,
        Err(AppRootError::UnexpectedParameters)
    ));
    open_side_menu_for_test(&mut window);
    let modal = execute_project(
        &root,
        &mut window,
        CommandId::RenameProject,
        PROJECT_A,
        CommandParameters::RenameProject {
            name: "modal".to_owned(),
        },
    );
    assert!(matches!(
        modal,
        Err(AppRootError::Focus(FocusError::ModalActive))
    ));
    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    root.read_with(cx, |root, _| assert_eq!(root.snapshot(), &before_invalid));
}
