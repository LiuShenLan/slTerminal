//! slTerminal 运行时设置(`settings.json`)与主题终端色表的权威实现。
//!
//! 分层原则:本 crate 是零生产依赖的值权威——值模型零序列化框架依赖,
//! JSON 值层由内部 `json` 模块自包含承载,文件格式与写通道硬机制
//! (.bak/保存锁/损坏三态)归应用侧适配层(M6.1)。
//!
//! 持久化契约:数据目录下 `settings.json`(路径唯一权威见 `paths`),段
//! 嵌套形态——每个设置键归属一个语义段(appearance/scrolling/…),「键 →
//! 段」映射单源在 `keys` 注册表;未知键与用户手改段在对象树原位保留。

use std::io;

mod agent_hooks;
pub use agent_hooks::AgentHook;
mod cursor_motion;
mod custom_theme;
pub use cursor_motion::CursorMotion;
pub use custom_theme::{
    IndexedPalette, TerminalThemeColors, ThemeDefinition, ThemeEffects, ThemeLayout,
    ThemeTypography, ThemeUiColors, ThemeValidationError, foreground_recommendations,
    meets_wcag_aa, wcag_contrast_ratio,
};
pub mod diagnostics;
pub mod json;
pub mod keys;
mod language;
mod ligatures;
pub use ligatures::Ligatures;
mod notifications;
pub use notifications::NotificationDuration;
mod paths;
pub use paths::{settings_dir, settings_path};
mod reset;
pub use reset::restore_default_settings;
mod scrolling;
pub use scrolling::{
    DEFAULT_SCROLL_SPEED, DEFAULT_SCROLLBACK_LINES, MAX_SCROLL_SPEED, MIN_SCROLL_SPEED,
    SCROLL_SPEED_STEP, SCROLLBACK_VALUES, normalize_scroll_speed,
};
mod themes;
pub use language::{LanguageInfo, LanguagePref};
pub use themes::{FreshPalette, ReviewedPalette};

use json::Value;

/// settings.json 写盘上界(契约数值):超出即拒绝落盘。
pub const MAX_PERSIST_BYTES: usize = 1024 * 1024;

/// `settings.json` 的宽容读取视图。键缺失/类型不符/值为 null 一律 None =
/// 「键未设置」,由调用方回退默认——逐字段宽容语义只换载体(txt → JSON),
/// 不改语义。未知键不在本结构表达,它们留在原始值树里由写通道浅合并保留。
///
/// 段嵌套:读取经 `keys` 注册表解析「键 → 段」,未登记键读不出。
#[derive(Default)]
pub struct RawSettings {
    tree: Value,
}

impl RawSettings {
    pub fn load() -> Self {
        Self::try_load().unwrap_or_default()
    }

    /// 授权路径(安装器)不许把读取失败当默认:IO 错误与损坏文件都是 Err。
    pub fn try_load() -> io::Result<Self> {
        Self::try_load_at(&settings_path())
    }

    fn try_load_at(path: &std::path::Path) -> io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => json::parse(&bytes)
                .map(Self::from_tree)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    /// 宽容入口:解析失败 = 空树(不 panic),所有键 None → 全默认。
    pub fn from_json_bytes(bytes: &[u8]) -> Self {
        json::parse(bytes).map(Self::from_tree).unwrap_or_default()
    }

    /// 根非对象 = 空树(损坏但可解析的文档不炸)。
    fn from_tree(tree: Value) -> Self {
        match tree {
            Value::Object(_) => Self { tree },
            _ => Self::default(),
        }
    }

    /// 段内查找:键 → 注册表段 → 段内成员。
    fn segment_value(&self, key: &str) -> Option<&Value> {
        let def = keys::lookup(key)?;
        self.tree.get(def.domain.segment())?.get(key)
    }

    /// 字符串值;非字符串/空串 → None(空串 = 未设置)。
    pub fn value(&self, key: &str) -> Option<&str> {
        self.segment_value(key)?
            .as_str()
            .filter(|value| !value.is_empty())
    }

    /// 数值;NaN/∞ 由调用方钳(见 scrolling 归一)。
    pub fn f32(&self, key: &str) -> Option<f32> {
        self.segment_value(key)?
            .as_f64()
            .map(|number| number as f32)
    }

    /// 非负整数;小数/负数/越界 → None。
    pub fn usize(&self, key: &str) -> Option<usize> {
        let number = self.segment_value(key)?.as_f64()?;
        if number.is_finite()
            && number.fract() == 0.0
            && number >= 0.0
            && number <= usize::MAX as f64
        {
            Some(number as usize)
        } else {
            None
        }
    }

    /// 真布尔直取;字符串 "true" 等拼写不是合法值(类型不符回默认)。
    pub fn bool_on(&self, key: &str) -> Option<bool> {
        self.segment_value(key)?.as_bool()
    }

    /// 文件树中实际出现且已登记的键名遍历。
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        keys::KEYS
            .iter()
            .filter(|def| self.segment_value(def.key).is_some())
            .map(|def| def.key)
    }
}

/// 读-改-写语义:已有键替换、缺失键追加、未知键/未知段在对象树原位保留。
/// 逐键落到注册表归属段内(段级浅合并);未登记键跳过——白名单硬校验归
/// M6.1 写通道,库态纯函数保守不写入。
pub fn apply_updates(tree: &Value, updates: &[(&str, Value)]) -> Value {
    let mut root = match tree {
        Value::Object(_) => tree.clone(),
        _ => Value::empty_object(),
    };
    for (key, value) in updates {
        let Some(def) = keys::lookup(key) else {
            continue;
        };
        let segment = def.domain.segment();
        if !matches!(root.get(segment), Some(Value::Object(_))) {
            root.set(segment, Value::empty_object());
        }
        if let Some(section) = root.object_mut().and_then(|members| {
            members
                .iter_mut()
                .find(|(name, _)| name == segment)
                .map(|(_, value)| value)
        }) {
            section.set(key, value.clone());
        }
    }
    root
}

/// 统一写通道(M1 最小原子写形态):读现有 → 浅合并 → 临时文件 + rename。
/// 损坏文件解析失败 = Err 传播,禁覆盖;.bak/保存锁/损坏三态归 M6.1 并入。
pub fn persist_keys(updates: &[(&str, Value)]) -> io::Result<()> {
    persist_keys_at(&settings_path(), updates)
}

fn persist_keys_at(path: &std::path::Path, updates: &[(&str, Value)]) -> io::Result<()> {
    let tree = match std::fs::read(path) {
        Ok(bytes) => json::parse(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Value::empty_object(),
        Err(error) => return Err(error),
    };
    let merged = apply_updates(&tree, updates);
    let text = json::to_string_pretty(&merged);
    if text.len() > MAX_PERSIST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "settings.json exceeds 1MB",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // 同目录临时文件 + rename:崩在半写的 settings.json 对外不可见。
    static TEMP_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
    let result = std::fs::write(&temporary, text).and_then(|_| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// 终端/UI 调色板通用三元色。
pub type Rgb8 = [u8; 3];
/// 带显式 alpha 的 UI 语义槽位色。
pub type Rgba8 = [u8; 4];

/// session schema 引用的可持久化颜色(05 篇消费:`TabSession.color`)。
/// 值模型零序列化:落盘形态(三元数组)由应用侧适配层表达。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// 主题标识;`settings.json` 的 `theme` 键持久化 [`Self::prompt_name`]。
/// 目录即合同:可选内置主题 = 暗色子集 + Linear 默认(D06-3)。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ThemeName {
    BreezeDark,
    MintDark,
    Nord,
    CatppuccinMocha,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    GlassDark,
    /// slTerminal 自有默认主题(UI 重设计定稿方案的暗色值迁入)。
    #[default]
    Linear,
}

impl ThemeName {
    /// 严格匹配持久化名;未知值(含退休/亮主题名)返回 None,由调用方回默认。
    pub fn from_prompt_name(name: &str) -> Option<Self> {
        Some(match name {
            "BreezeDark" => Self::BreezeDark,
            "MintDark" => Self::MintDark,
            "Nord" => Self::Nord,
            "CatppuccinMocha" => Self::CatppuccinMocha,
            "CatppuccinFrappe" => Self::CatppuccinFrappe,
            "CatppuccinMacchiato" => Self::CatppuccinMacchiato,
            "GlassDark" => Self::GlassDark,
            "Linear" => Self::Linear,
            _ => return None,
        })
    }

    /// 持久化名 = 展示名。
    pub const fn prompt_name(self) -> &'static str {
        match self {
            Self::BreezeDark => "BreezeDark",
            Self::MintDark => "MintDark",
            Self::Nord => "Nord",
            Self::CatppuccinMocha => "CatppuccinMocha",
            Self::CatppuccinFrappe => "CatppuccinFrappe",
            Self::CatppuccinMacchiato => "CatppuccinMacchiato",
            Self::GlassDark => "GlassDark",
            Self::Linear => "Linear",
        }
    }

    /// 主题对终端色表的全部影响。`exact: None` = 主题只声明背景,调用方
    /// 保留用户配置或宿主默认;当前目录全部成员都带完整色表。
    pub fn term_theme(self) -> TermTheme {
        match self {
            // Nord 色表逐字节照抄上游合同(Arctic Ice Studio 公开色板)。
            Self::Nord => TermTheme {
                background: [0x2e, 0x34, 0x40],
                exact: Some(ExactTermColors {
                    foreground: [0xf1, 0xf6, 0xff],
                    ansi: [
                        [0x3b, 0x42, 0x52],
                        [0xbf, 0x61, 0x6a],
                        [0xa3, 0xbe, 0x8c],
                        [0xeb, 0xcb, 0x8b],
                        [0x81, 0xa1, 0xc1],
                        [0xb4, 0x8e, 0xad],
                        [0x88, 0xc0, 0xd0],
                        [0xe5, 0xe9, 0xf0],
                        [0x4c, 0x56, 0x6a],
                        [0xbf, 0x61, 0x6a],
                        [0xa3, 0xbe, 0x8c],
                        [0xeb, 0xcb, 0x8b],
                        [0x81, 0xa1, 0xc1],
                        [0xb4, 0x8e, 0xad],
                        [0x8f, 0xbc, 0xbb],
                        [0xec, 0xef, 0xf4],
                    ],
                    cursor: Some([0xe5, 0xe9, 0xf0]),
                    cursor_text: Some([0x2e, 0x34, 0x40]),
                    cursor_stroke: Some([0x88, 0xc0, 0xd0]),
                    selection_foreground: Some([0x2e, 0x34, 0x40]),
                    selection_background: Some([0xe5, 0xe9, 0xf0]),
                }),
            },
            // Linear:终端段色表逐字节取自旧栈配色契约(附录 A 定稿值);
            // selection_background 是 rgba(accent, 0.28) 在终端底上的实色合成。
            Self::Linear => TermTheme {
                background: [0x0a, 0x0a, 0x0b],
                exact: Some(ExactTermColors {
                    foreground: [0xcf, 0xca, 0xc1],
                    ansi: [
                        [0x0a, 0x0a, 0x0b],
                        [0xd9, 0x70, 0x6b],
                        [0x93, 0xb5, 0x73],
                        [0xd6, 0xb2, 0x5e],
                        [0x7f, 0xa8, 0xe8],
                        [0xb4, 0x8c, 0xe0],
                        [0x6f, 0xbf, 0xc4],
                        [0xcf, 0xca, 0xc1],
                        [0x7d, 0x78, 0x71],
                        [0xe2, 0x87, 0x7f],
                        [0xa8, 0xc9, 0x8d],
                        [0xe3, 0xc6, 0x7f],
                        [0x9d, 0xbf, 0xee],
                        [0xc6, 0xa6, 0xe8],
                        [0x8d, 0xd0, 0xd4],
                        [0xf0, 0xed, 0xe8],
                    ],
                    cursor: Some([0x6e, 0x9f, 0xf2]),
                    cursor_text: Some([0x0a, 0x0a, 0x0b]),
                    cursor_stroke: Some([0x6e, 0x9f, 0xf2]),
                    selection_foreground: Some([0xf0, 0xed, 0xe8]),
                    selection_background: Some([38, 52, 76]),
                }),
            },
            fresh => themes::fresh_terminal(fresh),
        }
    }

    /// 内置主题共享「铺满」卡几何;窗口/控件圆角与此分离。
    /// 显式用户几何仍可配置;选择预设即恢复其几何。
    pub fn card_geometry(self) -> ThemeCardGeometry {
        ThemeCardGeometry {
            radius: 0.0,
            gutter: 0.0,
            shadow: false,
            divider: 1.0,
        }
    }
}

/// 主题明确携带的终端色表。`None` 字段 = 主题未声明该值,调用方保留
/// 用户配置或宿主默认。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExactTermColors {
    pub foreground: Rgb8,
    pub ansi: [Rgb8; 16],
    pub cursor: Option<Rgb8>,
    pub cursor_text: Option<Rgb8>,
    pub cursor_stroke: Option<Rgb8>,
    pub selection_foreground: Option<Rgb8>,
    pub selection_background: Option<Rgb8>,
}

/// 一个主题对终端色表的全部影响(仅暗色:无亮主题替换语义)。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TermTheme {
    pub background: Rgb8,
    pub exact: Option<ExactTermColors>,
}

// ---- 键域枚举族四件套:from_settings(宽容归一)/ settings_value(稳定写盘
// 值)/ 往返测试 / ALL+VALUES(GUI 枚举单源)。值域 = VALUES,别名不兼容——
// 零迁移原则下无历史拼写需要承接,新设置键一律按此形态落地。

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CursorShapeName {
    Block,
    Beam,
    Underline,
    Hollow,
}

impl CursorShapeName {
    pub const ALL: [Self; 4] = [Self::Block, Self::Beam, Self::Underline, Self::Hollow];
    pub const VALUES: [&'static str; 4] = ["block", "beam", "underline", "hollow"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "block" => Some(Self::Block),
            "beam" => Some(Self::Beam),
            "underline" => Some(Self::Underline),
            "hollow" => Some(Self::Hollow),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Beam => "beam",
            Self::Underline => "underline",
            Self::Hollow => "hollow",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AcceptKeyName {
    Right,
    Tab,
    #[default]
    Both,
}

impl AcceptKeyName {
    pub const ALL: [Self; 3] = [Self::Right, Self::Tab, Self::Both];
    pub const VALUES: [&'static str; 3] = ["right", "tab", "both"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "right" => Some(Self::Right),
            "tab" => Some(Self::Tab),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Tab => "tab",
            Self::Both => "both",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CompletionStyleName {
    #[default]
    Inline,
    Popup,
    Hybrid,
}

impl CompletionStyleName {
    pub const ALL: [Self; 3] = [Self::Inline, Self::Popup, Self::Hybrid];
    pub const VALUES: [&'static str; 3] = ["inline", "popup", "hybrid"];

    pub fn cycle(self) -> Self {
        match self {
            Self::Inline => Self::Popup,
            Self::Popup => Self::Hybrid,
            Self::Hybrid => Self::Inline,
        }
    }

    /// 混合模式仅在用户请求后显示列表,候选生成仍复用已有两种呈现。
    pub fn active_style(self, popup_requested: bool) -> Self {
        match self {
            Self::Hybrid if popup_requested => Self::Popup,
            Self::Hybrid => Self::Inline,
            style => style,
        }
    }

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inline" => Some(Self::Inline),
            "popup" => Some(Self::Popup),
            "hybrid" => Some(Self::Hybrid),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Popup => "popup",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TabRevealName {
    #[default]
    Slide,
    Instant,
}

impl TabRevealName {
    pub const ALL: [Self; 2] = [Self::Slide, Self::Instant];
    pub const VALUES: [&'static str; 2] = ["slide", "instant"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "slide" => Some(Self::Slide),
            "instant" => Some(Self::Instant),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Slide => "slide",
            Self::Instant => "instant",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DensityName {
    #[default]
    Standard,
    Compact,
}

impl DensityName {
    pub const ALL: [Self; 2] = [Self::Standard, Self::Compact];
    pub const VALUES: [&'static str; 2] = ["standard", "compact"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Compact => "compact",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NewTabPositionName {
    #[default]
    AfterCurrent,
    End,
}

impl NewTabPositionName {
    pub const ALL: [Self; 2] = [Self::AfterCurrent, Self::End];
    pub const VALUES: [&'static str; 2] = ["after_current", "end"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "after_current" => Some(Self::AfterCurrent),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::AfterCurrent => "after_current",
            Self::End => "end",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CellWidthModeName {
    #[default]
    Compact,
    Relaxed,
}

impl CellWidthModeName {
    pub const ALL: [Self; 2] = [Self::Compact, Self::Relaxed];
    pub const VALUES: [&'static str; 2] = ["compact", "relaxed"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "compact" => Some(Self::Compact),
            "relaxed" => Some(Self::Relaxed),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Relaxed => "relaxed",
        }
    }
}

/// 侧栏版本控制视图的数据源:自动探测或只认 Git(SVN 档已裁)。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum VcsDisplayName {
    #[default]
    Auto,
    Git,
}

impl VcsDisplayName {
    pub const ALL: [Self; 2] = [Self::Auto, Self::Git];
    pub const VALUES: [&'static str; 2] = ["auto", "git"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "git" => Some(Self::Git),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Git => "git",
        }
    }
}

/// 终端 BEL(`^G`)的通知方式:关 / 闪烁 / 声音 / 两者。缺省两者——
/// AI CLI 回合结束的可听提示默认开。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BellModeName {
    None,
    Visual,
    Audible,
    #[default]
    Both,
}

impl BellModeName {
    pub const ALL: [Self; 4] = [Self::None, Self::Visual, Self::Audible, Self::Both];
    pub const VALUES: [&'static str; 4] = ["none", "visual", "audible", "both"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "visual" => Some(Self::Visual),
            "audible" => Some(Self::Audible),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Visual => "visual",
            Self::Audible => "audible",
            Self::Both => "both",
        }
    }

    pub fn visual(self) -> bool {
        matches!(self, Self::Visual | Self::Both)
    }

    pub fn audible(self) -> bool {
        matches!(self, Self::Audible | Self::Both)
    }
}

/// 窗口背景材质。**按 DWM 每帧成本递增排列**,不是质量递进——五者是五套
/// 成本模型,不存在「越靠后越好」(D06-4 值域一次到位:五材质枚举 +
/// opacity 标量,无 bool 占位):
///
/// - `None`:无材质。窗口按不透明度直接透出后方内容,不模糊。
/// - `Mica`:Windows 系统壁纸 backdrop;由 DWM 合成,不含后方其他窗口。
/// - `MicaAlt`:Mica 的更强色调变体,适合带标签栏的窗口。
/// - `Aero`:实时模糊窗口**后方的真实内容**,并叠加深色玻璃色调。
/// - `Acrylic`:实时模糊 + tint/噪点/饱和度,最贵。
///
/// 默认档必须是性能安全的那个(高质量档实测会把 dwm.exe 顶到高占用),
/// 想要实时透视的用户显式选择。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BlurMode {
    #[default]
    None,
    Mica,
    MicaAlt,
    Aero,
    Acrylic,
}

impl BlurMode {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Mica,
        Self::MicaAlt,
        Self::Aero,
        Self::Acrylic,
    ];
    pub const VALUES: [&'static str; 5] = ["none", "mica", "mica-alt", "aero", "acrylic"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "mica" => Some(Self::Mica),
            "mica-alt" => Some(Self::MicaAlt),
            "aero" => Some(Self::Aero),
            "acrylic" => Some(Self::Acrylic),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mica => "mica",
            Self::MicaAlt => "mica-alt",
            Self::Aero => "aero",
            Self::Acrylic => "acrylic",
        }
    }

    /// 是否需要窗口内容保留透明像素(除 `None` 外都要,否则材质被自己盖住)。
    pub fn enabled(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// 新 UI 消费的运行时设置,单一权威大结构。`Option` 字段 None = 键未
/// 设置,调用方自选回退。
#[derive(Clone)]
pub struct RuntimeSettings {
    pub language: LanguagePref,
    pub theme: ThemeName,
    pub font_family: Option<String>,
    pub font_family_cjk: Option<String>,
    pub ui_font_family: Option<String>,
    pub ui_font_size_px: Option<f32>,
    /// 逻辑像素(设置页 spinner 与 Ctrl+滚轮持久化时已除 scale factor)。
    pub font_size_px: Option<f32>,
    /// Ctrl+滚轮缩放终端字号;关闭时该手势被整体消费。默认开。
    pub ctrl_wheel_font_zoom: bool,
    /// 默认开;Theme 档跟随主题 typography 声明。
    pub ligatures: Ligatures,
    pub cursor_shape: Option<CursorShapeName>,
    pub cursor_blink: Option<bool>,
    /// 平滑光标移动:显式 opt-in,默认关。
    pub cursor_motion: CursorMotion,
    pub copy_on_select: bool,
    /// 新建终端的历史容量上限;不动已打开会话。
    pub scrollback_lines: usize,
    /// 滚轮倍率;像素级触控板输入与此独立。
    pub scroll_speed: f32,
    pub focus_follows_mouse: Option<bool>,
    /// 非聚焦分屏压暗,默认开。
    pub dim_inactive_panes: bool,
    /// 裸 shell 风险粘贴确认:开 = 换行/提权命令/控制字符先确认。
    pub multiline_paste_confirm: bool,
    /// 标签页关闭按钮是否渲染:关 = 不渲染,仍可用中键关闭。
    pub tab_close_visible: bool,
    /// 新建本地终端是否写入 Windows 系统代理环境变量,默认关。
    pub terminal_proxy: bool,
    /// 新进程是否刷新注册表环境变量;关 = 继承启动进程环境。
    pub refresh_environment: bool,
    /// 默认 shell 的原始 id 往返;解析归 02 篇 shell 检测层。
    pub shell: Option<String>,
    pub startup_directory: Option<String>,
    /// AI 内联补全 ghost text。
    pub ghost: bool,
    pub accept: AcceptKeyName,
    pub completion_style: CompletionStyleName,
    /// 全宽字形(CJK 等)bold run 用 Regular 字形(粗体提亮不加粗)。
    pub cjk_bold_regular: bool,
    pub tab_reveal: TabRevealName,
    pub density: DensityName,
    pub new_tab_position: NewTabPositionName,
    pub cell_width_mode: CellWidthModeName,
    /// 侧栏版本控制视图数据源(auto/git)。
    pub vcs_display: VcsDisplayName,
    /// 终端 BEL:关 / 闪烁 / 声音 / 两者(缺省两者)。
    pub bell: BellModeName,
    /// 应用内 AI 消息卡片;系统通知与终端/标签状态相互独立。
    pub ai_toasts: bool,
    /// 应用内卡片存活时长;Default = 不覆盖各类自带时长。
    pub notification_duration: NotificationDuration,
    /// 新会话欢迎屏 fastfetch(默认关:启动速度优先)。
    pub fetch: bool,
    /// 启动后检查 GitHub Releases;关闭时仍可手动检查。
    pub auto_check_updates: bool,
    /// 后台包下载开关;不授予安装权限。
    pub auto_download_updates: bool,
    pub keep_session: bool,
    pub restore_session: bool,
    pub resume_ai: bool,
    /// 常驻系统托盘图标。
    pub tray: bool,
    /// 托盘可用且启用时,首窗以隐藏形态启动。
    pub silent_start: bool,
    /// 开机启动;归 09 篇消费。
    pub launch_at_login: bool,
    /// 窗口背景材质(D06-4:五材质枚举,默认 None)。
    pub blur: BlurMode,
    pub opacity: f32,
    /// 「显式值权威」三元模式:主题默认可填充未设置值,用户显式写过
    /// (哪怕恰是默认)必须保持权威。
    opacity_explicit: bool,
    blur_explicit: bool,
    /// 终端背景覆盖色(设置页取色器写入,优先于主题背景)。
    pub background: Option<Rgb8>,
    /// 终端前景覆盖色;切主题时可由调用方清除以恢复主题内置前景。
    pub theme_foreground: Option<Rgb8>,
    /// 自定义主题标识/载荷。设置层只保存值,文件加载与格式解码归
    /// theme_library(应用侧)。
    pub custom_theme: Option<String>,
    /// 壁纸路径(空 = 无壁纸);fit/alignment 存原文,解析归渲染层。
    pub background_image: Option<String>,
    /// 壁纸自身透明度,独立于窗口 opacity(保文字对比度)。
    pub background_image_opacity: f32,
    pub background_image_fit: Option<String>,
    pub background_image_alignment: Option<String>,
    /// 壁纸铺满整窗(含侧栏/标题栏)而非仅终端卡。
    pub background_image_cover_chrome: bool,
    pub panel_resize: bool,
    /// 终端卡圆角(逻辑像素)。`None` = 跟随主题自带几何
    /// ([`ThemeName::card_geometry`])。
    pub pane_card_radius: Option<f32>,
    /// 终端卡与周围 chrome 的卡缝(逻辑像素)。**只作用于左/右/下三边**,
    /// 上边恒为零(侧栏/终端卡/右侧抽屉顶边都贴 chrome 下沿)。
    pub pane_card_gutter: Option<f32>,
    /// 终端卡投影;关掉 + 半径与卡缝归零 = 「铺满到边」形态。
    pub pane_card_shadow: Option<bool>,
    /// 侧栏与终端之间的竖线宽度(0 = 不画);卡缝归零形态的结构分界。
    pub pane_card_divider: Option<f32>,
}

/// 主题自带的终端卡几何。四键一组:radius(圆角)、gutter(卡缝)、
/// shadow(投影)、divider(分界竖线);卡几何跟主题走。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeCardGeometry {
    pub radius: f32,
    pub gutter: f32,
    pub shadow: bool,
    pub divider: f32,
}

/// 终端卡几何的单一真源——半径/卡缝曾各写一份字面量,两处漂移就是
/// 那圈白边的来源;默认值只许从这里取。
///
/// 上限 28 是半径不超过卡最窄边一半之前的实用上界;0 与卡缝 0 搭配 =
/// 「铺满到窗口边、无圆角」形态。
pub const DEFAULT_PANE_CARD_RADIUS: f32 = 14.0;
pub const MIN_PANE_CARD_RADIUS: f32 = 0.0;
pub const MAX_PANE_CARD_RADIUS: f32 = 28.0;
pub const DEFAULT_PANE_CARD_GUTTER: f32 = 8.0;
pub const MIN_PANE_CARD_GUTTER: f32 = 0.0;
pub const MAX_PANE_CARD_GUTTER: f32 = 32.0;
pub const MAX_PANE_CARD_DIVIDER: f32 = 4.0;

/// 壁纸默认透明度(壁纸压不过文字)。
pub const DEFAULT_BACKGROUND_IMAGE_OPACITY: f32 = 0.38;

impl RuntimeSettings {
    pub fn load() -> Self {
        Self::from_raw(&RawSettings::load())
    }

    /// 逐键宽容解析:未知值回默认、范围钳制、None = 未设置。
    pub fn from_raw(raw: &RawSettings) -> Self {
        let blur = raw
            .value("blur")
            .and_then(BlurMode::from_settings)
            .unwrap_or_default();
        let blur_explicit = raw
            .value("blur")
            .and_then(BlurMode::from_settings)
            .is_some();
        // 主题色默认不透明;材质选择从不主动降低不透明度。
        let opacity = raw.f32("opacity").unwrap_or(1.0).clamp(0.0, 1.0);
        let opacity_explicit = raw.f32("opacity").is_some();

        Self {
            language: raw
                .value("language")
                .and_then(LanguagePref::from_settings)
                .unwrap_or_default(),
            theme: raw
                .value("theme")
                .and_then(ThemeName::from_prompt_name)
                .unwrap_or_default(),
            font_family: raw.value("font_family").map(str::to_owned),
            font_family_cjk: raw.value("font_family_cjk").map(str::to_owned),
            ui_font_family: raw.value("ui_font_family").map(str::to_owned),
            ui_font_size_px: raw.f32("ui_font_size").map(|size| size.clamp(10.0, 24.0)),
            font_size_px: raw.f32("font_size").map(|size| size.clamp(4.0, 96.0)),
            ctrl_wheel_font_zoom: raw.bool_on("ctrl_wheel_font_zoom").unwrap_or(true),
            ligatures: raw
                .value("ligatures")
                .and_then(Ligatures::from_settings)
                .unwrap_or_default(),
            cursor_shape: raw
                .value("cursor_shape")
                .and_then(CursorShapeName::from_settings),
            cursor_blink: raw.bool_on("cursor_blink"),
            cursor_motion: raw
                .value("cursor_motion")
                .and_then(CursorMotion::from_settings)
                .unwrap_or_default(),
            copy_on_select: raw.bool_on("copy_on_select").unwrap_or(false),
            scrollback_lines: scrolling::scrollback_lines(raw.usize("scrollback_lines")),
            scroll_speed: normalize_scroll_speed(
                raw.f32("scroll_speed").unwrap_or(DEFAULT_SCROLL_SPEED),
            ),
            focus_follows_mouse: raw.bool_on("focus_follows_mouse"),
            dim_inactive_panes: raw.bool_on("dim_inactive_panes").unwrap_or(true),
            multiline_paste_confirm: raw.bool_on("multiline_paste_confirm").unwrap_or(true),
            tab_close_visible: raw.bool_on("tab_close_visible").unwrap_or(true),
            terminal_proxy: raw.bool_on("terminal_proxy").unwrap_or(false),
            refresh_environment: raw.bool_on("refresh_environment").unwrap_or(true),
            shell: raw.value("shell").map(str::to_owned),
            startup_directory: raw.value("startup_directory").map(str::to_owned),
            ghost: raw.bool_on("ghost").unwrap_or(true),
            accept: raw
                .value("accept")
                .and_then(AcceptKeyName::from_settings)
                .unwrap_or_default(),
            completion_style: raw
                .value("completion_style")
                .and_then(CompletionStyleName::from_settings)
                .unwrap_or_default(),
            cjk_bold_regular: raw.bool_on("cjk_bold_regular").unwrap_or(true),
            tab_reveal: raw
                .value("tab_reveal")
                .and_then(TabRevealName::from_settings)
                .unwrap_or_default(),
            density: raw
                .value("density")
                .and_then(DensityName::from_settings)
                .unwrap_or_default(),
            new_tab_position: raw
                .value("new_tab_position")
                .and_then(NewTabPositionName::from_settings)
                .unwrap_or_default(),
            cell_width_mode: raw
                .value("cell_width_mode")
                .and_then(CellWidthModeName::from_settings)
                .unwrap_or_default(),
            vcs_display: raw
                .value("vcs_display")
                .and_then(VcsDisplayName::from_settings)
                .unwrap_or_default(),
            bell: raw
                .value("bell")
                .and_then(BellModeName::from_settings)
                .unwrap_or_default(),
            ai_toasts: raw.bool_on("ai_toasts").unwrap_or(true),
            notification_duration: raw
                .value("notification_duration")
                .and_then(NotificationDuration::from_settings)
                .unwrap_or_default(),
            fetch: raw.bool_on("fetch").unwrap_or(false),
            auto_check_updates: raw.bool_on("auto_check_updates").unwrap_or(true),
            auto_download_updates: raw.bool_on("auto_download_updates").unwrap_or(false),
            keep_session: raw.bool_on("keep_session").unwrap_or(false),
            restore_session: raw.bool_on("restore_session").unwrap_or(true),
            resume_ai: raw.bool_on("resume_ai").unwrap_or(true),
            tray: raw.bool_on("tray").unwrap_or(true),
            silent_start: raw.bool_on("silent_start").unwrap_or(false),
            launch_at_login: raw.bool_on("launch_at_login").unwrap_or(false),
            blur,
            opacity,
            opacity_explicit,
            blur_explicit,
            background: raw.value("background").and_then(parse_hex_rgb),
            theme_foreground: raw.value("theme_foreground").and_then(parse_hex_rgb),
            custom_theme: raw.value("custom_theme").map(str::to_owned),
            background_image: raw
                .value("background_image")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            background_image_opacity: raw
                .f32("background_image_opacity")
                .map(|opacity| opacity.clamp(0.0, 1.0))
                .unwrap_or(DEFAULT_BACKGROUND_IMAGE_OPACITY),
            background_image_fit: raw.value("background_image_fit").map(str::to_owned),
            background_image_alignment: raw.value("background_image_alignment").map(str::to_owned),
            background_image_cover_chrome: raw
                .bool_on("background_image_cover_chrome")
                .unwrap_or(false),
            panel_resize: raw.bool_on("panel_resize").unwrap_or(true),
            // 这四项一律保留 `Option`:`None` 就是「用户没设过」,让主题默认
            // 生效;若在这里 unwrap 成具体数字,切主题便再也换不了形态。
            pane_card_radius: raw
                .f32("pane_card_radius")
                .map(|radius| radius.clamp(MIN_PANE_CARD_RADIUS, MAX_PANE_CARD_RADIUS)),
            pane_card_gutter: raw
                .f32("pane_card_gutter")
                .map(|gutter| gutter.clamp(MIN_PANE_CARD_GUTTER, MAX_PANE_CARD_GUTTER)),
            pane_card_shadow: raw.bool_on("pane_card_shadow"),
            pane_card_divider: raw
                .f32("pane_card_divider")
                .map(|divider| divider.clamp(0.0, MAX_PANE_CARD_DIVIDER)),
        }
    }

    /// 用户显式写过合法 opacity 值(含显式 1.0)时保持权威。
    pub fn opacity_is_explicit(&self) -> bool {
        self.opacity_explicit
    }

    /// 用户显式写过合法 blur 值(含显式 none)时保持权威。
    pub fn blur_is_explicit(&self) -> bool {
        self.blur_explicit
    }
}

/// 解析 `#rgb` 或 `#rrggbb`;# 前缀可省,写盘统一六位小写。
/// 非 hex/长度错/含中文一律 None。
pub fn parse_hex_rgb(value: &str) -> Option<Rgb8> {
    let hex = value.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
        return Some([r, g, b]);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([r, g, b])
}

/// 格式化为 `#rrggbb`(写盘格式)。
pub fn format_hex_rgb(rgb: Rgb8) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod runtime_settings_tests {
    use super::*;

    fn raw(text: &str) -> RawSettings {
        RawSettings::from_json_bytes(text.as_bytes())
    }

    #[test]
    fn defaults_when_document_is_absent_or_junk() {
        for text in [
            "",
            "not json at all",
            "[1,2,3]",
            "{\"appearance\": {\"theme\": \"NoSuchTheme\"}}",
        ] {
            let settings = RuntimeSettings::from_raw(&raw(text));
            assert_eq!(settings.language, LanguagePref::System, "{text}");
            assert_eq!(settings.theme, ThemeName::Linear, "{text}");
            assert_eq!(settings.font_family, None);
            assert_eq!(settings.font_size_px, None);
            assert!(!settings.copy_on_select);
            assert!(settings.multiline_paste_confirm, "多行粘贴确认默认开");
            assert!(settings.tab_close_visible);
            assert!(!settings.terminal_proxy);
            assert!(settings.refresh_environment);
            assert!(settings.ghost);
            assert_eq!(settings.accept, AcceptKeyName::Both);
            assert_eq!(settings.completion_style, CompletionStyleName::Inline);
            assert!(settings.cjk_bold_regular);
            assert_eq!(settings.tab_reveal, TabRevealName::Slide);
            assert_eq!(settings.density, DensityName::Standard);
            assert_eq!(settings.new_tab_position, NewTabPositionName::AfterCurrent);
            assert_eq!(settings.cell_width_mode, CellWidthModeName::Compact);
            assert_eq!(settings.vcs_display, VcsDisplayName::Auto);
            assert_eq!(settings.bell, BellModeName::Both);
            assert!(settings.ai_toasts);
            assert_eq!(
                settings.notification_duration,
                NotificationDuration::Default
            );
            assert!(!settings.fetch);
            assert!(settings.auto_check_updates);
            assert!(!settings.auto_download_updates);
            assert!(!settings.keep_session);
            assert!(settings.restore_session);
            assert!(settings.resume_ai);
            assert!(settings.tray);
            assert!(!settings.silent_start);
            assert!(!settings.launch_at_login);
            assert_eq!(settings.blur, BlurMode::None);
            assert_eq!(settings.opacity, 1.0);
            assert_eq!(settings.background, None);
            assert_eq!(settings.theme_foreground, None);
            assert_eq!(settings.custom_theme, None);
            assert_eq!(settings.background_image, None);
            assert_eq!(
                settings.background_image_opacity,
                DEFAULT_BACKGROUND_IMAGE_OPACITY
            );
            assert!(settings.panel_resize);
            assert_eq!(settings.scrollback_lines, DEFAULT_SCROLLBACK_LINES);
            assert_eq!(settings.scroll_speed, 1.0);
            assert!(settings.ctrl_wheel_font_zoom);
            assert_eq!(settings.ligatures, Ligatures::On);
            assert_eq!(settings.cursor_motion, CursorMotion::Off);
        }
    }

    #[test]
    fn parses_real_settings_shape_across_segments() {
        let settings = RuntimeSettings::from_raw(&raw(r##"{
  "appearance": {
    "language": "zh-CN",
    "theme": "Nord",
    "font_family": "CaskaydiaCove Nerd Font",
    "font_family_cjk": "Microsoft YaHei",
    "ui_font_family": "Arial",
    "ui_font_size": 18,
    "font_size": 16.3,
    "density": "compact",
    "tab_reveal": "instant",
    "tab_close_visible": false,
    "cjk_bold_regular": false,
    "cell_width_mode": "relaxed",
    "vcs_display": "git",
    "opacity": 0.87,
    "blur": "mica",
    "background": "#101216",
    "theme_foreground": "#d6dae6",
    "custom_theme": "my-night",
    "background_image": "C:/wallpapers/night.png",
    "background_image_opacity": 0.5,
    "background_image_fit": "cover",
    "background_image_alignment": "center",
    "background_image_cover_chrome": true,
    "pane_card_radius": 999,
    "pane_card_gutter": 12,
    "pane_card_shadow": true,
    "pane_card_divider": 99
  },
  "scrolling": {
    "scrollback_lines": 50000,
    "scroll_speed": 1.75
  },
  "interaction": {
    "cursor_shape": "beam",
    "cursor_blink": true,
    "cursor_motion": "smooth",
    "copy_on_select": true,
    "focus_follows_mouse": true,
    "dim_inactive_panes": false,
    "multiline_paste_confirm": false,
    "ligatures": "theme",
    "bell": "audible",
    "ctrl_wheel_font_zoom": false,
    "ghost": false,
    "accept": "tab",
    "completion_style": "popup",
    "new_tab_position": "end",
    "panel_resize": false
  },
  "session": {
    "shell": "pwsh",
    "startup_directory": "D:/work",
    "keep_session": true,
    "restore_session": false,
    "resume_ai": false,
    "silent_start": true,
    "launch_at_login": true,
    "fetch": true
  },
  "system": {
    "terminal_proxy": true,
    "refresh_environment": false,
    "ai_toasts": false,
    "notification_duration": "90",
    "tray": false,
    "auto_check_updates": false,
    "auto_download_updates": true
  }
}"##));
        assert_eq!(settings.language, LanguagePref::ZhCn);
        assert_eq!(settings.theme, ThemeName::Nord);
        assert_eq!(
            settings.font_family.as_deref(),
            Some("CaskaydiaCove Nerd Font")
        );
        assert_eq!(settings.font_family_cjk.as_deref(), Some("Microsoft YaHei"));
        assert_eq!(settings.ui_font_family.as_deref(), Some("Arial"));
        assert_eq!(settings.ui_font_size_px, Some(18.0));
        // font_size 键存逻辑像素,不做 pt 换算。
        assert_eq!(settings.font_size_px, Some(16.3));
        assert_eq!(settings.density, DensityName::Compact);
        assert_eq!(settings.tab_reveal, TabRevealName::Instant);
        assert!(!settings.tab_close_visible);
        assert!(!settings.cjk_bold_regular);
        assert_eq!(settings.cell_width_mode, CellWidthModeName::Relaxed);
        assert_eq!(settings.vcs_display, VcsDisplayName::Git);
        assert!((settings.opacity - 0.87).abs() < 1e-6);
        assert_eq!(settings.blur, BlurMode::Mica);
        assert_eq!(settings.background, Some([0x10, 0x12, 0x16]));
        assert_eq!(settings.theme_foreground, Some([0xd6, 0xda, 0xe6]));
        assert_eq!(settings.custom_theme.as_deref(), Some("my-night"));
        assert_eq!(
            settings.background_image.as_deref(),
            Some("C:/wallpapers/night.png")
        );
        assert_eq!(settings.background_image_opacity, 0.5);
        assert_eq!(settings.background_image_fit.as_deref(), Some("cover"));
        assert!(settings.background_image_cover_chrome);
        // 卡几何越界钳制而非丢弃。
        assert_eq!(settings.pane_card_radius, Some(MAX_PANE_CARD_RADIUS));
        assert_eq!(settings.pane_card_gutter, Some(12.0));
        assert_eq!(settings.pane_card_shadow, Some(true));
        assert_eq!(settings.pane_card_divider, Some(MAX_PANE_CARD_DIVIDER));
        assert_eq!(settings.scrollback_lines, 50_000);
        assert_eq!(settings.scroll_speed, 1.75);
        assert_eq!(settings.cursor_shape, Some(CursorShapeName::Beam));
        assert_eq!(settings.cursor_blink, Some(true));
        assert_eq!(settings.cursor_motion, CursorMotion::Smooth);
        assert!(settings.copy_on_select);
        assert_eq!(settings.focus_follows_mouse, Some(true));
        assert!(!settings.dim_inactive_panes);
        assert!(!settings.multiline_paste_confirm);
        assert_eq!(settings.ligatures, Ligatures::Theme);
        assert_eq!(settings.bell, BellModeName::Audible);
        assert!(!settings.ctrl_wheel_font_zoom);
        assert!(!settings.ghost);
        assert_eq!(settings.accept, AcceptKeyName::Tab);
        assert_eq!(settings.completion_style, CompletionStyleName::Popup);
        assert_eq!(settings.new_tab_position, NewTabPositionName::End);
        assert!(!settings.panel_resize);
        assert_eq!(settings.shell.as_deref(), Some("pwsh"));
        assert_eq!(settings.startup_directory.as_deref(), Some("D:/work"));
        assert!(settings.keep_session);
        assert!(!settings.restore_session);
        assert!(!settings.resume_ai);
        assert!(settings.silent_start);
        assert!(settings.launch_at_login);
        assert!(settings.fetch);
        assert!(settings.terminal_proxy);
        assert!(!settings.refresh_environment);
        assert!(!settings.ai_toasts);
        assert_eq!(
            settings.notification_duration,
            NotificationDuration::NinetySeconds
        );
        assert!(!settings.tray);
        assert!(!settings.auto_check_updates);
        assert!(settings.auto_download_updates);
    }

    #[test]
    fn wrong_typed_values_fall_back_per_key_without_poisoning_neighbors() {
        // 类型不符/越界 = 该项回默认,其余键不受影响;应用永不炸。
        let settings = RuntimeSettings::from_raw(&raw(r#"{
  "appearance": {
    "theme": 42,
    "font_size": "huge",
    "opacity": "opaque",
    "blur": true
  },
  "interaction": {
    "bell": "loud",
    "cursor_blink": "yes"
  },
  "system": {
    "ai_toasts": "1"
  }
}"#));
        assert_eq!(settings.theme, ThemeName::Linear);
        assert_eq!(settings.font_size_px, None);
        assert_eq!(settings.opacity, 1.0);
        assert!(!settings.opacity_is_explicit());
        assert_eq!(settings.blur, BlurMode::None);
        assert!(!settings.blur_is_explicit());
        assert_eq!(settings.bell, BellModeName::Both);
        assert_eq!(settings.cursor_blink, None, "字符串拼写不是合法布尔");
        assert!(settings.ai_toasts, "类型不符回默认开");
    }

    #[test]
    fn explicit_values_stay_authoritative_over_theme_defaults() {
        // 「显式值权威」:显式 1.0/none 与未设置语义不同,切主题不得改写。
        let explicit =
            RuntimeSettings::from_raw(&raw(r#"{"appearance": {"opacity": 1.0, "blur": "none"}}"#));
        assert!(explicit.opacity_is_explicit());
        assert!(explicit.blur_is_explicit());
        assert_eq!(explicit.opacity, 1.0);
        assert_eq!(explicit.blur, BlurMode::None);

        let unset = RuntimeSettings::from_raw(&RawSettings::default());
        assert!(!unset.opacity_is_explicit());
        assert!(!unset.blur_is_explicit());
        assert_eq!(unset.opacity, 1.0, "未设置时主题默认可填充");

        // null 与缺键同语义:未设置。
        let nulled = RuntimeSettings::from_raw(&raw(
            r#"{"appearance": {"opacity": null, "blur": null, "custom_theme": null}}"#,
        ));
        assert!(!nulled.opacity_is_explicit());
        assert!(!nulled.blur_is_explicit());
        assert_eq!(nulled.custom_theme, None);
    }

    #[test]
    fn card_geometry_keys_stay_none_until_the_user_sets_them() {
        let untouched = RuntimeSettings::from_raw(&raw(r#"{"appearance": {"theme": "Nord"}}"#));
        assert_eq!(untouched.pane_card_radius, None);
        assert_eq!(untouched.pane_card_gutter, None);
        assert_eq!(untouched.pane_card_shadow, None);
        assert_eq!(untouched.pane_card_divider, None);
    }

    #[test]
    fn known_keys_iterator_reports_only_present_registered_keys() {
        let settings =
            raw(r#"{"appearance": {"theme": "Nord", "unknown_key": 1}, "handmade": {"x": true}}"#);
        assert_eq!(settings.keys().collect::<Vec<_>>(), ["theme"]);
    }
}

#[cfg(test)]
mod enum_names_tests {
    use super::*;

    /// 枚举族四件套:from_settings 与 settings_value 互为逆运算,
    /// ALL 与 VALUES 同序对齐(GUI 枚举与写盘值单源)。
    macro_rules! assert_round_trip {
        ($ty:ty) => {{
            for (variant, value) in <$ty>::ALL.into_iter().zip(<$ty>::VALUES) {
                assert_eq!(variant.settings_value(), value);
                assert_eq!(<$ty>::from_settings(value), Some(variant));
                // 大小写与空白归一(设置文件可被手改)。
                assert_eq!(<$ty>::from_settings(&value.to_uppercase()), Some(variant));
            }
            assert_eq!(<$ty>::ALL.len(), <$ty>::VALUES.len());
        }};
    }

    #[test]
    fn every_key_domain_enum_round_trips_its_stable_values() {
        assert_round_trip!(CursorShapeName);
        assert_round_trip!(AcceptKeyName);
        assert_round_trip!(CompletionStyleName);
        assert_round_trip!(TabRevealName);
        assert_round_trip!(DensityName);
        assert_round_trip!(NewTabPositionName);
        assert_round_trip!(CellWidthModeName);
        assert_round_trip!(VcsDisplayName);
        assert_round_trip!(BellModeName);
        assert_round_trip!(BlurMode);
    }

    #[test]
    fn unknown_and_legacy_spellings_are_rejected() {
        // 值域 = VALUES 单源;历史别名/布尔拼写不兼容(零迁移)。
        assert_eq!(CursorShapeName::from_settings("bar"), None);
        assert_eq!(CompletionStyleName::from_settings("ghost"), None);
        assert_eq!(CompletionStyleName::from_settings("list"), None);
        assert_eq!(BellModeName::from_settings("off"), None);
        assert_eq!(BellModeName::from_settings("loud"), None);
        for value in [
            "1",
            "true",
            "yes",
            "on",
            "0",
            "false",
            "off",
            "blurbehind",
            "frosted",
        ] {
            assert_eq!(BlurMode::from_settings(value), None, "{value}");
        }
        assert_eq!(VcsDisplayName::from_settings("svn"), None, "SVN 档已裁");
    }

    #[test]
    fn completion_style_cycles_and_hybrid_defers_until_requested() {
        assert_eq!(
            CompletionStyleName::Hybrid.cycle(),
            CompletionStyleName::Inline
        );
        assert_eq!(
            CompletionStyleName::Inline.cycle(),
            CompletionStyleName::Popup
        );
        assert_eq!(
            CompletionStyleName::Hybrid.active_style(false),
            CompletionStyleName::Inline
        );
        assert_eq!(
            CompletionStyleName::Hybrid.active_style(true),
            CompletionStyleName::Popup
        );
        assert_eq!(
            CompletionStyleName::Popup.active_style(false),
            CompletionStyleName::Popup
        );
    }

    #[test]
    fn bell_mode_decomposes_into_visual_and_audible_channels() {
        assert!(!BellModeName::None.visual() && !BellModeName::None.audible());
        assert!(BellModeName::Visual.visual() && !BellModeName::Visual.audible());
        assert!(!BellModeName::Audible.visual() && BellModeName::Audible.audible());
        assert!(BellModeName::Both.visual() && BellModeName::Both.audible());
    }

    #[test]
    fn blur_enabled_matches_material_presence() {
        assert!(!BlurMode::None.enabled());
        for mode in [
            BlurMode::Mica,
            BlurMode::MicaAlt,
            BlurMode::Aero,
            BlurMode::Acrylic,
        ] {
            assert!(mode.enabled());
        }
    }
}

#[cfg(test)]
mod hex_rgb_tests {
    use super::*;

    #[test]
    fn hex_rgb_roundtrip() {
        assert_eq!(parse_hex_rgb("#8bd5ca"), Some([0x8b, 0xd5, 0xca]));
        assert_eq!(parse_hex_rgb("8bd5ca"), Some([0x8b, 0xd5, 0xca]));
        assert_eq!(parse_hex_rgb("#nothex"), None);
        assert_eq!(format_hex_rgb([0x8b, 0xd5, 0xca]), "#8bd5ca");
    }

    #[test]
    fn shorthand_rgb_expands_and_rejects_non_hex_or_alpha_input() {
        for value in ["#123", "123", "  #123  "] {
            assert_eq!(parse_hex_rgb(value), Some([0x11, 0x22, 0x33]), "{value:?}");
        }
        assert_eq!(parse_hex_rgb("#aBc"), Some([0xaa, 0xbb, 0xcc]));
        assert_eq!(format_hex_rgb(parse_hex_rgb("#aBc").unwrap()), "#aabbcc");
        for value in [
            "",
            "#",
            "#12",
            "#1234",
            "#12345",
            "#12345678",
            "#12g",
            "+1b2c3",
            "中文",
        ] {
            assert_eq!(parse_hex_rgb(value), None, "{value:?}");
        }
    }
}

#[cfg(test)]
mod term_theme_tests {
    use super::*;

    #[test]
    fn theme_names_roundtrip_and_unknown_names_fall_back() {
        for theme in ThemeName::BUILTIN {
            assert_eq!(
                ThemeName::from_prompt_name(theme.prompt_name()),
                Some(theme)
            );
        }
        assert_eq!(ThemeName::from_prompt_name("NoSuchTheme"), None);
        // 默认主题 = Linear(D06-3)。
        assert_eq!(ThemeName::default(), ThemeName::Linear);
        assert_eq!(
            RuntimeSettings::from_raw(&RawSettings::default()).theme,
            ThemeName::Linear
        );
    }

    #[test]
    fn every_builtin_theme_declares_a_full_terminal_palette() {
        for theme in ThemeName::BUILTIN {
            let term = theme.term_theme();
            let exact = term
                .exact
                .unwrap_or_else(|| panic!("{} must declare exact colors", theme.prompt_name()));
            assert_eq!(exact.ansi.len(), 16);
        }
    }

    #[test]
    fn nord_and_linear_keep_their_declared_terminal_palettes() {
        let nord = ThemeName::Nord
            .term_theme()
            .exact
            .expect("Nord exact colors");
        assert_eq!(nord.foreground, [0xf1, 0xf6, 0xff]);
        assert_eq!(nord.ansi[0], [0x3b, 0x42, 0x52]);
        assert_eq!(nord.ansi[15], [0xec, 0xef, 0xf4]);
        assert_eq!(nord.cursor, Some([0xe5, 0xe9, 0xf0]));
        assert_eq!(nord.cursor_stroke, Some([0x88, 0xc0, 0xd0]));
        assert_eq!(nord.selection_background, Some([0xe5, 0xe9, 0xf0]));

        let linear = ThemeName::Linear
            .term_theme()
            .exact
            .expect("Linear exact colors");
        assert_eq!(
            ThemeName::Linear.term_theme().background,
            [0x0a, 0x0a, 0x0b]
        );
        assert_eq!(linear.foreground, [0xcf, 0xca, 0xc1]);
        assert_eq!(linear.ansi[0], [0x0a, 0x0a, 0x0b]);
        assert_eq!(linear.ansi[1], [0xd9, 0x70, 0x6b]);
        assert_eq!(linear.ansi[15], [0xf0, 0xed, 0xe8]);
        assert_eq!(linear.cursor, Some([0x6e, 0x9f, 0xf2]));
        assert_eq!(linear.selection_foreground, Some([0xf0, 0xed, 0xe8]));
    }

    #[test]
    fn card_geometry_is_the_single_source_of_the_flush_shape() {
        // 内置主题全部共享「铺满」几何:半径与卡缝归零、一条竖线分界。
        // 四个数一起构成形态,逐项钉死而非只断言 radius。
        for theme in ThemeName::BUILTIN {
            let geometry = theme.card_geometry();
            assert_eq!(geometry.radius, 0.0);
            assert_eq!(geometry.gutter, 0.0);
            assert_eq!(geometry.divider, 1.0);
            assert!(!geometry.shadow);
        }
    }
}

#[cfg(test)]
mod persist_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "slterm-settings-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn apply_updates_replaces_in_place_appends_and_preserves_unknown_data() {
        let tree = json::parse(
            br#"{
  "appearance": {
    "theme": "Nord",
    "opacity": 0.9
  },
  "handmade": {
    "note": "keep"
  },
  "keybind": [
    { "combo": "ctrl+shift+c", "action": "copy" }
  ]
}
"#,
        )
        .unwrap();
        let merged = apply_updates(
            &tree,
            &[
                ("theme", Value::String("Linear".into())),
                ("font_size", Value::Number(14.0)),
                ("bell", Value::String("visual".into())),
            ],
        );
        // 段内:已有键原位替换,缺失键追加;跨段键落到归属段。
        let appearance = merged.get("appearance").unwrap();
        assert_eq!(appearance.get("theme").unwrap().as_str(), Some("Linear"));
        assert_eq!(appearance.get("opacity").unwrap().as_f64(), Some(0.9));
        assert_eq!(appearance.get("font_size").unwrap().as_f64(), Some(14.0));
        assert_eq!(
            merged
                .get("interaction")
                .unwrap()
                .get("bell")
                .unwrap()
                .as_str(),
            Some("visual")
        );
        // 未知段与域外段原样保留。
        assert_eq!(
            merged
                .get("handmade")
                .unwrap()
                .get("note")
                .unwrap()
                .as_str(),
            Some("keep")
        );
        assert!(matches!(merged.get("keybind"), Some(Value::Array(_))));
        // 段序:已有段原位,新段追加尾部。
        assert_eq!(
            merged.object_keys().collect::<Vec<_>>(),
            ["appearance", "handmade", "keybind", "interaction"]
        );
    }

    #[test]
    fn apply_updates_skips_unregistered_keys_and_repairs_mistyped_segments() {
        let mut tree = Value::empty_object();
        tree.set("appearance", Value::Number(5.0));
        let merged = apply_updates(
            &tree,
            &[
                ("unknown_key", Value::Bool(true)),
                ("theme", Value::String("Nord".into())),
            ],
        );
        assert_eq!(merged.get("unknown_key"), None, "未登记键不写入");
        // 段被手改成非对象:写键时重建段,不炸。
        assert_eq!(
            merged
                .get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Nord")
        );
    }

    #[test]
    fn persist_round_trips_every_written_value_through_reload() {
        let directory = temp_dir("roundtrip");
        let path = directory.join("settings.json");
        persist_keys_at(
            &path,
            &[
                ("theme", Value::String("Nord".into())),
                ("font_size", Value::Number(16.5)),
                ("scrollback_lines", Value::Number(50000.0)),
                ("bell", Value::String("audible".into())),
                ("restore_session", Value::Bool(false)),
                ("slterm_ai_hooks_grok", Value::Bool(true)),
            ],
        )
        .unwrap();
        // 再写一次:浅合并不清既有键。
        persist_keys_at(&path, &[("opacity", Value::Number(0.9))]).unwrap();

        let settings = RuntimeSettings::from_raw(&RawSettings::from_json_bytes(
            &std::fs::read(&path).unwrap(),
        ));
        assert_eq!(settings.theme, ThemeName::Nord);
        assert_eq!(settings.font_size_px, Some(16.5));
        assert_eq!(settings.scrollback_lines, 50_000);
        assert_eq!(settings.bell, BellModeName::Audible);
        assert!(!settings.restore_session);
        assert_eq!(settings.opacity, 0.9);
        assert!(settings.opacity_is_explicit());
        assert!(AgentHook::Grok.enabled(&RawSettings::from_json_bytes(
            &std::fs::read(&path).unwrap()
        )));
        // 临时文件不残留。
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn corrupted_files_are_never_overwritten() {
        let directory = temp_dir("corrupted");
        let path = directory.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let result = persist_keys_at(&path, &[("theme", Value::String("Nord".into()))]);
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn writes_beyond_the_size_budget_are_rejected() {
        let directory = temp_dir("budget");
        let path = directory.join("settings.json");
        let huge = "x".repeat(MAX_PERSIST_BYTES);
        let result = persist_keys_at(&path, &[("custom_theme", Value::String(huge))]);
        assert!(result.is_err());
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn try_load_distinguishes_missing_from_corrupted() {
        let directory = temp_dir("load");
        let path = directory.join("settings.json");
        // 无文件 = 默认空树,不是错误。
        let missing = RawSettings::try_load_at(&path).unwrap();
        assert_eq!(missing.keys().count(), 0);
        // 损坏文件 = Err(授权路径不能把损坏当默认)。
        std::fs::write(&path, "{ not json").unwrap();
        assert!(RawSettings::try_load_at(&path).is_err());
        // 根非对象(可解析的损坏)= 空树,不炸。
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        assert_eq!(RawSettings::try_load_at(&path).unwrap().keys().count(), 0);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
