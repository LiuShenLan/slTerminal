//! Runtime 写命令的终端侧入口(04 篇 M7.5)。
//!
//! Workspace 只在 UI owner 上调用这些方法:输入经既有 `write_input` /
//! bracketed-paste 通路进 PTY,读取直读真实 Grid 尾部。`RuntimeAgent`/
//! `state_change_seq` 的记账在 hub,视图只管终端字节与只读投影。

use gpui::{Context, Keystroke, Modifiers};

use super::TerminalView;
use crate::gpui_terminal::keymap;
use crate::runtime_api::{
    ApiError, RuntimeKey, RuntimeKeyModifiers, RuntimePaneRead, RuntimeTaskState,
    capture_terminal_tail,
};

impl TerminalView {
    /// `pane.prompt` / `agent.prompt`:单行文本;`submit` 时同步追加 Enter。
    pub(crate) fn runtime_prompt(
        &mut self,
        text: &str,
        submit: bool,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), ApiError> {
        self.require_live_session()?;
        let mut bytes = text.as_bytes().to_vec();
        if submit {
            bytes.push(b'\r');
            self.note_text_input(text, cx);
            self.commit_line(cx);
        } else {
            self.note_text_input(text, cx);
        }
        self.write_input(bytes, cx);
        Ok(())
    }

    /// run 记账:调用方(Runtime Run)在提交后把同一 run 身份挂到视图。
    pub(crate) fn attach_run(&mut self, run: crate::runtime_api::RuntimePaneRun) {
        self.active_run = Some(run);
        self.last_run = None;
    }

    /// `pane.paste` / `agent.paste`:与 Ctrl+Shift+V 同一条 bracketed-paste
    /// 通路;`submit` 在收尾哨兵之后追加 Enter。
    pub(crate) fn runtime_paste(
        &mut self,
        text: &str,
        submit: bool,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), ApiError> {
        self.require_live_session()?;
        // paste_now_impl 已把文本镜像进行缓冲(emit=true)。
        self.paste_now_impl(text, true, cx);
        if submit {
            self.commit_line(cx);
            self.write_input(b"\r".to_vec(), cx);
        }
        Ok(())
    }

    /// `pane.send_key`:命名键白名单在 transport 侧校验;这里只做终端模式
    /// 相关的编码,字母键必须带 control(校验已保证)。
    pub(crate) fn runtime_send_key(
        &mut self,
        key: RuntimeKey,
        modifiers: RuntimeKeyModifiers,
        repeat: u16,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), ApiError> {
        self.require_live_session()?;
        let keystroke = Keystroke {
            key: gpui_key_name(key).to_owned(),
            key_char: None,
            modifiers: Modifiers {
                shift: modifiers.shift,
                alt: modifiers.alt,
                control: modifiers.control,
                ..Modifiers::default()
            },
        };
        let mode = self.term_mode();
        for _ in 0..repeat.max(1) {
            let Some(bytes) = keymap::encode_for_program(&keystroke, &mode, None) else {
                return Err(ApiError::new(
                    "invalid_params",
                    format!(
                        "key {:?} cannot be encoded in the current terminal mode",
                        key.as_str()
                    ),
                ));
            };
            self.write_input(bytes, cx);
        }
        Ok(())
    }

    /// `pane.read` / `agent.read`:真实 Grid 尾部,范围锚定 buffer 底部。
    pub(crate) fn runtime_read(
        &self,
        window_id: u64,
        pane_id: u64,
        lines: usize,
        task_state: RuntimeTaskState,
    ) -> Result<RuntimePaneRead, ApiError> {
        let Some(session) = &self.session else {
            return Err(pane_not_running());
        };
        let term = session.term.lock();
        capture_terminal_tail(
            &term,
            window_id,
            pane_id,
            lines,
            task_state,
            self.exited.is_some(),
            self.exited.clone(),
        )
    }

    /// `pane.procs`:PTY 直系 shell PID(0 = 无存活子进程)。
    pub(crate) fn runtime_shell_pid(&self) -> Option<u32> {
        self.session
            .as_ref()
            .map(|session| session.shell_pid)
            .filter(|pid| *pid != 0)
    }

    /// `pane.exec`:PTY 创建时冻结的环境/cwd 快照。
    pub(crate) fn runtime_exec_context(&self) -> Option<crate::runtime_exec::PaneExecContext> {
        self.exec_context.clone()
    }

    fn require_live_session(&self) -> Result<(), ApiError> {
        if self.session.is_none() || self.exited.is_some() {
            return Err(pane_not_running());
        }
        Ok(())
    }
}

fn pane_not_running() -> ApiError {
    ApiError::new("pane_exited", "the pane has no running terminal session")
}

/// Runtime 命名键 → GPUI 键名(分页键在 GPUI 是无下划线写法)。
fn gpui_key_name(key: RuntimeKey) -> &'static str {
    match key {
        RuntimeKey::PageUp => "pageup",
        RuntimeKey::PageDown => "pagedown",
        other => other.as_str(),
    }
}
