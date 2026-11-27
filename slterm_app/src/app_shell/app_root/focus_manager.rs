//! GPUI 焦点所有权：弱句柄、实例代次、项目记忆与模态还焦链。

#[cfg(all(test, feature = "gpui-test-support"))]
use std::cell::Cell;
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
    #[allow(dead_code)] // 仅凭持有关系维持节点 owner 存活，不提供读取入口。
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

#[cfg(all(test, feature = "gpui-test-support"))]
#[derive(Default)]
struct FocusFaults {
    fail_next_project_activation_precheck: Cell<bool>,
    fail_next_project_activation_after_scope_change: Cell<bool>,
    fail_terminal_request_after_activation: Cell<bool>,
    fail_next_request_focus: Cell<bool>,
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
    #[cfg(all(test, feature = "gpui-test-support"))]
    faults: FocusFaults,
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
            #[cfg(all(test, feature = "gpui-test-support"))]
            faults: FocusFaults::default(),
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
        #[cfg(all(test, feature = "gpui-test-support"))]
        if self.faults.fail_next_request_focus.replace(false) {
            return Err(FocusError::NoFocusTarget);
        }
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
        let focused_handle = window.focused(cx);
        let focused = focused_handle.as_ref().map(|handle| handle.downgrade());
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
            // 文本框等子控件自身不是注册节点，但渲染树中的可聚焦祖先能证明其作用域。
            if focused_handle
                .as_ref()
                .is_some_and(|focused| self.contained_by(focused, Some(modal), window).is_some())
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
        } else if let Some(focused) = focused_handle.as_ref() {
            if let Some(token) = self.contained_by(focused, None, window) {
                // 焦点落在已渲染子控件内：保留真实焦点，并把主语视图记为项目焦点记忆。
                if let Some(project) = self.nodes.get(&token).and_then(|node| node.project) {
                    self.last_focus.insert(project, token);
                }
            } else {
                self.recover_unregistered_focus(window, cx);
            }
        } else {
            self.recover_lost_focus(window, cx);
        }
    }

    /// 在渲染树中查找包含焦点控件且满足作用域的可聚焦注册节点。
    fn contained_by(
        &self,
        focused: &FocusHandle,
        within: Option<FocusToken>,
        window: &Window,
    ) -> Option<FocusToken> {
        self.nodes.iter().find_map(|(token, node)| {
            let in_scope = within.is_none_or(|ancestor| self.is_within(*token, ancestor));
            let contains = node
                .handle
                .upgrade()
                .is_some_and(|handle| handle.contains(focused, window));
            (in_scope && contains && self.is_focusable(*token)).then_some(*token)
        })
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

    /// 显式浮层记录打开前的还焦起点；调用方须先同步真实窗口焦点。
    pub fn observed_focus_token(&self) -> Option<FocusToken> {
        self.observed_focus.as_ref().and_then(|focus| focus.token)
    }

    /// 启动恢复只建立项目作用域，不送焦、不覆盖运行期焦点记忆。
    pub fn restore_active_project(&mut self, project: Option<ProjectId>) {
        self.active_project = project;
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn state_for_test(&self) -> (Option<ProjectId>, BTreeMap<FocusToken, bool>) {
        (
            self.active_project,
            self.nodes
                .iter()
                .map(|(token, node)| (*token, node.visible))
                .collect(),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn fail_next_request_focus_for_test(&mut self) {
        self.faults.fail_next_request_focus.set(true);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn fail_next_project_activation_precheck_for_test(&mut self) {
        self.faults.fail_next_project_activation_precheck.set(true);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn fail_next_project_activation_after_scope_change_for_test(&mut self) {
        self.faults
            .fail_next_project_activation_after_scope_change
            .set(true);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn fail_terminal_request_focus_after_activation_for_test(&mut self) {
        self.faults.fail_terminal_request_after_activation.set(true);
    }

    pub const fn modal_is_active(&self) -> bool {
        self.modal.is_some()
    }

    /// 激活或懒启动前的无副作用预检；模态期间禁止项目切换。
    pub fn ensure_activation_allowed(&self) -> Result<(), FocusError> {
        if self.modal.is_some() {
            Err(FocusError::ModalActive)
        } else {
            Ok(())
        }
    }

    /// 项目激活前的纯校验；不得同步、修改或请求实际焦点。
    pub fn ensure_project_activation_allowed(
        &self,
        project: ProjectId,
        default: FocusToken,
    ) -> Result<(), FocusError> {
        self.ensure_activation_allowed()?;
        #[cfg(all(test, feature = "gpui-test-support"))]
        if self
            .faults
            .fail_next_project_activation_precheck
            .replace(false)
        {
            return Err(FocusError::NoFocusTarget);
        }
        let default_project = self
            .nodes
            .get(&default)
            .map(|node| node.project)
            .ok_or(FocusError::UnknownToken)?;
        if default_project != Some(project) {
            return Err(FocusError::ScopeMismatch);
        }
        Ok(())
    }

    pub fn activate_project(
        &mut self,
        project: ProjectId,
        default: FocusToken,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<FocusToken, FocusError> {
        self.ensure_project_activation_allowed(project, default)?;
        self.sync_window_focus(window, cx);
        self.active_project = Some(project);
        #[cfg(all(test, feature = "gpui-test-support"))]
        if self
            .faults
            .fail_next_project_activation_after_scope_change
            .replace(false)
        {
            return Err(FocusError::NoFocusTarget);
        }
        let mut candidates = Vec::with_capacity(3);
        candidates.extend(self.last_focus.get(&project));
        candidates.push(default);
        candidates.push(self.workspace);
        let focused = self.focus_first(&candidates, window, cx);
        #[cfg(all(test, feature = "gpui-test-support"))]
        if focused.is_ok()
            && self
                .faults
                .fail_terminal_request_after_activation
                .replace(false)
        {
            self.faults.fail_next_request_focus.set(true);
        }
        focused
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
#[path = "focus_manager_tests.rs"]
mod focus_manager_tests;
