//! 侧栏文件树渲染与交互(07 篇 M8.4;照抄点 21/22/23/27)。
//!
//! 模型/渲染分层:本文件只按行号读 [`side_panel::SidePanel::file_rows`],
//! 不持有任何树结构。行几何以上游旧壳 `panel_layout` 为基准(34px 行距、
//! 30px 水洗)——“行缝”来自水洗比行距矮,不是 flex gap:用 gap 会把每行
//! 推成独立药丸,连续列表读感就散了。
//!
//! 交互合同:
//! - 单击目录 = 展开/折叠;单击 `..` = 切换窗口内根;
//! - 双击目录 = 以该目录为树根;双击文件 = `open_document_path`
//!   (应用内可读)或交系统处理器(未命中臂,M8.4 出口);
//! - 拖行 → 终端 = 路径原文进 PTY(规则在 `side_panel::drop_text_for_path`);
//!   拖出窗口 = 系统文件拖放(`external_drag_payload`);
//! - 右键菜单/CRUD 归 M8.5(菜单内容 = 07 篇动作面)。
//!
//! 文件图标载体 = 内嵌 Nerd Font 字形(开放问题 4 选型;色归 06 token)。

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement as _, ScrollStrategy,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::Scrollbar;
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
};

use super::{SltermWorkspace, WorkspaceTab};
use crate::app_shell::assets::REQUIRED_FONT_FAMILY;
use crate::app_shell::config::Settings;
use crate::app_shell::doc_tabs;
use crate::app_shell::file_drop::{FileDragGhost, FileTreeDrag};
use crate::i18n::Message;
use crate::side_panel::{self, SidePanel};

mod path_bar;

pub(super) use path_bar::PathEditor;

/// 行距(旧壳 `PanelLayout::row_h`)。
pub(super) const ROW_PITCH: f32 = 34.0;
/// 行水洗高度(旧壳 `row_h - 4`):行缝来自这里比行距矮。
pub(super) const ROW_WASH_H: f32 = 30.0;
/// 水洗相对抽屉内缘的额外内缩。
pub(super) const ROW_WASH_INSET: f32 = 2.0;
/// 内容左右留白。
pub(super) const DRAWER_TEXT_INSET: f32 = 4.0;

/// 右键菜单状态:挂 workspace 根渲染,不受抽屉裁剪与推出动画影响
/// (07 篇照抄点 23)。
pub(super) struct FileTreeContextMenu {
    pub(super) path: PathBuf,
    pub(super) position: gpui::Point<gpui::Pixels>,
}

/// 文件树视图状态:模型 + 输入实体 + 滚动句柄。挂在 workspace 上,
/// 渲染方法在 [`SltermWorkspace`] 的 impl 里(与旧壳同构)。
pub(super) struct FileTreeState {
    pub(super) panel: SidePanel,
    /// 搜索过滤框(gpui-component 输入实体持有 caret/IME 真值)。
    pub(super) search_input: Entity<InputState>,
    /// uniform_list 的唯一滚动模型(不再自持行偏移)。
    pub(super) scroll: UniformListScrollHandle,
    /// 路径栏编辑草稿;`None` = 只读路径按钮形态。
    pub(super) path_editor: Option<PathEditor>,
    /// 后台快照/搜索轮询泵是否在飞(单实例)。
    pub(super) pump_running: bool,
    /// 搜索框变更订阅(随视图释放)。
    pub(super) search_subscription: Option<Subscription>,
    /// 右键菜单(文件树行;删除动作入口)。
    pub(super) context_menu: Option<FileTreeContextMenu>,
    /// 删除失败等面向上抛的用户可读消息。
    pub(super) notice: Option<String>,
    /// 测试注入的系统处理器记录(生产走 `ShellExecuteW`)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) system_opened: Vec<PathBuf>,
    /// 测试注入的回收站调用记录(生产走 `SHFileOperationW`)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) recycled: Vec<PathBuf>,
    /// OS 拖出解析器调用记录(gpui 的平台断言口是 crate 私有的,
    /// 用例经注入锚点验证载荷生成)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) external_payloads: std::rc::Rc<std::cell::RefCell<Vec<(PathBuf, bool)>>>,
}

impl FileTreeState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<'_, SltermWorkspace>) -> Self {
        let language = cx.global::<Settings>().language;
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(language.text(Message::FilesSearchPlaceholder))
        });
        Self {
            panel: SidePanel::new(),
            search_input,
            scroll: UniformListScrollHandle::new(),
            path_editor: None,
            pump_running: false,
            search_subscription: None,
            context_menu: None,
            notice: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            system_opened: Vec::new(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            recycled: Vec::new(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            external_payloads: Default::default(),
        }
    }
}

impl SltermWorkspace {
    /// 装配搜索框变更订阅(需 workspace 实体已存在,故在 `new` 构造后调用)。
    pub(super) fn install_file_tree_events(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let subscription = cx.subscribe_in(
            &self.file_tree.search_input,
            window,
            Self::on_file_tree_search_event,
        );
        self.file_tree.search_subscription = Some(subscription);
    }

    /// 侧栏 explorer 槽可见时跟随聚焦 pane 的 cwd;非终端 tab 保留最后根。
    pub(super) fn sync_file_tree(&mut self, cx: &mut Context<'_, Self>) {
        if !self.explorer_open() {
            return;
        }
        let cwd = self.active_terminal_cwd(cx);
        self.file_tree.panel.sync(cwd);
        // B8:树根变化/重开 explorer 时对齐递归 watch(同根零成本)。
        let root = self
            .file_tree
            .panel
            .watch_root()
            .map(std::path::Path::to_path_buf);
        if let Some(watch) = self.fs_watch.as_mut() {
            watch.watch_tree_root(root.as_deref());
        }
        self.ensure_file_tree_pump(cx);
    }

    /// 每帧 watcher 兴趣对账(渲染根调用,explorer 关闭时也执行):
    /// - 树根 = explorer 可见时的面板根;关闭槽即释放树根 watch;
    /// - 文件兴趣 = 全部 Code tab 中 File 模式且非 gitshow 的路径;
    /// - 对账后确保事件泵在跑。
    pub(super) fn sync_fs_watch(&mut self, cx: &mut Context<'_, Self>) {
        let tree_root = if self.explorer_open() {
            self.file_tree
                .panel
                .watch_root()
                .map(std::path::Path::to_path_buf)
        } else {
            None
        };
        let files: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter_map(|entry| {
                let WorkspaceTab::Code { view } = &entry.tab else {
                    return None;
                };
                view.read(cx).editable_path().map(PathBuf::from)
            })
            .collect();
        if let Some(watch) = self.fs_watch.as_mut() {
            watch.watch_tree_root(tree_root.as_deref());
            watch.retain_files(files);
        }
        self.ensure_file_tree_pump(cx);
    }

    fn active_terminal_cwd(&self, cx: &gpui::App) -> Option<PathBuf> {
        let entry = self.tabs.get(self.active)?;
        let WorkspaceTab::Terminal { panes, focused, .. } = &entry.tab else {
            return None;
        };
        let pane = panes.iter().find(|pane| pane.id == *focused)?;
        let cwd = pane.view.read(cx).cwd.clone();
        (!cwd.is_empty()).then(|| PathBuf::from(cwd))
    }

    /// 后台轮询泵:文件树 worker 与 fs-notify 事件共用 30ms tick。
    /// 只在「面板有活」或「watcher 有兴趣」时存在;两者都消失即退出。
    fn ensure_file_tree_pump(&mut self, cx: &mut Context<'_, Self>) {
        if self.file_tree.pump_running || !self.file_tree_pump_active() {
            return;
        }
        self.file_tree.pump_running = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(30)).await;
                let keep = this
                    .update(cx, |workspace, cx| {
                        // 事件合并天然发生在两次 tick 之间:脏集按路径去重。
                        workspace.pump_fs_events(cx);
                        if workspace.file_tree.panel.harvest_pending() {
                            cx.notify();
                        }
                        workspace.file_tree_pump_active()
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
            let _ = this.update(cx, |workspace, _| workspace.file_tree.pump_running = false);
        })
        .detach();
    }

    /// 泵是否应继续:面板后台任务或 watcher 兴趣任一存在。
    fn file_tree_pump_active(&self) -> bool {
        if self.file_tree.panel.pending() {
            return true;
        }
        self.fs_watch
            .as_ref()
            .is_some_and(|watch| watch.has_interest())
    }

    /// 收割一次 fs 事件并落前台动作:
    /// - 目录脏 → 重臂面板快照(仅 explorer 可见时立即起 worker);
    /// - 文件脏 → 对路径匹配的 code tab 重读 gutter 快照。
    fn pump_fs_events(&mut self, cx: &mut Context<'_, Self>) {
        let pending = match self.fs_watch.as_mut() {
            Some(watch) => watch.take_pending(),
            None => return,
        };
        if pending.is_empty() {
            return;
        }
        if !pending.dirs.is_empty() {
            self.file_tree.panel.request_refresh();
            if self.explorer_open() {
                // 复用既有同步链:harvest + 新根对账 + worker 重臂。
                self.sync_file_tree(cx);
            }
        }
        if pending.files.is_empty() {
            return;
        }
        let dirty: std::collections::HashSet<PathBuf> = pending.files.into_iter().collect();
        let targets: Vec<Entity<crate::app_shell::code_tab::CodeTabView>> = self
            .tabs
            .iter()
            .filter_map(|entry| {
                let WorkspaceTab::Code { view } = &entry.tab else {
                    return None;
                };
                let path = view.read(cx).editable_path()?;
                dirty.contains(path).then(|| view.clone())
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        for view in targets {
            // 只读/gitshow 由视图内部 `spawn_gutter_sync` 跳过。
            view.update(cx, |view, cx| view.refresh_gutter(cx));
        }
        cx.notify();
    }

    /// 搜索框变更:推进模型 query;结果由轮询泵落地。
    fn on_file_tree_search_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.file_tree.scroll.scroll_to_item(0, ScrollStrategy::Top);
            self.file_tree
                .panel
                .set_file_search_query(input.read(cx).value().to_string());
            self.ensure_file_tree_pump(cx);
            cx.notify();
        }
    }

    /// 双击目录:以它为窗口内树根,并清掉过滤词(否则平铺结果集仍在)。
    fn browse_file_tree_to(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.file_tree.panel.browse_directory(path) {
            self.file_tree.panel.set_file_search_query(String::new());
            self.file_tree
                .search_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.file_tree.scroll.scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// 双击文件未命中应用内路由:交系统默认处理器。
    fn file_tree_open_in_system(&mut self, path: PathBuf) {
        #[cfg(all(test, feature = "gpui-test-support"))]
        {
            self.file_tree.system_opened.push(path);
        }
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        {
            crate::platform::file_operations::open_in_system(&path);
        }
    }

    /// explorer 槽内容(替换 M5.3b 的占位文案)。
    pub(super) fn render_file_tree(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        self.sync_file_tree(cx);
        let path_bar = self.render_file_tree_path(cx);
        let language = cx.global::<Settings>().language;
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let search_active = !self.file_tree.panel.search.trim().is_empty();
        let search_pending = self.file_tree.panel.file_search_pending();
        let search_error = self.file_tree.panel.file_search_error().is_some();
        let search_status: Option<SharedString> = search_active.then(|| {
            let text = if search_error {
                language.text(Message::FilesSearchFailed).to_owned()
            } else if search_pending {
                language.text(Message::FilesSearchRunning).to_owned()
            } else if self.file_tree.panel.file_index_truncated() {
                language.format(
                    Message::FilesSearchLimited,
                    &[("count", &self.file_tree.panel.file_rows().len().to_string())],
                )
            } else {
                language.format(
                    Message::FilesSearchCount,
                    &[(
                        "count",
                        &self.file_tree.panel.file_search_total().to_string(),
                    )],
                )
            };
            text.into()
        });
        let root_notice = self
            .file_tree
            .panel
            .localized_root_notice(language)
            .map(|notice| SharedString::from(notice.to_owned()));
        let delete_notice = self.file_tree.notice.clone().map(SharedString::from);
        let empty = self.file_tree_empty_state(language);
        let row_count = self.file_tree.panel.file_rows().len();
        let scroll_handle = self.file_tree.scroll.clone();
        let search_options = self.file_tree.panel.search_options();
        let search_box = div()
            .id("file-tree-search")
            .debug_selector(|| "file-tree-search".to_owned())
            .w_full()
            .flex_shrink_0()
            .h(px(30.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted)
            .overflow_hidden()
            .flex()
            .items_center()
            .child(
                div().flex_1().min_w_0().h_full().child(
                    Input::new(&self.file_tree.search_input)
                        .w_full()
                        .appearance(false)
                        .focus_bordered(false)
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).xsmall().text_color(muted))
                        .text_size(px(12.0)),
                ),
            )
            // 搜索选项三钮(07 篇 M8.4 缝合点):匹配大小写/全字/正则。
            .child(
                Button::new("file-tree-search-case")
                    .debug_selector(|| "file-tree-search-case".to_owned())
                    .label("Aa")
                    .ghost()
                    .xsmall()
                    .text_size(px(11.0))
                    .tooltip(language.text(Message::FilesSearchMatchCase))
                    .when(search_options.match_case, |button| {
                        button.text_color(theme.primary)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut options = this.file_tree.panel.search_options();
                        options.match_case = !options.match_case;
                        this.file_tree.panel.set_file_search_options(options);
                        this.ensure_file_tree_pump(cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("file-tree-search-word")
                    .debug_selector(|| "file-tree-search-word".to_owned())
                    .label("ab")
                    .ghost()
                    .xsmall()
                    .text_size(px(11.0))
                    .tooltip(language.text(Message::FilesSearchWholeWord))
                    .when(search_options.whole_word, |button| {
                        button.text_color(theme.primary)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut options = this.file_tree.panel.search_options();
                        options.whole_word = !options.whole_word;
                        this.file_tree.panel.set_file_search_options(options);
                        this.ensure_file_tree_pump(cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("file-tree-search-regex")
                    .debug_selector(|| "file-tree-search-regex".to_owned())
                    .label(".*")
                    .ghost()
                    .xsmall()
                    .text_size(px(11.0))
                    .tooltip(language.text(Message::FilesSearchRegex))
                    .when(search_options.regex, |button| {
                        button.text_color(theme.primary)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut options = this.file_tree.panel.search_options();
                        options.regex = !options.regex;
                        this.file_tree.panel.set_file_search_options(options);
                        this.ensure_file_tree_pump(cx);
                        cx.notify();
                    })),
            );

        let list = uniform_list(
            "file-tree-rows",
            row_count,
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                range
                    .map(|index| this.render_file_tree_row(index, cx))
                    .collect()
            }),
        )
        .w_full()
        .track_scroll(&scroll_handle);
        // 空态时列表高度收缩到真实行数(只剩 `..` 或 0 行),让空态提示
        // 占据剩余高度居中;有内容时吃满容器。
        let list = if empty.is_some() {
            list.h(px(row_count as f32 * ROW_PITCH))
                .flex_shrink_0()
                .with_sizing_behavior(ListSizingBehavior::Infer)
        } else {
            list.size_full()
                .with_sizing_behavior(ListSizingBehavior::Auto)
        };

        div()
            .id("file-tree")
            .debug_selector(|| "file-tree".to_owned())
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(8.0))
            .gap(px(6.0))
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().min_w_0().child(path_bar))
                    .when(self.file_tree.path_editor.is_none(), |bar| {
                        bar.child(
                            Button::new("file-tree-refresh")
                                .icon(IconName::Redo2)
                                .ghost()
                                .xsmall()
                                .tooltip(language.text(Message::FilesRefresh))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // 刷新同时恢复「跟随当前 pane」:否则路径栏/
                                    // `..` 产生的窗口内根会继续压住终端 cwd,
                                    // 按钮只会刷新旧目录。
                                    this.file_tree.panel.clear_custom_root();
                                    this.file_tree.panel.request_refresh();
                                    this.sync_file_tree(cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when_some(root_notice, |panel, notice| {
                panel.child(div().text_xs().text_color(theme.warning).child(notice))
            })
            .when_some(delete_notice, |panel, notice| {
                panel.child(div().text_xs().text_color(theme.danger).child(notice))
            })
            .child(search_box)
            .when_some(search_status, |panel, status| {
                panel.child(
                    div()
                        .flex_shrink_0()
                        .px(px(DRAWER_TEXT_INSET))
                        .text_xs()
                        .text_color(if search_error { theme.danger } else { muted })
                        .child(status),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    // 必须裁:uniform_list 会把行画到自身 bounds 之外,没有这层
                    // 末行会压过抽屉下内边距与圆角。
                    .overflow_hidden()
                    .child(list)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .w(px(16.0))
                            .child(Scrollbar::vertical(&self.file_tree.scroll)),
                    )
                    .when_some(empty, |content, (title, reason)| {
                        content.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px_4()
                                .text_center()
                                .child(
                                    div()
                                        .w_full()
                                        .max_w(px(240.0))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .items_center()
                                        .child(
                                            Icon::new(IconName::FolderOpen)
                                                .size(px(24.0))
                                                .text_color(muted),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_semibold()
                                                .text_color(theme.foreground)
                                                .child(title),
                                        )
                                        .child(div().text_xs().text_color(muted).child(reason)),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// 文件树右键菜单(挂 workspace 根;全窗遮罩点击关闭)。
    pub(super) fn render_file_tree_context_menu(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        let menu = self.file_tree.context_menu.as_ref()?;
        let position = menu.position;
        let path = menu.path.clone();
        let language = cx.global::<Settings>().language;
        let theme = cx.theme();
        let occluder = div().absolute().inset_0().occlude().on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.file_tree.context_menu = None;
                cx.stop_propagation();
                cx.notify();
            }),
        );
        let menu_element = div()
            .id("file-tree-context-menu")
            .debug_selector(|| "file-tree-context-menu".to_owned())
            .absolute()
            .left(position.x)
            .top(position.y)
            .w(px(160.0))
            .p_1()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .occlude()
            .child(
                Button::new("file-tree-context-delete")
                    .debug_selector(|| "file-tree-context-delete".to_owned())
                    .label(language.text(Message::FilesDelete))
                    .ghost()
                    .small()
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.delete_file_tree_path(path.clone(), cx);
                    })),
            );
        Some(
            div()
                .absolute()
                .inset_0()
                .child(occluder)
                .child(menu_element)
                .into_any_element(),
        )
    }

    /// 右键删除:回收站(生产 `SHFileOperationW`;测试走注入记录)。
    fn delete_file_tree_path(&mut self, path: PathBuf, cx: &mut Context<'_, Self>) {
        self.file_tree.context_menu = None;
        match self.send_path_to_recycle_bin(&path) {
            Ok(()) => {
                self.file_tree.notice = None;
                if self.file_tree.panel.selected.as_ref() == Some(&path) {
                    self.file_tree.panel.selected = None;
                }
                self.file_tree.panel.request_refresh();
                self.sync_file_tree(cx);
            }
            Err(error) => {
                let language = cx.global::<Settings>().language;
                self.file_tree.notice =
                    Some(language.format(Message::FilesDeleteFailed, &[("error", &error)]));
            }
        }
        cx.notify();
    }

    fn send_path_to_recycle_bin(&mut self, path: &Path) -> Result<(), String> {
        #[cfg(all(test, feature = "gpui-test-support"))]
        {
            self.file_tree.recycled.push(path.to_path_buf());
            Ok(())
        }
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        {
            crate::platform::file_operations::send_to_recycle_bin(path)
        }
    }

    /// 空态:标题 + 原因。判据与旧壳一致——`..` 不算内容;读不到/目录真空/
    /// 无根/搜索无命中必须分开说。
    fn file_tree_empty_state(
        &self,
        language: crate::i18n::UiLanguage,
    ) -> Option<(SharedString, SharedString)> {
        if self
            .file_tree
            .panel
            .file_rows()
            .iter()
            .any(|row| !row.is_parent)
        {
            return None;
        }
        let pair = if !self.file_tree.panel.search.trim().is_empty() {
            if self.file_tree.panel.file_search_error().is_some() {
                (
                    language.text(Message::FilesSearchFailed),
                    language.text(Message::FilesEmptyFailedHint),
                )
            } else if self.file_tree.panel.file_search_pending() {
                (
                    language.text(Message::FilesSearchRunning),
                    language.text(Message::FilesEmptyReading),
                )
            } else {
                (
                    language.text(Message::FilesSearchEmpty),
                    language.text(Message::FilesEmptyFailedHint),
                )
            }
        } else if self.file_tree.panel.snapshot_pending() {
            (
                language.text(Message::FilesEmptyReading),
                language.text(Message::FilesEmptyReading),
            )
        } else if self.file_tree.panel.enumeration_failed() {
            (
                language.text(Message::FilesEmptyFailed),
                language.text(Message::FilesEmptyFailedHint),
            )
        } else if self.file_tree.panel.root().is_none() {
            (
                language.text(Message::FilesEmptyNoRoot),
                language.text(Message::FilesEmptyNoRoot),
            )
        } else {
            (
                language.text(Message::FilesEmptyDirectory),
                language.text(Message::FilesEmptyDirectory),
            )
        };
        Some((pair.0.into(), pair.1.into()))
    }

    /// 单行:34px 行距里画 30px 水洗;行内容只按行号取模型。
    fn render_file_tree_row(&self, visible_ix: usize, cx: &Context<'_, Self>) -> AnyElement {
        let Some(row) = self.file_tree.panel.file_rows().get(visible_ix).cloned() else {
            return div().h(px(ROW_PITCH)).into_any_element();
        };
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let hover = theme.list_hover;
        let selected_bg = theme.tab_active;
        let selected_ring = theme.ring.opacity(0.16);
        let symbol_family: SharedString = REQUIRED_FONT_FAMILY.into();
        let selected = self.file_tree.panel.selected.as_ref() == Some(&row.path);
        let searching = !self.file_tree.panel.search.trim().is_empty();
        let path_hint: Option<SharedString> = searching.then(|| {
            row.path
                .parent()
                .and_then(|parent| {
                    self.file_tree
                        .panel
                        .root()
                        .and_then(|root| parent.strip_prefix(root).ok())
                })
                .map(|parent| parent.display().to_string())
                .unwrap_or_default()
                .into()
        });
        let path = row.path.clone();
        let menu_path = path.clone();
        let context_path = path.clone();
        let drag_path = path.clone();
        let drag_name = row.name.clone();
        let is_dir = row.is_dir;
        let is_parent = row.is_parent;
        let file_glyph = (!row.is_dir).then(|| side_panel::file_type_icon(&row.name));
        let chevron = row
            .is_dir
            .then(|| (!row.is_parent).then(|| side_panel::chevron_icon(row.expanded)))
            .flatten();
        let fg = if row.ignored { muted } else { theme.foreground };
        let row_background = if selected {
            selected_bg
        } else {
            gpui::transparent_black()
        };

        div()
            .h(px(ROW_PITCH))
            .w_full()
            .px(px(ROW_WASH_INSET))
            .child(
                div()
                    .id(SharedString::from(format!("file-tree-row-{visible_ix}")))
                    .debug_selector(move || format!("file-tree-row-{visible_ix}"))
                    .h(px(ROW_WASH_H))
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .pr_2()
                    .pl(px(8.0 + row.depth as f32 * 16.0))
                    .gap_1()
                    .rounded(px(6.0))
                    // 每行都带 1px 边(未选中时透明):只在选中时加边会让
                    // 行内文字横跳 1px。
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .text_color(fg)
                    .when(selected, |item| {
                        item.bg(row_background).border_color(selected_ring)
                    })
                    .hover(move |item| item.bg(hover))
                    .child(
                        div()
                            .w(px(12.0))
                            .flex_shrink_0()
                            .font_family(symbol_family.clone())
                            .text_sm()
                            .text_color(muted)
                            .child(chevron.unwrap_or("")),
                    )
                    .when(is_dir, |item| {
                        item.child(
                            div()
                                .w(px(16.0))
                                .flex_shrink_0()
                                .font_family(symbol_family.clone())
                                .text_sm()
                                .child(side_panel::folder_icon(row.expanded)),
                        )
                    })
                    .when_some(file_glyph, |item, glyph| {
                        item.child(
                            div()
                                .w(px(16.0))
                                .flex_shrink_0()
                                .font_family(symbol_family.clone())
                                .text_sm()
                                .child(glyph),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .when(row.ignored, |label| label.italic())
                            .child(row.name),
                    )
                    .when_some(path_hint.filter(|hint| !hint.is_empty()), |item, hint| {
                        item.child(
                            div()
                                .max_w(px(108.0))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(muted)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(hint),
                        )
                    })
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        if event.click_count() >= 2 && !is_parent {
                            if is_dir {
                                this.browse_file_tree_to(path.clone(), window, cx);
                            } else if doc_tabs::openable_in_app(&path) {
                                this.open_document_path(path.clone(), window, cx);
                            } else {
                                this.file_tree_open_in_system(path.clone());
                            }
                            return;
                        }
                        if is_dir {
                            if this.file_tree.panel.click_row(visible_ix) {
                                cx.notify();
                            } else {
                                this.file_tree.panel.selected = Some(path.clone());
                                cx.notify();
                            }
                        } else {
                            this.file_tree.panel.selected = Some(path.clone());
                            cx.notify();
                        }
                    }))
                    // 拖到终端 = 路径原文进 PTY(旧壳 `FileDrag` 同一合同);
                    // 拖出窗口 = 系统文件拖放。`..` 行不参与拖拽。
                    .when(!is_parent, |item| {
                        let drag = FileTreeDrag {
                            local_path: drag_path,
                            path_text: menu_path.display().to_string(),
                            name: drag_name,
                            is_dir,
                        };
                        #[cfg(all(test, feature = "gpui-test-support"))]
                        let payload_probe = self.file_tree.external_payloads.clone();
                        item.on_drag(drag.clone(), |drag, _, _, cx| {
                            cx.new(|_| FileDragGhost::new(drag.name.clone()))
                        })
                        .external_drag_payload(
                            move |drag: &FileTreeDrag, _, _| {
                                #[cfg(all(test, feature = "gpui-test-support"))]
                                payload_probe
                                    .borrow_mut()
                                    .push((drag.local_path.clone(), drag.is_dir));
                                Some(drag.external_payload())
                            },
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            // 合成 `..` 行不是真实条目,不提供删除入口。
                            if !is_parent {
                                this.file_tree.context_menu = Some(FileTreeContextMenu {
                                    path: context_path.clone(),
                                    position: event.position,
                                });
                                this.file_tree.notice = None;
                            }
                            // 阻断默认行为,避免右键落进行单击/拖拽路径。
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }

    // ── 测试专用读端 ──

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_row_names_for_test(&self) -> Vec<String> {
        self.file_tree
            .panel
            .file_rows()
            .iter()
            .map(|row| row.name.clone())
            .collect()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_root_for_test(&self) -> Option<PathBuf> {
        self.file_tree.panel.root().map(PathBuf::from)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_selected_for_test(&self) -> Option<PathBuf> {
        self.file_tree.panel.selected.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_system_opened_for_test(&self) -> Vec<PathBuf> {
        self.file_tree.system_opened.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_search_query_for_test(&self) -> String {
        self.file_tree.panel.search.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_search_options_for_test(&self) -> crate::side_panel::FileSearchOptions {
        self.file_tree.panel.search_options()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_context_menu_for_test(&self) -> Option<PathBuf> {
        self.file_tree
            .context_menu
            .as_ref()
            .map(|menu| menu.path.clone())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_recycled_for_test(&self) -> Vec<PathBuf> {
        self.file_tree.recycled.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_external_payloads_for_test(&self) -> Vec<(PathBuf, bool)> {
        self.file_tree.external_payloads.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_path_editor_active_for_test(&self) -> bool {
        self.file_tree.path_editor.is_some()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn file_tree_path_error_for_test(&self) -> Option<String> {
        self.file_tree_path_error()
    }
}

#[cfg(test)]
mod file_tree_geometry_tests {
    use super::*;

    /// 行缝来自水洗矮于行距(照抄点 22),不是 flex gap。
    #[test]
    fn row_wash_stays_shorter_than_the_row_pitch() {
        assert_eq!(ROW_PITCH, 34.0);
        assert_eq!(ROW_WASH_H, 30.0);
        const { assert!(ROW_WASH_H < ROW_PITCH) };
    }
}
