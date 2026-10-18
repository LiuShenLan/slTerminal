//! 通用 diff 对照面(双栏只读对照)。
//!
//! 左 HEAD / 右工作区,两栏都是**只读** `InputState`;数据经
//! `TextSnapshot::decode_prefix(.., read_only=true, ..)` 有界载入,
//! 只读 contract 在快照类型层(encode 入口拒绝),UI 的 `disabled` 只是
//! 辅助。hunk 导航 = 上一/下一处更改,双栏同步滚动到对应行。
//!
//! 落位:`WorkspaceTab::Code` 的只读臂(`CodeTabView` 的 Diff 模式),
//! 不新增 tab 变体。

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window, div, point, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{IconName, Sizable as _, button::Button, button::ButtonVariants as _};

use crate::app_shell::config::Settings;
use crate::app_shell::editor_theme::EditorThemeTokens;
use crate::git;
use crate::i18n::Message;
use crate::text_document::{MAX_BYTES, TextSnapshot};

/// diff 数据源(HEAD ↔ 工作区 / 任意文本对)。
#[derive(Clone, Debug)]
#[allow(dead_code)] // 消费点 = BlobPair 预留臂(纯内存对照,当前无生产入口)。
pub enum DiffSource {
    /// 文件 HEAD ↔ 工作区(gutter 点击 / gitshow / diff 面板三入口)。
    HeadToWorktree { path: PathBuf },
    /// 两任意文本源对照(纯内存,不走 git;预留臂)。
    BlobPair { left: DiffBlob, right: DiffBlob },
}

/// 纯内存文本源(经 `decode` 有界载入)。
#[derive(Clone, Debug)]
pub struct DiffBlob {
    pub label: String,
    pub bytes: Vec<u8>,
}

/// 一条统一 diff hunk。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

/// 载入后的双栏快照与 hunk 集(后台线程产物)。
struct LoadedDiff {
    left: TextSnapshot,
    right: TextSnapshot,
    left_label: Option<String>,
    right_label: Option<String>,
    hunks: Vec<DiffHunk>,
}

/// 双栏只读对照 tab。
pub struct DiffTabView {
    pub title: String,
    source: DiffSource,
    left_input: Entity<InputState>,
    right_input: Entity<InputState>,
    left_snapshot: Option<TextSnapshot>,
    right_snapshot: Option<TextSnapshot>,
    left_label: Option<String>,
    right_label: Option<String>,
    hunks: Vec<DiffHunk>,
    active_hunk: Option<usize>,
    loading: bool,
    error: Option<String>,
    focus: FocusHandle,
    _left_subscription: gpui::Subscription,
    _right_subscription: gpui::Subscription,
}

impl Focusable for DiffTabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DiffTabView {
    /// 打开 diff 源:hunk 计算与两侧字节读取都在后台任务。
    pub fn open(source: DiffSource, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let title = match &source {
            DiffSource::HeadToWorktree { path } => format!(
                "{} ↔ HEAD",
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            ),
            DiffSource::BlobPair { left, right } => {
                format!("{} ↔ {}", left.label, right.label)
            }
        };
        let left_input = cx.new(|cx| Self::new_pane("text", window, cx));
        let right_input = cx.new(|cx| Self::new_pane("text", window, cx));
        let left_subscription = cx.subscribe_in(
            &left_input,
            window,
            |view: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    // 只读臂:程序化 set_value 不触发 Change;键入路径不应存在。
                    let _ = view;
                    cx.notify();
                }
            },
        );
        let right_subscription = cx.subscribe_in(
            &right_input,
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
            source,
            left_input,
            right_input,
            left_snapshot: None,
            right_snapshot: None,
            left_label: None,
            right_label: None,
            hunks: Vec::new(),
            active_hunk: None,
            loading: true,
            error: None,
            focus: cx.focus_handle(),
            _left_subscription: left_subscription,
            _right_subscription: right_subscription,
        };
        view.spawn_load(window, cx);
        view
    }

    fn new_pane(
        language: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, InputState>,
    ) -> InputState {
        InputState::new(window, cx)
            .code_editor(language)
            .line_number(true)
            .indent_guides(false)
            .soft_wrap(false)
    }

    pub fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }

    /// 只读 contract:两栏快照均为 `read_only=true`。
    pub fn is_read_only(&self) -> bool {
        self.left_snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.read_only)
            && self
                .right_snapshot
                .as_ref()
                .is_none_or(|snapshot| snapshot.read_only)
    }

    #[allow(dead_code)] // 消费点 = diff 导航断言端(CodeTabView 测试读端)。
    pub fn hunks(&self) -> &[DiffHunk] {
        &self.hunks
    }

    #[allow(dead_code)] // 消费点 = diff 导航断言端(CodeTabView 测试读端)。
    pub fn active_hunk(&self) -> Option<usize> {
        self.active_hunk
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.loading = true;
        self.error = None;
        self.spawn_load(window, cx);
        cx.notify();
    }

    fn spawn_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let source = self.source.clone();
        let task = cx
            .background_executor()
            .spawn(async move { load_diff(&source) });
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
        loaded: Result<LoadedDiff, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match loaded {
            Ok(loaded) => {
                let left_text = loaded.left.text.clone();
                let right_text = loaded.right.text.clone();
                self.left_input.update(cx, |input, cx| {
                    input.set_value(left_text, window, cx);
                });
                self.right_input.update(cx, |input, cx| {
                    input.set_value(right_text, window, cx);
                });
                self.left_snapshot = Some(loaded.left);
                self.right_snapshot = Some(loaded.right);
                self.left_label = loaded.left_label;
                self.right_label = loaded.right_label;
                self.hunks = loaded.hunks;
                self.active_hunk = None;
            }
            Err(error) => self.error = Some(error),
        }
        self.loading = false;
        cx.notify();
    }

    /// 下一处更改(到尾部保持末项)。
    pub fn next_hunk(&mut self, cx: &mut Context<'_, Self>) {
        let Some(index) = self.advance_hunk(true) else {
            return;
        };
        self.jump_to_hunk(index, cx);
    }

    /// 上一处更改(到头部保持首项)。
    pub fn prev_hunk(&mut self, cx: &mut Context<'_, Self>) {
        let Some(index) = self.advance_hunk(false) else {
            return;
        };
        self.jump_to_hunk(index, cx);
    }

    fn advance_hunk(&self, forward: bool) -> Option<usize> {
        if self.hunks.is_empty() {
            return None;
        }
        let last = self.hunks.len() - 1;
        Some(match (self.active_hunk, forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(index), true) => (index + 1).min(last),
            (Some(index), false) => index.saturating_sub(1),
        })
    }

    /// 双栏同步滚动到 hunk 首行(1 基 → 0 基)。
    fn jump_to_hunk(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        let Some(hunk) = self.hunks.get(index).copied() else {
            return;
        };
        self.active_hunk = Some(index);
        let old_line = hunk.old_start.saturating_sub(1);
        let new_line = hunk.new_start.saturating_sub(1);
        Self::scroll_pane(&self.left_input, old_line, cx);
        Self::scroll_pane(&self.right_input, new_line, cx);
        cx.notify();
    }

    fn scroll_pane(input: &Entity<InputState>, line: u32, cx: &mut Context<'_, Self>) {
        input.update(cx, |input, cx| {
            let line_height = input.line_height().map(f32::from).unwrap_or(16.0);
            input.set_scroll_offset(point(px(0.0), px(-(line as f32 * line_height))), cx);
        });
    }

    fn pane_label(label: &Option<String>, fallback: Message, cx: &App) -> String {
        label
            .clone()
            .unwrap_or_else(|| cx.global::<Settings>().language.text(fallback).to_owned())
    }

    fn render_pane(
        &self,
        pane_selector: &'static str,
        label: String,
        input: &Entity<InputState>,
        tokens: &EditorThemeTokens,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        div()
            .debug_selector(move || pane_selector.to_owned())
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
                    .text_color(crate::gpui_terminal::colors::from_rgb8(tokens.muted))
                    .child(label),
            )
            .child(
                div().flex_1().min_h_0().w_full().child(
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
                ),
            )
            .into_any_element()
    }
}

impl Render for DiffTabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let tokens = cx.global::<Settings>().editor_theme;
        let border = cx.theme().border;
        let language = cx.global::<Settings>().language;
        let left_label = Self::pane_label(&self.left_label, Message::EditorDiffHead, cx);
        let right_label = Self::pane_label(&self.right_label, Message::EditorDiffWorktree, cx);
        let counter = if self.hunks.is_empty() {
            language.text(Message::EditorDiffEmpty).to_owned()
        } else {
            format!(
                "{}/{}",
                self.active_hunk.map_or(1, |index| index + 1),
                self.hunks.len()
            )
        };
        let left = self.render_pane(
            "slterm-diff-left",
            left_label,
            &self.left_input,
            &tokens,
            cx,
        );
        let right = self.render_pane(
            "slterm-diff-right",
            right_label,
            &self.right_input,
            &tokens,
            cx,
        );
        let notice = self.error.clone().or_else(|| {
            self.loading
                .then(|| language.text(Message::EditorDiffLoading).to_owned())
        });
        div()
            .debug_selector(|| "slterm-diff-tab".to_owned())
            .id("slterm-diff-tab")
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
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(crate::gpui_terminal::colors::from_rgb8(tokens.muted))
                                .child(notice),
                        )
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(crate::gpui_terminal::colors::from_rgb8(tokens.muted))
                            .child(counter),
                    )
                    .child(
                        Button::new("diff-prev-hunk")
                            .debug_selector(|| "diff-prev-hunk".to_owned())
                            .icon(IconName::ChevronUp)
                            .ghost()
                            .xsmall()
                            .tooltip(language.text(Message::EditorDiffPrevious))
                            .on_click(cx.listener(|this, _, _, cx| this.prev_hunk(cx))),
                    )
                    .child(
                        Button::new("diff-next-hunk")
                            .debug_selector(|| "diff-next-hunk".to_owned())
                            .icon(IconName::ChevronDown)
                            .ghost()
                            .xsmall()
                            .tooltip(language.text(Message::EditorDiffNext))
                            .on_click(cx.listener(|this, _, _, cx| this.next_hunk(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .child(left)
                    .child(div().w(px(1.0)).h_full().flex_shrink_0().bg(border))
                    .child(right),
            )
    }
}

/// 后台载入两侧快照(只读合同)并计算 hunk。
fn load_diff(source: &DiffSource) -> Result<LoadedDiff, String> {
    match source {
        DiffSource::HeadToWorktree { path } => {
            let right = read_snapshot(path)?;
            let left = match git::head_file(path, MAX_BYTES) {
                Ok((bytes, truncated)) => TextSnapshot::decode_prefix(bytes, true, truncated),
                // HEAD 不存在(尚无提交/新文件):左栏空,差异即全文新增。
                Err(git::GitError::NotInHead(_)) => {
                    TextSnapshot::decode_prefix(Vec::new(), true, false)
                }
                Err(error) => return Err(error.to_string()),
            };
            let hunks = compute_diff_hunks(&left.text, &right.text);
            Ok(LoadedDiff {
                left,
                right,
                left_label: None,
                right_label: None,
                hunks,
            })
        }
        DiffSource::BlobPair { left, right } => {
            let left_snapshot = TextSnapshot::decode_prefix(left.bytes.clone(), true, false);
            let right_snapshot = TextSnapshot::decode_prefix(right.bytes.clone(), true, false);
            let hunks = compute_diff_hunks(&left_snapshot.text, &right_snapshot.text);
            Ok(LoadedDiff {
                left: left_snapshot,
                right: right_snapshot,
                left_label: Some(left.label.clone()),
                right_label: Some(right.label.clone()),
                hunks,
            })
        }
    }
}

/// 工作区侧有界只读快照(内存上限先于解码)。
fn read_snapshot(path: &Path) -> Result<TextSnapshot, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("无法读取 {} 元数据: {error}", path.display()))?;
    let (bytes, truncated) = crate::document_io::read_prefix(&mut file, metadata.len(), MAX_BYTES)
        .map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    Ok(TextSnapshot::decode_prefix(bytes, true, truncated))
}

/// 行级 LCS 差分(纯函数;超大输入退化为整文件单 hunk)。
pub(crate) fn compute_diff_hunks(left: &str, right: &str) -> Vec<DiffHunk> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    if left_lines == right_lines {
        return Vec::new();
    }
    if left_lines.is_empty() && right_lines.is_empty() {
        return Vec::new();
    }
    // 4M 单元格上限:病理性大文件不做二次方差分,退化为单个全量 hunk。
    if left_lines.len().saturating_mul(right_lines.len()) > 4_000_000 {
        return vec![DiffHunk {
            old_start: 1,
            old_lines: left_lines.len() as u32,
            new_start: 1,
            new_lines: right_lines.len() as u32,
        }];
    }
    let old_len = left_lines.len();
    let new_len = right_lines.len();
    let width = new_len + 1;
    let mut lcs = vec![0u32; (old_len + 1) * width];
    for old in (0..old_len).rev() {
        for new in (0..new_len).rev() {
            lcs[old * width + new] = if left_lines[old] == right_lines[new] {
                lcs[(old + 1) * width + new + 1] + 1
            } else {
                lcs[(old + 1) * width + new].max(lcs[old * width + new + 1])
            };
        }
    }
    let mut hunks = Vec::new();
    let (mut old, mut new) = (0usize, 0usize);
    while old < old_len || new < new_len {
        if old < old_len && new < new_len && left_lines[old] == right_lines[new] {
            old += 1;
            new += 1;
            continue;
        }
        let start_old = old;
        let start_new = new;
        // 连续不等块:严格更优才走插入,否则走删除(相同分值时删除优先
        // → 单行替换产出一条 `-1/+1` 而不是两条)。
        while old < old_len || new < new_len {
            if old < old_len && new < new_len && left_lines[old] == right_lines[new] {
                break;
            }
            let delete_old = old < old_len
                && (new == new_len || lcs[(old + 1) * width + new] >= lcs[old * width + new + 1]);
            if delete_old {
                old += 1;
            } else {
                new += 1;
            }
        }
        hunks.push(DiffHunk {
            old_start: (start_old + 1) as u32,
            old_lines: (old - start_old) as u32,
            new_start: (start_new + 1) as u32,
            new_lines: (new - start_new) as u32,
        });
    }
    hunks
}

#[cfg(test)]
mod diff_tab_tests {
    use super::*;
    use crate::text_document::SaveError;

    /// 单行替换 → 一条修改 hunk;纯插入/纯删除的起始行正确。
    #[test]
    fn hunks_locate_replacements_insertions_and_deletions() {
        let hunks = compute_diff_hunks("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(
            hunks,
            [DiffHunk {
                old_start: 2,
                old_lines: 1,
                new_start: 2,
                new_lines: 1
            }]
        );
        let inserted = compute_diff_hunks("a\nc\n", "a\nb\nc\n");
        assert_eq!(inserted.len(), 1);
        assert_eq!((inserted[0].old_start, inserted[0].old_lines), (2, 0));
        assert_eq!((inserted[0].new_start, inserted[0].new_lines), (2, 1));
        let deleted = compute_diff_hunks("a\nb\nc\n", "a\nc\n");
        assert_eq!(deleted.len(), 1);
        assert_eq!((deleted[0].old_start, deleted[0].old_lines), (2, 1));
        assert_eq!((deleted[0].new_start, deleted[0].new_lines), (2, 0));
        assert!(compute_diff_hunks("same\n", "same\n").is_empty());
    }

    /// 只读合同在快照类型层:两栏 `encode` 均拒绝。
    #[test]
    fn both_sides_reject_encode() {
        let snapshot = TextSnapshot::decode_prefix(b"text\n".to_vec(), true, false);
        assert!(snapshot.read_only);
        assert!(matches!(
            snapshot.encode("changed"),
            Err(SaveError::ReadOnly)
        ));
    }

    /// 超大数据退化为单个全量 hunk(不做二次方差分)。
    #[test]
    fn oversized_inputs_fall_back_to_one_hunk() {
        let left = "a\n".repeat(2100);
        let right = "b\n".repeat(2100);
        let hunks = compute_diff_hunks(&left, &right);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_lines, 2100);
        assert_eq!(hunks[0].new_lines, 2100);
    }
}
