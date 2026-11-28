//! 补全呈现接线:按键只更新输入快照与缓存键,候选在后台算。
//!
//! 双形态:Inline ghost 余量与 Popup 列表共用的计算核心;Hybrid 由
//! Tab 请求列表。取消令牌随任务释放,键变化/环境变化/退出都会让旧结果
//! 无法回填。

use std::rc::Rc;

use gpui::{AppContext as _, Context, Keystroke, Modifiers};

use super::TerminalView;
use crate::completion::{self, Cancellation, Session};
use crate::gpui_terminal::registry::TerminalTarget;
use crate::suggest::{Candidates, CompletionStyle, SltermCompletionItem, SuggestEnv};
use slterm_terminal::term::TermMode;

/// 一次补全请求与回包共用的输入身份快照。
///
/// 生命周期身份（PTY 目标、提示符代次）与输入身份（行、revision、键）
/// 在这里汇合；回包必须逐项与当前视图一致才可回填。
pub(super) struct CompletionQuery {
    key: String,
    target: Option<TerminalTarget>,
    prompt_epoch: Option<u64>,
    line: String,
    revision: u64,
    style: CompletionStyle,
}

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
        let line = self.shell_line.line();
        let (learning_generation, command_generation) =
            self.completion_session.as_ref().map_or((0, 0), |session| {
                (session.learning_generation(), session.command_generation())
            });
        format!(
            "{}\0cursor={}\0syntax={:?}\0revision={}",
            completion::cache_key(
                &self.cwd,
                &SuggestEnv::Local,
                line,
                style,
                learning_generation,
                command_generation,
            ),
            line.len(),
            self.term_mode().contains(TermMode::ALT_SCREEN),
            self.shell_line.revision(),
        )
    }

    /// 丢弃候选与在途计算；不依赖 `cx`，供帧采纳的信任边界直接调用。
    pub(super) fn cancel_completion(&mut self) {
        self.suggestion_task = None;
        if self.suggest.popup_requested {
            self.suggest.popup_requested = false;
        }
        self.suggest.clear_completion_hints();
    }

    /// 冻结一次请求的身份；后台回包只按这份快照复核。
    fn completion_query(
        &self,
        style: CompletionStyle,
        key: String,
        line: String,
        revision: u64,
    ) -> CompletionQuery {
        CompletionQuery {
            key,
            target: self.binding.as_ref().map(|binding| binding.target()),
            prompt_epoch: self.shell_line.prompt_epoch(),
            line,
            revision,
            style,
        }
    }

    /// 回包唯一入口：PTY 目标、提示符代次、键、行与 revision 任一变化
    /// 都拒绝回填。
    pub(super) fn apply_completion_result(
        &mut self,
        query: &CompletionQuery,
        result: Candidates,
    ) -> bool {
        let current_style = self
            .completion_style
            .active_style(self.suggest.popup_requested);
        let current_target = self.binding.as_ref().map(|binding| binding.target());
        if current_target != query.target
            || self.shell_line.prompt_epoch() != query.prompt_epoch
            || !self.suggest.completion_query_matches(&query.key)
            || self.shell_line.completion_line() != Some(query.line.as_str())
            || self.shell_line.revision() != query.revision
            || current_style != query.style
            || !self.ghost_enabled
            || self.exited.is_some()
        {
            return false;
        }
        self.suggest.suggestion = result.suggestion;
        self.suggest.suggestion_edit = result.suggestion_edit;
        self.suggest.completion_items = result.completion_items;
        self.suggest.finish_completion_query();
        true
    }

    /// 后台重算 ghost/弹窗。只在按键/结果失效点调用,不在绘制回调里做 IO。
    pub(super) fn refresh_suggestion(&mut self, cx: &mut Context<'_, Self>) {
        let style = self
            .completion_style
            .active_style(self.suggest.popup_requested);
        let line = self.shell_line.completion_line().map(str::to_owned);
        if self.exited.is_some() || !self.ghost_enabled || line.as_deref().is_none_or(str::is_empty)
        {
            self.cancel_completion();
            return;
        }
        let line = line.expect("可信非空行已经预检");
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
        let cursor = line.len();
        let cancellation = Cancellation::default();
        let worker_cancellation = cancellation.clone();
        let input_revision = self.shell_line.revision();
        let request = session.request_at_revision(
            self.cwd.clone(),
            SuggestEnv::Local,
            line.clone(),
            cursor,
            style,
            self.exec_context.as_ref(),
            // 方言是输入事实:由 Session 从冻结的受信 Shell 推导,未证明时
            // 只接受通用字面量。
            input_revision,
        );
        let query = self.completion_query(style, key, line.clone(), input_revision);
        let calculation =
            cx.background_spawn(async move { request.calculate(&worker_cancellation) });
        let task = cx.spawn(async move |this, cx| {
            let result = calculation.await;
            let _ = this.update(cx, |view, cx| {
                if view.apply_completion_result(&query, result) {
                    cx.notify();
                }
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
        // 补全键只服务未修饰按键；Ctrl/Alt/Shift 组合属于终端或全局命令。
        if mods.modified() {
            return false;
        }
        let hybrid = self.completion_style == CompletionStyle::Hybrid;
        let popup_active = !self.suggest.completion_items.is_empty();
        if key == "escape" && (hybrid && self.suggest.popup_requested || popup_active) {
            let line = self.shell_line.line().to_owned();
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
        if key == "tab"
            && hybrid
            && self.ghost_enabled
            && self
                .shell_line
                .completion_line()
                .is_some_and(|line| !line.is_empty())
        {
            // Tab 请求只改变呈现,不向 PTY 写入。
            self.suggest.request_completion_popup();
            self.refresh_suggestion(cx);
            return true;
        }
        let accept = cx
            .try_global::<crate::app_shell::config::Settings>()
            .map(|settings| settings.accept)
            .unwrap_or_default();
        let accept_enabled = Self::ghost_accept_key_enabled(key, accept)
            // Hybrid 的 Tab 已归弹窗请求，不再接受 ghost 余量。
            && !(key == "tab" && hybrid);
        // 只接受带编辑合同的 ghost；没有 `suggestion_edit` 的裸展示文本
        // 不得绕过原文/revision 复核写 PTY。
        if accept_enabled && let Some(item) = self.suggest.suggestion_edit.take() {
            if item.edit.span().start != item.edit.span().end || !item.edit.insert().is_empty() {
                self.suggest.suggestion.clear();
                return self.accept_completion_item(item, cx);
            }
            // 空编辑合同的残留候选不具备接受语义，按无候选处理。
            self.suggest.suggestion.clear();
        }
        false
    }

    /// 接受弹窗选中项。
    fn accept_completion_popup(&mut self, cx: &mut Context<'_, Self>) -> bool {
        let Some(item) = self.suggest.completion_popup_take() else {
            return false;
        };
        self.accept_completion_item(item, cx)
    }

    /// 把候选的编辑合同写回 PTY：可信镜像只支持行尾替换，先退格再键入余量。
    pub(super) fn accept_completion_item(
        &mut self,
        item: SltermCompletionItem,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(line) = self.shell_line.completion_line().map(str::to_owned) else {
            return false;
        };
        let Ok(head) = item.edit.apply(&line, self.shell_line.revision()) else {
            self.suggest.clear_completion_hints();
            self.refresh_suggestion(cx);
            return true;
        };
        let mode = self.term_mode();
        let span = item.edit.span();
        if span.end != line.len() {
            // 可信镜像只覆盖行尾编辑；中间游标候选不得猜测 Readline 操作。
            self.suggest.clear_completion_hints();
            self.refresh_suggestion(cx);
            return true;
        }
        let replace_chars = line[span.start..].chars().count();
        if replace_chars > 0 {
            let backspace = encode_named("backspace", &mode).unwrap_or_else(|| vec![0x7f]);
            self.write_input(backspace.repeat(replace_chars), cx);
        }
        self.shell_line.replace_trusted_line(head.clone());
        // 接受后整行已变化;抑制幽灵立即复现,直到下一次真实输入。
        self.suggest.suppressed_line = Some(head);
        self.suggest.completion_items.clear();
        self.suggest.completion_selected = None;
        self.suggest.popup_requested = false;
        let insert = item.edit.insert().to_owned();
        if !insert.is_empty() {
            let bytes = insert.clone().into_bytes();
            self.write_user_text(insert, false, bytes, cx);
        }
        self.refresh_suggestion(cx);
        true
    }

    /// 接受键设置门控 ghost 余量：Right/Tab/Both 三档只放行声明的键。
    fn ghost_accept_key_enabled(key: &str, accept: slterm_settings::AcceptKeyName) -> bool {
        use slterm_settings::AcceptKeyName;
        matches!(
            (key, accept),
            ("right", AcceptKeyName::Right | AcceptKeyName::Both)
                | ("tab", AcceptKeyName::Tab | AcceptKeyName::Both)
        )
    }

    /// 键击进入行缓冲的镜像。只在真正产生文本的键上调用。
    pub(super) fn note_text_key(&mut self, keystroke: &Keystroke, cx: &mut Context<'_, Self>) {
        let mods = &keystroke.modifiers;
        if mods.control || mods.alt {
            if keystroke.key == "c" && mods.control {
                self.shell_line.interrupt();
            } else {
                self.shell_line.invalidate();
            }
            self.refresh_suggestion(cx);
            return;
        }
        match keystroke.key.as_str() {
            "backspace" => {
                self.shell_line.backspace();
            }
            "enter" => {
                self.commit_line(cx);
                return;
            }
            // 任何非纯文本编辑都无法证明本地镜像仍等于 Shell 行。
            _ if keystroke.key_char.as_deref().is_none_or(str::is_empty) => {
                self.shell_line.invalidate();
                self.refresh_suggestion(cx);
                return;
            }
            _ => {
                let text = keystroke.key_char.as_deref().unwrap_or(&keystroke.key);
                if text.chars().count() != 1 || text.chars().any(char::is_control) {
                    return;
                }
                self.shell_line.note_text(text);
            }
        }
        self.refresh_suggestion(cx);
    }

    /// IME/粘贴文本进入行缓冲:换行视为提交边界。
    pub(super) fn note_text_input(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        let normalized = text.replace("\r\n", "\n");
        if let Some((head, _)) = normalized.split_once('\n') {
            self.shell_line.note_text(head);
            self.commit_line(cx);
            return;
        }
        self.shell_line.note_text(&normalized);
        self.refresh_suggestion(cx);
    }

    /// Enter 只建立待决执行；历史写入由 CommandStart→CommandDone 闭合。
    pub(super) fn commit_line(&mut self, _cx: &mut Context<'_, Self>) {
        self.shell_line.submit();
        // 提交后该行不再可编辑；在途请求必须取消，迟到回执不得再显示候选。
        self.suggestion_task = None;
        if let Some(session) = &self.completion_session {
            session.invalidate();
        }
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
        self.shell_line.line()
    }

    /// 捕获当前输入身份，供迟到结果用例注入旧代次回包。
    pub(super) fn completion_query_for_test(&self) -> CompletionQuery {
        let style = self
            .completion_style
            .active_style(self.suggest.popup_requested);
        CompletionQuery {
            key: self.suggest.suggestion_key.clone(),
            target: self.binding.as_ref().map(|binding| binding.target()),
            prompt_epoch: self.shell_line.prompt_epoch(),
            line: self
                .shell_line
                .completion_line()
                .unwrap_or_default()
                .to_owned(),
            revision: self.shell_line.revision(),
            style,
        }
    }

    /// 走生产的回包复核入口；返回是否真正回填。
    pub(super) fn apply_completion_result_for_test(
        &mut self,
        query: &CompletionQuery,
        result: Candidates,
    ) -> bool {
        self.apply_completion_result(query, result)
    }

    /// 经真实 VT 提示符标记建立测试镜像，不直接伪造 Trusted 状态。
    pub(crate) fn establish_test_prompt(&mut self) {
        self.shell_line = crate::gpui_terminal::shell_line::ShellLineTracker::new(Some("cmd.exe"));
        self.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07");
    }
}

#[cfg(test)]
mod completion_key_tests {
    use super::TerminalView;
    use slterm_settings::AcceptKeyName;

    /// Right/Tab/Both 三档只放行各自声明的 ghost 接受键。
    #[test]
    fn accept_key_setting_selects_ghost_acceptance_keys() {
        assert!(TerminalView::ghost_accept_key_enabled(
            "right",
            AcceptKeyName::Right
        ));
        assert!(!TerminalView::ghost_accept_key_enabled(
            "tab",
            AcceptKeyName::Right
        ));
        assert!(TerminalView::ghost_accept_key_enabled(
            "tab",
            AcceptKeyName::Tab
        ));
        assert!(!TerminalView::ghost_accept_key_enabled(
            "right",
            AcceptKeyName::Tab
        ));
        assert!(TerminalView::ghost_accept_key_enabled(
            "right",
            AcceptKeyName::Both
        ));
        assert!(TerminalView::ghost_accept_key_enabled(
            "tab",
            AcceptKeyName::Both
        ));
        assert!(!TerminalView::ghost_accept_key_enabled(
            "enter",
            AcceptKeyName::Both
        ));
    }
}
