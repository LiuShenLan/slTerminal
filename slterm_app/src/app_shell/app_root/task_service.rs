//! AppRoot 任务所有权：任务句柄集中登记，owner 销毁时统一取消。

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::Task;

/// 当前运行根的任务身份；新增后台任务必须在同一枚举登记。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TaskId {
    ShellEvents,
    /// 设置去抖批量的后台写盘；前台只提交快照与结算回执。
    SettingsSave,
    /// 普通设置导出：后台选择目标并原子写版本化信封。
    SettingsExport,
    /// 普通设置导入：后台选择与预检、确认后写盘。
    SettingsImport,
    /// 加密备份导出：后台收集、密封、目标选择与原子写。
    BackupExport,
    /// 更新安装包下载：后台流式下载与校验。
    UpdateDownload,
    /// 设置页手动检查更新：后台联网，前台只结算结果。
    UpdateCheck,
    /// 读取失败后的显式重试：后台重读设置文件并重建快照。
    SettingsLoad,
    /// 自定义主题定义的冷路径预载；热应用只查缓存。
    ThemeLoad,
    CloseFlush,
    TerminalClose(slterm_workspace::TerminalId),
    TerminalReclaim(crate::gpui_terminal::registry::TerminalTarget),
    TerminalCloseBatch(uuid::Uuid),
    ProjectClose(slterm_workspace::ProjectId),
    ProjectOpen(uuid::Uuid),
    /// 项目根重定位：后台目录选择、真实解析与前台事务回执同属一份所有权。
    ProjectRelocation(slterm_workspace::ProjectId),
    /// 导航历史扫描：后台读取 CLI 原生档案，前台只接收归属结果。
    NavHistoryScan,
    /// 导航历史恢复：后台复核真实 cwd 与项目根后回前台建终端。
    NavHistoryRestore,
    #[allow(dead_code)] // 消费点 = Document 后台真实打开队列。
    DocumentOpen,
    DocumentExternal(slterm_document::DocumentId),
    /// 根外 Document 的显式重授权后台读盘；同一 Document 同时最多一个。
    DocumentReauthorize(slterm_document::DocumentId),
    DocumentSave(slterm_document::DocumentId),
    /// 另存为的原生目标选择；选定后才进入保存写盘链。
    DocumentSaveAsPick(slterm_document::DocumentId),
    /// 冲突决定面的「重载磁盘」：后台重读真实文件后强制替换本地草稿。
    DocumentConflictReload(slterm_document::DocumentId),
    DocumentSystemOpen(slterm_document::DocumentId),
    EditorGit(slterm_workspace::PanelId),
    GitShowLoad(slterm_workspace::PanelId),
    GitShowSystemOpen(slterm_workspace::PanelId),
    ExplorerTerminalCwd,
    ExplorerFileOperation,
    ExplorerStatus,
    CommitStatus,
    CommitDiff,
    CommitGitShow,
    CommitRollback,
    CommitDelete,
    AgentRootsLoad,
    /// 设置页 AgentHooks 页的安装状态后台探测。
    AgentHooksInspect,
    /// 单个 agent 的 hook 安装/卸载后台执行。
    AgentHookApply(slterm_settings::AgentHook),
    /// Hooks 编辑器的磁盘载入与保存；身份含面板、CLI 与层级。
    HooksDraftLoad(crate::ai_hook::draft_host::HooksDraftKey),
    HooksDraftSave(crate::ai_hook::draft_host::HooksDraftKey),
    #[allow(dead_code)] // 消费点归 Diff 固定 HEAD 后台读取。
    DiffHead(slterm_workspace::PanelId),
    MarkdownLink(uuid::Uuid),
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
    pub(super) fn view_refresh_generation_for_test(&self, owner: ViewTaskOwner) -> Option<u64> {
        self.view_refreshes
            .get(&owner)
            .map(|state| state.generation)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn current_view_refresh_ticket_for_test(
        &self,
        owner: ViewTaskOwner,
    ) -> Option<ViewRefreshTicket> {
        self.view_refreshes
            .get(&owner)
            .and_then(|state| state.in_flight)
            .map(|generation| ViewRefreshTicket { owner, generation })
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

    /// Panel 销毁后使全部迟到票据失效，不保留无所有者的刷新状态。
    pub(super) fn remove_view_refresh(&mut self, owner: ViewTaskOwner) {
        self.view_refreshes.remove(&owner);
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

    /// Hooks 草稿任务身份必须含完整草稿键：同一面板的不同层级可并行载入／保存，
    /// 否则切层载入会被误判为重复任务而静默丢弃。
    #[gpui::test]
    fn hooks_draft_task_identity_includes_layer_and_agent(cx: &mut TestAppContext) {
        use crate::ai_hook::draft_host::{HooksDraftKey, HooksDraftLayer, HooksDraftOwner};
        use slterm_agents::AgentKind;
        use slterm_workspace::{PanelIdAllocator, ProjectId};

        cx.update(|cx| {
            let mut service = TaskService::new();
            let panel = PanelIdAllocator::new(1).allocate().unwrap();
            let project = ProjectId::from_uuid(uuid::Uuid::now_v7()).unwrap();
            let user_key = HooksDraftKey {
                owner: HooksDraftOwner::Panel(panel),
                agent: AgentKind::Claude,
                layer: HooksDraftLayer::User,
            };
            let project_key = HooksDraftKey {
                owner: HooksDraftOwner::Panel(panel),
                agent: AgentKind::Claude,
                layer: HooksDraftLayer::Project(project),
            };
            for key in [user_key, project_key] {
                service
                    .register(
                        TaskId::HooksDraftLoad(key),
                        CancellationToken::new(),
                        cx.spawn(async move |_| std::future::pending::<()>().await),
                    )
                    .unwrap();
            }
            assert!(service.is_registered(TaskId::HooksDraftLoad(user_key)));
            assert!(service.is_registered(TaskId::HooksDraftLoad(project_key)));
            service.complete(TaskId::HooksDraftLoad(user_key));
            assert!(
                service.is_registered(TaskId::HooksDraftLoad(project_key)),
                "结算一层不得移除另一层的在途任务"
            );
        });
    }
}
