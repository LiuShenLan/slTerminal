//! Editor 滚动状态的强类型 DTO、恢复缓存与统一写通道。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{ListOffset, px};
use serde::{Deserialize, Serialize};
use slterm_document::{DocumentId, LanguageId, MarkdownDocument};
use slterm_workspace::{
    InvalidViewState, ProjectId, ViewKey, ViewKeyKind, ViewState, ViewStateRegistry,
    ViewStateSchema,
};

use crate::persistence::workspace_writer::WorkspaceWriterClient;

/// 只保存语言覆盖、软换行与阅读位置；选区、目标列、IME 与待续动作均为瞬态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditorViewState {
    scroll_line: u64,
    scroll_column: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language_override: Option<LanguageId>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl EditorViewState {
    pub(crate) fn new(scroll_line: usize, scroll_column: usize) -> Self {
        Self {
            scroll_line: scroll_line as u64,
            scroll_column: scroll_column as u64,
            wrap: false,
            language_override: None,
        }
    }

    pub(crate) fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub(crate) fn wrap(self) -> bool {
        self.wrap
    }

    pub(crate) fn with_language_override(mut self, language: Option<LanguageId>) -> Self {
        self.language_override = language;
        self
    }

    pub(crate) fn language_override(self) -> Option<LanguageId> {
        self.language_override
    }

    pub(crate) fn scroll_line(self) -> usize {
        usize::try_from(self.scroll_line).unwrap_or(usize::MAX)
    }

    pub(crate) fn scroll_column(self) -> usize {
        usize::try_from(self.scroll_column).unwrap_or(usize::MAX)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MarkdownMode {
    Source,
    #[default]
    Render,
}

/// 持久阅读位置使用标题锚点与相对块序；绝对位置仅供锚点消失后的比例回退。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarkdownReadingPosition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    block_offset: u64,
    block_index: u64,
    document_blocks: u64,
    offset_in_block: u32,
}

impl MarkdownReadingPosition {
    pub(crate) fn capture(
        block_index: usize,
        offset_in_block: u32,
        document: &MarkdownDocument,
    ) -> Self {
        let heading = document
            .headings()
            .iter()
            .rfind(|heading| heading.block().index() <= block_index);
        Self {
            anchor: heading.map(|heading| heading.anchor().to_owned()),
            block_offset: heading.map_or(block_index, |heading| {
                block_index.saturating_sub(heading.block().index())
            }) as u64,
            block_index: block_index as u64,
            document_blocks: document.blocks().len() as u64,
            offset_in_block,
        }
    }

    pub(crate) fn resolve(&self, document: &MarkdownDocument) -> ListOffset {
        let count = document.blocks().len();
        let item_ix = if count == 0 {
            0
        } else if let Some(heading) = self.anchor.as_deref().and_then(|anchor| {
            document
                .headings()
                .iter()
                .find(|heading| heading.anchor() == anchor)
        }) {
            heading
                .block()
                .index()
                .saturating_add(self.block_offset as usize)
                .min(count - 1)
        } else if self.document_blocks == 0 {
            self.block_index as usize
        } else {
            ((self.block_index as u128 * count as u128) / self.document_blocks as u128) as usize
        }
        .min(count.saturating_sub(1));
        ListOffset {
            item_ix,
            offset_in_item: px(self.offset_in_block as f32),
        }
    }
}

/// Markdown 与源码子视图共享同一 ViewKey，但各自只更新所属字段。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarkdownViewState {
    #[serde(default)]
    mode: MarkdownMode,
    #[serde(default = "default_markdown_outline")]
    outline: bool,
    #[serde(default = "default_markdown_zoom")]
    zoom: u16,
    #[serde(default)]
    reading: MarkdownReadingPosition,
    #[serde(default)]
    source: EditorViewState,
}

const fn default_markdown_outline() -> bool {
    true
}

const fn default_markdown_zoom() -> u16 {
    1_000
}

impl Default for MarkdownViewState {
    fn default() -> Self {
        Self::new(MarkdownMode::Render)
    }
}

impl MarkdownViewState {
    pub(crate) fn new(mode: MarkdownMode) -> Self {
        Self {
            mode,
            outline: default_markdown_outline(),
            zoom: default_markdown_zoom(),
            reading: MarkdownReadingPosition::default(),
            source: EditorViewState::default(),
        }
    }

    pub(crate) fn with_outline(mut self, visible: bool) -> Self {
        self.outline = visible;
        self
    }

    pub(crate) fn with_zoom(mut self, zoom: u16) -> Self {
        self.zoom = zoom.clamp(250, 4_000);
        self
    }

    pub(crate) fn zoom_in(self) -> Self {
        let zoom = (self.zoom as u32 * 11 / 10).min(4_000) as u16;
        self.with_zoom(zoom)
    }

    pub(crate) fn zoom_out(self) -> Self {
        let zoom = (self.zoom as u32 * 10 / 11).max(250) as u16;
        self.with_zoom(zoom)
    }

    pub(crate) fn with_source(mut self, source: EditorViewState) -> Self {
        self.source = source;
        self
    }

    pub(crate) fn with_reading(mut self, reading: MarkdownReadingPosition) -> Self {
        self.reading = reading;
        self
    }

    pub(crate) const fn mode(&self) -> MarkdownMode {
        self.mode
    }

    pub(crate) const fn outline_visible(&self) -> bool {
        self.outline
    }

    pub(crate) const fn zoom(&self) -> u16 {
        self.zoom
    }

    pub(crate) fn source(&self) -> EditorViewState {
        self.source
    }

    pub(crate) fn reading(&self) -> &MarkdownReadingPosition {
        &self.reading
    }

    fn is_valid(&self) -> bool {
        (250..=4_000).contains(&self.zoom)
    }
}

#[derive(Clone, Debug)]
enum StoredDocumentViewState {
    Editor(EditorViewState),
    Markdown(MarkdownViewState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EditorKey {
    kind: ViewKeyKind,
    project_id: ProjectId,
    document_id: DocumentId,
}

impl EditorKey {
    fn new(key: &ViewKey) -> Self {
        let (kind, project_id, document_id) = match key {
            ViewKey::Editor {
                project_id,
                document_id,
            } => (ViewKeyKind::Editor, project_id, document_id),
            ViewKey::Markdown {
                project_id,
                document_id,
            } => (ViewKeyKind::Markdown, project_id, document_id),
            ViewKey::Diff {
                project_id,
                document_id,
            } => (ViewKeyKind::Diff, project_id, document_id),
            _ => panic!("EditorViewStateStore 只接受 Document 文本 ViewKey"),
        };
        Self {
            kind,
            project_id: *project_id,
            document_id: *document_id,
        }
    }

    fn view_key(self) -> ViewKey {
        match self.kind {
            ViewKeyKind::Editor => ViewKey::Editor {
                project_id: self.project_id,
                document_id: self.document_id,
            },
            ViewKeyKind::Markdown => ViewKey::Markdown {
                project_id: self.project_id,
                document_id: self.document_id,
            },
            ViewKeyKind::Diff => ViewKey::Diff {
                project_id: self.project_id,
                document_id: self.document_id,
            },
            _ => unreachable!("EditorKey 只构造 Document 文本视图"),
        }
    }
}

fn decode_editor_view_state(
    key: &ViewKey,
    text: &str,
) -> Result<serde_json::Value, InvalidViewState> {
    match key {
        ViewKey::Editor { .. } | ViewKey::Diff { .. } => {
            let state: EditorViewState =
                serde_json::from_str(text).map_err(|_| InvalidViewState)?;
            serde_json::to_value(state).map_err(|_| InvalidViewState)
        }
        ViewKey::Markdown { .. } => {
            let state: MarkdownViewState =
                serde_json::from_str(text).map_err(|_| InvalidViewState)?;
            if !state.is_valid() {
                return Err(InvalidViewState);
            }
            serde_json::to_value(state).map_err(|_| InvalidViewState)
        }
        _ => Err(InvalidViewState),
    }
}

/// Editor、Markdown 与 Diff 按完整 ViewKey 解码各自专属 DTO。
pub(crate) fn document_view_state_schemas() -> Vec<ViewStateSchema> {
    [
        ViewKeyKind::Editor,
        ViewKeyKind::Markdown,
        ViewKeyKind::Diff,
    ]
    .map(|kind| ViewStateSchema::new(kind, decode_editor_view_state))
    .into_iter()
    .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct EditorViewStateLease {
    key: ViewKey,
    generation: u64,
}

impl EditorViewStateLease {
    fn key(&self) -> &ViewKey {
        &self.key
    }
}

#[derive(Default)]
struct SharedEditorViewStates {
    restored: BTreeMap<EditorKey, StoredDocumentViewState>,
    generations: BTreeMap<EditorKey, u64>,
    dirty: BTreeSet<EditorKey>,
    pending_final: BTreeMap<EditorKey, ViewState>,
    removed_projects: BTreeSet<ProjectId>,
}

/// 同一 ViewKey 的关闭重开与重启恢复共用一份状态事实。
#[derive(Clone)]
pub(crate) struct EditorViewStateStore {
    registry: Arc<ViewStateRegistry>,
    writer: Option<WorkspaceWriterClient>,
    shared: Rc<RefCell<SharedEditorViewStates>>,
}

#[derive(Clone)]
pub(crate) struct DocumentViewStateBinding {
    view_key: ViewKey,
    view_states: EditorViewStateStore,
    view_lease: EditorViewStateLease,
}

impl DocumentViewStateBinding {
    pub(crate) fn new(view_key: ViewKey, view_states: EditorViewStateStore) -> Self {
        let view_lease = view_states.lease(&view_key);
        Self {
            view_key,
            view_states,
            view_lease,
        }
    }

    pub(crate) fn view_key(&self) -> &ViewKey {
        &self.view_key
    }

    pub(crate) fn view_states(&self) -> &EditorViewStateStore {
        &self.view_states
    }

    pub(crate) fn into_parts(self) -> (ViewKey, EditorViewStateStore, EditorViewStateLease) {
        (self.view_key, self.view_states, self.view_lease)
    }
}

impl EditorViewStateStore {
    pub(crate) fn new(
        registry: Arc<ViewStateRegistry>,
        writer: Option<WorkspaceWriterClient>,
    ) -> Self {
        Self {
            registry,
            writer,
            shared: Rc::default(),
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn transient() -> Self {
        let registry = ViewStateRegistry::new(&document_view_state_schemas())
            .expect("Document 文本 ViewState schema 封闭注册不得重复");
        Self::new(Arc::new(registry), None)
    }

    pub(crate) fn restore(&self, key: ViewKey, state: EditorViewState) {
        let key = EditorKey::new(&key);
        self.shared
            .borrow_mut()
            .restored
            .insert(key, StoredDocumentViewState::Editor(state));
    }

    pub(crate) fn restore_markdown(&self, key: ViewKey, state: MarkdownViewState) {
        let key = EditorKey::new(&key);
        self.shared
            .borrow_mut()
            .restored
            .insert(key, StoredDocumentViewState::Markdown(state));
    }

    pub(crate) fn lease(&self, key: &ViewKey) -> EditorViewStateLease {
        let id = EditorKey::new(key);
        let generation = {
            let mut shared = self.shared.borrow_mut();
            let current = shared.generations.entry(id).or_insert(0);
            *current = current.checked_add(1).expect("Editor 视图代次不得耗尽");
            *current
        };
        EditorViewStateLease {
            key: key.clone(),
            generation,
        }
    }

    /// 失败回滚后为仍存活的旧视图重新取得最新租约。
    pub(crate) fn renew(&self, lease: &mut EditorViewStateLease) {
        *lease = self.lease(&lease.key.clone());
    }

    pub(crate) fn state(&self, key: &ViewKey) -> EditorViewState {
        let id = EditorKey::new(key);
        match self.shared.borrow().restored.get(&id) {
            Some(StoredDocumentViewState::Editor(state)) => *state,
            Some(StoredDocumentViewState::Markdown(state)) => state.source(),
            None => EditorViewState::default(),
        }
    }

    pub(crate) fn markdown_state(&self, key: &ViewKey) -> MarkdownViewState {
        let id = EditorKey::new(key);
        match self.shared.borrow().restored.get(&id) {
            Some(StoredDocumentViewState::Markdown(state)) => state.clone(),
            _ => MarkdownViewState::default(),
        }
    }

    pub(crate) fn remove_project(&self, project_id: ProjectId) {
        let mut shared = self.shared.borrow_mut();
        shared.removed_projects.insert(project_id);
        shared
            .restored
            .retain(|key, _| key.project_id != project_id);
        shared.dirty.retain(|key| key.project_id != project_id);
        shared
            .generations
            .retain(|key, _| key.project_id != project_id);
        shared
            .pending_final
            .retain(|key, _| key.project_id != project_id);
    }

    pub(crate) fn checkpoint(&self, lease: &EditorViewStateLease, state: EditorViewState) {
        let state = if matches!(lease.key(), ViewKey::Markdown { .. }) {
            StoredDocumentViewState::Markdown(self.markdown_state(lease.key()).with_source(state))
        } else {
            StoredDocumentViewState::Editor(state)
        };
        self.checkpoint_state(lease, state);
    }

    pub(crate) fn checkpoint_markdown(
        &self,
        lease: &EditorViewStateLease,
        state: MarkdownViewState,
    ) {
        let state = state.with_source(self.state(lease.key()));
        self.checkpoint_state(lease, StoredDocumentViewState::Markdown(state));
    }

    fn checkpoint_state(&self, lease: &EditorViewStateLease, state: StoredDocumentViewState) {
        let key = lease.key();
        let id = EditorKey::new(key);
        let current = {
            let shared = self.shared.borrow();
            !shared.removed_projects.contains(&id.project_id)
                && shared.generations.get(&id) == Some(&lease.generation)
        };
        if !current {
            return;
        }
        let value = match &state {
            StoredDocumentViewState::Editor(state) => serde_json::to_value(state),
            StoredDocumentViewState::Markdown(state) => serde_json::to_value(state),
        }
        .expect("Document ViewState 必须可编码");
        let persisted = match ViewState::new(key, value, &self.registry) {
            Ok(state) => state,
            Err(error) => {
                log::warn!("could not encode Document ViewState: {error}");
                return;
            }
        };
        {
            let mut shared = self.shared.borrow_mut();
            shared.restored.insert(id, state);
            shared.dirty.insert(id);
        }
        let Some(writer) = &self.writer else {
            return;
        };
        match writer.save_view_state(key, &persisted) {
            Ok(()) => {
                self.shared.borrow_mut().pending_final.remove(&id);
            }
            Err(error) => {
                self.shared.borrow_mut().pending_final.insert(id, persisted);
                log::warn!("could not queue Document ViewState: {error}");
            }
        }
    }

    /// 只提交显式变更与曾拒收的副本；启动默认态不得回写。
    pub(crate) fn final_states(&self) -> Vec<(ViewKey, ViewState)> {
        let mut final_states = self.shared.borrow().pending_final.clone();
        let dirty = self.shared.borrow().dirty.clone();
        for id in dirty {
            let Some(state) = self.shared.borrow().restored.get(&id).cloned() else {
                continue;
            };
            let key = id.view_key();
            let value = match &state {
                StoredDocumentViewState::Editor(state) => serde_json::to_value(state),
                StoredDocumentViewState::Markdown(state) => serde_json::to_value(state),
            }
            .expect("Document ViewState 必须可编码");
            match ViewState::new(&key, value, &self.registry) {
                Ok(state) => {
                    final_states.insert(id, state);
                }
                Err(error) => log::warn!("could not encode final Document ViewState: {error}"),
            }
        }
        final_states
            .into_iter()
            .map(|(id, state)| (id.view_key(), state))
            .collect()
    }

    pub(crate) fn final_submitted(&self, key: &ViewKey) {
        let id = EditorKey::new(key);
        let mut shared = self.shared.borrow_mut();
        shared.dirty.remove(&id);
        shared.pending_final.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use slterm_document::LanguageId;
    use slterm_path::AppPaths;
    use slterm_workspace::{ViewState, ViewStateRegistry};

    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::{WorkspaceWriter, WorkspaceWriterError};

    use super::{EditorViewState, EditorViewStateStore, document_view_state_schemas};

    fn key() -> slterm_workspace::ViewKey {
        slterm_workspace::ViewKey::Editor {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
            document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
        }
    }

    #[test]
    fn editor_view_state_rejects_unknown_and_transient_fields() {
        let registry = ViewStateRegistry::new(&document_view_state_schemas()).unwrap();
        let valid = ViewState::new(
            &key(),
            json!({"scroll_line": 3, "scroll_column": 4}),
            &registry,
        )
        .unwrap();
        assert!(valid.to_canonical_json().unwrap().contains("scroll_line"));
        for state in [
            json!({"scroll_line": 3}),
            json!({"scroll_line": 3, "scroll_column": 4, "selection": [1, 2]}),
            json!({"scroll_line": 3, "scroll_column": 4, "pending_motion": true}),
            json!({"scroll_line": 3, "scroll_column": 4, "language_override": "unknown"}),
        ] {
            assert!(ViewState::new(&key(), state, &registry).is_err());
        }
    }

    #[test]
    fn editor_view_state_queue_full_preserves_latest_final_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let registry =
            std::sync::Arc::new(ViewStateRegistry::new(&document_view_state_schemas()).unwrap());
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let locker = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        let writer = WorkspaceWriter::start(paths).unwrap();
        let client = writer.client();
        let key = key();
        let initial = ViewState::new(
            &key,
            json!({"scroll_line": 1, "scroll_column": 1}),
            &registry,
        )
        .unwrap();

        // 用真实 SQLite 写锁停住写线程，再填满普通命令队列。
        locker.execute_batch("BEGIN EXCLUSIVE").unwrap();
        client.save_view_state(&key, &initial).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
        let mut responses = Vec::new();
        loop {
            match client.query_operation("missing") {
                Ok(response) => responses.push(response),
                Err(WorkspaceWriterError::QueueFull) => break,
                Err(error) => panic!("填充写队列失败: {error}"),
            }
        }
        assert!(!responses.is_empty());

        let store = EditorViewStateStore::new(registry.clone(), Some(client.clone()));
        let lease = store.lease(&key);
        store.checkpoint(&lease, EditorViewState::new(3, 4));
        assert_eq!(store.shared.borrow().pending_final.len(), 1);

        locker.execute_batch("ROLLBACK").unwrap();
        for response in responses {
            response
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
        }

        // 后续普通入队成功必须作废旧拒收副本，而不是在退出时复活旧滚动位置。
        store.checkpoint(&lease, EditorViewState::new(5, 6));
        assert!(store.shared.borrow().pending_final.is_empty());
        let final_states = store.final_states();
        assert_eq!(final_states.len(), 1);
        client
            .save_final_view_state(&final_states[0].0, &final_states[0].1)
            .unwrap();
        store.final_submitted(&key);
        writer.flush().unwrap();

        let persisted = observer.view_state(&key, &registry).unwrap().unwrap();
        let decoded =
            serde_json::from_value::<EditorViewState>(persisted.state.into_state()).unwrap();
        assert_eq!(decoded, EditorViewState::new(5, 6));
        let reloaded = EditorViewStateStore::new(registry, None);
        reloaded.restore(key.clone(), decoded);
        assert_eq!(reloaded.state(&key), EditorViewState::new(5, 6));
        writer.shutdown().unwrap();
    }

    #[test]
    fn stale_editor_view_state_lease_cannot_overwrite_current_state() {
        let registry =
            std::sync::Arc::new(ViewStateRegistry::new(&document_view_state_schemas()).unwrap());
        let store = EditorViewStateStore::new(registry, None);
        let old_lease = store.lease(&key());
        store.checkpoint(&old_lease, super::EditorViewState::new(1, 1));
        let current_lease = store.lease(&key());
        store.checkpoint(&current_lease, super::EditorViewState::new(2, 2));

        store.checkpoint(&old_lease, super::EditorViewState::new(3, 3));
        drop(old_lease);

        let current = store.state(&key());
        assert_eq!((current.scroll_line(), current.scroll_column()), (2, 2));
        let final_states = store.final_states();
        assert_eq!(final_states.len(), 1);
        assert_eq!(
            final_states[0].1.to_canonical_json().unwrap(),
            json!({"kind":"editor","state":{"scroll_line":2,"scroll_column":2},"version":1})
                .to_string()
        );
    }

    #[test]
    fn editor_view_state_accepts_registered_language_override() {
        let registry = ViewStateRegistry::new(&document_view_state_schemas()).unwrap();
        for language in LanguageId::ALL {
            let state = ViewState::new(
                &key(),
                json!({
                    "scroll_line": 3,
                    "scroll_column": 4,
                    "language_override": language.as_str()
                }),
                &registry,
            )
            .unwrap();
            assert!(
                state
                    .to_canonical_json()
                    .unwrap()
                    .contains(&format!("\"language_override\":\"{}\"", language.as_str()))
            );
        }
    }

    #[test]
    fn editor_view_state_persists_only_explicit_wrap_choice() {
        let registry = ViewStateRegistry::new(&document_view_state_schemas()).unwrap();
        let wrapped = EditorViewState::new(3, 0).with_wrap(true);
        let state =
            ViewState::new(&key(), serde_json::to_value(wrapped).unwrap(), &registry).unwrap();
        assert!(state.to_canonical_json().unwrap().contains("\"wrap\":true"));
        let decoded = serde_json::from_value::<EditorViewState>(state.into_state()).unwrap();
        assert!(decoded.wrap());
        let default = ViewState::new(
            &key(),
            json!({"scroll_line": 3, "scroll_column": 4}),
            &registry,
        )
        .unwrap();
        assert!(!default.to_canonical_json().unwrap().contains("\"wrap\""));
    }

    #[test]
    fn document_text_view_states_keep_markdown_and_diff_identity() {
        let registry =
            std::sync::Arc::new(ViewStateRegistry::new(&document_view_state_schemas()).unwrap());
        let slterm_workspace::ViewKey::Editor {
            project_id,
            document_id,
        } = key()
        else {
            unreachable!()
        };
        for key in [
            slterm_workspace::ViewKey::Markdown {
                project_id,
                document_id,
            },
            slterm_workspace::ViewKey::Diff {
                project_id,
                document_id,
            },
        ] {
            let store = EditorViewStateStore::new(registry.clone(), None);
            let lease = store.lease(&key);
            store.checkpoint(&lease, EditorViewState::new(3, 4));
            let final_states = store.final_states();
            assert_eq!(final_states.len(), 1);
            assert_eq!(final_states[0].0, key);
        }
    }

    #[test]
    fn editor_view_state_defaults_do_not_enter_final_writes() {
        let registry =
            std::sync::Arc::new(ViewStateRegistry::new(&document_view_state_schemas()).unwrap());
        let store = EditorViewStateStore::new(registry, None);
        assert!(store.final_states().is_empty());
        let lease = store.lease(&key());
        store.checkpoint(&lease, super::EditorViewState::new(3, 4));
        assert_eq!(store.final_states().len(), 1);
        store.final_submitted(&key());
        assert!(store.final_states().is_empty());
    }

    #[test]
    fn markdown_view_state_restore() {
        let slterm_workspace::ViewKey::Editor {
            project_id,
            document_id,
        } = key()
        else {
            unreachable!()
        };
        let key = slterm_workspace::ViewKey::Markdown {
            project_id,
            document_id,
        };
        let registry =
            std::sync::Arc::new(ViewStateRegistry::new(&document_view_state_schemas()).unwrap());
        let store = EditorViewStateStore::new(registry.clone(), None);
        let source = super::EditorViewState::new(8, 2).with_wrap(true);
        let document = slterm_document::MarkdownDocument::parse("# One\n\nfirst\n\nsecond\n");
        let reading = super::MarkdownReadingPosition::capture(2, 12, &document);
        let expected = super::MarkdownViewState::new(super::MarkdownMode::Source)
            .with_outline(true)
            .with_zoom(1_650)
            .with_source(source)
            .with_reading(reading);
        store.restore_markdown(key.clone(), expected.clone());
        assert_eq!(store.markdown_state(&key), expected);
        assert_eq!(store.state(&key), source);

        // 源码子视图只更新源态度量，不得覆盖 Markdown 的专属阅读状态。
        let lease = store.lease(&key);
        store.checkpoint(&lease, super::EditorViewState::new(9, 3));
        let current = store.markdown_state(&key);
        assert_eq!(current.mode(), super::MarkdownMode::Source);
        assert!(current.outline_visible());
        assert_eq!(current.zoom(), 1_650);
        assert_eq!(current.source(), super::EditorViewState::new(9, 3));
        assert_eq!(current.reading().resolve(&document).item_ix, 2);
        let final_states = store.final_states();
        assert_eq!(final_states.len(), 1);
        let json = final_states[0].1.to_canonical_json().unwrap();
        assert!(json.contains("\"mode\":\"source\""));
        for transient in ["selection", "search", "focus", "menu", "revision"] {
            assert!(!json.contains(transient));
        }
        for invalid in [
            serde_json::json!({"selection": [1, 2]}),
            serde_json::json!({"zoom": 100}),
            serde_json::json!({"mode": "unknown"}),
        ] {
            assert!(ViewState::new(&key, invalid, &registry).is_err());
        }
    }

    #[test]
    fn markdown_zoom_uses_bounded_decimal_steps() {
        let state = super::MarkdownViewState::default();
        assert_eq!(state.clone().zoom_in().zoom(), 1_100);
        assert_eq!(state.clone().zoom_in().zoom_out().zoom(), 1_000);
        let mut maximum = state.clone();
        for _ in 0..100 {
            maximum = maximum.zoom_in();
        }
        assert_eq!(maximum.zoom(), 4_000);
        let mut minimum = state;
        for _ in 0..100 {
            minimum = minimum.zoom_out();
        }
        assert_eq!(minimum.zoom(), 250);
    }

    #[test]
    fn markdown_reading_position_uses_stable_anchor_and_proportional_fallback() {
        let document = slterm_document::MarkdownDocument::parse(
            "# One\n\nfirst\n\nsecond\n\n# Two\n\nthird\n",
        );
        let position = super::MarkdownReadingPosition::capture(2, 17, &document);
        assert_eq!(position.resolve(&document).item_ix, 2);
        assert_eq!(f32::from(position.resolve(&document).offset_in_item), 17.0);

        let without_anchor =
            slterm_document::MarkdownDocument::parse("first\n\nsecond\n\n# Two\n\nthird\n");
        assert_eq!(position.resolve(&without_anchor).item_ix, 1);
    }
}
