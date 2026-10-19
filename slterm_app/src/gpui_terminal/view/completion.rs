//! 补全呈现接线:按键只更新输入快照与缓存键,候选在后台算。
//!
//! 双形态:Inline ghost 余量与 Popup 列表共用的计算核心;Hybrid 由
//! Tab 请求列表。取消令牌随任务释放,键变化/环境变化/退出都会让旧结果
//! 无法回填。

use std::rc::Rc;

use gpui::{AppContext as _, Context, Keystroke, Modifiers};

use super::TerminalView;
use crate::completion::{self, Cancellation, Session};
use crate::suggest::{CompletionStyle, SltermCompletionItem, SuggestEnv};
use slterm_terminal::term::TermMode;

/// 一次后台候选计算:任务本体 + 取消令牌。视图丢弃任务即通知计算放弃。
pub(crate) struct PendingCompletion {
    _task: gpui::Task<()>,
    cancellation: Cancellation,
}

impl PendingCompletion {
    fn new(task: gpui::Task<()>, cancellation: Cancellation) -> Self {
        Self {
            _task: task,
            cancellation,
        }
    }
}

impl Drop for PendingCompletion {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl TerminalView {
    /// 壳侧按 pane 持有来源缓存,这里只拿同一份句柄(不复制缓存)。
    pub(crate) fn attach_completion_session(&mut self, session: Rc<Session>) {
        self.completion_session = Some(session);
    }

    /// 当前输入快照的缓存键:输入 + 光标 + 呈现形态 + 命令代际。
    pub(crate) fn completion_key(&self, style: CompletionStyle) -> String {
        format!(
            "{}\0cursor={}\0syntax={:?}",
            completion::cache_key(&self.cwd, &SuggestEnv::Local, &self.suggest_line, style,),
            self.suggest_cursor.min(self.suggest_line.len()),
            self.term_mode().contains(TermMode::ALT_SCREEN),
        )
    }

    /// 后台重算 ghost/弹窗。只在按键/结果失效点调用,不在绘制回调里做 IO。
    pub(super) fn refresh_suggestion(&mut self, cx: &mut Context<'_, Self>) {
        let style = self
            .completion_style
            .active_style(self.suggest.popup_requested);
        let line = self.suggest_line.clone();
        if self.exited.is_some() || !self.ghost_enabled || line.is_empty() {
            self.suggestion_task = None;
            if self.suggest.popup_requested {
                self.suggest.popup_requested = false;
            }
            self.suggest.clear_completion_hints();
            return;
        }
        let key = self.completion_key(style);
        if self.suggest.completion_query_matches(&key) {
            if self.suggest.completion_query_ready(&key) {
                cx.notify();
            }
            return;
        }
        // Esc 关闭语义:行不变不重开;键保留,输入继续时自然失配。
        if self.suggest.suppressed_line.as_deref() == Some(line.as_str()) {
            self.suggest.suggestion_key = key;
            self.suggest.suggestion.clear();
            self.suggest.suggestion_edit = None;
            self.suggest.completion_items.clear();
            self.suggest.completion_selected = None;
            return;
        }
        self.suggest.suppressed_line = None;
        let Some(session) = self.completion_session.clone() else {
            return;
        };
        self.suggestion_task = None;
        self.suggest.begin_completion_query(key.clone());
        let cursor = self.suggest_cursor.min(line.len());
        let cancellation = Cancellation::default();
        let worker_cancellation = cancellation.clone();
        let request = session.request_at(
            self.cwd.clone(),
            SuggestEnv::Local,
            line.clone(),
            cursor,
            style,
            self.exec_context.as_ref(),
            // 方言是输入事实:由 Session 从冻结的执行环境推导,未证明时
            // 只接受通用字面量。
            None,
        );
        let calculation =
            cx.background_spawn(async move { request.calculate(&worker_cancellation) });
        let task = cx.spawn(async move |this, cx| {
            let result = calculation.await;
            let _ = this.update(cx, |view, cx| {
                // 键、环境、形态或会话一变,旧结果不得回填。
                let current_style = view
                    .completion_style
                    .active_style(view.suggest.popup_requested);
                if !view.suggest.completion_query_matches(&key)
                    || view.suggest_line != line
                    || current_style != style
                    || !view.ghost_enabled
                    || view.exited.is_some()
                {
                    return;
                }
                view.suggest.suggestion = result.suggestion;
                view.suggest.suggestion_edit = result.suggestion_edit;
                view.suggest.completion_items = result.completion_items;
                if view.suggest.popup_requested
                    && !view.suggest.completion_items.is_empty()
                    && view.suggest.completion_selected.is_none()
                {
                    view.suggest.completion_selected = Some(0);
                }
                view.suggest.finish_completion_query();
                cx.notify();
            });
        });
        self.suggestion_task = Some(PendingCompletion::new(task, cancellation));
    }

    /// 按键语义。返回 true = 终端已消费,不进入编码器。
    ///
    /// 顺序照抄上游:Esc 关闭 → 加载中 Enter/Tab 不提交 → 弹窗导航/接受 →
    /// Hybrid Tab 请求列表 → ghost 接受。
    pub(super) fn handle_completion_key(
        &mut self,
        key: &str,
        mods: &Modifiers,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let hybrid = self.completion_style == CompletionStyle::Hybrid;
        let popup_active = !self.suggest.completion_items.is_empty();
        if key == "escape" && (hybrid && self.suggest.popup_requested || popup_active) {
            let line = self.suggest_line.clone();
            self.suggestion_task = None;
            self.suggest.completion_popup_dismiss(&line);
            self.refresh_suggestion(cx);
            return true;
        }
        if self.suggest.popup_requested
            && !self.suggest.completion_query_ready(
                &self.completion_key(self.completion_style.active_style(true)),
            )
            && matches!(key, "enter" | "tab")
        {
            // 列表加载中:接受不能提交未编辑的行,也不能吞掉 Enter 的提交换行。
            if popup_active {
                return true;
            }
        }
        if popup_active {
            match key {
                "tab" if !hybrid => {
                    if self.suggest.completion_selected.is_none() {
                        self.suggest.completion_popup_move(1);
                    }
                    return self.accept_completion_popup(cx);
                }
                "tab" | "down" | "up" => {
                    self.suggest
                        .completion_popup_move(if key == "up" { -1 } else { 1 });
                    return true;
                }
                "enter" | "right" if self.suggest.completion_selected.is_some() => {
                    return self.accept_completion_popup(cx);
                }
                _ => {}
            }
        }
        if key == "tab" && hybrid && self.ghost_enabled && !self.suggest_line.is_empty() {
            // Tab 请求只改变呈现,不向 PTY 写入。
            self.suggest.request_completion_popup();
            self.refresh_suggestion(cx);
            return true;
        }
        let ghost_available = !self.suggest.suggestion.is_empty()
            || self
                .suggest
                .suggestion_edit
                .as_ref()
                .is_some_and(|item| item.replace_after_chars != 0);
        if ghost_available && (key == "right" || key == "tab" && !hybrid) {
            if let Some(item) = self.suggest.suggestion_edit.take() {
                self.suggest.suggestion.clear();
                return self.accept_completion_item(item, cx);
            }
            let ghost = std::mem::take(&mut self.suggest.suggestion);
            for ch in ghost.chars() {
                self.suggest_line.push(ch);
            }
            self.suggest_cursor = self.suggest_line.len();
            if !ghost.is_empty() {
                let bytes = ghost.as_bytes().to_vec();
                self.write_user_text(ghost, false, bytes, cx);
            }
            self.refresh_suggestion(cx);
            return true;
        }
        let _ = mods;
        false
    }

    /// 接受弹窗选中项。
    fn accept_completion_popup(&mut self, cx: &mut Context<'_, Self>) -> bool {
        let Some(item) = self.suggest.completion_popup_take() else {
            return false;
        };
        self.accept_completion_item(item, cx)
    }

    /// 把候选的编辑合同写回 PTY:先右移越过已证尾部,再退格替换已打部分,
    /// 最后键入余量。终端行编辑器证明过的尾部才允许删除。
    pub(super) fn accept_completion_item(
        &mut self,
        item: SltermCompletionItem,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let mode = self.term_mode();
        if item.replace_after_chars > 0 {
            let right = encode_named("right", &mode).unwrap_or_else(|| b"\x1b[C".to_vec());
            self.write_input(right.repeat(item.replace_after_chars), cx);
        }
        if item.replace_chars + item.replace_after_chars > 0 {
            let backspace = encode_named("backspace", &mode).unwrap_or_else(|| vec![0x7f]);
            self.write_input(
                backspace.repeat(item.replace_chars + item.replace_after_chars),
                cx,
            );
        }
        let mut head = self.suggest_line.clone();
        for _ in 0..item.replace_chars {
            head.pop();
        }
        head.push_str(&item.insert);
        self.suggest_line = head.clone();
        self.suggest_cursor = head.len();
        // 接受后整行已变化;抑制幽灵立即复现,直到下一次真实输入。
        self.suggest.suppressed_line = Some(head);
        self.suggest.completion_items.clear();
        self.suggest.completion_selected = None;
        self.suggest.popup_requested = false;
        if !item.insert.is_empty() {
            let bytes = item.insert.clone().into_bytes();
            self.write_user_text(item.insert, false, bytes, cx);
        }
        self.refresh_suggestion(cx);
        true
    }

    /// 键击进入行缓冲的镜像。只在真正产生文本的键上调用。
    pub(super) fn note_text_key(&mut self, keystroke: &Keystroke, cx: &mut Context<'_, Self>) {
        let mods = &keystroke.modifiers;
        if mods.control || mods.alt {
            return;
        }
        match keystroke.key.as_str() {
            "backspace" => {
                self.suggest_line.pop();
                self.suggest_cursor = self.suggest_line.len();
            }
            "enter" => {
                self.commit_line(cx);
                return;
            }
            // 光标移动/历史召回/中线删除后无法证明镜像等于屏幕真值;
            // 记录本轮不可信,Enter 时宁缺毋滥(上游 screen_line 纪律)。
            "left" | "right" | "home" | "end" | "up" | "down" | "delete" => {
                self.suggest_line_trusted = false;
                return;
            }
            "tab" | "escape" | "pageup" | "pagedown" | "insert" | "f1" | "f2" | "f3" | "f4"
            | "f5" | "f6" | "f7" | "f8" | "f9" | "f10" | "f11" | "f12" => return,
            _ => {
                let text = keystroke
                    .key_char
                    .clone()
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| keystroke.key.clone());
                if text.chars().count() != 1 || text.chars().any(char::is_control) {
                    return;
                }
                self.suggest_line.push_str(&text);
                self.suggest_cursor = self.suggest_line.len();
            }
        }
        self.refresh_suggestion(cx);
    }

    /// IME/粘贴文本进入行缓冲:换行视为提交边界。
    pub(super) fn note_text_input(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        let normalized = text.replace("\r\n", "\n");
        if let Some((head, _)) = normalized.split_once('\n') {
            self.suggest_line.push_str(head);
            self.suggest_cursor = self.suggest_line.len();
            self.commit_line(cx);
            return;
        }
        self.suggest_line.push_str(&normalized);
        self.suggest_cursor = self.suggest_line.len();
        self.refresh_suggestion(cx);
    }

    /// Enter 提交:历史进共享池、来源缓存代次 +1、行镜像清空。
    ///
    /// 网格读回(screen_line)是 Windows 下的权威来源,但本视图没有独立的
    /// 行编辑器快照;使用按键镜像并在记录前要求非空、无控制字符。
    pub(super) fn commit_line(&mut self, cx: &mut Context<'_, Self>) {
        let line = self.suggest_line.trim().to_owned();
        self.last_committed = line.clone();
        self.suggest_line.clear();
        self.suggest_cursor = 0;
        if let Some(session) = &self.completion_session {
            session.invalidate();
        }
        // 备用屏/Vi 模式里 Enter 属于前台应用(编辑器或 AI CLI 输入),
        // 不是 shell 提交:只清镜像,不污染历史。
        let shell_owned = !self
            .term_mode()
            .intersects(TermMode::ALT_SCREEN | TermMode::VI);
        if shell_owned
            && self.suggest_line_trusted
            && !line.is_empty()
            && !line.chars().any(char::is_control)
        {
            completion::record_command(&crate::history::HistoryScope::Local, &line, &self.cwd);
            cx.emit(super::TerminalViewEvent::CommandSubmitted(line));
        }
        self.suggest_line_trusted = true;
        self.suggest.clear_completion_hints();
        self.suggest.suppressed_line = None;
    }
}

/// 命名键编码:与真实按键走同一编码器,不另写一套特例。
fn encode_named(key: &str, mode: &TermMode) -> Option<Vec<u8>> {
    let keystroke = Keystroke {
        key: key.to_owned(),
        key_char: None,
        modifiers: Modifiers::default(),
    };
    crate::gpui_terminal::keymap::encode_for_program(&keystroke, mode, None)
}

/// 测试与调试用:当前行镜像。
#[cfg(all(test, feature = "gpui-test-support"))]
impl TerminalView {
    pub(crate) fn suggest_line(&self) -> &str {
        &self.suggest_line
    }
}
