// Explorer 侧栏、右键菜单与粘贴冲突决定面的 GPUI 渲染。

use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, Context, IntoElement as _, MouseButton, ParentElement as _, Window};
use gpui_component::ActiveTheme as _;

use super::super::side_view::SideView;
use super::*;
use crate::i18n::Message;

pub(crate) fn render_explorer(view: &mut SideView, cx: &mut Context<'_, SideView>) -> AnyElement {
    let inline_input = view.explorer_inline_edit_input();
    let scroll_handle = view.explorer_scroll_handle().cloned();
    let multi_selection = view.explorer_multi_selection().cloned().unwrap_or_default();
    let Some(runtime) = &view.explorer else {
        return div().size_full().into_any_element();
    };
    let Some(scroll_handle) = scroll_handle else {
        return div().size_full().into_any_element();
    };
    let root = runtime.tree.root_path().clone();
    let show_ignored_now = runtime.show_ignored;
    let status = runtime.status.clone();
    let filter_active = !runtime.filter.is_empty();
    let rows = runtime
        .tree
        .visible_rows()
        .into_iter()
        .filter(|row| {
            show_ignored_now || !status.get(&row.path).is_some_and(|(_, ignored)| *ignored)
        })
        .collect::<Vec<_>>();
    let error = runtime.error;
    let notice = runtime.notice;
    let inline_target = runtime.inline_edit.as_ref().map(|edit| edit.target.clone());
    let filter_input = runtime.filter_input.clone();
    let show_ignored = runtime.show_ignored;
    let status_label = runtime
        .repo_root
        .as_ref()
        .map(|_| format!("changes: {}", runtime.status_entries));
    let truncated = runtime.truncated;
    let expanded = rows
        .iter()
        .filter(|row| runtime.tree.is_expanded(&row.path))
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    // 大目录只渲染可见区间，两端用等高占位维持完整滚动范围。
    let scroll_top = -f32::from(scroll_handle.offset().y);
    let viewport = f32::from(scroll_handle.bounds().size.height);
    let visible_rows = explorer_render_window(rows.len(), scroll_top, viewport);
    let mut list = div()
        .id("explorer-tree")
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .text_sm();
    if let Some(error) = error {
        let message = match error {
            ExplorerError::RootUnavailable => "无法读取项目根目录",
            ExplorerError::DirectoryRead => "无法读取目录",
        };
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "explorer-error".to_owned())
                .child(message),
        );
    }
    if rows.is_empty() && filter_active {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "explorer-filter-empty".to_owned())
                .child(language.text(Message::ExplorerFilterEmpty))
                .child(
                    div()
                        .id("explorer-filter-clear")
                        .debug_selector(|| "explorer-filter-clear".to_owned())
                        .cursor_pointer()
                        .text_color(cx.theme().accent)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|view, _, window, cx| {
                                view.explorer_clear_filter(window, cx)
                            }),
                        )
                        .child(language.text(Message::ExplorerFilterClear)),
                ),
        );
    }
    if visible_rows.start > 0 {
        list = list.child(
            div()
                .flex_none()
                .h(px(visible_rows.start as f32 * EXPLORER_ROW_HEIGHT_PX)),
        );
    }
    for (offset, row) in rows[visible_rows.clone()].iter().enumerate() {
        let index = visible_rows.start + offset;
        let selected = row.selected;
        let is_directory = row.kind == FileTreeEntryKind::Directory;
        let open = expanded.contains(&row.path);
        let relative = relative_text(&root, &row.path).unwrap_or_default();
        let label = if is_directory {
            format!("{} {}", if open { "▾" } else { "▸" }, row.name)
        } else {
            row.name.clone()
        };
        let status_char = status.get(&row.path).map(|(label, _)| *label);
        let indent = 8.0 + row.depth as f32 * 12.0;
        let selector = format!("explorer-row-{relative}");
        let editing = inline_target.as_ref() == Some(&row.path);
        let multi = multi_selection.contains(&row.path);
        let path = row.path.clone();
        let menu_path = path.clone();
        let text_color = if selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let selection = cx.theme().list_active;
        let editing_input = editing.then(|| inline_input.clone()).flatten();
        let mut row_element = div()
            .id(("explorer-row", index))
            .h(px(22.0))
            .pl(px(indent))
            .flex()
            .items_center()
            .when(selected || multi, |element| element.bg(selection))
            .text_color(text_color)
            .debug_selector(move || selector.clone())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    if event.click_count >= 2 {
                        // 双击只走激活：目录切换展开，文件打开；多选序列不得打开。
                        view.explorer_double_click_at(&path, window, cx);
                    } else if event.modifiers.control {
                        view.explorer_note_single_click();
                        view.explorer_toggle_multi_selection(&path, cx);
                    } else {
                        view.explorer_note_single_click();
                        // 单击只单选，不展开目录、不打开文件。
                        view.explorer_select_at(&path, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, _, _, cx| {
                    view.explorer_open_menu_at(&menu_path, cx);
                }),
            );
        row_element = match editing_input {
            Some(input) => row_element.child(gpui_component::input::Input::new(&input)),
            None => row_element.child(label),
        };
        if let Some(label) = status_char {
            let color = match label {
                'U' | 'D' => cx.theme().danger,
                'A' => cx.theme().success,
                'M' | 'R' => cx.theme().warning,
                _ => cx.theme().muted_foreground,
            };
            row_element = row_element.child(
                div()
                    .ml_auto()
                    .pr_2()
                    .text_color(color)
                    .debug_selector(move || format!("explorer-status-{label}"))
                    .child(label.to_string()),
            );
        }
        list = list.child(row_element);
    }
    if visible_rows.end < rows.len() {
        list = list.child(div().flex_none().h(px(
            (rows.len() - visible_rows.end) as f32 * EXPLORER_ROW_HEIGHT_PX
        )));
    }

    let mut header = div()
        .id("explorer-header")
        .flex()
        .items_center()
        .gap(px(4.0))
        .px_2()
        .py_1();
    if let Some(input) = filter_input {
        header = header.child(
            div()
                .flex_grow(1.0)
                .min_w_0()
                .child(gpui_component::input::Input::new(&input)),
        );
    }
    if let Some(label) = status_label {
        header = header.child(
            div()
                .flex_none()
                .debug_selector(|| "explorer-status".to_owned())
                .text_color(cx.theme().muted_foreground)
                .child(label),
        );
    }
    if truncated {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let count = EXPLORER_DIRECTORY_ENTRY_LIMIT.to_string();
        header = header.child(
            div()
                .flex_none()
                .debug_selector(|| "explorer-truncated".to_owned())
                .text_color(cx.theme().muted_foreground)
                .child(language.format(Message::ExplorerTruncated, &[("count", &count)])),
        );
    }
    header = header.child(
        div()
            .id("explorer-show-ignored")
            .flex_none()
            .cursor_pointer()
            .debug_selector(|| "explorer-show-ignored".to_owned())
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    let next = !view.explorer_show_ignored();
                    view.explorer_set_show_ignored(next, cx);
                }),
            )
            .child(if show_ignored {
                "ignored: on"
            } else {
                "ignored: off"
            }),
    );

    let mut panel = div()
        .id("explorer-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(header);
    if let Some(notice) = notice {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        panel = panel.child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "explorer-notice".to_owned())
                .child(language.text(notice)),
        );
    }
    panel = panel.child(list);
    if let Some(menu) = render_explorer_menu(view, cx) {
        panel = panel.child(menu);
    }
    panel.into_any_element()
}

/// 行菜单动作回调；菜单本身不做授权判定。
type ExplorerMenuAction = Box<dyn Fn(&mut SideView, &mut Window, &mut Context<'_, SideView>)>;

/// 行菜单是打开与文件操作的唯一鼠标入口；菜单本身不做授权判定。
fn render_explorer_menu(view: &mut SideView, cx: &mut Context<'_, SideView>) -> Option<AnyElement> {
    let menu_state = view.explorer.as_ref()?.menu.as_ref()?;
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let mut menu = div()
        .id("explorer-menu")
        .flex()
        .flex_col()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .debug_selector(|| "explorer-menu".to_owned());
    match menu_state {
        ExplorerMenu::Actions { kind, multi, .. } => {
            let (kind, multi) = (*kind, *multi);
            let can_open = kind == FileTreeEntryKind::File && !multi;
            let button = |id: &'static str,
                          label: &'static str,
                          enabled: bool,
                          on_click: ExplorerMenuAction| {
                let color = if enabled {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                };
                div()
                    .id(id)
                    .debug_selector(move || id.to_owned())
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(color)
                    .when(enabled, |element| element.cursor_pointer())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, window, cx| {
                            if enabled {
                                on_click(view, window, cx);
                            }
                        }),
                    )
                    .child(label)
                    .into_any_element()
            };
            menu = menu
                .child(button(
                    "explorer-menu-open",
                    language.text(Message::ExplorerMenuOpen),
                    can_open,
                    Box::new(|view, window, cx| view.explorer_menu_open(window, cx)),
                ))
                .child(button(
                    "explorer-menu-open-terminal",
                    language.text(Message::ExplorerMenuOpenTerminal),
                    !multi,
                    Box::new(|view, window, cx| view.explorer_menu_open_terminal(window, cx)),
                ))
                .child(button(
                    "explorer-menu-new-file",
                    language.text(Message::ExplorerMenuNewFile),
                    true,
                    Box::new(|view, window, cx| view.explorer_menu_new(false, window, cx)),
                ))
                .child(button(
                    "explorer-menu-new-folder",
                    language.text(Message::ExplorerMenuNewFolder),
                    true,
                    Box::new(|view, window, cx| view.explorer_menu_new(true, window, cx)),
                ))
                .child(button(
                    "explorer-menu-rename",
                    language.text(Message::ExplorerMenuRename),
                    !multi,
                    Box::new(|view, window, cx| view.explorer_menu_rename(window, cx)),
                ))
                .child(button(
                    "explorer-menu-delete",
                    language.text(Message::ExplorerMenuDelete),
                    true,
                    Box::new(|view, _, cx| view.explorer_menu_delete(cx)),
                ))
                .child(button(
                    "explorer-menu-copy",
                    language.text(Message::ExplorerMenuCopy),
                    true,
                    Box::new(|view, _, cx| {
                        view.explorer_menu_clipboard(slterm_sidebar::FileClipboardMode::Copy, cx)
                    }),
                ))
                .child(button(
                    "explorer-menu-cut",
                    language.text(Message::ExplorerMenuCut),
                    true,
                    Box::new(|view, _, cx| {
                        view.explorer_menu_clipboard(slterm_sidebar::FileClipboardMode::Cut, cx)
                    }),
                ))
                .child(button(
                    "explorer-menu-paste",
                    language.text(Message::ExplorerMenuPaste),
                    true,
                    Box::new(|view, window, cx| view.explorer_menu_paste(window, cx)),
                ))
                .child(button(
                    "explorer-menu-copy-path",
                    language.text(Message::ExplorerMenuCopyPath),
                    true,
                    Box::new(|view, _, cx| view.explorer_menu_copy_path(cx)),
                ))
                .child(button(
                    "explorer-menu-refresh",
                    language.text(Message::ExplorerMenuRefresh),
                    true,
                    Box::new(|view, _, cx| view.explorer_menu_refresh(cx)),
                ))
                .child(button(
                    "explorer-menu-cancel",
                    language.text(Message::ExplorerMenuCancel),
                    true,
                    Box::new(|view, _, cx| view.explorer_close_menu(cx)),
                ));
        }
        ExplorerMenu::Input { kind, input, .. } => {
            let kind = *kind;
            let title = match kind {
                ExplorerInputKind::NewFile => Message::ExplorerMenuNewFile,
                ExplorerInputKind::NewFolder => Message::ExplorerMenuNewFolder,
                ExplorerInputKind::Rename => Message::ExplorerMenuRename,
            };
            menu = menu
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(title)),
                )
                .child(
                    div()
                        .px_2()
                        .debug_selector(|| "explorer-menu-input".to_owned())
                        .child(gpui_component::input::Input::new(input)),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .child(
                            div()
                                .id("explorer-menu-input-save")
                                .debug_selector(|| "explorer-menu-input-save".to_owned())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().accent)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _, window, cx| {
                                        view.explorer_commit_menu_input(window, cx)
                                    }),
                                )
                                .child(language.text(Message::ExplorerConfirm)),
                        )
                        .child(
                            div()
                                .id("explorer-menu-input-cancel")
                                .debug_selector(|| "explorer-menu-input-cancel".to_owned())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().muted_foreground)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _, _, cx| {
                                        view.explorer_cancel_menu_input(cx)
                                    }),
                                )
                                .child(language.text(Message::ExplorerMenuCancel)),
                        ),
                );
        }
        ExplorerMenu::ConfirmDelete { permanent, .. } => {
            menu = menu
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .child(language.text(Message::ExplorerConfirmDeleteTitle)),
                )
                .child(
                    div()
                        .px_2()
                        .text_xs()
                        .text_color(if *permanent {
                            cx.theme().danger
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(language.text(Message::ExplorerConfirmDeleteBody)),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .child(
                            div()
                                .id("explorer-menu-delete-confirm")
                                .debug_selector(|| "explorer-menu-delete-confirm".to_owned())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().danger)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _, window, cx| {
                                        view.explorer_confirm_delete(window, cx)
                                    }),
                                )
                                .child(language.text(Message::ExplorerConfirm)),
                        )
                        .child(
                            div()
                                .id("explorer-menu-delete-cancel")
                                .debug_selector(|| "explorer-menu-delete-cancel".to_owned())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(cx.theme().muted_foreground)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _, _, cx| view.explorer_close_menu(cx)),
                                )
                                .child(language.text(Message::ExplorerMenuCancel)),
                        ),
                );
        }
    }
    Some(menu.into_any_element())
}

/// Explorer 粘贴冲突决定面；只有存在冲突预检结果时出现。
pub(crate) fn render_explorer_paste_dialog(
    root: &AppRoot,
    cx: &mut Context<'_, AppRoot>,
) -> Option<AnyElement> {
    let pending = root.explorer_paste.as_ref()?;
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let conflicts = pending.prepared.conflicts().len().to_string();
    Some(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(cx.theme().background.opacity(0.7))
            .debug_selector(|| "explorer-paste-dialog".to_owned())
            .child(
                div()
                    .absolute()
                    .top(px(90.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .w(px(520.0))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().foreground)
                            .child(
                                div()
                                    .text_sm()
                                    .child(language.text(Message::ExplorerPasteConflictTitle)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text(Message::ExplorerPasteConflictBody)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(conflicts),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap_3()
                                    .child(dialog_button(
                                        "explorer-paste-cancel",
                                        language.text(Message::ExplorerPasteCancel),
                                        cx.theme().foreground,
                                        cx.listener(|root, _, _, cx| {
                                            root.explorer_paste_cancel(cx);
                                        }),
                                    ))
                                    .child(dialog_button(
                                        "explorer-paste-skip",
                                        language.text(Message::ExplorerPasteSkip),
                                        cx.theme().accent,
                                        cx.listener(|root, _, _, cx| {
                                            root.explorer_paste_decide(
                                                crate::app_shell::paste_service::PasteConflictDecision::Skip,
                                                cx,
                                            );
                                        }),
                                    ))
                                    .child(dialog_button(
                                        "explorer-paste-replace",
                                        language.text(Message::ExplorerPasteReplace),
                                        cx.theme().danger,
                                        cx.listener(|root, _, _, cx| {
                                            root.explorer_paste_decide(
                                                crate::app_shell::paste_service::PasteConflictDecision::Replace,
                                                cx,
                                            );
                                        }),
                                    )),
                            ),
                    ),
            )
            .into_any_element(),
    )
}

fn dialog_button(
    debug_id: &'static str,
    label: &'static str,
    color: gpui::Hsla,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(debug_id)
        .debug_selector(move || debug_id.to_owned())
        .px_1()
        .text_xs()
        .cursor_pointer()
        .text_color(color)
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
        .into_any_element()
}

#[cfg(test)]
mod explorer_status_tests {
    use super::*;
    use std::str::FromStr as _;

    /// 单目录读取有上限，超限必须显式报告截断而不是静默伪装成完整目录。
    #[test]
    fn directory_read_is_capped_and_reports_truncation() {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..5 {
            std::fs::write(temp.path().join(format!("f{index}.txt")), b"x").unwrap();
        }
        let root = CanonicalPath::new(temp.path()).unwrap();
        let capped = read_directory_limited(&root, 3).unwrap();
        assert_eq!(capped.entries.len(), 3);
        assert!(capped.truncated, "超过上限必须报告截断");

        let complete = read_directory_limited(&root, 5).unwrap();
        assert_eq!(complete.entries.len(), 5);
        assert!(!complete.truncated, "恰好等于上限不算截断");
    }

    /// 渲染窗口有界并跟随滚动；首尾区间不得越过真实行数。
    #[test]
    fn explorer_render_window_is_bounded_and_follows_scroll() {
        let total = 10_000;
        let top = explorer_render_window(total, 0.0, 220.0);
        assert_eq!(top.start, 0);
        assert!(top.end - top.start <= 60, "首屏只渲染视口与 overscan");

        let scrolled = explorer_render_window(total, 2_200.0, 220.0);
        assert_eq!(scrolled.start, 100 - EXPLORER_RENDER_OVERSCAN);
        assert!(scrolled.end < total);

        let last = explorer_render_window(total, 1_000_000.0, 220.0);
        assert_eq!(last.end, total, "末尾区间必须夹紧到总行数");
    }

    /// 根切换推进代次并丢弃旧合并位；在途回执只结算当前代次。
    #[test]
    fn explorer_runtime_advances_generation_and_defers_status() {
        let project = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let root = CanonicalPath::new("C:/work/a").unwrap();
        let mut runtime = ExplorerRuntime::new(project, root.clone(), None);
        let first = runtime.generation;
        runtime.defer_status();
        assert!(runtime.take_deferred_status());

        runtime.reset(project, root, None);

        assert!(runtime.generation > first, "根切换必须推进代次");
        assert!(!runtime.take_deferred_status(), "新根不得继承旧合并位");
    }
}
