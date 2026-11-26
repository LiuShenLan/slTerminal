//! Markdown 双态生产视图。

mod render;
mod resources;
mod search;
mod selection;

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::{
    App, AppContext as _, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable,
    IntoElement, ListAlignment, ListState, Render, ScrollHandle, Subscription, Task, Window, px,
};
use ropey::{LineType, Rope};
use slterm_document::{
    DocumentId, DocumentSession, EditTransaction, LanguageId, MarkdownDocument, MarkdownLink,
};
use slterm_path::{PathGrant, ResolvedPath};
use slterm_workspace::ViewKey;

use super::progressive::{
    BlockCompletion, MarkdownRenderDecision, MarkdownRenderGate, ProgressiveMarkdownRenderer,
};
use crate::app_shell::text_view::{
    DocumentEditSink, DocumentSyntax, DocumentTextEvent, DocumentTextSnapshot, DocumentTextView,
    DocumentViewStateBinding, EditorViewStateLease, EditorViewStateStore, MarkdownMode,
    MarkdownReadingPosition, MarkdownViewState,
};

#[derive(Clone)]
pub(crate) struct MarkdownRenderServices {
    images: Arc<Mutex<super::local_resource::MarkdownImageService>>,
    formulas: Option<Arc<Mutex<super::formula::FormulaBitmapCache>>>,
}

impl Default for MarkdownRenderServices {
    fn default() -> Self {
        let formulas = super::formula::FormulaBitmapCache::new()
            .map(|cache| Arc::new(Mutex::new(cache)))
            .map_err(|error| log::warn!("Markdown 公式位图缓存不可用: {error:?}"))
            .ok();
        Self {
            images: Arc::new(Mutex::new(
                super::local_resource::MarkdownImageService::default(),
            )),
            formulas,
        }
    }
}

pub(crate) enum MarkdownViewEvent {
    Save,
    OpenLink(MarkdownLink),
}

impl EventEmitter<MarkdownViewEvent> for MarkdownView {}

/// 两态只持同一份 Session 快照；渲染态没有源码编辑入口。
pub(crate) struct MarkdownView {
    document_id: DocumentId,
    location: ResolvedPath,
    text: Rope,
    revision: u64,
    read_only: bool,
    source: Entity<DocumentTextView>,
    render_services: MarkdownRenderServices,
    resource_grants: Vec<PathGrant>,
    images: HashMap<String, Arc<gpui::RenderImage>>,
    image_blocks: HashMap<String, BTreeSet<usize>>,
    image_failures: HashSet<String>,
    image_tasks: HashMap<String, (u64, Task<()>)>,
    remote_image_generations: HashMap<String, u64>,
    formula_images: HashMap<[u8; 32], Arc<gpui::RenderImage>>,
    formula_blocks: HashMap<[u8; 32], BTreeSet<usize>>,
    formula_failures: HashSet<[u8; 32]>,
    formula_tasks: HashMap<[u8; 32], (u64, Task<()>)>,
    next_resource_id: u64,
    mode: MarkdownMode,
    allow_large_document: bool,
    outline: bool,
    zoom: u16,
    document: Option<Arc<MarkdownDocument>>,
    document_revision: Option<u64>,
    pending_fragment: Option<(u64, String)>,
    parse_task: Option<Task<()>>,
    render_task: Option<Task<()>>,
    renderer: ProgressiveMarkdownRenderer,
    scroll: ListState,
    block_scrolls: Rc<RefCell<HashMap<usize, ScrollHandle>>>,
    text_layouts: Rc<RefCell<HashMap<String, selection::TextLayoutRecord>>>,
    selection_cursors: Rc<RefCell<HashMap<usize, usize>>>,
    selection: Option<selection::MarkdownSelection>,
    search: Option<search::MarkdownSearch>,
    selecting: bool,
    view_key: ViewKey,
    view_states: EditorViewStateStore,
    view_lease: EditorViewStateLease,
    focus: FocusHandle,
    _source_subscription: Subscription,
}

impl MarkdownView {
    pub(crate) fn new(
        session: &DocumentSession,
        edit_sink: DocumentEditSink,
        syntax: Option<Rc<std::cell::RefCell<DocumentSyntax>>>,
        render_services: MarkdownRenderServices,
        resource_grants: Vec<PathGrant>,
        view_state: DocumentViewStateBinding,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let restored = view_state
            .view_states()
            .markdown_state(view_state.view_key());
        let snapshot = DocumentTextSnapshot::from_session(session);
        let focus = cx.focus_handle().tab_stop(true);
        let source = cx.new(|cx| {
            DocumentTextView::new_with_binding(
                session,
                edit_sink,
                LanguageId::Markdown,
                false,
                syntax,
                view_state.clone(),
                cx,
            )
        });
        source.update(cx, |source, _| source.share_focus(focus.clone()));
        let source_subscription = cx.subscribe(&source, |_, _, event, cx| {
            if matches!(event, DocumentTextEvent::Save) {
                cx.emit(MarkdownViewEvent::Save);
            }
        });
        let scroll = ListState::new(0, ListAlignment::Top, px(500.0));
        let owner = cx.entity().downgrade();
        scroll.set_scroll_handler(move |event, _, cx| {
            let _ = owner.update(cx, |view, cx| {
                view.schedule_blocks(event.visible_range.clone(), cx);
                if event.is_scrolled {
                    view.persist_state();
                }
            });
        });
        let (view_key, view_states, view_lease) = view_state.into_parts();
        let mut view = Self {
            document_id: session.id(),
            location: session.location().clone(),
            text: snapshot.text,
            revision: snapshot.revision,
            read_only: snapshot.read_only,
            source,
            render_services,
            resource_grants,
            images: HashMap::new(),
            image_blocks: HashMap::new(),
            image_failures: HashSet::new(),
            image_tasks: HashMap::new(),
            remote_image_generations: HashMap::new(),
            formula_images: HashMap::new(),
            formula_blocks: HashMap::new(),
            formula_failures: HashSet::new(),
            formula_tasks: HashMap::new(),
            next_resource_id: 0,
            mode: restored.mode(),
            allow_large_document: false,
            outline: restored.outline_visible(),
            zoom: restored.zoom(),
            document: None,
            document_revision: None,
            pending_fragment: None,
            parse_task: None,
            render_task: None,
            renderer: ProgressiveMarkdownRenderer::default(),
            scroll,
            block_scrolls: Rc::default(),
            text_layouts: Rc::default(),
            selection_cursors: Rc::default(),
            selection: None,
            search: None,
            selecting: false,
            view_key,
            view_states,
            view_lease,
            focus,
            _source_subscription: source_subscription,
        };
        if view.mode == MarkdownMode::Render {
            view.schedule_parse(cx);
        }
        view
    }

    fn effective_mode(&self) -> MarkdownRenderDecision {
        MarkdownRenderGate::evaluate(
            self.text.len(),
            self.text.len_lines(LineType::LF),
            self.mode,
            self.allow_large_document,
        )
    }

    fn schedule_parse(&mut self, cx: &mut Context<'_, Self>) {
        if self.effective_mode() != MarkdownRenderDecision::Progressive {
            return;
        }
        let revision = self.revision;
        let source = self.text.to_string();
        let parse = cx
            .background_executor()
            .spawn(async move { MarkdownDocument::parse(&source) });
        self.parse_task = Some(cx.spawn(async move |this, cx| {
            let document = parse.await;
            let _ = this.update(cx, |view, cx| {
                if view.revision != revision || view.mode != MarkdownMode::Render {
                    return;
                }
                let restore = view
                    .view_states
                    .markdown_state(&view.view_key)
                    .reading()
                    .clone();
                view.scroll.reset(document.blocks().len());
                view.scroll.scroll_to(restore.resolve(&document));
                view.renderer.cancel();
                view.block_scrolls.borrow_mut().clear();
                view.document = Some(Arc::new(document));
                view.document_revision = Some(revision);
                if let Some((fragment_revision, fragment)) = view.pending_fragment.take()
                    && fragment_revision == view.revision
                    && let Some(block) = view
                        .document
                        .as_ref()
                        .and_then(|document| document.resolve_anchor(&fragment))
                {
                    view.scroll_to_block(block);
                }
                let initial = view
                    .document
                    .as_ref()
                    .map_or(0, |document| document.blocks().len().min(32));
                view.refresh_search(cx);
                view.schedule_blocks(0..initial, cx);
                cx.notify();
            });
        }));
    }

    pub(crate) fn relocate(&mut self, location: ResolvedPath, cx: &mut Context<'_, Self>) {
        self.location = location;
        self.images.clear();
        self.image_blocks.clear();
        self.image_failures.clear();
        self.image_tasks.clear();
        cx.notify();
    }

    fn schedule_blocks(&mut self, visible: std::ops::Range<usize>, cx: &mut Context<'_, Self>) {
        if self.effective_mode() != MarkdownRenderDecision::Progressive {
            return;
        }
        let Some(document) = self.document.clone() else {
            return;
        };
        self.render_task = None;
        self.prune_resources(visible.clone(), 4);
        self.renderer
            .schedule(self.revision, &self.text, &document, visible, 4);
        self.spawn_render_batch(cx);
    }

    fn spawn_render_batch(&mut self, cx: &mut Context<'_, Self>) {
        let tasks = self.renderer.next_chunk(16);
        if tasks.is_empty() {
            return;
        }
        let background = cx.background_executor().spawn(async move {
            tasks
                .into_iter()
                .map(|task| {
                    BlockCompletion::success(
                        &task,
                        task.document_revision(),
                        task.render_generation(),
                    )
                })
                .collect::<Vec<_>>()
        });
        self.render_task = Some(cx.spawn(async move |this, cx| {
            let completions = background.await;
            let _ = this.update(cx, |view, cx| {
                view.render_task = None;
                for completion in completions {
                    view.renderer.complete(completion);
                }
                view.spawn_render_batch(cx);
                cx.notify();
            });
        }));
    }

    pub(crate) fn apply_snapshot(&mut self, session: &DocumentSession, cx: &mut Context<'_, Self>) {
        let snapshot = DocumentTextSnapshot::from_session(session);
        self.adopt_snapshot(snapshot, session.location().clone());
        self.source
            .update(cx, |source, cx| source.apply_snapshot(session, cx));
        self.schedule_parse(cx);
        cx.notify();
    }

    pub(crate) fn apply_edit_snapshot(
        &mut self,
        session: &DocumentSession,
        transaction: &EditTransaction,
        origin: EntityId,
        cx: &mut Context<'_, Self>,
    ) {
        let snapshot = DocumentTextSnapshot::from_session(session);
        self.adopt_snapshot(snapshot, session.location().clone());
        if self.source.entity_id() != origin {
            self.source.update(cx, |source, cx| {
                source.apply_edit_snapshot(session, transaction, cx);
            });
        }
        self.schedule_parse(cx);
        cx.notify();
    }

    fn adopt_snapshot(&mut self, snapshot: DocumentTextSnapshot, location: ResolvedPath) {
        self.image_tasks.clear();
        self.formula_tasks.clear();
        self.images.clear();
        self.formula_images.clear();
        self.image_failures.clear();
        self.formula_failures.clear();
        self.selection = None;
        if let Some(search) = &mut self.search {
            search.matches.clear();
            search.active = None;
        }
        self.selecting = false;
        self.text_layouts.borrow_mut().clear();
        self.text = snapshot.text;
        self.revision = snapshot.revision;
        self.read_only = snapshot.read_only;
        self.location = location;
    }

    fn toggle_outline(&mut self, cx: &mut Context<'_, Self>) {
        self.outline = !self.outline;
        self.persist_state();
        cx.notify();
    }

    fn set_zoom(&mut self, zoom: u16, cx: &mut Context<'_, Self>) {
        self.zoom = MarkdownViewState::default().with_zoom(zoom).zoom();
        self.formula_tasks.clear();
        self.formula_images.clear();
        self.formula_blocks.clear();
        self.formula_failures.clear();
        self.persist_state();
        cx.notify();
    }

    fn zoom_in(&mut self, cx: &mut Context<'_, Self>) {
        let zoom = MarkdownViewState::default()
            .with_zoom(self.zoom)
            .zoom_in()
            .zoom();
        self.set_zoom(zoom, cx);
    }

    fn zoom_out(&mut self, cx: &mut Context<'_, Self>) {
        let zoom = MarkdownViewState::default()
            .with_zoom(self.zoom)
            .zoom_out()
            .zoom();
        self.set_zoom(zoom, cx);
    }

    fn zoom_reset(&mut self, cx: &mut Context<'_, Self>) {
        self.set_zoom(1_000, cx);
    }

    fn zoom_wheel(&mut self, event: &gpui::ScrollWheelEvent, cx: &mut Context<'_, Self>) {
        if self.mode != MarkdownMode::Render || !event.modifiers.control {
            cx.propagate();
            return;
        }
        let delta = event.delta.pixel_delta(px(16.0)).y.as_f32();
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        if delta > 0.0 {
            self.zoom_in(cx);
        } else {
            self.zoom_out(cx);
        }
    }

    fn allow_large_document(&mut self, cx: &mut Context<'_, Self>) {
        self.allow_large_document = true;
        self.schedule_parse(cx);
        cx.notify();
    }

    pub(crate) fn scroll_to_block(&mut self, block: slterm_document::BlockId) {
        self.scroll.scroll_to(gpui::ListOffset {
            item_ix: block.index(),
            offset_in_item: px(0.0),
        });
    }

    pub(crate) fn scroll_to_anchor(
        &mut self,
        target: Option<slterm_document::BlockId>,
        revision: u64,
    ) {
        if self.revision != revision {
            return;
        }
        self.pending_fragment = None;
        if let Some(block) = target {
            self.scroll_to_block(block);
        } else {
            self.scroll.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(0.0),
            });
        }
    }

    pub(crate) fn scroll_to_fragment(&mut self, fragment: &str, revision: u64) {
        if self.revision != revision {
            return;
        }
        if self.document_revision == Some(revision) {
            if let Some(block) = self
                .document
                .as_ref()
                .and_then(|document| document.resolve_anchor(fragment))
            {
                self.scroll_to_block(block);
            }
        } else {
            self.pending_fragment = Some((revision, fragment.to_owned()));
        }
    }

    pub(crate) fn renew_view_lease(&mut self, cx: &mut App) {
        self.view_states.renew(&mut self.view_lease);
        self.source
            .update(cx, |source, _| source.share_lease(self.view_lease.clone()));
    }

    fn persist_state(&self) {
        let top = self.scroll.logical_scroll_top();
        let reading = self.document.as_deref().map(|document| {
            MarkdownReadingPosition::capture(
                top.item_ix,
                f32::from(top.offset_in_item).max(0.0).round() as u32,
                document,
            )
        });
        let mut state = MarkdownViewState::new(self.mode)
            .with_outline(self.outline)
            .with_zoom(self.zoom)
            .with_reading(
                self.view_states
                    .markdown_state(&self.view_key)
                    .reading()
                    .clone(),
            );
        if let Some(reading) = reading {
            state = state.with_reading(reading);
        }
        self.view_states
            .checkpoint_markdown(&self.view_lease, state);
    }

    fn toggle_mode(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.mode = match self.mode {
            MarkdownMode::Source => MarkdownMode::Render,
            MarkdownMode::Render => MarkdownMode::Source,
        };
        match self.mode {
            MarkdownMode::Source => {
                self.search = None;
                self.parse_task = None;
                self.render_task = None;
                self.image_tasks.clear();
                self.formula_tasks.clear();
                self.images.clear();
                self.image_blocks.clear();
                self.image_failures.clear();
                self.formula_images.clear();
                self.formula_blocks.clear();
                self.formula_failures.clear();
                self.renderer.cancel();
                let focus = self.source.read(cx).focus_handle(cx);
                focus.focus(window, cx);
            }
            MarkdownMode::Render => {
                self.schedule_parse(cx);
                self.focus.focus(window, cx);
            }
        }
        self.persist_state();
        cx.notify();
    }

    pub(crate) fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn toggle_mode_for_test(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.toggle_mode(window, cx);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn source_for_test(&self) -> Entity<DocumentTextView> {
        self.source.clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn input_text_for_test(&self, cx: &App) -> String {
        self.source.read(cx).input_text_for_test()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_reading_for_test(&mut self, block: usize, offset: u32) {
        self.scroll.scroll_to(gpui::ListOffset {
            item_ix: block,
            offset_in_item: px(offset as f32),
        });
        self.persist_state();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn display_state_for_test(&self) -> (MarkdownMode, bool, u16) {
        (self.mode, self.outline, self.zoom)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn toggle_outline_for_test(&mut self, cx: &mut Context<'_, Self>) {
        self.toggle_outline(cx);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn zoom_in_for_test(&mut self, cx: &mut Context<'_, Self>) {
        self.zoom_in(cx);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn rendered_text_for_test(&self) -> String {
        self.document
            .as_deref()
            .map(|document| document.visible_text().to_owned())
            .unwrap_or_default()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn rendered_revision_for_test(&self) -> u64 {
        self.revision
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn reading_top_for_test(&self) -> (usize, u32) {
        let top = self.scroll.logical_scroll_top();
        (
            top.item_ix,
            f32::from(top.offset_in_item).max(0.0).round() as u32,
        )
    }
}

impl Focusable for MarkdownView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        MarkdownView::focus_handle(self, cx)
    }
}

impl Render for MarkdownView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        render::render_markdown(self, window, cx)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;
