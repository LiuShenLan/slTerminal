//! 指针交互:选区、鼠标协议上报、滚轮滚动、选区拖拽自动回滚。
//!
//! 裁剪(归属见字段节):滚动条 overlay 族(scrollbar_thumb/grab/
//! drag,随分屏渲染扩)、补全浮层族(08)、OSC8 链接族(07/09 裁定)、
//! focus_follows_mouse(06 设置键)、Ctrl+滚轮字号缩放(06,保留消费不缩放)。

use super::*;

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

impl TerminalView {
    /// 像素坐标 → 网格坐标(含滚动偏移)与半格判定。
    pub(super) fn grid_point(&self, position: Point<Pixels>) -> (TermPoint, Side) {
        let rel_x = (position.x - self.origin.x).as_f32() / self.cell_width.as_f32().max(1.0);
        let rel_y = (position.y - self.origin.y).as_f32() / self.line_height.as_f32().max(1.0);
        let col = (rel_x.floor().max(0.0) as usize).min(self.cols.saturating_sub(1));
        let row = (rel_y.floor().max(0.0) as usize).min(self.rows.saturating_sub(1));
        let side = if rel_x.fract() > 0.5 {
            Side::Right
        } else {
            Side::Left
        };
        let Some(session) = self.session.as_ref() else {
            return (TermPoint::new(Line(row as i32), Column(col)), side);
        };
        let term = session.term.lock();
        let point = term.visual_viewport_to_point(self.rows, TermPoint::new(row, Column(col)));
        (point, side)
    }

    pub(super) fn selection_is_empty(&self) -> bool {
        self.session.as_ref().is_none_or(|session| {
            session
                .term
                .lock()
                .selection
                .as_ref()
                .is_none_or(|selection| selection.is_empty())
        })
    }

    /// 应用是否接管了鼠标(vim/htop 等)。Shift 按住时强制旁路——这是
    /// 终端的通用逃生门:应用吃鼠标时用户仍能选择/复制。
    pub(super) fn mouse_mode_active(&self, mods: &gpui::Modifiers) -> bool {
        !mods.shift && self.term_mode().intersects(TermMode::MOUSE_MODE)
    }

    /// 把一次鼠标事件按当前协议(SGR/normal/UTF-8)编码上报给应用。
    pub(super) fn send_mouse_report(
        &mut self,
        position: Point<Pixels>,
        button: u8,
        pressed: bool,
        mods: &gpui::Modifiers,
    ) {
        let (point, _) = self.grid_point(position);
        self.last_report_point = Some(point);
        let mode = self.term_mode();
        let mods = mouse_protocol::ReportMods {
            shift: mods.shift,
            alt: mods.alt,
            control: mods.control,
        };
        if let Some(bytes) = mouse_protocol::report(&mode, point, button, pressed, mods) {
            self.write_bytes(bytes);
        }
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        if self.session.is_none() {
            return;
        }
        if self.mouse_mode_active(&event.modifiers) {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_LEFT,
                true,
                &event.modifiers,
            );
            return;
        }
        let (point, side) = self.grid_point(event.position);
        let ty = match event.click_count {
            1 => SelectionType::Simple,
            2 => SelectionType::Semantic,
            _ => SelectionType::Lines,
        };
        if let Some(session) = &self.session {
            let mut term = session.term.lock();
            // Shift+单击扩展既有选区到点击处(原生文本框行为,对齐上游);
            // 其余情况都开新选区。
            let extend = event.modifiers.shift
                && event.click_count == 1
                && term.selection.as_ref().is_some_and(|s| !s.is_empty());
            match term.selection.as_mut() {
                Some(selection) if extend => selection.update(point, side),
                _ => term.selection = Some(Selection::new(ty, point, side)),
            }
        }
        self.selecting = true;
        // 自动回滚在这里就上弦,而不是等第一次「拖出网格」的移动事件:指针
        // 一旦离开元素 hitbox 就不再有 move 送进来,一甩到顶的拖法最后一个
        // 元素内事件往往还在网格中间,等 move 判定=永远等不到。tick 自己看
        // 指针位置,网格内是一次比较就返回的空转。
        self.start_selection_scroll(window, cx);
        cx.notify();
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.selecting {
            if event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let (point, side) = self.grid_point(event.position);
            if let Some(session) = &self.session
                && let Some(selection) = session.term.lock().selection.as_mut()
            {
                selection.update(point, side);
            }
            // 越界回滚不在这里判:链在按下时就起了(见 start_selection_scroll),
            // 指针是否出了网格由 tick 自己看。
            cx.notify();
            return;
        }
        if self.mouse_mode_active(&event.modifiers) {
            // 鼠标模式的移动上报:拖动 = 按钮码+32(需 DRAG 或 MOTION 任一),
            // 无按键纯移动 = 35(仅 MOTION);同一单元格内的移动不重报。
            let mode = self.term_mode();
            if !mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
                return;
            }
            let button = match event.pressed_button {
                Some(MouseButton::Left) => {
                    mouse_protocol::BUTTON_LEFT + mouse_protocol::DRAG_OFFSET
                }
                Some(MouseButton::Middle) => {
                    mouse_protocol::BUTTON_MIDDLE + mouse_protocol::DRAG_OFFSET
                }
                Some(MouseButton::Right) => {
                    mouse_protocol::BUTTON_RIGHT + mouse_protocol::DRAG_OFFSET
                }
                _ => {
                    if !mode.contains(TermMode::MOUSE_MOTION) {
                        return;
                    }
                    mouse_protocol::MOTION_ONLY
                }
            };
            let (point, _) = self.grid_point(event.position);
            if self.last_report_point == Some(point) {
                return;
            }
            self.send_mouse_report(event.position, button, true, &event.modifiers);
        }
    }

    pub(super) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.stop_selection_scroll();
        if !self.selecting && self.mouse_mode_active(&event.modifiers) {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_LEFT,
                false,
                &event.modifiers,
            );
            return;
        }
        self.selecting = false;
        if self.copy_on_select {
            self.copy_selection(false, window, cx);
        }
        cx.notify();
    }

    /// 指针在终端之外松手(按下后甩出窗口——Windows 平台按下即
    /// `SetCapture`,事件仍会送达)。GPUI 的 `on_mouse_up` 只在 hitbox 命中
    /// 时派发,没有这条兜底,`selecting` 会一直停在按下态:自动回滚停不
    /// 下来,下一次移动还会继续改选区。
    pub(super) fn on_mouse_up_out(
        &mut self,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.stop_selection_scroll();
        if !self.selecting {
            return;
        }
        self.selecting = false;
        // 指针不在终端上,只补选中即复制这一条收尾。
        if self.copy_on_select {
            self.copy_selection(false, window, cx);
        }
        cx.notify();
    }

    /// 网格的上下边界(窗口坐标),自动回滚的触发线。与上游同义:
    /// 卡内 8px 呼吸边距之内就已经算「出了网格」。
    pub(super) fn grid_vertical_bounds(&self) -> (f32, f32) {
        let top = self.origin.y.as_f32();
        (top, top + self.line_height.as_f32() * self.rows as f32)
    }

    pub(super) fn selection_scroll_lines_at(&self, y: f32) -> i32 {
        let (top, bottom) = self.grid_vertical_bounds();
        selection_scroll_lines(y, top, bottom, self.rows as i32)
    }

    /// 选区拖拽全程常驻的自动回滚链:按下即起,松手才停。
    ///
    /// 不用「移动越界才起链」是因为那条判据依赖 move 事件送得到——指针离开
    /// 元素 hitbox 后 GPUI 就不再派发,一甩到顶的拖法最后一个元素内事件通常
    /// 还在网格中间,链于是永远起不来。常驻链把「是否越界」挪进 tick,指针
    /// 在网格内时它只是一次比较加返回,不重绘也不取 Term 锁。
    pub(super) fn start_selection_scroll(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if self.selection_scroll_active {
            return;
        }
        self.selection_scroll_active = true;
        self.selection_scroll_epoch = self.selection_scroll_epoch.wrapping_add(1);
        let epoch = self.selection_scroll_epoch;
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                executor.timer(SELECTION_SCROLL_INTERVAL).await;
                let keep = this.update_in(cx, |view, window, cx| {
                    view.selection_scroll_tick(epoch, window, cx)
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn stop_selection_scroll(&mut self) {
        if !self.selection_scroll_active {
            return;
        }
        self.selection_scroll_active = false;
        self.selection_scroll_epoch = self.selection_scroll_epoch.wrapping_add(1);
    }

    /// 一次自动回滚 tick:越界就滚 N 行,并把选区末端跟到指针**此刻**所在
    /// 的格;没越界就什么都不做,等下一次 tick。
    ///
    /// 位置每 tick 从 `window.mouse_position()` 现取而不是沿用最后一次 move:
    /// 指针停在元素外时根本不会再有 move 事件,沿用旧值就等于速度被钉死在
    /// 贴边那一档。
    ///
    /// 返回 `false` 表示这条定时器链该结束(松手、换会话,或被新链顶掉)。
    pub(super) fn selection_scroll_tick(
        &mut self,
        epoch: u64,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        if epoch != self.selection_scroll_epoch || !self.selection_scroll_active {
            return false;
        }
        if !self.selecting || self.session.is_none() {
            self.stop_selection_scroll();
            return false;
        }
        // 按下但还没拖出一个格:要求选区非空才滚,否则在卡片边距上按一下
        // 不动就会开始翻历史。
        if self.selection_is_empty() {
            return true;
        }
        let position = window.mouse_position();
        let lines = self.selection_scroll_lines_at(position.y.as_f32());
        if lines == 0 {
            return true;
        }
        if let Some(session) = &self.session {
            session.term.lock().scroll_display(Scroll::Delta(lines));
        }
        // 滚动换了视口到绝对行的映射,选区末端必须按滚动后的网格重算——
        // 否则视口滑过去了,选区还钉在原来那几行。两段各自取一次锁:
        // `grid_point` 内部还要读 Term,握着锁进去会自锁。
        let (point, side) = self.grid_point(position);
        if let Some(session) = &self.session
            && let Some(selection) = session.term.lock().selection.as_mut()
        {
            selection.update(point, side);
        }
        cx.notify();
        true
    }

    /// 右键:无上下文菜单——有选区复制、无选区粘贴
    /// (对齐上游 copy_on_select=true 形态;菜单请求事件不立)。
    pub(super) fn on_right_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        if self.session.is_none() {
            return;
        }
        if self.mouse_mode_active(&event.modifiers) && !event.modifiers.control {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_RIGHT,
                true,
                &event.modifiers,
            );
            return;
        }
        let selected_text = self
            .session
            .as_ref()
            .and_then(|session| session.term.lock().selection_to_string())
            .filter(|text| !text.is_empty());
        if selected_text.is_some() {
            self.copy_selection(true, window, cx);
        } else {
            self.paste(window, cx);
        }
        cx.stop_propagation();
    }

    pub(super) fn on_right_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.mouse_mode_active(&event.modifiers) && !event.modifiers.control {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_RIGHT,
                false,
                &event.modifiers,
            );
        }
        cx.notify();
    }

    /// 中键只在鼠标模式下有意义(上报给应用);Windows 上没有中键粘贴路径,
    /// 与上游保持一致。
    pub(super) fn on_middle_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        if self.mouse_mode_active(&event.modifiers) {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_MIDDLE,
                true,
                &event.modifiers,
            );
        }
        cx.notify();
    }

    pub(super) fn on_middle_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.mouse_mode_active(&event.modifiers) {
            self.send_mouse_report(
                event.position,
                mouse_protocol::BUTTON_MIDDLE,
                false,
                &event.modifiers,
            );
        }
        cx.notify();
    }

    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let delta_y = event.delta.pixel_delta(self.line_height).y.as_f32();
        // Ctrl+滚轮 = 字号缩放手势位:键域归 06,这里只消费不缩放——
        // 不消费的话手势会落进下面的滚动逻辑,缩放松手即翻屏。
        if event.modifiers.control && !event.modifiers.alt && delta_y != 0.0 {
            cx.stop_propagation();
            return;
        }
        let speed = cx
            .try_global::<Settings>()
            .map_or(1.0, |settings| settings.scroll_speed);
        let lines = wheel_scroll_lines(&mut self.scroll_px, event.delta, self.line_height, speed);
        if lines == 0 {
            return;
        }
        let mode = self.term_mode();
        // 应用接管鼠标时滚轮也归应用(htop 列表滚动);Shift 旁路回本地回滚。
        if !event.modifiers.shift && mode.intersects(TermMode::MOUSE_MODE) {
            let code = if lines > 0 {
                mouse_protocol::WHEEL_UP
            } else {
                mouse_protocol::WHEEL_DOWN
            };
            for _ in 0..lines.unsigned_abs() {
                self.send_mouse_report(event.position, code, true, &event.modifiers);
            }
            cx.notify();
            return;
        }
        let Some(session) = &self.session else { return };
        if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            // 备用屏(less/vim):滚轮翻译成方向键。
            let seq: &[u8] = if mode.contains(TermMode::APP_CURSOR) {
                if lines > 0 { b"\x1bOA" } else { b"\x1bOB" }
            } else if lines > 0 {
                b"\x1b[A"
            } else {
                b"\x1b[B"
            };
            let mut bytes = Vec::new();
            for _ in 0..lines.unsigned_abs() {
                bytes.extend_from_slice(seq);
            }
            use slterm_terminal::event::Notify as _;
            session.notifier.notify(bytes);
        } else {
            session.term.lock().scroll_display(Scroll::Delta(lines));
        }
        cx.notify();
    }
}

/// 跨事件保留小数增量:滚轮速度不得重缩放像素精确的触控板输入。
fn wheel_scroll_lines(
    remainder: &mut f32,
    delta: gpui::ScrollDelta,
    line_height: Pixels,
    speed: f32,
) -> i32 {
    let pixels = delta.pixel_delta(line_height).y.as_f32();
    let pixels = match delta {
        gpui::ScrollDelta::Lines(_) => pixels * speed,
        gpui::ScrollDelta::Pixels(_) => pixels,
    };
    if !pixels.is_finite() {
        return 0;
    }
    let height = line_height.as_f32().max(1.0);
    *remainder += pixels;
    let lines = (*remainder / height).trunc() as i32;
    *remainder -= lines as f32 * height;
    lines
}

#[cfg(test)]
mod scrolling_tests {
    use super::wheel_scroll_lines;
    use gpui::{ScrollDelta, point, px};

    #[test]
    fn normal_wheel_speed_preserves_existing_steps() {
        let mut remainder = 0.0;
        assert_eq!(
            wheel_scroll_lines(
                &mut remainder,
                ScrollDelta::Lines(point(0.0, 3.0)),
                px(20.0),
                1.0
            ),
            3
        );
        assert_eq!(
            wheel_scroll_lines(
                &mut remainder,
                ScrollDelta::Lines(point(0.0, -3.0)),
                px(20.0),
                2.0
            ),
            -6
        );
        assert_eq!(remainder, 0.0);
    }

    #[test]
    fn slow_wheel_accumulates_without_dropping_small_inputs() {
        let mut remainder = 0.0;
        let lines: Vec<_> = (0..4)
            .map(|_| {
                wheel_scroll_lines(
                    &mut remainder,
                    ScrollDelta::Lines(point(0.0, 1.0)),
                    px(20.0),
                    0.25,
                )
            })
            .collect();
        assert_eq!(lines, [0, 0, 0, 1]);
        assert_eq!(remainder, 0.0);
    }

    #[test]
    fn trackpad_pixels_are_independent_of_wheel_speed_and_keep_direction() {
        for speed in [0.25, 1.0, 4.0] {
            let mut remainder = 0.0;
            let delta = ScrollDelta::Pixels(point(px(0.0), px(5.0)));
            assert_eq!(
                (0..4)
                    .map(|_| wheel_scroll_lines(&mut remainder, delta, px(20.0), speed))
                    .sum::<i32>(),
                1
            );
            let delta = ScrollDelta::Pixels(point(px(0.0), px(-20.0)));
            assert_eq!(
                wheel_scroll_lines(&mut remainder, delta, px(20.0), speed),
                -1
            );
            assert_eq!(remainder, 0.0);
        }
    }
}
