//! 目录枚举与平铺(照抄点 21;WSL find 族已砍)。
//!
//! 深度优先按 `expanded` 展开;每目录先排序后截断([`MAX_PER_DIR`]),
//! 全树硬上限 [`MAX_ROWS`]。排序口径 = 目录在前、名字不分大小写升序。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{FileRow, gitignore};

/// 平铺行硬上限,同时约束 fs 遍历与渲染。
pub(crate) const MAX_ROWS: usize = 1000;
/// 单目录条目上限。排序**之后**截断:截断 `read_dir` 迭代器采样到的是
/// 文件系统顺序,点文件多的根会把真实源码目录挤出上限(实测教训)。
pub(crate) const MAX_PER_DIR: usize = 600;
/// 有序头部保留的内存预算(字符串 + 路径容量之和)。
const ORDERED_BYTES: usize = 512 * 1024;

/// 构建 Files 视图的平铺树快照(后台 worker 调用,不持有 `&self`)。
pub(crate) fn tree_rows(root: &Path, expanded: &HashSet<PathBuf>) -> Vec<FileRow> {
    let mut rows = Vec::new();
    // 合成 `..` 导航行:必须显式标记,普通目录逻辑不得把它当真实条目。
    if let Some(parent) = root.parent() {
        rows.push(FileRow {
            path: parent.to_path_buf(),
            name: "..".to_owned(),
            depth: 0,
            is_dir: true,
            expanded: false,
            is_parent: true,
            ignored: false,
        });
    }
    flatten_dir_into(&mut rows, expanded, root, 0);
    mark_ignored(root, &mut rows);
    rows
}

fn flatten_dir_into(
    rows: &mut Vec<FileRow>,
    expanded: &HashSet<PathBuf>,
    dir: &Path,
    depth: usize,
) {
    if rows.len() >= MAX_ROWS {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let entries = read.flatten().filter_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        // `.git` 是树噪声;其余全部显示。
        if name == ".git" {
            return None;
        }
        let is_dir = entry.file_type().ok()?.is_dir();
        Some((is_dir, name, entry.path()))
    });
    for (is_dir, name, path) in ordered_entries(entries, MAX_PER_DIR) {
        if rows.len() >= MAX_ROWS {
            return;
        }
        let is_expanded = is_dir && expanded.contains(&path);
        rows.push(FileRow {
            path: path.clone(),
            name,
            depth,
            is_dir,
            expanded: is_expanded,
            is_parent: false,
            ignored: false,
        });
        if is_expanded {
            flatten_dir_into(rows, expanded, &path, depth + 1);
        }
    }
}

/// 单目录条目排序 + 截断:目录在前,再按名字不分大小写升序。
///
/// 保持有序头部而不先全量收集:很宽的单目录(几十万条目)不能把内存
/// 和遍历成本放大到无界。
fn ordered_entries(
    entries: impl IntoIterator<Item = (bool, String, PathBuf)>,
    cap: usize,
) -> Vec<(bool, String, PathBuf)> {
    let mut kept: Vec<(bool, String, PathBuf)> = Vec::with_capacity(cap);
    let mut bytes = kept.capacity() * std::mem::size_of::<(bool, String, PathBuf)>();
    for entry in entries {
        let position = kept
            .binary_search_by(|other| {
                entry
                    .0
                    .cmp(&other.0)
                    .then(other.1.to_lowercase().cmp(&entry.1.to_lowercase()))
            })
            .unwrap_or_else(|position| position);
        if position >= cap {
            continue;
        }
        let cost = entry.1.capacity() + entry.2.capacity();
        while kept.len() >= cap || bytes + cost > ORDERED_BYTES {
            if kept.len() <= position {
                break;
            }
            let Some(removed) = kept.pop() else {
                break;
            };
            bytes -= removed.1.capacity() + removed.2.capacity();
        }
        if position <= kept.len() && kept.len() < cap && bytes + cost <= ORDERED_BYTES {
            bytes += cost;
            kept.insert(position, entry);
        }
    }
    kept
}

/// 用一条 `git check-ignore -z --stdin` 标注已排序快照。
///
/// 放在平铺**之后**执行,忽略状态永远不改变行序(边界条 8:ignored 条目
/// 仍在树中、降色渲染)。`git --no-optional-locks` 是每命令纪律(边界条 7)。
fn mark_ignored(root: &Path, rows: &mut [FileRow]) {
    use std::io::Write as _;
    use std::process::Stdio;

    // 沿祖先探测 `.git` 不 spawn 进程;不在仓库里连 check-ignore 都不发。
    if gitignore::git_repository_root(root).is_none() {
        return;
    }
    let candidates: Vec<_> = rows
        .iter()
        .filter(|row| !row.is_parent)
        .filter_map(|row| row.path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();
    if candidates.is_empty() {
        return;
    }

    let mut command = crate::git::command(root);
    command
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return;
    };
    let Some(mut stdin) = child.stdin.take() else {
        return;
    };
    for path in candidates {
        if stdin.write_all(path.as_bytes()).is_err() || stdin.write_all(&[0]).is_err() {
            return;
        }
    }
    drop(stdin);
    let Ok(output) = child.wait_with_output() else {
        return;
    };
    // 退出码 0 = 有命中;1 = 无命中;其余是真实失败。
    if !output.status.success() && output.status.code() != Some(1) {
        return;
    }
    let ignored: HashSet<String> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).replace('\\', "/"))
        .collect();
    for row in rows {
        row.ignored = row.path.strip_prefix(root).ok().is_some_and(|relative| {
            ignored.contains(&relative.to_string_lossy().replace('\\', "/"))
        });
    }
}

#[cfg(test)]
mod enumerate_tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(dir: bool, name: &str) -> (bool, String, PathBuf) {
        (dir, name.to_owned(), PathBuf::from(name))
    }

    /// 截断必须在排序之后:取有序头部,而不是 read_dir 采样序。
    #[test]
    fn per_directory_cap_keeps_the_ordered_head() {
        let raw = vec![
            entry(true, "zz-last-dir"),
            entry(true, ".tmp-scratch"),
            entry(false, "a.txt"),
            entry(true, "nebula_app"),
        ];
        let kept = ordered_entries(raw, 2);
        let names: Vec<_> = kept.iter().map(|(_, name, _)| name.as_str()).collect();
        assert_eq!(names, [".tmp-scratch", "nebula_app"]);
        let all = ordered_entries(vec![entry(false, "a.txt"), entry(true, "zz-last-dir")], 10);
        assert_eq!(all[0].1, "zz-last-dir", "目录永远排在文件前");
    }

    /// 很宽的单目录:容量与字节预算都有界,且保留字典序头部。
    #[test]
    fn wide_directory_selection_bounds_capacity_and_keeps_late_best_entries() {
        let entries = (0..50_000).rev().map(|number| {
            let name = format!("item-{number:05}-{}", "a".repeat(80));
            (false, name.clone(), PathBuf::from(name))
        });
        let kept = ordered_entries(entries, MAX_PER_DIR);
        assert_eq!(kept.len(), MAX_PER_DIR);
        assert!(kept.first().unwrap().1.starts_with("item-00000-"));
        let bytes = kept.capacity() * std::mem::size_of::<(bool, String, PathBuf)>()
            + kept
                .iter()
                .map(|row| row.1.capacity() + row.2.capacity())
                .sum::<usize>();
        assert!(bytes <= ORDERED_BYTES);
    }

    /// 平铺:目录在前、点目录隐藏、`..` 行在顶、展开集生效、忽略位可标注。
    #[test]
    fn tree_rows_flatten_dirs_first_and_follow_expansion() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::write(root.join("a.txt"), b"x").unwrap();
        std::fs::write(root.join("src/nested/inner.rs"), b"y").unwrap();

        let collapsed = tree_rows(root, &HashSet::new());
        assert_eq!(collapsed[0].name, "..");
        assert!(collapsed[0].is_parent);
        assert_eq!(collapsed[1].name, "src", "目录排在文件前");
        assert_eq!(collapsed[2].name, "a.txt");
        assert!(!collapsed.iter().any(|row| row.name == ".git"));

        let expanded: HashSet<PathBuf> = [root.join("src"), root.join("src/nested")].into();
        let rows = tree_rows(root, &expanded);
        let inner = rows
            .iter()
            .find(|row| row.name == "inner.rs")
            .expect("展开两层后必须出现深层文件");
        assert_eq!(inner.depth, 2);
        assert!(rows.iter().find(|row| row.name == "src").unwrap().expanded);
    }

    /// ignored 语义:仍在树序中,只置位不重排。
    #[test]
    fn ignored_state_does_not_change_existing_tree_order() {
        let mut rows = [
            FileRow {
                path: PathBuf::from("src"),
                name: "src".into(),
                depth: 0,
                is_dir: true,
                expanded: false,
                is_parent: false,
                ignored: false,
            },
            FileRow {
                path: PathBuf::from("target"),
                name: "target".into(),
                depth: 0,
                is_dir: true,
                expanded: false,
                is_parent: false,
                ignored: false,
            },
        ];
        let before: Vec<_> = rows.iter().map(|row| row.name.clone()).collect();
        rows[1].ignored = true;
        let after: Vec<_> = rows.iter().map(|row| row.name.clone()).collect();
        assert_eq!(before, after);
    }

    /// check-ignore 过滤索引:仓库内被忽略条目置位,非忽略条目不置位。
    #[test]
    fn mark_ignored_flags_repository_ignored_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        run_git(root, &["init", "--initial-branch=main"]);
        std::fs::write(root.join(".gitignore"), "/target/\n").unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("target/debug.log"), b"noise").unwrap();
        std::fs::write(root.join("main.rs"), b"fn main() {}").unwrap();

        let rows = tree_rows(root, &HashSet::new());
        let target = rows.iter().find(|row| row.name == "target").unwrap();
        assert!(target.ignored, "gitignore 命中的目录必须置位");
        let main = rows.iter().find(|row| row.name == "main.rs").unwrap();
        assert!(!main.ignored);
    }

    fn run_git(root: &Path, args: &[&str]) {
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(root).args(args);
        crate::platform::process::hidden_command(&mut command);
        let output = command.output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
