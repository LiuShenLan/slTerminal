//! git gutter(07 篇 M8.5 新建):按行标记增/改/删。
//!
//! 数据源与文件树 git 快照同一子进程执行体(D07-1):`crate::git` 的
//! `file_status` + `file_hunks`,每命令 `--no-optional-locks`。文件级状态是
//! 零成本基线;hunk 可得时细化到首个变更行(设计稿改造节 3 的可选增强)。
//!
//! 只读文档(truncated/gitshow/diff)不挂 gutter——无编辑域即无行标意义。

use std::path::Path;

use crate::git::{self, FileHunk, FileStatus};

/// guter 行标值集(与 vcs 子进程状态值集实现期归一)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GutterMark {
    Untracked,
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflicted,
}

/// 按行号升序的行标集 + 重渲 revision(单调递增)。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GitGutter {
    marks: Vec<(u32, GutterMark)>,
    revision: u64,
}

impl GitGutter {
    pub(crate) fn marks(&self) -> &[(u32, GutterMark)] {
        &self.marks
    }

    #[allow(dead_code)] // 消费点 = gutter revision 断言端(渲染按 marks 直接重算)。
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// 状态 + hunk → 行标集(纯函数投影,供注入用例锁定)。
    pub(crate) fn from_status(status: Option<FileStatus>, hunks: &[FileHunk]) -> Self {
        Self {
            marks: marks_from_status(status, hunks),
            revision: 0,
        }
    }

    /// 采纳一份新快照:行标变化才推进 revision(渲染方按 revision 重渲)。
    pub(crate) fn sync(&mut self, next: GitGutter) -> bool {
        if self.marks == next.marks {
            return false;
        }
        self.marks = next.marks;
        self.revision = self.revision.wrapping_add(1);
        true
    }
}

/// 状态 + hunk → 行升序标记。规则:
/// - `new_lines == 0` → 该行位一条删除标记;
/// - `old_lines == 0` → 新增行;两边都有 → 修改行;
/// - hunk 不可得时退回文件级基线(第 1 行);
/// - 删除/重命名/冲突/未跟踪即使有 hunk 也补一条文件级标记。
pub(crate) fn marks_from_status(
    status: Option<FileStatus>,
    hunks: &[FileHunk],
) -> Vec<(u32, GutterMark)> {
    let mut marks = Vec::new();
    for hunk in hunks {
        if hunk.new_lines == 0 {
            marks.push((hunk.new_start.max(1), GutterMark::Deleted));
            continue;
        }
        let mark = if hunk.old_lines == 0 {
            GutterMark::Added
        } else {
            GutterMark::Modified
        };
        let end = hunk.new_start.saturating_add(hunk.new_lines);
        for line in hunk.new_start..end {
            marks.push((line.max(1), mark));
        }
    }
    if let Some(status) = status {
        let file_level = gutter_for_status(status);
        if marks.is_empty()
            || matches!(
                status,
                FileStatus::Deleted
                    | FileStatus::Renamed
                    | FileStatus::Conflicted
                    | FileStatus::Untracked
            )
        {
            marks.push((1, file_level));
        }
    }
    marks.sort_by_key(|(line, _)| *line);
    marks.dedup_by_key(|(line, _)| *line);
    marks
}

fn gutter_for_status(status: FileStatus) -> GutterMark {
    match status {
        FileStatus::Untracked => GutterMark::Untracked,
        FileStatus::Added => GutterMark::Added,
        FileStatus::Modified => GutterMark::Modified,
        FileStatus::Deleted => GutterMark::Deleted,
        FileStatus::Renamed => GutterMark::Renamed,
        FileStatus::Conflicted => GutterMark::Conflicted,
    }
}

/// 读文件当前 gutter 快照(后台线程调用;非仓库/git 缺失 = 空行标)。
pub(crate) fn read_for_path(path: &Path) -> GitGutter {
    let Ok(root) = git::repository_root(path) else {
        return GitGutter::default();
    };
    let Ok(relative) = git::relative_path(&root, path) else {
        return GitGutter::default();
    };
    let status = git::file_status(&root, &relative).unwrap_or(None);
    let hunks = git::file_hunks(&root, &relative).unwrap_or_default();
    GitGutter::from_status(status, &hunks)
}

#[cfg(test)]
mod gutter_tests {
    use super::*;
    use std::process::Command;

    /// 纯映射:新增/修改/删除行号与升序去重。
    #[test]
    fn hunks_map_to_ascending_line_marks() {
        let hunks = [
            FileHunk {
                old_start: 2,
                old_lines: 2,
                new_start: 2,
                new_lines: 3,
            },
            FileHunk {
                old_start: 9,
                old_lines: 1,
                new_start: 10,
                new_lines: 0,
            },
        ];
        let marks = marks_from_status(Some(FileStatus::Modified), &hunks);
        assert_eq!(
            marks,
            [
                (2, GutterMark::Modified),
                (3, GutterMark::Modified),
                (4, GutterMark::Modified),
                (10, GutterMark::Deleted),
            ]
        );
        let added = marks_from_status(
            Some(FileStatus::Added),
            &[FileHunk {
                old_start: 0,
                old_lines: 0,
                new_start: 1,
                new_lines: 2,
            }],
        );
        assert_eq!(added, [(1, GutterMark::Added), (2, GutterMark::Added)]);
    }

    /// revision 只在行标变化时推进。
    #[test]
    fn revision_bumps_only_when_marks_change() {
        let mut gutter = GitGutter::default();
        assert!(gutter.sync(GitGutter::from_status(
            Some(FileStatus::Modified),
            &[FileHunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }]
        )));
        let revision = gutter.revision();
        assert!(!gutter.sync(GitGutter::from_status(
            Some(FileStatus::Modified),
            &[FileHunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }]
        )));
        assert_eq!(gutter.revision(), revision);
    }

    /// 真 git 仓库:修改/新增行进入行标;未跟踪文件走文件级基线。
    #[test]
    fn real_repository_produces_line_marks() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        git_run(root, &["init", "--initial-branch=main"]);
        std::fs::write(root.join("tracked.txt"), b"one\ntwo\nthree\n").unwrap();
        git_run(root, &["add", "tracked.txt"]);
        git_run(
            root,
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
        let path = root.join("tracked.txt");
        std::fs::write(&path, b"one\nchanged\nthree\nfour\n").unwrap();

        let gutter = read_for_path(&path);
        assert!(
            gutter
                .marks()
                .iter()
                .any(|(line, mark)| *line == 2 && *mark == GutterMark::Modified),
            "第 2 行必须标修改: {:?}",
            gutter.marks()
        );
        assert!(
            gutter
                .marks()
                .iter()
                .any(|(line, mark)| *line == 4 && *mark == GutterMark::Added),
            "第 4 行必须标新增: {:?}",
            gutter.marks()
        );

        let fresh = root.join("fresh.txt");
        std::fs::write(&fresh, b"new\n").unwrap();
        let untracked = read_for_path(&fresh);
        assert_eq!(untracked.marks(), [(1, GutterMark::Untracked)]);

        let outside = tempfile::tempdir().unwrap();
        let plain = outside.path().join("plain.txt");
        std::fs::write(&plain, b"plain\n").unwrap();
        assert!(read_for_path(&plain).marks().is_empty(), "非仓库零标记");
    }

    fn git_run(root: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        command.arg("-C").arg(root).args(args);
        crate::platform::process::hidden_command(&mut command)
            .output()
            .expect("run git");
    }
}
