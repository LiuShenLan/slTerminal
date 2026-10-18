//! 设置注册表——键名/段归属/重置清单的单源(D06-1 段嵌套落地)。
//!
//! settings.json 为段嵌套形态:每个设置键归属一个语义段(appearance/
//! scrolling/…),写通道按段浅合并。本表是「键 → 段」映射的唯一权威:
//! `RawSettings` 读取、写通道顶层白名单(M6.1)、`reset` 删除清单全部由
//! 本表驱动,防「加了新设置忘了加进重置清单」的结构性漂移。
//!
//! 三清单同源(段形态):
//! - 键域枚举 [`SettingsDomain`] = 注册表可归属的段集合;
//! - 段内键白名单 [`whitelist`] = 注册表 leaf 键集合;
//! - 重置清单 [`reset_keys`] = `resettable = true` 的 leaf 键集合。
//!
//! 域外顶层段(不占注册表条目,结构与消费归归属篇锚定,[`EXTERNAL_SEGMENTS`]):
//! - [`KEYBIND_SEGMENT`]:键位绑定数组,归 07 篇锚定结构,写通道整表替换
//!   语义由 07 消费;段名常量由 07 域模块引用,禁二次字面量。
//! - [`CONPTY_INPUT_MODES_SEGMENT`]:ConPTY 输入模式矩阵,归 02 篇锚定,
//!   段名常量由 02 域模块引用。
//!
//! 写通道顶层白名单 = [`segment_whitelist`] = 域段 + 域外段。

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

const fn key(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    SettingsKeyDef {
        key,
        domain,
        resettable: true,
        search_terms,
    }
}

/// 授权类键(重置保留)的注册条目;搜索别名与普通键同源登记。
const fn protected(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    SettingsKeyDef {
        key,
        domain,
        resettable: false,
        search_terms,
    }
}

/// 全部已登记设置键。新键落地 = 在本表加一行,白名单/重置清单/读取路径
/// 自动入列。`static` 而非 `const`:注册表是单实例单源,const 引用会在
/// 各使用点各自实例化(对账指针比较即依赖唯一实例)。
pub static KEYS: &[SettingsKeyDef] = &[
    // ---- appearance:主题/字体/窗口材质/壁纸/卡几何/侧栏显示 ----
    key(
        "language",
        SettingsDomain::Appearance,
        "language 语言 locale 本地化",
    ),
    key(
        "theme",
        SettingsDomain::Appearance,
        "theme 主题 colors 配色",
    ),
    key(
        "font_family",
        SettingsDomain::Appearance,
        "font 字体 family 字族",
    ),
    key(
        "font_family_cjk",
        SettingsDomain::Appearance,
        "cjk font 中文字体",
    ),
    key(
        "ui_font_family",
        SettingsDomain::Appearance,
        "ui font 界面字体",
    ),
    key(
        "ui_font_size",
        SettingsDomain::Appearance,
        "ui size 界面字号",
    ),
    key("font_size", SettingsDomain::Appearance, "font size 字号"),
    key(
        "density",
        SettingsDomain::Appearance,
        "density 密度 spacing 间距",
    ),
    key(
        "tab_reveal",
        SettingsDomain::Appearance,
        "tab reveal 标签动画",
    ),
    key(
        "tab_close_visible",
        SettingsDomain::Appearance,
        "close button 关闭按钮",
    ),
    key(
        "cjk_bold_regular",
        SettingsDomain::Appearance,
        "cjk bold 中文粗体",
    ),
    key(
        "cell_width_mode",
        SettingsDomain::Appearance,
        "cell width 字宽",
    ),
    key(
        "vcs_display",
        SettingsDomain::Appearance,
        "vcs git 版本控制",
    ),
    key("opacity", SettingsDomain::Appearance, "opacity 不透明度"),
    key(
        "blur",
        SettingsDomain::Appearance,
        "blur 模糊 material 材质",
    ),
    key(
        "background",
        SettingsDomain::Appearance,
        "background 背景色",
    ),
    key(
        "theme_foreground",
        SettingsDomain::Appearance,
        "foreground 前景色",
    ),
    key(
        "custom_theme",
        SettingsDomain::Appearance,
        "custom theme 自定义主题",
    ),
    key(
        "background_image",
        SettingsDomain::Appearance,
        "wallpaper 壁纸 image 图片",
    ),
    key(
        "background_image_opacity",
        SettingsDomain::Appearance,
        "wallpaper opacity 壁纸透明度",
    ),
    key(
        "background_image_fit",
        SettingsDomain::Appearance,
        "wallpaper fit 壁纸适应",
    ),
    key(
        "background_image_alignment",
        SettingsDomain::Appearance,
        "wallpaper alignment 壁纸对齐",
    ),
    key(
        "background_image_cover_chrome",
        SettingsDomain::Appearance,
        "wallpaper chrome 壁纸铺满",
    ),
    key(
        "pane_card_radius",
        SettingsDomain::Appearance,
        "card radius 圆角",
    ),
    key(
        "pane_card_gutter",
        SettingsDomain::Appearance,
        "card gutter 卡缝",
    ),
    key(
        "pane_card_shadow",
        SettingsDomain::Appearance,
        "card shadow 投影",
    ),
    key(
        "pane_card_divider",
        SettingsDomain::Appearance,
        "divider 分界线",
    ),
    // ---- scrolling:回滚与滚速 ----
    key(
        "scrollback_lines",
        SettingsDomain::Scrolling,
        "scrollback history 回滚行数",
    ),
    key(
        "scroll_speed",
        SettingsDomain::Scrolling,
        "scroll speed 滚动速度 wheel 滚轮",
    ),
    // ---- interaction:光标/输入/剪贴/响铃/补全/标签行为 ----
    key(
        "cursor_shape",
        SettingsDomain::Interaction,
        "cursor shape 光标形状",
    ),
    key(
        "cursor_blink",
        SettingsDomain::Interaction,
        "cursor blink 光标闪烁",
    ),
    key(
        "cursor_motion",
        SettingsDomain::Interaction,
        "cursor motion 平滑光标",
    ),
    key(
        "copy_on_select",
        SettingsDomain::Interaction,
        "copy on select 选中复制",
    ),
    key(
        "focus_follows_mouse",
        SettingsDomain::Interaction,
        "focus follows mouse 焦点跟随鼠标",
    ),
    key(
        "dim_inactive_panes",
        SettingsDomain::Interaction,
        "dim inactive 压暗分屏",
    ),
    key(
        "multiline_paste_confirm",
        SettingsDomain::Interaction,
        "paste confirm 粘贴确认",
    ),
    key("ligatures", SettingsDomain::Interaction, "ligatures 连字"),
    key("bell", SettingsDomain::Interaction, "bell 铃声"),
    key(
        "ctrl_wheel_font_zoom",
        SettingsDomain::Interaction,
        "ctrl wheel zoom 滚轮缩放",
    ),
    key("ghost", SettingsDomain::Interaction, "ghost 命令补全"),
    key("accept", SettingsDomain::Interaction, "accept key 接受键"),
    key(
        "completion_style",
        SettingsDomain::Interaction,
        "completion style 补全样式",
    ),
    key(
        "new_tab_position",
        SettingsDomain::Interaction,
        "new tab 新标签位置",
    ),
    key(
        "panel_resize",
        SettingsDomain::Interaction,
        "panel resize 侧栏宽度",
    ),
    // ---- session:启动与会话恢复 ----
    key("shell", SettingsDomain::Session, "shell 默认终端"),
    key(
        "startup_directory",
        SettingsDomain::Session,
        "startup directory 启动目录",
    ),
    key(
        "keep_session",
        SettingsDomain::Session,
        "keep session 保留会话",
    ),
    key(
        "restore_session",
        SettingsDomain::Session,
        "restore session 恢复会话",
    ),
    key("resume_ai", SettingsDomain::Session, "resume ai 接续对话"),
    key(
        "silent_start",
        SettingsDomain::Session,
        "silent start 静默启动",
    ),
    key(
        "launch_at_login",
        SettingsDomain::Session,
        "launch at login 开机启动",
    ),
    key("fetch", SettingsDomain::Session, "fetch 欢迎屏"),
    // ---- system:系统联动/环境/通知/更新 ----
    key("terminal_proxy", SettingsDomain::System, "proxy 代理"),
    key(
        "refresh_environment",
        SettingsDomain::System,
        "environment 环境变量 refresh 刷新",
    ),
    key("ai_toasts", SettingsDomain::System, "ai toast 消息通知"),
    key(
        "notification_duration",
        SettingsDomain::System,
        "notification duration 通知时长",
    ),
    key("tray", SettingsDomain::System, "tray 托盘"),
    key(
        "auto_check_updates",
        SettingsDomain::System,
        "update 更新 check 检查",
    ),
    key(
        "auto_download_updates",
        SettingsDomain::System,
        "download 下载更新",
    ),
    // ---- agent_hooks:AI CLI 接入授权(resettable=false,重置保留选择)----
    // 键名单源在 `AgentHook::settings_key`,此处引用常量防双源漂移。
    protected(
        AgentHook::GLOBAL_SETTINGS_KEY,
        SettingsDomain::AgentHooks,
        "agent hook 智能体钩子 接入",
    ),
    protected(
        AgentHook::Claude.settings_key(),
        SettingsDomain::AgentHooks,
        "claude hook 接入",
    ),
    protected(
        AgentHook::Codex.settings_key(),
        SettingsDomain::AgentHooks,
        "codex hook 接入",
    ),
    protected(
        AgentHook::OpenCode.settings_key(),
        SettingsDomain::AgentHooks,
        "opencode hook 接入",
    ),
    protected(
        AgentHook::Pi.settings_key(),
        SettingsDomain::AgentHooks,
        "pi hook 接入",
    ),
    protected(
        AgentHook::Copilot.settings_key(),
        SettingsDomain::AgentHooks,
        "copilot hook 接入",
    ),
    protected(
        AgentHook::Grok.settings_key(),
        SettingsDomain::AgentHooks,
        "grok hook 接入",
    ),
    protected(
        AgentHook::OhMyPi.settings_key(),
        SettingsDomain::AgentHooks,
        "oh my pi hook 接入",
    ),
    protected(
        AgentHook::Cursor.settings_key(),
        SettingsDomain::AgentHooks,
        "cursor hook 接入",
    ),
    protected(
        AgentHook::Kimi.settings_key(),
        SettingsDomain::AgentHooks,
        "kimi hook 接入",
    ),
];

/// 域外顶层段键名:keybind(07 篇锚定结构;写通道整表替换)。
pub const KEYBIND_SEGMENT: &str = "keybind";

/// 域外顶层段键名:conpty_input_modes(02 篇锚定矩阵)。
pub const CONPTY_INPUT_MODES_SEGMENT: &str = "conpty_input_modes";

/// 全部域外顶层段(不占注册表 leaf 条目,结构归归属篇)。
pub const EXTERNAL_SEGMENTS: [&str; 2] = [KEYBIND_SEGMENT, CONPTY_INPUT_MODES_SEGMENT];

/// 域外段判定:写通道按段整体替换,不做段内 leaf 校验。
pub fn is_external_segment(name: &str) -> bool {
    EXTERNAL_SEGMENTS.contains(&name)
}

/// 写通道顶层白名单:注册表域段 + 域外段。段名唯一,无孤儿段。
pub fn segment_whitelist() -> impl Iterator<Item = &'static str> {
    SettingsDomain::ALL
        .into_iter()
        .map(SettingsDomain::segment)
        .chain(EXTERNAL_SEGMENTS)
}

/// 段内键名 → 注册条目。未登记键返回 None——读取路径经此解析段,
/// 未登记键读不出,注册表因此成为读取面的机械边界。
pub fn lookup(key: &str) -> Option<&'static SettingsKeyDef> {
    KEYS.iter().find(|def| def.key == key)
}

/// 全部已登记 leaf 键名(段内白名单;顶层段白名单见 [`segment_whitelist`])。
pub fn whitelist() -> impl Iterator<Item = &'static str> {
    KEYS.iter().map(|def| def.key)
}

/// 重置删除清单:resettable=true 的全部键。
pub fn reset_keys() -> impl Iterator<Item = &'static str> {
    KEYS.iter().filter(|def| def.resettable).map(|def| def.key)
}

/// 设置页搜索索引:键名 + 域归属 + 中英别名串(设置页路由查找消费)。
/// 导航元数据从键注册表导出,不另立平行字面量表。
pub fn search_index() -> impl Iterator<Item = (&'static str, SettingsDomain, &'static str)> {
    KEYS.iter()
        .map(|def| (def.key, def.domain, def.search_terms))
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
            lookup(KEYBIND_SEGMENT).is_none(),
            "keybind 是 07 篇域外段,不占注册表条目"
        );
        assert!(
            lookup(CONPTY_INPUT_MODES_SEGMENT).is_none(),
            "conpty_input_modes 是 02 篇域外段,不占注册表条目"
        );
    }

    /// 三清单对账:域段枚举 ⊆ 顶层白名单、每条注册键落在白名单段内、
    /// 每个域段都有键、leaf 名与段名不相交——段形态自洽且无孤儿键。
    #[test]
    fn segment_whitelist_reconciles_registry_and_external_segments() {
        let segments: Vec<_> = segment_whitelist().collect();
        let unique: HashSet<_> = segments.iter().copied().collect();
        assert_eq!(unique.len(), segments.len(), "顶层段名重复");
        assert_eq!(
            unique.len(),
            SettingsDomain::ALL.len() + EXTERNAL_SEGMENTS.len(),
            "顶层白名单 = 域段 + 域外段"
        );
        for domain in SettingsDomain::ALL {
            assert!(
                unique.contains(domain.segment()),
                "域段 {} 未进顶层白名单",
                domain.segment()
            );
            assert!(
                KEYS.iter().any(|def| def.domain == domain),
                "域段 {} 没有任何注册键",
                domain.segment()
            );
        }
        for external in EXTERNAL_SEGMENTS {
            assert!(unique.contains(external), "域外段 {external} 未进白名单");
            assert!(is_external_segment(external));
        }
        assert!(!is_external_segment("appearance"));
        for def in KEYS {
            assert!(
                unique.contains(def.domain.segment()),
                "键 {} 归属段 {} 不在顶层白名单",
                def.key,
                def.domain.segment()
            );
            assert!(
                !unique.contains(def.key),
                "leaf 键名 {} 与顶层段名冲突",
                def.key
            );
        }
        // 段内白名单与域外段不相交:域外段结构归归属篇,不走 leaf 校验。
        for key in whitelist() {
            assert!(!is_external_segment(key), "域外段混入 leaf 白名单: {key}");
        }
    }

    /// 搜索别名登记完备性:每个键至少一条中英别名,索引与白名单同集。
    /// 新键忘登记别名 = 搜索路由无声漏键,此用例即红。
    #[test]
    fn every_registered_key_exports_a_non_empty_search_alias() {
        let mut indexed = HashSet::new();
        for (key, domain, terms) in search_index() {
            assert!(indexed.insert(key), "搜索索引重复键: {key}");
            assert!(!terms.trim().is_empty(), "键 {key} 缺搜索别名");
            assert_eq!(lookup(key).map(|def| def.domain), Some(domain));
        }
        let whitelisted: HashSet<_> = whitelist().collect();
        assert_eq!(indexed, whitelisted, "搜索索引与 leaf 白名单必须同集");
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
