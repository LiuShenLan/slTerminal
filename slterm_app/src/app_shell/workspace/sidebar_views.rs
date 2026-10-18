//! 侧栏三视图渲染(B73):导航树 / Commit / Agent 全局文件。
//!
//! 数据源与持久化边界:
//! - Nav 每帧从 `SltermWorkspace::tabs` 派生,不新增持久化面;
//! - Commit 只读 `SidePanel::git_snapshot()`(后台快照,Arc 共享),渲染前
//!   经 `sync_file_tree` 让 Commit 槽也参与快照刷新;
//! - AgentFiles 用进程内缓存(`agent_dirs_cache`),首次渲染加载,禁止每帧
//!   27 次 `is_dir`。
//!
//! 槽外层容器与 `sidebar-slot-*` selector 由 `sidebar::render_view_slot`
//! 提供;本模块只产出槽内容。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use gpui_component::{ActiveTheme as _, v_flex};
use slterm_split::PaneId;

use super::{SltermWorkspace, WorkspaceTab};
use crate::agent_dirs::{self, AgentGlobalDir};
use crate::ai_agents::AgentKind;
use crate::app_shell::config::Settings;
use crate::i18n::{Message, UiLanguage};
use crate::side_panel::{GitCommit, GitInfo};

// ── 纯投影:渲染与测试共用同一份数据整形 ──

/// 一条 Commit 变更行投影(路径借用 git 快照,渲染期零拷贝)。
pub(super) struct CommitChange<'a> {
    pub status: char,
    pub path: &'a str,
}

/// Commit 视图投影:冲突单列,staged/unstaged 中同路径条目剔除。
pub(super) struct CommitProjection<'a> {
    pub branch: &'a str,
    pub ahead: u32,
    pub plus: u64,
    pub minus: u64,
    pub conflicts: Vec<CommitChange<'a>>,
    pub staged: Vec<CommitChange<'a>>,
    pub unstaged: Vec<CommitChange<'a>>,
    pub history: &'a [GitCommit],
}

impl<'a> CommitProjection<'a> {
    /// 从 git 快照整形:冲突路径从 staged/unstaged 两组过滤,避免重复显示。
    pub(super) fn from_git(git: &'a GitInfo) -> Self {
        let conflict_paths: std::collections::HashSet<&str> = git
            .conflicts
            .iter()
            .map(|(_, path)| path.as_str())
            .collect();
        let without_conflicts = |entries: &'a [(char, String)]| -> Vec<CommitChange<'a>> {
            entries
                .iter()
                .filter(|(_, path)| !conflict_paths.contains(path.as_str()))
                .map(|(status, path)| CommitChange {
                    status: *status,
                    path: path.as_str(),
                })
                .collect()
        };
        Self {
            branch: git.branch.as_str(),
            ahead: git.ahead,
            plus: git.plus,
            minus: git.minus,
            conflicts: git
                .conflicts
                .iter()
                .map(|(status, path)| CommitChange {
                    status: *status,
                    path: path.as_str(),
                })
                .collect(),
            staged: without_conflicts(&git.staged),
            unstaged: without_conflicts(&git.unstaged),
            history: git.history.as_slice(),
        }
    }

    /// 去重后的变更行总数(冲突 + 已暂存 + 未暂存)。
    pub(super) fn change_count(&self) -> usize {
        self.conflicts.len() + self.staged.len() + self.unstaged.len()
    }
}

/// AgentFiles 一行投影;缺失目录保留行位并标记 `exists=false`(降色占位)。
pub(super) struct AgentDirRow {
    pub cli_id: String,
    pub display_name: String,
    pub path: String,
    pub exists: bool,
}

/// 目录条目 → 展示行:显示名按 slug 反查 [`AgentKind`],未知 slug 回落原值。
pub(super) fn agent_dir_rows(dirs: &[AgentGlobalDir]) -> Vec<AgentDirRow> {
    dirs.iter()
        .map(|directory| AgentDirRow {
            cli_id: directory.cli_id.clone(),
            display_name: AgentKind::parse(&directory.cli_id)
                .map(|kind| kind.display_name().to_owned())
                .unwrap_or_else(|| directory.cli_id.clone()),
            path: directory.path.display().to_string(),
            exists: directory.exists,
        })
        .collect()
}

// ── 渲染辅助 ──

/// 槽标题行(30px,与旧占位同高;语言解析在调用点完成)。
fn view_header(title: Message, language: UiLanguage) -> Div {
    div()
        .h(px(30.0))
        .px(px(10.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .text_size(px(12.0))
        .child(language.text(title))
}

/// 分组标题行(冲突/已暂存/未暂存/最近提交)。
fn group_header(text: &'static str, color: Hsla) -> AnyElement {
    div()
        .w_full()
        .pt(px(6.0))
        .pb(px(2.0))
        .px(px(4.0))
        .text_size(px(10.0))
        .text_color(color)
        .child(text)
        .into_any_element()
}

/// 分支行右侧统计:`↑领先 +增 −删`(零值省略)。
fn commit_stats_text(projection: &CommitProjection<'_>) -> String {
    let mut parts = Vec::new();
    if projection.ahead > 0 {
        parts.push(format!("↑{}", projection.ahead));
    }
    if projection.plus > 0 || projection.minus > 0 {
        parts.push(format!("+{} −{}", projection.plus, projection.minus));
    }
    parts.join("  ")
}

/// 一条变更行:状态字符 + 路径。
fn commit_change_row(
    index: usize,
    change: &CommitChange<'_>,
    status_color: Hsla,
    foreground: Hsla,
    hover: Hsla,
) -> AnyElement {
    div()
        .id(SharedString::from(format!("sidebar-commit-change-{index}")))
        .debug_selector(move || format!("sidebar-commit-change-{index}"))
        .w_full()
        .h(px(22.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .text_size(px(11.0))
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .w(px(12.0))
                .flex_shrink_0()
                .text_size(px(10.0))
                .text_color(status_color)
                .child(change.status.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(foreground)
                .child(change.path.to_owned()),
        )
        .into_any_element()
}

/// 一条历史提交行:短哈希 + 主题。
fn commit_history_row(
    index: usize,
    commit: &GitCommit,
    muted: Hsla,
    foreground: Hsla,
    hover: Hsla,
) -> AnyElement {
    div()
        .id(SharedString::from(format!(
            "sidebar-commit-history-{index}"
        )))
        .debug_selector(move || format!("sidebar-commit-history-{index}"))
        .w_full()
        .h(px(22.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .text_size(px(11.0))
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(10.0))
                .text_color(muted)
                .child(commit.short_hash.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(foreground)
                .child(commit.subject.clone()),
        )
        .into_any_element()
}

/// Nav 投影的一行 pane(终端 tab 的子树)。
struct NavPaneSnapshot {
    pane_id: PaneId,
    title: String,
    cwd: String,
    focused: bool,
}

/// Nav 投影的一个 tab。
struct NavTabSnapshot {
    title: String,
    active: bool,
    panes: Vec<NavPaneSnapshot>,
}

impl SltermWorkspace {
    /// Nav 视图:全部 tab(标题 = 现有 tab 标题逻辑/自定义名)→ 终端 tab
    /// 下再列 pane(标题 + cwd)。点击 tab 行激活,点击 pane 行聚焦。
    pub(super) fn render_nav_view(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let language = cx.global::<Settings>().language;
        let foreground = cx.theme().sidebar_foreground;
        let muted = cx.theme().muted_foreground;
        let active_bg = cx.theme().sidebar_accent;
        let active_fg = cx.theme().sidebar_accent_foreground;
        let hover_bg = cx.theme().list_hover;
        let accent = cx.theme().primary;
        let active = self.active;
        // 每帧从 tabs 派生;读终端视图的标题/cwd 后立刻转成自持快照,
        // 后续链式闭包不再借 self。
        let tabs: Vec<NavTabSnapshot> = (0..self.tabs.len())
            .map(|ix| {
                let title = self.tab_title(ix, cx);
                let panes = match &self.tabs[ix].tab {
                    WorkspaceTab::Terminal { panes, focused, .. } => panes
                        .iter()
                        .map(|pane| {
                            let view = pane.view.read(cx);
                            NavPaneSnapshot {
                                pane_id: pane.id,
                                title: if view.title.is_empty() {
                                    "shell".to_owned()
                                } else {
                                    view.title.clone()
                                },
                                cwd: view.cwd.clone(),
                                focused: pane.id == *focused,
                            }
                        })
                        .collect(),
                    WorkspaceTab::Settings { .. }
                    | WorkspaceTab::Document { .. }
                    | WorkspaceTab::Code { .. }
                    | WorkspaceTab::Image { .. } => Vec::new(),
                };
                NavTabSnapshot {
                    title,
                    active: ix == active,
                    panes,
                }
            })
            .collect();
        let rows = tabs.into_iter().enumerate().map(|(ix, tab)| {
            let is_active = tab.active;
            let tab_row = div()
                .id(SharedString::from(format!("sidebar-nav-tab-{ix}")))
                .debug_selector(move || format!("sidebar-nav-tab-{ix}"))
                .w_full()
                .h(px(24.0))
                .flex()
                .items_center()
                .px(px(6.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .when(is_active, |row| row.bg(active_bg).text_color(active_fg))
                .when(!is_active, |row| {
                    row.text_color(foreground)
                        .hover(move |style| style.bg(hover_bg))
                })
                .child(div().min_w_0().truncate().child(tab.title))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_tab(ix, window, cx);
                }));
            let pane_rows = tab.panes.into_iter().map(|pane| {
                let pane_id = pane.pane_id;
                let highlighted = pane.focused && is_active;
                div()
                    .id(SharedString::from(format!(
                        "sidebar-nav-pane-{}",
                        pane_id.0
                    )))
                    .debug_selector(move || format!("sidebar-nav-pane-{}", pane_id.0))
                    .w_full()
                    .min_h(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .pl(px(16.0))
                    .pr(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .when(highlighted, |row| {
                        row.bg(active_bg)
                            .text_color(active_fg)
                            .border_l_2()
                            .border_color(accent)
                    })
                    .when(!highlighted, |row| {
                        row.text_color(foreground)
                            .hover(move |style| style.bg(hover_bg))
                    })
                    .child(div().min_w_0().truncate().child(pane.title))
                    .when(!pane.cwd.is_empty(), |row| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(10.0))
                                .text_color(muted)
                                .child(pane.cwd),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.active != ix {
                            this.activate_tab(ix, window, cx);
                        }
                        this.focus_pane(ix, pane_id, window, cx);
                    }))
            });
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(tab_row)
                .children(pane_rows)
        });
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(view_header(Message::SidebarViewNav, language).text_color(foreground))
            .child(
                v_flex()
                    .id("sidebar-nav-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .px(px(6.0))
                    .py(px(4.0))
                    .children(rows),
            )
            .into_any_element()
    }

    /// Commit 视图:分支/领先落后/增删行 + 冲突/已暂存/未暂存分组 +
    /// 最近提交。无 git 快照时显示空态。
    pub(super) fn render_commit_view(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        // Commit 槽同样依赖文件树后台快照;同步在槽渲染处触发(gate 由
        // 文件树域维护:explorer 或 commit 可见)。
        self.sync_file_tree(cx);
        let language = cx.global::<Settings>().language;
        let foreground = cx.theme().sidebar_foreground;
        let muted = cx.theme().muted_foreground;
        let success = cx.theme().success;
        let warning = cx.theme().warning;
        let danger = cx.theme().danger;
        let hover = cx.theme().list_hover;
        #[cfg(all(test, feature = "gpui-test-support"))]
        let snapshot = self
            .commit_snapshot_for_test
            .clone()
            .or_else(|| self.file_tree.panel.git_snapshot());
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let snapshot = self.file_tree.panel.git_snapshot();
        let body = match snapshot {
            Some(git) => {
                let projection = CommitProjection::from_git(&git);
                let mut rows: Vec<AnyElement> = Vec::new();
                rows.push(
                    div()
                        .id("sidebar-commit-branch")
                        .debug_selector(|| "sidebar-commit-branch".to_owned())
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .px(px(4.0))
                        .py(px(2.0))
                        .text_size(px(12.0))
                        .text_color(foreground)
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .child(projection.branch.to_owned()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(10.0))
                                .text_color(muted)
                                .child(commit_stats_text(&projection)),
                        )
                        .into_any_element(),
                );
                let mut change_ix = 0usize;
                for (label, entries, color) in [
                    (
                        Message::SidebarCommitConflicts,
                        &projection.conflicts,
                        danger,
                    ),
                    (Message::SidebarCommitStaged, &projection.staged, success),
                    (
                        Message::SidebarCommitUnstaged,
                        &projection.unstaged,
                        warning,
                    ),
                ] {
                    if entries.is_empty() {
                        continue;
                    }
                    rows.push(group_header(language.text(label), muted));
                    for change in entries {
                        rows.push(commit_change_row(
                            change_ix, change, color, foreground, hover,
                        ));
                        change_ix += 1;
                    }
                }
                debug_assert_eq!(
                    change_ix,
                    projection.change_count(),
                    "变更行计数必须与投影一致"
                );
                if !projection.history.is_empty() {
                    rows.push(group_header(
                        language.text(Message::SidebarCommitHistory),
                        muted,
                    ));
                    for (ix, commit) in projection.history.iter().enumerate() {
                        rows.push(commit_history_row(ix, commit, muted, foreground, hover));
                    }
                }
                v_flex()
                    .id("sidebar-commit-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .px(px(8.0))
                    .py(px(4.0))
                    .children(rows)
                    .into_any_element()
            }
            None => div()
                .id("sidebar-commit-empty")
                .debug_selector(|| "sidebar-commit-empty".to_owned())
                .flex_1()
                .min_h_0()
                .w_full()
                .px(px(10.0))
                .py(px(8.0))
                .text_size(px(12.0))
                .text_color(muted)
                .child(language.text(Message::SidebarCommitEmpty))
                .into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(view_header(Message::SidebarViewCommit, language).text_color(foreground))
            .child(body)
            .into_any_element()
    }

    /// AgentFiles 视图:列出全部 Agent 全局目录;首次渲染加载进程内缓存,
    /// 缺失目录保留行位并降色。
    pub(super) fn render_agent_files_view(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        if self.agent_dirs_cache.is_none() {
            self.agent_dirs_cache = Some(agent_dirs::agent_global_dirs());
        }
        let rows = agent_dir_rows(self.agent_dirs_cache.as_deref().unwrap_or(&[]));
        let language = cx.global::<Settings>().language;
        let foreground = cx.theme().sidebar_foreground;
        let muted = cx.theme().muted_foreground;
        let hover = cx.theme().list_hover;
        let body = if rows.is_empty() {
            div()
                .id("sidebar-agent-empty")
                .debug_selector(|| "sidebar-agent-empty".to_owned())
                .flex_1()
                .min_h_0()
                .w_full()
                .px(px(10.0))
                .py(px(8.0))
                .text_size(px(12.0))
                .text_color(muted)
                .child(language.text(Message::SidebarAgentEmpty))
                .into_any_element()
        } else {
            let missing = language.text(Message::SidebarAgentMissing);
            let rows = rows.into_iter().map(|row| {
                let selector = format!("sidebar-agent-dir-{}", row.cli_id);
                let exists = row.exists;
                let detail = if exists {
                    row.path
                } else {
                    format!("{} · {missing}", row.path)
                };
                div()
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector)
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .text_color(if exists { foreground } else { muted })
                    .hover(move |style| style.bg(hover))
                    .child(div().text_size(px(12.0)).child(row.display_name))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(muted)
                            .truncate()
                            .child(detail),
                    )
            });
            v_flex()
                .id("sidebar-agent-files-scroll")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .px(px(6.0))
                .py(px(4.0))
                .children(rows)
                .into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(view_header(Message::SidebarViewAgentFiles, language).text_color(foreground))
            .child(body)
            .into_any_element()
    }

    /// 显式刷新 Agent 目录缓存(后续 AgentFiles 刷新按钮的接线点)。
    #[allow(dead_code)] // 消费点 = 后续刷新按钮;当前只保留显式入口。
    pub(crate) fn refresh_agent_dirs(&mut self) {
        self.agent_dirs_cache = None;
    }

    /// 测试专用:注入 Agent 目录替身,避免 UI 用例读真实 USERPROFILE。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_agent_dirs_for_test(&mut self, dirs: Vec<AgentGlobalDir>) {
        self.agent_dirs_cache = Some(dirs);
    }

    /// 测试专用:注入 Commit 快照(渲染 selector 的确定性数据源)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_commit_snapshot_for_test(
        &mut self,
        snapshot: Option<std::sync::Arc<GitInfo>>,
    ) {
        self.commit_snapshot_for_test = snapshot;
    }
}

/// 纯投影单测:渲染前整形与真实渲染共用同一份逻辑。
#[cfg(test)]
mod sidebar_view_tests {
    use super::*;
    use std::path::PathBuf;

    fn git_fixture() -> GitInfo {
        GitInfo {
            branch: "main".to_owned(),
            plus: 3,
            minus: 1,
            ahead: 2,
            conflicts: vec![
                ('U', "shared.txt".to_owned()),
                ('U', "conflict.txt".to_owned()),
            ],
            staged: vec![('M', "shared.txt".to_owned()), ('A', "new.txt".to_owned())],
            unstaged: vec![
                ('M', "shared.txt".to_owned()),
                ('?', "draft.txt".to_owned()),
            ],
            history: Vec::new(),
        }
    }

    /// 冲突路径从 staged/unstaged 过滤;计数 = 去重后三组之和。
    #[test]
    fn commit_projection_dedups_conflict_paths_and_counts_rows() {
        let git = git_fixture();
        let projection = CommitProjection::from_git(&git);
        assert_eq!(projection.conflicts.len(), 2);
        assert_eq!(
            projection
                .staged
                .iter()
                .map(|change| change.path)
                .collect::<Vec<_>>(),
            ["new.txt"],
            "已暂存组必须剔除冲突路径"
        );
        assert_eq!(
            projection
                .unstaged
                .iter()
                .map(|change| change.path)
                .collect::<Vec<_>>(),
            ["draft.txt"],
            "未暂存组必须剔除冲突路径"
        );
        assert_eq!(projection.change_count(), 4);
        assert_eq!(
            (projection.ahead, projection.plus, projection.minus),
            (2, 3, 1)
        );
    }

    /// 空快照 → 空投影(渲染层据此走空态)。
    #[test]
    fn empty_git_info_projects_no_changes() {
        let git = GitInfo::default();
        let projection = CommitProjection::from_git(&git);
        assert_eq!(projection.change_count(), 0);
        assert!(projection.history.is_empty());
        assert!(projection.branch.is_empty());
    }

    /// Agent 目录显示名按 slug 反查;缺失目录保留 exists=false 占位。
    #[test]
    fn agent_rows_map_slugs_and_keep_missing_placeholders() {
        let dirs = vec![
            AgentGlobalDir {
                cli_id: "claude".to_owned(),
                path: PathBuf::from("C:/home/.claude"),
                exists: true,
            },
            AgentGlobalDir {
                cli_id: "codex".to_owned(),
                path: PathBuf::from("C:/home/.codex"),
                exists: false,
            },
            AgentGlobalDir {
                cli_id: "unknown-cli".to_owned(),
                path: PathBuf::from("C:/home/.unknown"),
                exists: false,
            },
        ];
        let rows = agent_dir_rows(&dirs);
        assert_eq!(rows[0].display_name, "Claude Code");
        assert!(rows[0].exists);
        assert_eq!(rows[1].display_name, "Codex");
        assert!(!rows[1].exists, "缺失目录必须保留占位行");
        assert_eq!(rows[2].display_name, "unknown-cli", "未知 slug 回落原值");
    }
}
