use slterm_workspace::SideViewId;

use crate::{SideSlot, SidebarDtoError, SidebarState};

fn default_canonical_json() -> &'static str {
    concat!(
        r#"{"buttons":["#,
        r#"{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"upper","view":"explorer"},"#,
        r#"{"slot":"upper","view":"commit"},"#,
        r#"{"slot":"upper","view":"agent_files"}],"#,
        r#""hidden_buttons":[],"kind":"sidebar_state","#,
        r#""open_views":[{"slot":"upper","view":"nav"}],"#,
        r#""ratio":0.5,"version":1,"visible_intent":true,"width":250}"#
    )
}

fn custom_canonical_json() -> &'static str {
    concat!(
        r#"{"buttons":["#,
        r#"{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"upper","view":"commit"},"#,
        r#"{"slot":"upper","view":"agent_files"},"#,
        r#"{"slot":"lower","view":"explorer"}],"#,
        r#""hidden_buttons":["explorer","commit"],"kind":"sidebar_state","#,
        r#""open_views":[{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"lower","view":"explorer"}],"#,
        r#""ratio":0.25,"version":1,"visible_intent":false,"width":333}"#
    )
}

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

#[test]
fn sidebar_dto_serializes_stable_schema_order_and_persistent_fields() {
    assert_eq!(
        custom_state().to_canonical_json(),
        Ok(custom_canonical_json().to_owned())
    );
}

#[test]
fn sidebar_dto_decodes_then_replays_canonical_bytes() {
    for (text, expected) in [
        (default_canonical_json(), SidebarState::default()),
        (custom_canonical_json(), custom_state()),
    ] {
        let decoded = SidebarState::from_canonical_json(text);
        assert_eq!(decoded, Ok(expected), "{text}");
        assert_eq!(
            decoded.and_then(|state| state.to_canonical_json()),
            Ok(text.to_owned())
        );
    }
}

#[test]
fn sidebar_dto_distinguishes_envelope_and_structure_damage() {
    let missing_width = default_canonical_json().replace(r#","width":250}"#, "}");
    let missing_record_view = default_canonical_json().replacen(
        r#"{"slot":"upper","view":"nav"}"#,
        r#"{"slot":"upper"}"#,
        1,
    );
    let unknown_field = default_canonical_json().replacen('{', r#"{"extra":true,"#, 1);
    let duplicate_version = format!("{{\"version\":1,{}", &default_canonical_json()[1..]);
    let duplicate_width =
        default_canonical_json().replace(r#""width":250}"#, r#""width":250,"width":250}"#);
    let duplicate_record_field = default_canonical_json().replacen(
        r#"{"slot":"upper","view":"nav"}"#,
        r#"{"slot":"upper","slot":"lower","view":"nav"}"#,
        1,
    );
    let invalid_slot = default_canonical_json().replacen(
        r#"{"slot":"upper","view":"nav"}"#,
        r#"{"slot":"middle","view":"nav"}"#,
        1,
    );
    let wrong_field_type =
        default_canonical_json().replace(r#""visible_intent":true"#, r#""visible_intent":"true""#);
    let unsupported = default_canonical_json().replace(r#""version":1"#, r#""version":2"#);
    let unknown_kind =
        default_canonical_json().replace(r#""kind":"sidebar_state""#, r#""kind":"other""#);

    for (text, expected) in [
        ("{".to_owned(), SidebarDtoError::InvalidStructure),
        ("null".to_owned(), SidebarDtoError::InvalidStructure),
        (missing_width, SidebarDtoError::InvalidStructure),
        (missing_record_view, SidebarDtoError::InvalidStructure),
        (unknown_field, SidebarDtoError::InvalidStructure),
        (duplicate_version, SidebarDtoError::InvalidStructure),
        (duplicate_width, SidebarDtoError::InvalidStructure),
        (duplicate_record_field, SidebarDtoError::InvalidStructure),
        (invalid_slot, SidebarDtoError::InvalidStructure),
        (wrong_field_type, SidebarDtoError::InvalidStructure),
        (unsupported, SidebarDtoError::UnsupportedVersion),
        (unknown_kind, SidebarDtoError::UnknownKind),
    ] {
        assert_eq!(
            SidebarState::from_canonical_json(&text),
            Err(expected),
            "{text}"
        );
    }
}

#[test]
fn sidebar_dto_rejects_unknown_ids_in_every_view_field() {
    let unknown_button =
        default_canonical_json().replacen(r#""view":"nav""#, r#""view":"future_view""#, 1);
    let unknown_hidden = default_canonical_json().replace(
        r#""hidden_buttons":[]"#,
        r#""hidden_buttons":["future_view"]"#,
    );
    let unknown_open = default_canonical_json().replace(
        r#""open_views":[{"slot":"upper","view":"nav"}]"#,
        r#""open_views":[{"slot":"upper","view":"future_view"}]"#,
    );

    for text in [unknown_button, unknown_hidden, unknown_open] {
        assert_eq!(
            SidebarState::from_canonical_json(&text),
            Err(SidebarDtoError::UnknownView),
            "{text}"
        );
    }
}

#[test]
fn sidebar_dto_rejects_registry_slot_and_geometry_invariant_damage() {
    let duplicate_button = default_canonical_json().replacen(
        r#"{"slot":"upper","view":"nav"},"#,
        r#"{"slot":"upper","view":"nav"},{"slot":"upper","view":"nav"},"#,
        1,
    );
    let duplicate_and_missing_button = default_canonical_json().replacen(
        r#"{"slot":"upper","view":"explorer"}"#,
        r#"{"slot":"upper","view":"nav"}"#,
        1,
    );
    let missing_button =
        default_canonical_json().replace(r#",{"slot":"upper","view":"agent_files"}"#, "");
    let duplicate_hidden = default_canonical_json().replace(
        r#""hidden_buttons":[]"#,
        r#""hidden_buttons":["nav","nav"]"#,
    );
    let open_in_wrong_slot = custom_canonical_json().replace(
        r#""open_views":[{"slot":"upper","view":"nav"},{"slot":"lower","view":"explorer"}]"#,
        r#""open_views":[{"slot":"upper","view":"explorer"}]"#,
    );
    let multiple_open_in_one_slot = custom_canonical_json().replace(
        r#""open_views":[{"slot":"upper","view":"nav"},{"slot":"lower","view":"explorer"}]"#,
        r#""open_views":[{"slot":"upper","view":"nav"},{"slot":"upper","view":"explorer"}]"#,
    );
    let duplicate_open_view = custom_canonical_json().replace(
        r#""open_views":[{"slot":"upper","view":"nav"},{"slot":"lower","view":"explorer"}]"#,
        r#""open_views":[{"slot":"upper","view":"explorer"},{"slot":"lower","view":"explorer"}]"#,
    );
    let narrow = default_canonical_json().replace(r#""width":250"#, r#""width":159"#);
    let wide = default_canonical_json().replace(r#""width":250"#, r#""width":721"#);
    let zero_ratio = default_canonical_json().replace(r#""ratio":0.5"#, r#""ratio":0.0"#);
    let full_ratio = default_canonical_json().replace(r#""ratio":0.5"#, r#""ratio":1.0"#);

    for text in [
        duplicate_button,
        duplicate_and_missing_button,
        missing_button,
        duplicate_hidden,
        open_in_wrong_slot,
        multiple_open_in_one_slot,
        duplicate_open_view,
        narrow,
        wide,
        zero_ratio,
        full_ratio,
    ] {
        assert_eq!(
            SidebarState::from_canonical_json(&text),
            Err(SidebarDtoError::InvalidState),
            "{text}"
        );
    }
}

#[test]
fn sidebar_dto_errors_expose_stable_standard_error_messages() {
    fn require_error<T: std::error::Error>() {}
    require_error::<SidebarDtoError>();

    for (error, message) in [
        (
            SidebarDtoError::InvalidStructure,
            "侧栏组织状态持久结构非法",
        ),
        (SidebarDtoError::UnknownKind, "未知侧栏组织状态类型"),
        (
            SidebarDtoError::UnsupportedVersion,
            "不支持的侧栏组织状态 DTO 版本",
        ),
        (SidebarDtoError::UnknownView, "侧栏组织状态包含未知视图"),
        (SidebarDtoError::InvalidState, "侧栏组织状态违反领域不变量"),
        (SidebarDtoError::Encode, "侧栏组织状态规范 JSON 编码失败"),
    ] {
        assert_eq!(error.to_string(), message);
    }
}
