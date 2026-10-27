//! SideView 实例所有权与 core 组织状态转换的接线。
//!
//! 创建闭包按调用注入，使 GPUI `Context` 不进入 Host 的自引用状态；
//! 实例句柄及其私有任务由 Host 所有权统一关闭。

use std::collections::BTreeMap;

use slterm_sidebar::{ButtonMovement, SideSlot, SidebarError, SidebarState, SidebarTransition};
use slterm_workspace::SideViewId;

/// 拥有侧栏组织状态与全部存活 SideView 实例。
/// 实例必须独占其私有任务句柄，并以 Drop 完成取消与实体释放。
pub struct SideViewHost<T> {
    state: SidebarState,
    instances: BTreeMap<SideViewId, T>,
}

impl<T> SideViewHost<T> {
    /// 恢复组织状态并只创建已打开视图；创建闭包在返回实例前恢复其持久状态。
    pub fn new(state: SidebarState, mut open: impl FnMut(SideViewId) -> T) -> Self {
        let mut host = Self {
            state,
            instances: BTreeMap::new(),
        };
        for slot in SideSlot::ALL {
            if let Some(view) = host.state.open_view(slot) {
                host.open_instance(view, &mut open);
            }
        }
        host
    }

    pub const fn state(&self) -> &SidebarState {
        &self.state
    }

    pub fn instance(&self, view: SideViewId) -> Option<&T> {
        self.instances.get(&view)
    }

    pub fn click_button(
        &mut self,
        view: SideViewId,
        open: impl FnMut(SideViewId) -> T,
    ) -> Result<SidebarTransition, SidebarError> {
        let transition = self.state.click_button(view)?;
        Ok(self.apply_transition(transition, open))
    }

    pub fn activate_shortcut(
        &mut self,
        view: SideViewId,
        focused: bool,
        open: impl FnMut(SideViewId) -> T,
    ) -> SidebarTransition {
        let transition = self.state.activate_shortcut(view, focused);
        self.apply_transition(transition, open)
    }

    pub fn toggle_sidebar(&mut self, open: impl FnMut(SideViewId) -> T) -> SidebarTransition {
        let transition = self.state.toggle_sidebar();
        self.apply_transition(transition, open)
    }

    pub fn close_view(&mut self, view: SideViewId) -> SidebarTransition {
        let transition = self.state.close_view(view);
        self.apply_transition(transition, |_| {
            unreachable!("关闭 SideView 不得产生打开效果")
        })
    }

    pub fn set_button_hidden(&mut self, view: SideViewId, hidden: bool) {
        self.state.set_button_hidden(view, hidden);
    }

    pub fn reorder_button(&mut self, view: SideViewId, movement: ButtonMovement) {
        self.state.reorder_button(view, movement);
    }

    pub fn move_button_to_slot(
        &mut self,
        view: SideViewId,
        target: SideSlot,
        target_index: usize,
    ) -> Result<SidebarTransition, SidebarError> {
        let transition = self.state.move_button_to_slot(view, target, target_index)?;
        Ok(self.apply_transition(transition, |_| {
            unreachable!("移动 SideView 按钮不得产生打开效果")
        }))
    }

    pub fn restore_default_buttons(&mut self) -> SidebarTransition {
        let transition = self.state.restore_default_buttons();
        self.apply_transition(transition, |_| {
            unreachable!("恢复默认 SideView 按钮不得产生打开效果")
        })
    }

    /// 显式关闭全部实例；Drop 复用同一路径，保证没有孤儿任务。
    pub fn shutdown(mut self) {
        self.close_all();
    }

    fn apply_transition(
        &mut self,
        transition: SidebarTransition,
        mut open: impl FnMut(SideViewId) -> T,
    ) -> SidebarTransition {
        for view in transition.closed.iter().copied() {
            self.close_instance(view);
        }
        for view in transition.opened.iter().copied() {
            self.open_instance(view, &mut open);
        }
        transition
    }

    fn open_instance(&mut self, view: SideViewId, open: &mut impl FnMut(SideViewId) -> T) {
        assert!(
            !self.instances.contains_key(&view),
            "SideView 打开效果不得重复创建存活实例"
        );
        let instance = open(view);
        let previous = self.instances.insert(view, instance);
        debug_assert!(previous.is_none());
    }

    fn close_instance(&mut self, view: SideViewId) {
        let instance = self
            .instances
            .remove(&view)
            .expect("SideView 关闭效果必须对应存活实例");
        drop(instance);
    }

    fn close_all(&mut self) {
        self.instances.clear();
    }
}

impl<T> Drop for SideViewHost<T> {
    fn drop(&mut self) {
        self.close_all();
    }
}

#[cfg(test)]
mod side_view_host_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RuntimeEvent {
        Open(SideViewId, u64),
        Shutdown(SideViewId, u64),
    }

    type Events = Rc<RefCell<Vec<RuntimeEvent>>>;
    type Runtime = Rc<RefCell<FakeRuntime>>;

    struct FakeInstance {
        view: SideViewId,
        serial: u64,
        events: Events,
    }

    impl Drop for FakeInstance {
        fn drop(&mut self) {
            self.events
                .borrow_mut()
                .push(RuntimeEvent::Shutdown(self.view, self.serial));
        }
    }

    struct FakeRuntime {
        next_serial: u64,
        events: Events,
    }

    impl FakeRuntime {
        fn open(&mut self, view: SideViewId) -> FakeInstance {
            self.next_serial += 1;
            let serial = self.next_serial;
            self.events
                .borrow_mut()
                .push(RuntimeEvent::Open(view, serial));
            FakeInstance {
                view,
                serial,
                events: Rc::clone(&self.events),
            }
        }
    }

    fn opener(runtime: &Runtime) -> impl FnMut(SideViewId) -> FakeInstance {
        let runtime = Rc::clone(runtime);
        move |view| runtime.borrow_mut().open(view)
    }

    fn harness(state: SidebarState) -> (SideViewHost<FakeInstance>, Events, Runtime) {
        let events = Rc::new(RefCell::new(Vec::new()));
        let runtime = Rc::new(RefCell::new(FakeRuntime {
            next_serial: 0,
            events: Rc::clone(&events),
        }));
        let host = SideViewHost::new(state, opener(&runtime));
        (host, events, runtime)
    }

    fn serial(host: &SideViewHost<FakeInstance>, view: SideViewId) -> u64 {
        host.instance(view).expect("视图实例必须存活").serial
    }

    #[test]
    fn side_view_host_same_slot_replacement_destroys_old_instance() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        let nav = serial(&host, SideViewId::Nav);
        let transition = host
            .click_button(SideViewId::Explorer, opener(&runtime))
            .unwrap();
        let explorer = serial(&host, SideViewId::Explorer);

        assert!(host.instance(SideViewId::Nav).is_none());
        assert_ne!(nav, explorer);
        assert_eq!(transition.focus, Some(SideViewId::Explorer));
        host.shutdown();
        assert_eq!(
            events.borrow().as_slice(),
            [
                RuntimeEvent::Open(SideViewId::Nav, nav),
                RuntimeEvent::Shutdown(SideViewId::Nav, nav),
                RuntimeEvent::Open(SideViewId::Explorer, explorer),
                RuntimeEvent::Shutdown(SideViewId::Explorer, explorer),
            ]
        );
    }

    #[test]
    fn side_view_host_hiding_sidebar_preserves_instance() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        let nav = serial(&host, SideViewId::Nav);
        events.borrow_mut().clear();

        assert!(!host.toggle_sidebar(opener(&runtime)).is_visible);
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        assert!(host.toggle_sidebar(opener(&runtime)).is_visible);
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        assert!(events.borrow().is_empty());
        drop(host);
        assert_eq!(
            events.borrow().as_slice(),
            [RuntimeEvent::Shutdown(SideViewId::Nav, nav)]
        );
    }

    #[test]
    fn side_view_host_hiding_button_preserves_instance() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        let nav = serial(&host, SideViewId::Nav);
        events.borrow_mut().clear();

        host.set_button_hidden(SideViewId::Nav, true);
        assert!(!host.state().button_is_visible(SideViewId::Nav));
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        let transition = host.activate_shortcut(SideViewId::Nav, false, opener(&runtime));
        assert_eq!(transition.focus, Some(SideViewId::Nav));
        assert!(host.state().button_is_visible(SideViewId::Nav));
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        assert!(events.borrow().is_empty());
    }

    #[test]
    fn side_view_host_cross_slot_swap_preserves_both_instances() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        host.move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
            .unwrap();
        host.click_button(SideViewId::Explorer, opener(&runtime))
            .unwrap();
        let nav = serial(&host, SideViewId::Nav);
        let explorer = serial(&host, SideViewId::Explorer);
        events.borrow_mut().clear();

        host.reorder_button(SideViewId::Nav, ButtonMovement::Bottom);
        host.move_button_to_slot(SideViewId::Explorer, SideSlot::Upper, 0)
            .unwrap();
        assert_eq!(
            host.state().open_view(SideSlot::Upper),
            Some(SideViewId::Explorer)
        );
        assert_eq!(
            host.state().open_view(SideSlot::Lower),
            Some(SideViewId::Nav)
        );
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        assert_eq!(serial(&host, SideViewId::Explorer), explorer);
        assert!(events.borrow().is_empty());
    }

    #[test]
    fn side_view_host_close_and_reopen_recreates_instance() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        let nav = serial(&host, SideViewId::Nav);
        events.borrow_mut().clear();

        host.close_view(SideViewId::Nav);
        assert!(host.instance(SideViewId::Nav).is_none());
        host.click_button(SideViewId::Nav, opener(&runtime))
            .unwrap();
        let reopened = serial(&host, SideViewId::Nav);
        assert_ne!(nav, reopened);
        host.shutdown();
        assert_eq!(
            events.borrow().as_slice(),
            [
                RuntimeEvent::Shutdown(SideViewId::Nav, nav),
                RuntimeEvent::Open(SideViewId::Nav, reopened),
                RuntimeEvent::Shutdown(SideViewId::Nav, reopened),
            ]
        );
    }

    #[test]
    fn side_view_host_startup_only_creates_open_views() {
        let mut state = SidebarState::default();
        state.close_view(SideViewId::Nav);
        let (mut host, events, runtime) = harness(state);
        for view in SideViewId::ALL {
            assert!(host.instance(view).is_none());
        }
        assert!(events.borrow().is_empty());

        host.toggle_sidebar(opener(&runtime));
        assert_eq!(serial(&host, SideViewId::Nav), 1);
        for view in SideViewId::ALL {
            if view != SideViewId::Nav {
                assert!(host.instance(view).is_none());
            }
        }
        host.shutdown();
        assert_eq!(
            events.borrow().as_slice(),
            [
                RuntimeEvent::Open(SideViewId::Nav, 1),
                RuntimeEvent::Shutdown(SideViewId::Nav, 1),
            ]
        );
    }

    #[test]
    fn side_view_host_restoring_defaults_closes_only_conflicting_instances() {
        let (mut host, events, runtime) = harness(SidebarState::default());
        host.move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
            .unwrap();
        host.click_button(SideViewId::Explorer, opener(&runtime))
            .unwrap();
        let nav = serial(&host, SideViewId::Nav);
        let explorer = serial(&host, SideViewId::Explorer);
        events.borrow_mut().clear();

        host.restore_default_buttons();
        assert_eq!(serial(&host, SideViewId::Nav), nav);
        assert!(host.instance(SideViewId::Explorer).is_none());
        host.shutdown();
        assert_eq!(
            events.borrow().as_slice(),
            [
                RuntimeEvent::Shutdown(SideViewId::Explorer, explorer),
                RuntimeEvent::Shutdown(SideViewId::Nav, nav),
            ]
        );
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod gpui_side_view_host_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        App, AppContext as _, Entity, EntityId, IntoElement, Render, Task, TestAppContext,
        WeakEntity, Window, div,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use slterm_path::AppPaths;
    use slterm_workspace::{
        InvalidViewState, ViewKey, ViewKeyKind, ViewState, ViewStateRegistry, ViewStateSchema,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum GpuiEvent {
        Open(SideViewId, u64),
        Shutdown(SideViewId, u64),
        TaskCancelled(SideViewId, u64),
    }

    type Events = Rc<RefCell<Vec<GpuiEvent>>>;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct TransientState {
        search: String,
        multi_select: bool,
        inline_edit: bool,
        menu_open: bool,
        error: bool,
        focused: bool,
    }

    impl TransientState {
        fn fill(&mut self) {
            self.search = "query".to_owned();
            self.multi_select = true;
            self.inline_edit = true;
            self.menu_open = true;
            self.error = true;
            self.focused = true;
        }
    }

    struct TaskProbe {
        view: SideViewId,
        serial: u64,
        events: Events,
    }

    impl Drop for TaskProbe {
        fn drop(&mut self) {
            self.events
                .borrow_mut()
                .push(GpuiEvent::TaskCancelled(self.view, self.serial));
        }
    }

    struct GpuiFakeInstance {
        view: SideViewId,
        serial: u64,
        position: u64,
        transient: TransientState,
        events: Events,
        // 私有任务由实体独占；实体析构时 Task Drop 必须取消挂起的 future。
        _task: Task<()>,
    }

    impl GpuiFakeInstance {
        fn new(
            view: SideViewId,
            serial: u64,
            position: u64,
            events: Events,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            let task_events = Rc::clone(&events);
            let task = cx.spawn(async move |_, _| {
                let _probe = TaskProbe {
                    view,
                    serial,
                    events: task_events,
                };
                std::future::pending::<()>().await;
            });
            Self {
                view,
                serial,
                position,
                transient: TransientState::default(),
                events,
                _task: task,
            }
        }
    }

    impl Drop for GpuiFakeInstance {
        fn drop(&mut self) {
            self.events
                .borrow_mut()
                .push(GpuiEvent::Shutdown(self.view, self.serial));
        }
    }

    struct Dummy;

    impl Render for Dummy {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<'_, Self>) -> impl IntoElement {
            div()
        }
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PersistentState {
        position: u64,
    }

    fn decode_persistent_state(text: &str) -> Result<Value, InvalidViewState> {
        let state: PersistentState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
        serde_json::to_value(state).map_err(|_| InvalidViewState)
    }

    fn registry() -> ViewStateRegistry {
        ViewStateRegistry::new(&[ViewStateSchema::new(
            ViewKeyKind::SideView,
            decode_persistent_state,
        )])
        .unwrap()
    }

    fn view_key(view: SideViewId) -> ViewKey {
        ViewKey::SideView { side_view_id: view }
    }

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn restored_position(
        store: &WorkspaceDatabase,
        registry: &ViewStateRegistry,
        view: SideViewId,
    ) -> u64 {
        store
            .view_state(&view_key(view), registry)
            .unwrap()
            .map(|stored| {
                serde_json::from_value::<PersistentState>(stored.state.into_state())
                    .unwrap()
                    .position
            })
            .unwrap_or(0)
    }

    fn save_position(
        store: &mut WorkspaceDatabase,
        registry: &ViewStateRegistry,
        view: SideViewId,
        position: u64,
    ) {
        let state = ViewState::new(
            ViewKeyKind::SideView,
            json!({"position": position}),
            registry,
        )
        .unwrap();
        store.save_view_state(&view_key(view), &state).unwrap();
    }

    fn create_instance(
        cx: &mut App,
        events: &Events,
        next_serial: &mut u64,
        view: SideViewId,
        position: u64,
    ) -> Entity<GpuiFakeInstance> {
        *next_serial += 1;
        let serial = *next_serial;
        events.borrow_mut().push(GpuiEvent::Open(view, serial));
        cx.new(|cx| GpuiFakeInstance::new(view, serial, position, Rc::clone(events), cx))
    }

    fn opener<'a>(
        cx: &'a mut App,
        events: &'a Events,
        next_serial: &'a mut u64,
    ) -> impl FnMut(SideViewId) -> Entity<GpuiFakeInstance> + 'a {
        move |view| create_instance(cx, events, next_serial, view, 0)
    }

    fn restored_opener<'a>(
        cx: &'a mut App,
        store: &'a WorkspaceDatabase,
        registry: &'a ViewStateRegistry,
        events: &'a Events,
        next_serial: &'a mut u64,
    ) -> impl FnMut(SideViewId) -> Entity<GpuiFakeInstance> + 'a {
        move |view| {
            let position = restored_position(store, registry, view);
            create_instance(cx, events, next_serial, view, position)
        }
    }

    fn dummy_window(cx: &mut TestAppContext) -> &mut gpui::VisualTestContext {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window
    }

    fn entity_id(host: &SideViewHost<Entity<GpuiFakeInstance>>, view: SideViewId) -> EntityId {
        host.instance(view).expect("视图实体必须存活").entity_id()
    }

    fn weak_instance(
        host: &SideViewHost<Entity<GpuiFakeInstance>>,
        view: SideViewId,
    ) -> WeakEntity<GpuiFakeInstance> {
        host.instance(view).expect("视图实体必须存活").downgrade()
    }

    fn assert_destroyed(events: &Events, view: SideViewId, serial: u64) {
        let events = events.borrow();
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == GpuiEvent::Shutdown(view, serial))
                .count(),
            1,
            "旧实体必须恰好析构一次"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == GpuiEvent::TaskCancelled(view, serial))
                .count(),
            1,
            "实体私有任务必须恰好取消一次"
        );
    }

    fn assert_not_destroyed(events: &Events, view: SideViewId, serial: u64) {
        assert!(!events.borrow().iter().any(|event| matches!(
            event,
            GpuiEvent::Shutdown(event_view, event_serial)
                | GpuiEvent::TaskCancelled(event_view, event_serial)
                if *event_view == view && *event_serial == serial
        )));
    }

    fn fill_transient(
        window: &mut gpui::VisualTestContext,
        host: &SideViewHost<Entity<GpuiFakeInstance>>,
        view: SideViewId,
    ) {
        window.update(|_, cx| {
            host.instance(view).unwrap().update(cx, |instance, _| {
                instance.position = 42;
                instance.transient.fill();
            });
        });
    }

    fn transient(
        window: &mut gpui::VisualTestContext,
        host: &SideViewHost<Entity<GpuiFakeInstance>>,
        view: SideViewId,
    ) -> TransientState {
        window.update(|_, cx| host.instance(view).unwrap().read(cx).transient.clone())
    }

    /// 同槽替换释放最后一个实体强句柄，并在 effect flush 中取消私有任务。
    #[gpui::test]
    fn sideview_same_slot_replacement_destroys_instance(cx: &mut TestAppContext) {
        let window = dummy_window(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut next_serial = 0;
        let mut host = window.update(|_, cx| {
            SideViewHost::new(
                SidebarState::default(),
                opener(cx, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        let nav = entity_id(&host, SideViewId::Nav);
        let weak_nav = weak_instance(&host, SideViewId::Nav);

        window
            .update(|_, cx| {
                host.click_button(SideViewId::Explorer, opener(cx, &events, &mut next_serial))
            })
            .unwrap();
        window.run_until_parked();
        let explorer = entity_id(&host, SideViewId::Explorer);

        assert_ne!(nav, explorer);
        assert!(host.instance(SideViewId::Nav).is_none());
        assert!(weak_nav.upgrade().is_none());
        assert_destroyed(&events, SideViewId::Nav, 1);
        assert_not_destroyed(&events, SideViewId::Explorer, 2);

        window.update(|_, _| host.shutdown());
        window.run_until_parked();
        assert_destroyed(&events, SideViewId::Explorer, 2);
    }

    /// 整栏隐藏只改变展示，不重建实体或清空视图瞬态。
    #[gpui::test]
    fn sideview_hide_preserves_instance(cx: &mut TestAppContext) {
        let window = dummy_window(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut next_serial = 0;
        let mut host = window.update(|_, cx| {
            SideViewHost::new(
                SidebarState::default(),
                opener(cx, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        fill_transient(window, &host, SideViewId::Nav);
        let nav = entity_id(&host, SideViewId::Nav);
        let weak_nav = weak_instance(&host, SideViewId::Nav);
        let preserved_transient = transient(window, &host, SideViewId::Nav);
        events.borrow_mut().clear();

        assert!(
            !window
                .update(|_, cx| host.toggle_sidebar(opener(cx, &events, &mut next_serial)))
                .is_visible
        );
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert!(weak_nav.upgrade().is_some());
        assert_eq!(
            transient(window, &host, SideViewId::Nav),
            preserved_transient
        );
        assert!(events.borrow().is_empty());

        assert!(
            window
                .update(|_, cx| host.toggle_sidebar(opener(cx, &events, &mut next_serial)))
                .is_visible
        );
        window.run_until_parked();
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert_eq!(
            transient(window, &host, SideViewId::Nav),
            preserved_transient
        );
        assert!(events.borrow().is_empty());

        window.update(|_, _| host.shutdown());
    }

    /// 隐藏按钮不改变内容实例；快捷入口恢复按钮时仍复用原实例。
    #[gpui::test]
    fn sideview_hide_button_preserves_instance(cx: &mut TestAppContext) {
        let window = dummy_window(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut next_serial = 0;
        let mut host = window.update(|_, cx| {
            SideViewHost::new(
                SidebarState::default(),
                opener(cx, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        fill_transient(window, &host, SideViewId::Nav);
        let nav = entity_id(&host, SideViewId::Nav);
        let preserved_transient = transient(window, &host, SideViewId::Nav);
        events.borrow_mut().clear();

        window.update(|_, _| host.set_button_hidden(SideViewId::Nav, true));
        assert!(!host.state().button_is_visible(SideViewId::Nav));
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert_eq!(
            transient(window, &host, SideViewId::Nav),
            preserved_transient
        );
        assert!(events.borrow().is_empty());

        let transition = window.update(|_, cx| {
            host.activate_shortcut(
                SideViewId::Nav,
                false,
                opener(cx, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        assert_eq!(transition.focus, Some(SideViewId::Nav));
        assert!(host.state().button_is_visible(SideViewId::Nav));
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert_eq!(
            transient(window, &host, SideViewId::Nav),
            preserved_transient
        );
        assert!(events.borrow().is_empty());

        window.update(|_, _| host.shutdown());
    }

    /// 跨槽交换保留两个实体；随后同槽替换只销毁被替换的一侧。
    #[gpui::test]
    fn sideview_cross_slot_swap_preserves_instances(cx: &mut TestAppContext) {
        let window = dummy_window(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut next_serial = 0;
        let mut host = window.update(|_, cx| {
            SideViewHost::new(
                SidebarState::default(),
                opener(cx, &events, &mut next_serial),
            )
        });
        host.move_button_to_slot(SideViewId::Explorer, SideSlot::Lower, 0)
            .unwrap();
        window
            .update(|_, cx| {
                host.click_button(SideViewId::Explorer, opener(cx, &events, &mut next_serial))
            })
            .unwrap();
        window.run_until_parked();
        let nav = entity_id(&host, SideViewId::Nav);
        let explorer = entity_id(&host, SideViewId::Explorer);
        let weak_nav = weak_instance(&host, SideViewId::Nav);
        let weak_explorer = weak_instance(&host, SideViewId::Explorer);
        events.borrow_mut().clear();

        window.update(|_, _| {
            host.reorder_button(SideViewId::Nav, ButtonMovement::Bottom);
            host.move_button_to_slot(SideViewId::Explorer, SideSlot::Upper, 0)
                .unwrap();
        });
        window.run_until_parked();
        assert_eq!(
            host.state().open_view(SideSlot::Upper),
            Some(SideViewId::Explorer)
        );
        assert_eq!(
            host.state().open_view(SideSlot::Lower),
            Some(SideViewId::Nav)
        );
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert_eq!(entity_id(&host, SideViewId::Explorer), explorer);
        assert!(events.borrow().is_empty());

        window
            .update(|_, cx| {
                host.click_button(SideViewId::Commit, opener(cx, &events, &mut next_serial))
            })
            .unwrap();
        window.run_until_parked();
        assert!(weak_explorer.upgrade().is_none());
        assert!(weak_nav.upgrade().is_some());
        assert_eq!(entity_id(&host, SideViewId::Nav), nav);
        assert_destroyed(&events, SideViewId::Explorer, 2);
        assert_not_destroyed(&events, SideViewId::Nav, 1);

        window.update(|_, _| host.shutdown());
    }

    /// 关闭重开与 Host 重建只恢复规范持久状态，所有视图瞬态重新初始化。
    #[gpui::test]
    fn sideview_close_and_restart_share_view_state(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let registry = registry();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let window = dummy_window(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut next_serial = 0;
        let mut host = window.update(|_, cx| {
            SideViewHost::new(
                SidebarState::default(),
                restored_opener(cx, &store, &registry, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        let first = entity_id(&host, SideViewId::Nav);
        let weak_first = weak_instance(&host, SideViewId::Nav);
        fill_transient(window, &host, SideViewId::Nav);
        save_position(&mut store, &registry, SideViewId::Nav, 42);

        window.update(|_, _| host.close_view(SideViewId::Nav));
        window.run_until_parked();
        assert!(weak_first.upgrade().is_none());
        assert_destroyed(&events, SideViewId::Nav, 1);
        assert_eq!(
            restored_position(&store, &registry, SideViewId::Nav),
            42,
            "关闭视图不得删除 ViewKey 持久状态"
        );

        window
            .update(|_, cx| {
                host.click_button(
                    SideViewId::Nav,
                    restored_opener(cx, &store, &registry, &events, &mut next_serial),
                )
            })
            .unwrap();
        window.run_until_parked();
        let reopened = entity_id(&host, SideViewId::Nav);
        assert_ne!(first, reopened);
        window.update(|_, cx| {
            let instance = host.instance(SideViewId::Nav).unwrap().read(cx);
            assert_eq!(instance.position, 42);
            assert_eq!(instance.transient, TransientState::default());
        });
        fill_transient(window, &host, SideViewId::Nav);

        let restored_layout = host.state().clone();
        window.update(|_, _| host.shutdown());
        window.run_until_parked();
        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let host = window.update(|_, cx| {
            SideViewHost::new(
                restored_layout,
                restored_opener(cx, &store, &registry, &events, &mut next_serial),
            )
        });
        window.run_until_parked();
        let restarted = entity_id(&host, SideViewId::Nav);
        assert_ne!(reopened, restarted);
        window.update(|_, cx| {
            let instance = host.instance(SideViewId::Nav).unwrap().read(cx);
            assert_eq!(instance.position, 42);
            assert_eq!(instance.transient, TransientState::default());
        });

        window.update(|_, _| host.shutdown());
    }
}
