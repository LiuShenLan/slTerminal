//! Agent 基础配置页与 Agent 文件树操作：从真实页面/树菜单进入生产链。

use std::sync::{Arc, Mutex};

use gpui::{Modifiers, MouseButton};
use slterm_settings::{AgentDisplayMode, AgentFilesConfig, AgentRoot};
use slterm_workspace::SideViewId;

use super::*;
use crate::ai_agents::AgentKind;
use crate::app_shell::app_root::settings_agents;
use crate::app_shell::app_root::settings_overlay::SettingsView;
use crate::app_shell::file_operation_service::RecycleBin;

fn root_id(seed: u8) -> slterm_settings::AgentRootId {
    let mut bytes = *uuid::Uuid::now_v7().as_bytes();
    bytes[15] ^= seed;
    slterm_settings::AgentRootId::from_bytes(bytes).unwrap()
}

fn canonical(path: std::path::PathBuf) -> CanonicalPath {
    CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap()
}

fn custom_root(
    seed: u8,
    name: &str,
    path: &std::path::Path,
    write: bool,
    terminal: bool,
) -> AgentRoot {
    AgentRoot::custom(
        root_id(seed),
        name.to_owned(),
        path.to_string_lossy().into_owned(),
    )
    .unwrap()
    .with_capabilities(slterm_settings::AgentRootCapabilities { write, terminal })
}

#[derive(Default)]
struct TestRecycleBin {
    deleted: Mutex<Vec<std::path::PathBuf>>,
}

impl RecycleBin for TestRecycleBin {
    fn send_to_recycle_bin(&self, path: &std::path::Path) -> std::io::Result<()> {
        self.deleted.lock().unwrap().push(path.to_path_buf());
        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    }
}

fn draw(window: &mut VisualTestContext) {
    draw_frame(window);
    window.run_until_parked();
}

fn wait_until(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    label: &str,
    mut predicate: impl FnMut(&AppRoot) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        if root.read_with(cx, |root, _| predicate(root)) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "等待超时: {label}");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn wait_for_entries(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    root_id: slterm_settings::AgentRootId,
    expected: &[String],
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        draw(window);
        cx.run_until_parked();
        let view = root.read_with(cx, |root, _| {
            root.side_views
                .instance(SideViewId::AgentFiles)
                .cloned()
                .expect("AgentFiles 必须已打开")
        });
        let entries = view.read_with(cx, |view, _| {
            view.agent_files_visible_entries_for_test(root_id)
        });
        if entries == expected {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "等待根层条目超时: actual={entries:?} expected={expected:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn open_agent_files_view(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.update(|window, cx| {
        let _ = root.update(cx, |root, cx| {
            root.activate_side_view(SideViewId::AgentFiles, window, cx)
        });
    });
    draw(window);
    root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::AgentFiles)
            .cloned()
            .expect("AgentFiles 必须已打开")
    })
}

fn selector(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// 等待根内某行出现并返回其可见行索引；目录读取是异步后台任务。
fn wait_for_row(window: &mut VisualTestContext, root_index: usize, name: &str) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        draw(window);
        for index in 0..64 {
            let selector = selector(format!("agent-files-row-{root_index}-{index}-{name}"));
            if window.debug_bounds(selector).is_some() {
                return index;
            }
        }
        assert!(std::time::Instant::now() < deadline, "等待树行超时: {name}");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// 等待调试元素出现；用于异步预检后才渲染的冲突面。
fn wait_for_bounds(window: &mut VisualTestContext, selector: &'static str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        draw(window);
        if window.debug_bounds(selector).is_some() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "等待元素超时: {selector}"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn click(window: &mut VisualTestContext, selector: &'static str) {
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少可点击元素: {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    draw(window);
}

fn right_click(window: &mut VisualTestContext, selector: &'static str) {
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少右键目标: {selector}"));
    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw(window);
}

fn open_agent_basic_page(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<SettingsView> {
    super::settings_pages::focus_workspace(root, window);
    super::settings_pages::open_settings(window);
    draw(window);
    click(window, "settings-page-agent-basic");
    root.read_with(cx, |root, _| {
        root.settings_overlay
            .as_ref()
            .expect("设置浮层必须已打开")
            .view
            .clone()
    })
}

fn set_view_input(
    view: &Entity<SettingsView>,
    window: &mut VisualTestContext,
    which: fn(&SettingsView) -> Option<gpui::Entity<gpui_component::input::InputState>>,
    text: &str,
) {
    window.update(|window, cx| {
        view.update(cx, |view, cx| {
            let input = which(view).expect("输入实体必须存在");
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        });
    });
}

fn custom_input(view: &SettingsView) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    view.agent_custom_input.clone()
}

fn alias_input(view: &SettingsView) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    view.agent_alias_input.clone()
}

fn runtime_input(view: &SettingsView) -> Option<gpui::Entity<gpui_component::input::InputState>> {
    view.agent_runtime_input.clone()
}

pub(super) fn saved_configs() -> (
    Arc<Mutex<Vec<AgentFilesConfig>>>,
    settings_agents::AgentFilesConfigSaveHook,
) {
    let saved = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&saved);
    let hook: settings_agents::AgentFilesConfigSaveHook =
        Arc::new(move |config: &AgentFilesConfig| {
            captured.lock().unwrap().push(config.clone());
            Ok(())
        });
    (saved, hook)
}

fn seed_agent_files_config(
    root: &Entity<AppRoot>,
    cx: &mut TestAppContext,
    config: AgentFilesConfig,
) {
    root.update(cx, |root, _| {
        root.replace_agent_files_config_for_test(config);
    });
}

/// 展示模式、自定义名单、别名与内置根运行时清单都从真实页面保存。
#[gpui::test]
fn agent_basic_page_edits_display_filter_alias_and_runtime_list(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (saved, hook) = saved_configs();
    root.update(cx, |root, _| root.set_agent_files_save_for_test(hook));
    let view = open_agent_basic_page(&root, &mut window, cx);

    click(&mut window, "settings-agent-display-custom");
    wait_until(&root, &mut window, cx, "display mode", |root| {
        root.agent_files_config
            .cli("claude")
            .is_some_and(|cli| cli.display_mode == AgentDisplayMode::Custom)
    });

    set_view_input(&view, &mut window, custom_input, "skills");
    click(&mut window, "settings-agent-custom-add");
    let pending_value = view.read_with(cx, |view, cx| {
        view.agent_custom_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
    });
    assert_eq!(
        pending_value.as_deref(),
        Some(""),
        "添加按钮必须消费输入并提交"
    );
    wait_until(&root, &mut window, cx, "custom name", |root| {
        root.agent_files_config
            .cli("claude")
            .is_some_and(|cli| cli.custom_names == ["skills"])
    });

    set_view_input(&view, &mut window, alias_input, "claude-custom");
    click(&mut window, "settings-agent-alias-add");
    wait_until(&root, &mut window, cx, "alias", |root| {
        root.agent_files_config
            .cli("claude")
            .is_some_and(|cli| cli.aliases == ["claude-custom"])
    });

    set_view_input(&view, &mut window, runtime_input, "settings.json");
    click(&mut window, "settings-agent-runtime-add");
    let claude_root = AgentKind::Claude.builtin_root_id();
    wait_until(&root, &mut window, cx, "runtime list", |root| {
        root.agent_files_config.runtime_names(claude_root) == ["settings.json"]
    });

    let saves = saved.lock().unwrap();
    let last = saves.last().expect("至少写盘一次");
    assert_eq!(
        last.cli("claude").map(|cli| cli.display_mode),
        Some(AgentDisplayMode::Custom)
    );
    assert_eq!(last.runtime_names(claude_root), ["settings.json"]);
}

/// 内置命令命名空间冲突在后台校验阶段拒绝，且不产生写盘。
#[gpui::test]
fn agent_basic_page_rejects_alias_conflict_without_writing(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (saved, hook) = saved_configs();
    root.update(cx, |root, _| root.set_agent_files_save_for_test(hook));
    let view = open_agent_basic_page(&root, &mut window, cx);

    set_view_input(&view, &mut window, alias_input, "codex");
    click(&mut window, "settings-agent-alias-add");
    let pending_value = view.read_with(cx, |view, cx| {
        view.agent_alias_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
    });
    assert_eq!(
        pending_value.as_deref(),
        Some(""),
        "别名添加按钮必须消费输入并提交"
    );
    wait_until(&root, &mut window, cx, "alias conflict", |root| {
        root.agent_files_config_error()
            == Some(settings_agents::AgentFilesConfigError::DuplicateAlias)
    });
    assert!(
        saved.lock().unwrap().is_empty(),
        "冲突别名不得进入统一设置写盘"
    );
}

/// 页级恢复默认只复位当前 CLI 与内置根清单，其他 CLI 与共享自定义根保留。
#[gpui::test]
fn agent_basic_page_reset_only_resets_current_cli_and_builtin_runtime(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let claude_root = AgentKind::Claude.builtin_root_id();
    let shared_root = root_id(9);
    let mut config = AgentFilesConfig::default();
    config.set_cli(
        "claude",
        slterm_settings::AgentCliConfig {
            display_mode: AgentDisplayMode::Custom,
            custom_names: vec!["skills".into()],
            aliases: vec!["claude-extra".into()],
            show_runtime_files: true,
        },
    );
    config.set_cli("codex", slterm_settings::AgentCliConfig::default());
    config.set_runtime_names(claude_root, vec!["settings.json".into()]);
    config.set_runtime_names(shared_root, vec!["cache".into()]);
    seed_agent_files_config(&root, cx, config);
    let (saved, hook) = saved_configs();
    root.update(cx, |root, _| root.set_agent_files_save_for_test(hook));
    let _view = open_agent_basic_page(&root, &mut window, cx);

    click(&mut window, "settings-reset-page");
    wait_until(&root, &mut window, cx, "page reset", |root| {
        root.agent_files_config.cli("claude").is_none()
            && root
                .agent_files_config
                .runtime_names(claude_root)
                .is_empty()
    });
    let config = saved.lock().unwrap().last().cloned().expect("必须写盘");
    assert!(config.cli("codex").is_some(), "其他 CLI 配置必须保留");
    assert_eq!(
        config.runtime_names(shared_root),
        ["cache"],
        "共享自定义根清单不得被页级恢复默认清除"
    );
}

/// 展示过滤配置立即作用于真实树：运行时清单隐藏、自定义模式只显示名单。
#[gpui::test]
fn agent_files_display_filter_hides_runtime_entries_and_custom_mode_lists_names(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let agent_dir = temp.path().join("claude-home");
    std::fs::create_dir(&agent_dir).unwrap();
    for name in ["settings.json", "skills", "notes"] {
        std::fs::write(agent_dir.join(name), "x\n").unwrap();
    }
    let claude_root = AgentKind::Claude.builtin_root_id();
    let dirs = vec![crate::agent_dirs::AgentGlobalDir {
        cli_id: "claude".to_owned(),
        root_id: claude_root,
        display_name: "Claude Code".to_owned(),
        path: agent_dir.clone(),
        exists: true,
    }];
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(dirs);
    });
    let mut config = AgentFilesConfig::default();
    config.set_runtime_names(claude_root, vec!["settings.json".to_owned()]);
    seed_agent_files_config(&root, cx, config);
    let _view = open_agent_files_view(&root, &mut window, cx);
    wait_for_entries(
        &root,
        &mut window,
        cx,
        claude_root,
        &["notes".to_owned(), "skills".to_owned()],
    );

    let (_, hook) = saved_configs();
    root.update(cx, |root, _| root.set_agent_files_save_for_test(hook));
    let settings = open_agent_basic_page(&root, &mut window, cx);
    set_view_input(&settings, &mut window, custom_input, "skills");
    click(&mut window, "settings-agent-custom-add");
    wait_until(&root, &mut window, cx, "custom name saved", |root| {
        root.agent_files_config
            .cli("claude")
            .is_some_and(|cli| cli.custom_names == ["skills"])
    });
    click(&mut window, "settings-agent-display-custom");
    wait_until(&root, &mut window, cx, "custom filter", |root| {
        root.agent_files_config
            .cli("claude")
            .is_some_and(|cli| cli.display_mode == AgentDisplayMode::Custom)
    });
    wait_for_entries(&root, &mut window, cx, claude_root, &["skills".to_owned()]);
}

/// 新建、重命名与删除都从真实行菜单进入统一文件操作链。
#[gpui::test]
fn agent_files_tree_creates_renames_and_deletes_through_menu(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let agent_dir = temp.path().join("ops-root");
    std::fs::create_dir_all(agent_dir.join("box")).unwrap();
    let custom = custom_root(1, "Ops", &agent_dir, true, false);
    let (root, mut window, _) = open_root(cx);
    let recycle = Arc::new(TestRecycleBin::default());
    root.update(cx, |root, _| {
        root.replace_file_operations_for_test(
            crate::app_shell::file_operation_service::FileOperationService::with_recycle_bin_for_test(
                recycle.clone(),
            ),
        );
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom]);
    });
    let view = open_agent_files_view(&root, &mut window, cx);
    wait_for_bounds(&mut window, "agent-files-root-0");
    right_click(&mut window, "agent-files-row-0-0-box");
    assert!(window.debug_bounds("agent-files-menu").is_some());
    click(&mut window, "agent-files-menu-new-file");
    let input = view.read_with(cx, |view, _| {
        view.agent_files_menu_input_for_test().expect("输入态")
    });
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("a.txt", window, cx));
    });
    click(&mut window, "agent-files-menu-save");
    let created = agent_dir.join("box/a.txt");
    wait_until(&root, &mut window, cx, "new file", |_| created.exists());

    // 展开目录后刷新到新行。
    let box_path = canonical(agent_dir.join("box"));
    window.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.agent_files_activate_path(&box_path, cx);
        });
    });
    let row = wait_for_row(&mut window, 0, "a.txt");
    let row_selector = selector(format!("agent-files-row-0-{row}-a.txt"));
    right_click(&mut window, row_selector);
    click(&mut window, "agent-files-menu-rename");
    let input = view.read_with(cx, |view, _| {
        view.agent_files_menu_input_for_test()
            .expect("重命名输入态")
    });
    window.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("b.txt", window, cx));
    });
    click(&mut window, "agent-files-menu-save");
    let renamed = agent_dir.join("box/b.txt");
    wait_until(&root, &mut window, cx, "rename", |_| {
        renamed.exists() && !created.exists()
    });

    let row = wait_for_row(&mut window, 0, "b.txt");
    right_click(
        &mut window,
        selector(format!("agent-files-row-0-{row}-b.txt")),
    );
    click(&mut window, "agent-files-menu-delete");
    click(&mut window, "agent-files-menu-confirm");
    wait_until(&root, &mut window, cx, "delete", |_| !renamed.exists());
    assert_eq!(recycle.deleted.lock().unwrap().len(), 1, "删除必须经回收站");
}

/// 只读根不提供写动作；点击禁用项不产生磁盘副作用。
#[gpui::test]
fn agent_files_read_only_root_denies_write_menu_actions(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let agent_dir = temp.path().join("readonly-root");
    std::fs::create_dir_all(agent_dir.join("box")).unwrap();
    let custom = custom_root(2, "ReadOnly", &agent_dir, false, false);
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom]);
    });
    open_agent_files_view(&root, &mut window, cx);
    wait_for_bounds(&mut window, "agent-files-root-0");
    right_click(&mut window, "agent-files-row-0-0-box");
    assert!(
        window
            .debug_bounds("agent-files-menu-write-denied")
            .is_some(),
        "只读根必须披露写入被拒"
    );
    click(&mut window, "agent-files-menu-new-file");
    assert!(
        window.debug_bounds("agent-files-menu-input").is_none(),
        "禁用动作不得进入输入态"
    );
    assert_eq!(std::fs::read_dir(agent_dir.join("box")).unwrap().count(), 0);
}

/// 跨根复制经 PasteService 执行；跨根剪切被纯转换拒绝且不移动源。
#[gpui::test]
fn agent_files_copy_paste_across_roots_and_cross_root_cut_rejected(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("clip-a");
    let target_dir = temp.path().join("clip-b");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(target_dir.join("inbox")).unwrap();
    std::fs::write(source_dir.join("one.txt"), "one\n").unwrap();
    let source = custom_root(3, "Source", &source_dir, true, false);
    let target = custom_root(4, "Target", &target_dir, true, false);
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![source, target]);
    });
    let view = open_agent_files_view(&root, &mut window, cx);
    wait_for_bounds(&mut window, "agent-files-root-0");

    right_click(&mut window, "agent-files-row-0-0-one.txt");
    click(&mut window, "agent-files-menu-copy");
    right_click(&mut window, "agent-files-row-1-0-inbox");
    click(&mut window, "agent-files-menu-paste");
    let copied = target_dir.join("inbox/one.txt");
    wait_until(&root, &mut window, cx, "cross root copy", |_| {
        copied.exists()
    });

    right_click(&mut window, "agent-files-row-0-0-one.txt");
    click(&mut window, "agent-files-menu-cut");
    right_click(&mut window, "agent-files-row-1-0-inbox");
    click(&mut window, "agent-files-menu-paste");
    // 跨根剪切在纯转换阶段拒绝：源保留、目标不变、提示可见。
    wait_for_bounds(&mut window, "agent-files-notice");
    let notice = view.read_with(cx, |view, _| view.agent_files_notice_for_test());
    assert_eq!(notice, Some(crate::i18n::Message::SidebarAgentOpFailed));
    assert!(source_dir.join("one.txt").exists(), "跨根剪切不得移动源");
}

/// 冲突聚合：跳过保留既有目标并复制其他项；替换覆盖冲突目标。
#[gpui::test]
fn agent_files_paste_conflict_skip_and_replace_partial(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("paste-a");
    let target_dir = temp.path().join("paste-b");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(target_dir.join("inbox")).unwrap();
    std::fs::write(source_dir.join("one.txt"), "new\n").unwrap();
    std::fs::write(source_dir.join("two.txt"), "two\n").unwrap();
    std::fs::write(target_dir.join("inbox/one.txt"), "old\n").unwrap();
    let source = custom_root(5, "Source", &source_dir, true, false);
    let target = custom_root(6, "Target", &target_dir, true, false);
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![source, target]);
    });
    open_agent_files_view(&root, &mut window, cx);
    wait_for_bounds(&mut window, "agent-files-root-0");

    // Ctrl 多选两个源文件后复制；菜单命中多选集。
    for name in ["one.txt", "two.txt"] {
        let index = wait_for_row(&mut window, 0, name);
        let bounds = window
            .debug_bounds(selector(format!("agent-files-row-0-{index}-{name}")))
            .unwrap();
        window.simulate_click(
            bounds.center(),
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        );
        draw(&mut window);
    }
    right_click(&mut window, "agent-files-row-0-0-one.txt");
    click(&mut window, "agent-files-menu-copy");
    right_click(&mut window, "agent-files-row-1-0-inbox");
    click(&mut window, "agent-files-menu-paste");
    wait_for_bounds(&mut window, "agent-files-paste-dialog");
    click(&mut window, "agent-files-paste-skip");
    let copied = target_dir.join("inbox/two.txt");
    wait_until(&root, &mut window, cx, "skip partial copy", |_| {
        copied.exists()
    });
    assert_eq!(
        std::fs::read_to_string(target_dir.join("inbox/one.txt")).unwrap(),
        "old\n",
        "跳过的冲突目标不得被覆盖"
    );

    right_click(&mut window, "agent-files-row-1-0-inbox");
    click(&mut window, "agent-files-menu-paste");
    wait_for_bounds(&mut window, "agent-files-paste-dialog");
    click(&mut window, "agent-files-paste-replace");
    wait_until(&root, &mut window, cx, "replace", |_| {
        std::fs::read_to_string(target_dir.join("inbox/one.txt"))
            .ok()
            .as_deref()
            == Some("new\n")
    });
}

/// “在终端中打开”经 AgentRoot 终端能力与 PathPolicy 校验后落在目标目录。
#[gpui::test]
fn agent_files_open_in_terminal_uses_agent_root_grant(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let agent_dir = temp.path().join("terminal-root");
    let project_root = temp.path().join("terminal-project");
    std::fs::create_dir_all(agent_dir.join("box")).unwrap();
    std::fs::create_dir(&project_root).unwrap();
    let custom = custom_root(7, "Terminal", &agent_dir, false, true);
    let (root, mut window, state) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    activate_project(&root, &mut window, PROJECT_A);
    root.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom]);
    });
    open_agent_files_view(&root, &mut window, cx);
    wait_for_bounds(&mut window, "agent-files-root-0");
    let attempts = state.borrow().attempts;
    right_click(&mut window, "agent-files-row-0-0-box");
    click(&mut window, "agent-files-menu-open-terminal");
    wait_until(&root, &mut window, cx, "terminal", |root| {
        root.snapshot().terminals().count() == 1
    });
    root.read_with(cx, |root, _| {
        let record = root.snapshot().terminals().next().unwrap();
        assert_eq!(
            record.cwd().as_path(),
            dunce::canonicalize(agent_dir.join("box")).unwrap(),
            "终端 cwd 必须是获准的 Agent 根目录"
        );
    });
    assert_eq!(state.borrow().attempts, attempts + 1);
}
