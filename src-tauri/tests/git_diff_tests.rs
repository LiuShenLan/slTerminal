//! git_diff 命令域测试（GIT-12 拆分）
//!
//! 原 `git/mod.rs` 的 `#[cfg(test)] mod tests` 按命令拆分为独立集成测试文件。
//! 本文件覆盖：diff hunk 基础行为、pathspec/workdir 路径处理、
//! `compute_diff_hunks` 精确行级合并（含 line callback 全分支）、DiffHunk DTO serde。
//!
//! 依赖系统 git CLI（`commit_file`/`git_add` 调用 git add/commit，见 common 工厂）；
//! 最低版本 2.28——`init_temp_repo` 的 `git -c init.defaultBranch=main init`
//! （`init.defaultBranch` 配置 2.28 引入，早期版本会静默忽略该键）。版本声明由
//! Stage 17 DOC-04 收编进 `src-tauri/src/git/CLAUDE.md`（GIT-08③）。
//!
//! GIT-01：B3 段命令层化（`git_diff_impl` 直调——最小 AppState + block_on await
//! 真实命令实现）；P8+P11 段为 git2/路径**底层原语**验证（GIT-01 保留）。

mod common;

use std::fs;
use std::process::Command;

use common::{block_on, commit_file, git_add, init_temp_repo, make_app_state};
use slterminal_lib::git::{compute_diff_hunks, git_diff_impl, DiffHunk};
use slterminal_lib::AppError;

// ---- B3: git_diff 命令层测试（GIT-01：inline git2 序列重写为调真实 git_diff_impl） ----

/// happy 路径：修改第 2 行 → 精确 1 个 modified hunk（GIT-08①：存在性断言 → 精确 hunk 断言）
#[test]
fn git_diff_returns_hunks() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "diff_test.txt", "line1\nline2\nline3\n");

    // 修改第 2 行
    fs::write(path.join("diff_test.txt"), "line1\nline2 MODIFIED\nline3\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("diff_test.txt").to_string_lossy(),
    ))
    .unwrap();

    // 精确 hunk 断言（DiffHunk 无 PartialEq，逐字段比对）
    assert_eq!(hunks.len(), 1, "修改 1 行应合并为 1 个 hunk");
    assert_eq!(hunks[0].old_start, 2, "modified hunk old_start 应为 2");
    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_start, 2, "modified hunk new_start 应为 2");
    assert_eq!(hunks[0].new_lines, 1);
}

/// 空仓库（UnbornBranch）→ Ok(空 Vec)，非错误
#[test]
fn git_diff_new_file_no_head() {
    let (_dir, path) = init_temp_repo();
    // 无 commit → HEAD 为 UnbornBranch → compute_diff_hunks 返回空 Vec
    let app = make_app_state(Some(path.clone()));
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("a.txt").to_string_lossy(),
    ))
    .unwrap();
    assert!(hunks.is_empty(), "空仓库 diff 应返回空 Vec");
}

/// 未修改文件 → 空 hunks
#[test]
fn git_diff_unchanged_file_empty_hunks() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "unchanged.txt", "same content\n");

    let app = make_app_state(Some(path.clone()));
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("unchanged.txt").to_string_lossy(),
    ))
    .unwrap();
    assert!(hunks.is_empty(), "未修改文件应返回空 hunks");
}

/// 追加 3 行 → 精确 1 个纯新增 hunk（GIT-08①：存在性断言 → 精确 hunk 断言）
#[test]
fn git_diff_added_lines_hunk() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "add_lines.txt", "line1\n");

    // 追加 3 行
    fs::write(path.join("add_lines.txt"), "line1\nline2\nline3\nline4\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("add_lines.txt").to_string_lossy(),
    ))
    .unwrap();

    // 精确 hunk 断言（DiffHunk 无 PartialEq，逐字段比对）
    assert_eq!(hunks.len(), 1, "追加 3 行应合并为 1 个纯新增 hunk");
    assert_eq!(hunks[0].old_start, 0, "纯新增 hunk old_start 应为 0");
    assert_eq!(hunks[0].old_lines, 0);
    assert_eq!(hunks[0].new_start, 2, "新增首行 new_start 应为 2");
    assert_eq!(hunks[0].new_lines, 3, "新增 3 行");
}

/// 删除 2 行 → 精确 1 个纯删除 hunk（GIT-08①：存在性断言 → 精确 hunk 断言）
#[test]
fn git_diff_deleted_lines_hunk() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "del_lines.txt", "a\nb\nc\nd\ne\n");

    // 删除 2 行（d/e 与 HEAD 后部不匹配 → 删除组落在第 4-5 行）
    fs::write(path.join("del_lines.txt"), "a\nb\nc\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("del_lines.txt").to_string_lossy(),
    ))
    .unwrap();

    // 精确 hunk 断言（DiffHunk 无 PartialEq，逐字段比对）
    assert_eq!(hunks.len(), 1, "删除 2 行应合并为 1 个纯删除 hunk");
    assert_eq!(hunks[0].old_start, 4, "删除组 old_start 应为 4（d/e 两行）");
    assert_eq!(hunks[0].old_lines, 2, "删除 2 行");
    assert_eq!(hunks[0].new_start, 4);
    assert_eq!(hunks[0].new_lines, 0, "纯删除 new_lines=0");
}

/// 沙箱拒绝（SEC-01 / GIT-10）：file_path 在 project_root 外 → 拒绝
#[test]
fn git_diff_command_sandbox_rejects_file_outside_root() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "x\n");
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("f.txt");
    fs::write(&outside_file, "x\n").unwrap();

    let app = make_app_state(Some(path.clone()));
    let err = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &outside_file.to_string_lossy(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, AppError::IoKind { .. }),
        "根外 file_path 应被沙箱拒绝为 IoKind，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("项目范围"),
        "错误消息应含'项目范围'，实际: {err}"
    );
}

/// 错误契约：非 git 目录 → AppError::Git，消息含"打开仓库失败"
#[test]
fn git_diff_command_non_repo_error_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let not_repo = tmp.path().join("not_a_repo");
    std::fs::create_dir_all(&not_repo).unwrap();

    let app = make_app_state(Some(tmp.path().to_path_buf()));
    let err = block_on(git_diff_impl(
        &app,
        &not_repo.to_string_lossy(),
        &not_repo.join("a.txt").to_string_lossy(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "非 git 目录应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("打开仓库失败"),
        "错误消息应含'打开仓库失败'，实际: {err}"
    );
}

// ---- P8+P11: 路径处理验证（git2/路径底层原语：pathspec 计算与 strip，不经命令） ----

/// 验证 pathspec 使用 workdir() 而非传入的 repo_path。
/// 从子目录传入 repo_path 不影响实际的 pathspec 计算。
#[test]
fn git_diff_pathspec_uses_workdir() {
    let (_dir, path) = init_temp_repo();
    // 先创建子目录，再 commit 文件
    std::fs::create_dir_all(path.join("src")).unwrap();
    commit_file(&path, "src/main.rs", "fn main() {}\n");

    // 在子目录中修改文件
    std::fs::write(
        path.join("src").join("main.rs"),
        "fn main() { println!(); }\n",
    )
    .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let head = repo.head().unwrap();
    let tree = head.peel_to_tree().unwrap();

    // 模拟：从子目录调用 git_diff，传入 parent_dir 作为 repo_path
    let workdir = dunce::simplified(repo.workdir().unwrap());
    let file_path = path.join("src").join("main.rs");
    let rel = file_path
        .strip_prefix(workdir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    // pathspec 应为 repo-相对路径，如 "src/main.rs"
    assert_eq!(
        rel, "src/main.rs",
        "pathspec 应为 repo-相对路径而非仅文件名"
    );

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(&rel);
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
        .unwrap();

    let mut hunk_count = 0u32;
    diff.foreach(
        &mut |_delta, _num| true,
        None,
        Some(&mut |_delta, _hunk| {
            hunk_count += 1;
            true
        }),
        None,
    )
    .unwrap();
    assert!(hunk_count > 0, "正确的 pathspec 应匹配到 diff");
}

/// 从错误父目录传入 repo_path 时，workdir() 纠正后仍能正确 strip
#[test]
fn git_diff_absolute_file_path_works() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "lib.rs", "pub fn add() -> i32 { 1 }\n");
    std::fs::write(path.join("lib.rs"), "pub fn add() -> i32 { 2 }\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let head = repo.head().unwrap();
    let tree = head.peel_to_tree().unwrap();

    let workdir = dunce::simplified(repo.workdir().unwrap());
    let file_path = path.join("lib.rs");
    let rel = file_path
        .strip_prefix(workdir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(rel, "lib.rs");

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(&rel);
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
        .unwrap();

    let mut hunk_count = 0u32;
    diff.foreach(
        &mut |_delta, _num| true,
        None,
        Some(&mut |_delta, _hunk| {
            hunk_count += 1;
            true
        }),
        None,
    )
    .unwrap();
    assert!(hunk_count > 0, "绝对路径 strip 后应正确匹配");
}

/// 反斜杠路径 → pathspec 归一化为正斜杠
#[test]
fn git_diff_path_forward_slash_normalized() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "test.rs", "// comment\n");

    let repo = git2::Repository::open(&path).unwrap();
    let workdir = repo.workdir().unwrap();

    // 模拟 Windows 反斜杠路径
    let raw = format!("{}\\test.rs", workdir.display());
    let file_path_std = std::path::Path::new(&raw);
    let rel = file_path_std
        .strip_prefix(workdir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    // 不应含反斜杠
    assert!(!rel.contains('\\'), "pathspec 不应含反斜杠: {rel}");
    assert_eq!(rel, "test.rs");
}

/// 深层嵌套文件：pathspec 应为完整相对路径
#[test]
fn git_diff_deep_nested_file() {
    let (_dir, path) = init_temp_repo();
    let deep = path.join("src").join("components").join("ui");
    std::fs::create_dir_all(&deep).unwrap();
    let deep_file = deep.join("Button.tsx");
    std::fs::write(&deep_file, "export const Button = () => null;\n").unwrap();
    git_add(&path, "src/components/ui/Button.tsx");
    Command::new("git")
        .args(["commit", "-m", "add deep file"])
        .current_dir(&path)
        .output()
        .unwrap();
    std::fs::write(&deep_file, "export const Button = () => <div/>;\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let workdir = dunce::simplified(repo.workdir().unwrap());
    let rel = deep_file
        .strip_prefix(workdir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(
        rel, "src/components/ui/Button.tsx",
        "深层嵌套文件 pathspec 应为完整 repo-相对路径"
    );
}

/// 模拟 workdir 含 \\?\ 前缀时，git_diff strip_prefix 仍然成功
#[test]
fn git_diff_strip_prefix_with_verbatim_workdir() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "diff.txt", "line1\nline2\n");
    fs::write(path.join("diff.txt"), "line1\nline2 MOD\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let workdir_raw = repo.workdir().unwrap();
    let workdir = dunce::simplified(workdir_raw);

    // 模拟 fs_read_dir 风格的绝对路径（无 \\?\ 前缀）
    let file_path = path.join("diff.txt");
    let rel = file_path
        .strip_prefix(workdir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(rel, "diff.txt", "strip_prefix 应成功得到相对路径");

    // 验证 pathspec 不是绝对路径
    assert!(!rel.contains(':'), "pathspec 不应是绝对路径: {rel}");
}

/// 防御性断言：pathspec 不可能是绝对路径（含盘符）
#[test]
fn git_diff_pathspec_never_absolute() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "check.txt", "data\n");
    fs::write(path.join("check.txt"), "data2\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let workdir = dunce::simplified(repo.workdir().unwrap());
    let file_path = path.join("check.txt");
    let rel = file_path
        .strip_prefix(workdir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .replace('\\', "/");

    // 如果 rel 含盘符，说明 strip_prefix 失败，绝对是 bug
    if rel.contains(':') {
        panic!("pathspec 不应含盘符（绝对路径）: {rel} — strip_prefix 失败！");
    }
    // 正常情况：rel 为相对路径
    assert_eq!(rel, "check.txt");
}

// ---- 行级 diff 精确性测试（line callback 逻辑，直调生产 compute_diff_hunks） ----

/// 修改一行 → compute_diff_hunks 将删除+新增合并为 modified hunk
#[test]
fn git_diff_precise_single_line_modification() {
    let (_dir, path) = init_temp_repo();
    // 5 行文件，修改第 3 行
    commit_file(&path, "f.txt", "line1\nline2\nline3\nline4\nline5\n");
    fs::write(
        path.join("f.txt"),
        "line1\nline2\nline3 MODIFIED\nline4\nline5\n",
    )
    .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 修改 = 删除+新增合并为 1 个 modified hunk（生产算法：'-'→'+' → prev_was_del 配对）
    assert_eq!(hunks.len(), 1, "修改一行应合并为 1 个 modified hunk");
    assert_eq!(hunks[0].old_start, 3);
    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_start, 3);
    assert_eq!(hunks[0].new_lines, 1);
}

/// 连续新增多行 → 合并为 1 个 hunk
#[test]
fn git_diff_precise_consecutive_additions_merged() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\n");
    fs::write(path.join("f.txt"), "line1\nline2\nline3\nline4\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    assert_eq!(hunks.len(), 1, "连续新增应合并为 1 个 hunk");
    assert_eq!(hunks[0].old_lines, 0, "纯新增 old_lines=0");
    assert_eq!(hunks[0].new_lines, 3, "新增 3 行");
    assert_eq!(hunks[0].new_start, 2);
}

/// 连续删除多行 → 合并为 1 个 hunk
#[test]
fn git_diff_precise_consecutive_deletions_merged() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "a\nb\nc\nd\n");
    fs::write(path.join("f.txt"), "a\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    assert_eq!(hunks.len(), 1, "连续删除应合并为 1 个 hunk");
    assert_eq!(hunks[0].old_lines, 3, "删除 3 行");
    assert_eq!(hunks[0].new_lines, 0, "纯删除 new_lines=0");
}

/// 多处修改由 context 分隔 → 各自独立 modified hunk
#[test]
fn git_diff_precise_multiple_groups_separated_by_context() {
    let (_dir, path) = init_temp_repo();
    // 7 行，修改第 2 行和第 6 行（中间 3 行 context 分隔）
    commit_file(&path, "f.txt", "a\nb\nc\nd\ne\nf\ng\n");
    fs::write(path.join("f.txt"), "a\nB\nc\nd\ne\nF\ng\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 两处独立修改，各合并为 modified hunk
    let modified_count = hunks
        .iter()
        .filter(|h| h.old_lines > 0 && h.new_lines > 0)
        .count();
    assert_eq!(modified_count, 2, "应有 2 个独立的 modified hunk");
    let total_changed: u32 = hunks.iter().map(|h| h.new_lines + h.old_lines).sum();
    assert!(
        total_changed <= 4,
        "总变更行数不应超过 4（修改 2 行=4），实际: {total_changed}"
    );
}

/// 无修改文件 → 0 hunk
#[test]
fn git_diff_precise_no_change_returns_empty() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "unchanged\n");

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();
    assert_eq!(hunks.len(), 0, "无修改应返回 0 hunk");
}

/// GIT-05 边界①：修改后多余新增行（git/mod.rs compute_diff_hunks 321-327）
///
/// 替换（'-'→'+' 配对）+ 尾部额外新增 → 拆为 modified hunk + added hunk；
/// added hunk 的 `old_start` 必须为 0（生产算法：多余新增行 old_start 硬编码 0）。
#[test]
fn git_diff_precise_extra_added_lines_old_start_zero() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nline2\nline3\n");
    fs::write(
        path.join("f.txt"),
        "line1\nline2 NEW\nline3\nline4 NEW\nline5 NEW\n",
    )
    .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 第 2 行替换 → modified {2,1,2,1}；line4/line5 多余新增 → added，old_start=0
    // 精确断言（DiffHunk 无 PartialEq，逐字段比对）
    assert_eq!(hunks.len(), 2, "替换+插入应拆为 2 个 hunk");
    assert_eq!(hunks[0].old_start, 2, "modified hunk old_start=2");
    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_start, 2);
    assert_eq!(hunks[0].new_lines, 1);
    assert_eq!(
        hunks[1].old_start, 0,
        "多余新增行 hunk 的 old_start 必须为 0"
    );
    assert_eq!(hunks[1].old_lines, 0, "added hunk old_lines=0");
    assert_eq!(hunks[1].new_start, 4, "新增首行（line4 NEW）new_start=4");
    assert_eq!(hunks[1].new_lines, 2, "多余新增 2 行");
}

/// GIT-05 边界②：prev_was_del flush 分支（git/mod.rs compute_diff_hunks 361-372）
///
/// 生产代码存在 `if prev_was_del { if add_count > 0 { flush } }` 分支——触发前提是
/// diff 行序列出现 '+' 紧跟 '-'（新增组后接 ≥2 连续删除）。libgit2 xdiff 的发射
/// 不变量（deps/xdiff/xemit.c：同一变更组内先 '-' 后 '+'、组间必有 ≥1 context 行）
/// 保证 '+' 后紧跟 '-' 永不发生（实证：17 万+ 文件对穷举零命中），该分支为不可达
/// 死代码。本测试用交错组场景（删除组→替换→删除组）验证 prev_was_del 标志的真实
/// 流转与精确 hunk，并断言序列不变量——若 xdiff 行为变化使该分支"复活"，此测试红。
#[test]
fn git_diff_precise_interleaved_groups_flush_guard() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "b\nc\nd\ne\n");
    fs::write(path.join("f.txt"), "a\nc\ne\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 交错组：b→a 替换（modified）+ d 删除（纯删除），两 hunk 精确锁定
    // 精确断言（DiffHunk 无 PartialEq，逐字段比对）
    assert_eq!(hunks.len(), 2, "交错组应拆为 2 个 hunk");
    assert_eq!(hunks[0].old_start, 1, "b→a 替换 hunk old_start=1");
    assert_eq!(hunks[0].old_lines, 1);
    assert_eq!(hunks[0].new_start, 1);
    assert_eq!(hunks[0].new_lines, 1);
    assert_eq!(hunks[1].old_start, 3, "d 删除 hunk old_start=3");
    assert_eq!(hunks[1].old_lines, 1);
    assert_eq!(hunks[1].new_start, 3);
    assert_eq!(hunks[1].new_lines, 0, "纯删除 new_lines=0");

    // 序列不变量：'+' 后绝不紧跟 '-'（flush 分支触发前提），锁死其不可达性
    let head = repo.head().unwrap();
    let tree = head.peel_to_tree().unwrap();
    let mut opts = git2::DiffOptions::new();
    opts.pathspec("f.txt");
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
        .unwrap();
    let mut seq: Vec<char> = Vec::new();
    diff.foreach(
        &mut |_d, _n| true,
        None,
        None,
        Some(&mut |_d, _h, line| {
            seq.push(line.origin());
            true
        }),
    )
    .unwrap();
    assert!(
        !seq.windows(2).any(|w| w[0] == '+' && w[1] == '-'),
        "xdiff 序列不应出现 '+' 紧跟 '-'：{seq:?}（prev_was_del flush 不可达的前提）"
    );
}

/// GIT-05 边界③：非 UnbornBranch 的 HEAD 读取失败 → Err（git/mod.rs 264-265）
///
/// 覆写 .git/HEAD 为非法内容模拟 corrupt refs——git2 报 corrupted loose reference
/// （错误码非 UnbornBranch），生产代码应走 Err 分支而非 UnbornBranch 的空 Vec 分支。
#[test]
fn git_diff_corrupt_head_returns_err() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "content\n");
    // 模拟 corrupt refs：.git/HEAD 内容非法
    fs::write(path.join(".git").join("HEAD"), "garbage-not-a-ref\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let err = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap_err();
    assert!(
        err.to_string().contains("获取 HEAD 失败"),
        "非 UnbornBranch 的 HEAD 错误应走 Err 分支，实际: {err}"
    );
}

#[test]
fn line_callback_single_modified_line() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nline2\nline3\n");
    // 修改 line2
    fs::write(path.join("f.txt"), "line1\nline2 MODIFIED\nline3\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    assert_eq!(hunks.len(), 1, "单行修改应只有 1 个 hunk");
    assert_eq!(hunks[0].old_lines, 1, "old_lines=1");
    assert_eq!(hunks[0].new_lines, 1, "new_lines=1 → ModifiedMarker");
}

#[test]
fn line_callback_context_lines_not_included() {
    let (_dir, path) = init_temp_repo();
    // 创建一个有 10 行的文件，只修改中间 1 行
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    commit_file(&path, "f.txt", &content);
    // 修改第 5 行
    let new_content: String = (1..=10)
        .map(|i| {
            if i == 5 {
                format!("line{i} MODIFIED\n")
            } else {
                format!("line{i}\n")
            }
        })
        .collect();
    fs::write(path.join("f.txt"), &new_content).unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 只修改了 1 行，应只有 1 个 hunk，old_lines=1（不包含 context）
    assert_eq!(hunks.len(), 1, "单行修改只应有 1 个 hunk，不含 context");
    assert_eq!(hunks[0].old_lines, 1, "不应包含 context 行");
    assert_eq!(hunks[0].new_lines, 1, "不应包含 context 行");
}

#[test]
fn line_callback_pure_addition() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nline2\n");
    // 在 line1 后插入 3 行
    fs::write(path.join("f.txt"), "line1\nnewA\nnewB\nnewC\nline2\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    let added = hunks.iter().find(|h| h.old_lines == 0);
    assert!(added.is_some(), "应有纯新增 hunk（old_lines=0）");
    assert_eq!(added.unwrap().new_lines, 3, "new_lines=3（3 行新增）");
}

#[test]
fn line_callback_pure_deletion() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nline2\nline3\nline4\n");
    // 删除 line2, line3
    fs::write(path.join("f.txt"), "line1\nline4\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    let deleted = hunks.iter().find(|h| h.new_lines == 0);
    assert!(deleted.is_some(), "应有纯删除 hunk（new_lines=0）");
    assert_eq!(deleted.unwrap().old_lines, 2, "old_lines=2（2 行删除）");
}

#[test]
fn line_callback_modified_plus_extra_additions() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nline2\nline3\n");
    fs::write(
        path.join("f.txt"),
        "line1\nline2 NEW\nline3\nline4 NEW\nline5 NEW\n",
    )
    .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 应包含 modified（蓝色）hunk
    let modified = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines > 0);
    assert!(modified.is_some(), "应有 modified hunk");
    assert_eq!(
        modified.unwrap().old_lines,
        modified.unwrap().new_lines,
        "modified 的 old/new 行数应相等"
    );

    // 应包含新增（绿色）hunk（old_lines=0）
    let added = hunks.iter().find(|h| h.old_lines == 0 && h.new_lines > 0);
    assert!(added.is_some(), "应有额外新增 hunk");
    assert!(added.unwrap().new_lines > 0, "new_lines > 0");

    // 所有 hunk 的变更行数之和应覆盖全部改动
    let total_del: u32 = hunks.iter().map(|h| h.old_lines).sum();
    let total_add: u32 = hunks.iter().map(|h| h.new_lines).sum();
    assert!(total_del > 0, "应有删除行");
    assert!(total_add > total_del, "新增行应多于删除行");
}

#[test]
fn line_callback_modified_plus_extra_deletions() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "line1\nold2\nold3\nold4\nold5\n");
    fs::write(path.join("f.txt"), "line1\nnew2\nnew3\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 应包含 modified（蓝色）hunk
    let modified = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines > 0);
    assert!(modified.is_some(), "应有 modified hunk");
    assert_eq!(
        modified.unwrap().old_lines,
        modified.unwrap().new_lines,
        "modified 的 old/new 行数应相等"
    );

    // 应包含删除（灰三角）hunk（new_lines=0）
    let deleted = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines == 0);
    assert!(deleted.is_some(), "应有多余删除 hunk");
    assert!(deleted.unwrap().old_lines > 0, "old_lines > 0");

    // 删除行应多于修改行
    let total_del: u32 = hunks.iter().map(|h| h.old_lines).sum();
    let total_add: u32 = hunks.iter().map(|h| h.new_lines).sum();
    assert!(total_del > total_add, "删除行应多于新增行");
}

#[test]
fn line_callback_multiple_change_groups() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "A1\nA2\nA3\nB1\nB2\nB3\n");
    // 修改 A2 和 B2（中间有 context 行 A3/B1）
    fs::write(path.join("f.txt"), "A1\nA2 MOD\nA3\nB1\nB2 MOD\nB3\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 两处独立的修改
    let modified = hunks
        .iter()
        .filter(|h| h.old_lines > 0 && h.new_lines > 0)
        .count();
    assert_eq!(modified, 2, "应有 2 个独立的 modified hunk");
}

#[test]
fn line_callback_no_changes_returns_empty() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "unchanged\n");
    // 不修改

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    assert_eq!(hunks.len(), 0, "无修改应返回 0 hunk");
}

#[test]
fn line_callback_delete_all_lines() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "a\nb\nc\n");
    // 全部删除
    fs::write(path.join("f.txt"), "").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 全删应只有删除类 hunk（new_lines=0），无 modified 或 added
    let deleted_hunks: Vec<_> = hunks.iter().filter(|h| h.new_lines == 0).collect();
    assert!(!deleted_hunks.is_empty(), "全删应有删除 hunk");
    let total_deleted: u32 = deleted_hunks.iter().map(|h| h.old_lines).sum();
    assert_eq!(total_deleted, 3, "总计 3 行删除");
    // 不应有 modified 或 added
    assert!(hunks.iter().all(|h| h.new_lines == 0), "全删不应有新增行");
}

#[test]
fn line_callback_add_all_new_lines_after_commit() {
    let (_dir, path) = init_temp_repo();
    // 先 commit 一个空文件，再追加 3 行 → 纯新增
    commit_file(&path, "f.txt", "original\n");
    fs::write(path.join("f.txt"), "original\nnewA\nnewB\nnewC\n").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

    // 应有纯新增 hunk
    let added = hunks.iter().find(|h| h.old_lines == 0 && h.new_lines > 0);
    assert!(added.is_some(), "追加行应有 added hunk");
    assert_eq!(added.unwrap().new_lines, 3, "3 行纯新增");
}

// ---- serde CamelCase 序列化（DiffHunk DTO） ----

#[test]
fn diff_hunk_serializes_camelcase() {
    let hunk = DiffHunk {
        old_start: 10,
        old_lines: 2,
        new_start: 12,
        new_lines: 3,
    };
    let json = serde_json::to_string(&hunk).unwrap();
    assert!(
        json.contains("\"oldStart\""),
        "应包含 camelCase 字段 oldStart: {json}"
    );
    assert!(
        json.contains("\"oldLines\""),
        "应包含 camelCase 字段 oldLines: {json}"
    );
    assert!(
        json.contains("\"newStart\""),
        "应包含 camelCase 字段 newStart: {json}"
    );
    assert!(
        json.contains("\"newLines\""),
        "应包含 camelCase 字段 newLines: {json}"
    );
}

// ---- TQ-COV-06：compute_diff_hunks 错误分支闭包补测 ----

/// broken HEAD（HEAD 指向 blob，非 tree）→ peel_to_tree 失败 → AppError::Git
/// （行 293 map_err 闭包；UnbornBranch 场景已有用例走 tree=None 分支）
#[test]
fn compute_diff_hunks_broken_head_peel_tree_err() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "f.txt", "x\n");
    // blob → tag → HEAD symbolic 指向 blob（git 拒绝把非 commit 写入 branch ref，
    // tag 可指向任意对象）
    let blob = Command::new("git")
        .args(["hash-object", "-w", "f.txt"])
        .current_dir(&path)
        .output()
        .unwrap();
    let blob_sha = String::from_utf8(blob.stdout).unwrap().trim().to_string();
    Command::new("git")
        .args(["tag", "blobtag", &blob_sha])
        .current_dir(&path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/tags/blobtag"])
        .current_dir(&path)
        .output()
        .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    assert!(
        repo.head().is_ok(),
        "前置：HEAD 引用应存在（指向 blob tag）"
    );
    let err = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap_err();
    assert!(
        err.to_string().contains("获取 HEAD tree 失败"),
        "broken HEAD 应报'获取 HEAD tree 失败'，实际: {err}"
    );
}

/// bare repo（无 workdir）→ ok_or_else 闭包 → "仓库无工作目录"
/// （行 302；bare repo 的 HEAD 指向不存在的 ref → UnbornBranch → tree=None 先行）
#[test]
fn compute_diff_hunks_bare_repo_no_workdir() {
    let dir = tempfile::tempdir().unwrap();
    // 无参 `git init --bare` 将当前目录初始化为裸仓库（bare repo 无 workdir）
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let bare = dir.path().to_path_buf();

    let repo = git2::Repository::open(&bare).unwrap();
    let err = compute_diff_hunks(&repo, &bare.join("f.txt")).unwrap_err();
    assert!(
        err.to_string().contains("仓库无工作目录"),
        "bare repo 应报'仓库无工作目录'，实际: {err}"
    );
}

/// renamed 文件 diff（前端 renamed 分派传 filePath=新路径，queryPath=oldPath 查
/// HEAD 内容）——diff 无 rename detection，对工作区新路径 diff 呈「全量新增」
/// 形态（old_lines==0）且不报错（修复前传旧路径 pathspec 只匹配 DELETED 侧，
/// 形态为全量删除；修复后传新路径，行为固化）
#[test]
fn git_diff_renamed_file_returns_hunks() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "a.txt", "line1\nline2\nline3\n");

    // 工作区 rename（后端 fs_rename 同款）→ git 检测为 renamed
    std::fs::rename(path.join("a.txt"), path.join("b.txt")).unwrap();

    let app = make_app_state(Some(path.clone()));
    // 传新路径（git status 修复后 renamed 条目的 path 字段）
    let hunks = block_on(git_diff_impl(
        &app,
        &path.to_string_lossy(),
        &path.join("b.txt").to_string_lossy(),
    ))
    .unwrap();

    assert!(
        !hunks.is_empty(),
        "renamed 文件 diff 应返回 hunks（全量新增形态）"
    );
    // 全量新增：HEAD 侧 0 行、工作区侧 3 行（git diff 对 renamed 无 rename detection）
    assert_eq!(
        hunks[0].old_lines, 0,
        "renamed 全量新增形态 old_lines 应为 0"
    );
    assert_eq!(
        hunks[0].new_lines, 3,
        "renamed 全量新增形态 new_lines 应为文件行数"
    );
}
