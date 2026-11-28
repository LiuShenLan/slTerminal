//! 快捷键页：用户覆盖、额外绑定、解绑、冲突与坏值修复清单。

use slterm_settings::keys::{keybind_pairs_from_json, keybind_value};

use super::super::command_registry::{CommandId, CommandRegistry};
use super::super::keybind_config::KeybindConfig;

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(combo, action)| ((*combo).to_owned(), (*action).to_owned()))
        .collect()
}

/// 每个进入快捷键页的命令必须有稳定 slug 并能反解唯一身份。
#[test]
fn every_shortcut_command_has_a_unique_stable_slug() {
    let mut seen = std::collections::BTreeMap::new();
    for group in super::super::command_registry::ShortcutGroup::ALL {
        for definition in CommandRegistry::shortcut_definitions(group) {
            let slug = definition.id.slug();
            assert_eq!(
                CommandId::from_slug(slug),
                Some(definition.id),
                "slug 必须可反解: {slug}"
            );
            if definition.id.label_message().is_some() {
                match seen.get(slug) {
                    Some(existing) => {
                        assert_eq!(*existing, definition.id, "slug 不得跨命令重复: {slug}")
                    }
                    None => {
                        seen.insert(slug, definition.id);
                    }
                }
            }
        }
    }
}

/// 覆盖替换默认；额外绑定保留；解绑行让命令显示未绑定。
#[test]
fn keybind_config_applies_override_extra_and_unbind_rows() {
    let config = KeybindConfig::from_pairs(&pairs(&[
        ("ctrl-shift-f1", "command.open-settings"),
        ("ctrl-shift-f2", "command.open-settings"),
        ("unbound", "unbind:command.new-terminal"),
    ]));
    assert_eq!(
        config.effective_combos(CommandId::OpenSettings),
        ["ctrl-shift-f1".to_owned(), "ctrl-shift-f2".to_owned()]
    );
    assert!(config.is_unbound(CommandId::NewTerminal));
    assert!(config.effective_combos(CommandId::NewTerminal).is_empty());
    assert!(config.invalid().is_empty(), "{:?}", config.invalid());

    let value = keybind_value(&config.pairs());
    let round_trip = KeybindConfig::from_pairs(&keybind_pairs_from_json(&value));
    assert_eq!(round_trip.pairs(), config.pairs());
}

/// 未知命令与非法组合不生效，但必须留在坏值清单与持久对表中。
#[test]
fn keybind_config_keeps_invalid_rows_for_explicit_repair() {
    let config = KeybindConfig::from_pairs(&pairs(&[
        ("ctrl-shift-f1", "command.does-not-exist"),
        ("ctrl-", "command.open-settings"),
        ("ctrl-shift-f3", "command.open-settings"),
    ]));
    assert_eq!(config.invalid().len(), 2, "{:?}", config.invalid());
    assert_eq!(
        config.effective_combos(CommandId::OpenSettings),
        ["ctrl-shift-f3".to_owned()]
    );
    assert_eq!(config.pairs().len(), 3);
    let repaired = config.without_invalid_rows();
    assert_eq!(repaired.pairs().len(), 1);
    assert!(repaired.invalid().is_empty());
}

/// 同上下文冲突返回全部命令并按后绑定优先；终端保留 Ctrl+C 不可录制。
#[test]
fn keybind_config_reports_conflicts_and_reserved_combos() {
    let config = KeybindConfig::from_pairs(&pairs(&[
        ("ctrl-shift-f1", "command.open-settings"),
        ("ctrl-shift-f1", "command.new-terminal"),
    ]));
    let conflict = config.conflicts(CommandId::OpenSettings, "ctrl-shift-f1");
    assert!(
        conflict.contains(&CommandId::NewTerminal),
        "同全局上下文必须报告冲突: {conflict:?}"
    );
    assert!(
        config.reserved(CommandId::NewTerminal, "ctrl-c"),
        "终端上下文必须保留 Ctrl+C"
    );
    assert!(!config.reserved(CommandId::OpenSettings, "ctrl-shift-f1"));
}

use super::open_root;
use super::settings_pages::focus_workspace;
use gpui::{Modifiers, TestAppContext};

fn open_shortcuts_page(
    cx: &mut TestAppContext,
) -> (gpui::Entity<super::AppRoot>, gpui::VisualTestContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    super::settings_pages::open_settings(&mut window);
    super::draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-shortcuts").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    super::draw_frame(&mut window);
    (root, window)
}

fn click_row(window: &mut gpui::VisualTestContext, selector: &'static str) {
    let row = window.debug_bounds(selector).expect(selector);
    window.simulate_click(row.center(), Modifiers::default());
    super::draw_frame(window);
}

/// 录制替换默认键位后立即派发新组合，旧默认组合不再触发命令。
#[gpui::test]
fn shortcuts_page_records_override_and_dispatches_it(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.keybind_config
                .effective_combos(CommandId::OpenSettings),
            ["ctrl-shift-u".to_owned()]
        );
        let segment = root
            .settings
            .values()
            .external_value(slterm_settings::keys::KEYBIND_SEGMENT)
            .expect("keybind 段必须写入");
        assert_eq!(
            keybind_pairs_from_json(segment),
            [(
                "ctrl-shift-u".to_owned(),
                "command.open-settings".to_owned()
            )]
        );
    });
    window.simulate_keystrokes("escape");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "空查询下 Esc 必须关闭设置浮层"
    );
    window.simulate_keystrokes("ctrl-,");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "被覆盖的默认组合不得再打开设置"
    );
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "新组合必须立即派发"
    );
}

/// Esc 取消录制；保留键拒绝并继续录制。
#[gpui::test]
fn shortcuts_recording_cancels_on_escape_and_rejects_reserved_keys(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("escape");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.keybind_config.pairs().is_empty()),
        "Esc 不得形成绑定"
    );
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "Esc 被录制消费，不得关闭设置"
    );

    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("ctrl-c");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.keybind_config.pairs().is_empty()),
        "终端保留键不得形成绑定"
    );
    assert!(
        window
            .debug_bounds("settings-shortcut-recording-error")
            .is_some(),
        "保留键必须显示可见原因并继续录制"
    );
    assert!(
        cx.update(|cx| {
            cx.try_global::<super::super::keybind_config::KeybindRecording>()
                .and_then(|recording| recording.command)
                .is_some()
        }),
        "拒绝后必须仍在录制"
    );
}

/// 同上下文冲突允许保存但必须显示冲突命令。
#[gpui::test]
fn shortcuts_conflict_is_visible_after_recording(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("ctrl-shift-t");
    super::draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root
            .keybind_config
            .effective_combos(CommandId::OpenSettings)),
        ["ctrl-shift-t".to_owned()]
    );
    assert!(
        window.debug_bounds("settings-shortcut-conflict").is_some(),
        "冲突命令必须显示"
    );
}

/// Backspace 解绑进入未绑定；复位恢复默认组合。
#[gpui::test]
fn shortcuts_backspace_unbinds_and_reset_restores_default(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("backspace");
    super::draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(root.keybind_config.is_unbound(CommandId::OpenSettings));
        assert!(
            root.keybind_config
                .effective_combos(CommandId::OpenSettings)
                .is_empty()
        );
    });
    let reset = window
        .debug_bounds("settings-shortcut-reset-command.open-settings")
        .expect("已定制命令必须提供复位");
    window.simulate_click(reset.center(), Modifiers::default());
    super::draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(!root.keybind_config.is_customized(CommandId::OpenSettings));
        assert_eq!(
            root.keybind_config
                .effective_combos(CommandId::OpenSettings),
            ["ctrl-,".to_owned()]
        );
    });
}

/// “+” 在保留默认组合的前提下追加额外绑定。
#[gpui::test]
fn shortcuts_add_button_appends_an_extra_binding(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    let add = window
        .debug_bounds("settings-shortcut-add-command.open-settings")
        .expect("每行必须有额外绑定入口");
    window.simulate_click(add.center(), Modifiers::default());
    super::draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.keybind_config
                .effective_combos(CommandId::OpenSettings),
            ["ctrl-,", "ctrl-shift-u"]
                .iter()
                .map(|combo| (*combo).to_owned())
                .collect::<Vec<_>>()
        );
    });
}

/// 坏值进入可见修复清单；修复全部后不再残留。
#[gpui::test]
fn shortcuts_invalid_rows_are_visible_and_repairable(cx: &mut TestAppContext) {
    use super::super::keybind_config::KeybindConfig;
    let (root, mut window) = open_shortcuts_page(cx);
    let config = KeybindConfig::from_pairs(&[
        ("ctrl-".to_owned(), "command.open-settings".to_owned()),
        ("ctrl-shift-f1".to_owned(), "command.unknown".to_owned()),
    ]);
    root.update(cx, |root, cx| {
        root.apply_keybind_config(config, Vec::new(), None, cx);
    });
    super::draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-shortcut-repair").is_some(),
        "坏值修复清单必须可见"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.keybind_config.invalid().len()),
        2
    );
    let repair = window
        .debug_bounds("settings-shortcut-repair-all")
        .expect("修复全部必须可点击");
    window.simulate_click(repair.center(), Modifiers::default());
    super::draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert!(root.keybind_config.pairs().is_empty());
        assert!(root.keybind_config.invalid().is_empty());
    });
}

/// 切页卸载快捷键页后录制必须结束，普通键位恢复派发。
#[gpui::test]
fn shortcuts_recording_ends_when_page_unloads(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    let appearance = window
        .debug_bounds("settings-page-appearance")
        .expect("外观页导航必须存在");
    window.simulate_click(appearance.center(), Modifiers::default());
    super::draw_frame(&mut window);
    window.simulate_keystrokes("escape");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "切页后 Esc 必须恢复关闭设置，而不是继续被录制消费"
    );
    assert!(
        root.read_with(cx, |root, _| root.keybind_config.pairs().is_empty()),
        "切页不得形成绑定"
    );
}

/// 启动装载持久键位段后，新组合立即成为派发绑定。
#[gpui::test]
fn shortcuts_persisted_segment_is_loaded_at_startup(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    super::settings_pages::focus_workspace(&root, &mut window);
    let segment = keybind_value(&[(
        "ctrl-shift-u".to_owned(),
        "command.open-settings".to_owned(),
    )]);
    root.update(cx, |root, cx| {
        root.settings
            .adopt_imported(&[(slterm_settings::keys::KEYBIND_SEGMENT, segment.clone())]);
        root.initialize_keybind_config(cx);
    });
    super::draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "持久覆盖必须在启动装载后立即派发"
    );
}

/// 编辑器与文件树命令进入页面分组；补全键位只读展示且不可录制。
#[gpui::test]
fn shortcuts_page_lists_editor_files_and_fixed_completion_keys(cx: &mut TestAppContext) {
    let (_root, mut window) = open_shortcuts_page(cx);
    assert!(
        window
            .debug_bounds("settings-shortcut-group-editor")
            .is_some(),
        "编辑器分组必须渲染"
    );
    assert!(
        window
            .debug_bounds("settings-shortcut-item-command.editor-save")
            .is_some(),
        "编辑器保存必须进入目录"
    );
    assert!(
        window
            .debug_bounds("settings-shortcut-group-files")
            .is_some(),
        "文件浏览器分组必须渲染"
    );
    assert!(
        window
            .debug_bounds("settings-shortcut-item-command.explorer-rename")
            .is_some(),
        "文件树重命名必须进入目录"
    );
    assert!(
        window.debug_bounds("settings-shortcut-fixed-0").is_some(),
        "补全固定键必须只读展示"
    );
}

/// 同组合冲突的“后录制者优先”必须跨重启保持，而不是退回枚举顺序。
#[gpui::test]
fn shortcuts_conflict_priority_survives_restart(cx: &mut TestAppContext) {
    let (root, mut window) = open_shortcuts_page(cx);
    click_row(&mut window, "settings-shortcut-item-command.open-settings");
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    click_row(&mut window, "settings-shortcut-item-command.new-terminal");
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    window.simulate_keystrokes("escape");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "录制结束应已关闭设置"
    );
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "后录制的新建终端必须优先，不得打开设置"
    );

    // 模拟重启：以默认配置为旧基线，从当前持久段重新装载全部绑定
    root.update(cx, |root, cx| {
        root.keybind_config = KeybindConfig::default();
        root.initialize_keybind_config(cx);
    });
    super::draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-u");
    super::draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_none()),
        "重启后冲突优先级必须保持，不得翻转为先录制者"
    );
}
