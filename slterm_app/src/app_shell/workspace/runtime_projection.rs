//! Workspace → RuntimeSnapshot 投影(04 篇 M7.2,05 篇共建)。
//!
//! 纯读取:不持有第二份状态、不改 workspace。`RuntimeHub::publish` 负责
//! 盖戳与判等;本模块产出的 `state_change_seq` 恒 0,`active_run`/`last_run`
//! 待 02 篇 OSC 133 记账链(M9)回填。
//!
//! 不变式:snapshot pane 集合 == Terminal tab 树叶集合;`PaneId.0` 直出协议;
//! Settings tab 无 pane/layout(单例、不进会话快照)。

use gpui::App;
use slterm_split::{PaneId, SplitDirection, SplitTree};

use super::{SltermWorkspace, WorkspaceTab};
use crate::agent_host::AgentPaneHost;
use crate::ai_agents::{AgentKind, AgentStatus, AgentStatusSource};
use crate::runtime_api::{
    RuntimeAgent, RuntimeAgentStateSource, RuntimeLayout, RuntimePane, RuntimeSnapshot,
    RuntimeSplitDirection, RuntimeTab, RuntimeTaskState, RuntimeWindow, SINGLE_WINDOW_ID,
};

/// 当前工作区 → 单窗口快照草稿(revision/状态计数由 hub 盖戳)。
pub(crate) fn project_workspace(workspace: &SltermWorkspace, cx: &App) -> RuntimeSnapshot {
    let focused_pane_id = workspace.focused_pane().map(|id| id.0);
    let tabs = workspace
        .tabs
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let active = index == workspace.active;
            match &entry.tab {
                WorkspaceTab::Terminal {
                    panes,
                    tree,
                    focused,
                    zoomed,
                } => RuntimeTab {
                    tab_id: Some(entry.id),
                    index,
                    active,
                    label: workspace.tab_title(index, cx),
                    kind: "terminal".to_owned(),
                    bell: false,
                    focused_pane_id: Some(focused.0),
                    zoomed_pane_id: zoomed.then_some(focused.0),
                    layout: Some(runtime_layout(tree)),
                    panes: panes
                        .iter()
                        .map(|pane| {
                            let view = pane.view.read(cx);
                            let host = workspace.agent_hub.host(pane.id);
                            RuntimePane {
                                id: pane.id.0,
                                active: pane.id == *focused,
                                title: view.title.clone(),
                                cwd: super::session_recovery::pane_cwd(pane, cx),
                                branch: view.branch.clone(),
                                running_program: view.running_program.clone(),
                                agent: host.and_then(runtime_agent),
                                // 六态唯一投影:03 篇仲裁器的首版桥(见映射函数)。
                                task_state: host.map_or(RuntimeTaskState::Idle, |host| {
                                    runtime_task_state(host.status())
                                }),
                                state_change_seq: 0,
                                active_run: None,
                                last_run: None,
                            }
                        })
                        .collect(),
                },
                WorkspaceTab::Settings { view, .. } => RuntimeTab {
                    tab_id: Some(entry.id),
                    index,
                    active,
                    label: view.read(cx).title(cx).to_string(),
                    kind: "settings".to_owned(),
                    bell: false,
                    focused_pane_id: None,
                    zoomed_pane_id: None,
                    layout: None,
                    panes: Vec::new(),
                },
            }
        })
        .collect();
    RuntimeSnapshot::new(vec![RuntimeWindow {
        id: SINGLE_WINDOW_ID,
        focused: true,
        active_tab: workspace.active,
        focused_pane_id,
        tabs,
    }])
}

/// 运行时分屏树 → 协议布局(比例原样,不做二次钳制)。
fn runtime_layout(tree: &SplitTree<PaneId>) -> RuntimeLayout {
    match tree {
        SplitTree::Leaf(id) => RuntimeLayout::Pane { pane_id: id.0 },
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
            ..
        } => RuntimeLayout::Split {
            direction: match direction {
                SplitDirection::LeftRight => RuntimeSplitDirection::LeftRight,
                SplitDirection::TopBottom => RuntimeSplitDirection::TopBottom,
            },
            ratio: *ratio,
            first: Box::new(runtime_layout(first)),
            second: Box::new(runtime_layout(second)),
        },
    }
}

/// 03 篇 `AgentStatus` → 协议六态。03 无边界的等待输入/失败细分:
/// Blocked(权限/提问屏幕)归 attention;Failed 只由 run 记账链产生。
fn runtime_task_state(status: AgentStatus) -> RuntimeTaskState {
    match status {
        AgentStatus::Idle => RuntimeTaskState::Idle,
        AgentStatus::Working => RuntimeTaskState::Running,
        AgentStatus::Blocked => RuntimeTaskState::Attention,
        AgentStatus::Done => RuntimeTaskState::Finished,
        // 普通 shell / 未识别:没有 agent 语义,按空闲呈现。
        AgentStatus::Unknown => RuntimeTaskState::Idle,
    }
}

/// 识别为 AI Agent 的 pane 才有身份证据;进程名认不出或状态来源未定
/// (`Unknown`)时不虚构 agent。
fn runtime_agent(host: &AgentPaneHost) -> Option<RuntimeAgent> {
    let kind = host.program().and_then(AgentKind::parse)?;
    let state_source = runtime_state_source(host.source())?;
    Some(RuntimeAgent {
        agent_id: None,
        generation: None,
        name: None,
        kind: kind.slug().to_owned(),
        display_name: kind.display_name().to_owned(),
        session_id: host
            .session()
            .and_then(|session| session.session_id.clone()),
        state_source,
        state_rule: host.rule().map(str::to_owned),
        hook_seen: host.hook_seen(),
    })
}

fn runtime_state_source(source: AgentStatusSource) -> Option<RuntimeAgentStateSource> {
    match source {
        AgentStatusSource::Hook => Some(RuntimeAgentStateSource::Hook),
        AgentStatusSource::Screen => Some(RuntimeAgentStateSource::Screen),
        AgentStatusSource::Process => Some(RuntimeAgentStateSource::Process),
        AgentStatusSource::Unknown => None,
    }
}

#[cfg(test)]
mod runtime_projection_tests {
    use super::*;
    use crate::ai_hook::parse_envelope;

    fn hook_event(pane: u64, payload: &str) -> crate::ai_hook::AiHookEvent {
        parse_envelope(format!("slterm-hook/1 source=claude pane={pane}\n{payload}").as_bytes())
            .expect("测试信封应可解析")
    }

    #[test]
    fn runtime_layout_preserves_tree_shape_and_direction() {
        let mut tree = SplitTree::leaf(PaneId(1));
        assert!(tree.split_leaf(PaneId(1), PaneId(2), SplitDirection::LeftRight, 0.6));
        assert!(tree.split_leaf(PaneId(2), PaneId(3), SplitDirection::TopBottom, 0.4));
        let layout = runtime_layout(&tree);
        let RuntimeLayout::Split {
            direction,
            ratio,
            first,
            second,
        } = layout
        else {
            panic!("根应是 Split");
        };
        assert_eq!(direction, RuntimeSplitDirection::LeftRight);
        assert!((ratio - 0.6).abs() < f32::EPSILON);
        assert!(matches!(*first, RuntimeLayout::Pane { pane_id: 1 }));
        let RuntimeLayout::Split { direction, .. } = *second else {
            panic!("右子树应是 Split");
        };
        assert_eq!(direction, RuntimeSplitDirection::TopBottom);
    }

    #[test]
    fn runtime_task_state_maps_agent_statuses_to_the_six_state_projection() {
        assert_eq!(
            runtime_task_state(AgentStatus::Idle),
            RuntimeTaskState::Idle
        );
        assert_eq!(
            runtime_task_state(AgentStatus::Working),
            RuntimeTaskState::Running
        );
        assert_eq!(
            runtime_task_state(AgentStatus::Blocked),
            RuntimeTaskState::Attention
        );
        assert_eq!(
            runtime_task_state(AgentStatus::Done),
            RuntimeTaskState::Finished
        );
        assert_eq!(
            runtime_task_state(AgentStatus::Unknown),
            RuntimeTaskState::Idle
        );
    }

    #[test]
    fn runtime_agent_is_projected_only_with_hook_authority() {
        let mut host = AgentPaneHost::new(PaneId(3));
        assert!(runtime_agent(&host).is_none(), "无身份不虚构 agent");
        let event = hook_event(
            3,
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"session-3"}"#,
        );
        assert!(host.handle_hook(&event));
        let agent = runtime_agent(&host).expect("hook 权威应产出 agent");
        assert_eq!(agent.kind, "claude");
        assert_eq!(agent.display_name, "Claude Code");
        assert_eq!(agent.session_id.as_deref(), Some("session-3"));
        assert_eq!(agent.state_source, RuntimeAgentStateSource::Hook);
        assert!(agent.hook_seen);
    }
}
