use std::ops::{Index, IndexMut};

use vte::ansi::{NamedColor, Rgb};

/// Number of terminal colors.
pub const COUNT: usize = 269;

/// Array of indexed colors.
///
/// | Indices  | Description       |
/// | -------- | ----------------- |
/// | 0..16    | Named ANSI colors |
/// | 16..232  | Color cube        |
/// | 233..256 | Grayscale ramp    |
/// | 256      | Foreground        |
/// | 257      | Background        |
/// | 258      | Cursor            |
/// | 259..267 | Dim colors        |
/// | 267      | Bright foreground |
/// | 268      | Dim background    |
#[derive(Copy, Clone)]
pub struct Colors([Option<Rgb>; COUNT]);

impl Default for Colors {
    fn default() -> Self {
        Self([None; COUNT])
    }
}

impl Index<usize> for Colors {
    type Output = Option<Rgb>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Colors {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Index<NamedColor> for Colors {
    type Output = Option<Rgb>;

    #[inline]
    fn index(&self, index: NamedColor) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl IndexMut<NamedColor> for Colors {
    #[inline]
    fn index_mut(&mut self, index: NamedColor) -> &mut Self::Output {
        &mut self.0[index as usize]
    }
}

use crate::event::{Event, EventListener};

use super::{Term, TermMode};

impl<T: EventListener> Term<T> {
    /// 记录当前默认背景是暗还是亮，并在有 DECSET 2031 订阅时通知子进程。
    ///
    /// 宿主在终端 palette 热应用时调用（主题切换、跟随系统翻转、用户改
    /// `background=`）。值没变就什么都不做：热应用路径每次设置改动都会整体走一
    /// 遍，无条件上报会把重复字节塞进 PTY，落在 shell 提示符上就是一串垃圾。
    pub fn set_color_scheme(&mut self, is_dark: bool) {
        if self.color_scheme_dark == is_dark {
            return;
        }
        self.color_scheme_dark = is_dark;
        self.report_color_scheme();
    }

    /// 发 `CSI ? 997 ; 1 n`（暗）/ `CSI ? 997 ; 2 n`（亮）。
    ///
    /// 编号来自 color-palette-update-notifications 提案；主流终端实现采用
    /// 同一套应答格式。没订阅就不发——这个序列对没要求过它的程序来说是
    /// 不认识的输入。
    fn report_color_scheme(&mut self) {
        if !self.mode.contains(TermMode::COLOR_SCHEME_UPDATES) {
            return;
        }
        let value = if self.color_scheme_dark { 1 } else { 2 };
        self.event_proxy
            .send_event(Event::PtyWrite(format!("\x1b[?997;{value}n")));
    }
}
