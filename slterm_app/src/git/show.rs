//! 固定仓库、提交与路径的 GitShow 对象读取；不回退工作区或后续 HEAD。

use std::{str::FromStr as _, sync::Arc};

use sha2::{Digest as _, Sha256};
use slterm_document::{
    BINARY_PROBE_BYTES, CommitId, PAGE_SOURCE_BYTES, PageIndex, PagedLineIndex, PagedSource,
    RepoPath, RepositoryBinding, SourceByteRange,
};
use slterm_path::PathGrant;

use super::{
    GitError, GitFacade,
    repository::{repository_command, run_repository, run_repository_with_limit},
};

const SIZE_NOTICE_BYTES: usize = 1_000_000;
const PAGED_CONTENT_BYTES: usize = 10_000_000;
const SHOW_OUTPUT_LIMIT: usize = 200_000_000 + 64 * 1024;

#[derive(Clone, Debug)]
pub(crate) enum GitShowContent {
    Text { text: Arc<str> },
    Paged(PagedSource),
    Binary,
    InvalidUtf8,
}

#[derive(Clone, Debug)]
pub(crate) struct GitShowSnapshot {
    pub(crate) commit: CommitId,
    pub(crate) path: RepoPath,
    pub(crate) blob: CommitId,
    pub(crate) bytes: Arc<[u8]>,
    content: GitShowContent,
    pub(crate) generation: u64,
}

impl GitShowSnapshot {
    pub(crate) const fn content(&self) -> &GitShowContent {
        &self.content
    }

    pub(crate) fn requires_size_notice(&self) -> bool {
        self.bytes.len() > SIZE_NOTICE_BYTES && self.bytes.len() <= PAGED_CONTENT_BYTES
    }

    pub(crate) fn page_text(&self, page: usize) -> Option<String> {
        let GitShowContent::Paged(source) = &self.content else {
            return None;
        };
        let range = source.index().page(page)?;
        decode_page(&self.bytes, range)
    }
}

impl GitFacade {
    pub(crate) fn show(
        &self,
        binding: &RepositoryBinding,
        path: &RepoPath,
        grants: &[PathGrant],
        authorization_generation: u64,
        generation: u64,
    ) -> Result<GitShowSnapshot, GitError> {
        self.revalidate_binding(binding, grants, authorization_generation)?;
        let mut ls_tree = repository_command(binding.worktree_root().canonical_path().as_path());
        ls_tree.args([
            "ls-tree",
            "-z",
            binding.head().as_str(),
            "--",
            path.as_str(),
        ]);
        let output = run_repository(ls_tree)?;
        if !output.status.success() {
            return Err(object_failure(&output.stderr));
        }
        let output = String::from_utf8(output.stdout)
            .map_err(|_| GitError::Command("ls-tree 输出不是有效 UTF-8".to_owned()))?;
        let blob = parse_blob(&output, path)?;
        let mut cat_file = repository_command(binding.worktree_root().canonical_path().as_path());
        cat_file.args(["cat-file", "blob", blob.as_str()]);
        let content = run_repository_with_limit(cat_file, SHOW_OUTPUT_LIMIT)?;
        if !content.status.success() {
            return Err(object_failure(&content.stderr));
        }
        let bytes = Arc::from(content.stdout);
        let content = classify_content(&bytes)?;
        Ok(GitShowSnapshot {
            commit: binding.head().clone(),
            path: path.clone(),
            blob,
            bytes,
            content,
            generation,
        })
    }
}

fn classify_content(bytes: &Arc<[u8]>) -> Result<GitShowContent, GitError> {
    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        return Ok(GitShowContent::Binary);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| GitShowContent::InvalidUtf8);
    let text = match text {
        Ok(text) => text,
        Err(content) => return Ok(content),
    };
    if bytes.len() <= PAGED_CONTENT_BYTES {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        return Ok(GitShowContent::Text {
            text: Arc::from(text),
        });
    }
    build_paged_source(bytes).map(GitShowContent::Paged)
}

fn build_paged_source(bytes: &[u8]) -> Result<PagedSource, GitError> {
    let mut ranges = Vec::new();
    let mut line_stats = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let mut end = (start + PAGE_SOURCE_BYTES as usize).min(bytes.len());
        if end < bytes.len()
            && let Err(error) = std::str::from_utf8(&bytes[start..end])
        {
            if error.error_len().is_some() {
                return Err(GitError::Command("固定版本分页包含非法 UTF-8".to_owned()));
            }
            end = start + error.valid_up_to();
        }
        if end < bytes.len() {
            if let Some(newline) = bytes[start..end].iter().rposition(|byte| *byte == b'\n') {
                end = start + newline + 1;
            }
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
        }
        let range = SourceByteRange::new(start as u64, end as u64)
            .map_err(|_| GitError::Command("固定版本分页边界无效".to_owned()))?;
        let decoded = decode_page(bytes, range)
            .ok_or_else(|| GitError::Command("固定版本分页解码失败".to_owned()))?;
        line_stats.push((
            decoded.bytes().filter(|byte| *byte == b'\n').count(),
            decoded.len(),
            decoded.ends_with('\n'),
        ));
        ranges.push(range);
        start = end;
    }
    let index = PageIndex::new(bytes.len() as u64, ranges)
        .map_err(|_| GitError::Command("固定版本分页索引无效".to_owned()))?;
    let line_index = PagedLineIndex::from_page_stats(&line_stats);
    Ok(PagedSource::new(
        index,
        Sha256::digest(bytes).into(),
        line_index,
    ))
}

fn decode_page(bytes: &[u8], range: SourceByteRange) -> Option<String> {
    let text = std::str::from_utf8(&bytes[range.start() as usize..range.end() as usize]).ok()?;
    let text = if range.start() == 0 {
        text.strip_prefix('\u{feff}').unwrap_or(text)
    } else {
        text
    };
    Some(text.replace("\r\n", "\n"))
}

fn parse_blob(output: &str, path: &RepoPath) -> Result<CommitId, GitError> {
    let mut records = output.split_terminator('\0');
    let record = records.next().ok_or(GitError::ObjectUnavailable)?;
    let (metadata, actual_path) = record.split_once('\t').ok_or(GitError::ObjectUnavailable)?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if records.next().is_some()
        || actual_path != path.as_str()
        || fields.len() != 3
        || fields[1] != "blob"
    {
        return Err(GitError::ObjectUnavailable);
    }
    CommitId::from_str(fields[2]).map_err(|_| GitError::ObjectUnavailable)
}

fn object_failure(stderr: &[u8]) -> GitError {
    let stderr = String::from_utf8_lossy(stderr);
    super::classify_git_failure(&stderr).unwrap_or(GitError::ObjectUnavailable)
}
