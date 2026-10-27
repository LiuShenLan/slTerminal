use std::str::FromStr;

use crate::{
    PanelGroup, PanelGroupId, Project, ProjectId, ProjectLayout, ProjectRegistry,
    ProjectRootStatus, ProjectSnapshot, ProjectSnapshotError, RunIntent, TerminalId,
    TerminalRecord, TerminalRecordError, ViewKey,
};
use slterm_path::CanonicalPath;

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
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
