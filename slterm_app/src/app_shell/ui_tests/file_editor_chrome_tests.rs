//! 编辑器 chrome 关键路径(B17):reader-focus 与全屏控制。
//!
//! reader-focus = 隐藏工具条与详情栏、只留编辑器内容,退出入口固定在
//! 右上角浮层;全屏只切窗口态,不新增窗口控制命令。全程 tempdir 假文件。

use super::*;

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

/// 断言 selector 在上一帧是否绘制。
fn assert_painted(window: &mut VisualTestContext, selector: &'static str, painted: bool) {
    assert_eq!(
        window.debug_bounds(selector).is_some(),
        painted,
        "{selector} 绘制态不符(期望 {painted})"
    );
}

/// 当前窗口全屏态。
fn window_is_fullscreen(window: &mut VisualTestContext) -> bool {
    window.update(|window, _| window.is_fullscreen())
}

/// 已提交文件的临时仓库(真实 git 子进程,autocrlf 关闭保字节一致)。
fn committed_repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    run_git(directory.path(), &["init", "--initial-branch=main"]);
    run_git(directory.path(), &["config", "core.autocrlf", "false"]);
    std::fs::write(directory.path().join("tracked.txt"), "committed\n").unwrap();
    run_git(directory.path(), &["add", "tracked.txt"]);
    run_git(
        directory.path(),
        &[
            "-c",
            "user.name=slTerminal Test",
            "-c",
            "user.email=slterm@example.invalid",
            "commit",
            "-m",
            "initial",
        ],
    );
    directory
}

/// 执行 git 并断言成功。
fn run_git(cwd: &std::path::Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(cwd).args(args);
    crate::platform::process::hidden_command(&mut command);
    let output = command.output().expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} 失败: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 源码文件:reader-focus 隐藏工具条与详情栏,浮层退出入口恢复现场。
#[gpui::test]
fn reader_focus_hides_and_restores_the_toolbar(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "main.rs", b"fn main() {}\n");
    open_document_at(&workspace, &mut window, path);
    draw_frame(&mut window);

    assert_painted(&mut window, "file-toolbar", true);
    assert_painted(&mut window, "file-reader-focus", true);
    assert_painted(&mut window, "file-fullscreen", true);

    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", false);
    assert_painted(&mut window, "file-toggle-source", false);
    assert_painted(&mut window, "file-details-toggle", false);
    assert_painted(&mut window, "file-save", false);
    assert_painted(&mut window, "file-source-content", true);
    // 工具条隐藏后,右上角浮层保留唯一恢复入口。
    assert_painted(&mut window, "file-reader-focus-overlay", true);
    assert_painted(&mut window, "file-reader-focus", true);

    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", true);
    assert_painted(&mut window, "file-reader-focus-overlay", false);
    assert_painted(&mut window, "file-source-content", true);
}

/// Markdown 文档:reader-focus 同时隐藏详情栏(大纲),预览正文保留;
/// 退出后详情栏按原开关恢复。
#[gpui::test]
fn reader_focus_hides_the_details_panel_on_documents(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "note.md", "# 标题\n\n正文\n".as_bytes());
    open_document_at(&workspace, &mut window, path);
    draw_frame(&mut window);

    assert_painted(&mut window, "file-toolbar", true);
    assert_painted(&mut window, "file-details-header", true);
    assert_painted(&mut window, "markdown-outline", true);
    assert_painted(&mut window, "markdown-preview-viewport", true);

    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", false);
    assert_painted(&mut window, "file-details-header", false);
    assert_painted(&mut window, "markdown-outline", false);
    assert_painted(&mut window, "markdown-preview-viewport", true);

    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", true);
    assert_painted(&mut window, "markdown-outline", true);
    assert_painted(&mut window, "markdown-preview-viewport", true);
}

/// 全屏按钮:点击翻转 `Window::is_fullscreen`,再点恢复。
#[gpui::test]
fn fullscreen_button_toggles_the_window_state(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = write_temp_file(directory.path(), "main.rs", b"fn main() {}\n");
    open_document_at(&workspace, &mut window, path);
    draw_frame(&mut window);
    assert!(!window_is_fullscreen(&mut window), "初始必须是非全屏");

    click_selector(&mut window, "file-fullscreen");
    draw_frame(&mut window);
    assert!(window_is_fullscreen(&mut window), "点击必须进入全屏");
    assert_painted(&mut window, "file-fullscreen", true);

    click_selector(&mut window, "file-fullscreen");
    draw_frame(&mut window);
    assert!(!window_is_fullscreen(&mut window), "再次点击必须退出全屏");
    assert_painted(&mut window, "file-fullscreen", true);
}

/// gitshow 只读视图:reader-focus 与全屏控件仍显示且行为一致。
#[gpui::test]
fn git_show_view_keeps_reader_focus_and_fullscreen_controls(cx: &mut TestAppContext) {
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
    draw_frame(&mut window);

    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_read_only_for_test(cx)),
        Some(true),
        "gitshow 必须停在只读臂"
    );
    assert_painted(&mut window, "file-toolbar", true);
    assert_painted(&mut window, "file-reader-focus", true);
    assert_painted(&mut window, "file-fullscreen", true);

    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", false);
    assert_painted(&mut window, "file-source-content", true);
    assert_painted(&mut window, "file-reader-focus", true);
    click_selector(&mut window, "file-reader-focus");
    draw_frame(&mut window);
    assert_painted(&mut window, "file-toolbar", true);

    click_selector(&mut window, "file-fullscreen");
    draw_frame(&mut window);
    assert!(window_is_fullscreen(&mut window), "只读臂同样可全屏");
    click_selector(&mut window, "file-fullscreen");
    draw_frame(&mut window);
    assert!(!window_is_fullscreen(&mut window));
}
