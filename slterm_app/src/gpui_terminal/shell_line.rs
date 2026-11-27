//! 本地 Shell 输入镜像：提示符回显、可信编辑与执行确认。

use slterm_terminal::tty::{ShellKind, shell_kind_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mirror {
    Unestablished,
    AwaitingEcho,
    Trusted,
    Invalid,
}

struct PendingExecution {
    line: String,
    started: bool,
    cancelled: bool,
}

/// 一个逻辑终端独占的本地行镜像；只有真实回显能恢复可信状态。
pub(crate) struct ShellLineTracker {
    supported: bool,
    epoch: Option<u64>,
    line: String,
    revision: u64,
    mirror: Mirror,
    pending: Option<PendingExecution>,
}

impl ShellLineTracker {
    pub(crate) fn new(shell_program: Option<&str>) -> Self {
        let supported = shell_program.is_some_and(|program| {
            matches!(
                shell_kind_of(program),
                ShellKind::Pwsh | ShellKind::PowerShell | ShellKind::Cmd
            )
        });
        Self {
            supported,
            epoch: None,
            line: String::new(),
            revision: 0,
            mirror: Mirror::Unestablished,
            pending: None,
        }
    }

    fn advance_revision(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Shell 输入代次不得回绕");
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// 当前已观察到的提示符代次；补全请求用它与回包时的镜像代次对齐。
    pub(crate) fn prompt_epoch(&self) -> Option<u64> {
        self.epoch
    }

    pub(crate) fn observe_prompt(&mut self, epoch: u64, echo: &str) {
        if !self.supported {
            return;
        }
        if self.epoch != Some(epoch) {
            // 新完整边界结束一切未闭合执行；缺 Done 的旧行不得入库。
            self.epoch = Some(epoch);
            self.pending = None;
            self.line = echo.to_owned();
            self.advance_revision();
            self.mirror = Mirror::Trusted;
            return;
        }
        if self.pending.is_some() || self.mirror == Mirror::Invalid {
            return;
        }
        self.mirror = if self.line == echo {
            Mirror::Trusted
        } else {
            Mirror::AwaitingEcho
        };
    }

    pub(crate) fn note_text(&mut self, text: &str) -> bool {
        if text.is_empty() || !self.can_edit() {
            return false;
        }
        self.line.push_str(text);
        self.advance_revision();
        self.mirror = Mirror::AwaitingEcho;
        true
    }

    fn can_edit(&self) -> bool {
        self.supported
            && self.epoch.is_some()
            && self.pending.is_none()
            && self.mirror != Mirror::Invalid
    }

    pub(crate) fn backspace(&mut self) -> bool {
        if self.line.is_empty() || !self.can_edit() {
            return false;
        }
        self.line.pop();
        self.advance_revision();
        self.mirror = Mirror::AwaitingEcho;
        true
    }

    pub(crate) fn replace_trusted_line(&mut self, line: String) -> bool {
        if self.completion_line().is_none() {
            return false;
        }
        self.line = line;
        self.advance_revision();
        self.mirror = Mirror::AwaitingEcho;
        true
    }

    /// Enter 只产生待决执行；记录由 CommandDone 返回，而不是在这里发生。
    pub(crate) fn submit(&mut self) -> bool {
        if !self.supported || self.pending.is_some() {
            return false;
        }
        let line = self.line.trim().to_owned();
        let accepted = self.mirror == Mirror::Trusted
            && !line.is_empty()
            && !line.chars().any(char::is_control);
        self.line.clear();
        self.advance_revision();
        self.mirror = Mirror::Invalid;
        if accepted {
            self.pending = Some(PendingExecution {
                line,
                started: false,
                cancelled: false,
            });
        }
        accepted
    }

    pub(crate) fn command_start(&mut self) -> Option<String> {
        let pending = self.pending.as_mut()?;
        if pending.started {
            return None;
        }
        pending.started = true;
        Some(pending.line.clone())
    }

    pub(crate) fn command_done(&mut self) -> Option<String> {
        let pending = self.pending.take()?;
        (pending.started && !pending.cancelled).then_some(pending.line)
    }

    pub(crate) fn interrupt(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.cancelled = true;
        }
        self.invalidate();
    }

    /// Readline、历史召回、resize/reset、备用屏或前台接管均走同一失效口。
    pub(crate) fn invalidate(&mut self) {
        self.line.clear();
        self.advance_revision();
        self.mirror = Mirror::Invalid;
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.started)
        {
            self.pending = None;
        }
    }

    pub(crate) fn completion_line(&self) -> Option<&str> {
        (self.supported && self.pending.is_none() && self.mirror == Mirror::Trusted)
            .then_some(self.line.as_str())
    }

    pub(crate) fn line(&self) -> &str {
        &self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> ShellLineTracker {
        ShellLineTracker::new(Some("cmd.exe"))
    }

    #[test]
    fn echo_mismatch_blocks_completion_until_match() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        tracker.note_text("git");
        assert_eq!(tracker.completion_line(), None);
        tracker.observe_prompt(1, "gi");
        assert_eq!(tracker.completion_line(), None);
        tracker.observe_prompt(1, "git");
        assert_eq!(tracker.completion_line(), Some("git"));
    }

    #[test]
    fn execution_is_confirmed_only_by_start_then_done() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        tracker.note_text("echo ok");
        tracker.observe_prompt(1, "echo ok");
        assert!(tracker.submit());
        assert_eq!(tracker.command_done(), None, "没有开始不得入库");

        tracker.observe_prompt(2, "");
        tracker.note_text("echo ok");
        tracker.observe_prompt(2, "echo ok");
        assert!(tracker.submit());
        assert_eq!(tracker.command_start().as_deref(), Some("echo ok"));
        assert_eq!(tracker.command_done().as_deref(), Some("echo ok"));
        assert_eq!(tracker.command_done(), None, "同一次执行不得重复入库");
    }

    #[test]
    fn readline_and_resize_invalidate_until_a_new_prompt() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        tracker.note_text("git");
        tracker.observe_prompt(1, "git");
        tracker.invalidate();
        assert_eq!(tracker.completion_line(), None);
        tracker.observe_prompt(1, "");
        assert_eq!(tracker.completion_line(), None, "旧边界不得复活镜像");
        tracker.observe_prompt(2, "");
        tracker.note_text("cd");
        tracker.observe_prompt(2, "cd");
        assert_eq!(tracker.completion_line(), Some("cd"));
        tracker.invalidate();
        assert!(!tracker.submit(), "失效行不得进入执行确认");
    }

    #[test]
    fn cancelled_or_unclosed_execution_is_never_collected() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        tracker.note_text("echo no");
        tracker.observe_prompt(1, "echo no");
        tracker.submit();
        tracker.command_start();
        tracker.interrupt();
        assert_eq!(tracker.command_done(), None);

        tracker.observe_prompt(2, "");
        tracker.note_text("echo open");
        tracker.observe_prompt(2, "echo open");
        tracker.submit();
        tracker.command_start();
        tracker.observe_prompt(3, "");
        assert_eq!(tracker.command_done(), None, "缺失完成边界不得入库");
    }

    #[test]
    fn backspace_waits_for_the_corrected_echo() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        tracker.note_text("git");
        tracker.observe_prompt(1, "git");
        assert!(tracker.backspace());
        assert_eq!(tracker.completion_line(), None);
        tracker.observe_prompt(1, "gi");
        assert_eq!(tracker.completion_line(), Some("gi"));
    }

    #[test]
    fn input_revision_tracks_local_edits_but_not_echo() {
        let mut tracker = tracker();
        tracker.observe_prompt(1, "");
        let initial = tracker.revision();
        tracker.observe_prompt(1, "");
        assert_eq!(tracker.revision(), initial);
        tracker.note_text("g");
        let typed = tracker.revision();
        assert!(typed > initial);
        tracker.observe_prompt(1, "g");
        assert_eq!(tracker.revision(), typed, "PTY 回显不得推进输入代次");
        assert!(tracker.replace_trusted_line("git".into()));
        assert!(tracker.revision() > typed);
    }

    #[test]
    fn unsupported_shell_never_opens_the_mirror() {
        let mut tracker = ShellLineTracker::new(Some("bash.exe"));
        tracker.observe_prompt(1, "");
        assert!(!tracker.note_text("echo no"));
        assert_eq!(tracker.completion_line(), None);
        assert!(!tracker.submit());
    }
}
