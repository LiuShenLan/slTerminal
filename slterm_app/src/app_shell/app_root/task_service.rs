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
        if self.tasks.contains_key(&id) {
            return Err(TaskError::Duplicate(id));
        }
        self.tasks.insert(
            id,
            TaskEntry {
                cancellation,
                _task: task,
            },
        );
        Ok(())
    }

    pub(crate) fn complete(&mut self, id: TaskId) {
        self.tasks.remove(&id);
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
    fn duplicate_task_registration_is_rejected(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut service = TaskService::new();
            let first = cx.spawn(async move |_| std::future::pending::<()>().await);
            let second = cx.spawn(async move |_| std::future::pending::<()>().await);
            assert!(
                service
                    .register(TaskId::ShellEvents, CancellationToken::new(), first)
                    .is_ok()
            );
            assert_eq!(
                service.register(TaskId::ShellEvents, CancellationToken::new(), second),
                Err(TaskError::Duplicate(TaskId::ShellEvents))
            );
        });
    }
}
