//! 键事实 → 转义序列的构建器(kitty/legacy VT 编码器本体)。
//!
//! 来源:上游 `input/terminal_input.rs` 的 `KeyInput`/`build_sequence` 族,
//! 随 `keymap.rs` 三路径编码器一并迁移。与上游的形态差(裁剪登记):
//!
//! - **winit 类型 → 壳内同名轻量类型**:`Key`/`NamedKey`/`ElementState`/
//!   `KeyLocation`/`ModifiersState` 在本文件自足(winit 事件循环本体不迁,
//!   根 Cargo.toml 注释)。`Key::Character` 的 `SmolStr` 退化为 `String`——
//!   键名是短串,本路径非热点。
//! - **Win32 记录臂裁**:`raw: RawKeyEventInfo` 字段、`build_win32_input_sequence`
//!   族与 `build_sequence` 开头的 win32 分流一并不迁。GPUI 没有原生键事实
//!   通道(上游旧壳从 winit fork 直读 `KEY_EVENT_RECORD`),壳侧记录由
//!   `keymap/win32.rs` 从 GPUI 键名重造并专管;`keymap.rs` 在进入本构建器前
//!   已完成 win32/kitty 分流,本函数永远在 kitty 位激活时被调
//!   (`use_win32_input_mode` 此时恒 false),臂即死码。
//! - **runtime_api 族不迁**(`build_runtime_*`,归 runtime_api);
//!   `build_focus_loss_key_ups` 不迁(上游 GPUI 壳无消费点,焦点丢失合成
//!   key-up 只被旧壳触摸路径调用);`Key::Dead` 随组合路径臂裁。

use std::borrow::Cow;

use slterm_terminal::term::TermMode;

/// 一次键转换的键盘事实,从平台事件捕获一次,供各编码器消费。
///
/// 字段值镜像 OS 实际报告的内容。GPUI 通道下 `location` 恒 `Standard`、
/// `repeat` 恒 false、`text_with_all_modifiers` 恒 `None`(GPUI 的
/// `Keystroke` 不带这三条通道;associated-text 臂因此对 GPUI 键退化)。
#[derive(Clone, Debug)]
pub(super) struct KeyInput {
    pub logical_key: Key,
    pub state: ElementState,
    pub location: KeyLocation,
    pub repeat: bool,
    /// 剥掉全部修饰后的逻辑键(增强 alternate-key 上报需要 unshifted
    /// 基键,如 `!` 报 `1`)。
    pub key_without_modifiers: Key,
    /// 带修饰的 OS 文本通道(WM_CHAR / ToUnicode)。key-up 与不产文本的
    /// 键为 `None`。
    pub text_with_all_modifiers: Option<String>,
}

/// `winit::keyboard::Key` 的壳内对应物;变体集 = 本构建器与 keymap 的
/// 消费面(可打印字符 + 命名键)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Key {
    Character(String),
    Named(NamedKey),
}

/// `winit::keyboard::NamedKey` 的壳内对应物;变体集 = SequenceBuilder
/// 三张编码表与 keymap 构造面的全集。编码表全量保留是协议映射完整性的
/// 前提(构造面只及 Enter——多数变体仅出现在编码表的匹配模式)。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NamedKey {
    Enter,
    Tab,
    Escape,
    Space,
    Backspace,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
    Shift,
    Control,
    Alt,
    Super,
    Hyper,
    Meta,
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    ContextMenu,
    MediaPlay,
    MediaPause,
    MediaPlayPause,
    MediaStop,
    MediaFastForward,
    MediaRewind,
    MediaTrackNext,
    MediaTrackPrevious,
    MediaRecord,
    AudioVolumeDown,
    AudioVolumeUp,
    AudioVolumeMute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ElementState {
    Pressed,
    Released,
}

impl ElementState {
    fn is_pressed(self) -> bool {
        self == ElementState::Pressed
    }
}

/// `winit::keyboard::KeyLocation` 的壳内对应物。GPUI 不报告键位,构造面
/// 恒 `Standard`;`Left`/`Numpad` 仅测试与编码表可达性保留。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyLocation {
    Standard,
    Left,
    Numpad,
}

bitflags::bitflags! {
    /// `winit::keyboard::ModifiersState` 的壳内对应物。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct ModifiersState: u8 {
        const SHIFT   = 0b0000_0001;
        const ALT     = 0b0000_0010;
        const CONTROL = 0b0000_0100;
        const SUPER   = 0b0000_1000;
    }
}

/// ConPTY Win32 input mode 是否当家。
///
/// 子进程请求的 Win32 记录是**回落**协议,不是与增强键盘协议并列的第二
/// 编码器:一旦子进程请求过增强键盘标志,那份标志就是线上合同,压过
/// DECSET 9001。
#[inline]
pub(super) fn use_win32_input_mode(mode: TermMode) -> bool {
    mode.contains(TermMode::WIN32_INPUT_MODE) && !mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL)
}

/// 应用未协商键盘协议时的多行兼容。Claude 只对终端名白名单(含 WT)启用
/// CSI-u;不认识slterm 时用它的 LF 换行绑定。ConPTY 会把原生 Shift+Return
/// 记录翻译成 CR(即使 UnicodeChar 是 LF),其他 Windows 读者(Codex、
/// PSReadLine)必须保留原生记录。
///
/// 留位:`program` 恒 `None`——agent 检测归 AI 集成域,
/// 上游 `AgentKind::parse(program) == Claude` 的特判臂随 AI 集成域回填;回填前
/// Windows 上 win32 模式恒保原生记录。
pub(super) fn shift_enter_as_lf(program: Option<&str>, mode: TermMode) -> bool {
    // 回填前参数不消费(Windows-only:!cfg!(windows) 臂随定位约束消亡)。
    let _ = program;
    !mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL) && !use_win32_input_mode(mode)
}

/// 按 `input`、`mods`、`mode` 构建键的转义序列——各平台共享的
/// kitty/legacy VT 编码器。
///
/// APP_KEYPAD 之类的序列在绑定层处理。win32-only 模式不到达本函数
/// (keymap.rs 先行分流到 `keymap/win32.rs`),故无上游的 win32 记录臂。
#[inline(never)]
pub(super) fn build_sequence(input: &KeyInput, mods: ModifiersState, mode: TermMode) -> Vec<u8> {
    let mut modifiers = mods.into();

    let kitty_seq = mode.intersects(
        TermMode::REPORT_ALL_KEYS_AS_ESC
            | TermMode::DISAMBIGUATE_ESC_CODES
            | TermMode::REPORT_EVENT_TYPES,
    );

    let kitty_encode_all = mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC);
    // 默认参数是 1,可以省略。
    let kitty_event_type = mode.contains(TermMode::REPORT_EVENT_TYPES)
        && (input.repeat || input.state == ElementState::Released);

    let context = SequenceBuilder {
        mode,
        modifiers,
        kitty_seq,
        kitty_encode_all,
        kitty_event_type,
    };

    let associated_text = input.text_with_all_modifiers.as_deref().filter(|text| {
        mode.contains(TermMode::REPORT_ASSOCIATED_TEXT)
            && input.state != ElementState::Released
            && !text.is_empty()
            && !is_control_character(text)
    });

    let sequence_base = context
        .try_build_numpad(input)
        .or_else(|| context.try_build_named_kitty(input))
        .or_else(|| context.try_build_named_normal(input, associated_text.is_some()))
        .or_else(|| context.try_build_control_char_or_mod(input, &mut modifiers))
        .or_else(|| context.try_build_textual(input, associated_text));

    let (payload, terminator) = match sequence_base {
        Some(SequenceBase {
            payload,
            terminator,
        }) => (payload, terminator),
        _ => return Vec::new(),
    };

    let mut payload = format!("\x1b[{payload}");

    // 附加修饰信息。
    if kitty_event_type || !modifiers.is_empty() || associated_text.is_some() {
        payload.push_str(&format!(";{}", modifiers.encode_esc_sequence()));
    }

    // 附加事件类型。
    if kitty_event_type {
        payload.push(':');
        let event_type = match input.state {
            _ if input.repeat => '2',
            ElementState::Pressed => '1',
            ElementState::Released => '3',
        };
        payload.push(event_type);
    }

    if let Some(text) = associated_text {
        let mut codepoints = text.chars().map(u32::from);
        if let Some(codepoint) = codepoints.next() {
            payload.push_str(&format!(";{codepoint}"));
        }
        for codepoint in codepoints {
            payload.push_str(&format!(":{codepoint}"));
        }
    }

    payload.push(terminator.encode_esc_sequence());

    payload.into_bytes()
}

/// 从 [`KeyInput`] 构建转义序列载荷的辅助器。
struct SequenceBuilder {
    mode: TermMode,
    /// 发出的序列应遵循 kitty 键盘协议。
    kitty_seq: bool,
    /// 按协议编码全部键。
    kitty_encode_all: bool,
    /// 上报事件类型。
    kitty_event_type: bool,
    modifiers: SequenceModifiers,
}

impl SequenceBuilder {
    /// 尝试从事件的文本构建序列。
    fn try_build_textual(
        &self,
        input: &KeyInput,
        associated_text: Option<&str>,
    ) -> Option<SequenceBase> {
        let character = match &input.logical_key {
            Key::Character(character) if self.kitty_seq => character,
            _ => return None,
        };

        if character.chars().count() == 1 {
            let shift = self.modifiers.contains(SequenceModifiers::SHIFT);

            let ch = character.chars().next().unwrap();
            let unshifted_ch = if shift {
                ch.to_lowercase().next().unwrap()
            } else {
                ch
            };

            let alternate_key_code = u32::from(ch);
            let mut unicode_key_code = u32::from(unshifted_ch);

            // 对随修饰变化的键取基键,如 `!` 取 `1`。只在 SHIFT 按下时进行。
            if shift
                && alternate_key_code == unicode_key_code
                && let Key::Character(unmodded) = &input.key_without_modifiers
            {
                unicode_key_code = u32::from(unmodded.chars().next().unwrap_or(unshifted_ch));
            }

            // 注意:基础布局被忽略——GPUI 同样不暴露这一信息。
            let payload = if self.mode.contains(TermMode::REPORT_ALTERNATE_KEYS)
                && alternate_key_code != unicode_key_code
            {
                format!("{unicode_key_code}:{alternate_key_code}")
            } else {
                unicode_key_code.to_string()
            };

            Some(SequenceBase::new(payload.into(), SequenceTerminator::Kitty))
        } else if self.kitty_encode_all && associated_text.is_some() {
            // 需要上报文本但没有关联键时的回落。
            Some(SequenceBase::new("0".into(), SequenceTerminator::Kitty))
        } else {
            None
        }
    }

    /// 尝试从小键盘键构建。
    ///
    /// 键未知或不在小键盘时返回 `None`。GPUI 不报告 `Numpad` 键位——
    /// 本臂对 GPUI 键恒不命中,随编码表全量保留以守协议映射完整。
    fn try_build_numpad(&self, input: &KeyInput) -> Option<SequenceBase> {
        if !self.kitty_seq || input.location != KeyLocation::Numpad {
            return None;
        }

        let base = match &input.logical_key {
            Key::Character(c) => match c.as_str() {
                "0" => "57399",
                "1" => "57400",
                "2" => "57401",
                "3" => "57402",
                "4" => "57403",
                "5" => "57404",
                "6" => "57405",
                "7" => "57406",
                "8" => "57407",
                "9" => "57408",
                "." => "57409",
                "/" => "57410",
                "*" => "57411",
                "-" => "57412",
                "+" => "57413",
                "=" => "57415",
                _ => return None,
            },
            Key::Named(named) => match named {
                NamedKey::Enter => "57414",
                NamedKey::ArrowLeft => "57417",
                NamedKey::ArrowRight => "57418",
                NamedKey::ArrowUp => "57419",
                NamedKey::ArrowDown => "57420",
                NamedKey::PageUp => "57421",
                NamedKey::PageDown => "57422",
                NamedKey::Home => "57423",
                NamedKey::End => "57424",
                NamedKey::Insert => "57425",
                NamedKey::Delete => "57426",
                _ => return None,
            },
        };

        Some(SequenceBase::new(base.into(), SequenceTerminator::Kitty))
    }

    /// 尝试按 kitty 键盘协议的功能键编码从 [`NamedKey`] 构建。
    fn try_build_named_kitty(&self, input: &KeyInput) -> Option<SequenceBase> {
        let named = match input.logical_key {
            Key::Named(named) if self.kitty_seq => named,
            _ => return None,
        };

        let (base, terminator) = match named {
            // F3 在 kitty 协议中与本仓 terminfo 分歧。
            NamedKey::F3 => ("13", SequenceTerminator::Normal('~')),
            NamedKey::F13 => ("57376", SequenceTerminator::Kitty),
            NamedKey::F14 => ("57377", SequenceTerminator::Kitty),
            NamedKey::F15 => ("57378", SequenceTerminator::Kitty),
            NamedKey::F16 => ("57379", SequenceTerminator::Kitty),
            NamedKey::F17 => ("57380", SequenceTerminator::Kitty),
            NamedKey::F18 => ("57381", SequenceTerminator::Kitty),
            NamedKey::F19 => ("57382", SequenceTerminator::Kitty),
            NamedKey::F20 => ("57383", SequenceTerminator::Kitty),
            NamedKey::F21 => ("57384", SequenceTerminator::Kitty),
            NamedKey::F22 => ("57385", SequenceTerminator::Kitty),
            NamedKey::F23 => ("57386", SequenceTerminator::Kitty),
            NamedKey::F24 => ("57387", SequenceTerminator::Kitty),
            NamedKey::F25 => ("57388", SequenceTerminator::Kitty),
            NamedKey::F26 => ("57389", SequenceTerminator::Kitty),
            NamedKey::F27 => ("57390", SequenceTerminator::Kitty),
            NamedKey::F28 => ("57391", SequenceTerminator::Kitty),
            NamedKey::F29 => ("57392", SequenceTerminator::Kitty),
            NamedKey::F30 => ("57393", SequenceTerminator::Kitty),
            NamedKey::F31 => ("57394", SequenceTerminator::Kitty),
            NamedKey::F32 => ("57395", SequenceTerminator::Kitty),
            NamedKey::F33 => ("57396", SequenceTerminator::Kitty),
            NamedKey::F34 => ("57397", SequenceTerminator::Kitty),
            NamedKey::F35 => ("57398", SequenceTerminator::Kitty),
            NamedKey::ScrollLock => ("57359", SequenceTerminator::Kitty),
            NamedKey::PrintScreen => ("57361", SequenceTerminator::Kitty),
            NamedKey::Pause => ("57362", SequenceTerminator::Kitty),
            NamedKey::ContextMenu => ("57363", SequenceTerminator::Kitty),
            NamedKey::MediaPlay => ("57428", SequenceTerminator::Kitty),
            NamedKey::MediaPause => ("57429", SequenceTerminator::Kitty),
            NamedKey::MediaPlayPause => ("57430", SequenceTerminator::Kitty),
            NamedKey::MediaStop => ("57432", SequenceTerminator::Kitty),
            NamedKey::MediaFastForward => ("57433", SequenceTerminator::Kitty),
            NamedKey::MediaRewind => ("57434", SequenceTerminator::Kitty),
            NamedKey::MediaTrackNext => ("57435", SequenceTerminator::Kitty),
            NamedKey::MediaTrackPrevious => ("57436", SequenceTerminator::Kitty),
            NamedKey::MediaRecord => ("57437", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeDown => ("57438", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeUp => ("57439", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeMute => ("57440", SequenceTerminator::Kitty),
            _ => return None,
        };

        Some(SequenceBase::new(base.into(), terminator))
    }

    /// 尝试从 [`NamedKey`] 构建(传统 xterm/dec 形态)。
    fn try_build_named_normal(
        &self,
        input: &KeyInput,
        has_associated_text: bool,
    ) -> Option<SequenceBase> {
        let named = match input.logical_key {
            Key::Named(named) => named,
            _ => return None,
        };

        // 默认参数是 1,可以省略。
        let one_based =
            if self.modifiers.is_empty() && !self.kitty_event_type && !has_associated_text {
                ""
            } else {
                "1"
            };
        let (base, terminator) = match named {
            NamedKey::PageUp => ("5", SequenceTerminator::Normal('~')),
            NamedKey::PageDown => ("6", SequenceTerminator::Normal('~')),
            NamedKey::Insert => ("2", SequenceTerminator::Normal('~')),
            NamedKey::Delete => ("3", SequenceTerminator::Normal('~')),
            NamedKey::Home => (one_based, SequenceTerminator::Normal('H')),
            NamedKey::End => (one_based, SequenceTerminator::Normal('F')),
            NamedKey::ArrowLeft => (one_based, SequenceTerminator::Normal('D')),
            NamedKey::ArrowRight => (one_based, SequenceTerminator::Normal('C')),
            NamedKey::ArrowUp => (one_based, SequenceTerminator::Normal('A')),
            NamedKey::ArrowDown => (one_based, SequenceTerminator::Normal('B')),
            NamedKey::F1 => (one_based, SequenceTerminator::Normal('P')),
            NamedKey::F2 => (one_based, SequenceTerminator::Normal('Q')),
            NamedKey::F3 => (one_based, SequenceTerminator::Normal('R')),
            NamedKey::F4 => (one_based, SequenceTerminator::Normal('S')),
            NamedKey::F5 => ("15", SequenceTerminator::Normal('~')),
            NamedKey::F6 => ("17", SequenceTerminator::Normal('~')),
            NamedKey::F7 => ("18", SequenceTerminator::Normal('~')),
            NamedKey::F8 => ("19", SequenceTerminator::Normal('~')),
            NamedKey::F9 => ("20", SequenceTerminator::Normal('~')),
            NamedKey::F10 => ("21", SequenceTerminator::Normal('~')),
            NamedKey::F11 => ("23", SequenceTerminator::Normal('~')),
            NamedKey::F12 => ("24", SequenceTerminator::Normal('~')),
            NamedKey::F13 => ("25", SequenceTerminator::Normal('~')),
            NamedKey::F14 => ("26", SequenceTerminator::Normal('~')),
            NamedKey::F15 => ("28", SequenceTerminator::Normal('~')),
            NamedKey::F16 => ("29", SequenceTerminator::Normal('~')),
            NamedKey::F17 => ("31", SequenceTerminator::Normal('~')),
            NamedKey::F18 => ("32", SequenceTerminator::Normal('~')),
            NamedKey::F19 => ("33", SequenceTerminator::Normal('~')),
            NamedKey::F20 => ("34", SequenceTerminator::Normal('~')),
            _ => return None,
        };

        Some(SequenceBase::new(base.into(), terminator))
    }

    /// 尝试从控制字符(如 Enter)与修饰构建转义序列。
    fn try_build_control_char_or_mod(
        &self,
        input: &KeyInput,
        mods: &mut SequenceModifiers,
    ) -> Option<SequenceBase> {
        if !self.kitty_encode_all && !self.kitty_seq {
            return None;
        }

        let named = match input.logical_key {
            Key::Named(named) => named,
            _ => return None,
        };

        let base = match named {
            NamedKey::Tab => "9",
            NamedKey::Enter => "13",
            NamedKey::Escape => "27",
            NamedKey::Space => "32",
            NamedKey::Backspace => "127",
            _ => "",
        };

        // 既不是命名控制字符、当前模式又禁止编码修饰键本身时失败。
        if !self.kitty_encode_all && base.is_empty() {
            return None;
        }

        let base = match (named, input.location) {
            (NamedKey::Shift, KeyLocation::Left) => "57441",
            (NamedKey::Control, KeyLocation::Left) => "57442",
            (NamedKey::Alt, KeyLocation::Left) => "57443",
            (NamedKey::Super, KeyLocation::Left) => "57444",
            (NamedKey::Hyper, KeyLocation::Left) => "57445",
            (NamedKey::Meta, KeyLocation::Left) => "57446",
            (NamedKey::Shift, _) => "57447",
            (NamedKey::Control, _) => "57448",
            (NamedKey::Alt, _) => "57449",
            (NamedKey::Super, _) => "57450",
            (NamedKey::Hyper, _) => "57451",
            (NamedKey::Meta, _) => "57452",
            (NamedKey::CapsLock, _) => "57358",
            (NamedKey::NumLock, _) => "57360",
            _ => base,
        };

        // 注意:CSI-u 协议要求修饰状态取按键**之前**的值;修饰键自身的事件
        // 按 keysym 而不是实际修饰状态合成,这是已协商键盘协议的要求。
        let press = input.state.is_pressed();
        match named {
            NamedKey::Shift => mods.set(SequenceModifiers::SHIFT, press),
            NamedKey::Control => mods.set(SequenceModifiers::CONTROL, press),
            NamedKey::Alt => mods.set(SequenceModifiers::ALT, press),
            NamedKey::Super => mods.set(SequenceModifiers::SUPER, press),
            _ => (),
        }

        if base.is_empty() {
            None
        } else {
            Some(SequenceBase::new(base.into(), SequenceTerminator::Kitty))
        }
    }
}

struct SequenceBase {
    /// 增强键盘协议定义的载荷 `number` 与可选 alternate 基键。
    payload: Cow<'static, str>,
    terminator: SequenceTerminator,
}

impl SequenceBase {
    fn new(payload: Cow<'static, str>, terminator: SequenceTerminator) -> Self {
        Self {
            payload,
            terminator,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceTerminator {
    /// xterm/dec 定义的常规键转义序列终止符。
    Normal(char),
    /// kitty 转义序列的终止符。
    Kitty,
}

impl SequenceTerminator {
    fn encode_esc_sequence(self) -> char {
        match self {
            SequenceTerminator::Normal(char) => char,
            SequenceTerminator::Kitty => 'u',
        }
    }
}

bitflags::bitflags! {
    /// 转义序列的修饰编码。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SequenceModifiers : u8 {
        const SHIFT   = 0b0000_0001;
        const ALT     = 0b0000_0010;
        const CONTROL = 0b0000_0100;
        const SUPER   = 0b0000_1000;
        // 注意:CSI-u 协议还定义了更多修饰位(如 Capslock),但 GPUI 侧
        // 同样不把它们当修饰键。
    }
}

impl SequenceModifiers {
    /// 取转义序列中应传的值。
    pub fn encode_esc_sequence(self) -> u8 {
        self.bits() + 1
    }
}

impl From<ModifiersState> for SequenceModifiers {
    fn from(mods: ModifiersState) -> Self {
        let mut modifiers = Self::empty();
        modifiers.set(Self::SHIFT, mods.contains(ModifiersState::SHIFT));
        modifiers.set(Self::ALT, mods.contains(ModifiersState::ALT));
        modifiers.set(Self::CONTROL, mods.contains(ModifiersState::CONTROL));
        modifiers.set(Self::SUPER, mods.contains(ModifiersState::SUPER));
        modifiers
    }
}

/// 检查 `text` 是否为 `0x7f`、C0 或 C1 控制码。
fn is_control_character(text: &str) -> bool {
    // 0x7f (DEL) 收录在内:它有专属控制码(`^?`),与上报文本(`^H`)
    // 通常不一致,尽管严格说它不属于 C0/C1。
    let codepoint = text.bytes().next().unwrap();
    text.len() == 1 && (codepoint < 0x20 || (0x7f..=0x9f).contains(&codepoint))
}

/// 平台无关测试:kitty/legacy 编码器只消费 [`KeyInput`] 的可移植字段——
/// 「单一跨平台编码器」合同的可见形态。runtime_api 族(归 runtime_api)与
/// focus_loss 族(GPUI 壳无消费点)的用例不迁。
#[cfg(test)]
mod vt_tests {
    use super::*;

    fn input(logical_key: Key, state: ElementState, text: Option<&str>) -> KeyInput {
        KeyInput {
            key_without_modifiers: logical_key.clone(),
            logical_key,
            state,
            location: KeyLocation::Standard,
            repeat: false,
            text_with_all_modifiers: text.map(str::to_owned),
        }
    }

    #[test]
    fn win32_input_mode_is_used_when_no_kitty_flags_are_active() {
        assert!(use_win32_input_mode(TermMode::WIN32_INPUT_MODE));
    }

    #[test]
    fn kitty_input_takes_precedence_over_win32_input_mode() {
        let mode = TermMode::WIN32_INPUT_MODE | TermMode::DISAMBIGUATE_ESC_CODES;
        assert!(!use_win32_input_mode(mode));
    }

    #[test]
    fn unrelated_modes_do_not_enable_win32_input() {
        assert!(!use_win32_input_mode(TermMode::APP_CURSOR));
    }

    #[test]
    fn kitty_disambiguate_escape_is_csi_27_u() {
        let esc = input(
            Key::Named(NamedKey::Escape),
            ElementState::Pressed,
            Some("\x1b"),
        );
        let bytes = build_sequence(
            &esc,
            ModifiersState::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_eq!(bytes, b"\x1b[27u");
    }

    #[test]
    fn kitty_shift_enter_reports_the_shift_modifier() {
        let enter = input(
            Key::Named(NamedKey::Enter),
            ElementState::Pressed,
            Some("\r"),
        );
        let bytes = build_sequence(
            &enter,
            ModifiersState::SHIFT,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_eq!(bytes, b"\x1b[13;2u");
    }

    #[test]
    fn kitty_ctrl_character_encodes_codepoint_and_modifier() {
        let key = input(
            Key::Character("a".into()),
            ElementState::Pressed,
            Some("\x01"),
        );
        let bytes = build_sequence(
            &key,
            ModifiersState::CONTROL,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_eq!(bytes, b"\x1b[97;5u");
    }

    #[test]
    fn legacy_arrow_and_function_keys_keep_xterm_encoding() {
        let up = input(Key::Named(NamedKey::ArrowUp), ElementState::Pressed, None);
        assert_eq!(
            build_sequence(&up, ModifiersState::empty(), TermMode::empty()),
            b"\x1b[A"
        );

        let f5 = input(Key::Named(NamedKey::F5), ElementState::Pressed, None);
        assert_eq!(
            build_sequence(&f5, ModifiersState::SHIFT, TermMode::empty()),
            b"\x1b[15;2~"
        );
    }

    #[test]
    fn kitty_event_types_mark_release() {
        let mode = TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let release = input(Key::Character("a".into()), ElementState::Released, None);
        assert_eq!(
            build_sequence(&release, ModifiersState::empty(), mode),
            b"\x1b[97;1:3u"
        );
    }

    #[test]
    fn kitty_left_shift_uses_its_dedicated_keysym() {
        let mut shift = input(Key::Named(NamedKey::Shift), ElementState::Pressed, None);
        shift.location = KeyLocation::Left;
        let bytes = build_sequence(
            &shift,
            ModifiersState::empty(),
            TermMode::REPORT_ALL_KEYS_AS_ESC,
        );
        // 按已协商键盘协议,修饰在 press 时作用于自身。
        assert_eq!(bytes, b"\x1b[57441;2u");
    }

    #[test]
    fn kitty_associated_text_appends_codepoints() {
        let mode = TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_ASSOCIATED_TEXT;
        let key = input(Key::Character("a".into()), ElementState::Pressed, Some("a"));
        assert_eq!(
            build_sequence(&key, ModifiersState::empty(), mode),
            b"\x1b[97;1;97u"
        );
    }

    /// 行为钉(留位):agent 检测归 AI 集成域,回填前 `program` 不消费——
    /// Windows 上 win32 记录模式恒保原生 Shift+Enter 记录,kitty 协商或传统
    /// VT 下 Shift+Enter 是 LF。
    #[test]
    fn shift_enter_as_lf_m3_ignores_program_until_agent_detection_lands() {
        assert!(shift_enter_as_lf(None, TermMode::empty()));
        assert!(shift_enter_as_lf(Some("claude"), TermMode::empty()));
        assert!(!shift_enter_as_lf(None, TermMode::WIN32_INPUT_MODE));
        assert!(!shift_enter_as_lf(
            Some("claude"),
            TermMode::WIN32_INPUT_MODE
        ));
        assert!(!shift_enter_as_lf(None, TermMode::DISAMBIGUATE_ESC_CODES));
    }
}
