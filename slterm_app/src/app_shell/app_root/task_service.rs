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

/// GPUI 任务句柄只由所属运行根持有，禁止 detach 形成游离循环。
pub(crate) struct TaskService {
    tasks: BTreeMap<TaskId, TaskEntry>,
}

impl TaskService {
    pub(crate) fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
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

    #[cfg(all(test, feature = "gpui-test-support"))]
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

    pub(crate) fn shutdown(&mut self) {
        for entry in self.tasks.values() {
            entry.cancellation.cancel();
        }
        self.tasks.clear();
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
