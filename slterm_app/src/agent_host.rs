//! 单 pane agent 宿主(M4.5 壳侧事件通道的落点)。
//!
//! 每个 pane 一份 `AgentPaneHost`:持有 `AgentActivity` 仲裁器与 `AnswerInbox`
//! 收件箱。`AgentHub` 是 workspace 侧按 `PaneId` 的归属表——事件泵把整批
//! 事件交给它,由信封 pane 字段严格路由(已关闭 pane 的迟到事件直接丢弃,
//! 只有信封确实缺 pane 时才回退焦点 pane)。
//!
//! 纪律:
//! - SDK 状态只在前台线程更新:管道读与解析在服务线程,本模块的入口全部由
//!   workspace 的钩子泵在 GPUI foreground executor 上调用;
//! - 屏幕是有限权威观察:只有 `allows_screen()` 的 pane 才吃屏幕样本,
//!   采样节奏由 workspace 的 1 Hz 看门狗统一驱动,不在每个输出事件上跑规则;
//! - 本步不写会话注册表、不发通知;屏幕规则引擎只读(用户覆盖热加载归
//!   `ai_agents` 自身)。
//!
//! 缝合点(后阶段件,不在 M4.5 落地):
//! - 宿主随 `TerminalPane` 持有归 05 篇 M5.3(届时 `AgentHub` 退成薄查询面);
//! - `session` 反填的恢复注入消费归 05 篇 M5.5(`restore_agent` 只构造命令);
//! - 回答展示(收件箱正文)归 07 篇;
//! - TurnDone/NeedsAttention 状态边 → 通知漏斗归 09 篇 M10.1;
//! - 会话注册表写入(`ai_sessions::record_hook_session`)归 05 篇 M5.5。

use std::collections::HashMap;

use slterm_split::PaneId;

#[cfg(test)]
use crate::ai_agents::Detection;
use crate::ai_agents::{self, AgentKind, AgentStatus, AgentStatusSource};
use crate::ai_hook::{
    AgentActivity, AiHookEvent, AiHookKind, GateVerdict, accept_for_pane, reorder_batch,
};
use crate::assistant_answer::{AnswerInbox, AnswerSnapshot};

/// 一个 pane 的 agent 宿主状态。
pub(crate) struct AgentPaneHost {
    pane_id: PaneId,
    /// 生命周期仲裁器(hook 是事实,屏幕是有限权威观察)。
    activity: AgentActivity,
    /// 回答收件箱:只收 provider 原文,不从屏幕猜测。
    inbox: AnswerInbox,
    /// 屏幕采样与未来徽标的程序身份:hook 事件为权威来源;无 hook 的
    /// pane 才允许由屏幕 chrome 兜底识别(见 `observe_screen`)。
    program: Option<String>,
    /// hook 反填的 `(source, session_id)`;恢复注入消费归 05 篇 M5.5。
    session: Option<(String, String)>,
}

impl AgentPaneHost {
    pub(crate) fn new(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            activity: AgentActivity::default(),
            inbox: AnswerInbox::default(),
            program: None,
            session: None,
        }
    }

    /// 路由一条已按 pane 归属的事件:所有权预检 → 事件门(七判)→ 仲裁器
    /// → 回答收编。返回是否真正改写了本 pane 的状态(门拒收/所有权不符
    /// 返回 false,调用方据此记日志)。
    pub(crate) fn handle_hook(&mut self, event: &AiHookEvent) -> bool {
        if !self.activity.accepts_hook(event) {
            log::debug!(
                "ai_hook: pane={} ignored hook outside the active lifecycle",
                self.pane_id.0
            );
            return false;
        }
        // 事件门在最终 pane 已解析后调用:全进程共用一扇门,被拒事件必须
        // 留下可取证的原因,静默丢弃是这条链路最难查的一类故障。
        let verdict: GateVerdict = accept_for_pane(event, self.pane_id.0);
        if !verdict.accepted() {
            log::debug!(
                "ai_hook: pane={} source={} dropped {verdict:?}",
                self.pane_id.0,
                event.source
            );
            return false;
        }
        self.activity.apply_hook(event);
        // 收件箱自带 pane/provider/session 三重归属校验;焦点兜底
        // (pane=None)事件不得供给阅读器正文,防两个 pane 串台。
        self.inbox.observe(event, self.pane_id.0);
        if event.kind == AiHookKind::SessionEnd {
            self.program = None;
            self.session = None;
        } else {
            self.program = Some(event.source.clone());
            if let Some(id) = event.session_id.as_deref() {
                self.session = Some((event.source.clone(), id.to_owned()));
            }
        }
        true
    }

    /// 一次屏幕样本(最近至多 24 行)进仲裁器。身份只在没有 hook 权威时
    /// 由屏幕 chrome 建立——hook 永远是更高置信度的来源。
    pub(crate) fn observe_screen(&mut self, screen: &str) -> bool {
        if !self.activity.allows_screen() {
            return false;
        }
        if self.program.as_deref().and_then(AgentKind::parse).is_none() {
            let Some(agent) = ai_agents::identify(screen) else {
                // 没有可识别的 agent:连续空闲证据清零(静默不等于空闲)。
                self.activity.observe_screen(None);
                return false;
            };
            self.program = Some(agent.slug().to_owned());
            self.activity.identify(AgentStatusSource::Screen);
        }
        let Some(program) = self.program.as_deref() else {
            return false;
        };
        let detection = ai_agents::detect(program, screen);
        self.activity.observe_screen(detection)
    }

    /// 当前仲裁状态(页签徽标/runtime 快照消费归 05 篇 M5.3 / 04 篇 M7)。
    #[allow(dead_code)]
    pub(crate) fn status(&self) -> AgentStatus {
        self.activity.status()
    }

    /// 状态来源(Hook/Screen/Process;消费点同上)。
    #[allow(dead_code)]
    pub(crate) fn source(&self) -> AgentStatusSource {
        self.activity.source()
    }

    /// 是否已有 hook 权威(通知漏斗的 fallback 判据归 09 篇 M10.1)。
    #[allow(dead_code)]
    pub(crate) fn hook_seen(&self) -> bool {
        self.activity.hook_seen()
    }

    /// 命中的屏幕规则 id(可解释性面;消费归 05/09 篇)。
    #[allow(dead_code)]
    pub(crate) fn rule(&self) -> Option<&str> {
        self.activity.rule()
    }

    /// 当前屏幕/进程身份(未来页签徽标与 runtime 快照消费)。
    #[allow(dead_code)] // 消费点归 05 篇 M5.3 / 04 篇 M7。
    pub(crate) fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    /// hook 反填的会话身份(恢复注入消费归 05 篇 M5.5)。
    #[allow(dead_code)]
    pub(crate) fn session(&self) -> Option<(&str, &str)> {
        self.session
            .as_ref()
            .map(|(source, id)| (source.as_str(), id.as_str()))
    }

    /// 最新回答快照(展示归 07 篇)。
    #[allow(dead_code)]
    pub(crate) fn latest_answer(&self) -> Option<&AnswerSnapshot> {
        self.inbox.latest.as_ref()
    }

    /// 仅测试断言用:直接喂一次检测结果,绕开规则引擎的磁盘覆盖读取。
    #[cfg(test)]
    pub(crate) fn observe_detection(&mut self, detection: Option<Detection>) -> bool {
        self.activity.observe_screen(detection)
    }
}

/// workspace 侧的 pane 归属表。写入点与 `terminal_registry` 同纪律:
/// 只在 pane 创建/关闭两处增删,泵与看门狗只读路由。
#[derive(Default)]
pub(crate) struct AgentHub {
    hosts: HashMap<PaneId, AgentPaneHost>,
}

impl AgentHub {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&mut self, pane_id: PaneId) {
        self.hosts.insert(pane_id, AgentPaneHost::new(pane_id));
    }

    pub(crate) fn remove(&mut self, pane_id: PaneId) {
        self.hosts.remove(&pane_id);
    }

    /// 读端(徽标/runtime 快照消费归 05 篇 M5.3 / 04 篇 M7;测试经此断言)。
    #[allow(dead_code)]
    pub(crate) fn host(&self, pane_id: PaneId) -> Option<&AgentPaneHost> {
        self.hosts.get(&pane_id)
    }

    /// 事件泵单批派发:同批重排后逐条按 pane 路由。返回被接纳(真正改写
    /// 状态)的事件数。
    ///
    /// 严格路由合同:信封带 pane 时它必须对应现存 pane,已关闭 pane 的
    /// 迟到 hook 直接丢弃;只有信封确实缺 pane 字段(环境变量半路被剥掉)
    /// 才回退到焦点 pane。
    pub(crate) fn dispatch(
        &mut self,
        events: Vec<AiHookEvent>,
        focus_fallback: Option<PaneId>,
    ) -> usize {
        let mut accepted = 0;
        for event in reorder_batch(events) {
            let target = match event.pane {
                Some(pane) => Some(PaneId(pane)),
                None => focus_fallback,
            };
            let Some(target) = target else {
                continue;
            };
            let Some(host) = self.hosts.get_mut(&target) else {
                log::debug!(
                    "ai_hook: pane={} already closed; late event dropped",
                    target.0
                );
                continue;
            };
            if host.handle_hook(&event) {
                accepted += 1;
            }
        }
        accepted
    }

    /// 屏幕看门狗单步(1 Hz 节奏由 workspace 调度)。无宿主的 pane 静默跳过。
    pub(crate) fn observe_screen(&mut self, pane_id: PaneId, screen: &str) -> bool {
        self.hosts
            .get_mut(&pane_id)
            .is_some_and(|host| host.observe_screen(screen))
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;
    use crate::ai_agents::AgentKind;
    use crate::ai_hook::parse_envelope;

    fn event(source: &str, pane: Option<u64>, payload: &str) -> AiHookEvent {
        let header = match pane {
            Some(pane) => format!("slterm-hook/1 source={source} pane={pane}\n"),
            None => format!("slterm-hook/1 source={source}\n"),
        };
        parse_envelope(format!("{header}{payload}").as_bytes()).expect("测试信封应可解析")
    }

    fn prompt_submit(session: &str, pane: u64) -> AiHookEvent {
        event(
            "claude",
            Some(pane),
            &format!(r#"{{"hook_event_name":"UserPromptSubmit","session_id":"{session}"}}"#),
        )
    }

    #[test]
    fn dispatch_routes_strictly_by_pane_and_drops_closed_targets() {
        let mut hub = AgentHub::new();
        hub.register(PaneId(1));
        hub.register(PaneId(2));
        assert_eq!(hub.dispatch(vec![prompt_submit("route-a", 1)], None), 1);
        assert_eq!(hub.host(PaneId(1)).unwrap().status(), AgentStatus::Working);
        assert_eq!(hub.host(PaneId(2)).unwrap().status(), AgentStatus::Unknown);
        // 已关闭/未注册 pane 的迟到事件不得回退到焦点 pane。
        assert_eq!(
            hub.dispatch(vec![prompt_submit("route-closed", 9)], Some(PaneId(2))),
            0
        );
        assert_eq!(hub.host(PaneId(2)).unwrap().status(), AgentStatus::Unknown);
        // 信封缺 pane 字段时才允许焦点兜底。
        let mut fallback = prompt_submit("route-fallback", 2);
        fallback.pane = None;
        assert_eq!(hub.dispatch(vec![fallback], Some(PaneId(2))), 1);
        assert_eq!(hub.host(PaneId(2)).unwrap().status(), AgentStatus::Working);
    }

    #[test]
    fn turn_done_answer_is_owned_by_its_pane_and_provider_session() {
        let mut hub = AgentHub::new();
        hub.register(PaneId(3));
        hub.register(PaneId(4));
        let done = event(
            "claude",
            Some(3),
            r#"{"hook_event_name":"Stop","session_id":"answer-session","last_assistant_message":"原样回答"}"#,
        );
        assert_eq!(hub.dispatch(vec![done], None), 1);
        let host = hub.host(PaneId(3)).unwrap();
        assert_eq!(host.status(), AgentStatus::Done);
        assert_eq!(host.source(), AgentStatusSource::Hook);
        let latest = host.latest_answer().expect("回答应入收件箱");
        assert_eq!(latest.provider, "claude");
        assert_eq!(latest.session_id, "answer-session");
        assert_eq!(
            latest.content.source().expect("完整回答").as_ref(),
            "原样回答"
        );
        // 另一个 pane 的收件箱不得串台。
        assert!(hub.host(PaneId(4)).unwrap().latest_answer().is_none());
    }

    #[test]
    fn screen_identity_only_speaks_for_panes_without_hook_authority() {
        // 无 hook 的 pane:屏幕品牌 chrome 建立身份,规则引擎给出 Working。
        let mut host = AgentPaneHost::new(PaneId(5));
        let screen = "welcome to kimi code!\n🌔 · Tip: keep going";
        assert!(host.observe_screen(screen));
        assert_eq!(host.status(), AgentStatus::Working);
        assert_eq!(host.source(), AgentStatusSource::Screen);
        assert_eq!(host.program(), Some("kimi"));
        assert_eq!(host.rule(), Some("moon_spinner_working"));

        // 完整生命周期 hook(claude)拥有状态:屏幕结果被忽略。
        let mut hooked = AgentPaneHost::new(PaneId(6));
        let start = event(
            "claude",
            Some(6),
            r#"{"hook_event_name":"SessionStart","session_id":"screen-authority"}"#,
        );
        assert!(hooked.handle_hook(&start));
        assert_eq!(hooked.status(), AgentStatus::Idle);
        assert!(!hooked.observe_screen("esc to interrupt"));
        assert_eq!(hooked.status(), AgentStatus::Idle);
        assert_eq!(hooked.source(), AgentStatusSource::Hook);

        // 纯检测注入路径(不触规则引擎磁盘覆盖):仲裁器边与 M4.2 用例同源。
        assert!(host.observe_detection(Some(Detection {
            agent: AgentKind::Kimi,
            status: AgentStatus::Blocked,
            rule_id: "test.injected".into(),
        })));
        assert_eq!(host.status(), AgentStatus::Blocked);
    }
}
