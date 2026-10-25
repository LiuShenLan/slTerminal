//! 单 pane agent 宿主(壳侧事件通道的落点)。
//!
//! 每个 pane 一份 `AgentPaneHost`:持有 `AgentActivity` 仲裁器。
//! `AgentHub` 是 workspace 侧按 `PaneId` 的归属表——事件泵把整批
//! 事件交给它,由信封 pane 字段严格路由(已关闭 pane 的迟到事件直接丢弃,
//! 只有信封确实缺 pane 时才回退焦点 pane)。
//!
//! 纪律:
//! - SDK 状态只在前台线程更新:管道读与解析在服务线程,本模块的入口全部由
//!   workspace 的钩子泵在 GPUI foreground executor 上调用;
//! - 屏幕是有限权威观察:只有 `allows_screen()` 的 pane 才吃屏幕样本,
//!   采样节奏由 workspace 的 1 Hz 看门狗统一驱动,不在每个输出事件上跑规则;
//! - 屏幕规则引擎只读(用户覆盖热加载归 `ai_agents` 自身)。
//!
//! 缝合点(已接):
//! - `SessionStart` 边把会话身份写进 `ai_sessions` 注册表:写盘在后台线程,
//!   生产路径由 workspace 显式启用;宿主本身不碰磁盘。
//! - 恢复注入:workspace 恢复 tab 时先 `seed_session`,再让视图
//!   `restore_agent` 构造并提交 resume 命令;
//! - TurnDone/NeedsAttention 状态边 → 通知漏斗归 `notify`。

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use slterm_split::PaneId;

#[cfg(test)]
use crate::ai_agents::Detection;
use crate::ai_agents::{self, AgentKind, AgentStatus, AgentStatusSource};
use crate::ai_hook::{
    AgentActivity, AiHookEvent, AiHookKind, GateVerdict, accept_for_pane, reorder_batch,
};
use crate::session::AgentSession;

/// 会话身份注册表写入通道(生产 = 后台线程 + `ai_sessions::record_hook_session`;
/// 测试可注入捕获器,禁止触碰真实数据目录)。
pub(crate) type SessionStartRecorder = Rc<dyn Fn(&AiHookEvent)>;

/// 注册表写入动作:参数由 SessionStart 事件解出。生产 = 真实写盘;测试注入
/// 捕获器,断言字段原样到达且写入发生在后台线程。
type SessionRegistryWrite = Arc<dyn Fn(&str, &str, Option<PathBuf>) + Send + Sync>;

/// 后台注册表写入线程的唯一启动点:锁 + fsync 不得占用调用方(GPUI 前台)。
/// 线程起不来只 warn——注册表是恢复线索,不阻塞事件泵。
fn spawn_session_registry_write(write: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new()
        .name("slterm-ai-sessions".to_owned())
        .spawn(write)
    {
        log::warn!("agent session index: could not spawn writer thread: {error}");
    }
}

/// 一个 pane 的 agent 宿主状态。
pub(crate) struct AgentPaneHost {
    pane_id: PaneId,
    /// 生命周期仲裁器(hook 是事实,屏幕是有限权威观察)。
    activity: AgentActivity,
    /// 屏幕采样与未来徽标的程序身份:hook 事件为权威来源;无 hook 的
    /// pane 才允许由屏幕 chrome 兜底识别(见 `observe_screen`)。
    program: Option<String>,
    /// hook 反填的会话身份;快照落布局叶的 `agent` 字段,恢复注入据此
    /// 重建 resume 命令。
    session: Option<AgentSession>,
}

impl AgentPaneHost {
    pub(crate) fn new(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            activity: AgentActivity::default(),
            program: None,
            session: None,
        }
    }

    /// 路由一条已按 pane 归属的事件:所有权预检 → 事件门(七判)→ 仲裁器。
    /// 返回是否真正改写了本 pane 的状态(门拒收/所有权不符返回 false)。
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
        if event.kind == AiHookKind::SessionEnd {
            self.program = None;
            self.session = None;
        } else {
            self.program = Some(event.source.clone());
            if event.session_id.is_some() || event.session_file.is_some() {
                // 同一会话的后续事件可能只带 id 或只带原生文件:两个缺省
                // 字段都沿用同来源的既有值,一次半量更新不能抹掉恢复线索。
                let previous = self
                    .session
                    .as_ref()
                    .filter(|session| session.source == event.source);
                let session_id = event
                    .session_id
                    .clone()
                    .or_else(|| previous.and_then(|session| session.session_id.clone()));
                let session_file = event
                    .session_file
                    .clone()
                    .or_else(|| previous.and_then(|session| session.session_file.clone()));
                if session_id.is_some() || session_file.is_some() {
                    self.session = Some(AgentSession {
                        source: event.source.clone(),
                        session_id,
                        session_file,
                    });
                }
            }
        }
        true
    }

    /// OSC 133;C 的提交边:已知程序身份时按程序语义(含 codex `/plan`
    /// 例外)推进,否则走通用 submitted。
    pub(crate) fn command_started(&mut self) {
        match self.program.as_deref() {
            Some(program) => self.activity.submitted_text(program, ""),
            None => self.activity.submitted(),
        }
    }

    /// OSC 133;D 的完成边。
    pub(crate) fn command_finished(&mut self) {
        self.activity.command_finished();
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

    /// 当前仲裁状态（页签徽标、关闭守卫与通知消费归工作区）。
    pub(crate) fn status(&self) -> AgentStatus {
        self.activity.status()
    }

    /// 状态来源（Hook/Screen/Process）。
    #[allow(dead_code)]
    pub(crate) fn source(&self) -> AgentStatusSource {
        self.activity.source()
    }

    /// 是否已有 hook 权威(通知漏斗的 fallback 判据归 `notify`)。
    #[allow(dead_code)]
    pub(crate) fn hook_seen(&self) -> bool {
        self.activity.hook_seen()
    }

    /// 命中的屏幕规则 id(可解释性面;消费归工作区与通知域)。
    #[allow(dead_code)]
    pub(crate) fn rule(&self) -> Option<&str> {
        self.activity.rule()
    }

    /// 当前屏幕/进程身份（托盘与恢复消费）。
    pub(crate) fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    /// hook 反填的会话身份(快照/恢复注入的权威来源)。
    pub(crate) fn session(&self) -> Option<&AgentSession> {
        self.session.as_ref()
    }

    /// 仅测试断言用:直接喂一次检测结果,绕开规则引擎的磁盘覆盖读取。
    #[cfg(test)]
    pub(crate) fn observe_detection(&mut self, detection: Option<Detection>) -> bool {
        self.activity.observe_screen(detection)
    }
}

/// workspace 侧的 pane 归属表。只在 pane 创建/关闭两处增删,
/// 泵与看门狗只读路由。
#[derive(Default)]
pub(crate) struct AgentHub {
    hosts: HashMap<PaneId, AgentPaneHost>,
    /// 生产路径由 workspace 显式启用;未启用 = 纯内存宿主(测试默认态)。
    record_session_start: Option<SessionStartRecorder>,
}

impl AgentHub {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 生产启动时启用注册表写入。测试不直接调用:会话身份可能来自任意信封,
    /// 让默认宿主直连真实数据目录会破坏测试隔离(测试改用
    /// [`Self::enable_session_registry_with`] 注入捕获器)。
    pub(crate) fn enable_session_registry(&mut self) {
        self.enable_session_registry_with(|source, session_id, file| {
            if let Err(error) =
                crate::ai_sessions::record_hook_session(source, session_id, file, None)
            {
                log::warn!("agent session index: could not record {source} {session_id}: {error}");
            }
        });
    }

    /// 注册表写入动作的注入点:写入任务始终经
    /// [`spawn_session_registry_write`] 落到后台线程,失败只由写入动作 warn。
    fn enable_session_registry_with(
        &mut self,
        write: impl Fn(&str, &str, Option<PathBuf>) + Send + Sync + 'static,
    ) {
        let write: SessionRegistryWrite = Arc::new(write);
        self.record_session_start = Some(Rc::new(move |event: &AiHookEvent| {
            let Some(id) = event.session_id.clone() else {
                return;
            };
            let source = event.source.clone();
            let file = event.session_file.as_deref().map(PathBuf::from);
            let write = Arc::clone(&write);
            // 写盘(锁 + fsync)不在前台线程上做;失败只 warn。
            spawn_session_registry_write(move || write(&source, &id, file));
        }));
    }

    /// 测试注入点:替换注册表写入通道(不触盘)。
    #[cfg(test)]
    pub(crate) fn set_session_start_recorder(&mut self, recorder: SessionStartRecorder) {
        self.record_session_start = Some(recorder);
    }

    pub(crate) fn register(&mut self, pane_id: PaneId) {
        self.hosts.insert(pane_id, AgentPaneHost::new(pane_id));
    }

    pub(crate) fn remove(&mut self, pane_id: PaneId) {
        self.hosts.remove(&pane_id);
    }

    /// 恢复注入:把保存的会话身份放回宿主。hook 到达后照常覆盖/清除。
    pub(crate) fn seed_session(&mut self, pane_id: PaneId, session: AgentSession) {
        if let Some(host) = self.hosts.get_mut(&pane_id) {
            host.session = Some(session);
        }
    }

    /// 快照用的会话身份:hook 精确身份优先;无 hook 的 pane 退到已识别
    /// 的前台程序名(仍可在 claude 上按目录 `--continue`)。
    pub(crate) fn session_agent(&self, pane_id: PaneId) -> Option<AgentSession> {
        let host = self.hosts.get(&pane_id)?;
        host.session().cloned().or_else(|| {
            host.program.as_ref().map(|source| AgentSession {
                source: source.clone(),
                session_id: None,
                session_file: None,
            })
        })
    }

    /// 按 pane 读取宿主（关闭守卫、托盘与测试）。
    pub(crate) fn host(&self, pane_id: PaneId) -> Option<&AgentPaneHost> {
        self.hosts.get(&pane_id)
    }

    /// 事件泵单批派发:同批重排后逐条按 pane 路由。返回被接纳(真正改写
    /// 状态)的事件数。
    ///
    /// 严格路由合同:信封带 pane 时它必须对应现存 pane,已关闭 pane 的
    /// 迟到 hook 直接丢弃;只有信封确实缺 pane 字段(环境变量半路被剥掉)
    /// 才回退到焦点 pane。
    #[allow(dead_code)] // 生产消费点 = dispatch_lifecycle;测试保留只取计数薄入口。
    pub(crate) fn dispatch(
        &mut self,
        events: Vec<AiHookEvent>,
        focus_fallback: Option<PaneId>,
    ) -> usize {
        self.dispatch_lifecycle(events, focus_fallback).0
    }

    /// 同 [`Self::dispatch`],额外返回**被接纳**的事件(按处理后顺序)。
    /// 生命周期接线只允许对通过所有权门的事件清理任务栏进度；
    /// 伪造 pane/session 的迟到信封不得影响现存 pane。
    pub(crate) fn dispatch_lifecycle(
        &mut self,
        events: Vec<AiHookEvent>,
        focus_fallback: Option<PaneId>,
    ) -> (usize, Vec<AiHookEvent>) {
        let mut accepted = 0;
        let mut accepted_events = Vec::new();
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
            let changed = host.handle_hook(&event);
            if changed {
                accepted += 1;
                accepted_events.push(event.clone());
            }
            // 只在事件真的被接纳后记账:被门拒收的伪造身份不得进注册表。
            if changed
                && event.kind == AiHookKind::SessionStart
                && let Some(recorder) = &self.record_session_start
            {
                recorder(&event);
            }
        }
        (accepted, accepted_events)
    }

    /// OSC 133;C 的命令启动边。
    pub(crate) fn command_started(&mut self, pane_id: PaneId) {
        if let Some(host) = self.hosts.get_mut(&pane_id) {
            host.command_started();
        }
    }

    /// OSC 133;D 边。
    pub(crate) fn command_finished(&mut self, pane_id: PaneId) {
        if let Some(host) = self.hosts.get_mut(&pane_id) {
            host.command_finished();
        }
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
    fn turn_done_updates_only_its_owning_pane() {
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
        assert_eq!(hub.host(PaneId(4)).unwrap().status(), AgentStatus::Unknown);
    }

    /// SessionStart 边写注册表:只有被接纳的事件才进写入通道;SessionEnd
    /// 清掉宿主里的恢复身份。
    #[test]
    fn session_start_edges_reach_the_registry_recorder() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut hub = AgentHub::new();
        hub.register(PaneId(7));
        let recorded = Rc::new(RefCell::new(Vec::new()));
        hub.set_session_start_recorder({
            let recorded = Rc::clone(&recorded);
            Rc::new(move |event: &AiHookEvent| {
                recorded
                    .borrow_mut()
                    .push(event.session_id.clone().unwrap_or_default());
            })
        });
        // 未注册 pane 的伪造信封被丢弃,不得进注册表。
        let spoofed = event(
            "claude",
            Some(9),
            r#"{"hook_event_name":"SessionStart","session_id":"spoofed"}"#,
        );
        assert_eq!(hub.dispatch(vec![spoofed], None), 0);
        assert!(recorded.borrow().is_empty());

        let start = event(
            "claude",
            Some(7),
            r#"{"hook_event_name":"SessionStart","session_id":"registered-1"}"#,
        );
        assert_eq!(hub.dispatch(vec![start], None), 1);
        assert_eq!(recorded.borrow().as_slice(), ["registered-1".to_owned()]);
        assert_eq!(
            hub.session_agent(PaneId(7))
                .and_then(|agent| agent.session_id),
            Some("registered-1".to_owned())
        );

        // 后续事件只带原生文件时,id 不被抹掉(半量更新按字段合并)。
        let mut file_only = prompt_submit("ignored", 7);
        file_only.session_id = None;
        file_only.session_file = Some("/sessions/rollout-2026-09-21.jsonl".into());
        assert_eq!(hub.dispatch(vec![file_only], None), 1);
        let merged = hub.session_agent(PaneId(7)).unwrap();
        assert_eq!(merged.session_id.as_deref(), Some("registered-1"));
        assert_eq!(
            merged.session_file.as_deref(),
            Some("/sessions/rollout-2026-09-21.jsonl")
        );

        let end = event(
            "claude",
            Some(7),
            r#"{"hook_event_name":"SessionEnd","session_id":"registered-1"}"#,
        );
        assert_eq!(hub.dispatch(vec![end], None), 1);
        assert!(hub.session_agent(PaneId(7)).is_none(), "会话结束后身份清空");
    }

    /// 防复发:SessionStart 的注册表写入必须离开调用方线程(锁 + fsync
    /// 不得占用 GPUI 前台)。注入 writer 经 channel 回传线程 id 与字段,
    /// 不触碰真实数据目录。
    #[test]
    fn session_start_registry_writes_on_a_background_thread() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut hub = AgentHub::new();
        hub.register(PaneId(9));
        let caller_thread = std::thread::current().id();
        let (sent_tx, sent_rx) = mpsc::channel();
        hub.enable_session_registry_with(move |source, session_id, file| {
            sent_tx
                .send((
                    std::thread::current().id(),
                    source.to_owned(),
                    session_id.to_owned(),
                    file,
                ))
                .expect("测试接收端存活");
        });

        let start = event(
            "claude",
            Some(9),
            r#"{"hook_event_name":"SessionStart","session_id":"bg-write","session_file":"D:/tmp/bg-write.jsonl"}"#,
        );
        assert_eq!(hub.dispatch(vec![start], None), 1);

        let (writer_thread, source, session_id, file) = sent_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("后台写入必须抵达注入 writer");
        assert_ne!(writer_thread, caller_thread, "写盘不得发生在调用方线程");
        assert_eq!(source, "claude");
        assert_eq!(session_id, "bg-write");
        assert_eq!(file, Some(PathBuf::from("D:/tmp/bg-write.jsonl")));
    }

    /// 恢复注入先入宿主:快照立刻能取到;无 hook 的 pane 退到程序名。
    #[test]
    fn seeded_restore_identity_is_visible_to_snapshots() {
        let mut hub = AgentHub::new();
        hub.register(PaneId(3));
        hub.seed_session(
            PaneId(3),
            AgentSession {
                source: "codex".into(),
                session_id: Some("thread-7".into()),
                session_file: None,
            },
        );
        assert_eq!(
            hub.session_agent(PaneId(3)).unwrap().session_id.as_deref(),
            Some("thread-7")
        );
        // 未登记会话的 pane:无身份。
        hub.register(PaneId(4));
        assert!(hub.session_agent(PaneId(4)).is_none());
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

        // 纯检测注入路径(不触规则引擎磁盘覆盖):仲裁器边与用例同源。
        assert!(host.observe_detection(Some(Detection {
            agent: AgentKind::Kimi,
            status: AgentStatus::Blocked,
            rule_id: "test.injected".into(),
        })));
        assert_eq!(host.status(), AgentStatus::Blocked);
    }
}
