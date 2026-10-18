//! M8 文件与编辑关键路径(L4 虚拟窗口):打开路由/只读臂(M8.1)、
//! 源码态编辑 + 保存链 + 脏关闭确认(M8.2)。
//!
//! 全程 tempdir 假文件;git 夹具用真实子进程,argv 不经 shell。

use super::*;
use crate::app_shell::file_editor::SaveFile;
use crate::i18n::Message;

// ── M8.1 文件 tab 路由 + 只读臂 ──

/// 写一个临时文件(自动建父目录),返回路径。
fn write_temp_file(directory: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = directory.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, bytes).unwrap();
    path
}

/// 走生产入口打开文件 tab,并推进后台载入。
fn open_document_at(
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    path: std::path::PathBuf,
) {
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.open_document_path(path, window, cx);
        });
    });
    window.run_until_parked();
    window.refresh().unwrap();
}

/// 画一帧并断言 tab 容器的 debug_bounds 非零。
fn assert_file_tab_paints(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} 必须绘制"));
    assert!(
        bounds.size.width > px(0.0) && bounds.size.height > px(0.0),
        "{selector} 矩形必须非零: {bounds:?}"
    );
}

/// 生成 2×2 PNG(图片 tab 解码路径的真实输入)。
fn png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

/// 源码文件 → Code tab:快照文本 + 渲染矩形非零;M8.2 起可编辑;同路径重开复用 tab。
#[gpui::test]
fn code_tab_renders_opened_source_file(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "main.rs", b"fn main() {}\n");

    open_document_at(&workspace, &mut window, path.clone());
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "code"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_text_for_test(cx)),
        Some("fn main() {}\n".to_owned()),
        "Code tab 必须载入磁盘快照"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_read_only_for_test(cx)),
        Some(false),
        "M8.2 起普通源码 tab 源码态可编辑"
    );
    assert_file_tab_paints(&mut window, "slterm-code-tab");

    // 同路径再次打开:激活既有 tab,不堆第二份。
    open_document_at(&workspace, &mut window, path);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        2,
        "terminal + 复用后的 code = 2"
    );
}

/// Markdown → Document tab:预览渲染态归 M8.3,M8.1 以只读文本占位。
#[gpui::test]
fn document_tab_renders_markdown_file(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "note.md", "# 标题\n正文\n".as_bytes());

    open_document_at(&workspace, &mut window, path);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "document"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_text_for_test(cx)),
        Some("# 标题\n正文\n".to_owned())
    );
    assert_file_tab_paints(&mut window, "slterm-document-tab");
}

/// 图片 → Image tab:后台 BGRA 解码完成 + 渲染矩形非零(缩放归 M8.3)。
#[gpui::test]
fn image_tab_decodes_and_renders_opened_image(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "pixel.png", &png_bytes());

    open_document_at(&workspace, &mut window, path);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "image"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_image_loaded_for_test(cx)),
        Some(true),
        "后台解码必须完成"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_image_zoom_for_test(cx)),
        Some(1.0),
        "M8.3 前的占位缩放恒 1.0(fit-width)"
    );
    assert_file_tab_paints(&mut window, "slterm-image-tab");
}

/// 快照只收 Terminal tab;三类文件 tab 的关闭语义与 Settings 一致。
#[gpui::test]
fn file_tabs_stay_out_of_the_session_snapshot_and_close_uniformly(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let code = write_temp_file(directory.path(), "lib.rs", b"pub fn f() {}\n");
    let doc = write_temp_file(directory.path(), "notes.md", b"# notes\n");
    let image = write_temp_file(directory.path(), "dot.png", &png_bytes());

    for path in [code, doc, image] {
        open_document_at(&workspace, &mut window, path);
    }
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 4);
    let snapshot = workspace.read_with(cx, |ws, cx| ws.snapshot_for_test(None, cx));
    assert_eq!(
        snapshot.tabs.len(),
        1,
        "Code/Document/Image 不得进 session 快照(快照只收 Terminal tab)"
    );

    for expected in [3, 2, 1] {
        window.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.close_active_tab_for_test(window, cx);
            });
        });
        window.run_until_parked();
        assert_eq!(
            workspace.read_with(cx, |ws, _| ws.tab_count()),
            expected,
            "文件 tab 必须按统一关闭语义摘除"
        );
    }
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "terminal",
        "幸存 tab 必须是终端"
    );
    draw_frame(&mut window);
    for selector in ["slterm-code-tab", "slterm-document-tab", "slterm-image-tab"] {
        assert!(
            window.debug_bounds(selector).is_none(),
            "{selector} 关闭后不得继续绘制"
        );
    }
    let snapshot = workspace.read_with(cx, |ws, cx| ws.snapshot_for_test(None, cx));
    assert_eq!(snapshot.tabs.len(), 1);
}

/// gitshow 只读臂:HEAD 字节(非工作区),标题 `@ HEAD`,只读不可编辑。
#[gpui::test]
fn git_show_code_tab_renders_head_bytes_read_only(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let repository = committed_repository();
    let path = repository.path().join("tracked.txt");
    std::fs::write(&path, "worktree edit\n").unwrap();

    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.open_git_show_tab(path, window, cx);
        });
    });
    window.run_until_parked();
    window.refresh().unwrap();

    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "code",
        "gitshow 复用 Code tab 变体"
    );
    let title = workspace.read_with(cx, |ws, cx| ws.active_tab_title_for_test(cx));
    assert!(title.ends_with(" @ HEAD"), "gitshow 标题: {title}");
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_text_for_test(cx)),
        Some("committed\n".to_owned()),
        "必须展示 HEAD 内容而非工作区"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_read_only_for_test(cx)),
        Some(true),
        "gitshow 是只读臂"
    );
    assert_file_tab_paints(&mut window, "slterm-code-tab");

    // 只读不可编辑:键入不得改变快照文本(disabled 输入体 + 类型层只读)。
    window.simulate_keystrokes("x");
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_text_for_test(cx)),
        Some("committed\n".to_owned()),
        "只读臂不得接受编辑"
    );
}

/// 已提交文件的临时仓库(autocrlf 关闭,HEAD 字节与写入逐字节一致)。
fn committed_repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    assert!(
        run_git(directory.path(), &["init", "--initial-branch=main"])
            .status
            .success()
    );
    std::fs::write(directory.path().join("tracked.txt"), "committed\n").unwrap();
    assert!(
        run_git(directory.path(), &["add", "tracked.txt"])
            .status
            .success()
    );
    let commit = run_git(directory.path(), &["commit", "-m", "initial"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    directory
}

fn run_git(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(cwd)
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "user.name=slTerminal Test",
            "-c",
            "user.email=slterm@example.invalid",
        ])
        .args(args);
    crate::platform::process::hidden_command(&mut command)
        .output()
        .expect("run git")
}

// ── M8.2 编辑器内核:源码态编辑 + 保存 + 脏确认 ──

/// 取当前界面语言的文案(提示框按钮标签必须与生产渲染逐字一致)。
fn ui_label(cx: &mut TestAppContext, message: Message) -> String {
    cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .language
            .text(message)
            .to_owned()
    })
}

/// 焦点交活跃文件 tab(键入前置)。
fn focus_active_file_tab(
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
) {
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.focus_active_for_test(window, cx);
        });
    });
    window.run_until_parked();
}

/// 源码态可编辑:编辑 → 脏标记「•」→ Ctrl+S → 磁盘字节正确 → 脏标记清除。
#[gpui::test]
fn source_editor_edit_save_round_trip_writes_bytes_and_clears_dirty(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "main.rs", b"fn main() {}\n");
    open_document_at(&workspace, &mut window, path.clone());
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_read_only_for_test(cx)),
        Some(false),
        "M8.2 普通源码 tab 必须可编辑"
    );
    focus_active_file_tab(&workspace, &mut window);

    // 多行输入载入后光标在 0..0(组件库语义):键入落在文件开头。
    window.simulate_input("// ");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_editor_text_for_test(cx)),
        Some("// fn main() {}\n".to_owned()),
        "键入必须进入工作副本"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_dirty_for_test(cx)),
        Some(true),
        "编辑后必须置脏"
    );
    let dirty_title = workspace.read_with(cx, |ws, cx| ws.active_tab_title_for_test(cx));
    assert!(
        dirty_title.ends_with(" •"),
        "脏标记必须进 tab 标题: {dirty_title}"
    );

    // 生产键位路径:Ctrl+S 经由工作区动作分发。
    window.dispatch_action(SaveFile);
    window.run_until_parked();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        "// fn main() {}\n".as_bytes(),
        "保存必须把工作副本逐字节写回磁盘"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_dirty_for_test(cx)),
        Some(false),
        "保存成功后脏标记必须清除"
    );
    let clean_title = workspace.read_with(cx, |ws, cx| ws.active_tab_title_for_test(cx));
    assert!(
        !clean_title.ends_with('•'),
        "保存后 tab 标题不得再带脏标记: {clean_title}"
    );
}

/// 关闭脏 tab 必须走确认:取消保留草稿;保存关闭写盘;丢弃关闭不写盘。
#[gpui::test]
fn closing_a_dirty_file_tab_confirms_before_save_or_discard(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "edit.rs", b"initial\n");
    open_document_at(&workspace, &mut window, path.clone());
    focus_active_file_tab(&workspace, &mut window);
    window.simulate_input("draft ");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_dirty_for_test(cx)),
        Some(true)
    );

    // 取消:tab 保留、草稿与脏标记原样。
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.close_active_tab_for_test(window, cx);
        });
    });
    window.run_until_parked();
    assert!(window.has_pending_prompt(), "脏关闭必须弹确认框");
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        2,
        "确认在途不得摘除 tab"
    );
    let cancel = ui_label(cx, Message::EditorCancel);
    window.simulate_prompt_answer(&cancel);
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        2,
        "取消后 tab 必须保留"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_dirty_for_test(cx)),
        Some(true)
    );

    // 保存并关闭:磁盘写入工作副本,tab 摘除。
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.close_active_tab_for_test(window, cx);
        });
    });
    window.run_until_parked();
    assert!(window.has_pending_prompt());
    let save = ui_label(cx, Message::EditorSave);
    window.simulate_prompt_answer(&save);
    window.run_until_parked();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        "draft initial\n".as_bytes(),
        "保存关闭必须落盘"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        1,
        "保存成功后 tab 摘除"
    );

    // 丢弃并关闭:重开同一文件,编辑后丢弃,磁盘保持原样。
    open_document_at(&workspace, &mut window, path.clone());
    focus_active_file_tab(&workspace, &mut window);
    window.simulate_input("discard ");
    window.run_until_parked();
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.close_active_tab_for_test(window, cx);
        });
    });
    window.run_until_parked();
    assert!(window.has_pending_prompt());
    let discard = ui_label(cx, Message::EditorDiscard);
    window.simulate_prompt_answer(&discard);
    window.run_until_parked();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        "draft initial\n".as_bytes(),
        "丢弃关闭不得写盘"
    );
    assert_eq!(workspace.read_with(cx, |ws, _| ws.tab_count()), 1);
}
