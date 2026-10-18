//! 键位收口(07 篇 M8.5;spec 采纳点 29–32 JSON 载体化)。
//!
//! `settings.json` 的 `keybind` 域外段(M6.1 已登记 `KEYBIND_SEGMENT`)是
//! 用户绑定单源:元素 `{combo, action}`、行序即优先级、整表替换。本模块
//! 负责:
//! - 宽容解析/整表写回(纯函数 + 设置写通道);
//! - [`EDITABLE_ACTIONS`] = 设置面可重绑动作集中表(编辑器域 + 工作区域,
//!   跨域动作仍归各自篇,表内只收本仓真实存在的 GPUI action);
//! - [`STATIC_DEFAULT_COMBOS`] = 默认绑定镜像(恢复默认 / Unbind 精确收回);
//! - **保留键红线**:`Ctrl+C`(终端中断透传)与 `Ctrl+Shift+C`(复制)在
//!   [`RESERVED_ROWS`] 只读登记,用户表任何覆盖/改绑尝试被拒绝。
//!
//! 存储 combo 规范:`ctrl+shift+t` → GPUI 绑定串 `ctrl-shift-t`
//! ([`gpui_binding_combo`];digitN 折回数字、plus/minus 折回字符)。

use std::io;
use std::path::Path;

use gpui::{App, KeyBinding};
use slterm_settings::keys::KEYBIND_SEGMENT;

use super::super::file_editor::{SaveFile, ToggleSource};
use super::{
    CloseActivePane, CloseActiveTab, FocusPaneDown, FocusPaneLeft, FocusPaneRight, FocusPaneUp,
    NewTerminalTab, OpenSettings, SelectNextTab, SelectPreviousTab, SplitDown, SplitRight,
    ToggleSidebar, ToggleZoom,
};

/// 设置面可重绑动作(顺序 = 展示顺序;名称即 `keybind` 段 `action` 值)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EditableAction {
    pub name: &'static str,
    pub label_zh: &'static str,
    pub label_en: &'static str,
}

pub(super) const EDITABLE_ACTIONS: &[EditableAction] = &[
    // -- 编辑器 --
    EditableAction {
        name: "save_file",
        label_zh: "保存文件",
        label_en: "Save file",
    },
    EditableAction {
        name: "toggle_source",
        label_zh: "切换源码/预览",
        label_en: "Toggle source",
    },
    // -- 标签页 --
    EditableAction {
        name: "new_terminal_tab",
        label_zh: "新建标签页",
        label_en: "New tab",
    },
    EditableAction {
        name: "select_next_tab",
        label_zh: "下一个标签页",
        label_en: "Next tab",
    },
    EditableAction {
        name: "select_previous_tab",
        label_zh: "上一个标签页",
        label_en: "Previous tab",
    },
    EditableAction {
        name: "close_active_tab",
        label_zh: "关闭标签页",
        label_en: "Close tab",
    },
    // -- 窗格 --
    EditableAction {
        name: "split_right",
        label_zh: "左右分屏",
        label_en: "Split right",
    },
    EditableAction {
        name: "split_down",
        label_zh: "上下分屏",
        label_en: "Split down",
    },
    EditableAction {
        name: "toggle_zoom",
        label_zh: "放大当前分屏",
        label_en: "Zoom current pane",
    },
    EditableAction {
        name: "focus_pane_left",
        label_zh: "焦点：左侧分屏",
        label_en: "Focus pane left",
    },
    EditableAction {
        name: "focus_pane_right",
        label_zh: "焦点：右侧分屏",
        label_en: "Focus pane right",
    },
    EditableAction {
        name: "focus_pane_up",
        label_zh: "焦点：上方分屏",
        label_en: "Focus pane up",
    },
    EditableAction {
        name: "focus_pane_down",
        label_zh: "焦点：下方分屏",
        label_en: "Focus pane down",
    },
    EditableAction {
        name: "close_active_pane",
        label_zh: "关闭当前分屏",
        label_en: "Close pane",
    },
    // -- 界面 --
    EditableAction {
        name: "toggle_sidebar",
        label_zh: "侧栏开合",
        label_en: "Toggle sidebar",
    },
    EditableAction {
        name: "open_settings",
        label_zh: "打开设置",
        label_en: "Open settings",
    },
];

/// 保留键(只读展示;用户表不可绑定/覆盖)。
#[allow(dead_code)] // 消费点 = 设置面键位只读行(M8.5 落结构,编辑 UI 归后续)。
pub(super) const RESERVED_ROWS: &[(&str, &str, &str)] = &[
    (
        "interrupt",
        "中断（终端保留）",
        "Interrupt (terminal reserved)",
    ),
    ("copy", "复制（终端保留）", "Copy (terminal reserved)"),
];

/// 默认不绑键的可编辑动作(可重绑但启动无默认键;`CloseActiveTab` 与
/// 上游 ctrl-shift-w 的整 tab 关闭语义冲突,入口归菜单)。
#[allow(dead_code)] // 消费点 = 默认键镜像对账用例。
const UNBOUND_ACTIONS: &[&str] = &["close_active_tab"];

/// 保留 combo(存储格式;规范化后比较,别名/大小写不敏感)。
pub(super) const RESERVED_COMBOS: &[&str] = &["ctrl+c", "ctrl+shift+c"];

/// 默认绑定镜像(combo, action):「恢复默认」写回这张表,「Unbind」按
/// 同表精确收回;与 [`default_workspace_bindings`] 及 file_editor 默认键
/// 同源维护,测试锁定解析往返。
#[allow(dead_code)] // 消费点 = 恢复默认/Unbind 精确收回(M8.5 落语义,编辑 UI 归后续)。
pub(super) const STATIC_DEFAULT_COMBOS: &[(&str, &str)] = &[
    ("ctrl+shift+d", "split_right"),
    ("ctrl+shift+s", "split_down"),
    ("ctrl+shift+enter", "toggle_zoom"),
    ("ctrl+alt+left", "focus_pane_left"),
    ("ctrl+alt+right", "focus_pane_right"),
    ("ctrl+alt+up", "focus_pane_up"),
    ("ctrl+alt+down", "focus_pane_down"),
    ("ctrl+shift+w", "close_active_pane"),
    ("ctrl+shift+t", "new_terminal_tab"),
    ("ctrl+tab", "select_next_tab"),
    ("ctrl+shift+tab", "select_previous_tab"),
    ("ctrl+shift+b", "toggle_sidebar"),
    ("ctrl+,", "open_settings"),
    ("ctrl+s", "save_file"),
    ("ctrl+/", "toggle_source"),
];

/// 工作区静态默认绑定(05 篇 M5.3;键位照抄上游)。
///
/// `Ctrl+S` 只在 `FileEditor` 上下文注册(file_editor::init):无 context 的
/// 全局绑定会吞掉终端焦点下的 Ctrl+S(保留键红线;M8.5 收口)。
pub(super) fn default_workspace_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-shift-d", SplitRight, None),
        KeyBinding::new("ctrl-shift-s", SplitDown, None),
        KeyBinding::new("ctrl-shift-enter", ToggleZoom, None),
        KeyBinding::new("ctrl-alt-left", FocusPaneLeft, None),
        KeyBinding::new("ctrl-alt-right", FocusPaneRight, None),
        KeyBinding::new("ctrl-alt-up", FocusPaneUp, None),
        KeyBinding::new("ctrl-alt-down", FocusPaneDown, None),
        KeyBinding::new("ctrl-shift-w", CloseActivePane, None),
        KeyBinding::new("ctrl-shift-t", NewTerminalTab, None),
        KeyBinding::new("ctrl-tab", SelectNextTab, None),
        KeyBinding::new("ctrl-shift-tab", SelectPreviousTab, None),
        KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        // 07 篇编辑器动作:源码态保存/双模态切换只在文件编辑上下文生效。
        KeyBinding::new("ctrl-s", SaveFile, Some("FileEditor")),
        KeyBinding::new("ctrl-/", ToggleSource, Some("FileEditor")),
    ]
}

/// 存储 combo → GPUI 绑定串(`ctrl+shift+t` → `ctrl-shift-t`)。
///
/// digitN 折回数字、plus/minus 先占位再替换;最后由 GPUI 解析并规范化
/// 修饰键别名与顺序,保证旧写法/录制结果同一运行时身份。
pub(super) fn gpui_binding_combo(combo: &str) -> String {
    let combo = combo
        .to_ascii_lowercase()
        .replace("plus", "\u{1}")
        .replace("minus", "\u{2}")
        .replace('+', "-")
        .replace("digit", "")
        .replace('\u{1}', "+")
        .replace('\u{2}', "-");
    gpui::Keystroke::parse(&combo)
        .map(|key| key.unparse())
        .unwrap_or(combo)
}

/// 保留 combo 判定(规范化比较)。
pub(super) fn is_reserved_combo(combo: &str) -> bool {
    let normalized = gpui_binding_combo(combo);
    RESERVED_COMBOS
        .iter()
        .any(|reserved| gpui_binding_combo(reserved) == normalized)
}

/// 用户表准入:已知动作 + 非保留键 + GPUI 可解析;其余整行拒绝(notice 用)。
pub(super) struct KeybindAdmission {
    pub accepted: Vec<(String, String)>,
    pub rejected: Vec<(String, String)>,
}

pub(super) fn admit_keybind_pairs(pairs: Vec<(String, String)>) -> KeybindAdmission {
    let mut admission = KeybindAdmission {
        accepted: Vec::new(),
        rejected: Vec::new(),
    };
    for (combo, action) in pairs {
        let known = EDITABLE_ACTIONS.iter().any(|row| row.name == action)
            || matches!(action.as_str(), "none" | "unbound");
        let parseable = gpui::Keystroke::parse(&gpui_binding_combo(&combo)).is_ok();
        if known && parseable && !is_reserved_combo(&combo) {
            admission.accepted.push((combo, action));
        } else {
            admission.rejected.push((combo, action));
        }
    }
    admission
}

/// 动作的默认 combo 集(存储格式)。
#[allow(dead_code)] // 消费点 = clear_action/reset_action。
fn default_combos_for(action: &str) -> Vec<String> {
    STATIC_DEFAULT_COMBOS
        .iter()
        .filter(|(_, candidate)| *candidate == action)
        .map(|(combo, _)| (*combo).to_owned())
        .collect()
}

/// 解绑:撤掉该动作的自定义行,并把它的默认 combo 落成 `none`(精确收回,
/// 旧键透传终端;同键已被别的动作显式占用时不覆盖)。
#[allow(dead_code)] // 消费点 = 设置面「解绑」动作(M8.5 落语义,编辑 UI 归后续)。
pub(super) fn clear_action(pairs: &mut Vec<(String, String)>, action: &str) {
    pairs.retain(|(_, candidate)| candidate != action);
    for combo in default_combos_for(action) {
        let normalized = gpui_binding_combo(&combo);
        let claimed = pairs
            .iter()
            .rev()
            .find(|(candidate, _)| gpui_binding_combo(candidate) == normalized)
            .is_some_and(|(_, candidate)| candidate != "none" && candidate != "unbound");
        if claimed {
            continue;
        }
        pairs.retain(|(candidate, name)| {
            gpui_binding_combo(candidate) != normalized
                || !matches!(name.as_str(), "none" | "unbound")
        });
        pairs.push((combo, "none".to_owned()));
    }
}

/// 恢复默认:撤掉该动作的自定义行与该动作默认键的 `none` 标记。
#[allow(dead_code)] // 消费点 = 设置面「恢复默认」动作(M8.5 落语义,编辑 UI 归后续)。
pub(super) fn reset_action(pairs: &mut Vec<(String, String)>, action: &str) {
    let defaults = default_combos_for(action);
    pairs.retain(|(combo, name)| {
        name != action
            && !(matches!(name.as_str(), "none" | "unbound")
                && defaults
                    .iter()
                    .any(|default| gpui_binding_combo(default) == gpui_binding_combo(combo)))
    });
}

/// 应用用户绑定(启动路径;测试可直调注入 pairs)。
pub(super) fn bind_user_keybinds(cx: &mut App, pairs: &[(String, String)]) {
    let admission = admit_keybind_pairs(pairs.to_vec());
    for (combo, action) in admission.accepted {
        let combo = gpui_binding_combo(&combo);
        if gpui::Keystroke::parse(&combo).is_err() {
            continue;
        }
        match action.as_str() {
            "save_file" => cx.bind_keys([KeyBinding::new(&combo, SaveFile, Some("FileEditor"))]),
            "toggle_source" => {
                cx.bind_keys([KeyBinding::new(&combo, ToggleSource, Some("FileEditor"))])
            }
            "new_terminal_tab" => cx.bind_keys([KeyBinding::new(&combo, NewTerminalTab, None)]),
            "select_next_tab" => cx.bind_keys([KeyBinding::new(&combo, SelectNextTab, None)]),
            "select_previous_tab" => {
                cx.bind_keys([KeyBinding::new(&combo, SelectPreviousTab, None)])
            }
            "close_active_tab" => cx.bind_keys([KeyBinding::new(&combo, CloseActiveTab, None)]),
            "split_right" => cx.bind_keys([KeyBinding::new(&combo, SplitRight, None)]),
            "split_down" => cx.bind_keys([KeyBinding::new(&combo, SplitDown, None)]),
            "toggle_zoom" => cx.bind_keys([KeyBinding::new(&combo, ToggleZoom, None)]),
            "focus_pane_left" => cx.bind_keys([KeyBinding::new(&combo, FocusPaneLeft, None)]),
            "focus_pane_right" => cx.bind_keys([KeyBinding::new(&combo, FocusPaneRight, None)]),
            "focus_pane_up" => cx.bind_keys([KeyBinding::new(&combo, FocusPaneUp, None)]),
            "focus_pane_down" => cx.bind_keys([KeyBinding::new(&combo, FocusPaneDown, None)]),
            "close_active_pane" => cx.bind_keys([KeyBinding::new(&combo, CloseActivePane, None)]),
            "toggle_sidebar" => cx.bind_keys([KeyBinding::new(&combo, ToggleSidebar, None)]),
            "open_settings" => cx.bind_keys([KeyBinding::new(&combo, OpenSettings, None)]),
            "none" | "unbound" => cx.bind_keys([KeyBinding::new(&combo, gpui::NoAction, None)]),
            _ => {}
        }
    }
}

/// 生产启动读用户表;测试构建不读真实数据目录(与 `Settings::load` 隔离
/// 同纪律,用例经 [`bind_user_keybinds`] 注入)。
pub(super) fn install_user_keybinds(cx: &mut App) {
    if cfg!(all(test, feature = "gpui-test-support")) {
        return;
    }
    let pairs = load_keybind_pairs();
    bind_user_keybinds(cx, &pairs);
}

/// 读生产设置文件里的 `keybind` 段(损坏/缺失 = 空表,不炸启动)。
pub(crate) fn load_keybind_pairs() -> Vec<(String, String)> {
    load_keybind_pairs_at(&slterm_settings::settings_path())
}

/// 显式路径读入口(测试 tempdir)。
pub(crate) fn load_keybind_pairs_at(path: &Path) -> Vec<(String, String)> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let raw = slterm_settings::RawSettings::from_json_bytes(&bytes);
    raw.external_value(KEYBIND_SEGMENT)
        .map(slterm_settings::keys::keybind_pairs_from_json)
        .unwrap_or_default()
}

/// 整表写回生产设置文件(域外段替换,其余内容原样保留)。
#[allow(dead_code)] // 消费点 = 设置面键位编辑(M8.5 落段结构,编辑 UI 归后续)。
pub(crate) fn persist_keybinds(pairs: &[(String, String)]) -> io::Result<()> {
    persist_keybinds_at(&slterm_settings::settings_path(), pairs)
}

/// 显式路径写入口(测试 tempdir)。
pub(crate) fn persist_keybinds_at(path: &Path, pairs: &[(String, String)]) -> io::Result<()> {
    let value = slterm_settings::keys::keybind_value(pairs);
    crate::settings_store::save_settings_blocking_at(path, &[(KEYBIND_SEGMENT, value)])
}

#[cfg(test)]
mod keyboard_binding_tests {
    use super::*;

    /// 存储格式与 GPUI 绑定串往返:大小写/修饰键序/digitN/plus-minus。
    #[test]
    fn combo_normalization_round_trips() {
        for combo in [
            "ctrl+shift+t",
            "ctrl+tab",
            "alt+enter",
            "ctrl+alt+left",
            "f5",
            "ctrl+shift+digit1",
            "ctrl+plus",
            "ctrl+minus",
        ] {
            let gpui = gpui_binding_combo(combo);
            let parsed = gpui::Keystroke::parse(&gpui).unwrap_or_else(|_| panic!("{combo}"));
            assert!(!parsed.unparse().is_empty());
        }
        assert_eq!(gpui_binding_combo("CTRL+SHIFT+T"), "ctrl-shift-t");
        assert_eq!(gpui_binding_combo("ctrl+shift+digit1"), "ctrl-shift-1");
        assert_eq!(gpui_binding_combo("ctrl+plus"), "ctrl-+");
        assert_eq!(gpui_binding_combo("ctrl+minus"), "ctrl--");
    }

    /// 保留键红线:任何用户表条目都不能占用 Ctrl+C / Ctrl+Shift+C。
    #[test]
    fn reserved_terminal_combos_are_rejected() {
        assert!(is_reserved_combo("ctrl+c"));
        assert!(is_reserved_combo("CTRL+SHIFT+C"));
        assert!(!is_reserved_combo("ctrl+s"));
        let admission = admit_keybind_pairs(vec![
            ("ctrl+c".into(), "save_file".into()),
            ("ctrl+shift+c".into(), "save_file".into()),
            ("ctrl+s".into(), "save_file".into()),
            ("ctrl+.".into(), "not_an_action".into()),
        ]);
        assert_eq!(
            admission.accepted,
            [("ctrl+s".to_owned(), "save_file".to_owned())],
            "保留键与未知动作必须被拒绝"
        );
        assert_eq!(admission.rejected.len(), 3);
    }

    /// 恢复默认 vs Unbind 精确收回:默认键变 none,同键被别的动作占用时不覆盖。
    #[test]
    fn clear_and_reset_follow_exact_retraction_semantics() {
        let mut pairs = Vec::new();
        clear_action(&mut pairs, "split_right");
        assert_eq!(
            pairs,
            [("ctrl+shift+d".to_owned(), "none".to_owned())],
            "解绑必须把默认键精确收回"
        );
        reset_action(&mut pairs, "split_right");
        assert!(pairs.is_empty(), "恢复默认撤掉 none 标记");

        let mut claimed = vec![("ctrl+shift+d".to_owned(), "split_down".to_owned())];
        clear_action(&mut claimed, "split_right");
        assert_eq!(
            claimed,
            [("ctrl+shift+d".to_owned(), "split_down".to_owned())],
            "同键已被别的动作显式占用时不得覆盖"
        );
        reset_action(&mut claimed, "split_right");
        assert_eq!(claimed.len(), 1, "恢复默认不得误删别的动作");
    }

    /// 表完整性:名称唯一、默认 combo 全部可解析且至少有一个默认动作命中,
    /// 所有静态默认动作都在可编辑表内(目录单点、无影子绑定)。
    #[test]
    fn editable_actions_cover_every_static_default_action() {
        let mut names = std::collections::HashSet::new();
        for row in EDITABLE_ACTIONS {
            assert!(names.insert(row.name), "重复动作名: {}", row.name);
            assert!(!row.label_zh.is_empty() && !row.label_en.is_empty());
            assert!(
                STATIC_DEFAULT_COMBOS
                    .iter()
                    .any(|(_, action)| *action == row.name)
                    || UNBOUND_ACTIONS.contains(&row.name),
                "动作 {} 没有默认键镜像且未登记为默认不绑定",
                row.name
            );
        }
        for (combo, action) in STATIC_DEFAULT_COMBOS {
            assert!(
                gpui::Keystroke::parse(&gpui_binding_combo(combo)).is_ok(),
                "默认 combo {combo} 无法被 GPUI 解析"
            );
            assert!(
                EDITABLE_ACTIONS.iter().any(|row| row.name == *action),
                "默认动作 {action} 不在可编辑表内"
            );
            assert!(!is_reserved_combo(combo), "默认键不得与保留键冲突");
        }
        assert!(
            RESERVED_ROWS
                .iter()
                .all(|(_, zh, en)| !zh.is_empty() && !en.is_empty())
        );
    }

    /// 键位写通道:整表替换可被设置读取面读回,其余段保留。
    #[test]
    fn keybind_persist_round_trips_through_the_settings_channel() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let pairs = vec![
            ("ctrl+s".to_owned(), "save_file".to_owned()),
            ("ctrl+shift+p".to_owned(), "toggle_sidebar".to_owned()),
        ];
        persist_keybinds_at(&path, &pairs).unwrap();
        assert_eq!(load_keybind_pairs_at(&path), pairs);
        // 第二次写整表替换,旧行不残留。
        let next = vec![("ctrl+alt+r".to_owned(), "split_right".to_owned())];
        persist_keybinds_at(&path, &next).unwrap();
        assert_eq!(load_keybind_pairs_at(&path), next);
    }

    /// 缺失/损坏设置文件回空表(启动不炸)。
    #[test]
    fn missing_or_corrupt_settings_yield_no_bindings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        assert!(load_keybind_pairs_at(&path).is_empty());
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(load_keybind_pairs_at(&path).is_empty());
    }
}
