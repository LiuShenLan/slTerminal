//! 设置注册表——字段元数据、段归属与派生清单的单源(段嵌套落地)。
//!
//! settings.json 为段嵌套形态:每个设置键归属一个语义段(appearance/
//! scrolling/…),写通道按段浅合并。本表声明键、类型、缺省值、作用域、
//! 校验器、敏感性、导出/重置资格与保存模式;`RawSettings` 校验和缺省
//! 回退、写通道白名单、重置及导出清单均由本表驱动。
//!
//! 派生规则同源(段形态):
//! - 键域枚举 [`SettingsDomain`] = 注册表可归属的段集合;
//! - 段内键白名单 [`whitelist`] = 注册表 leaf 键集合;
//! - 重置清单 [`reset_keys`] = `resettable = true` 的 leaf 键集合;
//! - 普通导出清单 [`SettingsRegistry::export_keys`] = 非敏感且允许导出的键。
//!
//! 域外顶层段(不占注册表条目,结构与消费由所属领域锚定,[`EXTERNAL_SEGMENTS`]):
//! - [`KEYBIND_SEGMENT`][]:键位绑定数组,由键位绑定领域锚定结构,写通道整表替换
//!   语义由其消费;段名常量由该领域模块引用,禁二次字面量。
//! - [`CONPTY_INPUT_MODES_SEGMENT`]:ConPTY 输入模式矩阵,由 ConPTY 输入领域锚定,
//!   段名常量由该领域模块引用。
//!
//! 写通道顶层白名单 = [`segment_whitelist`] = 域段 + 域外段。

use crate::json::Value;
use crate::{AgentHook, RawSettings};

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

/// 设置字段的持久 JSON 值类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsValueType {
    String,
    Number,
    Integer,
    Boolean,
}

/// 字段缺省值；Unset 表示未设置，而非 JSON null。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsDefault {
    Unset,
    Text(&'static str),
    Number(f64),
    Integer(usize),
    Boolean(bool),
}

/// 设置字段的归属作用域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsScope {
    Application,
    Agent(AgentHook),
}

/// 设置字段进入保存队列的方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSaveMode {
    Automatic,
    Explicit,
}

/// 单字段恢复后的来源。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingRecoveryStatus {
    Persisted,
    Missing,
    Invalid,
}

/// 单字段恢复结果；非法字段只替换自身，不影响邻接字段。
#[derive(Clone, Debug, PartialEq)]
pub struct SettingRecovery {
    pub value: Option<Value>,
    pub status: SettingRecoveryStatus,
}

fn valid_non_empty_string(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.is_empty())
}

/// 一个设置键的注册条目。
pub struct SettingsKeyDef {
    /// 段内 snake_case 键名。
    pub key: &'static str,
    /// 段归属(= settings.json 顶层段)。
    pub domain: SettingsDomain,
    /// 持久值类型。
    pub value_type: SettingsValueType,
    /// 字段缺省值。
    pub default: SettingsDefault,
    /// 字段作用域。
    pub scope: SettingsScope,
    /// 字段校验器。
    pub validator: fn(&Value) -> bool,
    /// 是否含敏感值。
    pub sensitive: bool,
    /// 是否允许普通设置导出。
    pub exportable: bool,
    /// false = 授权类显式选择(agent hooks 接入状态),重置时保留。
    pub resettable: bool,
    /// 保存模式。
    pub save_mode: SettingsSaveMode,
    /// 设置页路由搜索的中英别名串;设置页点亮时回填,此前为空。
    pub search_terms: &'static str,
}

impl SettingsKeyDef {
    /// 将声明的缺省值转换为内部 JSON 值。
    pub fn default_value(&self) -> Option<Value> {
        match self.default {
            SettingsDefault::Unset => None,
            SettingsDefault::Text(text) => Some(Value::String(text.to_owned())),
            SettingsDefault::Number(number) => Some(Value::Number(number)),
            SettingsDefault::Integer(number) => Some(Value::from(number)),
            SettingsDefault::Boolean(flag) => Some(Value::Bool(flag)),
        }
    }

    pub(crate) fn default_text(&self) -> &'static str {
        match self.default {
            SettingsDefault::Text(text) => text,
            _ => panic!("键 {} 的缺省值不是字符串", self.key),
        }
    }

    pub(crate) fn default_number(&self) -> f64 {
        match self.default {
            SettingsDefault::Number(number) => number,
            _ => panic!("键 {} 的缺省值不是数值", self.key),
        }
    }

    pub(crate) fn default_integer(&self) -> usize {
        match self.default {
            SettingsDefault::Integer(number) => number,
            _ => panic!("键 {} 的缺省值不是整数", self.key),
        }
    }

    pub(crate) fn default_boolean(&self) -> bool {
        match self.default {
            SettingsDefault::Boolean(flag) => flag,
            _ => panic!("键 {} 的缺省值不是布尔", self.key),
        }
    }
}

/// 应用设置唯一字段目录。
#[derive(Clone, Copy, Debug, Default)]
pub struct SettingsRegistry;

impl SettingsRegistry {
    pub const fn new() -> Self {
        Self
    }

    pub fn fields(&self) -> &'static [SettingsKeyDef] {
        KEYS
    }

    pub fn lookup(&self, key: &str) -> Option<&'static SettingsKeyDef> {
        lookup(key)
    }

    /// 校验一个字段值；未登记键不进入设置通道。
    pub fn validate(&self, key: &str, value: &Value) -> Option<bool> {
        lookup(key).map(|def| (def.validator)(value))
    }

    /// 恢复一个字段；未登记键返回 None。
    pub fn recover(&self, raw: &RawSettings, key: &str) -> Option<SettingRecovery> {
        let def = lookup(key)?;
        let recovery = match raw.segment_value(key) {
            None | Some(Value::Null) => SettingRecovery {
                value: def.default_value(),
                status: SettingRecoveryStatus::Missing,
            },
            Some(value) if (def.validator)(value) => SettingRecovery {
                value: Some(value.clone()),
                status: SettingRecoveryStatus::Persisted,
            },
            Some(_) => SettingRecovery {
                value: def.default_value(),
                status: SettingRecoveryStatus::Invalid,
            },
        };
        Some(recovery)
    }

    /// 普通设置导出允许的键；未知命名空间不在目录中。
    pub fn export_keys(&self) -> impl Iterator<Item = &'static str> {
        KEYS.iter()
            .filter(|def| def.exportable && !def.sensitive)
            .map(|def| def.key)
    }
}

const fn entry(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    value_type: SettingsValueType,
    default: SettingsDefault,
    validator: fn(&Value) -> bool,
) -> SettingsKeyDef {
    SettingsKeyDef {
        key,
        domain,
        value_type,
        default,
        scope: SettingsScope::Application,
        validator,
        sensitive: false,
        exportable: true,
        resettable: true,
        save_mode: SettingsSaveMode::Automatic,
        search_terms,
    }
}

/// 保留字段的其他登记规则，仅禁止普通设置导出。
const fn non_exported(mut def: SettingsKeyDef) -> SettingsKeyDef {
    def.exportable = false;
    def
}

/// 可选非空字符串。
const fn key(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::String,
        SettingsDefault::Unset,
        valid_non_empty_string,
    )
}

/// 有缺省值且由领域解析器校验的字符串。
const fn text(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    default: &'static str,
    validator: fn(&Value) -> bool,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::String,
        SettingsDefault::Text(default),
        validator,
    )
}

/// 可选且由领域解析器校验的字符串。
const fn validated(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    validator: fn(&Value) -> bool,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::String,
        SettingsDefault::Unset,
        validator,
    )
}

const fn boolean(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    default: bool,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::Boolean,
        SettingsDefault::Boolean(default),
        valid_boolean,
    )
}

const fn optional_boolean(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::Boolean,
        SettingsDefault::Unset,
        valid_boolean,
    )
}

const fn number(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    default: f64,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::Number,
        SettingsDefault::Number(default),
        valid_number,
    )
}

const fn optional_number(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::Number,
        SettingsDefault::Unset,
        valid_number,
    )
}

const fn integer(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
    default: usize,
    validator: fn(&Value) -> bool,
) -> SettingsKeyDef {
    entry(
        key,
        domain,
        search_terms,
        SettingsValueType::Integer,
        SettingsDefault::Integer(default),
        validator,
    )
}

/// 全局授权键显式保存，不进入普通导出或恢复默认。
const fn protected(
    key: &'static str,
    domain: SettingsDomain,
    search_terms: &'static str,
) -> SettingsKeyDef {
    SettingsKeyDef {
        key,
        domain,
        value_type: SettingsValueType::Boolean,
        default: SettingsDefault::Boolean(true),
        scope: SettingsScope::Application,
        validator: valid_boolean,
        sensitive: false,
        exportable: false,
        resettable: false,
        save_mode: SettingsSaveMode::Explicit,
        search_terms,
    }
}

/// 单 CLI 授权键缺省为未设置，由全局键与内置默认集继续解析。
const fn agent(agent: AgentHook, search_terms: &'static str) -> SettingsKeyDef {
    SettingsKeyDef {
        key: agent.settings_key(),
        domain: SettingsDomain::AgentHooks,
        value_type: SettingsValueType::Boolean,
        default: SettingsDefault::Unset,
        scope: SettingsScope::Agent(agent),
        validator: valid_boolean,
        sensitive: false,
        exportable: false,
        resettable: false,
        save_mode: SettingsSaveMode::Explicit,
        search_terms,
    }
}

fn valid_boolean(value: &Value) -> bool {
    matches!(value, Value::Bool(_))
}

fn valid_number(value: &Value) -> bool {
    value.as_f64().is_some_and(|number| number.is_finite())
}

pub(crate) fn integer_value(value: &Value) -> Option<usize> {
    let number = value.as_f64()?;
    (number.is_finite() && number.fract() == 0.0 && number >= 0.0 && number <= usize::MAX as f64)
        .then_some(number as usize)
}

fn valid_scrollback(value: &Value) -> bool {
    integer_value(value).is_some_and(|lines| crate::SCROLLBACK_VALUES.contains(&lines))
}

fn valid_non_blank_string(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

macro_rules! string_validator {
    ($name:ident, $parser:path) => {
        fn $name(value: &Value) -> bool {
            value.as_str().is_some_and(|text| $parser(text).is_some())
        }
    };
}

string_validator!(valid_language, crate::LanguagePref::from_settings);
string_validator!(valid_theme, crate::ThemeName::from_prompt_name);
string_validator!(valid_density, crate::DensityName::from_settings);
string_validator!(valid_tab_reveal, crate::TabRevealName::from_settings);
string_validator!(valid_cell_width, crate::CellWidthModeName::from_settings);
string_validator!(valid_vcs_display, crate::VcsDisplayName::from_settings);
string_validator!(valid_blur, crate::BlurMode::from_settings);
string_validator!(valid_color, crate::parse_hex_rgb);
string_validator!(valid_cursor_shape, crate::CursorShapeName::from_settings);
string_validator!(valid_cursor_motion, crate::CursorMotion::from_settings);
string_validator!(valid_ligatures, crate::Ligatures::from_settings);
string_validator!(valid_bell, crate::BellModeName::from_settings);
string_validator!(valid_accept, crate::AcceptKeyName::from_settings);
string_validator!(valid_completion, crate::CompletionStyleName::from_settings);
string_validator!(valid_new_tab, crate::NewTabPositionName::from_settings);
string_validator!(
    valid_notification_duration,
    crate::NotificationDuration::from_settings
);

/// 全部已登记设置键。新键落地 = 在本表加一行,白名单/重置清单/读取路径
/// 自动入列。`static` 而非 `const`:注册表是单实例单源,const 引用会在
/// 各使用点各自实例化(对账指针比较即依赖唯一实例)。
pub static KEYS: &[SettingsKeyDef] = &[
    // ---- appearance:主题/字体/窗口材质/壁纸/卡几何/侧栏显示 ----
    text(
        "language",
        SettingsDomain::Appearance,
        "language 语言 locale 本地化",
        "system",
        valid_language,
    ),
    text(
        "theme",
        SettingsDomain::Appearance,
        "theme 主题 colors 配色",
        "Linear",
        valid_theme,
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
    optional_number(
        "ui_font_size",
        SettingsDomain::Appearance,
        "ui size 界面字号",
    ),
    optional_number("font_size", SettingsDomain::Appearance, "font size 字号"),
    text(
        "density",
        SettingsDomain::Appearance,
        "density 密度 spacing 间距",
        "standard",
        valid_density,
    ),
    text(
        "tab_reveal",
        SettingsDomain::Appearance,
        "tab reveal 标签动画",
        "slide",
        valid_tab_reveal,
    ),
    boolean(
        "tab_close_visible",
        SettingsDomain::Appearance,
        "close button 关闭按钮",
        true,
    ),
    boolean(
        "cjk_bold_regular",
        SettingsDomain::Appearance,
        "cjk bold 中文粗体",
        true,
    ),
    text(
        "cell_width_mode",
        SettingsDomain::Appearance,
        "cell width 字宽",
        "compact",
        valid_cell_width,
    ),
    text(
        "vcs_display",
        SettingsDomain::Appearance,
        "vcs git 版本控制",
        "auto",
        valid_vcs_display,
    ),
    number(
        "opacity",
        SettingsDomain::Appearance,
        "opacity 不透明度",
        1.0,
    ),
    text(
        "blur",
        SettingsDomain::Appearance,
        "blur 模糊 material 材质",
        "none",
        valid_blur,
    ),
    validated(
        "background",
        SettingsDomain::Appearance,
        "background 背景色",
        valid_color,
    ),
    validated(
        "theme_foreground",
        SettingsDomain::Appearance,
        "foreground 前景色",
        valid_color,
    ),
    non_exported(key(
        "custom_theme",
        SettingsDomain::Appearance,
        "custom theme 自定义主题",
    )),
    validated(
        "background_image",
        SettingsDomain::Appearance,
        "wallpaper 壁纸 image 图片",
        valid_non_blank_string,
    ),
    number(
        "background_image_opacity",
        SettingsDomain::Appearance,
        "wallpaper opacity 壁纸透明度",
        crate::DEFAULT_BACKGROUND_IMAGE_OPACITY as f64,
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
    boolean(
        "background_image_cover_chrome",
        SettingsDomain::Appearance,
        "wallpaper chrome 壁纸铺满",
        false,
    ),
    optional_number(
        "pane_card_radius",
        SettingsDomain::Appearance,
        "card radius 圆角",
    ),
    optional_number(
        "pane_card_gutter",
        SettingsDomain::Appearance,
        "card gutter 卡缝",
    ),
    optional_boolean(
        "pane_card_shadow",
        SettingsDomain::Appearance,
        "card shadow 投影",
    ),
    optional_number(
        "pane_card_divider",
        SettingsDomain::Appearance,
        "divider 分界线",
    ),
    // ---- scrolling:回滚与滚速 ----
    integer(
        "scrollback_lines",
        SettingsDomain::Scrolling,
        "scrollback history 回滚行数",
        crate::DEFAULT_SCROLLBACK_LINES,
        valid_scrollback,
    ),
    number(
        "scroll_speed",
        SettingsDomain::Scrolling,
        "scroll speed 滚动速度 wheel 滚轮",
        crate::DEFAULT_SCROLL_SPEED as f64,
    ),
    // ---- interaction:光标/输入/剪贴/响铃/补全/标签行为 ----
    validated(
        "cursor_shape",
        SettingsDomain::Interaction,
        "cursor shape 光标形状",
        valid_cursor_shape,
    ),
    optional_boolean(
        "cursor_blink",
        SettingsDomain::Interaction,
        "cursor blink 光标闪烁",
    ),
    text(
        "cursor_motion",
        SettingsDomain::Interaction,
        "cursor motion 平滑光标",
        "off",
        valid_cursor_motion,
    ),
    boolean(
        "copy_on_select",
        SettingsDomain::Interaction,
        "copy on select 选中复制",
        false,
    ),
    optional_boolean(
        "focus_follows_mouse",
        SettingsDomain::Interaction,
        "focus follows mouse 焦点跟随鼠标",
    ),
    boolean(
        "dim_inactive_panes",
        SettingsDomain::Interaction,
        "dim inactive 压暗分屏",
        true,
    ),
    boolean(
        "multiline_paste_confirm",
        SettingsDomain::Interaction,
        "paste confirm 粘贴确认",
        true,
    ),
    text(
        "ligatures",
        SettingsDomain::Interaction,
        "ligatures 连字",
        "on",
        valid_ligatures,
    ),
    text(
        "bell",
        SettingsDomain::Interaction,
        "bell 铃声",
        "both",
        valid_bell,
    ),
    boolean(
        "ctrl_wheel_font_zoom",
        SettingsDomain::Interaction,
        "ctrl wheel zoom 滚轮缩放",
        true,
    ),
    boolean("ghost", SettingsDomain::Interaction, "ghost 命令补全", true),
    text(
        "accept",
        SettingsDomain::Interaction,
        "accept key 接受键",
        "both",
        valid_accept,
    ),
    text(
        "completion_style",
        SettingsDomain::Interaction,
        "completion style 补全样式",
        "inline",
        valid_completion,
    ),
    text(
        "new_tab_position",
        SettingsDomain::Interaction,
        "new tab 新标签位置",
        "after_current",
        valid_new_tab,
    ),
    boolean(
        "panel_resize",
        SettingsDomain::Interaction,
        "panel resize 侧栏宽度",
        true,
    ),
    // ---- session:启动与会话恢复 ----
    key("shell", SettingsDomain::Session, "shell 默认终端"),
    key(
        "startup_directory",
        SettingsDomain::Session,
        "startup directory 启动目录",
    ),
    boolean(
        "keep_session",
        SettingsDomain::Session,
        "keep session 保留会话",
        false,
    ),
    boolean(
        "restore_session",
        SettingsDomain::Session,
        "restore session 恢复会话",
        true,
    ),
    boolean(
        "resume_ai",
        SettingsDomain::Session,
        "resume ai 接续对话",
        true,
    ),
    boolean(
        "silent_start",
        SettingsDomain::Session,
        "silent start 静默启动",
        false,
    ),
    boolean(
        "launch_at_login",
        SettingsDomain::Session,
        "launch at login 开机启动",
        false,
    ),
    boolean("fetch", SettingsDomain::Session, "fetch 欢迎屏", false),
    // ---- system:系统联动/环境/通知/更新 ----
    boolean(
        "terminal_proxy",
        SettingsDomain::System,
        "proxy 代理",
        false,
    ),
    boolean(
        "refresh_environment",
        SettingsDomain::System,
        "environment 环境变量 refresh 刷新",
        true,
    ),
    boolean(
        "ai_toasts",
        SettingsDomain::System,
        "ai toast 消息通知",
        true,
    ),
    text(
        "notification_duration",
        SettingsDomain::System,
        "notification duration 通知时长",
        "default",
        valid_notification_duration,
    ),
    boolean("tray", SettingsDomain::System, "tray 托盘", true),
    boolean(
        "auto_check_updates",
        SettingsDomain::System,
        "update 更新 check 检查",
        true,
    ),
    boolean(
        "auto_download_updates",
        SettingsDomain::System,
        "download 下载更新",
        false,
    ),
    // ---- agent_hooks:AI CLI 接入授权(resettable=false,重置保留选择)----
    // 键名单源在 `AgentHook::settings_key`,此处引用常量防双源漂移。
    protected(
        AgentHook::GLOBAL_SETTINGS_KEY,
        SettingsDomain::AgentHooks,
        "agent hook 智能体钩子 接入",
    ),
    agent(AgentHook::Claude, "claude hook 接入"),
    agent(AgentHook::Codex, "codex hook 接入"),
    agent(AgentHook::OpenCode, "opencode hook 接入"),
    agent(AgentHook::Pi, "pi hook 接入"),
    agent(AgentHook::Copilot, "copilot hook 接入"),
    agent(AgentHook::Grok, "grok hook 接入"),
    agent(AgentHook::OhMyPi, "oh my pi hook 接入"),
    agent(AgentHook::Cursor, "cursor hook 接入"),
    agent(AgentHook::Kimi, "kimi hook 接入"),
];

/// 域外顶层段键名:keybind(键位绑定锚定结构;写通道整表替换)。
pub const KEYBIND_SEGMENT: &str = "keybind";

/// 域外顶层段键名:conpty_input_modes(ConPTY 输入模式锚定矩阵)。
pub const CONPTY_INPUT_MODES_SEGMENT: &str = "conpty_input_modes";

/// 全部域外顶层段(不占注册表 leaf 条目,结构由所属领域锚定)。
pub const EXTERNAL_SEGMENTS: [&str; 2] = [KEYBIND_SEGMENT, CONPTY_INPUT_MODES_SEGMENT];

/// 域外段判定:写通道按段整体替换,不做段内 leaf 校验。
pub fn is_external_segment(name: &str) -> bool {
    EXTERNAL_SEGMENTS.contains(&name)
}

/// `keybind` 段宽容解析:元素缺 `combo`/`action`、类型不符或空串 → 跳过
/// 该行;行序即优先级(数组顺序原样保留)。
pub fn keybind_pairs_from_json(value: &crate::json::Value) -> Vec<(String, String)> {
    let crate::json::Value::Array(entries) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let combo = entry.get("combo")?.as_str()?.trim();
            let action = entry.get("action")?.as_str()?.trim();
            (!combo.is_empty() && !action.is_empty()).then(|| (combo.to_owned(), action.to_owned()))
        })
        .collect()
}

/// `(combo, action)` 对表 → `keybind` 段值(整表形态)。
pub fn keybind_value(pairs: &[(String, String)]) -> crate::json::Value {
    crate::json::Value::Array(
        pairs
            .iter()
            .map(|(combo, action)| {
                crate::json::Value::Object(vec![
                    (
                        "combo".to_owned(),
                        crate::json::Value::String(combo.clone()),
                    ),
                    (
                        "action".to_owned(),
                        crate::json::Value::String(action.clone()),
                    ),
                ])
            })
            .collect(),
    )
}

/// 整表替换 `keybind` 段,其余段/未知段原样保留(行序即优先级)。
pub fn apply_keybinds(
    existing: &crate::json::Value,
    pairs: &[(String, String)],
) -> crate::json::Value {
    crate::apply_updates(existing, &[(KEYBIND_SEGMENT, keybind_value(pairs))])
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
            "keybind 是键位绑定域外段,不占注册表条目"
        );
        assert!(
            lookup(CONPTY_INPUT_MODES_SEGMENT).is_none(),
            "conpty_input_modes 是 ConPTY 输入模式域外段,不占注册表条目"
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
        // 段内白名单与域外段不相交:域外段结构由所属领域锚定,不走 leaf 校验。
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

    /// 注册表缺省值必须有匹配类型并通过所属字段校验器。
    #[test]
    fn settings_registry_declares_valid_defaults_and_complete_metadata() {
        let registry = SettingsRegistry::new();
        assert_eq!(registry.fields().len(), KEYS.len());
        for def in registry.fields() {
            match (def.default, def.value_type) {
                (SettingsDefault::Unset, _)
                | (SettingsDefault::Text(_), SettingsValueType::String)
                | (SettingsDefault::Number(_), SettingsValueType::Number)
                | (SettingsDefault::Integer(_), SettingsValueType::Integer)
                | (SettingsDefault::Boolean(_), SettingsValueType::Boolean) => {}
                _ => panic!("键 {} 的缺省值与类型不符", def.key),
            }
            if let Some(default) = def.default_value() {
                assert_eq!(
                    registry.validate(def.key, &default),
                    Some(true),
                    "键 {} 的缺省值必须有效",
                    def.key
                );
            }
            assert!(!def.exportable || !def.sensitive, "敏感键不得导出");
        }

        let mut unset: HashSet<_> = registry
            .fields()
            .iter()
            .filter(|def| def.default == SettingsDefault::Unset)
            .map(|def| def.key)
            .collect();
        let optional_fields = [
            ("font_family", SettingsValueType::String),
            ("font_family_cjk", SettingsValueType::String),
            ("ui_font_family", SettingsValueType::String),
            ("ui_font_size", SettingsValueType::Number),
            ("font_size", SettingsValueType::Number),
            ("background", SettingsValueType::String),
            ("theme_foreground", SettingsValueType::String),
            ("custom_theme", SettingsValueType::String),
            ("background_image", SettingsValueType::String),
            ("background_image_fit", SettingsValueType::String),
            ("background_image_alignment", SettingsValueType::String),
            ("pane_card_radius", SettingsValueType::Number),
            ("pane_card_gutter", SettingsValueType::Number),
            ("pane_card_shadow", SettingsValueType::Boolean),
            ("pane_card_divider", SettingsValueType::Number),
            ("cursor_shape", SettingsValueType::String),
            ("cursor_blink", SettingsValueType::Boolean),
            ("focus_follows_mouse", SettingsValueType::Boolean),
            ("shell", SettingsValueType::String),
            ("startup_directory", SettingsValueType::String),
        ];
        for (key, value_type) in optional_fields {
            let def = lookup(key).unwrap();
            assert_eq!(def.value_type, value_type, "{key}");
            assert!(unset.remove(key), "键 {key} 应声明 Unset");
        }
        for agent in AgentHook::ALL {
            let def = lookup(agent.settings_key()).unwrap();
            assert_eq!(def.value_type, SettingsValueType::Boolean);
            assert!(unset.remove(def.key), "键 {} 应声明 Unset", def.key);
        }
        assert!(unset.is_empty(), "意外 Unset 字段: {unset:?}");

        assert_eq!(
            lookup("theme").unwrap().default_value(),
            Some(Value::String("Linear".into()))
        );
        assert_eq!(
            lookup("multiline_paste_confirm").unwrap().default_value(),
            Some(Value::Bool(true))
        );
        assert_eq!(lookup("custom_theme").unwrap().default_value(), None);
    }

    /// 缺失、合法和非法字段分别分流，单字段回退不污染邻接值。
    #[test]
    fn settings_registry_recovers_each_field_independently() {
        let registry = SettingsRegistry::new();
        let raw = RawSettings::from_json_bytes(
            br#"{
              "appearance": {"theme": 42, "opacity": 0.87},
              "interaction": {"bell": "loud", "copy_on_select": true, "cursor_blink": null},
              "handmade": {"note": "keep"}
            }"#,
        );
        assert_eq!(
            registry.recover(&raw, "theme"),
            Some(SettingRecovery {
                value: Some(Value::String("Linear".into())),
                status: SettingRecoveryStatus::Invalid,
            })
        );
        assert_eq!(
            registry.recover(&raw, "bell"),
            Some(SettingRecovery {
                value: Some(Value::String("both".into())),
                status: SettingRecoveryStatus::Invalid,
            })
        );
        assert_eq!(
            registry.recover(&raw, "opacity"),
            Some(SettingRecovery {
                value: Some(Value::Number(0.87)),
                status: SettingRecoveryStatus::Persisted,
            })
        );
        assert_eq!(
            registry.recover(&raw, "copy_on_select"),
            Some(SettingRecovery {
                value: Some(Value::Bool(true)),
                status: SettingRecoveryStatus::Persisted,
            })
        );
        assert_eq!(
            registry.recover(&raw, "font_size"),
            Some(SettingRecovery {
                value: None,
                status: SettingRecoveryStatus::Missing,
            })
        );
        assert_eq!(
            registry.recover(&raw, "cursor_blink"),
            Some(SettingRecovery {
                value: None,
                status: SettingRecoveryStatus::Missing,
            })
        );
        assert_eq!(registry.recover(&raw, "unknown_key"), None);
    }

    /// 校验器复用领域值规则，而非只检查 JSON 外层类型。
    #[test]
    fn settings_registry_validates_domain_values_before_runtime_reads() {
        let registry = SettingsRegistry::new();
        let cases = [
            ("theme", Value::String("Nord".into()), true),
            ("theme", Value::String("NoSuchTheme".into()), false),
            ("opacity", Value::Number(2.0), true),
            ("opacity", Value::Number(f64::NAN), false),
            ("scrollback_lines", Value::Number(10_000.0), true),
            ("scrollback_lines", Value::Number(1_001.0), false),
            ("scrollback_lines", Value::Number(-1.0), false),
            ("scrollback_lines", Value::Number(1.5), false),
            ("copy_on_select", Value::Bool(true), true),
            ("copy_on_select", Value::String("true".into()), false),
        ];
        for (key, value, expected) in cases {
            assert_eq!(registry.validate(key, &value), Some(expected), "{key}");
        }
        assert_eq!(registry.validate("unknown_key", &Value::Null), None);
    }

    /// RawSettings 的缺省回退直接来自注册表，不维护第二份字面量。
    #[test]
    fn settings_registry_defaults_feed_raw_accessors() {
        let defaults = RawSettings::default();
        assert_eq!(defaults.value_or_default("theme"), "Linear");
        assert_eq!(defaults.value_or_default("language"), "system");
        assert!(defaults.bool_or_default("multiline_paste_confirm"));
        assert!(!defaults.bool_or_default("copy_on_select"));
        assert_eq!(defaults.f32_or_default("opacity"), 1.0);
        assert_eq!(defaults.f32_or_default("background_image_opacity"), 0.38);
        assert_eq!(defaults.usize_or_default("scrollback_lines"), 10_000);

        let invalid = RawSettings::from_json_bytes(
            br#"{
              "appearance": {"theme": 42, "opacity": "opaque"},
              "scrolling": {"scrollback_lines": 1001},
              "interaction": {"multiline_paste_confirm": "yes"}
            }"#,
        );
        assert_eq!(invalid.value_or_default("theme"), "Linear");
        assert_eq!(invalid.f32_or_default("opacity"), 1.0);
        assert_eq!(invalid.usize_or_default("scrollback_lines"), 10_000);
        assert!(invalid.bool_or_default("multiline_paste_confirm"));
    }

    /// 导出、重置、保存与 Agent 作用域都由登记元数据决定。
    #[test]
    fn settings_registry_export_scope_and_save_rules_are_explicit() {
        let registry = SettingsRegistry::new();
        let custom = lookup("custom_theme").unwrap();
        assert!(!custom.exportable);
        assert!(custom.resettable);
        assert_eq!(custom.save_mode, SettingsSaveMode::Automatic);

        let global = lookup(AgentHook::GLOBAL_SETTINGS_KEY).unwrap();
        assert!(!global.exportable);
        assert!(!global.resettable);
        assert_eq!(global.save_mode, SettingsSaveMode::Explicit);
        assert_eq!(global.scope, SettingsScope::Application);
        assert_eq!(global.default, SettingsDefault::Boolean(true));
        for agent in AgentHook::ALL {
            let def = lookup(agent.settings_key()).unwrap();
            assert_eq!(def.scope, SettingsScope::Agent(agent));
            assert_eq!(def.default, SettingsDefault::Unset);
            assert!(!def.exportable);
            assert!(!def.resettable);
            assert_eq!(def.save_mode, SettingsSaveMode::Explicit);
        }
        for def in KEYS
            .iter()
            .filter(|def| def.domain != SettingsDomain::AgentHooks)
        {
            assert_eq!(def.scope, SettingsScope::Application, "{}", def.key);
            assert_eq!(def.save_mode, SettingsSaveMode::Automatic, "{}", def.key);
        }

        let exported: HashSet<_> = registry.export_keys().collect();
        assert!(exported.contains("theme"));
        assert!(!exported.contains("custom_theme"));
        assert!(!exported.contains("unknown_key"));
        assert!(!exported.contains(AgentHook::GLOBAL_SETTINGS_KEY));
        for agent in AgentHook::ALL {
            assert!(!exported.contains(agent.settings_key()));
        }
        let expected: HashSet<_> = KEYS
            .iter()
            .filter(|def| def.domain != SettingsDomain::AgentHooks && def.key != "custom_theme")
            .map(|def| def.key)
            .collect();
        assert_eq!(exported, expected);
        for key in exported {
            let def = lookup(key).unwrap();
            assert!(def.exportable && !def.sensitive);
        }
    }

    /// keybind 段宽容解析:坏行跳过、行序即优先级、空串拒绝。
    #[test]
    fn keybind_segment_tolerates_bad_rows_and_keeps_order() {
        let value = crate::json::parse(
            br#"[
              {"combo": "ctrl+shift+c", "action": "copy"},
              {"combo": "ctrl+s", "action": "save_file"},
              {"combo": "", "action": "save_file"},
              {"combo": "ctrl+x"},
              {"action": "paste"},
              {"combo": 5, "action": "paste"},
              "garbage"
            ]"#,
        )
        .unwrap();
        assert_eq!(
            keybind_pairs_from_json(&value),
            [
                ("ctrl+shift+c".to_owned(), "copy".to_owned()),
                ("ctrl+s".to_owned(), "save_file".to_owned()),
            ],
            "坏行不得炸整表,合法行序原样保留"
        );
        assert!(keybind_pairs_from_json(&crate::json::Value::Bool(true)).is_empty());
    }

    /// 整表替换:旧数组不参与浅合并,其余段与未知段原位保留。
    #[test]
    fn apply_keybinds_replaces_the_segment_whole() {
        let tree = crate::json::parse(
            br#"{
              "keybind": [{"combo": "ctrl+x", "action": "old"}],
              "appearance": {"theme": "Nord"},
              "handmade": {"note": "keep"}
            }"#,
        )
        .unwrap();
        let pairs = vec![
            ("ctrl+s".to_owned(), "save_file".to_owned()),
            ("ctrl+/".to_owned(), "toggle_source".to_owned()),
        ];
        let merged = apply_keybinds(&tree, &pairs);
        assert_eq!(
            keybind_pairs_from_json(merged.get(KEYBIND_SEGMENT).unwrap()),
            pairs,
            "整表替换后只剩新数组"
        );
        assert_eq!(
            merged
                .get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Nord")
        );
        assert_eq!(
            merged
                .get("handmade")
                .unwrap()
                .get("note")
                .unwrap()
                .as_str(),
            Some("keep")
        );
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
            "raw.value_or_default(\"",
            "raw.f32(\"",
            "raw.f32_or_default(\"",
            "raw.bool_on(\"",
            "raw.bool_or_default(\"",
            "raw.usize(\"",
            "raw.usize_or_default(\"",
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
