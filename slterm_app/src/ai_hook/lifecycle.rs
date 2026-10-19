//! 一个 pane 的 agent 生命周期,两种 UI 壳共用。
//!
//! hook 事件是事实;屏幕匹配是有限权威的观察。命令所有权止于 shell 边界,
//! 绝不因为输出安静下来而结束。
//!
//! 上游的 remote_process 归属维随 SSH 语义整支裁剪(默认裁定)。

use crate::ai_agents::{AgentStatus, AgentStatusSource, Detection};

use super::{AiHookEvent, AiHookKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HookCoverage {
    #[default]
    None,
    Completion,
    Turns,
    Lifecycle,
}

/// 嵌套子代理区分:source/session/pid/bridge_instance。
#[derive(Debug, Clone)]
struct Owner {
    source: String,
    session: Option<String>,
    pid: Option<u32>,
    bridge_instance: Option<String>,
}

/// 持有状态与仲裁。适配器可以提交事实,但不能直接写字段。
#[derive(Debug, Clone)]
pub(crate) struct AgentActivity {
    status: AgentStatus,
    source: AgentStatusSource,
    rule: Option<String>,
    coverage: HookCoverage,
    owner: Option<Owner>,
    idle_samples: u8,
    turn_observed: bool,
    pending_submit: bool,
    /// completion-only 的 hook 把结果锁存,直到一次空闲屏幕或一次显式提交
    /// 把旧帧与新回合分开。
    screen_armed: bool,
    /// 显式 shell 边界;同时拒绝旧 CLI 的迟到事件。
    command_ended: bool,
    native_codex: bool,
    turn_id: Option<String>,
}

impl Default for AgentActivity {
    fn default() -> Self {
        Self {
            status: AgentStatus::Unknown,
            source: AgentStatusSource::Unknown,
            rule: None,
            coverage: HookCoverage::None,
            owner: None,
            idle_samples: 0,
            turn_observed: false,
            pending_submit: false,
            screen_armed: true,
            command_ended: false,
            native_codex: false,
            turn_id: None,
        }
    }
}

impl AgentActivity {
    pub fn status(&self) -> AgentStatus {
        self.status
    }
    pub fn source(&self) -> AgentStatusSource {
        self.source
    }
    pub fn rule(&self) -> Option<&str> {
        self.rule.as_deref()
    }
    pub fn hook_seen(&self) -> bool {
        self.coverage != HookCoverage::None
    }
    pub fn primary_pid(&self) -> Option<u32> {
        self.owner.as_ref().and_then(|owner| owner.pid)
    }
    pub fn allows_screen(&self) -> bool {
        !self.command_ended && self.coverage != HookCoverage::Lifecycle
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 显式 shell 边界同时拒绝旧 CLI 的迟到事件。
    pub fn command_finished(&mut self) {
        self.reset();
        self.command_ended = true;
    }

    /// 启动一个 agent 不是它已经回答了 prompt 的证据。
    pub fn begin_command(&mut self, agent: bool) {
        self.reset();
        if agent {
            self.status = AgentStatus::Working;
            self.source = AgentStatusSource::Process;
        }
    }

    pub fn identify(&mut self, source: AgentStatusSource) {
        if !self.hook_seen() && self.status == AgentStatus::Unknown {
            self.source = source;
        }
    }

    /// 一次具体的运行时提交或被接受的确认启动工作。上一帧输入绝不能在工作
    /// 开始之前造出一次完成。
    pub fn submitted(&mut self) {
        self.status = AgentStatus::Working;
        self.source = AgentStatusSource::Process;
        self.rule = None;
        self.turn_observed = true;
        self.pending_submit = true;
        self.input_sent();
    }

    pub fn submitted_text(&mut self, program: &str, text: &str) {
        // codex /plan 只切换交互模式,不触发任务 hook;若先制造 Running,
        // 对话输入会一直等一个不存在的完成事件。普通任务仍保留提交屏障。
        if program == "codex" && text.trim() == "/plan" {
            self.input_sent();
        } else {
            self.submitted();
        }
    }

    /// 仅按下物理 Enter 也可以消掉一个菜单。它只准许观察一个新回合;本身
    /// 不是一次成功提交,也不是一次完成。
    pub fn input_sent(&mut self) {
        self.screen_armed = true;
        self.idle_samples = 0;
    }

    /// 在排序门记录这条事件之前检查所有权。一条被忽略的 legacy 通知绝不能
    /// 重新打开门里已经完成的流。
    pub fn accepts_hook(&self, event: &AiHookEvent) -> bool {
        if self.command_ended && event.kind != AiHookKind::SessionStart {
            return false;
        }
        if event.legacy_attention
            && !matches!(self.status, AgentStatus::Working | AgentStatus::Blocked)
        {
            return false;
        }
        // 两座桥都为旧客户端与待定 hook 信任保持安装。一旦本会话证明它的
        // native 桥活着,notify 就不能降级它的覆盖面,也不能再发第二次完成
        // 通知。
        if self.native_codex && event.source == "codex" && event.codex_hooks.is_none() {
            return false;
        }
        if event.codex_hooks.is_some()
            && self.turn_id.is_some()
            && event.turn_id.is_some()
            && self.turn_id != event.turn_id
            && !matches!(
                event.kind,
                AiHookKind::SessionStart | AiHookKind::PromptSubmit
            )
        {
            return false;
        }
        if event.kind == AiHookKind::TurnDone
            && self.status == AgentStatus::Done
            && event.turn_id.is_some()
            && event.turn_id == self.turn_id
        {
            return false;
        }
        if let Some(owner) = &self.owner {
            if owner.source != event.source {
                return false;
            }
            let same_process = owner.pid.is_some() && owner.pid == event.agent_pid
                || matches!(event.source.as_str(), "pi" | "omp")
                    && owner.bridge_instance.is_some()
                    && owner.bridge_instance == event.bridge_instance;
            match (owner.pid, event.agent_pid) {
                (Some(primary), Some(incoming)) if primary != incoming => return false,
                _ if owner.session.is_some()
                    && event.session_id.is_some()
                    && owner.session != event.session_id
                    && !(same_process && event.kind == AiHookKind::SessionStart) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    fn record_owner(&mut self, event: &AiHookEvent) {
        if let Some(owner) = &mut self.owner {
            owner.pid = owner.pid.or(event.agent_pid);
            if owner.bridge_instance.is_none() {
                owner.bridge_instance = event.bridge_instance.clone();
            }
            if event.session_id.is_some() {
                owner.session = event.session_id.clone();
            }
        } else {
            self.owner = Some(Owner {
                source: event.source.clone(),
                session: event.session_id.clone(),
                pid: event.agent_pid,
                bridge_instance: event.bridge_instance.clone(),
            });
        }
    }

    /// 应用一条已排序的事件。嵌套/外来会话对主 pane 没有任何权威,包括它的
    /// 身份、attention 与通知。
    pub fn apply_hook(&mut self, event: &AiHookEvent) -> bool {
        if !self.accepts_hook(event) {
            return false;
        }
        self.record_owner(event);
        self.native_codex |= event.codex_hooks.is_some();
        if event.turn_id.is_some()
            || (event.kind == AiHookKind::SessionStart && !event.session_compacted)
        {
            self.turn_id = event.turn_id.clone();
        }
        self.command_ended = false;
        if event.kind == AiHookKind::SessionEnd {
            self.command_finished();
            return true;
        }
        let capabilities = event.capabilities();
        let coverage = if capabilities.lifecycle {
            if capabilities.attention_events {
                HookCoverage::Lifecycle
            } else {
                HookCoverage::Turns
            }
        } else {
            HookCoverage::Completion
        };
        self.coverage = coverage;
        self.source = AgentStatusSource::Hook;
        self.rule = None;
        self.idle_samples = 0;
        if event.kind != AiHookKind::SessionStart {
            self.pending_submit = false;
        }
        self.status = match event.kind {
            AiHookKind::SessionStart if event.session_compacted => self.status,
            AiHookKind::SessionStart => {
                self.turn_observed = false;
                AgentStatus::Idle
            }
            AiHookKind::PromptSubmit | AiHookKind::ToolComplete => {
                self.turn_observed = true;
                AgentStatus::Working
            }
            AiHookKind::TurnDone if event.active_background_tasks() > 0 => {
                self.turn_observed = true;
                self.rule = Some(format!(
                    "hook.background_tasks.active={}",
                    event.active_background_tasks()
                ));
                AgentStatus::Working
            }
            AiHookKind::TurnDone => {
                self.screen_armed = false;
                if matches!(
                    event.turn_outcome,
                    super::AiTurnOutcome::Failed
                        | super::AiTurnOutcome::Incomplete
                        | super::AiTurnOutcome::Cancelled
                ) || (matches!(event.source.as_str(), "pi" | "omp" | "cursor" | "copilot")
                    && event.turn_outcome == super::AiTurnOutcome::Unknown)
                {
                    AgentStatus::Idle
                } else {
                    AgentStatus::Done
                }
            }
            AiHookKind::NeedsAttention => AgentStatus::Blocked,
            AiHookKind::SessionEnd => unreachable!("handled above"),
        };
        true
    }

    /// 不匹配会重置连续空闲证据。完整生命周期的 hook 永不参考屏幕规则,
    /// 即便当前帧里有一段真实的旧表单。
    pub fn observe_screen(&mut self, detection: Option<Detection>) -> bool {
        if !self.allows_screen() {
            return false;
        }
        let Some(detection) = detection else {
            self.idle_samples = 0;
            return false;
        };
        let next = match detection.status {
            AgentStatus::Idle => {
                if self.pending_submit {
                    self.idle_samples = 0;
                    return false;
                }
                self.idle_samples = self.idle_samples.saturating_add(1);
                if self.idle_samples < 2 {
                    return false;
                }
                self.screen_armed = true;
                // completion hook 拥有完成判定的所有权,即便在旧客户端上
                // 开始/权限证据不得不来自屏幕。
                if matches!(
                    self.coverage,
                    HookCoverage::Completion | HookCoverage::Turns
                ) {
                    return false;
                }
                if self.turn_observed {
                    AgentStatus::Done
                } else {
                    AgentStatus::Idle
                }
            }
            status @ (AgentStatus::Working | AgentStatus::Blocked) => {
                self.idle_samples = 0;
                // Pi 报开始与完成,但没有 attention hook。屏幕证据只在 hook
                // 拥有的回合进行中才能补这个缺;它不能开启一个回合,也不能
                // 推翻 hook 的结果。
                if self.coverage == HookCoverage::Turns
                    && (status != AgentStatus::Blocked
                        || !matches!(self.status, AgentStatus::Working | AgentStatus::Blocked))
                {
                    return false;
                }
                if !self.screen_armed {
                    return false;
                }
                self.pending_submit = false;
                self.turn_observed |= status == AgentStatus::Working;
                status
            }
            AgentStatus::Unknown | AgentStatus::Done => {
                self.idle_samples = 0;
                return false;
            }
        };
        if next == self.status {
            return false;
        }
        self.status = next;
        self.source = AgentStatusSource::Screen;
        self.rule = Some(detection.rule_id);
        true
    }
}

/// 通知策略是一条边,不是轮询采样的次数。
pub(crate) fn screen_notification(
    previous: AgentStatus,
    next: AgentStatus,
    hooks: bool,
) -> Option<bool> {
    match next {
        AgentStatus::Blocked if previous != AgentStatus::Blocked => Some(true),
        AgentStatus::Done if matches!(previous, AgentStatus::Working | AgentStatus::Blocked) => {
            (!hooks).then_some(false)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
