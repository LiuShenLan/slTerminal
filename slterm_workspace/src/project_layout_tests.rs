//! ProjectLayout 领域行为与规范 DTO 测试。

use std::collections::BTreeSet;
use std::str::FromStr;

use proptest::prelude::*;
use slterm_split::{SplitDirection, SplitTree};

use crate::{
    LayoutDtoError, LayoutError, LogicalSize, PanelGroup, PanelGroupId, ProjectId, ProjectLayout,
    TerminalId, ViewKey, ViewKeyError,
};

fn project(sequence: u8) -> ProjectId {
    ProjectId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d00{sequence:02x}")).unwrap()
}

fn group_id(sequence: u8) -> PanelGroupId {
    PanelGroupId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d00{sequence:02x}")).unwrap()
}

fn terminal(sequence: u8) -> TerminalId {
    TerminalId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d00{sequence:02x}")).unwrap()
}

fn view(project_id: ProjectId, sequence: u8) -> ViewKey {
    ViewKey::Terminal {
        project_id,
        terminal_id: terminal(sequence),
    }
}

fn assert_group(
    layout: &ProjectLayout,
    group_id: PanelGroupId,
    views: &[ViewKey],
    active: &ViewKey,
) {
    let group = layout.group(group_id).unwrap();
    assert_eq!(group.views(), views);
    assert_eq!(group.active(), active);
}

fn two_group_layout() -> (ProjectLayout, [ViewKey; 4]) {
    let project_id = project(1);
    let views = [
        view(project_id, 1),
        view(project_id, 2),
        view(project_id, 3),
        view(project_id, 4),
    ];
    let first =
        PanelGroup::new(vec![views[0].clone(), views[1].clone()], views[0].clone()).unwrap();
    let second =
        PanelGroup::new(vec![views[2].clone(), views[3].clone()], views[2].clone()).unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), first).unwrap();
    layout
        .split(
            group_id(1),
            group_id(2),
            second,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(242.0, 80.0),
        )
        .unwrap();
    (layout, views)
}

fn single_layout_fixture() -> (ProjectLayout, String) {
    let project_id = project(1);
    let group_id = group_id(1);
    let first = view(project_id, 1);
    let group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let layout = ProjectLayout::new(project_id, group_id, group).unwrap();
    let key = first.to_canonical_json().unwrap();
    let project = project_id.to_string();
    let group = group_id.to_string();
    let json = format!(
        r#"{{"groups":[{{"active":{key},"group_id":"{group}","views":[{key}]}}],"kind":"project_layout","project_id":"{project}","tree":{{"group_id":"{group}","kind":"group"}},"version":1}}"#
    );
    (layout, json)
}

fn two_group_dto_fixture() -> (ProjectLayout, String) {
    let (layout, views) = two_group_layout();
    let [first, second, third, fourth] = views.map(|view| view.to_canonical_json().unwrap());
    let project = project(1).to_string();
    let first_group = group_id(1).to_string();
    let second_group = group_id(2).to_string();
    let json = format!(
        r#"{{"groups":[{{"active":{first},"group_id":"{first_group}","views":[{first},{second}]}},{{"active":{third},"group_id":"{second_group}","views":[{third},{fourth}]}}],"kind":"project_layout","project_id":"{project}","tree":{{"direction":"left_right","first":{{"group_id":"{first_group}","kind":"group"}},"kind":"split","ratio":0.5,"second":{{"group_id":"{second_group}","kind":"group"}}}},"version":1}}"#
    );
    (layout, json)
}

fn root_ratio(layout: &ProjectLayout) -> f32 {
    match layout.tree().unwrap() {
        SplitTree::Split { ratio, .. } => *ratio,
        SplitTree::Leaf(_) => panic!("测试布局必须包含分割节点"),
    }
}

#[test]
fn project_layout_preserves_project_tree_and_group_identity() {
    let project_id = project(1);
    let group_id = group_id(1);
    let active = view(project_id, 1);
    let group = PanelGroup::new(vec![active.clone()], active).unwrap();

    let layout = ProjectLayout::new(project_id, group_id, group.clone()).unwrap();

    assert_eq!(layout.project_id(), project_id);
    assert_eq!(layout.group_ids(), [group_id]);
    assert_eq!(layout.group(group_id), Some(&group));
}

#[test]
fn project_layout_rejects_views_outside_project_scope() {
    for foreign in [view(project(2), 1), ViewKey::AppSettingsOverlay] {
        let group = PanelGroup::new(vec![foreign.clone()], foreign).unwrap();
        assert_eq!(
            ProjectLayout::new(project(1), group_id(1), group),
            Err(LayoutError::ProjectMismatch)
        );
    }
}

#[test]
fn project_layout_group_view_commands_preserve_order_and_activation() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    let group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), group).unwrap();

    layout.insert_view(group_id(1), second.clone()).unwrap();
    assert_group(
        &layout,
        group_id(1),
        &[first.clone(), second.clone()],
        &second,
    );
    layout.activate_view(group_id(1), &first).unwrap();
    assert_group(&layout, group_id(1), &[first.clone(), second], &first);
}

#[test]
fn project_layout_group_commands_reject_invalid_references_without_mutation() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    let group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let original = ProjectLayout::new(project_id, group_id(1), group).unwrap();

    for (target, content, expected) in [
        (1, first.clone(), LayoutError::DuplicateView),
        (1, view(project(2), 2), LayoutError::ProjectMismatch),
        (1, ViewKey::AppSettingsOverlay, LayoutError::ProjectMismatch),
        (9, second.clone(), LayoutError::GroupNotFound),
    ] {
        let mut layout = original.clone();
        assert_eq!(layout.insert_view(group_id(target), content), Err(expected));
        assert_eq!(layout, original);
    }

    let mut layout = original.clone();
    assert_eq!(
        layout.activate_view(group_id(1), &second),
        Err(LayoutError::ViewNotFound)
    );
    assert_eq!(
        layout.activate_view(group_id(9), &first),
        Err(LayoutError::GroupNotFound)
    );
    assert_eq!(layout.close_view(&second), Err(LayoutError::ViewNotFound));
    let new_group = PanelGroup::new(vec![second.clone()], second).unwrap();
    assert_eq!(
        layout.open_first_group(group_id(2), new_group),
        Err(LayoutError::LayoutNotEmpty)
    );
    assert_eq!(layout, original);
}

#[test]
fn project_layout_close_uses_neighbor_then_removes_empty_group() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    let third = view(project_id, 3);
    let group = PanelGroup::new(
        vec![first.clone(), second.clone(), third.clone()],
        second.clone(),
    )
    .unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), group).unwrap();

    layout.close_view(&second).unwrap();
    assert_group(
        &layout,
        group_id(1),
        &[first.clone(), third.clone()],
        &third,
    );
    layout.close_view(&third).unwrap();
    assert_group(&layout, group_id(1), std::slice::from_ref(&first), &first);
    layout.close_view(&first).unwrap();
    assert!(layout.group_ids().is_empty());
    assert!(layout.view_keys().is_empty());

    let reopened = view(project_id, 4);
    let group = PanelGroup::new(vec![reopened.clone()], reopened.clone()).unwrap();
    layout.open_first_group(group_id(2), group).unwrap();
    assert_group(
        &layout,
        group_id(2),
        std::slice::from_ref(&reopened),
        &reopened,
    );
}

#[test]
fn project_layout_resize_changes_only_geometry() {
    let (mut layout, views) = two_group_layout();

    layout
        .resize_split(&[], 0.75, LogicalSize::new(480.0, 80.0))
        .unwrap();

    assert_eq!(root_ratio(&layout), 0.75);
    assert_eq!(layout.view_keys(), views);
    assert_group(
        &layout,
        group_id(1),
        &[views[0].clone(), views[1].clone()],
        &views[0],
    );
    assert_group(
        &layout,
        group_id(2),
        &[views[2].clone(), views[3].clone()],
        &views[2],
    );
}

#[test]
fn project_layout_resize_enforces_nested_minimum_space() {
    let (mut layout, views) = two_group_layout();
    let fifth = view(project(1), 5);
    let group = PanelGroup::new(vec![fifth.clone()], fifth).unwrap();
    layout
        .split(
            group_id(2),
            group_id(3),
            group,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(486.0, 80.0),
        )
        .unwrap();
    let original = layout.clone();

    assert_eq!(
        layout.resize_split(&[], 0.75, LogicalSize::new(480.0, 80.0)),
        Err(LayoutError::InsufficientSpace)
    );
    assert_eq!(layout, original);
    assert_eq!(
        layout.view_keys(),
        [
            views[0].clone(),
            views[1].clone(),
            views[2].clone(),
            views[3].clone(),
            view(project(1), 5)
        ]
    );
}

#[test]
fn project_layout_resize_rejects_invalid_requests_without_mutation() {
    let (original, views) = two_group_layout();

    for (path, ratio, viewport, expected) in [
        (
            &[][..],
            f32::NAN,
            LogicalSize::new(480.0, 80.0),
            LayoutError::InvalidGeometry,
        ),
        (
            &[][..],
            0.5,
            LogicalSize::new(f32::INFINITY, 80.0),
            LayoutError::InvalidGeometry,
        ),
        (
            &[true][..],
            0.5,
            LogicalSize::new(480.0, 80.0),
            LayoutError::SplitNotFound,
        ),
        (
            &[true, true][..],
            0.5,
            LogicalSize::new(480.0, 80.0),
            LayoutError::SplitNotFound,
        ),
    ] {
        let mut layout = original.clone();
        assert_eq!(layout.resize_split(path, ratio, viewport), Err(expected));
        assert_eq!(layout, original);
        assert_eq!(layout.view_keys(), views);
    }
}

#[test]
fn project_layout_move_preserves_views_and_collapses_empty_source() {
    let (mut layout, views) = two_group_layout();

    layout.move_view(&views[1], group_id(2), 1).unwrap();
    assert_group(&layout, group_id(1), &[views[0].clone()], &views[0]);
    assert_group(
        &layout,
        group_id(2),
        &[views[2].clone(), views[1].clone(), views[3].clone()],
        &views[1],
    );

    layout.move_view(&views[0], group_id(2), 0).unwrap();
    assert_eq!(layout.group_ids(), [group_id(2)]);
    assert_group(
        &layout,
        group_id(2),
        &[
            views[0].clone(),
            views[2].clone(),
            views[1].clone(),
            views[3].clone(),
        ],
        &views[0],
    );

    layout.move_view(&views[2], group_id(2), 3).unwrap();
    assert_group(
        &layout,
        group_id(2),
        &[
            views[0].clone(),
            views[1].clone(),
            views[3].clone(),
            views[2].clone(),
        ],
        &views[2],
    );
}

#[test]
fn project_layout_merge_appends_source_in_stable_order() {
    let (mut layout, views) = two_group_layout();

    layout.merge_groups(group_id(2), group_id(1)).unwrap();

    assert_eq!(layout.group_ids(), [group_id(1)]);
    assert_group(&layout, group_id(1), &views, &views[0]);
    assert_eq!(layout.view_keys(), views);
}

#[test]
fn project_layout_move_and_merge_reject_invalid_targets_without_mutation() {
    let (original, views) = two_group_layout();
    let mut layout = original.clone();
    let missing = view(project(1), 9);

    assert_eq!(
        layout.move_view(&views[0], group_id(9), 0),
        Err(LayoutError::GroupNotFound)
    );
    assert_eq!(
        layout.move_view(&missing, group_id(2), 0),
        Err(LayoutError::ViewNotFound)
    );
    assert_eq!(
        layout.move_view(&views[0], group_id(2), 3),
        Err(LayoutError::InvalidViewIndex)
    );
    assert_eq!(
        layout.move_view(&views[0], group_id(1), 2),
        Err(LayoutError::InvalidViewIndex)
    );
    assert_eq!(
        layout.merge_groups(group_id(1), group_id(1)),
        Err(LayoutError::SameGroup)
    );
    assert_eq!(
        layout.merge_groups(group_id(9), group_id(1)),
        Err(LayoutError::GroupNotFound)
    );
    assert_eq!(layout, original);
}

#[test]
fn project_layout_split_view_moves_content_without_duplicates() {
    let (mut layout, views) = two_group_layout();

    layout
        .split_view(
            &views[1],
            group_id(2),
            group_id(3),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(486.0, 80.0),
        )
        .unwrap();
    assert_eq!(layout.group_ids(), [group_id(1), group_id(2), group_id(3)]);
    assert_group(&layout, group_id(1), &[views[0].clone()], &views[0]);
    assert_group(&layout, group_id(3), &[views[1].clone()], &views[1]);

    layout
        .split_view(
            &views[0],
            group_id(2),
            group_id(4),
            SplitDirection::TopBottom,
            0.5,
            LogicalSize::new(486.0, 162.0),
        )
        .unwrap();
    assert_eq!(layout.group_ids(), [group_id(2), group_id(4), group_id(3)]);
    assert_eq!(
        layout.view_keys(),
        [
            views[2].clone(),
            views[3].clone(),
            views[0].clone(),
            views[1].clone(),
        ]
    );
}

#[test]
fn project_layout_split_view_within_group_preserves_order_and_activation() {
    let project_id = project(1);
    let views = [
        view(project_id, 1),
        view(project_id, 2),
        view(project_id, 3),
    ];
    let group = PanelGroup::new(views.to_vec(), views[0].clone()).unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), group).unwrap();

    layout
        .split_view(
            &views[1],
            group_id(1),
            group_id(2),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(242.0, 80.0),
        )
        .unwrap();

    assert_eq!(layout.group_ids(), [group_id(1), group_id(2)]);
    assert_group(
        &layout,
        group_id(1),
        &[views[0].clone(), views[2].clone()],
        &views[0],
    );
    assert_group(
        &layout,
        group_id(2),
        std::slice::from_ref(&views[1]),
        &views[1],
    );
}

#[test]
fn project_layout_split_view_rejects_invalid_sources_without_mutation() {
    let (original, views) = two_group_layout();
    let mut layout = original.clone();
    let missing = view(project(1), 9);
    assert_eq!(
        layout.split_view(
            &views[0],
            group_id(9),
            group_id(3),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(240.0, 80.0),
        ),
        Err(LayoutError::GroupNotFound)
    );
    assert_eq!(
        layout.split_view(
            &missing,
            group_id(2),
            group_id(3),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(240.0, 80.0),
        ),
        Err(LayoutError::ViewNotFound)
    );
    assert_eq!(
        layout.split_view(
            &views[0],
            group_id(2),
            group_id(1),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(240.0, 80.0),
        ),
        Err(LayoutError::DuplicateGroup)
    );
    assert_eq!(
        layout.split_view(
            &views[1],
            group_id(2),
            group_id(3),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(242.0, 80.0),
        ),
        Err(LayoutError::InsufficientSpace)
    );
    assert_eq!(layout, original);

    let project_id = project(1);
    let only = view(project_id, 1);
    let group = PanelGroup::new(vec![only.clone()], only.clone()).unwrap();
    let original = ProjectLayout::new(project_id, group_id(1), group).unwrap();
    let mut layout = original.clone();
    assert_eq!(
        layout.split_view(
            &only,
            group_id(1),
            group_id(2),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(240.0, 80.0),
        ),
        Err(LayoutError::SplitWouldEmptySource)
    );
    assert_eq!(layout, original);
}

#[test]
fn project_layout_split_preserves_existing_and_new_views() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    let first_group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let second_group = PanelGroup::new(vec![second.clone()], second.clone()).unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), first_group).unwrap();

    layout
        .split(
            group_id(1),
            group_id(2),
            second_group,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(242.0, 80.0),
        )
        .unwrap();

    assert_eq!(layout.group_ids(), [group_id(1), group_id(2)]);
    assert_eq!(layout.view_keys(), [first, second]);
}

#[test]
fn project_layout_split_rejects_conflicts_without_mutation() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let first_group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let original = ProjectLayout::new(project_id, group_id(1), first_group).unwrap();
    let split_with = |layout: &mut ProjectLayout, source: u8, target: u8, view_key: ViewKey| {
        let group = PanelGroup::new(vec![view_key.clone()], view_key).unwrap();
        layout.split(
            group_id(source),
            group_id(target),
            group,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(242.0, 80.0),
        )
    };

    for (source, target, content, expected) in [
        (1, 2, first.clone(), LayoutError::DuplicateView),
        (1, 1, view(project_id, 2), LayoutError::DuplicateGroup),
        (9, 2, view(project_id, 2), LayoutError::GroupNotFound),
        (1, 2, view(project(2), 2), LayoutError::ProjectMismatch),
        (
            1,
            2,
            ViewKey::AppSettingsOverlay,
            LayoutError::ProjectMismatch,
        ),
    ] {
        let mut layout = original.clone();
        assert_eq!(
            split_with(&mut layout, source, target, content),
            Err(expected)
        );
        assert_eq!(layout, original);
    }

    let mut undersized = original.clone();
    let group = PanelGroup::new(vec![view(project_id, 2)], view(project_id, 2)).unwrap();
    assert_eq!(
        undersized.split(
            group_id(1),
            group_id(2),
            group,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(241.0, 80.0),
        ),
        Err(LayoutError::InsufficientSpace)
    );
    assert_eq!(undersized, original);
}

#[test]
fn project_layout_split_rejects_invalid_geometry() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let first_group = PanelGroup::new(vec![first.clone()], first).unwrap();
    let original = ProjectLayout::new(project_id, group_id(1), first_group).unwrap();

    for (ratio, size) in [
        (0.0, LogicalSize::new(242.0, 80.0)),
        (1.0, LogicalSize::new(242.0, 80.0)),
        (f32::NAN, LogicalSize::new(242.0, 80.0)),
        (0.5, LogicalSize::new(f32::NAN, 80.0)),
        (0.5, LogicalSize::new(240.0, f32::INFINITY)),
        (0.5, LogicalSize::new(-240.0, 80.0)),
    ] {
        let mut layout = original.clone();
        let second = view(project_id, 2);
        let group = PanelGroup::new(vec![second.clone()], second).unwrap();
        assert_eq!(
            layout.split(
                group_id(1),
                group_id(2),
                group,
                SplitDirection::LeftRight,
                ratio,
                size,
            ),
            Err(LayoutError::InvalidGeometry)
        );
        assert_eq!(layout, original);
    }
}

#[test]
fn project_layout_split_accounts_for_divider_at_minimum_boundary() {
    let project_id = project(1);
    for (direction, rejected, accepted) in [
        (
            SplitDirection::LeftRight,
            LogicalSize::new(240.0, 80.0),
            LogicalSize::new(242.0, 80.0),
        ),
        (
            SplitDirection::TopBottom,
            LogicalSize::new(120.0, 160.0),
            LogicalSize::new(120.0, 162.0),
        ),
    ] {
        let first = view(project_id, 1);
        let first_group = PanelGroup::new(vec![first.clone()], first).unwrap();
        let original = ProjectLayout::new(project_id, group_id(1), first_group).unwrap();
        let second = view(project_id, 2);
        let second_group = PanelGroup::new(vec![second.clone()], second).unwrap();
        let mut layout = original.clone();
        assert_eq!(
            layout.split(
                group_id(1),
                group_id(2),
                second_group.clone(),
                direction,
                0.5,
                rejected,
            ),
            Err(LayoutError::InsufficientSpace)
        );
        assert_eq!(layout, original);
        layout
            .split(
                group_id(1),
                group_id(2),
                second_group,
                direction,
                0.5,
                accepted,
            )
            .unwrap();
    }
}

#[test]
fn project_layout_resize_accounts_for_divider_at_minimum_boundary() {
    let (mut layout, _) = two_group_layout();
    let original = layout.clone();

    assert_eq!(
        layout.resize_split(&[], 0.5, LogicalSize::new(240.0, 80.0)),
        Err(LayoutError::InsufficientSpace)
    );
    assert_eq!(layout, original);
}

#[test]
fn project_layout_split_view_uses_space_reclaimed_by_empty_source() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    let first_group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
    let mut layout = ProjectLayout::new(project_id, group_id(1), first_group).unwrap();
    let second_group = PanelGroup::new(vec![second.clone()], second.clone()).unwrap();
    layout
        .split(
            group_id(1),
            group_id(2),
            second_group,
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(400.0, 80.0),
        )
        .unwrap();

    layout
        .split_view(
            &first,
            group_id(2),
            group_id(3),
            SplitDirection::LeftRight,
            0.5,
            LogicalSize::new(400.0, 80.0),
        )
        .unwrap();

    assert_eq!(layout.group_ids(), [group_id(2), group_id(3)]);
    assert_group(&layout, group_id(2), std::slice::from_ref(&second), &second);
    assert_group(&layout, group_id(3), std::slice::from_ref(&first), &first);
}

#[test]
fn project_layout_dto_round_trips_canonical_json_without_panel_identity() {
    let empty = ProjectLayout::empty(project(1));
    let empty_json = format!(
        r#"{{"groups":[],"kind":"project_layout","project_id":"{}","tree":null,"version":1}}"#,
        project(1)
    );
    assert_eq!(empty.to_canonical_json().unwrap(), empty_json);
    assert_eq!(
        ProjectLayout::from_canonical_json(&empty_json).unwrap(),
        empty
    );
    let reordered = format!(
        r#"{{"version":1,"tree":null,"project_id":"{}","kind":"project_layout","groups":[]}}"#,
        project(1)
    );
    assert_eq!(
        ProjectLayout::from_canonical_json(&reordered)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        empty_json
    );

    for (layout, json) in [single_layout_fixture(), two_group_dto_fixture()] {
        assert_eq!(layout.to_canonical_json().unwrap(), json);
        let restored = ProjectLayout::from_canonical_json(&json).unwrap();
        assert_eq!(restored, layout);
        assert_eq!(restored.to_canonical_json().unwrap(), json);
        assert!(!json.contains("panel_id"));
        assert!(!json.contains("instance_epoch"));
        assert!(!json.contains("focus"));
    }
}

#[test]
fn project_layout_dto_distinguishes_envelope_failures() {
    for text in [
        r#"{"kind":"future_layout","version":1}"#,
        r#"{"kind":"future_layout","version":1,"anything":null}"#,
    ] {
        assert_eq!(
            ProjectLayout::from_canonical_json(text),
            Err(LayoutDtoError::UnknownKind)
        );
    }
    for version in [0, 2, u64::MAX] {
        let text = format!(r#"{{"kind":"project_layout","version":{version}}}"#);
        assert_eq!(
            ProjectLayout::from_canonical_json(&text),
            Err(LayoutDtoError::UnsupportedVersion)
        );
    }
    let (_, valid) = single_layout_fixture();
    let extra = format!("{},\"extra\":null}}", &valid[..valid.len() - 1]);
    for text in [
        "{".to_owned(),
        r#"{"kind":"project_layout","version":1}"#.to_owned(),
        extra,
    ] {
        assert_eq!(
            ProjectLayout::from_canonical_json(&text),
            Err(LayoutDtoError::InvalidStructure),
            "{text}"
        );
    }
}

#[test]
fn project_layout_dto_rejects_invalid_domain_references() {
    let (_, json) = two_group_dto_fixture();
    let base: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut cases = Vec::new();

    let mut missing_group = base.clone();
    missing_group["groups"].as_array_mut().unwrap().remove(1);
    cases.push((missing_group, LayoutError::TreeGroupMismatch));

    let mut orphan_tree = base.clone();
    orphan_tree["tree"] = serde_json::Value::Null;
    cases.push((orphan_tree, LayoutError::TreeGroupMismatch));

    let mut duplicate_tree_group = base.clone();
    duplicate_tree_group["tree"]["second"]["group_id"] =
        serde_json::Value::String(group_id(1).to_string());
    cases.push((duplicate_tree_group, LayoutError::DuplicateGroup));

    let mut duplicate_group_record = base.clone();
    duplicate_group_record["groups"][1]["group_id"] =
        serde_json::Value::String(group_id(1).to_string());
    cases.push((duplicate_group_record, LayoutError::DuplicateGroup));

    let mut duplicate_view = base.clone();
    duplicate_view["groups"][1]["views"][1] = base["groups"][0]["views"][0].clone();
    cases.push((duplicate_view, LayoutError::DuplicateView));

    let mut foreign_view = base.clone();
    foreign_view["groups"][1]["views"][0]["project_id"] =
        serde_json::Value::String(project(2).to_string());
    foreign_view["groups"][1]["active"]["project_id"] =
        serde_json::Value::String(project(2).to_string());
    cases.push((foreign_view, LayoutError::ProjectMismatch));

    let mut empty_group = base.clone();
    empty_group["groups"][0]["views"] = serde_json::Value::Array(Vec::new());
    cases.push((empty_group, LayoutError::EmptyViews));

    let mut detached_active = base.clone();
    detached_active["groups"][0]["active"] =
        serde_json::from_str(&view(project(1), 9).to_canonical_json().unwrap()).unwrap();
    cases.push((detached_active, LayoutError::ActiveNotMember));

    let mut invalid_ratio = base;
    invalid_ratio["tree"]["ratio"] = serde_json::Value::from(0.0);
    cases.push((invalid_ratio, LayoutError::InvalidGeometry));

    for (value, expected) in cases {
        assert_eq!(
            ProjectLayout::from_canonical_json(&value.to_string()),
            Err(LayoutDtoError::InvalidLayout(expected))
        );
    }
}

#[test]
fn project_layout_dto_rejects_duplicate_fields_and_nested_view_failures() {
    let (_, valid) = single_layout_fixture();
    let duplicate_groups = valid.replacen(r#""groups":["#, r#""groups":[],"groups":["#, 1);
    let duplicate_view_kind = valid.replacen(
        r#""active":{"kind":"terminal""#,
        r#""active":{"kind":"editor","kind":"terminal""#,
        1,
    );
    assert_eq!(
        ProjectLayout::from_canonical_json(&duplicate_groups),
        Err(LayoutDtoError::InvalidStructure)
    );
    assert_eq!(
        ProjectLayout::from_canonical_json(&duplicate_view_kind),
        Err(LayoutDtoError::ViewKey(ViewKeyError::InvalidStructure))
    );

    let mut group_extra: serde_json::Value = serde_json::from_str(&valid).unwrap();
    group_extra["groups"][0]["extra"] = serde_json::Value::Null;
    let mut tree_extra: serde_json::Value = serde_json::from_str(&valid).unwrap();
    tree_extra["tree"]["extra"] = serde_json::Value::Null;
    let mut unknown_tree_kind: serde_json::Value = serde_json::from_str(&valid).unwrap();
    unknown_tree_kind["tree"]["kind"] = serde_json::Value::String("future".to_owned());
    for value in [group_extra, tree_extra, unknown_tree_kind] {
        assert_eq!(
            ProjectLayout::from_canonical_json(&value.to_string()),
            Err(LayoutDtoError::InvalidStructure)
        );
    }

    let unknown_view = valid.replacen(r#""kind":"terminal""#, r#""kind":"future_view""#, 1);
    assert_eq!(
        ProjectLayout::from_canonical_json(&unknown_view),
        Err(LayoutDtoError::ViewKey(ViewKeyError::UnknownKind))
    );
    let unknown_view_with_fields = unknown_view.replacen(
        r#""kind":"future_view""#,
        r#""kind":"future_view","future_field":null"#,
        1,
    );
    assert_eq!(
        ProjectLayout::from_canonical_json(&unknown_view_with_fields),
        Err(LayoutDtoError::ViewKey(ViewKeyError::UnknownKind))
    );
    let unsupported_view = valid.replacen(r#""version":1"#, r#""version":2"#, 1);
    assert_eq!(
        ProjectLayout::from_canonical_json(&unsupported_view),
        Err(LayoutDtoError::ViewKey(ViewKeyError::UnsupportedVersion))
    );
}

#[test]
fn panel_group_rejects_empty_duplicate_and_detached_active() {
    let project_id = project(1);
    let first = view(project_id, 1);
    let second = view(project_id, 2);
    assert_eq!(
        PanelGroup::new(Vec::new(), first.clone()),
        Err(LayoutError::EmptyViews)
    );
    assert_eq!(
        PanelGroup::new(vec![first.clone(), first.clone()], first.clone()),
        Err(LayoutError::DuplicateView)
    );
    assert_eq!(
        PanelGroup::new(vec![first], second),
        Err(LayoutError::ActiveNotMember)
    );
}

#[test]
fn panel_group_preserves_view_order_and_active_identity() {
    let project_id = project(1);
    let views = vec![view(project_id, 1), view(project_id, 2)];

    let group = PanelGroup::new(views.clone(), views[1].clone()).unwrap();

    assert_eq!(group.views(), views);
    assert_eq!(group.active(), &views[1]);
}

fn group_for_view(layout: &ProjectLayout, view: &ViewKey) -> PanelGroupId {
    layout
        .group_ids()
        .into_iter()
        .find(|group_id| layout.group(*group_id).unwrap().views().contains(view))
        .unwrap()
}

const PROPERTY_VIEWPORT: LogicalSize = LogicalSize::new(4096.0, 4096.0);

fn split_request(extra: u8) -> SplitDirection {
    if extra.is_multiple_of(2) {
        SplitDirection::LeftRight
    } else {
        SplitDirection::TopBottom
    }
}

fn check_layout(layout: &ProjectLayout, expected: &[ViewKey]) -> Result<(), TestCaseError> {
    let actual = layout.view_keys();
    prop_assert_eq!(actual.len(), expected.len());
    for (index, view) in actual.iter().enumerate() {
        prop_assert!(!actual[..index].contains(view));
    }
    for view in expected {
        prop_assert_eq!(
            actual.iter().filter(|candidate| *candidate == view).count(),
            1
        );
    }
    let group_ids = layout.group_ids();
    let unique_ids: BTreeSet<_> = group_ids.iter().copied().collect();
    prop_assert_eq!(unique_ids.len(), group_ids.len());
    for group_id in group_ids {
        let group = layout.group(group_id).unwrap();
        prop_assert!(!group.views().is_empty());
        prop_assert!(group.views().contains(group.active()));
    }
    if let Some(tree) = layout.tree() {
        let rendered = tree.layout(
            slterm_split::Rect::new(0.0, 0.0, PROPERTY_VIEWPORT.width, PROPERTY_VIEWPORT.height),
            1.0,
            1.0,
            slterm_split::DIVIDER_GAP,
            false,
        );
        prop_assert_eq!(rendered.panes.len(), layout.group_ids().len());
        for (_, rect) in rendered.panes {
            prop_assert!(rect.w >= 120.0 && rect.h >= 80.0);
            prop_assert!(rect.x >= 0.0 && rect.y >= 0.0);
            prop_assert!(rect.x + rect.w <= PROPERTY_VIEWPORT.width);
            prop_assert!(rect.y + rect.h <= PROPERTY_VIEWPORT.height);
        }
    }
    let json = layout.to_canonical_json().unwrap();
    let restored = ProjectLayout::from_canonical_json(&json).unwrap();
    prop_assert_eq!(&restored, layout);
    prop_assert_eq!(restored.to_canonical_json().unwrap(), json);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 随机布局操作不得重复或丢失 ViewKey，且每步都可规范往返。
    #[test]
    fn project_layout_properties_random_operations_preserve_views_and_dto(
        operations in prop::collection::vec(any::<(u8, u8, u8)>(), 1..48),
    ) {
        let project_id = project(1);
        let first = view(project_id, 0);
        let group = PanelGroup::new(vec![first.clone()], first.clone()).unwrap();
        let mut layout = ProjectLayout::new(project_id, group_id(0), group).unwrap();
        let mut expected = vec![first];
        let mut next = 1_u8;

        for (operation, selector, extra) in operations {
            match operation % 8 {
                0 => {
                    let key = view(project_id, next);
                    let group = PanelGroup::new(vec![key.clone()], key.clone()).unwrap();
                    let group_ids = layout.group_ids();
                    if group_ids.is_empty() {
                        layout.open_first_group(group_id(next), group).unwrap();
                    } else {
                        layout
                            .insert_view(group_ids[selector as usize % group_ids.len()], key.clone())
                            .unwrap();
                    }
                    expected.push(key);
                    next += 1;
                }
                1 if !layout.group_ids().is_empty() => {
                    let key = view(project_id, next);
                    let group = PanelGroup::new(vec![key.clone()], key.clone()).unwrap();
                    let group_ids = layout.group_ids();
                    let target = group_ids[selector as usize % group_ids.len()];
                    let direction = split_request(extra);
                    let before = layout.clone();
                    match layout.split(
                        target,
                        group_id(next),
                        group,
                        direction,
                        0.5,
                        PROPERTY_VIEWPORT,
                    ) {
                        Ok(()) => {
                            expected.push(key);
                            next += 1;
                        }
                        Err(LayoutError::InsufficientSpace) => {
                            prop_assert_eq!(&layout, &before);
                        }
                        Err(error) => panic!("随机拆分出现意外错误：{error:?}"),
                    }
                }
                2 if !layout.group_ids().is_empty() => {
                    let keys = layout.view_keys();
                    let key = keys[selector as usize % keys.len()].clone();
                    let group_ids = layout.group_ids();
                    let source = group_for_view(&layout, &key);
                    let mut target_index = extra as usize % group_ids.len();
                    if source == group_ids[target_index]
                        && layout.group(source).unwrap().views().len() == 1
                    {
                        if group_ids.len() == 1 {
                            check_layout(&layout, &expected)?;
                            continue;
                        }
                        target_index = (target_index + 1) % group_ids.len();
                    }
                    let direction = split_request(selector);
                    let before = layout.clone();
                    match layout.split_view(
                        &key,
                        group_ids[target_index],
                        group_id(next),
                        direction,
                        0.5,
                        PROPERTY_VIEWPORT,
                    ) {
                        Ok(()) => next += 1,
                        Err(LayoutError::InsufficientSpace) => {
                            prop_assert_eq!(&layout, &before);
                        }
                        Err(error) => panic!("随机视图拆分出现意外错误：{error:?}"),
                    }
                }
                3 if !layout.group_ids().is_empty() => {
                    let keys = layout.view_keys();
                    let key = keys[selector as usize % keys.len()].clone();
                    let source = group_for_view(&layout, &key);
                    let group_ids = layout.group_ids();
                    let target = group_ids[extra as usize % group_ids.len()];
                    let target_len = layout.group(target).unwrap().views().len();
                    let positions = if source == target { target_len } else { target_len + 1 };
                    let target_index = (selector as usize + extra as usize) % positions;
                    layout.move_view(&key, target, target_index).unwrap();
                }
                4 if layout.group_ids().len() >= 2 => {
                    let group_ids = layout.group_ids();
                    let source_index = selector as usize % group_ids.len();
                    let target_index = (source_index + 1 + extra as usize % (group_ids.len() - 1))
                        % group_ids.len();
                    layout
                        .merge_groups(group_ids[source_index], group_ids[target_index])
                        .unwrap();
                }
                5 if !layout.group_ids().is_empty() => {
                    let keys = layout.view_keys();
                    let key = keys[selector as usize % keys.len()].clone();
                    layout.close_view(&key).unwrap();
                    expected.retain(|candidate| candidate != &key);
                }
                6 if !layout.group_ids().is_empty() => {
                    let group_ids = layout.group_ids();
                    let group_id = group_ids[selector as usize % group_ids.len()];
                    let group = layout.group(group_id).unwrap();
                    let key = group.views()[extra as usize % group.views().len()].clone();
                    layout.activate_view(group_id, &key).unwrap();
                }
                7 => {
                    if let Some(tree) = layout.tree() {
                        let paths = tree
                            .layout(
                                slterm_split::Rect::new(
                                    0.0,
                                    0.0,
                                    PROPERTY_VIEWPORT.width,
                                    PROPERTY_VIEWPORT.height,
                                ),
                                1.0,
                                1.0,
                                slterm_split::DIVIDER_GAP,
                                false,
                            )
                            .dividers
                            .into_iter()
                            .map(|divider| divider.path)
                            .collect::<Vec<_>>();
                        if !paths.is_empty() {
                            let before = layout.clone();
                            let ratio = [0.25, 0.5, 0.75][extra as usize % 3];
                            let path = &paths[selector as usize % paths.len()];
                            match layout.resize_split(path, ratio, PROPERTY_VIEWPORT) {
                                Ok(()) => {}
                                Err(LayoutError::InsufficientSpace) => {
                                    prop_assert_eq!(&layout, &before);
                                }
                                Err(error) => panic!("随机尺寸调整出现意外错误：{error:?}"),
                            }
                        }
                    }
                }
                _ => {}
            }
            check_layout(&layout, &expected)?;
        }
    }
}
