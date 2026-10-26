use std::fmt;
use std::str::FromStr;

use slterm_path::ResolvedPath;

/// Git 提交的完整对象身份，不用短哈希或分支名代替。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitId(String);

/// 提交 ID 不是完整规范小写对象哈希。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidCommitId;

impl CommitId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for CommitId {
    type Err = InvalidCommitId;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !matches!(text.len(), 40 | 64)
            || !text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(InvalidCommitId);
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for InvalidCommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("提交 ID 必须为 40 或 64 个小写 ASCII 十六进制字符")
    }
}

impl std::error::Error for InvalidCommitId {}

/// Git 输出与命令参数共用的规范仓库相对路径。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepoPath(String);

/// 仓库路径不是规范的相对文件路径。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidRepoPath;

impl RepoPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RepoPath {
    type Err = InvalidRepoPath;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bytes = text.as_bytes();
        let drive_prefixed = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if text.is_empty()
            || text.starts_with('/')
            || text.ends_with('/')
            || bytes.iter().any(|byte| matches!(byte, b'\\' | 0))
            || drive_prefixed
            || text
                .split('/')
                .any(|segment| matches!(segment, "" | "." | ".."))
        {
            return Err(InvalidRepoPath);
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for InvalidRepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("仓库路径必须为非空规范相对路径，并使用正斜杠分隔")
    }
}

impl std::error::Error for InvalidRepoPath {}

/// 创建固定版本视图时捕获的仓库路径、目录身份与 HEAD。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryBinding {
    worktree_root: ResolvedPath,
    git_dir: ResolvedPath,
    common_dir: ResolvedPath,
    head: CommitId,
}

/// 当前路径或身份不能证明仍属于创建时仓库。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepositoryBindingUnavailable;

impl RepositoryBinding {
    pub fn new(
        worktree_root: ResolvedPath,
        git_dir: ResolvedPath,
        common_dir: ResolvedPath,
        head: CommitId,
    ) -> Self {
        Self {
            worktree_root,
            git_dir,
            common_dir,
            head,
        }
    }

    pub const fn worktree_root(&self) -> &ResolvedPath {
        &self.worktree_root
    }

    pub const fn git_dir(&self) -> &ResolvedPath {
        &self.git_dir
    }

    pub const fn common_dir(&self) -> &ResolvedPath {
        &self.common_dir
    }

    pub const fn head(&self) -> &CommitId {
        &self.head
    }

    /// 复核当前仓库事实；HEAD 移动本身不改变绑定路径与身份。
    /// `original_head_exists` 必须查询 `self.head`，不得以 `current.head` 代替。
    pub fn revalidate(
        &self,
        current: &Self,
        original_head_exists: bool,
    ) -> Result<(), RepositoryBindingUnavailable> {
        if self.worktree_root.canonical_path() != current.worktree_root.canonical_path()
            || self.git_dir.canonical_path() != current.git_dir.canonical_path()
            || self.common_dir.canonical_path() != current.common_dir.canonical_path()
        {
            return Err(RepositoryBindingUnavailable);
        }

        let mut identity_proven = false;
        for (original, current) in [
            (&self.worktree_root, &current.worktree_root),
            (&self.git_dir, &current.git_dir),
            (&self.common_dir, &current.common_dir),
        ] {
            if let (Some(original), Some(current)) =
                (original.file_identity(), current.file_identity())
            {
                if original != current {
                    return Err(RepositoryBindingUnavailable);
                }
                identity_proven = true;
            }
        }

        if identity_proven || original_head_exists {
            Ok(())
        } else {
            Err(RepositoryBindingUnavailable)
        }
    }
}

impl fmt::Display for RepositoryBindingUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("仓库绑定不可用，无法证明当前仓库与创建时相同")
    }
}

impl std::error::Error for RepositoryBindingUnavailable {}

/// Diff 的固定仓库与 HEAD 旧路径，不包含任何展示参数。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffBinding {
    repository_binding: RepositoryBinding,
    head_path: RepoPath,
}

impl DiffBinding {
    pub fn new(repository_binding: RepositoryBinding, head_path: RepoPath) -> Self {
        Self {
            repository_binding,
            head_path,
        }
    }

    pub const fn repository_binding(&self) -> &RepositoryBinding {
        &self.repository_binding
    }

    pub const fn head_path(&self) -> &RepoPath {
        &self.head_path
    }

    /// 仅在当前事实仍证明同一仓库时刷新 HEAD，失败保留原绑定。
    pub fn refresh_head(
        &mut self,
        current: &RepositoryBinding,
        original_head_exists: bool,
    ) -> Result<(), RepositoryBindingUnavailable> {
        self.repository_binding
            .revalidate(current, original_head_exists)?;
        self.repository_binding.head = current.head.clone();
        Ok(())
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;
    use slterm_path::{CanonicalPath, FileIdentity};

    fn resolved(path: &str, identity: Option<u128>) -> ResolvedPath {
        ResolvedPath::new(
            CanonicalPath::new(path).unwrap(),
            identity.map(|file| FileIdentity { volume: 7, file }),
        )
    }

    fn commit(nibble: char) -> CommitId {
        CommitId::from_str(&nibble.to_string().repeat(40)).unwrap()
    }

    fn repository(identities: [Option<u128>; 3], head: CommitId) -> RepositoryBinding {
        let [worktree, git, common] = identities;
        RepositoryBinding::new(
            resolved("D:/repo", worktree),
            resolved("D:/git/repo.git", git),
            resolved("D:/git/common.git", common),
            head,
        )
    }

    /// 固定版本使用完整 SHA-1 或 SHA-256，序列化保持原值。
    #[test]
    fn commit_id_round_trips_full_object_ids() {
        for text in ["a".repeat(40), "0123456789abcdef".repeat(4)] {
            let id = CommitId::from_str(&text).unwrap();
            assert_eq!(id.as_str(), text);
            assert_eq!(id.to_string(), text);
            assert_eq!(CommitId::from_str(&id.to_string()).unwrap(), id);
        }
    }

    /// 短哈希、大写、空白、引用名和非十六进制输入一律拒绝。
    #[test]
    fn commit_id_rejects_noncanonical_or_incomplete_ids() {
        for text in [
            "".into(),
            "a".repeat(39),
            "a".repeat(41),
            "A".repeat(40),
            "g".repeat(40),
            format!(" {}", "a".repeat(40)),
            "refs/heads/main".into(),
            "0123456789abcdef".repeat(4) + "0",
        ] {
            assert_eq!(CommitId::from_str(&text), Err(InvalidCommitId), "{text:?}");
        }
    }

    /// 仓库路径保留 Unicode、空格、点号文件与正斜杠层级。
    #[test]
    fn repo_path_round_trips_canonical_relative_paths() {
        for text in [
            "README.md",
            ".gitignore",
            "dir/file name.md",
            "中文/文档.md",
            "a/.b/c",
        ] {
            let path = RepoPath::from_str(text).unwrap();
            assert_eq!(path.as_str(), text);
            assert_eq!(path.to_string(), text);
            assert_eq!(RepoPath::from_str(&path.to_string()).unwrap(), path);
        }
    }

    /// 绝对路径、反斜杠、空段与父目录跳转不得进入固定版本键。
    #[test]
    fn repo_path_rejects_noncanonical_or_escaping_paths() {
        for text in [
            "",
            "/root/file",
            "dir/",
            "dir//file",
            "./file",
            "dir/./file",
            "../file",
            "dir/../file",
            r"dir\file",
            "C:/file",
            "bad\0path",
        ] {
            assert_eq!(RepoPath::from_str(text), Err(InvalidRepoPath), "{text:?}");
        }
    }

    /// 绑定保留三个规范目录、可取得身份与创建时 HEAD；Diff 另存旧路径。
    #[test]
    fn repository_and_diff_bindings_keep_their_domain_facts() {
        let binding = repository([Some(1), Some(2), Some(3)], commit('a'));
        assert_eq!(
            binding.worktree_root().canonical_path().as_path(),
            std::path::Path::new("D:/repo")
        );
        assert_eq!(
            binding.git_dir().canonical_path().as_path(),
            std::path::Path::new("D:/git/repo.git")
        );
        assert_eq!(
            binding.common_dir().canonical_path().as_path(),
            std::path::Path::new("D:/git/common.git")
        );
        assert_eq!(
            binding.worktree_root().file_identity(),
            Some(FileIdentity { volume: 7, file: 1 })
        );
        assert_eq!(binding.git_dir().file_identity().unwrap().file, 2);
        assert_eq!(binding.common_dir().file_identity().unwrap().file, 3);
        assert_eq!(binding.head(), &commit('a'));

        let head_path = RepoPath::from_str("dir/file.md").unwrap();
        let diff = DiffBinding::new(binding.clone(), head_path.clone());
        assert_eq!(diff.repository_binding(), &binding);
        assert_eq!(diff.head_path(), &head_path);
    }

    /// 复核要求路径不变；HEAD 可移动，但任一可比身份失配即拒绝。
    #[test]
    fn repository_revalidation_checks_paths_and_available_identities() {
        let original = repository([Some(1), Some(2), Some(3)], commit('a'));
        let moved_head = repository([Some(1), Some(2), Some(3)], commit('b'));
        assert_eq!(original.revalidate(&moved_head, false), Ok(()));

        let relocated = RepositoryBinding::new(
            resolved("D:/elsewhere", Some(1)),
            resolved("D:/git/repo.git", Some(2)),
            resolved("D:/git/common.git", Some(3)),
            commit('b'),
        );
        assert_eq!(
            original.revalidate(&relocated, true),
            Err(RepositoryBindingUnavailable)
        );

        let replaced = repository([Some(1), Some(2), Some(99)], commit('b'));
        assert_eq!(
            original.revalidate(&replaced, true),
            Err(RepositoryBindingUnavailable)
        );
    }

    /// 没有可比目录身份时才以原 HEAD 仍存在作为保守证据。
    #[test]
    fn repository_revalidation_uses_original_head_only_as_fallback() {
        let unidentified = repository([None, None, None], commit('a'));
        let current = repository([None, None, None], commit('b'));
        assert_eq!(unidentified.revalidate(&current, true), Ok(()));
        assert_eq!(
            unidentified.revalidate(&current, false),
            Err(RepositoryBindingUnavailable)
        );

        let lost_identity = repository([Some(1), None, None], commit('a'));
        assert_eq!(
            lost_identity.revalidate(&current, false),
            Err(RepositoryBindingUnavailable)
        );
        assert_eq!(lost_identity.revalidate(&current, true), Ok(()));

        let one_matching = repository([Some(1), None, None], commit('b'));
        assert_eq!(lost_identity.revalidate(&one_matching, false), Ok(()));

        let one_conflicting = repository([Some(1), Some(9), None], commit('b'));
        let with_conflict = repository([Some(1), Some(2), None], commit('a'));
        assert_eq!(
            with_conflict.revalidate(&one_conflicting, false),
            Err(RepositoryBindingUnavailable)
        );
    }

    /// Diff 复核成功后只更新 HEAD；失败不改变已持久绑定。
    #[test]
    fn diff_refresh_updates_head_only_after_revalidation() {
        let mut diff = DiffBinding::new(
            repository([Some(1), Some(2), Some(3)], commit('a')),
            RepoPath::from_str("dir/file.md").unwrap(),
        );
        let moved = repository([Some(1), Some(2), Some(3)], commit('b'));
        assert_eq!(diff.refresh_head(&moved, false), Ok(()));
        assert_eq!(diff.repository_binding().head(), &commit('b'));
        assert_eq!(diff.head_path().as_str(), "dir/file.md");

        let replaced = repository([Some(1), Some(2), Some(4)], commit('c'));
        assert_eq!(
            diff.refresh_head(&replaced, true),
            Err(RepositoryBindingUnavailable)
        );
        assert_eq!(diff.repository_binding().head(), &commit('b'));
    }

    /// 绑定不可用错误实现标准接口并提供稳定说明。
    #[test]
    fn repository_binding_unavailable_describes_the_failure() {
        fn require_error<T: std::error::Error>() {}
        require_error::<RepositoryBindingUnavailable>();
        assert_eq!(
            RepositoryBindingUnavailable.to_string(),
            "仓库绑定不可用，无法证明当前仓库与创建时相同"
        );
    }

    /// 解析错误说明规范形状，不回显不可信输入。
    #[test]
    fn repository_value_errors_describe_the_contract() {
        fn require_error<T: std::error::Error>() {}
        require_error::<InvalidCommitId>();
        require_error::<InvalidRepoPath>();
        assert_eq!(
            InvalidCommitId.to_string(),
            "提交 ID 必须为 40 或 64 个小写 ASCII 十六进制字符"
        );
        assert_eq!(
            InvalidRepoPath.to_string(),
            "仓库路径必须为非空规范相对路径，并使用正斜杠分隔"
        );
    }
}
