//! 增量语法解析与有界后台任务。

use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ropey::{LineType, Rope};
use tree_sitter::{InputEdit, LanguageError, ParseOptions, Parser, Point, Tree};

use crate::{DocumentId, LanguageId, LanguageRegistry};

const COMMAND_CAPACITY: usize = 8;
// 初始任务与全部在途命令各至多产生一个结果，因此结果永不需无界排队。
const RESULT_CAPACITY: usize = COMMAND_CAPACITY + 1;

#[derive(Debug)]
pub enum SyntaxWorkerError {
    MissingGrammar,
    Language(LanguageError),
    StaleRevision,
    QueueFull,
    Closed,
    ThreadPanicked,
}

impl std::fmt::Display for SyntaxWorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MissingGrammar => "纯文本语言没有语法运行时",
            Self::Language(_) => "语法运行时与 grammar 不兼容",
            Self::StaleRevision => "语法编辑必须推进文本 revision",
            Self::QueueFull => "语法任务队列已满",
            Self::Closed => "语法工作线程已关闭",
            Self::ThreadPanicked => "语法工作线程异常终止",
        })
    }
}

impl std::error::Error for SyntaxWorkerError {}

pub struct SyntaxTree {
    document_id: DocumentId,
    revision: u64,
    generation: u64,
    tree: Arc<Tree>,
}

struct SyntaxPublication {
    document_id: DocumentId,
    revision: u64,
    generation: u64,
    // 队列不持有树；隐藏清除 worker 强引用后，迟到发布立即失效。
    tree: Weak<Tree>,
}

impl SyntaxTree {
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }
}

enum Command {
    Edit {
        generation: u64,
        revision: u64,
        text: Rope,
        edits: Vec<InputEdit>,
        rebuild: bool,
    },
    SetVisible {
        generation: u64,
        visible: bool,
    },
    Close,
}

#[cfg_attr(not(test), allow(dead_code))]
enum WorkerObservation {
    ParseStart { revision: u64, has_old_tree: bool },
    Progress,
    Publish { revision: u64 },
    Hidden,
}

/// 队列拒收后只保留最新全量重建副本。
struct PendingEdit {
    text: Rope,
    revision: u64,
}

/// 单个 Document 的语法树所有者；前台只经有界队列提交与接收。
pub struct SyntaxWorker {
    document_id: DocumentId,
    commands: SyncSender<Command>,
    results: Receiver<SyntaxPublication>,
    generation: Arc<AtomicU64>,
    latest_revision: u64,
    latest_generation: u64,
    pending: Option<PendingEdit>,
    visible: bool,
    worker: Option<JoinHandle<()>>,
}

impl SyntaxWorker {
    pub fn new(
        document_id: DocumentId,
        language: LanguageId,
        text: Rope,
        revision: u64,
    ) -> Result<Self, SyntaxWorkerError> {
        Self::with_parse_observer(document_id, language, text, revision, |_| {})
    }

    fn with_parse_observer<F>(
        document_id: DocumentId,
        language: LanguageId,
        text: Rope,
        revision: u64,
        observer: F,
    ) -> Result<Self, SyntaxWorkerError>
    where
        F: FnMut(WorkerObservation) + Send + 'static,
    {
        let grammar = LanguageRegistry::new()
            .descriptor(language)
            .grammar()
            .ok_or(SyntaxWorkerError::MissingGrammar)?;
        let mut parser = Parser::new();
        parser
            .set_language(&grammar)
            .map_err(SyntaxWorkerError::Language)?;
        let (commands, command_rx) = sync_channel(COMMAND_CAPACITY);
        let (results, result_rx) = sync_channel(RESULT_CAPACITY);
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = Arc::clone(&generation);
        let worker = thread::Builder::new()
            .name(format!("syntax-{document_id}"))
            .spawn(move || {
                run_parser(
                    document_id,
                    parser,
                    text,
                    revision,
                    command_rx,
                    results,
                    worker_generation,
                    observer,
                );
            })
            .map_err(|_| SyntaxWorkerError::Closed)?;
        Ok(Self {
            document_id,
            commands,
            results: result_rx,
            generation,
            latest_revision: revision,
            latest_generation: 0,
            pending: None,
            visible: true,
            worker: Some(worker),
        })
    }

    /// edits 的顺序必须与 Document 实际应用顺序一致，并覆盖本次全部变化。
    pub fn edit(
        &mut self,
        text: Rope,
        revision: u64,
        edits: Vec<InputEdit>,
    ) -> Result<(), SyntaxWorkerError> {
        if revision <= self.latest_revision {
            return Err(SyntaxWorkerError::StaleRevision);
        }
        self.latest_revision = revision;
        let rebuild = self.pending.is_some();
        let pending_text = text.clone();
        let generation = self.generation.fetch_add(1, Ordering::Release) + 1;
        match self.commands.try_send(Command::Edit {
            generation,
            revision,
            text,
            edits: if rebuild { Vec::new() } else { edits },
            rebuild,
        }) {
            Ok(()) => {
                self.latest_generation = generation;
                self.pending = None;
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                self.generation.fetch_sub(1, Ordering::Release);
                // 拒收后不得再把后续 InputEdit 落到缺代的旧树上。
                self.pending = Some(PendingEdit {
                    text: pending_text,
                    revision,
                });
                Err(SyntaxWorkerError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(SyntaxWorkerError::Closed),
        }
    }

    /// 重试唯一待写副本；待写之后的编辑合并为全量重建，不需要旧输入链。
    pub fn flush(&mut self) -> Result<(), SyntaxWorkerError> {
        let Some(pending) = &self.pending else {
            return Ok(());
        };
        let generation = self.generation.fetch_add(1, Ordering::Release) + 1;
        match self.commands.try_send(Command::Edit {
            generation,
            revision: pending.revision,
            text: pending.text.clone(),
            edits: Vec::new(),
            rebuild: true,
        }) {
            Ok(()) => {
                self.latest_generation = generation;
                self.pending = None;
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                self.generation.fetch_sub(1, Ordering::Release);
                Err(SyntaxWorkerError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(SyntaxWorkerError::Closed),
        }
    }

    /// 隐藏会取消在途解析并释放工作线程旧树；显示时从最新正文重建。
    pub fn set_visible(&mut self, visible: bool) -> Result<(), SyntaxWorkerError> {
        if visible == self.visible {
            return Ok(());
        }
        if visible && self.pending.is_some() {
            self.flush()?;
        }
        let generation = self.generation.fetch_add(1, Ordering::Release) + 1;
        match self.commands.try_send(Command::SetVisible {
            generation,
            visible,
        }) {
            Ok(()) => {
                self.visible = visible;
                self.latest_generation = generation;
                if !visible {
                    while self.results.try_recv().is_ok() {}
                }
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                self.generation.fetch_sub(1, Ordering::Release);
                Err(SyntaxWorkerError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(SyntaxWorkerError::Closed),
        }
    }

    fn adopt(&self, publication: SyntaxPublication) -> Option<SyntaxTree> {
        if !self.visible
            || publication.document_id != self.document_id
            || publication.revision != self.latest_revision
            || publication.generation != self.latest_generation
        {
            return None;
        }
        Some(SyntaxTree {
            document_id: publication.document_id,
            revision: publication.revision,
            generation: publication.generation,
            tree: publication.tree.upgrade()?,
        })
    }

    /// 只采纳调用方最新 revision；等待方法不得由 GPUI 前台调用。
    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<SyntaxTree, RecvTimeoutError> {
        loop {
            let publication = self.results.recv_timeout(timeout)?;
            if let Some(result) = self.adopt(publication) {
                return Ok(result);
            }
        }
    }

    pub fn try_recv(&mut self) -> Result<Option<SyntaxTree>, TryRecvError> {
        loop {
            match self.results.try_recv() {
                Ok(publication) => {
                    if let Some(result) = self.adopt(publication) {
                        return Ok(Some(result));
                    }
                }
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err(TryRecvError::Disconnected),
            }
        }
    }

    /// 中止在途解析、排空结果并回收线程；只能在后台执行体调用。
    pub fn shutdown(mut self) -> Result<(), SyntaxWorkerError> {
        self.generation.fetch_add(1, Ordering::Release);
        let (disconnected, _) = sync_channel(0);
        drop(std::mem::replace(&mut self.commands, disconnected));
        // 结果发布满时会阻塞后台线程；先排水到发送端断开，再 join。
        while self.results.recv().is_ok() {}
        self.worker
            .take()
            .expect("shutdown 只消费一次 worker")
            .join()
            .map_err(|_| SyntaxWorkerError::ThreadPanicked)
    }
}

impl Drop for SyntaxWorker {
    fn drop(&mut self) {
        // 前台 Drop 不 join；断连与代次推进保证线程退出，显式回收走 shutdown。
        self.generation.fetch_add(1, Ordering::Release);
        let _ = self.commands.try_send(Command::Close);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_parser<F>(
    document_id: DocumentId,
    mut parser: Parser,
    mut text: Rope,
    mut revision: u64,
    commands: Receiver<Command>,
    results: SyncSender<SyntaxPublication>,
    current_generation: Arc<AtomicU64>,
    mut observer: F,
) where
    F: FnMut(WorkerObservation),
{
    let mut tree = None;
    let mut visible = true;
    if !parse_current(
        document_id,
        &mut parser,
        &text,
        revision,
        0,
        &mut tree,
        &results,
        &current_generation,
        &mut observer,
    ) {
        return;
    }
    while let Ok(command) = commands.recv() {
        match command {
            Command::Edit {
                generation,
                revision: next_revision,
                text: next_text,
                edits,
                rebuild,
            } => {
                text = next_text;
                revision = next_revision;
                if rebuild {
                    tree = None;
                } else if let Some(old_tree) = &mut tree {
                    for edit in &edits {
                        Arc::make_mut(old_tree).edit(edit);
                    }
                }
                if !visible || generation != current_generation.load(Ordering::Acquire) {
                    continue;
                }
                if !parse_current(
                    document_id,
                    &mut parser,
                    &text,
                    revision,
                    generation,
                    &mut tree,
                    &results,
                    &current_generation,
                    &mut observer,
                ) {
                    return;
                }
            }
            Command::SetVisible {
                generation,
                visible: next_visible,
            } => {
                visible = next_visible;
                if !visible {
                    tree = None;
                    observer(WorkerObservation::Hidden);
                } else if generation == current_generation.load(Ordering::Acquire)
                    && !parse_current(
                        document_id,
                        &mut parser,
                        &text,
                        revision,
                        generation,
                        &mut tree,
                        &results,
                        &current_generation,
                        &mut observer,
                    )
                {
                    return;
                }
            }
            Command::Close => break,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_current<F>(
    document_id: DocumentId,
    parser: &mut Parser,
    text: &Rope,
    revision: u64,
    generation: u64,
    tree: &mut Option<Arc<Tree>>,
    results: &SyncSender<SyntaxPublication>,
    current_generation: &Arc<AtomicU64>,
    observer: &mut F,
) -> bool
where
    F: FnMut(WorkerObservation),
{
    observer(WorkerObservation::ParseStart {
        revision,
        has_old_tree: tree.is_some(),
    });
    let mut progress = |_: &tree_sitter::ParseState| {
        observer(WorkerObservation::Progress);
        if current_generation.load(Ordering::Acquire) == generation {
            std::ops::ControlFlow::Continue(())
        } else {
            std::ops::ControlFlow::Break(())
        }
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let parsed = parser.parse_with_options(
        &mut |offset, _| {
            if offset == text.len() {
                return "";
            }
            let (chunk, chunk_start) = text.chunk(offset);
            &chunk[offset - chunk_start..]
        },
        tree.as_deref(),
        Some(options),
    );
    if current_generation.load(Ordering::Acquire) != generation {
        // 取消返回 None 时同样保留旧栈；新代次必须先 reset，不能续跑。
        parser.reset();
        return true;
    }
    let Some(parsed) = parsed else {
        return true;
    };
    let parsed = Arc::new(parsed);
    *tree = Some(Arc::clone(&parsed));
    let publication = SyntaxPublication {
        document_id,
        revision,
        generation,
        tree: Arc::downgrade(&parsed),
    };
    observer(WorkerObservation::Publish { revision });
    results.send(publication).is_ok()
}

/// 由一次已应用的完整替换生成 Tree-sitter 坐标；正文内部统一使用 LF。
pub fn syntax_input_edit(before: &Rope, after: &Rope, old_range: Range<usize>) -> InputEdit {
    let new_end = after.len() - (before.len() - old_range.end);
    InputEdit {
        start_byte: old_range.start,
        old_end_byte: old_range.end,
        new_end_byte: new_end,
        start_position: point(before, old_range.start),
        old_end_position: point(before, old_range.end),
        new_end_position: point(after, new_end),
    }
}

fn point(text: &Rope, byte: usize) -> Point {
    let row = text.byte_to_line_idx(byte, LineType::LF);
    Point::new(row, byte - text.line_to_byte_idx(row, LineType::LF))
}

#[cfg(test)]
#[path = "syntax_tests.rs"]
mod tests;
