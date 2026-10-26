use slterm_workspace::SideViewId;

use crate::{ButtonMovement, SideSlot, SidebarError, SidebarState, SidebarTransition};

fn assert_transition(
    transition: &SidebarTransition,
    opened: &[SideViewId],
    closed: &[SideViewId],
    focus: Option<SideViewId>,
    was_visible: bool,
    is_visible: bool,
) {
    assert_eq!(transition.opened, opened);
    assert_eq!(transition.closed, closed);
    assert_eq!(transition.focus, focus);
    assert_eq!(transition.was_visible, was_visible);
    assert_eq!(transition.is_visible, is_visible);
    assert_eq!(transition.visibility_changed(), was_visible != is_visible);
}

fn state_with_two_open_views() -> SidebarState {
    let mut state = SidebarState::default();
    state
        .move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
        .unwrap();
    let transition = state.click_button(SideViewId::Explorer).unwrap();
    assert_transition(
        &transition,
        &[SideViewId::Explorer],
        &[],
        Some(SideViewId::Explorer),
        true,
        true,
    );
    state
}

#[test]
fn sidebar_defaults_match_first_use_contract() {
    let state = SidebarState::default();

    assert_eq!(state.buttons(SideSlot::Upper), SideViewId::ALL);
    assert_eq!(state.buttons(SideSlot::Lower), &[]);
    assert_eq!(state.visible_buttons(), SideViewId::ALL);
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
    assert_eq!(state.open_view(SideSlot::Lower), None);
    assert_eq!(state.open_slot(SideViewId::Nav), Some(SideSlot::Upper));
    assert_eq!(state.open_slot(SideViewId::Explorer), None);
    assert!(state.visible_intent());
    assert!(state.is_visible());
    assert_eq!(state.width(), 250);
    assert_eq!(state.ratio(), 0.5);
    for view in SideViewId::ALL {
        assert_eq!(state.button_slot(view), SideSlot::Upper);
        assert!(state.button_is_visible(view));
    }
}

#[test]
fn sidebar_close_and_explicit_hide_have_distinct_persistent_state() {
    let mut state = SidebarState::default();

    let closed = state.close_view(SideViewId::Nav);
    assert_transition(&closed, &[], &[SideViewId::Nav], None, true, false);
    assert!(state.visible_intent());
    assert_eq!(state.open_view(SideSlot::Upper), None);

    let reopened = state.click_button(SideViewId::Nav).unwrap();
    assert_transition(
        &reopened,
        &[SideViewId::Nav],
        &[],
        Some(SideViewId::Nav),
        false,
        true,
    );

    let hidden = state.toggle_sidebar();
    assert_transition(&hidden, &[], &[], None, true, false);
    assert!(!state.visible_intent());
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));

    let shown = state.toggle_sidebar();
    assert_transition(&shown, &[], &[], None, false, true);
    assert!(state.visible_intent());
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
}

#[test]
fn sidebar_button_click_replaces_visible_view_and_closes_open_view() {
    let mut state = SidebarState::default();

    let replaced = state.click_button(SideViewId::Explorer).unwrap();
    assert_transition(
        &replaced,
        &[SideViewId::Explorer],
        &[SideViewId::Nav],
        Some(SideViewId::Explorer),
        true,
        true,
    );
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Explorer));

    let closed = state.click_button(SideViewId::Explorer).unwrap();
    assert_transition(&closed, &[], &[SideViewId::Explorer], None, true, false);
    assert_eq!(state.open_view(SideSlot::Upper), None);

    let reopened = state.click_button(SideViewId::Explorer).unwrap();
    assert_transition(
        &reopened,
        &[SideViewId::Explorer],
        &[],
        Some(SideViewId::Explorer),
        false,
        true,
    );

    state.toggle_sidebar();
    let replaced_while_hidden = state.click_button(SideViewId::Nav).unwrap();
    assert_transition(
        &replaced_while_hidden,
        &[SideViewId::Nav],
        &[SideViewId::Explorer],
        Some(SideViewId::Nav),
        false,
        true,
    );
}

#[test]
fn sidebar_hidden_sidebar_button_click_reuses_open_instance() {
    let mut state = SidebarState::default();
    state.toggle_sidebar();

    let shown = state.click_button(SideViewId::Nav).unwrap();

    assert_transition(&shown, &[], &[], Some(SideViewId::Nav), false, true);
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
}

#[test]
fn sidebar_hiding_button_preserves_view_and_rejects_mouse_activation() {
    let mut state = SidebarState::default();
    state.set_button_hidden(SideViewId::Nav, true);
    let before_click = state.clone();

    assert_eq!(
        state.click_button(SideViewId::Nav),
        Err(SidebarError::HiddenButton)
    );
    assert_eq!(state, before_click);
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
    assert!(state.is_visible());
    assert!(!state.button_is_visible(SideViewId::Nav));
    assert!(!state.visible_buttons().contains(&SideViewId::Nav));

    state.set_button_hidden(SideViewId::Nav, false);
    assert!(state.button_is_visible(SideViewId::Nav));
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
}

#[test]
fn sidebar_shortcut_restores_then_opens_shows_focuses_or_closes() {
    let mut state = SidebarState::default();
    state.set_button_hidden(SideViewId::Nav, true);

    let restored_and_closed = state.activate_shortcut(SideViewId::Nav, true);
    assert!(state.button_is_visible(SideViewId::Nav));
    assert_transition(
        &restored_and_closed,
        &[],
        &[SideViewId::Nav],
        None,
        true,
        false,
    );

    let opened = state.activate_shortcut(SideViewId::Nav, false);
    assert_transition(
        &opened,
        &[SideViewId::Nav],
        &[],
        Some(SideViewId::Nav),
        false,
        true,
    );

    state.toggle_sidebar();
    let shown = state.activate_shortcut(SideViewId::Nav, false);
    assert_transition(&shown, &[], &[], Some(SideViewId::Nav), false, true);

    let focused = state.activate_shortcut(SideViewId::Nav, false);
    assert_transition(&focused, &[], &[], Some(SideViewId::Nav), true, true);

    let closed = state.activate_shortcut(SideViewId::Nav, true);
    assert_transition(&closed, &[], &[SideViewId::Nav], None, true, false);
}

#[test]
fn sidebar_shortcut_open_replaces_view_in_occupied_slot() {
    let mut state = SidebarState::default();

    let replaced = state.activate_shortcut(SideViewId::Explorer, false);

    assert_transition(
        &replaced,
        &[SideViewId::Explorer],
        &[SideViewId::Nav],
        Some(SideViewId::Explorer),
        true,
        true,
    );
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Explorer));
}

#[test]
fn sidebar_empty_toggle_uses_navigation_then_first_visible_button() {
    let mut navigation = SidebarState::default();
    navigation.close_view(SideViewId::Nav);
    let opened = navigation.toggle_sidebar();
    assert_transition(&opened, &[SideViewId::Nav], &[], None, false, true);
    assert_eq!(navigation.open_view(SideSlot::Upper), Some(SideViewId::Nav));

    let mut first_visible = SidebarState::default();
    first_visible.close_view(SideViewId::Nav);
    first_visible.set_button_hidden(SideViewId::Nav, true);
    let opened = first_visible.toggle_sidebar();
    assert_transition(&opened, &[SideViewId::Explorer], &[], None, false, true);
    assert_eq!(
        first_visible.open_view(SideSlot::Upper),
        Some(SideViewId::Explorer)
    );

    first_visible.close_view(SideViewId::Explorer);
    for view in SideViewId::ALL {
        first_visible.set_button_hidden(view, true);
    }
    let before = first_visible.clone();
    let unchanged = first_visible.toggle_sidebar();
    assert_transition(&unchanged, &[], &[], None, false, false);
    assert!(!first_visible.is_visible());
    assert_eq!(
        first_visible.buttons(SideSlot::Upper),
        before.buttons(SideSlot::Upper)
    );
    assert_eq!(first_visible.open_view(SideSlot::Upper), None);
}

#[test]
fn sidebar_reordering_preserves_groups_visibility_and_open_views() {
    let mut state = SidebarState::default();
    state
        .move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
        .unwrap();
    state.set_button_hidden(SideViewId::Commit, true);

    state.reorder_button(SideViewId::Nav, ButtonMovement::Down);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Commit, SideViewId::Nav, SideViewId::AgentFiles]
    );
    state.reorder_button(SideViewId::Nav, ButtonMovement::Bottom);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Commit, SideViewId::AgentFiles, SideViewId::Nav]
    );
    state.reorder_button(SideViewId::Nav, ButtonMovement::Top);
    state.reorder_button(SideViewId::Nav, ButtonMovement::Up);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Nav, SideViewId::Commit, SideViewId::AgentFiles]
    );

    state.reset_button_order();
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Nav, SideViewId::Commit, SideViewId::AgentFiles]
    );
    assert_eq!(state.buttons(SideSlot::Lower), [SideViewId::Explorer]);
    assert!(!state.button_is_visible(SideViewId::Commit));
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
    assert_eq!(state.open_view(SideSlot::Lower), None);

    state.reorder_button(SideViewId::AgentFiles, ButtonMovement::Down);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Nav, SideViewId::Commit, SideViewId::AgentFiles]
    );
}

#[test]
fn sidebar_same_group_move_uses_final_index_without_moving_views() {
    let mut state = SidebarState::default();

    let moved = state
        .move_button_to_slot(SideViewId::Nav, SideSlot::Upper, 2)
        .unwrap();

    assert_transition(&moved, &[], &[], None, true, true);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [
            SideViewId::Explorer,
            SideViewId::Commit,
            SideViewId::Nav,
            SideViewId::AgentFiles,
        ]
    );
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
}

#[test]
fn sidebar_moving_open_view_to_empty_slot_migrates_without_lifecycle_effects() {
    let mut state = SidebarState::default();

    let moved = state
        .move_button_to_slot(SideViewId::Nav, SideSlot::Lower, 0)
        .unwrap();

    assert_transition(&moved, &[], &[], None, true, true);
    assert_eq!(state.open_view(SideSlot::Upper), None);
    assert_eq!(state.open_view(SideSlot::Lower), Some(SideViewId::Nav));
    assert_eq!(state.open_slot(SideViewId::Nav), Some(SideSlot::Lower));
}

#[test]
fn sidebar_moving_open_view_to_occupied_slot_swaps_without_lifecycle_effects() {
    let mut state = state_with_two_open_views();
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Nav));
    assert_eq!(state.open_view(SideSlot::Lower), Some(SideViewId::Explorer));

    let moved = state
        .move_button_to_slot(SideViewId::Nav, SideSlot::Lower, 1)
        .unwrap();

    assert_transition(&moved, &[], &[], None, true, true);
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Explorer));
    assert_eq!(state.open_view(SideSlot::Lower), Some(SideViewId::Nav));
    assert_eq!(
        state.buttons(SideSlot::Lower),
        [SideViewId::Explorer, SideViewId::Nav]
    );
}

#[test]
fn sidebar_restore_defaults_keeps_first_registered_open_view_and_closes_conflicts() {
    let mut state = SidebarState::default();
    state.close_view(SideViewId::Nav);
    state
        .move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
        .unwrap();
    state.click_button(SideViewId::Explorer).unwrap();
    state.click_button(SideViewId::Commit).unwrap();
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Commit));
    assert_eq!(state.open_view(SideSlot::Lower), Some(SideViewId::Explorer));
    state.reorder_button(SideViewId::Nav, ButtonMovement::Bottom);
    state.set_button_hidden(SideViewId::AgentFiles, true);
    assert_eq!(
        state.buttons(SideSlot::Upper),
        [SideViewId::Commit, SideViewId::AgentFiles, SideViewId::Nav]
    );
    assert!(!state.button_is_visible(SideViewId::AgentFiles));
    state.toggle_sidebar();
    state.set_width(333);
    state.set_ratio(0.42).unwrap();

    let restored = state.restore_default_buttons();

    assert_transition(&restored, &[], &[SideViewId::Commit], None, false, false);
    assert_eq!(state.buttons(SideSlot::Upper), SideViewId::ALL);
    assert_eq!(state.buttons(SideSlot::Lower), &[]);
    assert_eq!(state.visible_buttons(), SideViewId::ALL);
    assert_eq!(state.open_view(SideSlot::Upper), Some(SideViewId::Explorer));
    assert_eq!(state.open_view(SideSlot::Lower), None);
    assert!(!state.visible_intent());
    assert_eq!(state.width(), 333);
    assert_eq!(state.ratio(), 0.42);
}

#[test]
fn sidebar_restore_defaults_with_empty_slots_does_not_create_view() {
    let mut state = SidebarState::default();
    state.close_view(SideViewId::Nav);
    state.set_width(333);
    state.set_ratio(0.42).unwrap();

    let restored = state.restore_default_buttons();

    assert_transition(&restored, &[], &[], None, false, false);
    assert_eq!(state.buttons(SideSlot::Upper), SideViewId::ALL);
    assert_eq!(state.buttons(SideSlot::Lower), &[]);
    assert_eq!(state.open_view(SideSlot::Upper), None);
    assert_eq!(state.open_view(SideSlot::Lower), None);
    assert!(state.visible_intent());
    assert_eq!(state.width(), 333);
    assert_eq!(state.ratio(), 0.42);
}

#[test]
fn sidebar_geometry_clamps_resets_and_rejects_non_finite_ratios() {
    let mut state = SidebarState::default();
    let before = state.clone();

    state.set_width(0);
    assert_eq!(state.width(), SidebarState::MIN_WIDTH);
    state.set_width(1000);
    assert_eq!(state.width(), SidebarState::MAX_WIDTH);
    state.adjust_width(-64);
    assert_eq!(state.width(), 656);
    state.adjust_width(i32::MAX);
    assert_eq!(state.width(), SidebarState::MAX_WIDTH);
    state.adjust_width(i32::MIN);
    assert_eq!(state.width(), SidebarState::MIN_WIDTH);
    state.reset_width();
    assert_eq!(state.width(), SidebarState::DEFAULT_WIDTH);

    state.set_ratio(0.0).unwrap();
    assert_eq!(state.ratio(), SidebarState::MIN_RATIO);
    state.set_ratio(1.0).unwrap();
    assert_eq!(state.ratio(), SidebarState::MAX_RATIO);
    state.adjust_ratio(-0.02).unwrap();
    assert!((state.ratio() - 0.88).abs() < 0.000_001);
    state.adjust_ratio(0.1).unwrap();
    assert_eq!(state.ratio(), SidebarState::MAX_RATIO);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(state.set_ratio(invalid), Err(SidebarError::InvalidRatio));
        assert_eq!(state.adjust_ratio(invalid), Err(SidebarError::InvalidRatio));
        assert_eq!(state.ratio(), SidebarState::MAX_RATIO);
    }
    state.reset_ratio();

    assert_eq!(state, before);
}

#[test]
fn sidebar_invalid_button_indices_leave_state_unchanged() {
    let mut state = SidebarState::default();
    let before = state.clone();

    assert_eq!(
        state.move_button_to_slot(SideViewId::Nav, SideSlot::Upper, 4),
        Err(SidebarError::InvalidButtonIndex)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.move_button_to_slot(SideViewId::Nav, SideSlot::Lower, 1),
        Err(SidebarError::InvalidButtonIndex)
    );
    assert_eq!(state, before);
}

#[test]
fn sidebar_errors_describe_rejected_requests() {
    assert_eq!(SidebarError::HiddenButton.to_string(), "侧栏按钮已隐藏");
    assert_eq!(
        SidebarError::InvalidButtonIndex.to_string(),
        "侧栏按钮目标索引非法"
    );
    assert_eq!(
        SidebarError::InvalidRatio.to_string(),
        "侧栏分割比例不是有限数值"
    );
}
