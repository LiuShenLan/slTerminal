//! 崩溃快照的生产生命周期：编辑写入、保存/清脏对账与关闭删除。
//!
//! 快照正文仍由独立写线程与 `DocumentSnapshotStore` 独占；AppRoot 只持
//! 非阻塞 client，不在 GPUI 前台读写磁盘。

use std::collections::BTreeSet;

use slterm_document::{DocumentId, DocumentSession};
use slterm_workspace::{CloseIntent, CloseTarget};

use super::AppRoot;
use crate::persistence::close::PersistedCloseIntent;
use crate::persistence::document_snapshot::envelope::DocumentSnapshot;
use crate::persistence::document_snapshot::writer::DocumentSnapshotClient;

impl AppRoot {
    /// 测试注入或生产启动链装配的快照 client；无 client 时只保留内存正文。
    pub(super) fn install_snapshot_client(
        &mut self,
        client: Option<DocumentSnapshotClient>,
        run_id: String,
    ) {
        self.snapshot_client = client;
        self.snapshot_run_id = run_id;
    }

    /// 当前仍打开且脏的 Document 集合；配额驱逐不得触碰这些材料。
    fn snapshot_protected(&self) -> BTreeSet<DocumentId> {
        self.documents
            .ids()
            .filter(|id| {
                self.documents
                    .session(*id)
                    .is_some_and(|session| session.is_dirty())
            })
            .collect()
    }

    /// 编辑落定或保存清脏后同步快照：脏则排队最新正文，干净则排队删除。
    pub(super) fn sync_document_snapshot(&self, session: &DocumentSession) {
        let Some(client) = &self.snapshot_client else {
            return;
        };
        if session.is_dirty() {
            let protected = self.snapshot_protected();
            // 只读、Paged 或超限正文没有崩溃快照入口。
            if let Ok(snapshot) = DocumentSnapshot::capture(session, &self.snapshot_run_id)
                && let Err(error) = client.write(&snapshot, &protected)
            {
                log::warn!("Document 快照写请求失败: {error}");
            }
        } else if let Err(error) = client.reconcile(session) {
            log::warn!("Document 快照对账请求失败: {error}");
        }
    }

    /// 关闭 Save 阶段结算后，对所有参与关闭的 Document 重新对账快照。
    pub(super) fn sync_snapshots_for_close_intent(&self, intent: &CloseIntent<CloseTarget>) {
        if self.snapshot_client.is_none() {
            return;
        }
        for object in intent.objects() {
            let CloseTarget::Document(id) = object.id else {
                continue;
            };
            if let Some(session) = self.documents.session(id) {
                self.sync_document_snapshot(session);
            }
        }
    }

    /// 关闭状态已越过 StateCommitted 后删除最后引用 Document 的冗余快照。
    pub(super) fn delete_closed_document_snapshots(&self, persisted: &PersistedCloseIntent) {
        let Some(client) = &self.snapshot_client else {
            return;
        };
        for object in persisted.intent.objects() {
            let CloseTarget::Document(id) = object.id else {
                continue;
            };
            if let Err(error) = client.delete_closed(id, persisted) {
                log::warn!("Document 快照删除请求失败: {error}");
            }
        }
    }
}

impl AppRoot {
    /// 保存回执成功后按最新会话对账；失败保存保留脏正文与旧快照。
    pub(super) fn reconcile_saved_document(&self, document_id: DocumentId) {
        if let Some(session) = self.documents.session(document_id) {
            self.sync_document_snapshot(session);
        }
    }
}
