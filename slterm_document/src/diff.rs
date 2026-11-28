//! 不可变文本输入的 Myers 行差分、侧降级与行对投影；不执行 I/O。

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use similar::{
    Algorithm,
    algorithms::{CachedLookup, DiffHook, Replace, diff_deadline},
};

use crate::{BINARY_PROBE_BYTES, CommitId, TextStateId};

const DIFF_BUDGET: Duration = Duration::from_secs(2);
const PAGED_SIDE_BYTES: usize = 10_000_000;
const MAX_DIFF_SIDE_BYTES: usize = 50_000_000;
const MAX_DIFF_SIDE_LINES: usize = 200_000;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DiffWhitespaceMode {
    #[default]
    Exact,
    Trailing,
    All,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffBaseline {
    Commit(CommitId),
    Conflict,
}

/// 一次计算的不可变输入；字节、文本状态、基线身份与代次共同固定身份。
#[derive(Clone, Debug)]
pub struct DiffInput {
    head: Arc<[u8]>,
    worktree: Arc<[u8]>,
    whitespace: DiffWhitespaceMode,
    text_state_id: TextStateId,
    baseline: DiffBaseline,
    generation: u64,
}

impl DiffInput {
    pub fn git(
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
            baseline: DiffBaseline::Commit(head_identity),
            generation,
        }
    }

    pub fn conflict(
        disk: Arc<[u8]>,
        buffer: Arc<[u8]>,
        whitespace: DiffWhitespaceMode,
        text_state_id: TextStateId,
        generation: u64,
    ) -> Self {
        Self {
            head: disk,
            worktree: buffer,
            whitespace,
            text_state_id,
            baseline: DiffBaseline::Conflict,
            generation,
        }
    }

    pub const fn text_state_id(&self) -> TextStateId {
        self.text_state_id
    }

    pub const fn baseline(&self) -> &DiffBaseline {
        &self.baseline
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffSideContent {
    Text { text: Arc<str> },
    Binary,
    TooLarge,
    TooManyLines,
    InvalidUtf8,
}

impl DiffSideContent {
    pub fn text(&self) -> Option<&Arc<str>> {
        match self {
            Self::Text { text } => Some(text),
            Self::Binary | Self::TooLarge | Self::TooManyLines | Self::InvalidUtf8 => None,
        }
    }

    pub fn is_paged(&self) -> bool {
        self.text()
            .is_some_and(|text| text.len() > PAGED_SIDE_BYTES)
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
    head: DiffSideContent,
    worktree: DiffSideContent,
    pairs: Vec<DiffPair>,
    text_state_id: TextStateId,
    baseline: DiffBaseline,
    generation: u64,
}

impl DiffSnapshot {
    pub const fn head(&self) -> &DiffSideContent {
        &self.head
    }

    pub const fn worktree(&self) -> &DiffSideContent {
        &self.worktree
    }

    /// 任一侧不可计算时为 None；空切片表示两侧可计算且没有差异。
    pub fn pairs(&self) -> Option<&[DiffPair]> {
        (self.head.text().is_some() && self.worktree.text().is_some()).then_some(&self.pairs)
    }

    pub const fn text_state_id(&self) -> TextStateId {
        self.text_state_id
    }

    pub const fn baseline(&self) -> &DiffBaseline {
        &self.baseline
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffError {
    DeadlineExceeded,
    /// 新一代输入已到达；旧计算尽早停止，不再消耗剩余预算。
    Cancelled,
}

impl fmt::Display for DiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DeadlineExceeded => "差异计算超过时间预算，近似结果已丢弃",
            Self::Cancelled => "差异计算已被新输入取消",
        })
    }
}

impl std::error::Error for DiffError {}

pub fn compute_diff(input: &DiffInput) -> Result<DiffSnapshot, DiffError> {
    compute_diff_with_cancel(input, None)
}

/// 可取消入口；取消标志在各阶段与行对生成边界轮询。
pub fn compute_diff_cancellable(
    input: &DiffInput,
    cancelled: &AtomicBool,
) -> Result<DiffSnapshot, DiffError> {
    compute_diff_with_cancel(input, Some(cancelled))
}

#[cfg(test)]
pub fn compute_diff_with_deadline(
    input: &DiffInput,
    deadline: Instant,
) -> Result<DiffSnapshot, DiffError> {
    compute_diff_at(input, deadline, None)
}

fn compute_diff_with_cancel(
    input: &DiffInput,
    cancel: Option<&AtomicBool>,
) -> Result<DiffSnapshot, DiffError> {
    compute_diff_at(input, Instant::now() + DIFF_BUDGET, cancel)
}

fn compute_diff_at(
    input: &DiffInput,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<DiffSnapshot, DiffError> {
    ensure_active(deadline, cancel)?;
    let mut head = analyze_side(&input.head, deadline, cancel)?;
    let mut worktree = analyze_side(&input.worktree, deadline, cancel)?;
    let head_text = head.text().cloned();
    let worktree_text = worktree.text().cloned();
    let head_lines = match &head_text {
        Some(text) => raw_lines(text, deadline, cancel)?,
        None => None,
    };
    if head_text.is_some() && head_lines.is_none() {
        head = DiffSideContent::TooManyLines;
    }
    let worktree_lines = match &worktree_text {
        Some(text) => raw_lines(text, deadline, cancel)?,
        None => None,
    };
    if worktree_text.is_some() && worktree_lines.is_none() {
        worktree = DiffSideContent::TooManyLines;
    }
    let (Some(head_lines), Some(worktree_lines)) = (head_lines, worktree_lines) else {
        ensure_active(deadline, cancel)?;
        return Ok(snapshot(input, head, worktree, Vec::new()));
    };
    let whitespace = input.whitespace;
    let head_keys = CachedLookup::new(head_lines.len(), |index| {
        line_key(head_lines[index], whitespace)
    });
    let worktree_keys = CachedLookup::new(worktree_lines.len(), |index| {
        line_key(worktree_lines[index], whitespace)
    });
    let mut hook = Replace::new(PairBuilder::new(
        &head_lines,
        &worktree_lines,
        deadline,
        cancel,
    ));
    diff_deadline(
        Algorithm::Myers,
        &mut hook,
        &head_keys,
        0..head_keys.len(),
        &worktree_keys,
        0..worktree_keys.len(),
        Some(deadline),
    )?;
    // similar 到期会返回近似差异；硬预算要求直接丢弃，不展示近似值。
    ensure_active(deadline, cancel)?;
    Ok(snapshot(input, head, worktree, hook.into_inner().pairs))
}

fn ensure_active(deadline: Instant, cancel: Option<&AtomicBool>) -> Result<(), DiffError> {
    if cancel.is_some_and(|cancel| cancel.load(Ordering::Relaxed)) {
        Err(DiffError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(DiffError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

fn snapshot(
    input: &DiffInput,
    head: DiffSideContent,
    worktree: DiffSideContent,
    pairs: Vec<DiffPair>,
) -> DiffSnapshot {
    DiffSnapshot {
        head,
        worktree,
        pairs,
        text_state_id: input.text_state_id,
        baseline: input.baseline.clone(),
        generation: input.generation,
    }
}

pub(crate) fn analyze_side(
    bytes: &[u8],
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<DiffSideContent, DiffError> {
    ensure_active(deadline, cancel)?;
    if bytes.len() > MAX_DIFF_SIDE_BYTES {
        return Ok(DiffSideContent::TooLarge);
    }
    if bytes[..bytes.len().min(BINARY_PROBE_BYTES)].contains(&0) {
        ensure_active(deadline, cancel)?;
        return Ok(DiffSideContent::Binary);
    }
    let content = match std::str::from_utf8(bytes) {
        Ok(text) => DiffSideContent::Text {
            text: Arc::from(text),
        },
        Err(_) => DiffSideContent::InvalidUtf8,
    };
    ensure_active(deadline, cancel)?;
    Ok(content)
}

fn raw_lines<'a>(
    text: &'a str,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<Option<Vec<&'a str>>, DiffError> {
    let mut lines = Vec::new();
    for line in text.split_inclusive('\n') {
        if lines.len() == MAX_DIFF_SIDE_LINES {
            return Ok(None);
        }
        if lines.len().is_multiple_of(4096) {
            ensure_active(deadline, cancel)?;
        }
        lines.push(line);
    }
    ensure_active(deadline, cancel)?;
    Ok(Some(lines))
}

/// 行键统一忽略 CRLF／LF 与末尾换行；Trailing 只忽略行尾空格和 Tab。
fn line_key(line: &str, whitespace: DiffWhitespaceMode) -> String {
    let line = match line.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => line,
    };
    match whitespace {
        DiffWhitespaceMode::Exact => line.to_owned(),
        DiffWhitespaceMode::Trailing => line.trim_end_matches([' ', '\t']).to_owned(),
        DiffWhitespaceMode::All => line
            .chars()
            .filter(|character| !matches!(character, ' ' | '\t'))
            .collect(),
    }
}

pub(crate) struct PairBuilder<'a> {
    head: &'a [&'a str],
    worktree: &'a [&'a str],
    deadline: Instant,
    cancel: Option<&'a AtomicBool>,
    pairs: Vec<DiffPair>,
}

impl<'a> PairBuilder<'a> {
    pub(crate) fn new(
        head: &'a [&'a str],
        worktree: &'a [&'a str],
        deadline: Instant,
        cancel: Option<&'a AtomicBool>,
    ) -> Self {
        Self {
            head,
            worktree,
            deadline,
            cancel,
            pairs: Vec::new(),
        }
    }

    fn push(&mut self, pair: DiffPair) -> Result<(), DiffError> {
        if self.pairs.len().is_multiple_of(4096) {
            ensure_active(self.deadline, self.cancel)?;
        }
        self.pairs.push(pair);
        Ok(())
    }

    fn append(
        &mut self,
        kind: DiffPairKind,
        old: std::ops::Range<usize>,
        new: std::ops::Range<usize>,
    ) -> Result<(), DiffError> {
        match kind {
            DiffPairKind::Equal => {
                for (old, new) in old.zip(new) {
                    self.push(pair(
                        DiffPairKind::Equal,
                        self.head,
                        self.worktree,
                        Some(old),
                        Some(new),
                    ))?;
                }
            }
            DiffPairKind::Delete => {
                for old in old {
                    self.push(pair(
                        DiffPairKind::Delete,
                        self.head,
                        self.worktree,
                        Some(old),
                        None,
                    ))?;
                }
            }
            DiffPairKind::Insert => {
                for new in new {
                    self.push(pair(
                        DiffPairKind::Insert,
                        self.head,
                        self.worktree,
                        None,
                        Some(new),
                    ))?;
                }
            }
            DiffPairKind::Replace => {
                let shared = old.len().min(new.len());
                for offset in 0..shared {
                    self.push(pair(
                        DiffPairKind::Replace,
                        self.head,
                        self.worktree,
                        Some(old.start + offset),
                        Some(new.start + offset),
                    ))?;
                }
                self.append(
                    DiffPairKind::Delete,
                    old.start + shared..old.end,
                    new.start..new.start,
                )?;
                self.append(
                    DiffPairKind::Insert,
                    old.end..old.end,
                    new.start + shared..new.end,
                )?;
            }
        }
        Ok(())
    }
}

impl DiffHook for PairBuilder<'_> {
    type Error = DiffError;

    fn equal(&mut self, old: usize, new: usize, len: usize) -> Result<(), Self::Error> {
        self.append(DiffPairKind::Equal, old..old + len, new..new + len)
    }

    fn delete(&mut self, old: usize, old_len: usize, new: usize) -> Result<(), Self::Error> {
        self.append(DiffPairKind::Delete, old..old + old_len, new..new)
    }

    fn insert(&mut self, old: usize, new: usize, new_len: usize) -> Result<(), Self::Error> {
        self.append(DiffPairKind::Insert, old..old, new..new + new_len)
    }

    fn replace(
        &mut self,
        old: usize,
        old_len: usize,
        new: usize,
        new_len: usize,
    ) -> Result<(), Self::Error> {
        self.append(
            DiffPairKind::Replace,
            old..old + old_len,
            new..new + new_len,
        )
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        ensure_active(self.deadline, self.cancel)
    }
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
