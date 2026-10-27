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
