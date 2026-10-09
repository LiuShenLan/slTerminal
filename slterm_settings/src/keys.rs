//! 设置注册表——键名/段归属/重置清单的单源(D06-1 段嵌套落地)。
//!
//! settings.json 为段嵌套形态:每个设置键归属一个语义段(appearance/
//! scrolling/…),写通道按段浅合并。本表是「键 → 段」映射的唯一权威:
//! `RawSettings` 读取、写通道白名单(M6.1)、`reset` 删除清单全部由本表
//! 驱动,防「加了新设置忘了加进重置清单」的结构性漂移。
//!
//! 域外顶层段(不占注册表条目,结构与消费归归属篇锚定):
//! - `keybind` 段:键位绑定数组,归 07 篇锚定结构,写通道整表替换语义
//!   由 07 消费;
//! - `conpty_input_modes` 段:ConPTY 输入模式矩阵,归 02 篇锚定,键名经
//!   02 域模块常量引用。
//!
//! M6.1 写通道白名单 = 本表段集合 + 上述两个域外段。

use crate::AgentHook;

/// 设置键的语义段。段名 = settings.json 顶层键名。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsDomain {
    Appearance,
    Scrolling,
    Interaction,
    Session,
    System,
    AgentHooks,
}

impl SettingsDomain {
    pub const ALL: [Self; 6] = [
        Self::Appearance,
        Self::Scrolling,
        Self::Interaction,
        Self::Session,
        Self::System,
        Self::AgentHooks,
    ];

    /// settings.json 顶层段名(持久化契约值,只追加不改名)。
    pub const fn segment(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Scrolling => "scrolling",
            Self::Interaction => "interaction",
            Self::Session => "session",
            Self::System => "system",
            Self::AgentHooks => "agent_hooks",
        }
    }
}

/// 一个设置键的注册条目。
pub struct SettingsKeyDef {
    /// 段内 snake_case 键名。
    pub key: &'static str,
    /// 段归属(= settings.json 顶层段)。
    pub domain: SettingsDomain,
    /// false = 授权类显式选择(agent hooks 接入状态),重置时保留。
    pub resettable: bool,
    /// 设置页路由搜索的中英别名串;M6.2 设置页点亮时回填,此前为空。
    pub search_terms: &'static str,
}

const fn key(key: &'static str, domain: SettingsDomain) -> SettingsKeyDef {
    SettingsKeyDef {
        key,
        domain,
        resettable: true,
        search_terms: "",
    }
}

/// 全部已登记设置键。新键落地 = 在本表加一行,白名单/重置清单/读取路径
/// 自动入列。`static` 而非 `const`:注册表是单实例单源,const 引用会在
/// 各使用点各自实例化(对账指针比较即依赖唯一实例)。
pub static KEYS: &[SettingsKeyDef] = &[
    // ---- appearance:主题/字体/窗口材质/壁纸/卡几何/侧栏显示 ----
    key("language", SettingsDomain::Appearance),
    key("theme", SettingsDomain::Appearance),
    key("font_family", SettingsDomain::Appearance),
    key("font_family_cjk", SettingsDomain::Appearance),
    key("ui_font_family", SettingsDomain::Appearance),
    key("ui_font_size", SettingsDomain::Appearance),
    key("font_size", SettingsDomain::Appearance),
    key("density", SettingsDomain::Appearance),
    key("tab_reveal", SettingsDomain::Appearance),
    key("tab_close_visible", SettingsDomain::Appearance),
    key("cjk_bold_regular", SettingsDomain::Appearance),
    key("cell_width_mode", SettingsDomain::Appearance),
    key("vcs_display", SettingsDomain::Appearance),
    key("opacity", SettingsDomain::Appearance),
    key("blur", SettingsDomain::Appearance),
    key("background", SettingsDomain::Appearance),
    key("theme_foreground", SettingsDomain::Appearance),
    key("custom_theme", SettingsDomain::Appearance),
    key("background_image", SettingsDomain::Appearance),
    key("background_image_opacity", SettingsDomain::Appearance),
    key("background_image_fit", SettingsDomain::Appearance),
    key("background_image_alignment", SettingsDomain::Appearance),
    key("background_image_cover_chrome", SettingsDomain::Appearance),
    key("pane_card_radius", SettingsDomain::Appearance),
    key("pane_card_gutter", SettingsDomain::Appearance),
    key("pane_card_shadow", SettingsDomain::Appearance),
    key("pane_card_divider", SettingsDomain::Appearance),
    // ---- scrolling:回滚与滚速 ----
    key("scrollback_lines", SettingsDomain::Scrolling),
    key("scroll_speed", SettingsDomain::Scrolling),
    // ---- interaction:光标/输入/剪贴/响铃/补全/标签行为 ----
    key("cursor_shape", SettingsDomain::Interaction),
    key("cursor_blink", SettingsDomain::Interaction),
    key("cursor_motion", SettingsDomain::Interaction),
    key("copy_on_select", SettingsDomain::Interaction),
    key("focus_follows_mouse", SettingsDomain::Interaction),
    key("dim_inactive_panes", SettingsDomain::Interaction),
    key("multiline_paste_confirm", SettingsDomain::Interaction),
    key("ligatures", SettingsDomain::Interaction),
    key("bell", SettingsDomain::Interaction),
    key("ctrl_wheel_font_zoom", SettingsDomain::Interaction),
    key("ghost", SettingsDomain::Interaction),
    key("accept", SettingsDomain::Interaction),
    key("completion_style", SettingsDomain::Interaction),
    key("new_tab_position", SettingsDomain::Interaction),
    key("panel_resize", SettingsDomain::Interaction),
    // ---- session:启动与会话恢复 ----
    key("shell", SettingsDomain::Session),
    key("startup_directory", SettingsDomain::Session),
    key("keep_session", SettingsDomain::Session),
    key("restore_session", SettingsDomain::Session),
    key("resume_ai", SettingsDomain::Session),
    key("silent_start", SettingsDomain::Session),
    key("launch_at_login", SettingsDomain::Session),
    key("fetch", SettingsDomain::Session),
    // ---- system:系统联动/环境/通知/更新 ----
    key("terminal_proxy", SettingsDomain::System),
    key("refresh_environment", SettingsDomain::System),
    key("ai_toasts", SettingsDomain::System),
    key("notification_duration", SettingsDomain::System),
    key("tray", SettingsDomain::System),
    key("auto_check_updates", SettingsDomain::System),
    key("auto_download_updates", SettingsDomain::System),
    // ---- agent_hooks:AI CLI 接入授权(resettable=false,重置保留选择)----
    // 键名单源在 `AgentHook::settings_key`,此处引用常量防双源漂移。
    SettingsKeyDef {
        key: AgentHook::GLOBAL_SETTINGS_KEY,
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Claude.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Codex.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::OpenCode.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Pi.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Copilot.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Grok.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::OhMyPi.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Cursor.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
    SettingsKeyDef {
        key: AgentHook::Kimi.settings_key(),
        domain: SettingsDomain::AgentHooks,
        resettable: false,
        search_terms: "",
    },
];

/// 段内键名 → 注册条目。未登记键返回 None——读取路径经此解析段,
/// 未登记键读不出,注册表因此成为读取面的机械边界。
pub fn lookup(key: &str) -> Option<&'static SettingsKeyDef> {
    KEYS.iter().find(|def| def.key == key)
}

/// 全部已登记键名(写通道白名单的数据源,M6.1 并入段形态硬校验)。
pub fn whitelist() -> impl Iterator<Item = &'static str> {
    KEYS.iter().map(|def| def.key)
}

/// 重置删除清单:resettable=true 的全部键。
pub fn reset_keys() -> impl Iterator<Item = &'static str> {
    KEYS.iter().filter(|def| def.resettable).map(|def| def.key)
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn keys_are_unique_and_segments_are_stable() {
        let mut seen = HashSet::new();
        for def in KEYS {
            assert!(seen.insert(def.key), "duplicate key: {}", def.key);
            assert!(!def.domain.segment().is_empty());
        }
        assert_eq!(
            SettingsDomain::ALL.map(SettingsDomain::segment),
            [
                "appearance",
                "scrolling",
                "interaction",
                "session",
                "system",
                "agent_hooks"
            ]
        );
    }

    #[test]
    fn every_registered_key_looks_up_its_own_entry() {
        for def in KEYS {
            let found = lookup(def.key).unwrap();
            assert!(std::ptr::eq(found, def));
        }
        assert!(lookup("unknown_key").is_none());
        assert!(
            lookup("keybind").is_none(),
            "keybind 是 07 篇域外段,不占注册表条目"
        );
    }

    #[test]
    fn reset_list_covers_everything_except_hook_authorizations() {
        let resettable: HashSet<_> = reset_keys().collect();
        let total: HashSet<_> = whitelist().collect();
        assert_eq!(total.len() - resettable.len(), 1 + AgentHook::ALL.len());
        assert!(!resettable.contains(AgentHook::GLOBAL_SETTINGS_KEY));
        for hook in AgentHook::ALL {
            assert!(!resettable.contains(hook.settings_key()));
        }
    }

    /// 重置覆盖对账:`RuntimeSettings::from_raw` 消费的键集合与注册表
    /// resettable 集合必须全等——新增键忘了登记,或登记了忘了消费,都是红。
    #[test]
    fn resettable_keys_match_every_key_consumed_by_from_raw() {
        let source = include_str!("lib.rs");
        let reader = source
            .split("pub fn from_raw(raw: &RawSettings) -> Self {")
            .nth(1)
            .expect("from_raw anchor");
        // 函数区间:到 parse_hex_rgb 的文档注释为止(双锚契约,勿改动锚文案)。
        // 链式调用跨行书写,比对前剥掉全部空白。
        let reader: String = reader
            .split("/// 解析 `#rgb`")
            .next()
            .expect("from_raw end anchor")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let mut consumed = HashSet::new();
        for accessor in [
            "raw.value(\"",
            "raw.f32(\"",
            "raw.bool_on(\"",
            "raw.usize(\"",
        ] {
            for expression in reader.split(accessor).skip(1) {
                let key = expression.split('"').next().unwrap();
                consumed.insert(key);
            }
        }
        let registered: HashSet<_> = reset_keys().collect();
        let missing: Vec<_> = consumed.difference(&registered).collect();
        let unconsumed: Vec<_> = registered.difference(&consumed).collect();
        assert!(missing.is_empty(), "from_raw 消费了未登记键: {missing:?}");
        assert!(
            unconsumed.is_empty(),
            "登记键未被 from_raw 消费: {unconsumed:?}"
        );
    }
}
