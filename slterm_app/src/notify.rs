//! 通知中心单一漏斗。
//!
//! 一切值得用户注意的信号(BEL / OSC 9 / OSC 133;D / hook 生命周期)归一到
//! [`Notification`] 六变体;新增来源 = 新变体,新增输出 = [`Funnel::plan`] 里
//! 新一行。投递纪律(边界 4):
//!
//! - **失焦门控先行**:窗口聚焦中全部丢弃——用户已看见终端,视觉 bell 覆盖
//!   该场景;门控收在漏斗入口,调用点不需要重复判断;
//! - **闪烁恒发先行**:任务栏闪烁幂等、静默、shell 自动合并;节流只挡系统
//!   toast,不挡闪烁(先闪后判,与上游因果注释一致);
//! - **双层节流**:全局 3s toast 间隔 + pane 级 3s 同键窗口;失败另走
//!   pane episode 30s 冷却(恢复事件结束旧 episode、同错冷却、新错独立);
//! - **正文 160 字符幂等截断**:通知是瞥读层,长回答撑破卡片会遮关闭钮;
//! - 任何投递失败只落日志:通知是 best-effort,永不影响终端。
//!
//! 系统 toast 走 `platform::notifications` 的常驻 MTA worker(跨进程 RPC
//! 不上事件循环)。真实投递面按「真实面登记豁免」由 `notify-test`
//! 诊断命令与新机人工 smoke 兜底。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use gpui::App;
use slterm_split::PaneId;

use crate::ai_hook::{AiHookEvent, AiHookKind, AiTurnOutcome};
use crate::app_shell::SltermShellEvent;
use crate::i18n::{Message, UiLanguage};
use crate::platform::notifications::{ToastAction, ToastActivation};

/// 一条值得注意的事件。漏斗不认事件文本里的关键字:来源标注一律问
/// `AgentKind::parse`(CLI profile 单点),避免按散文猜来源。
#[derive(Debug, Clone)]
pub(crate) enum Notification {
    /// BEL:AI CLI 回合完成的主信号(OSC/BEL 上抛接线后消费)。
    #[allow(dead_code)] // 消费点 = BEL 上抛接线;当前生产源 = hook/恢复。
    Bell { program: Option<String> },
    /// OSC 133;C/D 跟踪的命令完成(被 hook 覆盖时不再通知)。
    CommandDone {
        duration: Duration,
        program: Option<String>,
    },
    /// shell 报告的非零退出,短失败命令同样通知。
    CommandFailed {
        duration: Duration,
        program: Option<String>,
        exit_code: i32,
    },
    /// OSC 9 自由文本(Claude Code 等 iTerm2 通知通道)。
    #[allow(dead_code)] // 消费点 = OSC 9 上抛接线;当前生产源 = hook/恢复。
    Text {
        body: String,
        program: Option<String>,
    },
    /// TurnDone/NeedsAttention;`attention` = 等用户确认/输入。
    AiTurn {
        program: String,
        message: Option<String>,
        attention: bool,
    },
    /// provider 停止但回答未确认成功。
    AiTurnIssue {
        program: String,
        message: Option<String>,
        outcome: AiTurnOutcome,
    },
    /// 启动期一次性汇总(恢复结果等,/):走漏斗但豁免前台门控。
    RecoverySummary { text: String },
    /// 进程级提示:无 pane 归属、无动作按钮;走启动汇总通道豁免
    /// 前台门控。文本必须由领域类别生成,不得携带技术化合成命令。
    ProcessAlert { text: String },
}

impl Notification {
    /// 命令收尾归一:非零退出无条件成失败(含短命令);被 hook 确认过的
    /// 短成功命令不再通知(hook 覆盖是权威)。
    #[allow(dead_code)] // 消费点 = OSC 133;D 完成边接线；先落纯函数。
    pub(crate) fn command_finished(
        program: Option<String>,
        duration: Duration,
        exit_code: Option<i32>,
        hook_seen: bool,
    ) -> Option<Self> {
        if let Some(exit_code) = exit_code.filter(|code| *code != 0) {
            return Some(Self::CommandFailed {
                program,
                duration,
                exit_code,
            });
        }
        (!hook_seen && duration >= COMMAND_NOTIFY_MIN)
            .then_some(Self::CommandDone { program, duration })
    }

    /// hook 事件 → 通知。在飞后台任务未清时回合结束不提示;取消
    /// 静默;provider 报错/截断落 `AiTurnIssue`;pi/omp 的旧桥缺结果元数据
    /// 时按「未确认成功」处理。
    pub(crate) fn from_ai_hook(
        event: &AiHookEvent,
        message: Option<String>,
        attention: bool,
    ) -> Option<Self> {
        if event.kind == AiHookKind::TurnDone {
            if event.active_background_tasks() > 0 {
                return None;
            }
            match event.turn_outcome {
                AiTurnOutcome::Cancelled => return None,
                AiTurnOutcome::Failed | AiTurnOutcome::Incomplete => {
                    return Some(Self::AiTurnIssue {
                        program: event.source.clone(),
                        message,
                        outcome: event.turn_outcome,
                    });
                }
                AiTurnOutcome::Unknown | AiTurnOutcome::Unspecified
                    if matches!(event.source.as_str(), "pi" | "omp") =>
                {
                    return Some(Self::AiTurnIssue {
                        program: event.source.clone(),
                        message: None,
                        outcome: AiTurnOutcome::Unknown,
                    });
                }
                _ => {}
            }
        }
        Some(Self::AiTurn {
            program: event.source.clone(),
            message,
            attention,
        })
    }

    /// 权限确认/提问是「等用户」,强于「回合完成」。
    pub(crate) fn is_attention(&self) -> bool {
        matches!(
            self,
            Self::AiTurn {
                attention: true,
                ..
            }
        )
    }

    /// 失败 episode 的构成:命令非零退出,或 provider 明确报错/截断。
    pub(crate) fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::CommandFailed { .. }
                | Self::AiTurnIssue {
                    outcome: AiTurnOutcome::Failed | AiTurnOutcome::Incomplete,
                    ..
                }
        )
    }

    /// 仅做来源分类:应用内偏好不得静默系统通知,也不得按正文猜 AI。
    #[allow(dead_code)] // 消费点 = 壳内通知面(本片只交付系统通道)。
    pub(crate) fn is_ai(&self) -> bool {
        let program = match self {
            Self::AiTurn { .. } | Self::AiTurnIssue { .. } => return true,
            Self::RecoverySummary { .. } | Self::ProcessAlert { .. } => return false,
            Self::Bell { program }
            | Self::CommandDone { program, .. }
            | Self::CommandFailed { program, .. }
            | Self::Text { program, .. } => program.as_deref(),
        };
        program.is_some_and(|program| crate::ai_agents::AgentKind::parse(program).is_some())
    }

    /// toast 标题 + 正文:标题标来源(slTerminal 或程序名),正文是瞥读
    /// 预览;原文另走 [`Self::raw_toast_text`](日志/壳内横幅)。
    pub(crate) fn toast_text(&self) -> (String, String) {
        let (title, body) = self.raw_toast_text();
        (title, clamp_toast_body(&body))
    }

    pub(crate) fn raw_toast_text(&self) -> (String, String) {
        let language = notification_language();
        match self {
            Self::Bell { program } => match program {
                Some(program) => (
                    program.clone(),
                    language.text(Message::NotifyBell).to_owned(),
                ),
                None => (
                    crate::brand::NAME.to_owned(),
                    language.text(Message::NotifyBell).to_owned(),
                ),
            },
            Self::CommandDone { duration, program } => (
                program
                    .clone()
                    .unwrap_or_else(|| crate::brand::NAME.to_owned()),
                language.format(
                    Message::NotifyCommandFinished,
                    &[("seconds", &duration.as_secs().to_string())],
                ),
            ),
            Self::CommandFailed {
                duration,
                program,
                exit_code,
            } => (
                program
                    .clone()
                    .unwrap_or_else(|| crate::brand::NAME.to_owned()),
                language.format(
                    Message::NotifyCommandFailed,
                    &[
                        ("seconds", &duration.as_secs().to_string()),
                        ("code", &exit_code.to_string()),
                    ],
                ),
            ),
            Self::Text { body, program } => match program {
                Some(program) => (program.clone(), body.clone()),
                None => (crate::brand::NAME.to_owned(), body.clone()),
            },
            Self::RecoverySummary { text } | Self::ProcessAlert { text } => {
                (crate::brand::NAME.to_owned(), text.clone())
            }
            Self::AiTurn {
                program,
                message,
                attention,
            } => {
                let body = message.clone().unwrap_or_else(|| {
                    let message = if *attention {
                        Message::NotifyTurnAttention
                    } else {
                        Message::NotifyTurnDone
                    };
                    language.text(message).to_owned()
                });
                (program.clone(), body)
            }
            Self::AiTurnIssue {
                program,
                message,
                outcome,
            } => (
                program.clone(),
                turn_issue_text(*outcome, message.as_deref(), language),
            ),
        }
    }
}

fn notification_language() -> UiLanguage {
    crate::i18n::LanguagePreference::from(slterm_settings::RuntimeSettings::load().language)
        .resolved()
}

fn turn_issue_text(outcome: AiTurnOutcome, message: Option<&str>, language: UiLanguage) -> String {
    match outcome {
        AiTurnOutcome::Failed => match message.filter(|message| !message.trim().is_empty()) {
            Some(error) => language.format(Message::NotifyTurnFailedReason, &[("error", error)]),
            None => language.text(Message::NotifyTurnFailed).to_owned(),
        },
        AiTurnOutcome::Incomplete => language.text(Message::NotifyTurnIncomplete).to_owned(),
        _ => language.text(Message::NotifyTurnEnded).to_owned(),
    }
}

/// hook 未确认的命令最短通知时长:快的 `ls` 类命令不该整天闪任务栏。
pub(crate) const COMMAND_NOTIFY_MIN: Duration = Duration::from_secs(10);

/// 瞥读层正文上限。
pub(crate) const TOAST_BODY_MAX_CHARS: usize = 160;

/// 单行预览:空白折叠成空格、超限幂等截断并留一个 `…`。只多读一个字符
/// 判断是否省略;原文另存。
pub(crate) fn clamp_toast_body(body: &str) -> String {
    let mut chars = body
        .chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch });
    let mut preview = String::with_capacity(body.len().min(TOAST_BODY_MAX_CHARS * 4));
    preview.extend(chars.by_ref().take(TOAST_BODY_MAX_CHARS));
    if chars.next().is_some() {
        preview.pop();
        preview.push('…');
    }
    preview
}

/// 全局 toast 间隔(pane 间也生效,防后台脚本刷屏)。
const TOAST_THROTTLE: Duration = Duration::from_secs(3);

/// 同 pane + 同程序 + 同结局的失败 episode 冷却;恢复事件结 episode。
const FAILURE_COOLDOWN: Duration = Duration::from_secs(30);

/// pane 级失败冷却:自动重试只需一次提醒,不同错误/不同 pane 独立。
#[derive(Default)]
pub(crate) struct PaneFailureThrottle {
    recent: Vec<RecentFailure>,
}

struct RecentFailure {
    pane: PaneId,
    program: String,
    message: Option<String>,
    outcome: AiTurnOutcome,
    shown_at: Instant,
}

impl PaneFailureThrottle {
    pub(crate) const fn new() -> Self {
        Self { recent: Vec::new() }
    }

    /// `delivering` = 本轮确实有 toast 通道;false 时只做 episode 维护
    /// (恢复事件依旧结束旧 episode),不记账。
    pub(crate) fn accepts(
        &mut self,
        pane: PaneId,
        notification: &Notification,
        delivering: bool,
        now: Instant,
    ) -> bool {
        self.recent
            .retain(|last| now.saturating_duration_since(last.shown_at) < FAILURE_COOLDOWN);
        // 回合完成/attention 到达 = 进程恢复:结束该 pane 的旧失败 episode,
        // 之后同错重新可弹(episode 以恢复为终点,不是固定窗口)。
        if matches!(
            notification,
            Notification::AiTurn {
                attention: false,
                ..
            }
        ) {
            self.recent.retain(|last| last.pane != pane);
        }
        if !delivering {
            return false;
        }
        if !notification.is_failure() {
            return true;
        }
        let Notification::AiTurnIssue {
            program,
            message,
            outcome,
        } = notification
        else {
            return true;
        };
        if self.recent.iter().any(|last| {
            last.pane == pane
                && last.program == *program
                && last.message == *message
                && last.outcome == *outcome
        }) {
            return false;
        }
        self.recent.push(RecentFailure {
            pane,
            program: program.clone(),
            message: message.clone(),
            outcome: *outcome,
            shown_at: now,
        });
        true
    }
}

/// pane 级同键窗口:(pane, attention, confirmation request) 在 3s 内只放
/// 一条;连续提问的不同 request id 身份独立。
#[derive(Default)]
pub(crate) struct PaneNotificationThrottle {
    recent: HashMap<(PaneId, bool, Option<u64>), Instant>,
}

impl PaneNotificationThrottle {
    fn accepts_request(
        &mut self,
        pane_id: PaneId,
        attention: bool,
        request: Option<u64>,
        now: Instant,
    ) -> bool {
        self.recent
            .retain(|_, last| now.saturating_duration_since(*last) < TOAST_THROTTLE);
        if self.recent.contains_key(&(pane_id, attention, request)) {
            return false;
        }
        self.recent.insert((pane_id, attention, request), now);
        true
    }
}

/// 一次投递计划:闪烁恒为「未聚焦时 true」;toast 被任一层节流拦下即为
/// None(调用方据此只闪不弹)。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DeliveryPlan {
    pub(crate) flash: bool,
    pub(crate) toast: Option<(String, String)>,
}

/// 漏斗状态:节流窗口 + 失败 episode。时钟注入使全部策略可在单元层断言。
#[derive(Default)]
pub(crate) struct Funnel {
    last_toast: Option<Instant>,
    panes: PaneNotificationThrottle,
    failures: PaneFailureThrottle,
}

impl Funnel {
    pub(crate) fn new() -> Self {
        Self {
            last_toast: None,
            panes: PaneNotificationThrottle::default(),
            failures: PaneFailureThrottle::new(),
        }
    }

    /// 顺序即设计数据流:① 前台门控 → ② 闪烁先行 → ③ 全局间隔 →
    /// ④ pane 同键窗口 → ⑤ 失败 episode → 正文预览。
    pub(crate) fn plan(
        &mut self,
        notification: &Notification,
        pane_id: PaneId,
        focused: bool,
        request: Option<u64>,
        now: Instant,
    ) -> DeliveryPlan {
        self.plan_with(notification, pane_id, focused, request, false, now)
    }

    /// 启动期一次性汇总豁免①前台门控(其余节流层不变)。
    pub(crate) fn plan_startup(
        &mut self,
        notification: &Notification,
        pane_id: PaneId,
        focused: bool,
        request: Option<u64>,
        now: Instant,
    ) -> DeliveryPlan {
        self.plan_with(notification, pane_id, focused, request, true, now)
    }

    fn plan_with(
        &mut self,
        notification: &Notification,
        pane_id: PaneId,
        focused: bool,
        request: Option<u64>,
        startup_summary: bool,
        now: Instant,
    ) -> DeliveryPlan {
        // ① 聚焦中不打扰:全部变体零投递,连闪烁也不发。启动汇总例外。
        if focused && !startup_summary {
            return DeliveryPlan::default();
        }
        // ② 闪烁是「恢复中」信号,先于一切节流;被节流时仍恰好发生一次。
        // 启动汇总不闪(窗口已在前台,闪烁无意义)。
        let mut plan = DeliveryPlan {
            flash: !startup_summary,
            toast: None,
        };
        // ③ 全局间隔:窗口内任何 pane 的第二条 toast 被吞。
        if self
            .last_toast
            .is_some_and(|at| now.saturating_duration_since(at) < TOAST_THROTTLE)
        {
            self.failures.accepts(pane_id, notification, false, now);
            return plan;
        }
        // ④ pane 级同键窗口(注意力/请求 id 参与键)。
        if !self
            .panes
            .accepts_request(pane_id, notification.is_attention(), request, now)
        {
            self.failures.accepts(pane_id, notification, false, now);
            return plan;
        }
        // ⑤ 失败 episode 冷却。
        if !self.failures.accepts(pane_id, notification, true, now) {
            return plan;
        }
        self.last_toast = Some(now);
        plan.toast = Some(notification.toast_text());
        plan
    }
}

// ─── 前台状态(单窗单实例的进程级收口) ──────────────────────────────────

static FOREGROUND: AtomicBool = AtomicBool::new(false);

/// 壳窗口激活态变化唯一写入口(workspace 的 activation observer 调用)。
pub(crate) fn set_foreground(focused: bool) {
    FOREGROUND.store(focused, Ordering::Relaxed);
}

fn foreground() -> bool {
    FOREGROUND.load(Ordering::Relaxed)
}

// ─── toast 激活回窗 ──────────────────────────────────────────────────────

static GPUI_ACTIVATION: OnceLock<mpsc::Sender<SltermShellEvent>> = OnceLock::new();

/// 壳启动早期一次:toast 点击/动作按钮经此回投进程内事件队列。
pub(crate) fn init_gpui_activation(sender: mpsc::Sender<SltermShellEvent>) {
    let _ = GPUI_ACTIVATION.set(sender);
}

fn application_activation(pane_id: Option<PaneId>) -> Option<ToastActivation> {
    GPUI_ACTIVATION.get().map(|sender| {
        let sender = sender.clone();
        Arc::new(move || {
            let _ = sender.send(SltermShellEvent::NotificationFocus(pane_id));
        }) as ToastActivation
    })
}

// ─── 缝合:已接纳的 hook 事件 → 通知 ───────────────────────────────

/// 一条已通过 pane 路由/所有权门的 hook 事件 → (通知, 目标 pane)。
/// TurnDone/NeedsAttention 之外的边不产生通知。信封缺 pane 时用调用方
/// 解析出的焦点兜底(与 `AgentHub::dispatch_lifecycle` 同一路由语义)。
pub(crate) fn from_accepted_hook(
    event: &AiHookEvent,
    fallback: Option<PaneId>,
) -> Option<(Notification, PaneId)> {
    if !matches!(
        event.kind,
        AiHookKind::TurnDone | AiHookKind::NeedsAttention
    ) {
        return None;
    }
    let pane = event.pane.map(PaneId).or(fallback)?;
    let notification = Notification::from_ai_hook(
        event,
        event.message.clone(),
        event.kind == AiHookKind::NeedsAttention,
    )?;
    Some((notification, pane))
}

// ─── 投递入口 ────────────────────────────────────────────────────────────

pub(crate) fn deliver_gpui(notification: &Notification, pane_id: PaneId, cx: &App) {
    deliver_gpui_with_choices(notification, pane_id, None, cx);
}

/// 启动期一次性汇总——走漏斗(节流/闪烁统一),但豁免前台门控,
/// 且不绑定 pane(点击不回窗)。
pub(crate) fn deliver_startup_summary(notification: &Notification, cx: &App) {
    deliver_with(notification, PaneId(0), None, true, cx);
}

/// 单一投递入口:前台门控 → 节流计划 → 闪烁先行 → 系统 toast。
/// `confirmation` = 权限确认的 (request_id, 候选标签),动作按钮直达
/// confirmation 链(确认语义归安全半程)。HWND 查询经
/// `windowing` facade(单窗单实例的唯一读口)。
pub(crate) fn deliver_gpui_with_choices(
    notification: &Notification,
    pane_id: PaneId,
    confirmation: Option<(u64, Vec<String>)>,
    cx: &App,
) {
    deliver_with(notification, pane_id, confirmation, false, cx);
}

fn deliver_with(
    notification: &Notification,
    pane_id: PaneId,
    confirmation: Option<(u64, Vec<String>)>,
    startup_summary: bool,
    cx: &App,
) {
    static FUNNEL: OnceLock<Mutex<Funnel>> = OnceLock::new();
    let mut funnel = FUNNEL
        .get_or_init(|| Mutex::new(Funnel::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let request = confirmation.as_ref().map(|confirmation| confirmation.0);
    let plan = if startup_summary {
        funnel.plan_startup(notification, pane_id, foreground(), request, Instant::now())
    } else {
        funnel.plan(notification, pane_id, foreground(), request, Instant::now())
    };
    drop(funnel);
    if plan.flash {
        // 闪烁先行:节流只挡系统 toast,不挡任务栏提示。
        crate::platform::notifications::flash_taskbar(crate::windowing::focus::window_hwnd(cx));
    }
    let Some((title, body)) = plan.toast else {
        log::debug!(
            "notify: toast suppressed for pane={}: {notification:?}",
            pane_id.0
        );
        return;
    };
    log::debug!(
        "notify: system toast for pane={}: {notification:?}",
        pane_id.0
    );
    let actions = confirmation
        .and_then(|(request_id, labels)| {
            let sender = GPUI_ACTIVATION.get()?.clone();
            Some(
                labels
                    .into_iter()
                    .enumerate()
                    .map(|(choice, label)| {
                        let sender = sender.clone();
                        ToastAction {
                            label: label.chars().take(80).collect(),
                            activate: Arc::new(move || {
                                let _ = sender.send(SltermShellEvent::NotificationChoice {
                                    pane_id,
                                    request_id,
                                    choice,
                                });
                            }),
                        }
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    let open = if startup_summary {
        application_activation(None)
    } else {
        application_activation(Some(pane_id))
    };
    crate::platform::notifications::dispatch(title, body, open, actions);
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    use crate::ai_hook::parse_envelope;

    fn event(source: &str, pane: Option<u64>, payload: &str) -> AiHookEvent {
        let header = match pane {
            Some(pane) => format!("slterm-hook/1 source={source} pane={pane}\n"),
            None => format!("slterm-hook/1 source={source}\n"),
        };
        parse_envelope(format!("{header}{payload}").as_bytes()).expect("测试信封应可解析")
    }

    fn claude(payload: &str) -> AiHookEvent {
        event("claude", Some(1), payload)
    }

    fn ai_turn(program: &str, message: Option<&str>, attention: bool) -> Notification {
        Notification::AiTurn {
            program: program.to_owned(),
            message: message.map(str::to_owned),
            attention,
        }
    }

    fn ai_issue(message: Option<&str>, outcome: AiTurnOutcome) -> Notification {
        Notification::AiTurnIssue {
            program: "claude".to_owned(),
            message: message.map(str::to_owned),
            outcome,
        }
    }

    /// 短成功命令静默、hook 确认过的长成功命令静默、非零退出必成失败。
    #[test]
    fn command_finished_keeps_failures_and_silences_covered_successes() {
        for hook_seen in [false, true] {
            let failed = Notification::command_finished(
                Some("cargo".into()),
                Duration::from_millis(50),
                Some(1),
                hook_seen,
            )
            .expect("非零退出必须通知");
            assert!(failed.is_failure());
            assert!(!failed.is_attention());
            assert!(
                Notification::command_finished(
                    Some("cargo".into()),
                    Duration::from_millis(50),
                    Some(0),
                    hook_seen
                )
                .is_none(),
                "短成功命令不通知"
            );
        }
        assert!(
            Notification::command_finished(
                Some("codex".into()),
                Duration::from_secs(60),
                Some(0),
                true
            )
            .is_none(),
            "hook 覆盖的短成功命令静默"
        );
        assert!(
            Notification::command_finished(
                Some("cargo".into()),
                Duration::from_secs(60),
                Some(0),
                false
            )
            .is_some(),
            "未覆盖的长成功命令通知"
        );
    }

    /// 边映射:在飞后台任务/取消静默;失败与截断是 issue;pi/omp
    /// 旧桥缺结果元数据按未确认成功;NeedsAttention 带 attention。
    #[test]
    fn hook_edges_map_to_ai_turn_and_issue_notifications() {
        let busy = event(
            "claude",
            Some(1),
            r#"{"hook_event_name":"Stop","background_tasks":[{"type":"bash","status":"running"}]}"#,
        );
        assert!(Notification::from_ai_hook(&busy, None, false).is_none());

        // kimi 的 Interrupt 是显式取消边(claude 桥不产该事件)。
        let cancelled = event("kimi", Some(1), r#"{"hook_event_name":"Interrupt"}"#);
        assert!(Notification::from_ai_hook(&cancelled, None, false).is_none());

        let failed = claude(r#"{"hook_event_name":"StopFailure","error":"overloaded"}"#);
        let note = Notification::from_ai_hook(&failed, Some("detail".into()), false)
            .expect("provider 报错必须通知");
        assert!(note.is_failure());
        assert!(matches!(
            note,
            Notification::AiTurnIssue {
                outcome: AiTurnOutcome::Failed,
                ..
            }
        ));

        let incomplete = event("pi", Some(1), r#"{"kind":"done","stop_reason":"length"}"#);
        let note = Notification::from_ai_hook(&incomplete, Some("detail".into()), false)
            .expect("截断必须通知");
        assert!(note.is_failure());
        assert!(matches!(
            note,
            Notification::AiTurnIssue {
                outcome: AiTurnOutcome::Incomplete,
                ..
            }
        ));

        let bare_pi = event("pi", Some(1), r#"{"kind":"done"}"#);
        let issue = Notification::from_ai_hook(&bare_pi, Some("ignored".into()), false)
            .expect("pi 无结果元数据按未确认成功");
        assert!(matches!(
            issue,
            Notification::AiTurnIssue {
                outcome: AiTurnOutcome::Unknown,
                ..
            }
        ));
        assert!(!issue.is_failure());

        let attention = claude(r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash"}"#);
        let note = Notification::from_ai_hook(&attention, Some("允许?".into()), true)
            .expect("权限请求必须通知");
        assert!(note.is_attention());
        assert_eq!(note.toast_text(), ("claude".to_owned(), "允许?".to_owned()));

        let done = claude(r#"{"hook_event_name":"Stop"}"#);
        let note = Notification::from_ai_hook(&done, None, false).expect("回合完成通知");
        assert!(!note.is_attention());
        assert!(!note.is_failure());
    }

    /// 已接纳事件 → (通知, pane):只有 TurnDone/NeedsAttention 出通知,
    /// 缺 pane 时用焦点兜底。
    #[test]
    fn accepted_hook_edges_resolve_their_target_pane() {
        let done = claude(r#"{"hook_event_name":"Stop"}"#);
        let (notification, pane) = from_accepted_hook(&done, None).expect("TurnDone 必须出通知");
        assert!(matches!(notification, Notification::AiTurn { .. }));
        assert_eq!(pane, PaneId(1));

        let mut fallback = done.clone();
        fallback.pane = None;
        let (_, pane) = from_accepted_hook(&fallback, Some(PaneId(9))).expect("焦点兜底");
        assert_eq!(pane, PaneId(9));
        assert!(from_accepted_hook(&fallback, None).is_none());

        let prompt = claude(r#"{"hook_event_name":"UserPromptSubmit"}"#);
        assert!(from_accepted_hook(&prompt, None).is_none());
    }

    /// 预览:>160 截断加省略号、空白折叠、幂等、Unicode 边界安全。
    #[test]
    fn toast_previews_are_bounded_single_line_and_idempotent() {
        let long = "回答正文。".repeat(1_000);
        let notification = ai_turn("codex", Some(&long), false);
        let (title, body) = notification.toast_text();
        assert_eq!(title, "codex");
        assert!(body.chars().count() <= TOAST_BODY_MAX_CHARS);
        assert!(body.ends_with('…'));
        assert!(long.starts_with(body.trim_end_matches('…')));
        assert_eq!(clamp_toast_body(&body), body);

        for count in [0, 1, TOAST_BODY_MAX_CHARS - 1, TOAST_BODY_MAX_CHARS] {
            let source = "界".repeat(count);
            assert_eq!(clamp_toast_body(&source), source);
        }
        let emoji = "😀".repeat(TOAST_BODY_MAX_CHARS + 1);
        let preview = clamp_toast_body(&emoji);
        assert_eq!(
            preview,
            format!("{}…", "😀".repeat(TOAST_BODY_MAX_CHARS - 1))
        );
        assert_eq!(clamp_toast_body(&preview), preview);

        assert_eq!(
            clamp_toast_body("a\r\nb\tc\u{2028}d\u{2029}e"),
            "a  b c d e"
        );
    }

    /// 原文不被截断:预览只收窄正文,日志/壳内横幅仍拿全文。
    #[test]
    fn raw_text_stays_unbounded_while_preview_is_clamped() {
        let source = format!("{}done", "x\n".repeat(60));
        let notification = Notification::Text {
            body: source.clone(),
            program: None,
        };
        let (_, preview) = notification.toast_text();
        assert_eq!(preview.lines().count(), 1);
        assert_eq!(preview, source.replace('\n', " "));
        assert_eq!(notification.raw_toast_text().1, source);
    }

    /// 类型判据:attention 只认 AiTurn{true};失败只认 CommandFailed 与
    /// Failed/Incomplete 的 issue。
    #[test]
    fn attention_and_failure_classification_is_strict() {
        assert!(ai_turn("claude", Some("确认"), true).is_attention());
        assert!(!ai_turn("codex", None, false).is_attention());
        assert!(!Notification::Bell { program: None }.is_attention());
        assert!(
            !Notification::Text {
                body: "permission".into(),
                program: None
            }
            .is_attention()
        );

        assert!(ai_issue(None, AiTurnOutcome::Failed).is_failure());
        assert!(ai_issue(None, AiTurnOutcome::Incomplete).is_failure());
        assert!(!ai_issue(None, AiTurnOutcome::Unknown).is_failure());
        assert!(
            Notification::CommandFailed {
                duration: Duration::from_secs(1),
                program: None,
                exit_code: 2
            }
            .is_failure()
        );
    }

    /// 来源分类只问 AgentKind::parse;正文关键字不参与。
    #[test]
    fn source_classification_never_reads_message_text() {
        for source in ["claude", "codex", "gemini", "kimi", "pi", "omp"] {
            assert!(
                Notification::Bell {
                    program: Some(source.into())
                }
                .is_ai()
            );
        }
        for program in [r"C:\tools\CODEX.EXE", "/usr/bin/claude-code"] {
            assert!(
                Notification::Bell {
                    program: Some(program.into())
                }
                .is_ai()
            );
        }
        for program in [None, Some("cargo".to_owned()), Some("pwsh".to_owned())] {
            assert!(!Notification::Bell { program }.is_ai());
            assert!(
                !Notification::Text {
                    body: "codex claude permission required".into(),
                    program: Some("cargo".into()),
                }
                .is_ai()
            );
        }
    }

    /// 焦点闸门:窗口聚焦时全部六变体零投递(连闪烁都不发)。
    #[test]
    fn focused_window_suppresses_every_variant_before_flash() {
        let now = Instant::now();
        for notification in [
            Notification::Bell {
                program: Some("claude".into()),
            },
            Notification::CommandDone {
                duration: Duration::from_secs(30),
                program: None,
            },
            Notification::CommandFailed {
                duration: Duration::from_secs(1),
                program: None,
                exit_code: 1,
            },
            Notification::Text {
                body: "done".into(),
                program: None,
            },
            ai_turn("codex", None, false),
            ai_issue(None, AiTurnOutcome::Failed),
        ] {
            let mut funnel = Funnel::new();
            assert_eq!(
                funnel.plan(&notification, PaneId(1), true, None, now),
                DeliveryPlan::default(),
                "{notification:?}"
            );
        }
    }

    /// 闪烁先行:被全局节流拦下时 flash 仍恰好为一次,toast 为 None;
    /// 窗口外放行。
    #[test]
    fn flash_precedes_the_global_throttle_and_survives_it() {
        let now = Instant::now();
        let first = ai_turn("claude", None, true);
        let second = ai_turn("codex", None, true);
        let mut funnel = Funnel::new();
        let planned = funnel.plan(&first, PaneId(1), false, None, now);
        assert!(planned.flash);
        assert!(planned.toast.is_some());

        // 另一个 pane 也在全局 3s 窗口内被吞,但闪烁照发。
        let suppressed = funnel.plan(
            &second,
            PaneId(2),
            false,
            None,
            now + Duration::from_secs(2),
        );
        assert!(suppressed.flash);
        assert!(suppressed.toast.is_none());

        let allowed = funnel.plan(&second, PaneId(2), false, None, now + TOAST_THROTTLE);
        assert!(allowed.flash);
        assert!(allowed.toast.is_some());
    }

    /// 启动汇总走漏斗但豁免前台门控,且不闪烁、不绑定 pane 激活。
    #[test]
    fn startup_summary_bypasses_the_foreground_gate_without_flashing() {
        let mut funnel = Funnel::new();
        let notification = Notification::RecoverySummary {
            text: "恢复 2 个标签页".into(),
        };
        assert_eq!(
            funnel.plan(&notification, PaneId(1), true, None, Instant::now()),
            DeliveryPlan::default(),
            "普通通知在前台必须完全安静"
        );
        let plan = funnel.plan_startup(&notification, PaneId(1), true, None, Instant::now());
        assert!(plan.toast.is_some(), "启动汇总必须穿透前台门控");
        assert!(!plan.flash, "启动汇总不得闪烁任务栏");
    }

    /// 进程级提示沿用启动汇总通道,标题取品牌、正文原样、不算 AI 来源。
    #[test]
    fn process_alert_uses_the_brand_title_and_is_not_ai_sourced() {
        let notification = Notification::ProcessAlert {
            text: "Claude Code 的 hook 安装失败".into(),
        };
        let (title, body) = notification.raw_toast_text();
        assert_eq!(title, crate::brand::NAME);
        assert_eq!(body, "Claude Code 的 hook 安装失败");
        assert!(!notification.is_ai());
        assert!(!notification.is_attention());
        assert!(!notification.is_failure());

        let mut funnel = Funnel::new();
        let plan = funnel.plan_startup(&notification, PaneId(0), true, None, Instant::now());
        assert!(plan.toast.is_some(), "进程级提示必须穿透前台门控");
        assert!(!plan.flash);
    }

    /// pane 级同键窗口:同 pane+attention+request 3s 内只放一条;不同
    /// attention/request 独立(连续提问身份不串)。
    #[test]
    fn pane_throttle_keeps_attention_and_request_identities_independent() {
        let now = Instant::now();
        let mut throttle = PaneNotificationThrottle::default();
        assert!(throttle.accepts_request(PaneId(1), false, None, now));
        assert!(throttle.accepts_request(PaneId(2), false, None, now));
        assert!(!throttle.accepts_request(PaneId(1), false, None, now));
        assert!(throttle.accepts_request(PaneId(1), true, None, now));
        assert!(!throttle.accepts_request(PaneId(1), true, None, now));
        assert!(throttle.accepts_request(PaneId(1), true, Some(100), now));
        assert!(!throttle.accepts_request(PaneId(1), true, Some(100), now));
        assert!(throttle.accepts_request(PaneId(1), true, Some(101), now));
    }

    /// 失败 episode:同 pane+程序+结局 30s 只放一条;回合完成(恢复)结束
    /// episode;新错立即可弹;跨 pane 独立。
    #[test]
    fn failure_episode_cools_down_until_recovery_or_a_new_error() {
        let failure = ai_issue(Some("overload"), AiTurnOutcome::Failed);
        let now = Instant::now();
        let mut gate = PaneFailureThrottle::new();
        assert!(gate.accepts(PaneId(1), &failure, true, now));
        assert!(gate.accepts(PaneId(2), &failure, true, now));
        assert!(!gate.accepts(PaneId(1), &failure, true, now + Duration::from_secs(6)));

        let recovery = ai_turn("claude", None, false);
        let attention = ai_turn("claude", None, true);
        assert!(gate.accepts(PaneId(1), &attention, true, now + Duration::from_secs(7)));
        assert!(
            !gate.accepts(PaneId(1), &failure, true, now + Duration::from_secs(8)),
            "attention 不结束失败 episode"
        );
        let other = ai_issue(Some("authentication failed"), AiTurnOutcome::Failed);
        assert!(gate.accepts(PaneId(1), &other, true, now + Duration::from_secs(9)));
        assert!(!gate.accepts(PaneId(1), &failure, true, now + Duration::from_secs(29)));
        assert!(gate.accepts(PaneId(1), &failure, true, now + FAILURE_COOLDOWN));

        // 可见的恢复事件即使不投递也结束 episode。
        assert!(
            !gate.accepts(PaneId(1), &recovery, false, now + Duration::from_secs(31)),
            "恢复事件本身无需 toast"
        );
        assert!(gate.accepts(PaneId(1), &failure, true, now + Duration::from_secs(32)));
    }

    /// 失败 issue 与不完整 issue 的文案区分(错误原因只在有原文时出现)。
    #[test]
    fn turn_issue_text_distinguishes_failure_incomplete_and_unknown() {
        assert_eq!(
            turn_issue_text(AiTurnOutcome::Failed, Some("overloaded"), UiLanguage::EnUs),
            "Request failed: overloaded"
        );
        assert_eq!(
            turn_issue_text(AiTurnOutcome::Failed, None, UiLanguage::EnUs),
            "Request failed. Check the terminal for details."
        );
        assert_eq!(
            turn_issue_text(AiTurnOutcome::Incomplete, Some("ignored"), UiLanguage::ZhCn),
            "回答未完成，详情见终端。"
        );
        assert_eq!(
            turn_issue_text(AiTurnOutcome::Unknown, None, UiLanguage::ZhCn),
            "回合结束。"
        );
    }
}
