//! 冲突三栏合并 tab:base/ours/theirs 只读对照 + 采用版本动作。
//!
//! 数据源 = [`crate::git::conflict_stages`](`:1:/:2:/:3:` 有界读取)。
//! 动作链:本视图发 [`MergeTabEvent`] → `CodeTabView` 转 `CodeTabEvent` →
//! workspace 先做脏文件门控(同路径打开且未保存的 Code tab),再让本视图
//! 后台原子写盘;写盘成功后 workspace 刷新该文件 gutter/树快照。
//!
//! 三栏渲染复用 `diff_tab.rs` 的只读 pane 形态,但在本模块自持实现,
//! 不修改 diff 双栏视图。截断/非 UTF-8 stage 不允许采用(写回会损坏文件),
//! 只能回终端解决。

use std::io;
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Sizable as _, button::Button};

use crate::app_shell::config::Settings;
use crate::app_shell::editor_theme::EditorThemeTokens;
use crate::git::{self, ConflictStages};
use crate::i18n::{Message, UiLanguage};

/// 采用哪一方的版本。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MergeSide {
    Ours,
    Theirs,
}

impl MergeSide {
    /// 三栏/数组下标(= git stage 序 - 1)。
    fn index(self) -> usize {
        match self {
            Self::Ours => 1,
            Self::Theirs => 2,
        }
    }
}

/// merge tab 对外事件:采用动作与写盘完成由 CodeTabView 转发给工作区。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MergeTabEvent {
    /// 请求采用某一侧(工作区做脏文件门控后放行写盘)。
    Adopt(MergeSide),
    /// 请求在工作区编辑器中打开同一文件。
    OpenInEditor,
    /// 已成功写入工作区文件(工作区刷新 gutter/树快照)。
    Written,
}

/// 三栏展示状态(纯投影;缺失与截断在 UI 上有独立占位)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageStatus {
    Present,
    Missing,
    Truncated,
}

/// stage 存在性 + 截断位 → 展示状态。
pub(crate) fn stage_status(stage: Option<&str>, truncated: bool) -> StageStatus {
    match stage {
        None => StageStatus::Missing,
        Some(_) if truncated => StageStatus::Truncated,
        Some(_) => StageStatus::Present,
    }
}

/// 采用动作的拒绝原因(纯决策,供视图与用例共用)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdoptBlock {
    MissingStage,
    TruncatedStage,
    InvalidEncoding,
    DirtyDraft,
}

/// 采用决策:先判 stage 可用性,再判同路径未保存草稿。
pub(crate) fn adopt_block_reason(
    stage: Option<&str>,
    truncated: bool,
    valid_utf8: bool,
    dirty_draft: bool,
) -> Option<AdoptBlock> {
    if stage.is_none() {
        return Some(AdoptBlock::MissingStage);
    }
    if truncated {
        return Some(AdoptBlock::TruncatedStage);
    }
    if !valid_utf8 {
        return Some(AdoptBlock::InvalidEncoding);
    }
    if dirty_draft {
        return Some(AdoptBlock::DirtyDraft);
    }
    None
}

fn block_message(block: AdoptBlock) -> Message {
    match block {
        AdoptBlock::MissingStage => Message::MergeMissingStage,
        AdoptBlock::TruncatedStage => Message::MergeErrorTruncated,
        AdoptBlock::InvalidEncoding => Message::MergeErrorInvalidEncoding,
        AdoptBlock::DirtyDraft => Message::MergeDirtyRejected,
    }
}

fn stage_at(stages: &ConflictStages, index: usize) -> Option<&String> {
    match index {
        0 => stages.base.as_ref(),
        1 => stages.ours.as_ref(),
        _ => stages.theirs.as_ref(),
    }
}

/// 冲突三栏视图:三栏只读输入 + 动作条 + 载入/错误提示。
pub struct MergeTabView {
    pub title: String,
    path: PathBuf,
    base_input: Entity<InputState>,
    ours_input: Entity<InputState>,
    theirs_input: Entity<InputState>,
    stages: Option<ConflictStages>,
    loading: bool,
    error: Option<String>,
    notice: Option<String>,
    /// 写盘在途:防重复点击导致并发替换。
    writing: bool,
    focus: FocusHandle,
    _subscriptions: [gpui::Subscription; 3],
}

impl EventEmitter<MergeTabEvent> for MergeTabView {}

impl Focusable for MergeTabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl MergeTabView {
    /// 打开冲突文件:stage 读取全部在后台任务。
    pub fn open(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let language = cx.global::<Settings>().language;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let title = language.format(Message::MergeTabTitle, &[("name", &name)]);
        let base_input = cx.new(|cx| Self::new_pane(window, cx));
        let ours_input = cx.new(|cx| Self::new_pane(window, cx));
        let theirs_input = cx.new(|cx| Self::new_pane(window, cx));
        let base_subscription = cx.subscribe_in(
            &base_input,
            window,
            |view: &mut Self, _, event: &InputEvent, _, cx| {
                // 只读臂:程序化 set_value 不触发 Change;键入路径不应存在。
                if matches!(event, InputEvent::Change) {
                    let _ = view;
                    cx.notify();
                }
            },
        );
        let ours_subscription = cx.subscribe_in(
            &ours_input,
            window,
            |view: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let _ = view;
                    cx.notify();
                }
            },
        );
        let theirs_subscription = cx.subscribe_in(
            &theirs_input,
            window,
            |view: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let _ = view;
                    cx.notify();
                }
            },
        );
        let mut view = Self {
            title,
            path,
            base_input,
            ours_input,
            theirs_input,
            stages: None,
            loading: true,
            error: None,
            notice: None,
            writing: false,
            focus: cx.focus_handle(),
            _subscriptions: [base_subscription, ours_subscription, theirs_subscription],
        };
        view.spawn_load(window, cx);
        view
    }

    fn new_pane(window: &mut Window, cx: &mut Context<'_, InputState>) -> InputState {
        InputState::new(window, cx)
            .code_editor("text")
            .line_number(true)
            .indent_guides(false)
            .soft_wrap(false)
    }

    pub fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }

    /// 三栏恒只读(stage 展示面不存在编辑域)。
    pub fn is_read_only(&self) -> bool {
        true
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.writing {
            return;
        }
        self.loading = true;
        self.error = None;
        self.spawn_load(window, cx);
        cx.notify();
    }

    fn spawn_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let path = self.path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { load_conflict(&path) });
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| view.apply_loaded(loaded, window, cx));
            });
        })
        .detach();
    }

    fn apply_loaded(
        &mut self,
        loaded: Result<ConflictStages, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match loaded {
            Ok(stages) => {
                Self::set_pane(
                    &self.base_input,
                    stages.base.as_deref().unwrap_or_default(),
                    window,
                    cx,
                );
                Self::set_pane(
                    &self.ours_input,
                    stages.ours.as_deref().unwrap_or_default(),
                    window,
                    cx,
                );
                Self::set_pane(
                    &self.theirs_input,
                    stages.theirs.as_deref().unwrap_or_default(),
                    window,
                    cx,
                );
                self.stages = Some(stages);
                self.error = None;
            }
            Err(error) => {
                let language = cx.global::<Settings>().language;
                self.error = Some(language.format(Message::MergeErrorLoad, &[("error", &error)]));
            }
        }
        self.loading = false;
        cx.notify();
    }

    fn set_pane(
        input: &Entity<InputState>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let text = text.to_owned();
        input.update(cx, |input, cx| input.set_value(text, window, cx));
    }

    fn notice_for(&self, block: AdoptBlock, language: UiLanguage) -> String {
        language.text(block_message(block)).to_owned()
    }

    /// 按钮入口:本地 stage 可用性检查后发事件,脏文件门控在工作区。
    fn request_adopt(&mut self, side: MergeSide, cx: &mut Context<'_, Self>) {
        let language = cx.global::<Settings>().language;
        let stage = self
            .stages
            .as_ref()
            .and_then(|stages| stage_at(stages, side.index()))
            .map(String::as_str);
        let truncated = self
            .stages
            .as_ref()
            .is_some_and(|stages| stages.truncated[side.index()]);
        let valid = self
            .stages
            .as_ref()
            .is_some_and(|stages| stages.valid_utf8[side.index()]);
        match adopt_block_reason(stage, truncated, valid, false) {
            Some(block) => {
                self.notice = Some(self.notice_for(block, language));
                cx.notify();
            }
            None => cx.emit(MergeTabEvent::Adopt(side)),
        }
    }

    /// 工作区脏文件门控通过后执行写盘(原子替换,后台执行)。
    pub(crate) fn adopt(&mut self, side: MergeSide, cx: &mut Context<'_, Self>) {
        if self.writing {
            return;
        }
        let language = cx.global::<Settings>().language;
        let stage = self
            .stages
            .as_ref()
            .and_then(|stages| stage_at(stages, side.index()))
            .map(String::as_str);
        let truncated = self
            .stages
            .as_ref()
            .is_some_and(|stages| stages.truncated[side.index()]);
        let valid = self
            .stages
            .as_ref()
            .is_some_and(|stages| stages.valid_utf8[side.index()]);
        let Some(text) = stage else {
            self.notice = Some(self.notice_for(AdoptBlock::MissingStage, language));
            cx.notify();
            return;
        };
        if let Some(block) = adopt_block_reason(Some(text), truncated, valid, false) {
            self.notice = Some(self.notice_for(block, language));
            cx.notify();
            return;
        }
        let bytes = text.as_bytes().to_vec();
        let path = self.path.clone();
        self.writing = true;
        let task = cx
            .background_executor()
            .spawn(async move { crate::atomic_file::write(&path, &bytes) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| view.apply_write(result, cx));
        })
        .detach();
        cx.notify();
    }

    fn apply_write(&mut self, result: io::Result<()>, cx: &mut Context<'_, Self>) {
        self.writing = false;
        let language = cx.global::<Settings>().language;
        match result {
            Ok(()) => {
                self.notice = Some(language.text(Message::MergeWrote).to_owned());
                cx.emit(MergeTabEvent::Written);
            }
            Err(error) => {
                self.notice = Some(
                    language.format(Message::MergeErrorWrite, &[("error", &error.to_string())]),
                );
            }
        }
        cx.notify();
    }

    /// 工作区拒绝写盘(脏草稿/异常)时的视图内提示。
    pub(crate) fn set_notice(&mut self, notice: String, cx: &mut Context<'_, Self>) {
        self.notice = Some(notice);
        cx.notify();
    }

    fn render_pane(
        &self,
        selector: &'static str,
        label: Message,
        index: usize,
        tokens: &EditorThemeTokens,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        // 载入完成前保持输入体(空白),避免把「尚未载入」误画成「stage 缺失」。
        let loaded = self.stages.is_some();
        let stage = self
            .stages
            .as_ref()
            .and_then(|stages| stage_at(stages, index));
        let truncated = self
            .stages
            .as_ref()
            .is_some_and(|stages| stages.truncated[index]);
        let status = if loaded {
            stage_status(stage.map(String::as_str), truncated)
        } else {
            StageStatus::Present
        };
        let label_text = language.text(label);
        let header = match status {
            StageStatus::Missing => language.format(
                Message::MergePaneHeader,
                &[
                    ("label", label_text),
                    ("state", language.text(Message::MergeMissingStage)),
                ],
            ),
            StageStatus::Truncated => language.format(
                Message::MergePaneHeader,
                &[
                    ("label", label_text),
                    ("state", language.text(Message::MergeTruncated)),
                ],
            ),
            StageStatus::Present => label_text.to_owned(),
        };
        let input = match index {
            0 => &self.base_input,
            1 => &self.ours_input,
            _ => &self.theirs_input,
        };
        let muted = crate::gpui_terminal::colors::from_rgb8(tokens.muted);
        let body = if status == StageStatus::Missing {
            div()
                .debug_selector(move || format!("{selector}-missing"))
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(muted)
                .child(language.text(Message::MergeMissingStage).to_owned())
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    Input::new(input)
                        .h_full()
                        .disabled(true)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .rounded(px(0.0))
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(cx.theme().mono_font_size)
                        .line_height(gpui::relative(1.55))
                        .px(px(10.0))
                        .py(px(8.0)),
                )
                .into_any_element()
        };
        div()
            .debug_selector(move || selector.to_owned())
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.0))
                    .flex_shrink_0()
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(header),
            )
            .child(body)
            .into_any_element()
    }
}

impl Render for MergeTabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let tokens = cx.global::<Settings>().editor_theme;
        let border = cx.theme().border;
        let language = cx.global::<Settings>().language;
        let muted = crate::gpui_terminal::colors::from_rgb8(tokens.muted);
        let notice = self
            .error
            .clone()
            .or_else(|| {
                self.loading
                    .then(|| language.text(Message::MergeLoading).to_owned())
            })
            .or_else(|| self.notice.clone());
        let base = self.render_pane("slterm-merge-base", Message::MergeBase, 0, &tokens, cx);
        let ours = self.render_pane("slterm-merge-ours", Message::MergeOurs, 1, &tokens, cx);
        let theirs = self.render_pane("slterm-merge-theirs", Message::MergeTheirs, 2, &tokens, cx);
        div()
            .debug_selector(|| "slterm-merge-tab".to_owned())
            .id("slterm-merge-tab")
            .key_context("FileEditor")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(crate::gpui_terminal::colors::from_rgb8(tokens.background))
            .child(
                div()
                    .h(px(30.0))
                    .flex_shrink_0()
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.0))
                            .text_color(crate::gpui_terminal::colors::from_rgb8(tokens.foreground))
                            .child(self.title.clone()),
                    )
                    .when_some(notice, |bar, notice| {
                        bar.child(
                            div()
                                .debug_selector(|| "merge-notice".to_owned())
                                .flex_shrink_0()
                                .max_w(px(360.0))
                                .truncate()
                                .text_xs()
                                .text_color(muted)
                                .child(notice),
                        )
                    })
                    .child(
                        Button::new("merge-adopt-ours")
                            .debug_selector(|| "merge-adopt-ours".to_owned())
                            .xsmall()
                            .label(language.text(Message::MergeAdoptOurs))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_adopt(MergeSide::Ours, cx);
                            })),
                    )
                    .child(
                        Button::new("merge-adopt-theirs")
                            .debug_selector(|| "merge-adopt-theirs".to_owned())
                            .xsmall()
                            .label(language.text(Message::MergeAdoptTheirs))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_adopt(MergeSide::Theirs, cx);
                            })),
                    )
                    .child(
                        Button::new("merge-open-editor")
                            .debug_selector(|| "merge-open-editor".to_owned())
                            .xsmall()
                            .label(language.text(Message::MergeOpenInEditor))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(MergeTabEvent::OpenInEditor);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .child(base)
                    .child(div().w(px(1.0)).h_full().flex_shrink_0().bg(border))
                    .child(ours)
                    .child(div().w(px(1.0)).h_full().flex_shrink_0().bg(border))
                    .child(theirs),
            )
    }
}

/// 后台载入:仓库根/相对路径/三 stage 读取一并离线。
fn load_conflict(path: &Path) -> Result<ConflictStages, String> {
    let root = git::repository_root(path).map_err(|error| error.to_string())?;
    let relative = git::relative_path(&root, path).map_err(|error| error.to_string())?;
    git::conflict_stages(&root, &relative).map_err(|error| error.to_string())
}

impl MergeTabView {
    /// 测试专用:三栏文本(未载入时全 None)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn texts_for_test(&self) -> (Option<String>, Option<String>, Option<String>) {
        match &self.stages {
            Some(stages) => (
                stages.base.clone(),
                stages.ours.clone(),
                stages.theirs.clone(),
            ),
            None => (None, None, None),
        }
    }

    /// 测试专用:当前提示文案。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn notice_for_test(&self) -> Option<String> {
        self.notice.clone()
    }
}

#[cfg(test)]
mod merge_tab_tests {
    use super::*;

    /// 缺失 stage 走独立占位;存在且截断单独标记;正常 stage 原样展示。
    #[test]
    fn stage_status_separates_missing_truncated_and_present() {
        assert_eq!(stage_status(None, false), StageStatus::Missing);
        assert_eq!(stage_status(None, true), StageStatus::Missing);
        assert_eq!(stage_status(Some("text"), true), StageStatus::Truncated);
        assert_eq!(stage_status(Some("text"), false), StageStatus::Present);
    }

    /// 采用决策:草稿保护优先于可用 stage;缺失/截断/非 UTF-8 各自拒绝。
    #[test]
    fn adopt_decision_rejects_dirty_drafts_and_unusable_stages() {
        assert_eq!(
            adopt_block_reason(Some("ours\n"), false, true, true),
            Some(AdoptBlock::DirtyDraft),
            "同路径脏草稿必须拒绝写盘"
        );
        assert_eq!(
            adopt_block_reason(None, false, true, false),
            Some(AdoptBlock::MissingStage)
        );
        assert_eq!(
            adopt_block_reason(Some("ours\n"), true, true, false),
            Some(AdoptBlock::TruncatedStage),
            "截断 stage 写盘会损坏文件"
        );
        assert_eq!(
            adopt_block_reason(Some("ours\n"), false, false, false),
            Some(AdoptBlock::InvalidEncoding)
        );
        assert_eq!(adopt_block_reason(Some("ours\n"), false, true, false), None);
    }

    /// 拒绝原因映射到齐全的中英文案键(键集一致由 i18n 生成器强制)。
    #[test]
    fn every_adopt_block_maps_to_nonempty_bilingual_messages() {
        for block in [
            AdoptBlock::MissingStage,
            AdoptBlock::TruncatedStage,
            AdoptBlock::InvalidEncoding,
            AdoptBlock::DirtyDraft,
        ] {
            let message = block_message(block);
            assert!(!UiLanguage::EnUs.text(message).is_empty(), "{block:?}");
            assert!(!UiLanguage::ZhCn.text(message).is_empty(), "{block:?}");
        }
    }
}
