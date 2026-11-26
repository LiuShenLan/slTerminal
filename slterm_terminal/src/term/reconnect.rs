//! 同一逻辑终端跨 PTY 代次的网格与事件代理衔接。

use super::*;

impl<T> Term<T> {
    /// 保留主屏回滚，清理旧进程交互态并插入语言中立的会话分隔。
    pub fn begin_reconnect(&mut self)
    where
        T: EventListener,
    {
        // 先推进代次：旧 worker 的显示命令、输出解析与退出通知从此失效。
        self.display_epoch = self.display_epoch.wrapping_add(1);
        if self.mode.contains(TermMode::ALT_SCREEN) {
            self.swap_alt();
        }
        self.active_charset = Default::default();
        self.cursor_style = None;
        self.cursor_blinking_override = None;
        self.slterm_prompt_marks.clear();
        self.slterm_prompt_input = None;
        self.slterm_prompt_active = false;
        self.scroll_region = Line(0)..Line(self.screen_lines() as i32);
        self.tabs = TabStops::new(self.columns());
        self.title_stack.clear();
        self.title = None;
        self.selection = None;
        self.vi_mode_cursor = Default::default();
        self.keyboard_mode_stack = Default::default();
        self.inactive_keyboard_mode_stack = Default::default();
        self.mode = TermMode::default() | (self.mode & TermMode::VI);
        self.scroll_display(Scroll::Bottom);
        self.carriage_return();
        self.linefeed();
        for _ in 0..self.columns() {
            self.input('─');
        }
        self.carriage_return();
        self.linefeed();
        self.mark_fully_damaged();
    }

    /// 重连后终端语义事件只能进入新 worker 的订阅通道。
    pub fn replace_event_proxy(&mut self, event_proxy: T) {
        self.event_proxy = event_proxy;
    }

    /// 当前网格显示代次；旧 worker 在重连后据此拒绝写入。
    pub fn display_epoch(&self) -> u64 {
        self.display_epoch
    }
}
