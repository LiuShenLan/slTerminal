//! 生产 SideView 实体与私有持久状态。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, Render,
    Styled as _, Window, div,
};
use slterm_sidebar::{SideViewState, side_view_state_schema};
use slterm_workspace::{SideViewId, ViewState, ViewStateError, ViewStateRegistry};

use super::side_view_host::SideViewControl;
use crate::persistence::workspace_writer::WorkspaceWriterClient;

/// 关闭重开共享的内存恢复事实；启动时由 SQLite 在 GPUI 外预载。
pub(crate) type SideViewStateCache = Rc<RefCell<BTreeMap<SideViewId, SideViewState>>>;
/// 启动恢复未知的视图禁止自动回写；明确编辑在同一前台共享事实中解除屏障。
pub(crate) type SideViewWriteBarriers = Rc<RefCell<BTreeSet<SideViewId>>>;
/// 普通队列拒收的关闭状态；只由持久化 owner 在退出时经终态队列补交。
type SideViewPendingFinalStates = Rc<RefCell<BTreeMap<SideViewId, ViewState>>>;

/// AppRoot 持有的侧栏持久化依赖；各实体仍独占自己的强类型状态。
pub(crate) struct SideViewPersistence {
    cache: SideViewStateCache,
    registry: Arc<ViewStateRegistry>,
    writer: Option<WorkspaceWriterClient>,
    write_barriers: SideViewWriteBarriers,
    pending_final_states: SideViewPendingFinalStates,
}

impl SideViewPersistence {
    pub(crate) fn load(
        paths: &slterm_path::AppPaths,
        writer: Option<WorkspaceWriterClient>,
    ) -> Self {
        let persistence = Self::empty(writer);
        let store = match crate::persistence::workspace::WorkspaceDatabase::open(paths) {
            Ok(store) => store,
            Err(error) => {
                log::warn!("could not open SideView ViewState recovery: {error}");
                persistence
                    .write_barriers
                    .borrow_mut()
                    .extend(SideViewId::ALL);
                return persistence;
            }
        };
        for id in SideViewId::ALL {
            let key = slterm_workspace::ViewKey::SideView { side_view_id: id };
            match store.view_state(&key, &persistence.registry) {
                Ok(Some(stored)) => match SideViewState::from_view_state(id, stored.state) {
                    Ok(state) => {
                        persistence.cache.borrow_mut().insert(id, state);
                    }
                    Err(error) => {
                        log::warn!("could not decode SideView ViewState: {error:?}");
                        persistence.write_barriers.borrow_mut().insert(id);
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    log::warn!("could not restore SideView ViewState: {error}");
                    persistence.write_barriers.borrow_mut().insert(id);
                }
            }
        }
        persistence
    }

    pub(crate) fn empty(writer: Option<WorkspaceWriterClient>) -> Self {
        Self {
            cache: Rc::default(),
            registry: side_view_state_registry(),
            writer,
            write_barriers: Rc::default(),
            pending_final_states: Rc::default(),
        }
    }

    pub(super) fn controller<'a>(&self, cx: &'a mut App) -> SideViewController<'a> {
        SideViewController::new(
            cx,
            Rc::clone(&self.cache),
            Arc::clone(&self.registry),
            self.writer.clone(),
            Rc::clone(&self.write_barriers),
            Rc::clone(&self.pending_final_states),
        )
    }
}

impl Drop for SideViewPersistence {
    fn drop(&mut self) {
        let Some(writer) = &self.writer else {
            return;
        };
        for (id, state) in self.pending_final_states.borrow().iter() {
            let key = slterm_workspace::ViewKey::SideView { side_view_id: *id };
            if let Err(error) = writer.save_final_view_state(&key, state) {
                log::warn!("could not queue deferred final SideView ViewState: {error}");
            }
        }
    }
}

/// 应用级侧栏视图实体，独占自己的强类型持久状态。
pub(crate) struct SideView {
    state: SideViewState,
    cache: SideViewStateCache,
    registry: Arc<ViewStateRegistry>,
    writer: Option<WorkspaceWriterClient>,
    write_barriers: SideViewWriteBarriers,
    pending_final_states: SideViewPendingFinalStates,
    /// 同步关闭后禁止旧实体延迟析构再次回写。
    closed: bool,
}

impl SideView {
    pub(crate) fn with_persistence(
        id: SideViewId,
        cache: SideViewStateCache,
        registry: Arc<ViewStateRegistry>,
        writer: Option<WorkspaceWriterClient>,
        write_barriers: SideViewWriteBarriers,
        pending_final_states: SideViewPendingFinalStates,
    ) -> Self {
        let state = cache
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_else(|| SideViewState::new(id));
        Self {
            state,
            cache,
            registry,
            writer,
            write_barriers,
            pending_final_states,
            closed: false,
        }
    }

    /// 当前强类型状态经所属 schema 编码，测试与析构保存共用同一出口。
    pub(crate) fn view_state(&self) -> Result<ViewState, ViewStateError> {
        self.state.to_view_state(&self.registry)
    }

    /// 每个实体只提交一次最终状态，同步关闭与退出析构共用此出口。
    fn checkpoint(&mut self) -> Option<ViewState> {
        if self.closed {
            return None;
        }
        self.closed = true;
        let id = self.state.id();
        self.cache.borrow_mut().insert(id, self.state.clone());
        if self.write_barriers.borrow().contains(&id) {
            return None;
        }
        let view_state = match self.view_state() {
            Ok(state) => state,
            Err(error) => {
                log::warn!("could not encode SideView ViewState: {error}");
                return None;
            }
        };
        if let Some(writer) = &self.writer {
            let key = slterm_workspace::ViewKey::SideView { side_view_id: id };
            match writer.save_view_state(&key, &view_state) {
                Ok(()) => {
                    // 新一代次已进入普通队列，旧拒收副本不得在退出时倒灌。
                    self.pending_final_states.borrow_mut().remove(&id);
                }
                Err(error) => {
                    self.pending_final_states
                        .borrow_mut()
                        .insert(id, view_state.clone());
                    log::warn!("could not queue SideView ViewState: {error}");
                }
            }
        }
        Some(view_state)
    }
}

/// 将 GPUI 实体生命周期与持久化依赖交给 Host 的同一控制入口。
pub(crate) struct SideViewController<'a> {
    cx: &'a mut App,
    cache: SideViewStateCache,
    registry: Arc<ViewStateRegistry>,
    writer: Option<WorkspaceWriterClient>,
    write_barriers: SideViewWriteBarriers,
    pending_final_states: SideViewPendingFinalStates,
}

impl<'a> SideViewController<'a> {
    pub(crate) fn new(
        cx: &'a mut App,
        cache: SideViewStateCache,
        registry: Arc<ViewStateRegistry>,
        writer: Option<WorkspaceWriterClient>,
        write_barriers: SideViewWriteBarriers,
        pending_final_states: SideViewPendingFinalStates,
    ) -> Self {
        Self {
            cx,
            cache,
            registry,
            writer,
            write_barriers,
            pending_final_states,
        }
    }
}

impl SideViewControl<Entity<SideView>> for SideViewController<'_> {
    fn open(&mut self, id: SideViewId) -> Entity<SideView> {
        let cache = Rc::clone(&self.cache);
        let registry = Arc::clone(&self.registry);
        let writer = self.writer.clone();
        let write_barriers = Rc::clone(&self.write_barriers);
        let pending_final_states = Rc::clone(&self.pending_final_states);
        self.cx.new(|_| {
            SideView::with_persistence(
                id,
                cache,
                registry,
                writer,
                write_barriers,
                pending_final_states,
            )
        })
    }

    fn close(&mut self, _id: SideViewId, instance: Entity<SideView>) {
        instance.update(self.cx, |view, _| view.checkpoint());
    }
}

/// SideView schema 的唯一应用注册点。
pub(crate) fn side_view_state_registry() -> Arc<ViewStateRegistry> {
    Arc::new(
        ViewStateRegistry::new(&[side_view_state_schema()])
            .expect("SideView schema 封闭注册不得重复"),
    )
}

impl Render for SideView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let id = self.state.id();
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .debug_selector(move || format!("app-root-side-view-{}", id.id_str()))
    }
}

impl Drop for SideView {
    fn drop(&mut self) {
        let Some(view_state) = self.checkpoint() else {
            return;
        };
        if let Some(writer) = &self.writer
            && let Err(error) = writer.save_final_view_state(
                &slterm_workspace::ViewKey::SideView {
                    side_view_id: self.state.id(),
                },
                &view_state,
            )
        {
            log::warn!("could not queue final SideView ViewState: {error}");
        }
    }
}

#[cfg(test)]
mod side_view_state_persistence_tests {
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_workspace::{ViewKey, ViewState};

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriter;

    fn paths(temp: &tempfile::TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn persisted_nav(registry: &ViewStateRegistry) -> ViewState {
        ViewState::new(
            &ViewKey::SideView {
                side_view_id: SideViewId::Nav,
            },
            json!({"projects": [], "unassigned_expanded": true, "scroll": 42.0}),
            registry,
        )
        .unwrap()
    }

    #[test]
    fn side_view_load_restores_existing_private_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let persisted = persisted_nav(&registry);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .save_view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &persisted,
            )
            .unwrap();
        drop(store);

        let persistence = SideViewPersistence::load(&paths, None);
        let restored = persistence.cache.borrow()[&SideViewId::Nav]
            .to_view_state(&registry)
            .unwrap();
        assert_eq!(restored, persisted);
        assert!(
            !persistence
                .write_barriers
                .borrow()
                .contains(&SideViewId::Nav)
        );
    }

    #[test]
    fn side_view_load_marks_corrupt_state_write_barrier() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let persisted = persisted_nav(&registry);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .save_view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &persisted,
            )
            .unwrap();
        drop(store);
        let connection = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        connection
            .execute("UPDATE view_states SET state_json = 'corrupt'", [])
            .unwrap();
        drop(connection);

        let persistence = SideViewPersistence::load(&paths, None);
        assert!(!persistence.cache.borrow().contains_key(&SideViewId::Nav));
        assert!(
            persistence
                .write_barriers
                .borrow()
                .contains(&SideViewId::Nav)
        );
    }

    /// 同步关闭被普通队列拒收后，持久化 owner 仍须在退出时提交该代次的最终状态。
    #[test]
    fn side_view_queue_full_close_is_submitted_at_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let state = |scroll| {
            ViewState::new(
                &key,
                json!({"projects": [], "unassigned_expanded": true, "scroll": scroll}),
                &registry,
            )
            .unwrap()
        };
        let writer = WorkspaceWriter::start(paths.clone()).unwrap();
        let client = writer.client();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
        client.save_view_state(&key, &state(1.0)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
        for scroll in 2..66 {
            client
                .save_view_state(&key, &state(f64::from(scroll)))
                .unwrap();
        }

        let persistence = SideViewPersistence::empty(Some(client));
        let mut view = SideView::with_persistence(
            SideViewId::Nav,
            Rc::clone(&persistence.cache),
            Arc::clone(&persistence.registry),
            persistence.writer.clone(),
            Rc::clone(&persistence.write_barriers),
            Rc::clone(&persistence.pending_final_states),
        );
        view.state = SideViewState::from_view_state(SideViewId::Nav, state(99.0)).unwrap();
        view.checkpoint();
        drop(view);

        fault.execute_batch("ROLLBACK").unwrap();
        drop(persistence);
        writer.shutdown().unwrap();
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let stored = store.view_state(&key, &registry).unwrap().unwrap();
        assert_eq!(stored.state.state()["scroll"], json!(99.0));
    }

    /// 拒收后的新代次一旦成功提交，退出时不得再补写旧关闭快照。
    #[test]
    fn side_view_successful_reopen_supersedes_deferred_final_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let state = |scroll| {
            ViewState::new(
                &key,
                json!({"projects": [], "unassigned_expanded": true, "scroll": scroll}),
                &registry,
            )
            .unwrap()
        };
        let writer = WorkspaceWriter::start(paths.clone()).unwrap();
        let client = writer.client();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
        client.save_view_state(&key, &state(1.0)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
        for scroll in 2..66 {
            client
                .save_view_state(&key, &state(f64::from(scroll)))
                .unwrap();
        }

        let persistence = SideViewPersistence::empty(Some(client));
        let mut first = SideView::with_persistence(
            SideViewId::Nav,
            Rc::clone(&persistence.cache),
            Arc::clone(&persistence.registry),
            persistence.writer.clone(),
            Rc::clone(&persistence.write_barriers),
            Rc::clone(&persistence.pending_final_states),
        );
        first.state = SideViewState::from_view_state(SideViewId::Nav, state(99.0)).unwrap();
        first.checkpoint();
        drop(first);
        fault.execute_batch("ROLLBACK").unwrap();
        writer.flush().unwrap();

        let mut reopened = SideView::with_persistence(
            SideViewId::Nav,
            Rc::clone(&persistence.cache),
            Arc::clone(&persistence.registry),
            persistence.writer.clone(),
            Rc::clone(&persistence.write_barriers),
            Rc::clone(&persistence.pending_final_states),
        );
        reopened.state = SideViewState::from_view_state(SideViewId::Nav, state(123.0)).unwrap();
        reopened.checkpoint();
        drop(reopened);
        drop(persistence);
        writer.shutdown().unwrap();

        let store = WorkspaceDatabase::open(&paths).unwrap();
        let stored = store.view_state(&key, &registry).unwrap().unwrap();
        assert_eq!(stored.state.state()["scroll"], json!(123.0));
    }

    /// 析构保存与关闭重开共用强类型状态；writer 不可用仍保留内存恢复事实。
    #[test]
    fn side_view_drop_saves_and_reopen_restores_persistent_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let persisted = persisted_nav(&registry);
        let cache = Rc::new(RefCell::new(BTreeMap::from([(
            SideViewId::Nav,
            SideViewState::from_view_state(SideViewId::Nav, persisted.clone()).unwrap(),
        )])));
        let writer = WorkspaceWriter::start(paths.clone()).unwrap();

        let mut first = SideView::with_persistence(
            SideViewId::Nav,
            Rc::clone(&cache),
            Arc::clone(&registry),
            Some(writer.client()),
            Default::default(),
            Default::default(),
        );
        assert_eq!(first.view_state().unwrap(), persisted);
        let updated = ViewState::new(
            &ViewKey::SideView {
                side_view_id: SideViewId::Nav,
            },
            json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
            &registry,
        )
        .unwrap();
        first.state = SideViewState::from_view_state(SideViewId::Nav, updated.clone()).unwrap();
        drop(first);
        writer.flush().unwrap();

        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let stored = observer
            .view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &registry,
            )
            .unwrap()
            .expect("实体析构必须经 writer 保存 ViewState");
        assert_eq!(stored.state, updated);

        let reopened = SideView::with_persistence(
            SideViewId::Nav,
            Rc::clone(&cache),
            Arc::clone(&registry),
            None,
            Default::default(),
            Default::default(),
        );
        assert_eq!(reopened.view_state().unwrap(), updated);
        drop(reopened);
        writer.shutdown().unwrap();
    }

    /// 启动恢复未知的视图只能使用内存默认态，关闭时不得覆盖已有持久记录。
    #[test]
    fn side_view_recovery_barrier_prevents_default_state_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(&temp);
        let registry = side_view_state_registry();
        let persisted = persisted_nav(&registry);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .save_view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &persisted,
            )
            .unwrap();
        drop(store);
        let cache = Rc::new(RefCell::new(BTreeMap::new()));
        let writer = WorkspaceWriter::start(paths.clone()).unwrap();
        let view = SideView::with_persistence(
            SideViewId::Nav,
            cache,
            Arc::clone(&registry),
            Some(writer.client()),
            Rc::new(RefCell::new(BTreeSet::from([SideViewId::Nav]))),
            Default::default(),
        );
        assert_eq!(view.view_state().unwrap().state()["scroll"], json!(0.0));
        drop(view);
        writer.flush().unwrap();

        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let stored = observer
            .view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &registry,
            )
            .unwrap()
            .expect("原有有效记录必须保留");
        assert_eq!(stored.state, persisted);
        writer.shutdown().unwrap();
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod side_view_gpui_persistence_tests {
    use gpui::TestAppContext;
    use serde_json::json;
    use slterm_workspace::{ViewKey, ViewState};

    use super::*;
    use crate::app_shell::app_root::side_view_host::SideViewHost;
    use slterm_sidebar::SidebarState;

    fn nav_state(registry: &ViewStateRegistry, scroll: f64) -> SideViewState {
        let key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let state = ViewState::new(
            &key,
            json!({"projects": [], "unassigned_expanded": true, "scroll": scroll}),
            registry,
        )
        .unwrap();
        SideViewState::from_view_state(SideViewId::Nav, state).unwrap()
    }

    /// 旧实体仍被强引用时，同一 update 关闭重开也使用关闭快照；旧代次延迟析构不得回写。
    #[gpui::test]
    fn side_view_same_update_reopen_uses_latest_state_and_blocks_stale_drop(
        cx: &mut TestAppContext,
    ) {
        let registry = side_view_state_registry();
        let cache = Rc::new(RefCell::new(BTreeMap::from([(
            SideViewId::Nav,
            nav_state(&registry, 7.0),
        )])));
        let mut host = cx.update(|cx| {
            SideViewHost::new(
                SidebarState::default(),
                SideViewController::new(
                    cx,
                    Rc::clone(&cache),
                    Arc::clone(&registry),
                    None,
                    Default::default(),
                    Default::default(),
                ),
            )
        });
        let old = host.instance(SideViewId::Nav).unwrap().clone();
        cx.update(|cx| {
            old.update(cx, |view, _| {
                view.state = nav_state(&registry, 42.0);
            });
            host.close_view(
                SideViewId::Nav,
                SideViewController::new(
                    cx,
                    Rc::clone(&cache),
                    Arc::clone(&registry),
                    None,
                    Default::default(),
                    Default::default(),
                ),
            );
            host.click_button(
                SideViewId::Nav,
                SideViewController::new(
                    cx,
                    Rc::clone(&cache),
                    Arc::clone(&registry),
                    None,
                    Default::default(),
                    Default::default(),
                ),
            )
            .unwrap();
        });

        let reopened = host.instance(SideViewId::Nav).unwrap().clone();
        cx.update(|cx| {
            assert_eq!(
                reopened.read(cx).view_state().unwrap().state()["scroll"],
                json!(42.0),
                "重开必须立即读取旧实体的同步关闭快照"
            );
            reopened.update(cx, |view, _| {
                view.state = nav_state(&registry, 84.0);
            });
            host.close_view(
                SideViewId::Nav,
                SideViewController::new(
                    cx,
                    Rc::clone(&cache),
                    Arc::clone(&registry),
                    None,
                    Default::default(),
                    Default::default(),
                ),
            );
        });
        drop(old);
        // 驱动延迟释放，确保旧实体 Drop 真正执行而不是只做强计数断言。
        cx.update(|_| {});
        cx.run_until_parked();
        assert_eq!(
            cache.borrow()[&SideViewId::Nav]
                .to_view_state(&registry)
                .unwrap()
                .state()["scroll"],
            json!(84.0),
            "旧代次延迟 Drop 不得覆盖新实体状态"
        );
        host.shutdown();
    }
}
