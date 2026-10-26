//! 侧栏按钮组织、双槽与可见性的纯状态机。

use std::collections::BTreeSet;
use std::fmt;

use slterm_workspace::SideViewId;

/// 侧栏上下槽位。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SideSlot {
    Upper,
    Lower,
}

impl SideSlot {
    pub const ALL: [Self; 2] = [Self::Upper, Self::Lower];

    const fn index(self) -> usize {
        match self {
            Self::Upper => 0,
            Self::Lower => 1,
        }
    }
}

/// 按钮在所属组内的确定移动方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonMovement {
    Up,
    Down,
    Top,
    Bottom,
}

/// 一次状态转换产生的实例、焦点与实际可见性效果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarTransition {
    /// 需要新建并恢复状态的视图。
    pub opened: Vec<SideViewId>,
    /// 需要取消私有任务并销毁的视图。
    pub closed: Vec<SideViewId>,
    /// 需要接收焦点的视图。
    pub focus: Option<SideViewId>,
    pub was_visible: bool,
    pub is_visible: bool,
}

impl SidebarTransition {
    pub fn visibility_changed(&self) -> bool {
        self.was_visible != self.is_visible
    }
}

/// 侧栏请求无法应用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarError {
    HiddenButton,
    InvalidButtonIndex,
    InvalidRatio,
}

impl fmt::Display for SidebarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::HiddenButton => "侧栏按钮已隐藏",
            Self::InvalidButtonIndex => "侧栏按钮目标索引非法",
            Self::InvalidRatio => "侧栏分割比例不是有限数值",
        })
    }
}

impl std::error::Error for SidebarError {}

/// 应用级侧栏的唯一组织状态；不包含视图运行实例或各视图的私有状态。
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarState {
    buttons: [Vec<SideViewId>; 2],
    hidden_buttons: BTreeSet<SideViewId>,
    open_views: [Option<SideViewId>; 2],
    visible_intent: bool,
    width: u16,
    ratio: f32,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            buttons: [SideViewId::ALL.to_vec(), Vec::new()],
            hidden_buttons: BTreeSet::new(),
            open_views: [Some(SideViewId::Nav), None],
            visible_intent: true,
            width: Self::DEFAULT_WIDTH,
            ratio: Self::DEFAULT_RATIO,
        }
    }
}

impl SidebarState {
    pub const MIN_WIDTH: u16 = 160;
    pub const MAX_WIDTH: u16 = 720;
    pub const DEFAULT_WIDTH: u16 = 250;
    pub const MIN_RATIO: f32 = 0.1;
    pub const MAX_RATIO: f32 = 0.9;
    pub const DEFAULT_RATIO: f32 = 0.5;

    pub fn buttons(&self, slot: SideSlot) -> &[SideViewId] {
        &self.buttons[slot.index()]
    }

    /// 按上组到下组的活动栏顺序返回可见按钮。
    pub fn visible_buttons(&self) -> Vec<SideViewId> {
        self.buttons
            .iter()
            .flatten()
            .copied()
            .filter(|view| !self.hidden_buttons.contains(view))
            .collect()
    }

    pub fn button_slot(&self, view: SideViewId) -> SideSlot {
        SideSlot::ALL
            .into_iter()
            .find(|slot| self.buttons[slot.index()].contains(&view))
            .expect("封闭侧栏视图必须在按钮注册表中")
    }

    pub fn button_is_visible(&self, view: SideViewId) -> bool {
        !self.hidden_buttons.contains(&view)
    }

    pub fn open_view(&self, slot: SideSlot) -> Option<SideViewId> {
        self.open_views[slot.index()]
    }

    pub fn open_slot(&self, view: SideViewId) -> Option<SideSlot> {
        SideSlot::ALL
            .into_iter()
            .find(|slot| self.open_views[slot.index()] == Some(view))
    }

    /// 用户是否希望显示侧栏；两槽皆空时该意图与实际可见性分离。
    pub const fn visible_intent(&self) -> bool {
        self.visible_intent
    }

    pub fn is_visible(&self) -> bool {
        self.visible_intent && self.open_views.iter().any(Option::is_some)
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn ratio(&self) -> f32 {
        self.ratio
    }

    /// 鼠标只激活可见按钮；已打开且实际可见的视图再次点击即关闭。
    pub fn click_button(&mut self, view: SideViewId) -> Result<SidebarTransition, SidebarError> {
        if !self.button_is_visible(view) {
            return Err(SidebarError::HiddenButton);
        }
        let was_visible = self.is_visible();
        let mut opened = Vec::new();
        let mut closed = Vec::new();
        let mut focus = None;
        if self.open_slot(view).is_some() && was_visible {
            self.clear_open_view(view);
            closed.push(view);
        } else {
            if self.open_slot(view).is_none() {
                if let Some(replaced) = self.open_in_button_slot(view) {
                    closed.push(replaced);
                }
                opened.push(view);
            }
            self.visible_intent = true;
            focus = Some(view);
        }
        Ok(self.transition_with(was_visible, opened, closed, focus))
    }

    /// 快捷键可恢复隐藏按钮；`focused` 表示操作前该视图是否已聚焦。
    pub fn activate_shortcut(&mut self, view: SideViewId, focused: bool) -> SidebarTransition {
        let was_visible = self.is_visible();
        self.hidden_buttons.remove(&view);
        let mut opened = Vec::new();
        let mut closed = Vec::new();
        let mut focus = None;
        if self.open_slot(view).is_none() {
            if let Some(replaced) = self.open_in_button_slot(view) {
                closed.push(replaced);
            }
            opened.push(view);
            self.visible_intent = true;
            focus = Some(view);
        } else if !was_visible {
            self.visible_intent = true;
            focus = Some(view);
        } else if !focused {
            focus = Some(view);
        } else {
            self.clear_open_view(view);
            closed.push(view);
        }
        self.transition_with(was_visible, opened, closed, focus)
    }

    /// 显式隐藏保留实例；空槽显示时优先导航树，其次活动栏首个可见按钮。
    pub fn toggle_sidebar(&mut self) -> SidebarTransition {
        let was_visible = self.is_visible();
        let mut opened = Vec::new();
        if was_visible {
            self.visible_intent = false;
        } else {
            self.visible_intent = true;
            if self.open_views.iter().all(Option::is_none) {
                let fallback = if self.button_is_visible(SideViewId::Nav) {
                    Some(SideViewId::Nav)
                } else {
                    self.visible_buttons().first().copied()
                };
                if let Some(view) = fallback {
                    let replaced = self.open_in_button_slot(view);
                    debug_assert_eq!(replaced, None);
                    opened.push(view);
                }
            }
        }
        self.transition_with(was_visible, opened, Vec::new(), None)
    }

    /// 关闭只清理对应槽位，不改变用户的显示意图。
    pub fn close_view(&mut self, view: SideViewId) -> SidebarTransition {
        let was_visible = self.is_visible();
        let closed = if self.clear_open_view(view) {
            vec![view]
        } else {
            Vec::new()
        };
        self.transition_with(was_visible, Vec::new(), closed, None)
    }

    /// 隐藏按钮不影响对应视图或槽位。
    pub fn set_button_hidden(&mut self, view: SideViewId, hidden: bool) {
        if hidden {
            self.hidden_buttons.insert(view);
        } else {
            self.hidden_buttons.remove(&view);
        }
    }

    /// 同组排序只改变按钮顺序。
    pub fn reorder_button(&mut self, view: SideViewId, movement: ButtonMovement) {
        let slot = self.button_slot(view);
        let group = &mut self.buttons[slot.index()];
        let index = group
            .iter()
            .position(|candidate| *candidate == view)
            .expect("按钮必须属于其所在组");
        let target = match movement {
            ButtonMovement::Up => index.saturating_sub(1),
            ButtonMovement::Down => (index + 1).min(group.len() - 1),
            ButtonMovement::Top => 0,
            ButtonMovement::Bottom => group.len() - 1,
        };
        let button = group.remove(index);
        group.insert(target, button);
    }

    /// 跨组移动使用目标组插入索引，同组移动使用移除后的最终索引。
    /// 已打开视图迁往目标槽；目标槽非空时交换双方视图而不销毁实例。
    pub fn move_button_to_slot(
        &mut self,
        view: SideViewId,
        target: SideSlot,
        target_index: usize,
    ) -> Result<SidebarTransition, SidebarError> {
        let source = self.button_slot(view);
        let source_index = self.buttons[source.index()]
            .iter()
            .position(|candidate| *candidate == view)
            .expect("按钮必须属于其所在组");
        let valid_index = if source == target {
            target_index < self.buttons[target.index()].len()
        } else {
            target_index <= self.buttons[target.index()].len()
        };
        if !valid_index {
            return Err(SidebarError::InvalidButtonIndex);
        }

        let was_visible = self.is_visible();
        let button = self.buttons[source.index()].remove(source_index);
        self.buttons[target.index()].insert(target_index, button);
        if source != target
            && let Some(open_source) = self.open_slot(view)
            && open_source != target
        {
            self.open_views.swap(open_source.index(), target.index());
        }
        Ok(self.transition(was_visible))
    }

    /// 仅恢复各组内注册顺序，保留分组、显隐和打开视图。
    pub fn reset_button_order(&mut self) {
        for group in &mut self.buttons {
            group.sort_by_key(|view| Self::registration_index(*view));
        }
    }

    /// 恢复按钮默认值；打开视图按注册顺序只保留一个，其余走关闭效果。
    pub fn restore_default_buttons(&mut self) -> SidebarTransition {
        let was_visible = self.is_visible();
        let keep = SideViewId::ALL
            .into_iter()
            .find(|view| self.open_slot(*view).is_some());
        let closed = SideViewId::ALL
            .into_iter()
            .filter(|view| Some(*view) != keep && self.open_slot(*view).is_some())
            .collect();
        self.buttons = [SideViewId::ALL.to_vec(), Vec::new()];
        self.hidden_buttons.clear();
        self.open_views = [keep, None];
        self.transition_with(was_visible, Vec::new(), closed, None)
    }

    pub fn set_width(&mut self, width: u32) {
        self.width = width.clamp(u32::from(Self::MIN_WIDTH), u32::from(Self::MAX_WIDTH)) as u16;
    }

    pub fn adjust_width(&mut self, delta: i32) {
        let width = i64::from(self.width) + i64::from(delta);
        self.width = width.clamp(i64::from(Self::MIN_WIDTH), i64::from(Self::MAX_WIDTH)) as u16;
    }

    pub fn reset_width(&mut self) {
        self.width = Self::DEFAULT_WIDTH;
    }

    pub fn set_ratio(&mut self, ratio: f32) -> Result<(), SidebarError> {
        if !ratio.is_finite() {
            return Err(SidebarError::InvalidRatio);
        }
        self.ratio = ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO);
        Ok(())
    }

    pub fn adjust_ratio(&mut self, delta: f32) -> Result<(), SidebarError> {
        self.set_ratio(self.ratio + delta)
    }

    pub fn reset_ratio(&mut self) {
        self.ratio = Self::DEFAULT_RATIO;
    }

    fn clear_open_view(&mut self, view: SideViewId) -> bool {
        if let Some(slot) = self.open_slot(view) {
            self.open_views[slot.index()] = None;
            true
        } else {
            false
        }
    }

    fn open_in_button_slot(&mut self, view: SideViewId) -> Option<SideViewId> {
        debug_assert!(self.open_slot(view).is_none());
        let slot = self.button_slot(view);
        self.open_views[slot.index()].replace(view)
    }

    fn registration_index(view: SideViewId) -> usize {
        SideViewId::ALL
            .iter()
            .position(|candidate| *candidate == view)
            .expect("侧栏视图必须有注册顺序")
    }

    fn transition(&self, was_visible: bool) -> SidebarTransition {
        self.transition_with(was_visible, Vec::new(), Vec::new(), None)
    }

    fn transition_with(
        &self,
        was_visible: bool,
        opened: Vec<SideViewId>,
        closed: Vec<SideViewId>,
        focus: Option<SideViewId>,
    ) -> SidebarTransition {
        SidebarTransition {
            opened,
            closed,
            focus,
            was_visible,
            is_visible: self.is_visible(),
        }
    }
}
