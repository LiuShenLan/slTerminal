//! Markdown 大文档门禁与可取消的视口分片调度。

use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use slterm_document::{BlockId, MarkdownBlock, MarkdownBlockKind, MarkdownDocument};

const PROGRESSIVE_SOURCE_BYTES: usize = 5_000_000;
const FORBIDDEN_SOURCE_BYTES: usize = 50_000_000;
const PROGRESSIVE_SOURCE_LINES: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownMode {
    Source,
    Render,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownRenderDecision {
    Source,
    Progressive,
    Forbidden,
}

pub(crate) struct MarkdownRenderGate;

impl MarkdownRenderGate {
    pub(crate) fn evaluate(
        source_bytes: usize,
        source_lines: usize,
        mode: MarkdownMode,
        allow_large_document: bool,
    ) -> MarkdownRenderDecision {
        if source_bytes > FORBIDDEN_SOURCE_BYTES {
            return MarkdownRenderDecision::Forbidden;
        }
        if mode == MarkdownMode::Source {
            return MarkdownRenderDecision::Source;
        }
        if (source_bytes > PROGRESSIVE_SOURCE_BYTES || source_lines > PROGRESSIVE_SOURCE_LINES)
            && !allow_large_document
        {
            return MarkdownRenderDecision::Source;
        }
        MarkdownRenderDecision::Progressive
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BlockRenderTask {
    document_revision: u64,
    render_generation: u64,
    block: MarkdownBlock,
    source: String,
}

impl BlockRenderTask {
    pub(crate) fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub(crate) fn render_generation(&self) -> u64 {
        self.render_generation
    }

    pub(crate) fn block(&self) -> &MarkdownBlock {
        &self.block
    }
}

pub(crate) struct BlockCompletion {
    block: BlockId,
    document_revision: u64,
    render_generation: u64,
    result: Result<MarkdownBlockKind, MarkdownBlockFailure>,
}

impl BlockCompletion {
    pub(crate) fn success(
        task: &BlockRenderTask,
        document_revision: u64,
        render_generation: u64,
    ) -> Self {
        Self {
            block: task.block.id(),
            document_revision,
            render_generation,
            result: Ok(task.block.kind().clone()),
        }
    }

    pub(crate) fn failure(
        task: &BlockRenderTask,
        document_revision: u64,
        render_generation: u64,
        failure: MarkdownBlockFailure,
    ) -> Self {
        Self {
            block: task.block.id(),
            document_revision,
            render_generation,
            result: Err(failure),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownBlockFailure {
    Image,
    Formula,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownBlockOutcome {
    Rendered(MarkdownBlockKind),
    Fallback {
        source: String,
        failure: MarkdownBlockFailure,
    },
}

#[derive(Default)]
pub(crate) struct ProgressiveMarkdownRenderer {
    started: bool,
    document_revision: Option<u64>,
    render_generation: u64,
    pending: VecDeque<BlockRenderTask>,
    inflight: HashMap<BlockId, BlockRenderTask>,
    outcomes: HashMap<BlockId, MarkdownBlockOutcome>,
}

impl ProgressiveMarkdownRenderer {
    pub(crate) fn schedule(
        &mut self,
        document_revision: u64,
        source: &str,
        document: &MarkdownDocument,
        visible: Range<usize>,
        overscan: usize,
    ) {
        if self.started {
            self.advance_generation();
        }
        self.started = true;
        self.document_revision = Some(document_revision);
        self.pending.clear();
        self.inflight.clear();
        self.outcomes.clear();
        if visible.is_empty() {
            return;
        }
        let count = document.blocks().len();
        let start = visible.start.saturating_sub(overscan).min(count);
        let end = visible.end.saturating_add(overscan).min(count);
        for block in &document.blocks()[start..end] {
            let source = source
                .get(block.source_range().clone())
                .unwrap_or_default()
                .to_owned();
            self.pending.push_back(BlockRenderTask {
                document_revision,
                render_generation: self.render_generation,
                block: block.clone(),
                source,
            });
        }
    }

    pub(crate) fn next_chunk(&mut self, limit: usize) -> Vec<BlockRenderTask> {
        let mut tasks = Vec::with_capacity(limit.min(self.pending.len()));
        while tasks.len() < limit {
            let Some(task) = self.pending.pop_front() else {
                break;
            };
            self.inflight.insert(task.block.id(), task.clone());
            tasks.push(task);
        }
        tasks
    }

    pub(crate) fn complete(&mut self, completion: BlockCompletion) {
        if self.document_revision != Some(completion.document_revision)
            || self.render_generation != completion.render_generation
        {
            return;
        }
        let Some(task) = self.inflight.remove(&completion.block) else {
            return;
        };
        let outcome = match completion.result {
            Ok(kind) => MarkdownBlockOutcome::Rendered(kind),
            Err(failure) => MarkdownBlockOutcome::Fallback {
                source: task.source,
                failure,
            },
        };
        self.outcomes.insert(completion.block, outcome);
    }

    pub(crate) fn cancel(&mut self) {
        self.advance_generation();
        self.started = true;
        self.document_revision = None;
        self.pending.clear();
        self.inflight.clear();
        self.outcomes.clear();
    }

    pub(crate) fn outcome(&self, block: BlockId) -> Option<&MarkdownBlockOutcome> {
        self.outcomes.get(&block)
    }

    fn advance_generation(&mut self) {
        self.render_generation = self
            .render_generation
            .checked_add(1)
            .expect("Markdown 渲染代次不得耗尽");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slterm_document::MarkdownDocument;

    #[test]
    fn markdown_large_document_degradation() {
        assert_eq!(
            MarkdownRenderGate::evaluate(1_000, 10, MarkdownMode::Render, false),
            MarkdownRenderDecision::Progressive
        );
        assert_eq!(
            MarkdownRenderGate::evaluate(5_000_000, 100_000, MarkdownMode::Render, false),
            MarkdownRenderDecision::Progressive
        );
        for (bytes, lines) in [(5_000_001, 1), (1_000, 100_001)] {
            assert_eq!(
                MarkdownRenderGate::evaluate(bytes, lines, MarkdownMode::Render, false),
                MarkdownRenderDecision::Source
            );
            assert_eq!(
                MarkdownRenderGate::evaluate(bytes, lines, MarkdownMode::Render, true),
                MarkdownRenderDecision::Progressive
            );
        }
        assert_eq!(
            MarkdownRenderGate::evaluate(50_000_001, 1, MarkdownMode::Render, true),
            MarkdownRenderDecision::Forbidden
        );
        assert_eq!(
            MarkdownRenderGate::evaluate(1_000, 1, MarkdownMode::Source, false),
            MarkdownRenderDecision::Source
        );
    }

    #[test]
    fn progressive_renderer_scopes_viewport_and_rejects_stale_results() {
        let source = (0..8)
            .map(|index| format!("paragraph {index}\n\n"))
            .collect::<String>();
        let document = MarkdownDocument::parse(&source);
        let mut renderer = ProgressiveMarkdownRenderer::default();
        renderer.schedule(7, &source, &document, 2..4, 1);
        let tasks = renderer.next_chunk(2);
        assert_eq!(
            tasks
                .iter()
                .map(|task| task.block().id())
                .collect::<Vec<_>>(),
            document.blocks()[1..3]
                .iter()
                .map(|block| block.id())
                .collect::<Vec<_>>()
        );
        for task in &tasks {
            assert_eq!(task.document_revision(), 7);
            assert_eq!(task.render_generation(), 0);
        }
        let generation = tasks[0].render_generation();
        renderer.complete(BlockCompletion::success(&tasks[0], 6, generation));
        assert!(renderer.outcome(tasks[0].block().id()).is_none());
        renderer.complete(BlockCompletion::success(&tasks[0], 7, generation + 1));
        assert!(renderer.outcome(tasks[0].block().id()).is_none());

        renderer.cancel();
        renderer.complete(BlockCompletion::success(&tasks[0], 7, generation));
        assert!(renderer.outcome(tasks[0].block().id()).is_none());
        renderer.schedule(7, &source, &document, 2..3, 0);
        let current = renderer.next_chunk(1);
        assert_eq!(current[0].render_generation(), 2);
        renderer.complete(BlockCompletion::success(&current[0], 7, 2));
        assert!(renderer.outcome(current[0].block().id()).is_some());
    }

    #[test]
    fn invalid_viewport_range_schedules_no_work() {
        let source = "first\n\nsecond\n";
        let document = MarkdownDocument::parse(source);
        let mut renderer = ProgressiveMarkdownRenderer::default();
        let start = 2;
        let end = 1;
        renderer.schedule(1, source, &document, start..end, 0);
        assert!(renderer.next_chunk(2).is_empty());
    }

    #[test]
    fn single_block_failure_falls_back_to_source_without_blocking_neighbors() {
        let source = "first\n\nsecond\n";
        let document = MarkdownDocument::parse(source);
        let mut renderer = ProgressiveMarkdownRenderer::default();
        renderer.schedule(3, source, &document, 0..2, 0);
        let tasks = renderer.next_chunk(2);
        renderer.complete(BlockCompletion::failure(
            &tasks[0],
            3,
            0,
            MarkdownBlockFailure::Formula,
        ));
        renderer.complete(BlockCompletion::success(&tasks[1], 3, 0));
        let failed = renderer.outcome(tasks[0].block().id()).unwrap();
        let MarkdownBlockOutcome::Fallback { source, failure } = failed else {
            panic!("失败块必须回退源码")
        };
        assert_eq!(source, "first");
        assert_eq!(failure, &MarkdownBlockFailure::Formula);
        assert!(matches!(
            renderer.outcome(tasks[1].block().id()).unwrap(),
            MarkdownBlockOutcome::Rendered(_)
        ));
    }
}
