//! 不可变文本输入的 Myers 行差分、侧降级与行对投影；不执行 I/O。

use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use similar::{Algorithm, DiffOp, DiffTag, capture_diff_slices_by_key_deadline};

use crate::{CommitId, TextStateId};

const DIFF_BUDGET: Duration = Duration::from_secs(2);
const PAGED_SIDE_BYTES: usize = 10_000_000;
const MAX_DIFF_SIDE_BYTES: usize = 50_000_000;
const BINARY_PROBE_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DiffWhitespaceMode {
    #[default]
    Exact,
    Trailing,
    All,
}

/// 一次计算的不可变输入；字节、文本状态、HEAD 对象与代次共同固定身份。
#[derive(Clone, Debug)]
pub struct DiffInput {
    head: Arc<[u8]>,
    worktree: Arc<[u8]>,
    whitespace: DiffWhitespaceMode,
    text_state_id: TextStateId,
    head_identity: CommitId,
    generation: u64,
}

impl DiffInput {
    pub fn new(
        head: Arc<[u8]>,
        worktree: Arc<[u8]>,
        whitespace: DiffWhitespaceMode,
        text_state_id: TextStateId,
        head_identity: CommitId,
        generation: u64,
    ) -> Self {
        Self {
            head,
            worktree,
            whitespace,
            text_state_id,
            head_identity,
            generation,
        }
    }

    pub const fn text_state_id(&self) -> TextStateId {
        self.text_state_id
    }

    pub const fn head_identity(&self) -> &CommitId {
        &self.head_identity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffSideContent {
    Text { text: Arc<str>, paged: bool },
    Binary,
    TooLarge,
    InvalidUtf8,
}

impl DiffSideContent {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            Self::Binary | Self::TooLarge | Self::InvalidUtf8 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffPairKind {
    Equal,
    Delete,
    Insert,
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub index: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffPair {
    pub kind: DiffPairKind,
    pub head: Option<DiffLine>,
    pub worktree: Option<DiffLine>,
}

#[derive(Clone, Debug)]
pub struct DiffSnapshot {
    pub head: DiffSideContent,
    pub worktree: DiffSideContent,
    /// 任一侧不可计算时为 None；空 Vec 表示两侧可计算且没有差异。
    pub pairs: Option<Vec<DiffPair>>,
    pub text_state_id: TextStateId,
    pub head_identity: CommitId,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffError {
    DeadlineExceeded,
}

impl fmt::Display for DiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("差异计算超过时间预算，近似结果已丢弃")
    }
}

impl std::error::Error for DiffError {}

pub fn compute_diff(input: &DiffInput) -> Result<DiffSnapshot, DiffError> {
    compute_diff_at(input, Instant::now() + DIFF_BUDGET)
}

#[cfg(test)]
pub fn compute_diff_with_deadline(
    input: &DiffInput,
    deadline: Instant,
) -> Result<DiffSnapshot, DiffError> {
    compute_diff_at(input, deadline)
}

fn compute_diff_at(input: &DiffInput, deadline: Instant) -> Result<DiffSnapshot, DiffError> {
    let head = analyze_side(&input.head);
    let worktree = analyze_side(&input.worktree);
    let (Some(head_text), Some(worktree_text)) = (head.text(), worktree.text()) else {
        return Ok(snapshot(input, head, worktree, None));
    };
    let head_lines = raw_lines(head_text);
    let worktree_lines = raw_lines(worktree_text);
    let whitespace = input.whitespace;
    let ops = capture_diff_slices_by_key_deadline(
        Algorithm::Myers,
        &head_lines,
        &worktree_lines,
        |line| line_key(line, whitespace),
        Some(deadline),
    );
    // similar 到期会返回近似差异；硬预算要求直接丢弃，不展示近似值。
    if Instant::now() >= deadline {
        return Err(DiffError::DeadlineExceeded);
    }
    let pairs = ops
        .into_iter()
        .flat_map(|op| pair_op(op, &head_lines, &worktree_lines))
        .collect();
    Ok(snapshot(input, head, worktree, Some(pairs)))
}

fn snapshot(
    input: &DiffInput,
    head: DiffSideContent,
    worktree: DiffSideContent,
    pairs: Option<Vec<DiffPair>>,
) -> DiffSnapshot {
    DiffSnapshot {
        head,
        worktree,
        pairs,
        text_state_id: input.text_state_id,
        head_identity: input.head_identity.clone(),
        generation: input.generation,
    }
}

fn analyze_side(bytes: &Arc<[u8]>) -> DiffSideContent {
    if bytes.len() > MAX_DIFF_SIDE_BYTES {
        return DiffSideContent::TooLarge;
    }
    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        return DiffSideContent::Binary;
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => DiffSideContent::Text {
            text: Arc::from(text),
            paged: bytes.len() > PAGED_SIDE_BYTES,
        },
        Err(_) => DiffSideContent::InvalidUtf8,
    }
}

fn raw_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// 行键统一忽略 CRLF／LF 与末尾换行；Trailing 只忽略行尾空格和 Tab。
fn line_key(line: &str, whitespace: DiffWhitespaceMode) -> String {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    match whitespace {
        DiffWhitespaceMode::Exact => line.to_owned(),
        DiffWhitespaceMode::Trailing => line.trim_end_matches([' ', '\t']).to_owned(),
        DiffWhitespaceMode::All => line
            .chars()
            .filter(|character| !matches!(character, ' ' | '\t'))
            .collect(),
    }
}

fn pair_op(op: DiffOp, head: &[&str], worktree: &[&str]) -> Vec<DiffPair> {
    let old = op.old_range();
    let new = op.new_range();
    let mut pairs = Vec::new();
    match op.tag() {
        DiffTag::Equal => {
            for (old, new) in old.zip(new) {
                pairs.push(pair(
                    DiffPairKind::Equal,
                    head,
                    worktree,
                    Some(old),
                    Some(new),
                ));
            }
        }
        DiffTag::Delete => {
            for old in old {
                pairs.push(pair(DiffPairKind::Delete, head, worktree, Some(old), None));
            }
        }
        DiffTag::Insert => {
            for new in new {
                pairs.push(pair(DiffPairKind::Insert, head, worktree, None, Some(new)));
            }
        }
        DiffTag::Replace => {
            let shared = old.len().min(new.len());
            for offset in 0..shared {
                pairs.push(pair(
                    DiffPairKind::Replace,
                    head,
                    worktree,
                    Some(old.start + offset),
                    Some(new.start + offset),
                ));
            }
            for old in old.start + shared..old.end {
                pairs.push(pair(DiffPairKind::Delete, head, worktree, Some(old), None));
            }
            for new in new.start + shared..new.end {
                pairs.push(pair(DiffPairKind::Insert, head, worktree, None, Some(new)));
            }
        }
    }
    pairs
}

fn pair(
    kind: DiffPairKind,
    head: &[&str],
    worktree: &[&str],
    old: Option<usize>,
    new: Option<usize>,
) -> DiffPair {
    DiffPair {
        kind,
        head: old.map(|index| DiffLine {
            index,
            text: head[index].to_owned(),
        }),
        worktree: new.map(|index| DiffLine {
            index,
            text: worktree[index].to_owned(),
        }),
    }
}
