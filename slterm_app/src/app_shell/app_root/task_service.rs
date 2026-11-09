//! AppRoot 任务所有权：任务句柄集中登记，owner 销毁时统一取消。

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::Task;

/// 当前运行根的任务身份；新增后台任务必须在同一枚举登记。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TaskId {
    ShellEvents,
    CloseFlush,
    TerminalClose(slterm_workspace::TerminalId),
    TerminalCloseBatch(uuid::Uuid),
    ProjectClose(slterm_workspace::ProjectId),
    ProjectOpen(uuid::Uuid),
    #[allow(dead_code)] // 消费点 = Document 后台真实打开队列。
    DocumentOpen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskError {
    Duplicate(TaskId),
}

/// 前台任务共享的显式取消状态；只在 owner 所在线程读写。
#[derive(Clone)]
pub(crate) struct CancellationToken(Rc<Cell<bool>>);

impl CancellationToken {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(false)))
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.get()
    }

    fn cancel(&self) {
        self.0.set(true);
    }
}

struct TaskEntry {
    cancellation: CancellationToken,
    _task: Task<()>,
}

#[allow(dead_code)] // 消费点归后续 Panel、SideView 后台读取与刷新。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ViewTaskOwner {
    Panel(slterm_workspace::PanelId),
    SideView(slterm_workspace::SideViewId),
}

/// 一次可见性复核的唯一代次；回执必须原样归还。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewRefreshTicket {
    owner: ViewTaskOwner,
    generation: u64,
}

impl ViewRefreshTicket {
    #[allow(dead_code)] // 消费点归后续可见性任务的代次日志与断言。
    pub(super) const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewRefreshOutcome {
    Applied,
    Stale,
}

#[allow(dead_code)] // 字段由后续生产回执点读取。
pub(super) struct ViewRefreshCompletion {
    pub(super) outcome: ViewRefreshOutcome,
    pub(super) next: Option<ViewRefreshTicket>,
}

struct ViewRefreshState {
    visible: bool,
    pending: bool,
    generation: u64,
    in_flight: Option<u64>,
}

impl Default for ViewRefreshState {
    fn default() -> Self {
        Self {
            visible: true,
            pending: false,
            generation: 0,
            in_flight: None,
        }
    }
}

impl ViewRefreshState {
    fn start(&mut self, owner: ViewTaskOwner) -> Option<ViewRefreshTicket> {
        if !self.visible || !self.pending || self.in_flight.is_some() {
            return None;
        }
        self.pending = false;
        self.generation = self
            .generation
            .checked_add(1)
            .expect("视图刷新代次不得回绕");
        self.in_flight = Some(self.generation);
        Some(ViewRefreshTicket {
            owner,
            generation: self.generation,
        })
    }
}

/// GPUI 任务句柄只由所属运行根持有，禁止 detach 形成游离循环。
pub(crate) struct TaskService {
    tasks: BTreeMap<TaskId, TaskEntry>,
    view_refreshes: BTreeMap<ViewTaskOwner, ViewRefreshState>,
}

impl TaskService {
    pub(crate) fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            view_refreshes: BTreeMap::new(),
        }
    }

    pub(crate) fn register(
        &mut self,
        id: TaskId,
        cancellation: CancellationToken,
        task: Task<()>,
    ) -> Result<(), TaskError> {
        self.register_with(id, cancellation, || task)
    }

    /// 只有身份可登记时才构造任务，避免重复注册丢弃已移交的资源所有权。
    pub(crate) fn register_with(
        &mut self,
        id: TaskId,
        cancellation: CancellationToken,
        task: impl FnOnce() -> Task<()>,
    ) -> Result<(), TaskError> {
        if self.tasks.contains_key(&id) {
            return Err(TaskError::Duplicate(id));
        }
        self.tasks.insert(
            id,
            TaskEntry {
                cancellation,
                _task: task(),
            },
        );
        Ok(())
    }

    pub(crate) fn cancel(&mut self, id: TaskId) {
        if let Some(entry) = self.tasks.remove(&id) {
            entry.cancellation.cancel();
        }
    }

    pub(crate) fn complete(&mut self, id: TaskId) {
        self.tasks.remove(&id);
    }

    #[allow(dead_code)] // 消费点 = DocumentOpen 单任务所有权闸门。
    pub(super) fn is_registered(&self, id: TaskId) -> bool {
        self.tasks.contains_key(&id)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn project_open_task_count(&self) -> usize {
        self.tasks
            .keys()
            .filter(|id| matches!(id, TaskId::ProjectOpen(_)))
            .count()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn task_identity(&self, id: TaskId) -> Option<usize> {
        self.tasks
            .get(&id)
            .map(|entry| Rc::as_ptr(&entry.cancellation.0) as usize)
    }

    /// 可见时至多一个在途复核；其余请求只保留合并位。
    #[allow(dead_code)] // 生产调度由后续 Explorer、Diff 与历史视图接入。
    pub(super) fn request_view_refresh(
        &mut self,
        owner: ViewTaskOwner,
    ) -> Option<ViewRefreshTicket> {
        let state = self.view_refreshes.entry(owner).or_default();
        state.pending = true;
        state.start(owner)
    }

    /// 隐藏立即使旧代次失效；重新可见只启动一次合并复核。
    #[allow(dead_code)] // 生产调度由后续 Explorer、Diff 与历史视图接入。
    pub(super) fn set_view_visible(
        &mut self,
        owner: ViewTaskOwner,
        visible: bool,
    ) -> Option<ViewRefreshTicket> {
        let state = self.view_refreshes.entry(owner).or_default();
        if state.visible == visible {
            return state.start(owner);
        }
        state.visible = visible;
        state.pending = true;
        if !visible {
            state.in_flight = None;
        }
        state.start(owner)
    }

    /// 只有当前代次可结算；旧回执不得清除新任务或触发额外刷新。
    #[allow(dead_code)] // 生产调度由后续 Explorer、Diff 与历史视图接入。
    pub(super) fn finish_view_refresh(
        &mut self,
        ticket: ViewRefreshTicket,
    ) -> ViewRefreshCompletion {
        let outcome = self
            .view_refreshes
            .get(&ticket.owner)
            .and_then(|state| state.in_flight)
            .map_or(ViewRefreshOutcome::Stale, |generation| {
                if generation == ticket.generation {
                    ViewRefreshOutcome::Applied
                } else {
                    ViewRefreshOutcome::Stale
                }
            });
        if outcome == ViewRefreshOutcome::Stale {
            return ViewRefreshCompletion {
                outcome,
                next: None,
            };
        }
        let state = self
            .view_refreshes
            .get_mut(&ticket.owner)
            .expect("当前刷新回执必须有所有者状态");
        state.in_flight = None;
        let next = state.start(ticket.owner);
        ViewRefreshCompletion { outcome, next }
    }

    pub(crate) fn shutdown(&mut self) {
        for entry in self.tasks.values() {
            entry.cancellation.cancel();
        }
        self.tasks.clear();
        self.view_refreshes.clear();
    }
}

impl Drop for TaskService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn duplicate_terminal_close_registration_does_not_construct_replacement_task(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let mut service = TaskService::new();
            let terminal = slterm_workspace::TerminalId::from_uuid(uuid::Uuid::now_v7()).unwrap();
            let id = TaskId::TerminalClose(terminal);
            let first = cx.spawn(async move |_| std::future::pending::<()>().await);
            service
                .register(id, CancellationToken::new(), first)
                .unwrap();
            let constructed = Rc::new(Cell::new(false));
            let constructed_by_factory = Rc::clone(&constructed);
            let duplicate = service.register_with(id, CancellationToken::new(), || {
                constructed_by_factory.set(true);
                cx.spawn(async move |_| std::future::pending::<()>().await)
            });

            assert_eq!(duplicate, Err(TaskError::Duplicate(id)));
            assert!(!constructed.get(), "重复注册不得消费替代任务或资源");
            assert!(service.is_registered(id));
        });
    }
}
