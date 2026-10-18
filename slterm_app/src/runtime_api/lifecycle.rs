//! managed agent 生命周期入口(04 篇 M7.5 壳侧接线)。
//!
//! pane 关闭已由 `record_pane_closed/exited` 统一收口;本模块补 SessionEnd
//! 边:provider 会话结束时 pane 还在,注册表必须随事件关闭,`agent.wait`
//! 才能立刻拿到 `agent_exited` 而不是等到超时。

use super::{RuntimeHub, RuntimeManagedAgent};

impl RuntimeHub {
    /// 关闭某 pane 上所有 active managed agent(SessionEnd / pane 生命周期)。
    /// 注册表纯变化也重发规范快照,唤醒共用通道的等待方。
    pub(crate) fn close_agents_for_pane(&self, window_id: u64, pane_id: u64, reason: &str) {
        let mut state = self.lock();
        let mut changed = false;
        for agent in state.managed_agents.values_mut().filter(|agent| {
            agent.active && agent.window_id == window_id && agent.pane_id == pane_id
        }) {
            agent.active = false;
            agent.closed_reason = Some(reason.to_owned());
            changed = true;
        }
        if changed && let Some(snapshot) = state.current.clone() {
            super::notify_snapshot_subscribers(&mut state, &snapshot);
        }
    }

    /// 某 pane 当前 active managed agent(委派回传定位;身份未观测时为 None)。
    pub(crate) fn active_agent_in_pane(
        &self,
        window_id: u64,
        pane_id: u64,
    ) -> Option<RuntimeManagedAgent> {
        self.lock()
            .managed_agents
            .values()
            .find(|agent| agent.active && agent.window_id == window_id && agent.pane_id == pane_id)
            .cloned()
    }
}
