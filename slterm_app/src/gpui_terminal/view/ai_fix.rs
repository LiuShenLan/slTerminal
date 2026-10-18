//! AI 建议条呈现接线(08 篇 M9.5,M9.2 缝合点)。
//!
//! 触发链在每次命令失败上跑便宜门链(冷却 → 开关 → 规则表);全过才抓
//! 网格输出尾部、开后台请求。`Ctrl+.` 只贴入不执行;Esc/typing 关闭;
//! Pending 时误按 `Ctrl+.` 不丢请求。响应按 pane + seq 落位,过期即丢。

use std::time::Instant;

use gpui::{AppContext as _, Context, Keystroke};

use super::TerminalView;
use crate::ai_assistant::{self, AiFix, AiFixState, AssistantConfig, FixRequest, redact_secrets};
use crate::secrets::WindowsCredentialStore;
use slterm_terminal::grid::Dimensions as _;
use slterm_terminal::index::{Column, Line};

impl TerminalView {
    /// 失败命令之后的 AI 修复触发。任何一门不过都安静返回。
    pub(super) fn maybe_request_ai_fix(
        &mut self,
        exit_code: i32,
        program: Option<&str>,
        cx: &mut Context<'_, Self>,
    ) {
        let config = self.ai_config();
        let command = self.last_committed.clone();
        let Some(seq) = self.ai_fix_gate.evaluate(
            config.enabled,
            exit_code,
            &command,
            program,
            &config.ignored_exit_codes,
            Instant::now(),
        ) else {
            return;
        };
        let request = FixRequest {
            pane: self.pane_id,
            seq,
            command,
            exit_code,
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            output_tail: redact_secrets(&self.grid_output_tail(24, 2000)),
        };
        self.ai_fix = Some(AiFixState::Pending { seq });
        // 请求在后台执行器上跑;结果回前台时按 seq 比对,过期即丢。
        let calculation = cx.background_spawn(async move {
            ai_assistant::request_fix(&config, &request, &WindowsCredentialStore)
        });
        cx.spawn(async move |this, cx| {
            let result = calculation.await;
            let _ = this.update(cx, |view, cx| {
                view.apply_ai_fix_response(seq, result, cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// 响应落位:pane 归属由视图实体保证,seq 必须与当前态一致。
    /// `Err` = 配置/凭据错误(A6):显式上条,不再静默消失。
    pub(crate) fn apply_ai_fix_response(
        &mut self,
        seq: u64,
        result: Result<Option<AiFix>, String>,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ai_fix.as_ref().is_some_and(|state| state.seq() == seq) {
            self.ai_fix = match result {
                Ok(Some(fix)) => Some(AiFixState::Ready { seq, fix }),
                Ok(None) => None,
                Err(message) => Some(AiFixState::Failed { seq, message }),
            };
            cx.notify();
        }
    }

    /// 建议条键语义。返回 true = 终端已消费。
    pub(super) fn handle_ai_fix_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let mods = &keystroke.modifiers;
        // Ctrl+.:只贴入不执行(不带回车)。Pending 时消费按键但不丢请求。
        if mods.control && !mods.alt && !mods.shift && keystroke.key == "." {
            match self.ai_fix.take() {
                Some(AiFixState::Ready { fix, .. }) => {
                    if self.exited.is_none() {
                        self.write_input(fix.command.into_bytes(), cx);
                    }
                    cx.notify();
                    return true;
                }
                other => {
                    let pending = other.is_some();
                    self.ai_fix = other;
                    return pending;
                }
            }
        }
        if keystroke.key == "escape" && !mods.control && !mods.alt && self.ai_fix.is_some() {
            self.ai_fix = None;
            cx.notify();
            return true;
        }
        // 打字撤条:字符本身照常落进终端,不拦截。
        if !mods.control && !mods.alt && self.ai_fix.is_some() {
            let printable = keystroke
                .key_char
                .as_ref()
                .is_some_and(|text| !text.is_empty())
                || (keystroke.key.chars().count() == 1
                    && !keystroke.key.chars().any(char::is_control));
            if printable {
                self.ai_fix = None;
                cx.notify();
            }
        }
        false
    }

    /// 建议条展示内容;`None` = 无条。
    pub(crate) fn ai_fix_bar(&self) -> Option<AiFixBar> {
        match self.ai_fix.as_ref()? {
            AiFixState::Pending { .. } => Some(AiFixBar {
                command: String::new(),
                explain: String::new(),
                danger: false,
                pending: true,
            }),
            AiFixState::Ready { fix, .. } => Some(AiFixBar {
                command: fix.command.clone(),
                explain: fix.explain.clone(),
                danger: fix.danger,
                pending: false,
            }),
            // A6:错误以条面显示(message 走 explain 槽,command 为空)。
            AiFixState::Failed { message, .. } => Some(AiFixBar {
                command: String::new(),
                explain: message.clone(),
                danger: false,
                pending: false,
            }),
        }
    }

    /// 配置读取:生产每次失败读一次;测试用注入值隔离真实数据目录。
    fn ai_config(&self) -> AssistantConfig {
        #[cfg(all(test, feature = "gpui-test-support"))]
        if let Some(config) = &self.ai_config_override {
            return config.clone();
        }
        AssistantConfig::load()
    }

    /// 失败命令的屏幕输出尾部:光标上 24 行、尾部 2000 字符封顶、跳过
    /// `WIDE_CHAR_SPACER`(CJK 双胞空格污染)、去首尾空行。shell 侧集成看
    /// 不见渲染上下文,终端网格是权威源。
    pub(crate) fn grid_output_tail(&self, max_lines: usize, max_chars: usize) -> String {
        let Some(session) = &self.session else {
            return String::new();
        };
        let term = session.term.lock();
        let grid = term.grid();
        let cursor_line = grid.cursor.point.line.0;
        let columns = term.columns();
        let first = (cursor_line + 1 - max_lines as i32).max(0);
        let mut lines: Vec<String> = Vec::new();
        for line in first..=cursor_line {
            let row = &grid[Line(line)];
            let mut text = String::with_capacity(columns);
            for column in 0..columns {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .contains(slterm_terminal::term::cell::Flags::WIDE_CHAR_SPACER)
                {
                    continue;
                }
                text.push(cell.c);
            }
            lines.push(text.trim_end().to_owned());
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        while lines.first().is_some_and(String::is_empty) {
            lines.remove(0);
        }
        let text = lines.join("\n");
        let overflow = text.chars().count().saturating_sub(max_chars);
        if overflow > 0 {
            text.chars().skip(overflow).collect()
        } else {
            text
        }
    }
}

/// 建议条渲染态的纯数据投影。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiFixBar {
    pub command: String,
    pub explain: String,
    pub danger: bool,
    pub pending: bool,
}
