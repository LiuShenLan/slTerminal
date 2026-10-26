use std::fmt;
use std::str::FromStr;

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

#[cfg(test)]
mod repository_tests {
    use super::*;

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
