//! gitignore 仓库根探测与忽略规则形态(照抄点 21)。
//!
//! 本步消费面 = 文件树过滤索引的仓库门控([`super::enumerate`] 调用
//! [`git_repository_root`]);规则写入/移除的菜单入口归 M8.5,故
//! [`gitignore_entry`] 暂为测试锁定形态(cfg(test) 编译,消费点注释登记)。

use std::path::{Path, PathBuf};

/// 从 `path` 往上找 Git 仓库根(含 `.git` 的目录)。
///
/// 不 spawn `git rev-parse --show-toplevel`:判据只是「有没有 `.git`」,
/// 走文件系统就够,而它在文件树快照线程里每次刷新都调用——起进程纯属浪费。
/// `.git` 是目录(普通仓库)或文件(worktree/submodule 的 gitdir 指针)都算。
pub(crate) fn git_repository_root(path: &Path) -> Option<PathBuf> {
    // 文件的规则要写进它所在目录的仓库,所以从父目录起步。
    let start = if path.is_dir() { path } else { path.parent()? };
    let mut probe = Some(start);
    while let Some(current) = probe {
        if current.join(".git").exists() {
            return Some(current.to_owned());
        }
        probe = current.parent();
    }
    None
}

/// 一条锚定到仓库根、转义 glob 元字符的 gitignore 规则。
///
/// 三个刻意选择(照抄因果):
/// - 前置 `/` 锚定:不锚定的 `foo.txt` 会匹配仓库里每一个同名文件;
/// - 目录带尾斜杠:只匹配目录,不连同名文件一起吃掉;
/// - 转义 `\ * ? [ ]`:Windows 上 `a[1].psd` 完全合法,不转义会变成字符集。
///
/// 逐段走 [`std::path::Component`] 而不是整串 `replace('\\', "/")`:后者会
/// 把 Unix 文件名里的反斜杠也当分隔符。
#[cfg(test)]
pub(crate) fn gitignore_entry(root: &Path, path: &Path, is_dir: bool) -> Option<String> {
    use std::path::Component;

    let relative = path.strip_prefix(root).ok()?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => segments.push(escape_gitignore(&name.to_string_lossy())),
            // `..`、盘符、根目录都不该出现在仓库内的相对路径里。
            _ => return None,
        }
    }
    if segments.is_empty() {
        return None; // 仓库根自己不能忽略。
    }
    let mut entry = format!("/{}", segments.join("/"));
    if is_dir {
        entry.push('/');
    }
    Some(entry)
}

#[cfg(test)]
fn escape_gitignore(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for character in name.chars() {
        if matches!(character, '\\' | '*' | '?' | '[' | ']') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    // gitignore 丢弃行尾空白,除非转义;名字以空格结尾时不转义就等于写错行。
    if escaped.ends_with(' ') {
        escaped.pop();
        escaped.push_str("\\ ");
    }
    escaped
}

#[cfg(test)]
mod gitignore_tests {
    use super::*;

    /// 规则锚定、目录尾斜杠、glob 元字符/行尾空格转义、越界拒绝。
    #[test]
    fn gitignore_entries_are_anchored_typed_and_escaped() {
        let root = Path::new(r"D:\repo");
        assert_eq!(
            gitignore_entry(root, &root.join("src").join("main.rs"), false).as_deref(),
            Some("/src/main.rs")
        );
        assert_eq!(
            gitignore_entry(root, &root.join("target"), true).as_deref(),
            Some("/target/")
        );
        assert_eq!(
            gitignore_entry(root, &root.join("art").join("a[1].psd"), false).as_deref(),
            Some("/art/a\\[1\\].psd")
        );
        assert_eq!(
            gitignore_entry(root, &root.join("weird*name?.txt"), false).as_deref(),
            Some("/weird\\*name\\?.txt")
        );
        assert_eq!(
            gitignore_entry(root, &root.join("trailing "), false).as_deref(),
            Some("/trailing\\ ")
        );
        assert!(gitignore_entry(root, root, true).is_none());
        assert!(gitignore_entry(root, Path::new(r"D:\elsewhere\x.txt"), false).is_none());
    }

    /// 仓库根上溯:目录/文件两种起点;worktree 的 `.git` 文件同样算仓库。
    #[test]
    fn repository_root_walks_up_and_accepts_a_gitdir_file() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let nested = repo.join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(
            git_repository_root(&nested).as_deref(),
            Some(repo.as_path())
        );

        let file = nested.join("main.rs");
        std::fs::write(&file, "fn main() {}").unwrap();
        assert_eq!(
            git_repository_root(&file).as_deref(),
            Some(repo.as_path()),
            "文件从父目录起步"
        );

        let linked = temp.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: ../repo/.git/worktrees/linked").unwrap();
        assert_eq!(
            git_repository_root(&linked).as_deref(),
            Some(linked.as_path())
        );

        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        assert!(git_repository_root(&outside).is_none());
    }
}
