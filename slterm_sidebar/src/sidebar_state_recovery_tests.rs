use slterm_workspace::SideViewId;

use crate::{
    SideSlot, SidebarRecovery, SidebarRecoveryError, SidebarRecoveryWarning, SidebarState,
};

const CUSTOM_HIDDEN: &str = r#""hidden_buttons":["explorer","commit"]"#;
const CUSTOM_OPEN: &str =
    r#""open_views":[{"slot":"upper","view":"nav"},{"slot":"lower","view":"explorer"}]"#;
const DEFAULT_OPEN: &str = r#""open_views":[{"slot":"upper","view":"nav"}]"#;

fn custom_state() -> SidebarState {
    let mut state = SidebarState::default();
    state
        .move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
        .unwrap();
    state.click_button(SideViewId::Explorer).unwrap();
    state.set_button_hidden(SideViewId::Explorer, true);
    state.set_button_hidden(SideViewId::Commit, true);
    state.set_width(333);
    state.set_ratio(0.25).unwrap();
    state.toggle_sidebar();
    state
}

fn custom_json() -> String {
    custom_state().to_canonical_json().unwrap()
}

fn default_json() -> String {
    SidebarState::default().to_canonical_json().unwrap()
}

fn replace_buttons(json: &str, replacement: &str) -> String {
    let (_, tail) = json
        .split_once("],\"hidden_buttons\"")
        .expect("规范 JSON 必须包含按钮字段");
    format!("{{\"buttons\":{replacement},\"hidden_buttons\"{tail}")
}

fn recovered(state: SidebarState, warnings: &[SidebarRecoveryWarning]) -> SidebarRecovery {
    SidebarRecovery {
        state,
        warnings: warnings.to_vec(),
    }
}

#[test]
fn sidebar_recovery_valid_state_is_silent() {
    assert_eq!(
        SidebarRecovery::from_json(&custom_json()),
        Ok(recovered(custom_state(), &[]))
    );
}

#[test]
fn sidebar_recovery_distinguishes_fatal_structure_and_envelope_damage() {
    let unknown_top = default_json().replacen('{', r#"{"extra":true,"#, 1);
    let duplicate_nested_field = default_json().replacen(
        r#"{"slot":"upper","view":"nav"}"#,
        r#"{"slot":"upper","slot":"lower","view":"nav"}"#,
        1,
    );
    let duplicate_version = format!("{{\"version\":1,{}", &default_json()[1..]);
    let unsupported = default_json().replace(r#""version":1"#, r#""version":2"#);
    let unsupported_with_unknown = unknown_top.replace(r#""version":1"#, r#""version":2"#);
    let unknown_kind = default_json().replace(r#""kind":"sidebar_state""#, r#""kind":"other""#);

    for (text, expected) in [
        ("{".to_owned(), SidebarRecoveryError::InvalidStructure),
        (unknown_top, SidebarRecoveryError::InvalidStructure),
        (
            duplicate_nested_field,
            SidebarRecoveryError::InvalidStructure,
        ),
        (duplicate_version, SidebarRecoveryError::InvalidStructure),
        (unsupported, SidebarRecoveryError::UnsupportedVersion),
        (
            unsupported_with_unknown,
            SidebarRecoveryError::UnsupportedVersion,
        ),
        (unknown_kind, SidebarRecoveryError::UnknownKind),
    ] {
        assert_eq!(SidebarRecovery::from_json(&text), Err(expected), "{text}");
    }
}

#[test]
fn sideview_restore_normalizes_invalid_layout() {
    let buttons = concat!(
        r#"["#,
        r#"{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"middle","view":"explorer"},"#,
        r#"{"slot":"upper","view":"future_view"},"#,
        r#"{"slot":"lower","view":"commit"},"#,
        r#"{"slot":"lower","view":"nav"},"#,
        r#"{"slot":"upper","view":"nav","extra":0},"#,
        r#"{"slot":"upper"}"#,
        "]"
    );
    let open_views = concat!(
        r#""open_views":["#,
        r#"{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"upper","view":"explorer"},"#,
        r#"{"slot":"lower","view":"future_view"},"#,
        r#"{"slot":"lower"}"#,
        "]"
    );
    let json = replace_buttons(&custom_json(), buttons)
        .replace(
            CUSTOM_HIDDEN,
            r#""hidden_buttons":["nav","nav","future_view",7]"#,
        )
        .replace(CUSTOM_OPEN, open_views)
        .replace(r#""width":333"#, r#""width":100"#)
        .replace(r#""ratio":0.25"#, r#""ratio":0.95"#);
    let mut expected = SidebarState::default();
    expected
        .move_button_to_slot(SideViewId::Commit, SideSlot::Lower, 0)
        .unwrap();
    expected.set_button_hidden(SideViewId::Nav, true);
    expected.toggle_sidebar();

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            expected,
            &[
                SidebarRecoveryWarning::UnknownView,
                SidebarRecoveryWarning::DuplicateView,
                SidebarRecoveryWarning::ConflictingOpenView,
                SidebarRecoveryWarning::InvalidButtons,
                SidebarRecoveryWarning::InvalidHiddenButtons,
                SidebarRecoveryWarning::InvalidOpenViews,
                SidebarRecoveryWarning::InvalidWidth,
                SidebarRecoveryWarning::InvalidRatio,
            ],
        ))
    );
}

#[test]
fn sidebar_recovery_merges_missing_registered_views_without_damage_warning() {
    let json = replace_buttons(&default_json(), r#"[{"slot":"lower","view":"commit"}]"#).replace(
        DEFAULT_OPEN,
        r#""open_views":[{"slot":"lower","view":"commit"}]"#,
    );
    let mut expected = SidebarState::default();
    expected
        .move_button_to_slot(SideViewId::Commit, SideSlot::Lower, 0)
        .unwrap();
    expected.close_view(SideViewId::Nav);
    expected.click_button(SideViewId::Commit).unwrap();

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(expected, &[]))
    );
}

#[test]
fn sidebar_recovery_falls_back_only_invalid_fields() {
    let json = custom_json();
    let invalid_width = json.replace(r#""width":333"#, r#""width":"wide""#);
    let invalid_ratio = json.replace(r#""ratio":0.25"#, r#""ratio":0.0"#);
    let invalid_visible = json.replace(r#""visible_intent":false"#, r#""visible_intent":"hidden""#);
    let invalid_hidden = json.replace(CUSTOM_HIDDEN, r#""hidden_buttons":{}"#);
    let invalid_open = json.replace(CUSTOM_OPEN, r#""open_views":{}"#);
    let invalid_buttons = replace_buttons(&json, "{}");

    let mut width_expected = custom_state();
    width_expected.reset_width();
    let mut ratio_expected = custom_state();
    ratio_expected.reset_ratio();
    let mut visible_expected = custom_state();
    visible_expected.toggle_sidebar();
    let mut hidden_expected = custom_state();
    hidden_expected.set_button_hidden(SideViewId::Explorer, false);
    hidden_expected.set_button_hidden(SideViewId::Commit, false);
    let mut open_expected = custom_state();
    open_expected.close_view(SideViewId::Explorer);
    let mut buttons_expected = SidebarState::default();
    buttons_expected.set_button_hidden(SideViewId::Explorer, true);
    buttons_expected.set_button_hidden(SideViewId::Commit, true);
    buttons_expected.set_width(333);
    buttons_expected.set_ratio(0.25).unwrap();
    buttons_expected.toggle_sidebar();

    for (text, expected, warnings) in [
        (
            invalid_width,
            width_expected,
            vec![SidebarRecoveryWarning::InvalidWidth],
        ),
        (
            invalid_ratio,
            ratio_expected,
            vec![SidebarRecoveryWarning::InvalidRatio],
        ),
        (
            invalid_visible,
            visible_expected,
            vec![SidebarRecoveryWarning::InvalidVisibleIntent],
        ),
        (
            invalid_hidden,
            hidden_expected,
            vec![SidebarRecoveryWarning::InvalidHiddenButtons],
        ),
        (
            invalid_open,
            open_expected,
            vec![SidebarRecoveryWarning::InvalidOpenViews],
        ),
        (
            invalid_buttons,
            buttons_expected,
            vec![
                SidebarRecoveryWarning::ConflictingOpenView,
                SidebarRecoveryWarning::InvalidButtons,
                SidebarRecoveryWarning::InvalidOpenViews,
            ],
        ),
    ] {
        assert_eq!(
            SidebarRecovery::from_json(&text),
            Ok(SidebarRecovery {
                state: expected,
                warnings,
            }),
            "{text}"
        );
    }
}

#[test]
fn sidebar_recovery_open_view_follows_normalized_button_slot() {
    let json = custom_json().replace(
        CUSTOM_OPEN,
        r#""open_views":[{"slot":"upper","view":"explorer"}]"#,
    );
    let mut expected = custom_state();
    expected.close_view(SideViewId::Nav);

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            expected,
            &[SidebarRecoveryWarning::InvalidOpenViews],
        ))
    );
}

#[test]
fn sidebar_recovery_filters_and_deduplicates_hidden_buttons() {
    let json = default_json().replace(
        r#""hidden_buttons":[]"#,
        r#""hidden_buttons":["explorer","explorer","future_view"]"#,
    );
    let mut expected = SidebarState::default();
    expected.set_button_hidden(SideViewId::Explorer, true);

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            expected,
            &[
                SidebarRecoveryWarning::UnknownView,
                SidebarRecoveryWarning::DuplicateView,
            ],
        ))
    );
}

#[test]
fn sidebar_recovery_filters_and_deduplicates_open_views() {
    let json = default_json().replace(
        DEFAULT_OPEN,
        concat!(
            r#""open_views":["#,
            r#"{"slot":"upper","view":"explorer"},"#,
            r#"{"slot":"upper","view":"explorer"},"#,
            r#"{"slot":"upper","view":"future_view"}"#,
            "]"
        ),
    );
    let mut expected = SidebarState::default();
    expected.close_view(SideViewId::Nav);
    expected.click_button(SideViewId::Explorer).unwrap();

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            expected,
            &[
                SidebarRecoveryWarning::UnknownView,
                SidebarRecoveryWarning::DuplicateView,
            ],
        ))
    );
}

#[test]
fn sidebar_recovery_invalid_button_slot_uses_default_slot() {
    let json = replace_buttons(
        &default_json(),
        concat!(
            r#"["#,
            r#"{"slot":"upper","view":"nav"},"#,
            r#"{"slot":"middle","view":"explorer"},"#,
            r#"{"slot":"upper","view":"commit"},"#,
            r#"{"slot":"upper","view":"agent_files"}"#,
            "]"
        ),
    );

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            SidebarState::default(),
            &[SidebarRecoveryWarning::InvalidButtons],
        ))
    );
}

#[test]
fn sidebar_recovery_default_open_fallback_uses_normalized_button_slot() {
    let mut expected = SidebarState::default();
    expected
        .move_button_to_slot(SideViewId::Nav, SideSlot::Lower, 0)
        .unwrap();
    let json = expected.to_canonical_json().unwrap().replace(
        r#""open_views":[{"slot":"lower","view":"nav"}]"#,
        r#""open_views":{}"#,
    );

    assert_eq!(
        SidebarRecovery::from_json(&json),
        Ok(recovered(
            expected,
            &[SidebarRecoveryWarning::InvalidOpenViews],
        ))
    );
}

#[test]
fn sidebar_recovery_errors_expose_stable_standard_error_messages() {
    fn require_error<T: std::error::Error>() {}
    require_error::<SidebarRecoveryError>();

    for (error, message) in [
        (
            SidebarRecoveryError::InvalidStructure,
            "侧栏恢复输入结构损坏",
        ),
        (SidebarRecoveryError::UnknownKind, "未知侧栏恢复类型"),
        (
            SidebarRecoveryError::UnsupportedVersion,
            "不支持的侧栏恢复 DTO 版本",
        ),
    ] {
        assert_eq!(error.to_string(), message);
    }
}
