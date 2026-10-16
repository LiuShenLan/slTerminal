//! GPUI 键盘适配层丢弃的原生字符数据。
//!
//! GPUI 把控制字符文本从 `key_char` 里滤掉(Ctrl+C 的 ETX 即其中之一);
//! Win32 记录路径(`gpui_terminal/keymap/win32.rs`)必须携带真实
//! `KEY_EVENT_RECORD` 字符值,否则 OpenConsole 翻译层丢弃 uChar=0 的键,
//! 字节流读者(Claude Code 等)收不到。

/// 恢复原生字符数据,不改变键盘组合状态。
pub(crate) fn native_character(
    virtual_key: u16,
    scan_code: u32,
    shift: bool,
    control: bool,
    alt: bool,
) -> u16 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ToUnicode, VK_CONTROL, VK_MENU, VK_SHIFT,
    };

    let mut state = [0_u8; 256];
    state[VK_SHIFT as usize] = if shift { 0x80 } else { 0 };
    state[VK_CONTROL as usize] = if control { 0x80 } else { 0 };
    state[VK_MENU as usize] = if alt { 0x80 } else { 0 };
    let mut text = [0_u16; 2];
    // SAFETY:两个缓冲区都满足 ToUnicode 要求的长度。flags 0x4 防止死键
    // 状态突变;0x1 选择 menu 翻译语义。
    let count = unsafe {
        ToUnicode(
            u32::from(virtual_key),
            scan_code,
            state.as_ptr(),
            text.as_mut_ptr(),
            text.len() as i32,
            0x5,
        )
    };
    // 0 是合法的「无文本」。绝不捏造半个 UTF-16 单元。
    if count == 1 { text[0] } else { 0 }
}
