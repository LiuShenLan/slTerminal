//! 设置页路由:搜索是路由查找,不是页面过滤——查询命中即跳到拥有该
//! 控件的节。路由 id 的稳定集合来自 [`super::registry`];本模块只负责
//! 搜索索引拼接(节级别名 + 键注册表别名)与打分。
//!
//! 视觉顺序与路由 id 解耦:注册表 `order` 决定导航行顺序,id 只追加
//! 不重用,旧入口不会因重排指向另一页。

use gpui::SharedString;

use super::localization::UiLanguage;
use super::registry;

/// 可隐藏但不删位的路由下标;空 = 本阶段全部页面可见。
pub(crate) const HIDDEN_NAV_SECTIONS: &[usize] = &[];

/// 页面拥有的控件键(搜索别名拼接与页面归属断言的单一映射)。
/// 未在此列出的键(主题编辑/壁纸/卡几何等归后续阶段)不进搜索路由。
pub(crate) fn page_keys(page_id: &str) -> &'static [&'static str] {
    match page_id {
        "application" => &[],
        "appearance" => &[
            "theme",
            "font_family",
            "font_size",
            "ligatures",
            "opacity",
            "scrollback_lines",
            "scroll_speed",
            "density",
            "tab_reveal",
            "tab_close_visible",
            "cjk_bold_regular",
            "cell_width_mode",
        ],
        "profiles" => &[
            "startup_directory",
            "refresh_environment",
            "ghost",
            "completion_style",
            "accept",
            "ai_toasts",
            "notification_duration",
            "bell",
        ],
        "interaction" => &[
            "focus_follows_mouse",
            "copy_on_select",
            "multiline_paste_confirm",
            "panel_resize",
            "cursor_shape",
            "cursor_blink",
            "cursor_motion",
            "new_tab_position",
            "vcs_display",
            "dim_inactive_panes",
        ],
        "advanced" => &[
            "restore_session",
            "resume_ai",
            "keep_session",
            "silent_start",
            "launch_at_login",
            "tray",
            "auto_check_updates",
            "auto_download_updates",
            "fetch",
            "terminal_proxy",
        ],
        "localization" => &["language"],
        "agents" => &AGENT_PAGE_KEYS,
        _ => &[],
    }
}

/// Agent 组页拥有的控件键:全局授权键 + 9 家接入键(键名单源在
/// `AgentHook`,此处只做页面归属映射)。
static AGENT_PAGE_KEYS: [&str; 10] = [
    slterm_settings::AgentHook::GLOBAL_SETTINGS_KEY,
    slterm_settings::AgentHook::Claude.settings_key(),
    slterm_settings::AgentHook::Codex.settings_key(),
    slterm_settings::AgentHook::OpenCode.settings_key(),
    slterm_settings::AgentHook::Pi.settings_key(),
    slterm_settings::AgentHook::Copilot.settings_key(),
    slterm_settings::AgentHook::Grok.settings_key(),
    slterm_settings::AgentHook::OhMyPi.settings_key(),
    slterm_settings::AgentHook::Cursor.settings_key(),
    slterm_settings::AgentHook::Kimi.settings_key(),
];

/// 全部路由 id(注册表顺序 = 导航顺序)。
pub(crate) fn section_ids() -> Vec<&'static str> {
    registry::get_all().into_iter().map(|def| def.id).collect()
}

/// 页面标题(值/显示分离:路由 id 稳定,标题随语言刷新)。
pub(crate) fn section_label(index: usize, language: UiLanguage) -> SharedString {
    match registry::get_all().get(index) {
        Some(def) => language.text(def.title).into(),
        None => "".into(),
    }
}

/// 导航图标(按路由 id;未知 id 回通用设置图标)。
pub(crate) fn section_icon(index: usize) -> gpui_component::IconName {
    use gpui_component::IconName;
    match registry::get_all().get(index).map(|def| def.id) {
        Some("application") => IconName::Info,
        Some("appearance") => IconName::Palette,
        Some("profiles") => IconName::SquareTerminal,
        Some("interaction") => IconName::Globe,
        Some("advanced") => IconName::Settings2,
        Some("localization") => IconName::Globe,
        Some("reset") => IconName::Undo2,
        Some("agents") => IconName::Bot,
        _ => IconName::Settings,
    }
}

/// 可见导航下标(隐藏位不删,只过滤)。
pub(crate) fn visible_nav_sections() -> impl Iterator<Item = usize> {
    (0..registry::get_all().len()).filter(|index| !HIDDEN_NAV_SECTIONS.contains(index))
}

/// 单页搜索文本 = 双语标题 + 节级别名 + 该页控件的注册表别名串。
/// 标题从 typed 消息 id 取两语值,与显示文案同源不重复登记。
fn section_search_text(index: usize) -> String {
    let Some(def) = registry::get_all().get(index).copied() else {
        return String::new();
    };
    let mut text = format!(
        "{} {} {}",
        UiLanguage::ZhCn.text(def.title),
        UiLanguage::EnUs.text(def.title),
        def.search_terms
    );
    for key in page_keys(def.id) {
        if let Some(entry) = slterm_settings::keys::lookup(key) {
            text.push(' ');
            text.push_str(entry.search_terms);
        }
    }
    text.to_lowercase()
}

/// 查询 → 命中页面下标(得分降序,同分按路由顺序)。空查询 = 全部可见页。
/// 命中的是页面**是否拥有该控件**,从不拿查询去裁剪当前页内容。
pub(crate) fn matching_sections(query: &str, language: UiLanguage) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return visible_nav_sections().collect();
    }
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let mut matches = visible_nav_sections()
        .filter_map(|index| {
            let label = section_label(index, language).to_lowercase();
            let haystack = format!("{} {}", section_search_text(index), label);
            let words: Vec<&str> = haystack
                .split(|ch: char| !ch.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .collect();
            let mut score = 0usize;
            for token in &tokens {
                score = score.checked_add(score_token(token, &haystack, &words)?)?;
            }
            if haystack.contains(&query) {
                score += 1_500;
            }
            Some((index, score))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_index, left_score), (right_index, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_index.cmp(right_index))
    });
    matches.into_iter().map(|(index, _)| index).collect()
}

fn score_token(token: &str, haystack: &str, words: &[&str]) -> Option<usize> {
    if words.contains(&token) {
        return Some(1_000);
    }
    if words.iter().any(|word| word.starts_with(token)) {
        return Some(850);
    }
    if haystack.contains(token) {
        return Some(700);
    }
    if token.chars().count() >= 2
        && let Some(score) = words
            .iter()
            .filter_map(|word| subsequence_score(token, word))
            .max()
    {
        return Some(score);
    }
    if token.len() >= 4
        && token.is_ascii()
        && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        let allowed = if token.len() >= 8 { 2 } else { 1 };
        return words
            .iter()
            .filter(|word| word.is_ascii())
            .filter_map(|word| {
                let distance =
                    edit_distance_with_limit(token.as_bytes(), word.as_bytes(), allowed)?;
                Some(420usize.saturating_sub(distance * 60))
            })
            .max();
    }
    None
}

fn subsequence_score(needle: &str, word: &str) -> Option<usize> {
    let mut needle = needle.chars();
    let mut next = needle.next()?;
    let mut matched = 0usize;
    let mut span = 0usize;
    for ch in word.chars() {
        span += usize::from(matched > 0);
        if ch == next {
            matched += 1;
            if let Some(ch) = needle.next() {
                next = ch;
            } else {
                return Some(520usize.saturating_sub(span.saturating_sub(matched) * 8));
            }
        }
    }
    None
}

fn edit_distance_with_limit(left: &[u8], right: &[u8], limit: usize) -> Option<usize> {
    if left.len().abs_diff(right.len()) > limit {
        return None;
    }
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_byte) in left.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];
        for (right_index, right_byte) in right.iter().enumerate() {
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + usize::from(left_byte != right_byte));
            row_min = row_min.min(current[right_index + 1]);
        }
        if row_min > limit {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    (previous[right.len()] <= limit).then_some(previous[right.len()])
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use crate::app_shell::settings_pane::registry::_reset;

    fn bootstrap() {
        _reset();
        registry::register_builtin_pages();
    }

    fn index_of(id: &str) -> usize {
        registry::get_all()
            .iter()
            .position(|def| def.id == id)
            .unwrap_or_else(|| panic!("{id} 必须已注册"))
    }

    /// 页集合终表:全局组在上,Agent 组 hooks 页(inspect 联调)
    /// 在下;SSH/network 随 SSH 砍,providers/keymap/mobile/backup/about
    /// 不在本阶段。
    #[test]
    fn page_set_matches_the_reordered_global_group_then_agents() {
        bootstrap();
        assert_eq!(
            section_ids(),
            [
                "application",
                "appearance",
                "profiles",
                "interaction",
                "advanced",
                "localization",
                "reset",
                "agents",
            ]
        );
        for retired in [
            "ssh",
            "network",
            "providers",
            "keymap",
            "mobile",
            "backup",
            "about",
            "sponsor",
        ] {
            assert!(
                !section_ids().contains(&retired),
                "已裁/后移页面不得出现在本阶段路由: {retired}"
            );
        }
    }

    /// 搜索路由:中英混合别名命中页面,不是过滤当前页内容。
    #[test]
    fn search_routes_mixed_chinese_and_english_aliases_to_the_owning_page() {
        bootstrap();
        let cases = [
            ("字体", "appearance"),
            ("font", "appearance"),
            ("透明度", "appearance"),
            ("opacity", "appearance"),
            ("回滚", "appearance"),
            ("scrollback", "appearance"),
            ("语言", "localization"),
            ("language", "localization"),
            ("粘贴", "interaction"),
            ("paste", "interaction"),
            ("光标", "interaction"),
            ("cursor", "interaction"),
            ("恢复会话", "advanced"),
            ("restore session", "advanced"),
            ("恢复默认", "reset"),
            ("restore defaults", "reset"),
            ("通知", "profiles"),
            ("notification", "profiles"),
            ("配置文件", "profiles"),
            ("profile", "profiles"),
            ("智能体", "agents"),
            ("hooks", "agents"),
            ("claude", "agents"),
            ("接入", "agents"),
        ];
        for (query, expected) in cases {
            let hits = matching_sections(query, UiLanguage::ZhCn);
            assert_eq!(
                hits.first().copied(),
                Some(index_of(expected)),
                "查询 {query:?} 应命中 {expected},实际 {hits:?}"
            );
        }
        assert!(matching_sections("no such setting", UiLanguage::EnUs).is_empty());
        assert_eq!(
            matching_sections("", UiLanguage::EnUs),
            visible_nav_sections().collect::<Vec<_>>(),
            "空查询返回全部可见页"
        );
    }

    /// 每个页面拥有的控件键必须已进设置键注册表(搜索别名单源的结构闸)。
    #[test]
    fn every_page_controller_key_is_registered_with_search_terms() {
        bootstrap();
        for def in registry::get_all() {
            for key in page_keys(def.id) {
                let entry = slterm_settings::keys::lookup(key)
                    .unwrap_or_else(|| panic!("{} 的控件键 {key} 未注册", def.id));
                assert!(!entry.search_terms.trim().is_empty(), "{key} 缺搜索别名");
            }
        }
    }

    /// 隐藏路由不删位:隐藏后搜索与导航都跳过,但 id 顺序不变。
    #[test]
    fn hidden_sections_are_filtered_without_renumbering_routes() {
        bootstrap();
        assert!(HIDDEN_NAV_SECTIONS.is_empty());
        let ids_before = section_ids();
        let visible: Vec<_> = visible_nav_sections().collect();
        assert_eq!(visible.len(), ids_before.len());
        assert_eq!(visible, (0..ids_before.len()).collect::<Vec<_>>());
    }
}
