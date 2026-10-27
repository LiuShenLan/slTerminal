use std::str::FromStr;

use slterm_path::CanonicalPath;

use crate::{
    AddProjectOutcome, PanelGroup, PanelGroupId, Project, ProjectId, ProjectLayout,
    ProjectRegistry, ProjectRegistryError, ProjectRootStatus, TerminalId, ViewKey,
};

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const PROJECT_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";

fn id(text: &str) -> ProjectId {
    ProjectId::from_str(text).unwrap()
}

fn root(text: &str) -> CanonicalPath {
    CanonicalPath::new(text).unwrap()
}

/// 给项目放入可区分的非空布局，避免空布局掩盖关联被重置的问题。
fn populate_layout(registry: &mut ProjectRegistry, project: &str) {
    let project_id = id(project);
    let view = ViewKey::Terminal {
        project_id,
        terminal_id: TerminalId::from_str(TERMINAL_A).unwrap(),
    };
    let group = PanelGroup::new(vec![view.clone()], view.clone()).unwrap();
    assert!(
        registry.layout_mut(project_id).is_some(),
        "已登记项目必须提供受控的可变布局"
    );
    assert_eq!(
        registry
            .layout_mut(project_id)
            .unwrap()
            .open_first_group(PanelGroupId::from_str(GROUP_A).unwrap(), group),
        Ok(())
    );
    assert_eq!(
        registry.project(project_id).unwrap().layout().view_keys(),
        vec![view]
    );
}

/// 新根追加并激活，默认名消歧；重复根只聚焦既有项目且不复制布局。
#[test]
fn project_registry_add_activates_new_and_focuses_duplicate_root() {
    let mut registry = ProjectRegistry::new();
    assert_eq!(registry.active(), None);

    assert_eq!(
        registry.add_project(id(PROJECT_A), root("D:/work/alpha")),
        Ok(AddProjectOutcome::Added(id(PROJECT_A)))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_A)));
    assert_eq!(registry.projects().len(), 1);
    assert_eq!(registry.projects()[0].display_name(), "alpha");
    assert!(registry.projects()[0].layout().group_ids().is_empty());
    populate_layout(&mut registry, PROJECT_A);

    assert_eq!(
        registry.add_project(id(PROJECT_B), root("D:/else/alpha")),
        Ok(AddProjectOutcome::Added(id(PROJECT_B)))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_B)));
    assert_eq!(registry.projects()[1].display_name(), "alpha (2)");

    let original = registry.projects().to_vec();
    assert_eq!(
        registry.add_project(id(PROJECT_C), root("D:/work/alpha")),
        Ok(AddProjectOutcome::Existing(id(PROJECT_A)))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_A)));
    assert_eq!(registry.projects(), original);

    assert_eq!(
        registry.add_project(id(PROJECT_A), root("D:/else/alpha")),
        Err(ProjectRegistryError::DuplicateProject)
    );
    assert_eq!(registry.active(), Some(id(PROJECT_A)));
    assert_eq!(registry.projects(), original);
}

/// 激活只切换身份；移除活跃项先取下一个、再取上一个，全部移除进入无项目态。
#[test]
fn project_registry_activation_and_ordered_removal() {
    let mut registry = ProjectRegistry::new();
    for (id_text, path) in [
        (PROJECT_A, "D:/work/a"),
        (PROJECT_B, "D:/work/b"),
        (PROJECT_C, "D:/work/c"),
    ] {
        registry.add_project(id(id_text), root(path)).unwrap();
    }
    assert_eq!(
        registry.activate_project(id(PROJECT_B)),
        Ok(()),
        "既有项目必须可激活"
    );
    assert_eq!(registry.active(), Some(id(PROJECT_B)));

    assert_eq!(
        registry
            .remove_project(id(PROJECT_B))
            .map(|project| project.id()),
        Ok(id(PROJECT_B))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_C)));
    assert_eq!(
        registry
            .remove_project(id(PROJECT_C))
            .map(|project| project.id()),
        Ok(id(PROJECT_C))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_A)));
    assert_eq!(
        registry
            .remove_project(id(PROJECT_A))
            .map(|project| project.id()),
        Ok(id(PROJECT_A))
    );
    assert_eq!(registry.active(), None);
    assert!(registry.projects().is_empty());

    assert_eq!(
        registry.activate_project(id(PROJECT_A)),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert_eq!(
        registry.remove_project(id(PROJECT_A)),
        Err(ProjectRegistryError::ProjectNotFound)
    );
}

/// 重命名只改显示名，允许重名，空白与超过 80 字符的输入保持原值。
#[test]
fn project_registry_rename_validates_without_touching_identity() {
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(id(PROJECT_A), root("D:/work/a"))
        .unwrap();
    registry
        .add_project(id(PROJECT_B), root("D:/work/b"))
        .unwrap();
    populate_layout(&mut registry, PROJECT_A);
    let original = registry.project(id(PROJECT_A)).unwrap().clone();

    assert_eq!(
        registry.rename_project(id(PROJECT_A), "  renamed  "),
        Ok(())
    );
    let renamed = registry.project(id(PROJECT_A)).unwrap();
    assert_eq!(renamed.display_name(), "renamed");
    assert_eq!(renamed.id(), original.id());
    assert_eq!(renamed.root(), original.root());
    assert_eq!(renamed.layout(), original.layout());
    assert_eq!(registry.rename_project(id(PROJECT_B), "renamed"), Ok(()));

    assert_eq!(
        registry.rename_project(id(PROJECT_A), " \t "),
        Err(ProjectRegistryError::EmptyName)
    );
    assert_eq!(
        registry.rename_project(id(PROJECT_A), &"界".repeat(81)),
        Err(ProjectRegistryError::NameTooLong)
    );
    assert_eq!(
        registry.project(id(PROJECT_A)).unwrap().display_name(),
        "renamed"
    );
    assert_eq!(
        registry.rename_project(id(PROJECT_A), &"界".repeat(80)),
        Ok(())
    );
}

/// 排序只改变导航顺序，不改变活跃项目；非法索引不得产生部分移动。
#[test]
fn project_registry_reorder_preserves_active_and_rejects_bad_index() {
    let mut registry = ProjectRegistry::new();
    for (id_text, path) in [
        (PROJECT_A, "D:/work/a"),
        (PROJECT_B, "D:/work/b"),
        (PROJECT_C, "D:/work/c"),
    ] {
        registry.add_project(id(id_text), root(path)).unwrap();
    }
    registry.activate_project(id(PROJECT_B)).unwrap();

    assert_eq!(registry.move_project(id(PROJECT_A), 2), Ok(()));
    assert_eq!(
        registry
            .projects()
            .iter()
            .map(|project| project.id())
            .collect::<Vec<_>>(),
        vec![id(PROJECT_B), id(PROJECT_C), id(PROJECT_A)]
    );
    assert_eq!(registry.active(), Some(id(PROJECT_B)));
    let before_invalid_move = registry.projects().to_vec();
    assert_eq!(
        registry.move_project(id(PROJECT_A), 3),
        Err(ProjectRegistryError::InvalidProjectIndex)
    );
    assert_eq!(registry.projects(), before_invalid_move);
    assert_eq!(registry.active(), Some(id(PROJECT_B)));
}

/// 重定位冲突保留全部关联；成功只替换根，不改身份、名称、布局或活跃项。
#[test]
fn project_registry_relocation_preserves_layout_and_is_atomic() {
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(id(PROJECT_A), root("D:/work/a"))
        .unwrap();
    registry
        .add_project(id(PROJECT_B), root("D:/work/b"))
        .unwrap();
    registry.activate_project(id(PROJECT_A)).unwrap();
    populate_layout(&mut registry, PROJECT_A);
    let original = registry.project(id(PROJECT_A)).unwrap().clone();

    assert_eq!(
        registry.relocate_project(id(PROJECT_A), root("D:/work/b")),
        Err(ProjectRegistryError::DuplicateRoot(id(PROJECT_B)))
    );
    assert_eq!(registry.project(id(PROJECT_A)), Some(&original));
    assert_eq!(
        registry.relocate_project(id(PROJECT_A), root("D:/new/root")),
        Ok(())
    );
    let relocated = registry.project(id(PROJECT_A)).unwrap();
    assert_eq!(relocated.id(), original.id());
    assert_eq!(relocated.display_name(), original.display_name());
    assert_eq!(relocated.layout(), original.layout());
    assert_eq!(
        relocated.root().as_path(),
        std::path::Path::new("D:/new/root")
    );
    assert_eq!(registry.active(), Some(id(PROJECT_A)));
}

/// 重复身份与缺失项目命令全部失败，项目顺序、关联和活跃项保持原值。
#[test]
fn project_registry_rejects_invalid_commands_without_mutation() {
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(id(PROJECT_A), root("D:/work/a"))
        .unwrap();
    registry
        .add_project(id(PROJECT_B), root("D:/work/b"))
        .unwrap();
    let original = registry.projects().to_vec();
    let active = registry.active();

    assert_eq!(
        registry.add_project(id(PROJECT_A), root("D:/work/other")),
        Err(ProjectRegistryError::DuplicateProject)
    );
    for result in [
        registry.rename_project(id(PROJECT_C), "renamed"),
        registry.move_project(id(PROJECT_C), 0),
        registry.relocate_project(id(PROJECT_C), root("D:/work/c")),
        registry.activate_project(id(PROJECT_C)),
    ] {
        assert_eq!(result, Err(ProjectRegistryError::ProjectNotFound));
    }
    assert_eq!(
        registry.remove_project(id(PROJECT_C)),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert!(registry.layout_mut(id(PROJECT_C)).is_none());
    assert_eq!(registry.projects(), original);
    assert_eq!(registry.active(), active);
}

/// 同位排序与同根重定位为无操作；移除非活跃项不改变活跃关联。
#[test]
fn project_registry_noop_commands_and_inactive_removal_preserve_active() {
    let mut registry = ProjectRegistry::new();
    for (id_text, path) in [
        (PROJECT_A, "D:/work/a"),
        (PROJECT_B, "D:/work/b"),
        (PROJECT_C, "D:/work/c"),
    ] {
        registry.add_project(id(id_text), root(path)).unwrap();
    }
    registry.activate_project(id(PROJECT_B)).unwrap();
    populate_layout(&mut registry, PROJECT_B);
    let original = registry.projects().to_vec();

    assert_eq!(registry.move_project(id(PROJECT_B), 1), Ok(()));
    assert_eq!(
        registry.relocate_project(id(PROJECT_B), root("D:/work/b")),
        Ok(())
    );
    assert_eq!(registry.projects(), original);
    assert_eq!(registry.active(), Some(id(PROJECT_B)));

    assert_eq!(
        registry
            .remove_project(id(PROJECT_A))
            .map(|project| project.id()),
        Ok(id(PROJECT_A))
    );
    assert_eq!(registry.active(), Some(id(PROJECT_B)));
    assert_eq!(registry.project(id(PROJECT_B)), original.get(1));
}

/// 根状态与项目内终端编号独立推进，重定位成功后根重新就绪。
#[test]
fn project_registry_root_status_and_terminal_numbers_follow_project_commands() {
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(id(PROJECT_A), root("D:/work/a"))
        .unwrap();
    registry
        .add_project(id(PROJECT_B), root("D:/work/b"))
        .unwrap();
    let project_a = registry.project(id(PROJECT_A)).unwrap();
    assert_eq!(project_a.root_status(), ProjectRootStatus::Ready);
    assert_eq!(project_a.next_terminal_number(), 0);

    assert_eq!(
        registry.set_root_status(id(PROJECT_A), ProjectRootStatus::Inaccessible),
        Ok(())
    );
    assert_eq!(registry.allocate_terminal_number(id(PROJECT_A)), Ok(0));
    assert_eq!(registry.allocate_terminal_number(id(PROJECT_A)), Ok(1));
    assert_eq!(registry.allocate_terminal_number(id(PROJECT_B)), Ok(0));
    assert_eq!(
        registry
            .project(id(PROJECT_A))
            .unwrap()
            .next_terminal_number(),
        2
    );

    assert_eq!(
        registry.relocate_project(id(PROJECT_A), root("D:/new/a")),
        Ok(())
    );
    let relocated = registry.project(id(PROJECT_A)).unwrap();
    assert_eq!(relocated.root_status(), ProjectRootStatus::Ready);
    assert_eq!(relocated.next_terminal_number(), 2);
    registry
        .set_root_status(id(PROJECT_A), ProjectRootStatus::Inaccessible)
        .unwrap();
    assert_eq!(
        registry.relocate_project(id(PROJECT_A), root("D:/new/a")),
        Ok(())
    );
    assert_eq!(
        registry.project(id(PROJECT_A)).unwrap().root_status(),
        ProjectRootStatus::Ready
    );
    assert_eq!(
        registry.set_root_status(id(PROJECT_C), ProjectRootStatus::Ready),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert_eq!(
        registry.allocate_terminal_number(id(PROJECT_C)),
        Err(ProjectRegistryError::ProjectNotFound)
    );
}

/// 持久恢复保留全部项目字段，并拒绝身份、名称、布局和活跃关系损坏。
#[test]
fn project_registry_restore_preserves_metadata_and_rejects_invalid_relations() {
    let restored_a = Project::restore(
        id(PROJECT_A),
        root("D:/work/a"),
        "custom".to_owned(),
        ProjectLayout::empty(id(PROJECT_A)),
        ProjectRootStatus::Inaccessible,
        41,
    );
    assert!(restored_a.is_ok(), "合法项目必须可精确恢复");
    let restored_a = restored_a.unwrap();
    assert_eq!(restored_a.display_name(), "custom");
    assert_eq!(restored_a.root_status(), ProjectRootStatus::Inaccessible);
    assert_eq!(restored_a.next_terminal_number(), 41);

    let restored_b = Project::restore(
        id(PROJECT_B),
        root("D:/work/b"),
        "custom".to_owned(),
        ProjectLayout::empty(id(PROJECT_B)),
        ProjectRootStatus::RelocationRequired,
        7,
    )
    .unwrap();
    let restored = ProjectRegistry::restore(
        vec![restored_a.clone(), restored_b.clone()],
        Some(id(PROJECT_B)),
    );
    assert!(restored.is_ok(), "合法项目集合必须可恢复");
    let mut restored = restored.unwrap();
    assert_eq!(restored.projects(), &[restored_a.clone(), restored_b]);
    assert_eq!(restored.active(), Some(id(PROJECT_B)));
    assert_eq!(restored.allocate_terminal_number(id(PROJECT_A)), Ok(41));

    assert_eq!(
        Project::restore(
            id(PROJECT_A),
            root("D:/work/a"),
            " \t ".to_owned(),
            ProjectLayout::empty(id(PROJECT_A)),
            ProjectRootStatus::Ready,
            0,
        ),
        Err(ProjectRegistryError::EmptyName)
    );
    let long_name = "界".repeat(81);
    let restored_long_name = Project::restore(
        id(PROJECT_A),
        root("D:/work/a"),
        long_name.clone(),
        ProjectLayout::empty(id(PROJECT_A)),
        ProjectRootStatus::Ready,
        0,
    );
    assert!(
        restored_long_name.is_ok(),
        "长默认名及消歧后缀必须可精确恢复"
    );
    assert_eq!(restored_long_name.unwrap().display_name(), long_name);
    assert_eq!(
        Project::restore(
            id(PROJECT_A),
            root("D:/work/a"),
            "custom".to_owned(),
            ProjectLayout::empty(id(PROJECT_B)),
            ProjectRootStatus::Ready,
            0,
        ),
        Err(ProjectRegistryError::LayoutProjectMismatch)
    );
    assert_eq!(
        ProjectRegistry::restore(
            vec![restored_a.clone(), restored_a.clone()],
            Some(id(PROJECT_A))
        ),
        Err(ProjectRegistryError::DuplicateProject)
    );
    assert_eq!(
        ProjectRegistry::restore(vec![restored_a.clone()], Some(id(PROJECT_B))),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert_eq!(
        ProjectRegistry::restore(vec![restored_a.clone()], None),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert_eq!(
        ProjectRegistry::restore(Vec::new(), Some(id(PROJECT_A))),
        Err(ProjectRegistryError::ProjectNotFound)
    );
    assert!(ProjectRegistry::restore(Vec::new(), None).is_ok());

    let duplicate_root = Project::restore(
        id(PROJECT_B),
        root("D:/work/a"),
        "other".to_owned(),
        ProjectLayout::empty(id(PROJECT_B)),
        ProjectRootStatus::Ready,
        0,
    )
    .unwrap();
    assert_eq!(
        ProjectRegistry::restore(
            vec![restored_a.clone(), duplicate_root],
            Some(id(PROJECT_A))
        ),
        Err(ProjectRegistryError::DuplicateRoot(id(PROJECT_A)))
    );

    let duplicate_group_projects = [PROJECT_A, PROJECT_B].map(|project| {
        let project_id = id(project);
        let view = ViewKey::Terminal {
            project_id,
            terminal_id: TerminalId::from_str(TERMINAL_A).unwrap(),
        };
        let layout = ProjectLayout::new(
            project_id,
            PanelGroupId::from_str(GROUP_A).unwrap(),
            PanelGroup::new(vec![view.clone()], view).unwrap(),
        )
        .unwrap();
        Project::restore(
            project_id,
            root(if project == PROJECT_A {
                "D:/work/a"
            } else {
                "D:/work/b"
            }),
            "custom".to_owned(),
            layout,
            ProjectRootStatus::Ready,
            0,
        )
        .unwrap()
    });
    assert_eq!(
        ProjectRegistry::restore(
            duplicate_group_projects.into_iter().collect(),
            Some(id(PROJECT_A))
        ),
        Err(ProjectRegistryError::DuplicatePanelGroup(
            PanelGroupId::from_str(GROUP_A).unwrap()
        ))
    );

    let exhausted = Project::restore(
        id(PROJECT_A),
        root("D:/work/a"),
        "custom".to_owned(),
        ProjectLayout::empty(id(PROJECT_A)),
        ProjectRootStatus::Ready,
        u64::MAX,
    )
    .unwrap();
    let mut exhausted = ProjectRegistry::restore(vec![exhausted], Some(id(PROJECT_A))).unwrap();
    let before = exhausted.projects().to_vec();
    assert_eq!(
        exhausted.allocate_terminal_number(id(PROJECT_A)),
        Err(ProjectRegistryError::TerminalNumberExhausted)
    );
    assert_eq!(exhausted.projects(), before);
}
