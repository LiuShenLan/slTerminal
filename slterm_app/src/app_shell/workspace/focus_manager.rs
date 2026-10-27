//! GPUI 焦点所有权：弱句柄、实例代次、项目记忆与模态还焦链。

use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

use gpui::{App, Context, FocusHandle, Subscription, WeakFocusHandle, Window};
use slterm_workspace::ProjectId;

/// 由 FocusManager 分配的存活实例身份；不进入持久 DTO。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FocusToken {
    generation: u64,
}

/// 注册节点随 owner 保存；凭证释放后，即使渲染帧仍强持 FocusHandle 也不可聚焦。
#[derive(Debug)]
pub struct FocusRegistration {
    token: FocusToken,
    owner: Rc<()>,
}

impl FocusRegistration {
    pub const fn token(&self) -> FocusToken {
        self.token
    }
}

/// 组合根保存的窗口焦点订阅；pending-input 同步实际焦点，focus-lost 恢复失效路径。
pub struct FocusSubscriptions {
    _pending_input: Subscription,
    _focus_lost: Subscription,
}

impl FocusSubscriptions {
    pub fn new<T, F, G>(
        cx: &mut Context<'_, T>,
        window: &mut Window,
        sync: F,
        focus_lost: G,
    ) -> Self
    where
        T: 'static,
        F: FnMut(&mut T, &mut Window, &mut Context<'_, T>) + 'static,
        G: FnMut(&mut T, &mut Window, &mut Context<'_, T>) + 'static,
    {
        let pending_input = cx.observe_pending_input(window, {
            let mut sync = sync;
            move |this, window, cx| Self::sync_and_refresh(this, window, cx, &mut sync)
        });
        let focus_lost = cx.on_focus_lost(window, {
            let mut focus_lost = focus_lost;
            move |this, window, cx| Self::sync_and_refresh(this, window, cx, &mut focus_lost)
        });
        Self {
            _pending_input: pending_input,
            _focus_lost: focus_lost,
        }
    }

    fn sync_and_refresh<T, F>(
        this: &mut T,
        window: &mut Window,
        cx: &mut Context<'_, T>,
        sync: &mut F,
    ) where
        T: 'static,
        F: FnMut(&mut T, &mut Window, &mut Context<'_, T>),
    {
        let before = window.focused(cx).map(|handle| handle.downgrade());
        sync(this, window, cx);
        let after = window.focused(cx).map(|handle| handle.downgrade());
        if before != after {
            // focus-lost 发生在绘制末段；延迟到效果周期再标脏，确保下一帧重建输入处理器。
            window.defer(cx, |window, _| window.refresh());
        }
    }
}

/// 焦点请求或注册失败。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusError {
    UnknownToken,
    InvalidParent,
    DuplicateHandle,
    ScopeMismatch,
    GenerationExhausted,
    Unavailable,
    ModalActive,
    NoModal,
    NoFocusTarget,
    WorkspaceRoot,
}

struct FocusNode {
    handle: WeakFocusHandle,
    // 工作区根与 Manager 同属于组合根；其余节点只弱引用 owner 凭证。
    owner: Option<Weak<()>>,
    parent: Option<FocusToken>,
    project: Option<ProjectId>,
    visible: bool,
}

struct ObservedFocus {
    token: Option<FocusToken>,
    handle: WeakFocusHandle,
}

impl FocusNode {
    fn owner_alive(&self) -> bool {
        self.owner
            .as_ref()
            .is_none_or(|owner| owner.upgrade().is_some())
    }
}

struct ModalFocus {
    token: FocusToken,
    return_chain: Vec<FocusToken>,
}

/// 单窗口焦点权威；只弱引用 GPUI 句柄，不延长任何视图生命周期。
pub struct FocusManager {
    nodes: BTreeMap<FocusToken, FocusNode>,
    next_generation: u64,
    workspace: FocusToken,
    active_project: Option<ProjectId>,
    last_focus: BTreeMap<ProjectId, FocusToken>,
    observed_focus: Option<ObservedFocus>,
    modal: Option<ModalFocus>,
}

impl FocusManager {
    pub fn new(workspace: &FocusHandle) -> Self {
        let workspace_token = FocusToken { generation: 1 };
        let mut nodes = BTreeMap::new();
        nodes.insert(
            workspace_token,
            FocusNode {
                handle: workspace.downgrade(),
                owner: None,
                parent: None,
                project: None,
                visible: true,
            },
        );
        Self {
            nodes,
            next_generation: 1,
            workspace: workspace_token,
            active_project: None,
            last_focus: BTreeMap::new(),
            observed_focus: None,
            modal: None,
        }
    }

    pub const fn workspace_token(&self) -> FocusToken {
        self.workspace
    }

    pub fn register(
        &mut self,
        handle: &FocusHandle,
        parent: FocusToken,
        project: Option<ProjectId>,
        visible: bool,
    ) -> Result<FocusRegistration, FocusError> {
        let parent_project = self
            .nodes
            .get(&parent)
            .filter(|node| node.owner_alive() && node.handle.upgrade().is_some())
            .map(|node| node.project)
            .ok_or(FocusError::InvalidParent)?;
        if parent_project.is_some() && parent_project != project {
            return Err(FocusError::ScopeMismatch);
        }
        let handle = handle.downgrade();
        if self.nodes.values().any(|node| node.handle == handle) {
            return Err(FocusError::DuplicateHandle);
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(FocusError::GenerationExhausted)?;
        let token = FocusToken { generation };
        self.next_generation = generation;
        let owner = Rc::new(());
        self.nodes.insert(
            token,
            FocusNode {
                handle,
                owner: Some(Rc::downgrade(&owner)),
                parent: Some(parent),
                project,
                visible,
            },
        );
        Ok(FocusRegistration { token, owner })
    }

    pub fn unregister(&mut self, token: FocusToken) -> Result<(), FocusError> {
        if token == self.workspace {
            return Err(FocusError::WorkspaceRoot);
        }
        if self
            .modal
            .as_ref()
            .is_some_and(|modal| modal.token == token)
        {
            return Err(FocusError::ModalActive);
        }
        let removed = self.nodes.remove(&token).ok_or(FocusError::UnknownToken)?;
        let children = self
            .nodes
            .iter()
            .filter(|(_, node)| node.parent == Some(token))
            .map(|(child, _)| *child)
            .collect::<Vec<_>>();
        for child in children {
            if let Some(node) = self.nodes.get_mut(&child) {
                node.parent = removed.parent;
            }
        }
        Ok(())
    }

    pub fn set_visible(&mut self, token: FocusToken, visible: bool) -> Result<(), FocusError> {
        let node = self.nodes.get_mut(&token).ok_or(FocusError::UnknownToken)?;
        node.visible = visible;
        Ok(())
    }

    /// 非模态内容只恢复可见性；键盘入口仍经 request_focus 显式送焦。
    pub fn present_nonmodal(&mut self, token: FocusToken) -> Result<(), FocusError> {
        self.set_visible(token, true)
    }

    pub fn request_focus(
        &mut self,
        token: FocusToken,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusError> {
        if self
            .modal
            .as_ref()
            .is_some_and(|modal| !self.is_within(token, modal.token))
        {
            return Err(FocusError::ModalActive);
        }
        let project = self.focusable_project(token)?;
        let handle = self
            .nodes
            .get(&token)
            .and_then(|node| node.handle.upgrade())
            .ok_or(FocusError::Unavailable)?;
        handle.focus(window, cx);
        self.observe_window_focus(window, cx);
        if let Some(project) = project {
            self.last_focus.insert(project, token);
        }
        Ok(())
    }

    /// 同步 GPUI 实际焦点；上一弱快照区分主动 blur 与句柄完全释放。
    pub fn sync_window_focus(&mut self, window: &mut Window, cx: &mut App) {
        self.sync_window_focus_inner(window, cx);
        self.observe_window_focus(window, cx);
    }

    /// 渲染树丢失原焦点路径时，复用同一弱快照恢复有效焦点。
    pub fn recover_focus_lost(&mut self, window: &mut Window, cx: &mut App) {
        self.sync_window_focus(window, cx);
    }

    fn sync_window_focus_inner(&mut self, window: &mut Window, cx: &mut App) {
        let focused = window.focused(cx).map(|handle| handle.downgrade());
        let focused_token = focused.as_ref().and_then(|focused| {
            self.nodes
                .iter()
                .find(|(_, node)| node.handle == *focused)
                .map(|(token, _)| *token)
        });

        if let Some(modal) = self.modal.as_ref().map(|modal| modal.token) {
            if focused_token
                .is_some_and(|token| self.is_within(token, modal) && self.is_focusable(token))
            {
                return;
            }
            if self.is_focusable(modal)
                && let Some(handle) = self
                    .nodes
                    .get(&modal)
                    .and_then(|node| node.handle.upgrade())
            {
                handle.focus(window, cx);
            }
            return;
        }

        if let Some(token) = focused_token {
            match self.focusable_project(token) {
                Ok(Some(project)) => {
                    self.last_focus.insert(project, token);
                }
                Ok(None) => {}
                Err(_) => self.recover_invalid_focus(token, window, cx),
            }
        } else if focused.is_some() {
            self.recover_unregistered_focus(window, cx);
        } else {
            self.recover_lost_focus(window, cx);
        }
    }

    fn recover_lost_focus(&mut self, window: &mut Window, cx: &mut App) {
        let Some(observed) = self.observed_focus.as_ref() else {
            return;
        };
        if observed.handle.upgrade().is_some() {
            // 上一句柄仍存活但 Window 已无实际焦点，表示主动 blur 而非句柄释放。
            return;
        }
        let token = observed.token;
        if let Some(token) = token.filter(|token| self.nodes.contains_key(token)) {
            self.recover_invalid_focus(token, window, cx);
        } else {
            self.recover_unregistered_focus(window, cx);
        }
    }

    fn observe_window_focus(&mut self, window: &Window, cx: &App) {
        self.observed_focus = window.focused(cx).map(|handle| {
            let handle = handle.downgrade();
            let token = self
                .nodes
                .iter()
                .find(|(_, node)| node.handle == handle)
                .map(|(token, _)| *token);
            ObservedFocus { token, handle }
        });
    }

    fn recover_unregistered_focus(&mut self, window: &mut Window, cx: &mut App) {
        let mut candidates = Vec::with_capacity(2);
        candidates.extend(
            self.active_project
                .as_ref()
                .and_then(|project| self.last_focus.get(project)),
        );
        candidates.push(self.workspace);
        if self.focus_first(&candidates, window, cx).is_err() {
            window.blur();
        }
    }

    fn recover_invalid_focus(&mut self, token: FocusToken, window: &mut Window, cx: &mut App) {
        let mut candidates = vec![token];
        let mut current = self.nodes.get(&token).and_then(|node| node.parent);
        let mut remaining = self.nodes.len();
        while let Some(token) = current {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            candidates.push(token);
            current = self.nodes.get(&token).and_then(|node| node.parent);
        }
        push_unique(&mut candidates, self.workspace);
        if self.focus_first(&candidates, window, cx).is_err() {
            window.blur();
        }
    }

    pub fn activate_project(
        &mut self,
        project: ProjectId,
        default: FocusToken,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<FocusToken, FocusError> {
        if self.modal.is_some() {
            return Err(FocusError::ModalActive);
        }
        let default_project = self
            .nodes
            .get(&default)
            .map(|node| node.project)
            .ok_or(FocusError::UnknownToken)?;
        if default_project != Some(project) {
            return Err(FocusError::ScopeMismatch);
        }
        self.sync_window_focus(window, cx);
        self.active_project = Some(project);
        let mut candidates = Vec::with_capacity(3);
        candidates.extend(self.last_focus.get(&project));
        candidates.push(default);
        candidates.push(self.workspace);
        self.focus_first(&candidates, window, cx)
    }

    pub fn restore_focus(
        &mut self,
        candidates: &[FocusToken],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<FocusToken, FocusError> {
        self.focus_first(candidates, window, cx)
    }

    pub fn open_modal(
        &mut self,
        trigger: FocusToken,
        panel_default: FocusToken,
        modal: FocusToken,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), FocusError> {
        if self.modal.is_some() {
            return Err(FocusError::ModalActive);
        }
        let trigger_project = self.focusable_project(trigger)?;
        if self.focusable_project(panel_default)? != trigger_project {
            return Err(FocusError::ScopeMismatch);
        }
        self.focusable_project(modal)?;

        // 还焦链在打开时冻结；销毁后的旧 token 不会被新实例复用。
        let mut return_chain = Vec::new();
        let mut current = Some(trigger);
        let mut remaining = self.nodes.len();
        while let Some(token) = current {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            if token == self.workspace {
                break;
            }
            push_unique(&mut return_chain, token);
            current = self.nodes.get(&token).and_then(|node| node.parent);
        }
        push_unique(&mut return_chain, panel_default);
        push_unique(&mut return_chain, self.workspace);
        self.modal = Some(ModalFocus {
            token: modal,
            return_chain,
        });
        if let Err(error) = self.request_focus(modal, window, cx) {
            self.modal = None;
            return Err(error);
        }
        Ok(())
    }

    pub fn close_modal(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<FocusToken, FocusError> {
        let modal = self.modal.take().ok_or(FocusError::NoModal)?;
        match self.focus_first(&modal.return_chain, window, cx) {
            Ok(token) => Ok(token),
            Err(error) => {
                self.modal = Some(modal);
                Err(error)
            }
        }
    }

    fn is_within(&self, token: FocusToken, ancestor: FocusToken) -> bool {
        let mut current = Some(token);
        let mut remaining = self.nodes.len();
        while let Some(token) = current {
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            if token == ancestor {
                return true;
            }
            let Some(node) = self.nodes.get(&token) else {
                return false;
            };
            current = node.parent;
        }
        false
    }

    fn focusable_project(&self, token: FocusToken) -> Result<Option<ProjectId>, FocusError> {
        let node = self.nodes.get(&token).ok_or(FocusError::UnknownToken)?;
        if !self.is_focusable(token) {
            return Err(FocusError::Unavailable);
        }
        Ok(node.project)
    }

    /// 目标与全部逻辑祖先都必须存活、可见，项目节点还须属于当前项目。
    fn is_focusable(&self, token: FocusToken) -> bool {
        let mut current = Some(token);
        let mut remaining = self.nodes.len();
        while let Some(token) = current {
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            let Some(node) = self.nodes.get(&token) else {
                return false;
            };
            if !node.visible
                || !node.owner_alive()
                || node.handle.upgrade().is_none()
                || (node.project.is_some() && self.active_project != node.project)
            {
                return false;
            }
            current = node.parent;
        }
        true
    }

    fn focus_first(
        &mut self,
        candidates: &[FocusToken],
        window: &mut Window,
        cx: &mut App,
    ) -> Result<FocusToken, FocusError> {
        for &token in candidates {
            match self.request_focus(token, window, cx) {
                Ok(()) => return Ok(token),
                Err(FocusError::UnknownToken | FocusError::Unavailable) => {}
                Err(error) => return Err(error),
            }
        }
        Err(FocusError::NoFocusTarget)
    }
}

fn push_unique(chain: &mut Vec<FocusToken>, token: FocusToken) {
    if !chain.contains(&token) {
        chain.push(token);
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod focus_manager_tests {
    use std::{cell::Cell, ops::Range, rc::Rc};

    use gpui::{
        App, Bounds, InputHandler, InteractiveElement as _, IntoElement, KeyBinding, KeyDownEvent,
        Keystroke, ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext,
        UTF16Selection, canvas, div,
    };

    use super::*;

    gpui::actions!(focus_manager_test, [FocusToolbar]);

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";

    struct Dummy;

    impl Render for Dummy {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<'_, Self>) -> impl IntoElement {
            div()
        }
    }

    struct KeyboardHarness {
        _workspace: FocusHandle,
        panel: FocusHandle,
        toolbar: FocusHandle,
        _panel_registration: FocusRegistration,
        _toolbar_registration: FocusRegistration,
        manager: FocusManager,
        panel_token: FocusToken,
        toolbar_token: FocusToken,
    }

    impl KeyboardHarness {
        fn new(cx: &mut gpui::Context<'_, Self>) -> Self {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let toolbar = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let toolbar_registration = manager
                .register(&toolbar, panel_token, Some(project(PROJECT_A)), false)
                .unwrap();
            let toolbar_token = toolbar_registration.token();
            Self {
                _workspace: workspace,
                panel,
                toolbar,
                _panel_registration: panel_registration,
                _toolbar_registration: toolbar_registration,
                manager,
                panel_token,
                toolbar_token,
            }
        }
    }

    impl Render for KeyboardHarness {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<'_, Self>) -> impl IntoElement {
            div()
                .on_action(cx.listener(|this, _: &FocusToolbar, window, cx| {
                    this.manager
                        .request_focus(this.toolbar_token, window, cx)
                        .unwrap();
                }))
                .child(div().track_focus(&self.panel))
                .child(div().track_focus(&self.toolbar))
        }
    }

    struct ModalInputHandler;

    impl InputHandler for ModalInputHandler {
        fn selected_text_range(
            &mut self,
            _ignore_disabled_input: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<UTF16Selection> {
            None
        }

        fn marked_text_range(
            &mut self,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<Range<usize>> {
            None
        }

        fn text_for_range(
            &mut self,
            _range: Range<usize>,
            _adjusted_range: &mut Option<Range<usize>>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<String> {
            None
        }

        fn replace_text_in_range(
            &mut self,
            _range: Option<Range<usize>>,
            _text: &str,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            _range: Option<Range<usize>>,
            _text: &str,
            _selected_range: Option<Range<usize>>,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

        fn bounds_for_range(
            &mut self,
            _range: Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<Bounds<Pixels>> {
            None
        }

        fn character_index_for_point(
            &mut self,
            _point: Point<Pixels>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<usize> {
            None
        }
    }

    struct WindowControl {
        handle: FocusHandle,
        _registration: FocusRegistration,
    }

    struct WindowHarness {
        _workspace: FocusHandle,
        panel: FocusHandle,
        trigger: Option<WindowControl>,
        modal: FocusHandle,
        modal_child: Option<WindowControl>,
        modal_child_visible: bool,
        _panel_registration: FocusRegistration,
        _modal_registration: FocusRegistration,
        manager: FocusManager,
        panel_token: FocusToken,
        trigger_token: FocusToken,
        modal_token: FocusToken,
        modal_child_token: FocusToken,
        input_probe: Option<(FocusHandle, Rc<Cell<usize>>)>,
        focus_subscriptions: Option<FocusSubscriptions>,
    }

    impl WindowHarness {
        fn new(cx: &mut gpui::Context<'_, Self>) -> Self {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle().tab_stop(true).tab_index(0);
            let trigger = cx.focus_handle().tab_stop(true).tab_index(1);
            let modal = cx.focus_handle().tab_stop(true).tab_index(2);
            let modal_child = cx.focus_handle().tab_stop(true).tab_index(3);
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let trigger_registration = manager
                .register(&trigger, panel_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let trigger_token = trigger_registration.token();
            let modal_registration = manager
                .register(&modal, manager.workspace_token(), None, true)
                .unwrap();
            let modal_token = modal_registration.token();
            let modal_child_registration = manager
                .register(&modal_child, modal_token, None, false)
                .unwrap();
            let modal_child_token = modal_child_registration.token();
            Self {
                _workspace: workspace,
                panel,
                trigger: Some(WindowControl {
                    handle: trigger,
                    _registration: trigger_registration,
                }),
                modal,
                modal_child: Some(WindowControl {
                    handle: modal_child,
                    _registration: modal_child_registration,
                }),
                modal_child_visible: false,
                _panel_registration: panel_registration,
                _modal_registration: modal_registration,
                manager,
                panel_token,
                trigger_token,
                modal_token,
                modal_child_token,
                input_probe: None,
                focus_subscriptions: None,
            }
        }

        fn observe_focus(&mut self, window: &mut Window, cx: &mut gpui::Context<'_, Self>) {
            self.focus_subscriptions = Some(FocusSubscriptions::new(
                cx,
                window,
                |harness, window, cx| harness.manager.sync_window_focus(window, cx),
                |harness, window, cx| harness.manager.recover_focus_lost(window, cx),
            ));
        }

        fn open_modal_with_child(&mut self, window: &mut Window, cx: &mut gpui::Context<'_, Self>) {
            self.manager
                .activate_project(project(PROJECT_A), self.panel_token, window, cx)
                .unwrap();
            self.manager
                .open_modal(
                    self.panel_token,
                    self.panel_token,
                    self.modal_token,
                    window,
                    cx,
                )
                .unwrap();
            self.manager
                .set_visible(self.modal_child_token, true)
                .unwrap();
            self.modal_child_visible = true;
            self.manager
                .request_focus(self.modal_child_token, window, cx)
                .unwrap();
            self.observe_focus(window, cx);
        }
    }

    impl Render for WindowHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<'_, Self>) -> impl IntoElement {
            let input_probe = self.input_probe.as_ref().map(|(handle, registrations)| {
                let handle = handle.clone();
                let registrations = registrations.clone();
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| {
                        if handle.is_focused(window) {
                            window.handle_input(&handle, ModalInputHandler, cx);
                            registrations.set(registrations.get() + 1);
                        }
                    },
                )
                .size_full()
            });
            div()
                .size_full()
                .child(div().track_focus(&self.panel))
                .children(
                    self.trigger
                        .as_ref()
                        .map(|trigger| div().track_focus(&trigger.handle)),
                )
                .child(
                    div().track_focus(&self.modal).children(
                        self.modal_child
                            .as_ref()
                            .filter(|_| self.modal_child_visible)
                            .map(|child| div().track_focus(&child.handle)),
                    ),
                )
                .children(input_probe)
        }
    }

    fn project(text: &str) -> ProjectId {
        text.parse().unwrap()
    }

    /// 非模态操作栏出现不抢焦，真实键位经动作分派后仍可到达操作栏。
    #[gpui::test]
    fn focus_nonmodal_no_steal(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("ctrl-alt-t", FocusToolbar, None)]);
        });
        let (harness, window) = cx.add_window_view(|_, cx| KeyboardHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness
                    .manager
                    .present_nonmodal(harness.toolbar_token)
                    .unwrap();
                assert!(harness.panel.is_focused(window));
                assert!(!harness.toolbar.is_focused(window));
            });

            window.dispatch_event(
                gpui::PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse("ctrl-alt-t").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );

            harness.update(cx, |harness, _| {
                assert!(harness.toolbar.is_focused(window));
                assert!(!harness.panel.is_focused(window));
            });
        });
    }

    /// 模态约束焦点；关闭时沿触发控件、逻辑父链、独立面板默认点和工作区还焦。
    #[gpui::test]
    fn focus_modal_fallback_after_owner_destroy(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let container = cx.focus_handle();
            let trigger = cx.focus_handle();
            let panel_default = cx.focus_handle();
            let modal = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let container_registration = manager
                .register(&container, panel_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let container_token = container_registration.token();
            let trigger_registration = manager
                .register(&trigger, container_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let trigger_token = trigger_registration.token();
            let default_registration = manager
                .register(
                    &panel_default,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let default_token = default_registration.token();
            let modal_registration = manager
                .register(&modal, manager.workspace_token(), None, true)
                .unwrap();
            let modal_token = modal_registration.token();
            manager
                .activate_project(project(PROJECT_A), panel_token, window, cx)
                .unwrap();
            manager.request_focus(trigger_token, window, cx).unwrap();

            manager
                .open_modal(trigger_token, panel_token, modal_token, window, cx)
                .unwrap();
            assert!(modal.is_focused(window));
            assert_eq!(
                manager.request_focus(panel_token, window, cx),
                Err(FocusError::ModalActive)
            );
            assert!(modal.is_focused(window));
            assert_eq!(manager.close_modal(window, cx).unwrap(), trigger_token);
            assert!(trigger.is_focused(window));

            manager
                .open_modal(trigger_token, panel_token, modal_token, window, cx)
                .unwrap();
            drop(trigger);
            drop(trigger_registration);
            assert_eq!(manager.close_modal(window, cx).unwrap(), container_token);
            assert!(container.is_focused(window));
            manager.unregister(trigger_token).unwrap();

            manager
                .open_modal(container_token, panel_token, modal_token, window, cx)
                .unwrap();
            drop(container);
            drop(container_registration);
            manager.unregister(container_token).unwrap();
            assert_eq!(manager.close_modal(window, cx).unwrap(), panel_token);
            assert!(panel.is_focused(window));

            manager
                .open_modal(panel_token, default_token, modal_token, window, cx)
                .unwrap();
            drop(panel);
            drop(panel_registration);
            manager.unregister(panel_token).unwrap();
            assert_eq!(manager.close_modal(window, cx).unwrap(), default_token);
            assert!(panel_default.is_focused(window));

            manager
                .open_modal(default_token, default_token, modal_token, window, cx)
                .unwrap();
            drop(panel_default);
            drop(default_registration);
            manager.unregister(default_token).unwrap();
            assert_eq!(
                manager.close_modal(window, cx).unwrap(),
                manager.workspace_token()
            );
            assert!(workspace.is_focused(window));
        });
    }

    /// owner 已销毁但上一帧仍强持 FocusHandle 时，还焦必须跳过失效 owner。
    #[gpui::test]
    fn focus_modal_skips_destroyed_rendered_trigger(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness
                    .manager
                    .request_focus(harness.trigger_token, window, cx)
                    .unwrap();
            });
            window.draw(cx).clear(cx);
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .open_modal(
                        harness.trigger_token,
                        harness.panel_token,
                        harness.modal_token,
                        window,
                        cx,
                    )
                    .unwrap();
            });
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                let trigger = harness.trigger.take().unwrap();
                let weak_trigger = trigger.handle.downgrade();
                drop(trigger);
                assert!(
                    weak_trigger.upgrade().is_some(),
                    "上一渲染帧应保留 FocusHandle 强引用"
                );
                assert_eq!(
                    harness.manager.close_modal(window, cx).unwrap(),
                    harness.panel_token
                );
                assert!(harness.panel.is_focused(window));
            });
        });
    }

    /// GPUI 默认 Tab 与直接鼠标焦点路径都不得把实际焦点留在模态外。
    #[gpui::test]
    fn focus_modal_syncs_gpui_focus_back_inside(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness
                    .manager
                    .open_modal(
                        harness.panel_token,
                        harness.panel_token,
                        harness.modal_token,
                        window,
                        cx,
                    )
                    .unwrap();
                harness.observe_focus(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| window.focus_next(cx));
        window.run_until_parked();
        window.update(|window, cx| {
            assert!(harness.read(cx).modal.is_focused(window));
        });

        window.update(|window, cx| {
            let panel = harness.read(cx).panel.clone();
            panel.focus(window, cx);
        });
        window.run_until_parked();
        window.update(|window, cx| {
            assert!(harness.read(cx).modal.is_focused(window));
        });
    }

    /// 非模态 Tab 落入旧帧中的失效节点时，同步须恢复最近可用祖先。
    #[gpui::test]
    fn focus_sync_recovers_destroyed_nonmodal_tab_stop(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness.observe_focus(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, _| {
                let trigger = harness.trigger.take().unwrap();
                let weak_trigger = trigger.handle.downgrade();
                drop(trigger);
                assert!(
                    weak_trigger.upgrade().is_some(),
                    "上一渲染帧应保留 FocusHandle 强引用"
                );
            });
            window.focus_next(cx);
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).panel.is_focused(window));
        });
    }

    /// 非模态当前控件销毁并重绘后，句柄完全释放也须恢复父链焦点与输入。
    #[gpui::test]
    fn focus_sync_recovers_destroyed_nonmodal_after_redraw(cx: &mut TestAppContext) {
        let panel_input_registrations = Rc::new(Cell::new(0));
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness.input_probe =
                    Some((harness.panel.clone(), panel_input_registrations.clone()));
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness
                    .manager
                    .request_focus(harness.trigger_token, window, cx)
                    .unwrap();
                harness.observe_focus(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, _| {
                harness.trigger.take();
            });
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).panel.is_focused(window));
            assert_eq!(
                panel_input_registrations.get(),
                1,
                "非模态焦点恢复后未补帧注册输入处理器"
            );
        });
    }

    /// 主动 blur 不是渲染节点失效，非模态同步不得反向抢焦。
    #[gpui::test]
    fn focus_sync_preserves_explicit_blur(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness.observe_focus(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, _| window.blur());
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(window.focused(cx).is_none());
        });
    }

    /// 无重绘注销当前焦点后，显式同步须恢复本项目最后有效焦点。
    #[gpui::test]
    fn focus_sync_recovers_unregistered_focus(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                let trigger = harness.trigger.take().unwrap();
                trigger.handle.focus(window, cx);
                assert!(trigger.handle.is_focused(window));
                drop(trigger);
                harness.manager.unregister(harness.trigger_token).unwrap();

                harness.manager.sync_window_focus(window, cx);

                assert!(harness.panel.is_focused(window));
            });
        });
    }

    /// 未渲染控件在显式同步前完全释放时，仍须沿逻辑父链恢复。
    #[gpui::test]
    fn focus_sync_recovers_released_nonmodal_without_redraw(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                let handle = cx.focus_handle();
                let registration = harness
                    .manager
                    .register(&handle, harness.panel_token, Some(project(PROJECT_A)), true)
                    .unwrap();
                let token = registration.token();
                harness.manager.request_focus(token, window, cx).unwrap();
                let weak_handle = handle.downgrade();
                drop(registration);
                drop(handle);
                assert!(
                    weak_handle.upgrade().is_none(),
                    "测试要求无旧渲染帧保留 FocusHandle"
                );
                assert!(window.focused(cx).is_none());

                harness.manager.sync_window_focus(window, cx);

                assert!(harness.panel.is_focused(window));
            });
        });
    }

    /// 未渲染控件注销并完全释放后，显式同步仍须回退到有效工作区焦点。
    #[gpui::test]
    fn focus_sync_recovers_released_unregistered_without_redraw(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                let handle = cx.focus_handle();
                let registration = harness
                    .manager
                    .register(&handle, harness.panel_token, Some(project(PROJECT_A)), true)
                    .unwrap();
                let token = registration.token();
                harness.manager.request_focus(token, window, cx).unwrap();
                let weak_handle = handle.downgrade();
                drop(registration);
                drop(handle);
                harness.manager.unregister(token).unwrap();
                assert!(
                    weak_handle.upgrade().is_none(),
                    "测试要求无旧渲染帧保留 FocusHandle"
                );
                assert!(window.focused(cx).is_none());

                harness.manager.sync_window_focus(window, cx);

                assert!(harness._workspace.is_focused(window));
            });
        });
    }

    /// 无重绘后的显式同步也须补帧，确保恢复焦点同时恢复文本输入。
    #[gpui::test]
    fn focus_explicit_sync_restores_input_handler(cx: &mut TestAppContext) {
        let panel_input_registrations = Rc::new(Cell::new(0));
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness.input_probe =
                    Some((harness.panel.clone(), panel_input_registrations.clone()));
                harness
                    .manager
                    .activate_project(project(PROJECT_A), harness.panel_token, window, cx)
                    .unwrap();
                harness.observe_focus(window, cx);
            });
            window.draw(cx).clear(cx);
            assert_eq!(panel_input_registrations.get(), 1);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                let trigger = harness.trigger.take().unwrap();
                trigger.handle.focus(window, cx);
                drop(trigger);
                harness.manager.unregister(harness.trigger_token).unwrap();
                harness.manager.sync_window_focus(window, cx);
                assert!(harness.panel.is_focused(window));
                assert_eq!(panel_input_registrations.get(), 1);
            });
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).panel.is_focused(window));
            assert_eq!(
                panel_input_registrations.get(),
                2,
                "显式同步恢复焦点后未补帧注册输入处理器"
            );
        });
    }

    /// 已聚焦模态子控件销毁后，焦点丢失通道须恢复仍存活的模态根。
    #[gpui::test]
    fn focus_modal_restores_after_focused_child_destroyed(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness.open_modal_with_child(window, cx);
                assert!(
                    harness
                        .modal_child
                        .as_ref()
                        .unwrap()
                        .handle
                        .is_focused(window)
                );
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, _| {
                harness.modal_child.take();
            });
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).modal.is_focused(window));
        });
    }

    /// 模态焦点丢失后的补帧必须真实重建输入处理器，而不只恢复 FocusId。
    #[gpui::test]
    fn focus_modal_restores_input_handler_after_focus_lost(cx: &mut TestAppContext) {
        let modal_input_registrations = Rc::new(Cell::new(0));
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness.input_probe =
                    Some((harness.modal.clone(), modal_input_registrations.clone()));
                harness.open_modal_with_child(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, _| {
                harness.modal_child.take();
            });
            window.draw(cx).clear(cx);
            assert!(harness.read(cx).modal.is_focused(window));
            assert_eq!(
                modal_input_registrations.get(),
                0,
                "当次绘制早于焦点丢失回调，不应倒填输入处理器"
            );
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).modal.is_focused(window));
            assert_eq!(
                modal_input_registrations.get(),
                1,
                "恢复模态焦点后未补帧注册输入处理器"
            );
        });
    }

    /// 已聚焦模态子控件隐藏后，焦点丢失通道须恢复仍存活的模态根。
    #[gpui::test]
    fn focus_modal_restores_after_focused_child_hidden(cx: &mut TestAppContext) {
        let (harness, window) = cx.add_window_view(|_, cx| WindowHarness::new(cx));
        window.update(|window, cx| {
            harness.update(cx, |harness, cx| {
                harness.open_modal_with_child(window, cx);
            });
            window.draw(cx).clear(cx);
        });

        window.update(|window, cx| {
            harness.update(cx, |harness, _| {
                harness.modal_child_visible = false;
                harness
                    .manager
                    .set_visible(harness.modal_child_token, false)
                    .unwrap();
            });
            window.draw(cx).clear(cx);
        });
        window.run_until_parked();

        window.update(|window, cx| {
            assert!(harness.read(cx).modal.is_focused(window));
        });
    }

    /// 模态子树内可以移动焦点，模态外项目节点仍被拒绝。
    #[gpui::test]
    fn focus_modal_allows_descendants_and_blocks_outside(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let modal = cx.focus_handle();
            let safe_action = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let modal_registration = manager
                .register(&modal, manager.workspace_token(), None, true)
                .unwrap();
            let modal_token = modal_registration.token();
            let safe_action_registration = manager
                .register(&safe_action, modal_token, None, true)
                .unwrap();
            let safe_action_token = safe_action_registration.token();
            manager
                .activate_project(project(PROJECT_A), panel_token, window, cx)
                .unwrap();
            manager
                .open_modal(panel_token, panel_token, modal_token, window, cx)
                .unwrap();

            assert_eq!(manager.request_focus(safe_action_token, window, cx), Ok(()));
            assert!(safe_action.is_focused(window));
            assert_eq!(
                manager.request_focus(panel_token, window, cx),
                Err(FocusError::ModalActive)
            );
            assert!(safe_action.is_focused(window));
            assert_eq!(manager.close_modal(window, cx).unwrap(), panel_token);
            assert!(panel.is_focused(window));
        });
    }

    /// 项目切换只恢复本项目最后焦点；隐藏项目 token 不得抢焦。
    #[gpui::test]
    fn focus_project_switch_restores_memory(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let a_first = cx.focus_handle();
            let a_second = cx.focus_handle();
            let b_first = cx.focus_handle();
            let b_second = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let a_first_registration = manager
                .register(
                    &a_first,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let a_first_token = a_first_registration.token();
            let a_second_registration = manager
                .register(
                    &a_second,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let a_second_token = a_second_registration.token();
            let b_first_registration = manager
                .register(
                    &b_first,
                    manager.workspace_token(),
                    Some(project(PROJECT_B)),
                    true,
                )
                .unwrap();
            let b_first_token = b_first_registration.token();
            let b_second_registration = manager
                .register(
                    &b_second,
                    manager.workspace_token(),
                    Some(project(PROJECT_B)),
                    true,
                )
                .unwrap();
            let b_second_token = b_second_registration.token();

            assert_eq!(
                manager
                    .activate_project(project(PROJECT_A), a_first_token, window, cx)
                    .unwrap(),
                a_first_token
            );
            assert!(a_first.is_focused(window));
            manager.request_focus(a_second_token, window, cx).unwrap();
            assert_eq!(
                manager
                    .activate_project(project(PROJECT_B), b_first_token, window, cx)
                    .unwrap(),
                b_first_token
            );
            assert!(b_first.is_focused(window));
            assert_eq!(
                manager.request_focus(a_second_token, window, cx),
                Err(FocusError::Unavailable)
            );
            assert!(b_first.is_focused(window));
            manager.request_focus(b_second_token, window, cx).unwrap();

            assert_eq!(
                manager
                    .activate_project(project(PROJECT_A), a_first_token, window, cx)
                    .unwrap(),
                a_second_token
            );
            assert!(a_second.is_focused(window));
            assert_eq!(
                manager
                    .activate_project(project(PROJECT_B), b_first_token, window, cx)
                    .unwrap(),
                b_second_token
            );
            assert!(b_second.is_focused(window));
        });
    }

    /// 鼠标等 GPUI 原生入口产生的实际焦点，必须成为切走前的项目记忆。
    #[gpui::test]
    fn focus_project_switch_snapshots_actual_focus(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let a_default = cx.focus_handle();
            let a_actual = cx.focus_handle();
            let b_default = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let a_default_registration = manager
                .register(
                    &a_default,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let a_default_token = a_default_registration.token();
            let a_actual_registration = manager
                .register(
                    &a_actual,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let a_actual_token = a_actual_registration.token();
            let b_default_registration = manager
                .register(
                    &b_default,
                    manager.workspace_token(),
                    Some(project(PROJECT_B)),
                    true,
                )
                .unwrap();
            let b_default_token = b_default_registration.token();
            manager
                .activate_project(project(PROJECT_A), a_default_token, window, cx)
                .unwrap();
            a_actual.focus(window, cx);
            assert!(a_actual.is_focused(window));

            manager
                .activate_project(project(PROJECT_B), b_default_token, window, cx)
                .unwrap();
            assert_eq!(
                manager
                    .activate_project(project(PROJECT_A), a_default_token, window, cx)
                    .unwrap(),
                a_actual_token
            );
            assert!(a_actual.is_focused(window));
        });
    }

    /// 普通还焦逐个跳过隐藏、销毁与已注销 token，最终落到可用面板。
    #[gpui::test]
    fn focus_restore_skips_unavailable_tokens(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let container = cx.focus_handle();
            let leaf = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let container_registration = manager
                .register(&container, panel_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let container_token = container_registration.token();
            let leaf_registration = manager
                .register(&leaf, container_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let leaf_token = leaf_registration.token();
            manager
                .activate_project(project(PROJECT_A), panel_token, window, cx)
                .unwrap();

            manager.set_visible(container_token, false).unwrap();
            assert_eq!(
                manager.request_focus(leaf_token, window, cx),
                Err(FocusError::Unavailable)
            );
            assert!(panel.is_focused(window));
            assert_eq!(
                manager
                    .restore_focus(&[leaf_token, panel_token], window, cx)
                    .unwrap(),
                panel_token
            );

            manager.set_visible(container_token, true).unwrap();
            drop(leaf);
            drop(leaf_registration);
            assert_eq!(
                manager.request_focus(leaf_token, window, cx),
                Err(FocusError::Unavailable)
            );
            assert_eq!(
                manager
                    .restore_focus(&[leaf_token, panel_token], window, cx)
                    .unwrap(),
                panel_token
            );

            manager.unregister(leaf_token).unwrap();
            assert_eq!(
                manager.request_focus(leaf_token, window, cx),
                Err(FocusError::UnknownToken)
            );
            assert_eq!(
                manager
                    .restore_focus(&[leaf_token, panel_token], window, cx)
                    .unwrap(),
                panel_token
            );
            assert_eq!(
                manager.restore_focus(&[], window, cx),
                Err(FocusError::NoFocusTarget)
            );
        });
    }

    /// 注销逻辑父节点时，幸存子节点接到祖父节点而不是变成不可聚焦孤儿。
    #[gpui::test]
    fn focus_unregister_reparents_children(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let container = cx.focus_handle();
            let leaf = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();
            let container_registration = manager
                .register(&container, panel_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let container_token = container_registration.token();
            let leaf_registration = manager
                .register(&leaf, container_token, Some(project(PROJECT_A)), true)
                .unwrap();
            let leaf_token = leaf_registration.token();
            manager
                .activate_project(project(PROJECT_A), panel_token, window, cx)
                .unwrap();

            manager.unregister(container_token).unwrap();
            assert_eq!(manager.request_focus(leaf_token, window, cx), Ok(()));
            assert!(leaf.is_focused(window));
        });
    }

    /// 重复句柄、跨项目父链与已失效父 token 均在注册边界拒绝。
    #[gpui::test]
    fn focus_registration_rejects_invalid_relationships(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|_, cx| {
            let workspace = cx.focus_handle();
            let panel = cx.focus_handle();
            let other = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let panel_registration = manager
                .register(
                    &panel,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let panel_token = panel_registration.token();

            assert_eq!(
                manager
                    .register(&panel, panel_token, Some(project(PROJECT_A)), true)
                    .unwrap_err(),
                FocusError::DuplicateHandle
            );
            assert_eq!(
                manager
                    .register(&other, panel_token, Some(project(PROJECT_B)), true)
                    .unwrap_err(),
                FocusError::ScopeMismatch
            );
            manager.unregister(panel_token).unwrap();
            assert_eq!(
                manager
                    .register(&other, panel_token, Some(project(PROJECT_A)), true)
                    .unwrap_err(),
                FocusError::InvalidParent
            );
            assert_eq!(
                manager.unregister(manager.workspace_token()),
                Err(FocusError::WorkspaceRoot)
            );
        });
    }

    /// 实例代次只增不减；注销后旧 token 不得寻址新实例，耗尽时禁止回绕。
    #[gpui::test]
    fn focus_token_generation_never_reuses_or_wraps(cx: &mut TestAppContext) {
        let (_, window) = cx.add_window_view(|_, _| Dummy);
        window.update(|window, cx| {
            let workspace = cx.focus_handle();
            let first = cx.focus_handle();
            let second = cx.focus_handle();
            let mut manager = FocusManager::new(&workspace);
            let first_registration = manager
                .register(
                    &first,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let first_token = first_registration.token();
            manager.unregister(first_token).unwrap();
            let second_registration = manager
                .register(
                    &second,
                    manager.workspace_token(),
                    Some(project(PROJECT_A)),
                    true,
                )
                .unwrap();
            let second_token = second_registration.token();
            assert!(second_token.generation > first_token.generation);
            manager
                .activate_project(project(PROJECT_A), second_token, window, cx)
                .unwrap();

            assert_eq!(
                manager.request_focus(first_token, window, cx),
                Err(FocusError::UnknownToken)
            );
            assert!(second.is_focused(window));
            manager.next_generation = u64::MAX;
            let third = cx.focus_handle();
            assert_eq!(
                manager
                    .register(
                        &third,
                        manager.workspace_token(),
                        Some(project(PROJECT_A)),
                        true,
                    )
                    .unwrap_err(),
                FocusError::GenerationExhausted
            );
            assert!(second.is_focused(window));
        });
    }
}
