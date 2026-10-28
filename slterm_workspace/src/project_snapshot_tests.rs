use std::str::FromStr;

use slterm_path::CanonicalPath;
use slterm_split::SplitDirection;

use crate::{
    LayoutError, LogicalSize, NewTerminalSpec, PanelGroup, PanelGroupId, Project,
    ProjectCommandError, ProjectId, ProjectLayout, ProjectRegistry, ProjectRegistryError,
    ProjectRootStatus, ProjectSnapshot, ProjectSnapshotError, RunIntent, SplitTerminalSpec,
    TerminalId, TerminalRecord, TerminalRecordError, ViewKey,
};

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";

fn project_id(text: &str) -> ProjectId {
    ProjectId::from_str(text).unwrap()
}

fn terminal_id(text: &str) -> TerminalId {
    TerminalId::from_str(text).unwrap()
}

fn path(text: &str) -> CanonicalPath {
    CanonicalPath::new(text).unwrap()
}

fn record(project: &str, terminal: &str, number: u64) -> TerminalRecord {
    TerminalRecord::new(
        terminal_id(terminal),
        project_id(project),
        number,
        path("D:/work/a"),
        path("C:/Program Files/PowerShell/7/pwsh.exe"),
        Some("build".to_owned()),
        RunIntent::Launch,
    )
    .unwrap()
}

fn registry_with_views(next_terminal_number: u64, terminals: &[&str]) -> ProjectRegistry {
    let project_id = project_id(PROJECT_A);
    let views = terminals
        .iter()
        .map(|terminal| ViewKey::Terminal {
            project_id,
            terminal_id: terminal_id(terminal),
        })
        .collect::<Vec<_>>();
    let layout = if views.is_empty() {
        ProjectLayout::empty(project_id)
    } else {
        ProjectLayout::new(
            project_id,
            PanelGroupId::from_str(GROUP_A).unwrap(),
            PanelGroup::new(views.clone(), views[0].clone()).unwrap(),
        )
        .unwrap()
    };
    ProjectRegistry::restore(
        vec![
            Project::restore(
                project_id,
                path("D:/work/a"),
                "a".to_owned(),
                layout,
                ProjectRootStatus::Ready,
                next_terminal_number,
            )
            .unwrap(),
        ],
        Some(project_id),
    )
    .unwrap()
}

fn group_id(text: &str) -> PanelGroupId {
    PanelGroupId::from_str(text).unwrap()
}

fn view(project: &str, terminal: &str) -> ViewKey {
    ViewKey::Terminal {
        project_id: project_id(project),
        terminal_id: terminal_id(terminal),
    }
}

fn empty_snapshot() -> ProjectSnapshot {
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    ProjectSnapshot::restore(registry, Vec::new()).unwrap()
}

fn new_terminal_spec(project: &str, group: &str, terminal: &str) -> NewTerminalSpec {
    NewTerminalSpec {
        project_id: project_id(project),
        terminal_id: terminal_id(terminal),
        target_group: group_id(group),
        shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
        custom_title: None,
    }
}

fn snapshot_with_two_terminals() -> ProjectSnapshot {
    let (snapshot, _) = empty_snapshot()
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    snapshot
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B))
        .unwrap()
        .0
}

#[test]
fn terminal_record_preserves_metadata_and_validates_custom_title() {
    let record = record(PROJECT_A, TERMINAL_A, 3);
    assert_eq!(record.number(), 3);
    assert_eq!(record.cwd(), &path("D:/work/a"));
    assert_eq!(
        record.shell(),
        &path("C:/Program Files/PowerShell/7/pwsh.exe")
    );
    assert_eq!(record.custom_title(), Some("build"));
    assert_eq!(record.run_intent(), RunIntent::Launch);

    for title in [Some(String::new()), Some(" \t".to_owned())] {
        assert_eq!(
            TerminalRecord::new(
                terminal_id(TERMINAL_A),
                project_id(PROJECT_A),
                0,
                path("D:/work/a"),
                path("C:/shell/pwsh.exe"),
                title,
                RunIntent::Launch,
            ),
            Err(TerminalRecordError::EmptyTitle)
        );
    }
    assert_eq!(
        TerminalRecord::new(
            terminal_id(TERMINAL_A),
            project_id(PROJECT_A),
            0,
            path("D:/work/a"),
            path("C:/shell/pwsh.exe"),
            Some("界".repeat(129)),
            RunIntent::Launch,
        ),
        Err(TerminalRecordError::TitleTooLong)
    );
}

#[test]
fn project_snapshot_restores_terminal_metadata_by_stable_id() {
    let registry = registry_with_views(1, &[TERMINAL_A]);
    let record = record(PROJECT_A, TERMINAL_A, 0);
    let snapshot = ProjectSnapshot::restore(registry.clone(), vec![record.clone()]).unwrap();

    assert_eq!(snapshot.registry(), &registry);
    assert_eq!(snapshot.terminal(terminal_id(TERMINAL_A)), Some(&record));
    assert_eq!(snapshot.terminals().collect::<Vec<_>>(), vec![&record]);
}

#[test]
fn project_snapshot_rejects_missing_and_orphan_terminal_records() {
    assert_eq!(
        ProjectSnapshot::restore(registry_with_views(1, &[TERMINAL_A]), Vec::new()),
        Err(ProjectSnapshotError::MissingTerminal(terminal_id(
            TERMINAL_A
        )))
    );
    assert_eq!(
        ProjectSnapshot::restore(
            registry_with_views(1, &[]),
            vec![record(PROJECT_A, TERMINAL_A, 0)],
        ),
        Err(ProjectSnapshotError::OrphanTerminal(terminal_id(
            TERMINAL_A
        )))
    );
}

#[test]
fn project_snapshot_rejects_terminal_for_unknown_project() {
    assert_eq!(
        ProjectSnapshot::restore(
            registry_with_views(0, &[]),
            vec![record(PROJECT_B, TERMINAL_A, 0)],
        ),
        Err(ProjectSnapshotError::TerminalProjectNotFound(terminal_id(
            TERMINAL_A
        )))
    );
}

#[test]
fn project_snapshot_rejects_terminal_project_mismatch() {
    let mut registry = registry_with_views(1, &[TERMINAL_A]);
    registry
        .add_project(project_id(PROJECT_B), path("D:/work/b"))
        .unwrap();

    assert_eq!(
        ProjectSnapshot::restore(registry, vec![record(PROJECT_B, TERMINAL_A, 0)]),
        Err(ProjectSnapshotError::TerminalProjectMismatch(terminal_id(
            TERMINAL_A
        )))
    );
}

#[test]
fn project_snapshot_rejects_duplicate_terminal_records_and_numbers() {
    assert_eq!(
        ProjectSnapshot::restore(
            registry_with_views(1, &[TERMINAL_A]),
            vec![
                record(PROJECT_A, TERMINAL_A, 0),
                record(PROJECT_A, TERMINAL_A, 0),
            ],
        ),
        Err(ProjectSnapshotError::DuplicateTerminal(terminal_id(
            TERMINAL_A
        )))
    );
    assert_eq!(
        ProjectSnapshot::restore(
            registry_with_views(2, &[TERMINAL_A, TERMINAL_B]),
            vec![
                record(PROJECT_A, TERMINAL_A, 0),
                record(PROJECT_A, TERMINAL_B, 0),
            ],
        ),
        Err(ProjectSnapshotError::DuplicateTerminalNumber {
            project_id: project_id(PROJECT_A),
            number: 0,
        })
    );
}

#[test]
fn project_snapshot_rejects_terminal_number_beyond_project_counter() {
    assert_eq!(
        ProjectSnapshot::restore(
            registry_with_views(1, &[TERMINAL_A]),
            vec![record(PROJECT_A, TERMINAL_A, 1)],
        ),
        Err(ProjectSnapshotError::TerminalNumberBeyondCounter(
            terminal_id(TERMINAL_A)
        ))
    );
}

#[test]
fn project_snapshot_terminal_mutations_preserve_consistency() {
    let mut snapshot = ProjectSnapshot::restore(
        registry_with_views(1, &[TERMINAL_A]),
        vec![record(PROJECT_A, TERMINAL_A, 0)],
    )
    .unwrap();
    let number = snapshot
        .registry_mut()
        .allocate_terminal_number(project_id(PROJECT_A))
        .unwrap();
    snapshot
        .registry_mut()
        .layout_mut(project_id(PROJECT_A))
        .unwrap()
        .insert_view(
            PanelGroupId::from_str(GROUP_A).unwrap(),
            ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_B),
            },
        )
        .unwrap();
    let mut added = record(PROJECT_A, TERMINAL_B, number);
    snapshot.add_terminal(added.clone()).unwrap();
    assert_eq!(snapshot.terminal(terminal_id(TERMINAL_B)), Some(&added));
    assert_eq!(snapshot.validate(), Ok(()));

    snapshot
        .set_run_intent(terminal_id(TERMINAL_B), RunIntent::Exited)
        .unwrap();
    added.set_run_intent(RunIntent::Exited);
    assert_eq!(
        snapshot
            .terminal(terminal_id(TERMINAL_B))
            .unwrap()
            .run_intent(),
        RunIntent::Exited
    );

    snapshot
        .registry_mut()
        .layout_mut(project_id(PROJECT_A))
        .unwrap()
        .close_view(&ViewKey::Terminal {
            project_id: project_id(PROJECT_A),
            terminal_id: terminal_id(TERMINAL_B),
        })
        .unwrap();
    assert_eq!(snapshot.remove_terminal(terminal_id(TERMINAL_B)), Ok(added));
    assert_eq!(snapshot.terminal(terminal_id(TERMINAL_B)), None);
    assert_eq!(snapshot.validate(), Ok(()));
}

#[test]
fn project_snapshot_failed_terminal_mutations_do_not_partially_change_state() {
    let original = ProjectSnapshot::restore(
        registry_with_views(1, &[TERMINAL_A]),
        vec![record(PROJECT_A, TERMINAL_A, 0)],
    )
    .unwrap();
    let mut snapshot = original.clone();

    assert_eq!(
        snapshot.add_terminal(record(PROJECT_A, TERMINAL_A, 0)),
        Err(ProjectSnapshotError::DuplicateTerminal(terminal_id(
            TERMINAL_A
        )))
    );
    assert_eq!(snapshot, original);
    assert_eq!(
        snapshot.add_terminal(record(PROJECT_A, TERMINAL_B, 1)),
        Err(ProjectSnapshotError::OrphanTerminal(terminal_id(
            TERMINAL_B
        )))
    );
    assert_eq!(snapshot, original);
    assert_eq!(
        snapshot.remove_terminal(terminal_id(TERMINAL_A)),
        Err(ProjectSnapshotError::MissingTerminal(terminal_id(
            TERMINAL_A
        )))
    );
    assert_eq!(snapshot, original);
    assert_eq!(
        snapshot.set_run_intent(terminal_id(TERMINAL_B), RunIntent::Exited),
        Err(ProjectSnapshotError::MissingTerminal(terminal_id(
            TERMINAL_B
        )))
    );
    assert_eq!(snapshot, original);
}

#[test]
fn new_terminal_commands_open_first_group_and_extend_existing_group() {
    let original = empty_snapshot();
    let (with_first, first) = original
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();

    assert_eq!(original, empty_snapshot());
    assert_eq!(first.number(), 0);
    assert_eq!(first.cwd(), &path("D:/work/a"));
    assert_eq!(first.run_intent(), RunIntent::Launch);
    let first_layout = with_first
        .registry()
        .project(project_id(PROJECT_A))
        .unwrap()
        .layout();
    assert_eq!(first_layout.group_ids(), vec![group_id(GROUP_A)]);
    assert_eq!(
        first_layout.group(group_id(GROUP_A)).unwrap().views(),
        &[view(PROJECT_A, TERMINAL_A)]
    );
    assert_eq!(
        first_layout.group(group_id(GROUP_A)).unwrap().active(),
        &view(PROJECT_A, TERMINAL_A)
    );
    assert_eq!(with_first.validate(), Ok(()));

    let (with_second, second) = with_first
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B))
        .unwrap();
    assert_eq!(second.number(), 1);
    let second_layout = with_second
        .registry()
        .project(project_id(PROJECT_A))
        .unwrap()
        .layout();
    assert_eq!(second_layout.group_ids(), vec![group_id(GROUP_A)]);
    assert_eq!(
        second_layout.group(group_id(GROUP_A)).unwrap().views(),
        &[view(PROJECT_A, TERMINAL_A), view(PROJECT_A, TERMINAL_B)]
    );
    assert_eq!(
        second_layout.group(group_id(GROUP_A)).unwrap().active(),
        &view(PROJECT_A, TERMINAL_B)
    );
    assert_eq!(with_second.validate(), Ok(()));
}

#[test]
fn split_terminal_command_creates_two_non_empty_groups() {
    let (original, _) = empty_snapshot()
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    let (split, added) = original
        .with_split_terminal(SplitTerminalSpec {
            new_terminal: new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
            new_group_id: group_id(GROUP_B),
            direction: SplitDirection::LeftRight,
            ratio: 0.5,
            viewport: LogicalSize::new(242.0, 80.0),
        })
        .unwrap();

    assert_eq!(added.number(), 1);
    let layout = split
        .registry()
        .project(project_id(PROJECT_A))
        .unwrap()
        .layout();
    assert_eq!(
        layout.group_ids(),
        vec![group_id(GROUP_A), group_id(GROUP_B)]
    );
    assert_eq!(
        layout.group(group_id(GROUP_A)).unwrap().views(),
        &[view(PROJECT_A, TERMINAL_A)]
    );
    assert_eq!(
        layout.group(group_id(GROUP_B)).unwrap().views(),
        &[view(PROJECT_A, TERMINAL_B)]
    );
    assert_eq!(
        layout.group(group_id(GROUP_B)).unwrap().active(),
        &view(PROJECT_A, TERMINAL_B)
    );
    assert_eq!(split.validate(), Ok(()));
    assert_eq!(
        original
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group_ids(),
        vec![group_id(GROUP_A)]
    );
}

#[test]
fn close_terminal_command_removes_record_group_and_preserves_project() {
    let original = snapshot_with_two_terminals();
    let close_result = original.without_terminal(terminal_id(TERMINAL_B));
    assert!(close_result.is_ok(), "关闭既有终端必须生成候选快照");
    let (closed, removed) = close_result.unwrap();
    assert_eq!(removed.id(), terminal_id(TERMINAL_B));
    assert_eq!(closed.terminal(terminal_id(TERMINAL_B)), None);
    let layout = closed
        .registry()
        .project(project_id(PROJECT_A))
        .unwrap()
        .layout();
    assert_eq!(
        layout.group(group_id(GROUP_A)).unwrap().views(),
        &[view(PROJECT_A, TERMINAL_A)]
    );
    assert_eq!(
        layout.group(group_id(GROUP_A)).map(PanelGroup::active),
        Some(&view(PROJECT_A, TERMINAL_A))
    );

    let (empty, _) = closed.without_terminal(terminal_id(TERMINAL_A)).unwrap();
    let project = empty.registry().project(project_id(PROJECT_A)).unwrap();
    assert!(project.layout().group_ids().is_empty());
    assert!(project.layout().tree().is_none());
    assert_eq!(empty.terminals().count(), 0);
    assert_eq!(empty.validate(), Ok(()));
}

#[test]
fn activate_and_exit_commands_only_change_their_owning_state() {
    let original = snapshot_with_two_terminals();
    let activated = original
        .with_active_terminal(terminal_id(TERMINAL_A))
        .unwrap();
    let layout = activated
        .registry()
        .project(project_id(PROJECT_A))
        .unwrap()
        .layout();
    assert_eq!(
        layout.group(group_id(GROUP_A)).map(PanelGroup::active),
        Some(&view(PROJECT_A, TERMINAL_A))
    );
    assert_eq!(
        activated.terminals().collect::<Vec<_>>(),
        original.terminals().collect::<Vec<_>>()
    );

    let exited = activated
        .with_terminal_run_intent(terminal_id(TERMINAL_A), RunIntent::Exited)
        .unwrap();
    assert_eq!(
        exited
            .terminal(terminal_id(TERMINAL_A))
            .unwrap()
            .run_intent(),
        RunIntent::Exited
    );
    assert_eq!(
        exited
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout(),
        layout
    );
}

#[test]
fn failed_terminal_creation_commands_return_errors_without_changing_input() {
    let (original, _) = empty_snapshot()
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    assert_eq!(
        original.with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A)),
        Err(ProjectCommandError::Snapshot(
            ProjectSnapshotError::DuplicateTerminal(terminal_id(TERMINAL_A))
        ))
    );
    assert_eq!(
        original.with_new_terminal(new_terminal_spec(PROJECT_B, GROUP_A, TERMINAL_B)),
        Err(ProjectCommandError::Registry(
            ProjectRegistryError::ProjectNotFound
        ))
    );
    let mut invalid_title = new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B);
    invalid_title.custom_title = Some(" \t".to_owned());
    assert_eq!(
        original.with_new_terminal(invalid_title),
        Err(ProjectCommandError::Record(TerminalRecordError::EmptyTitle))
    );

    let split = |viewport, ratio| SplitTerminalSpec {
        new_terminal: new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
        new_group_id: group_id(GROUP_B),
        direction: SplitDirection::LeftRight,
        ratio,
        viewport,
    };
    assert_eq!(
        original.with_split_terminal(split(LogicalSize::new(240.0, 80.0), 0.5)),
        Err(ProjectCommandError::Layout(LayoutError::InsufficientSpace))
    );
    assert_eq!(
        original.with_split_terminal(split(LogicalSize::new(242.0, 80.0), 1.0)),
        Err(ProjectCommandError::Layout(LayoutError::InvalidGeometry))
    );
    let mut duplicate_terminal = split(LogicalSize::new(242.0, 80.0), 0.5);
    duplicate_terminal.new_terminal.terminal_id = terminal_id(TERMINAL_A);
    assert_eq!(
        original.with_split_terminal(duplicate_terminal),
        Err(ProjectCommandError::Snapshot(
            ProjectSnapshotError::DuplicateTerminal(terminal_id(TERMINAL_A))
        ))
    );
    let mut missing_target = split(LogicalSize::new(242.0, 80.0), 0.5);
    missing_target.new_terminal.target_group = group_id(GROUP_B);
    assert_eq!(
        original.with_split_terminal(missing_target),
        Err(ProjectCommandError::Layout(LayoutError::GroupNotFound))
    );
    let mut duplicate_group = split(LogicalSize::new(242.0, 80.0), 0.5);
    duplicate_group.new_group_id = group_id(GROUP_A);
    assert_eq!(
        original.with_split_terminal(duplicate_group),
        Err(ProjectCommandError::Layout(LayoutError::DuplicateGroup))
    );
    assert_eq!(
        original
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .next_terminal_number(),
        1
    );
}

#[test]
fn unknown_terminal_commands_report_missing_record() {
    let (original, _) = empty_snapshot()
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    let expected = ProjectCommandError::Snapshot(ProjectSnapshotError::MissingTerminal(
        terminal_id(TERMINAL_B),
    ));
    assert_eq!(
        original.without_terminal(terminal_id(TERMINAL_B)),
        Err(expected.clone())
    );
    assert_eq!(
        original.with_active_terminal(terminal_id(TERMINAL_B)),
        Err(expected.clone())
    );
    assert_eq!(
        original.with_terminal_run_intent(terminal_id(TERMINAL_B), RunIntent::Exited),
        Err(expected)
    );
}

#[test]
fn candidate_commands_reject_an_invalid_input_snapshot() {
    let (original, _) = empty_snapshot()
        .with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    let mut invalid = original;
    invalid
        .registry_mut()
        .layout_mut(project_id(PROJECT_A))
        .unwrap()
        .close_view(&view(PROJECT_A, TERMINAL_A))
        .unwrap();
    let expected_snapshot = ProjectSnapshotError::OrphanTerminal(terminal_id(TERMINAL_A));
    let expected = ProjectCommandError::Snapshot(expected_snapshot.clone());
    assert_eq!(invalid.validate(), Err(expected_snapshot));

    assert_eq!(
        invalid.with_new_terminal(new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B)),
        Err(expected.clone())
    );
    assert_eq!(
        invalid.with_split_terminal(SplitTerminalSpec {
            new_terminal: new_terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
            new_group_id: group_id(GROUP_B),
            direction: SplitDirection::LeftRight,
            ratio: 0.5,
            viewport: LogicalSize::new(242.0, 80.0),
        }),
        Err(expected.clone())
    );
    assert_eq!(
        invalid.without_terminal(terminal_id(TERMINAL_A)),
        Err(expected.clone())
    );
    assert_eq!(
        invalid.with_active_terminal(terminal_id(TERMINAL_A)),
        Err(expected.clone())
    );
    assert_eq!(
        invalid.with_terminal_run_intent(terminal_id(TERMINAL_A), RunIntent::Exited),
        Err(expected)
    );
}
