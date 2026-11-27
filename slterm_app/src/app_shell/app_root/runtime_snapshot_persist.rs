//! Runtime 写操作回执与项目快照事务提交对账。
//!
//! 前台内存提交不再等于成功：同步写意图与异步打开／Agent 注入完成后，
//! 必须等唯一写线程提交项目快照事务才回执；事务失败或断连回结构化失败。

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};

use gpui::Context;
use slterm_runtime_protocol::JsonRpcError;

use super::AppRoot;
use super::runtime_snapshot::{
    RuntimeMutationCompletion, RuntimeMutationDispatch, closing_error, not_ready_error,
};
use crate::app_shell::shell_events::RuntimeMutationReply;
use crate::persistence::project::ProjectStoreError;

/// 等待事务提交的回执；异步变更先等自身结果，再等快照提交。
pub(super) enum PendingSnapshotReply {
    AwaitCommit {
        receipt: Receiver<Result<(), ProjectStoreError>>,
        reply: RuntimeMutationReply,
        completion: RuntimeMutationCompletion,
    },
    AwaitMutation {
        receipt: Receiver<Result<RuntimeMutationReply, JsonRpcError>>,
        completion: RuntimeMutationCompletion,
    },
}

impl AppRoot {
    /// 同步写意图：内存提交成功后，把成功回执推迟到项目快照事务提交。
    pub(super) fn defer_until_snapshot_persisted(
        &mut self,
        reply: RuntimeMutationReply,
        completion: &SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
        _cx: &mut Context<'_, Self>,
    ) -> RuntimeMutationDispatch {
        if self.project_snapshot_recovery_failed || self.project_writer.is_none() {
            // 无 writer 仅存在内存测试根；生产启动链不允许该形态。
            return RuntimeMutationDispatch::Reply(Ok(reply));
        }
        match self
            .project_writer
            .as_ref()
            .expect("writer 已在上面检查")
            .commit_project_snapshot(&self.snapshot)
        {
            Ok(receipt) => {
                self.pending_snapshot_replies
                    .push(PendingSnapshotReply::AwaitCommit {
                        receipt,
                        reply,
                        completion: RuntimeMutationCompletion::new(completion.clone()),
                    });
                RuntimeMutationDispatch::Deferred
            }
            Err(error) => {
                log::warn!("runtime 快照提交入队失败: {error}");
                RuntimeMutationDispatch::Reply(Err(not_ready_error(
                    "project snapshot was not persisted",
                )))
            }
        }
    }

    /// 异步变更（打开文件、Agent 注入）的结果通道；结果到达后再等快照事务。
    pub(super) fn begin_deferred_snapshot_reply(
        &mut self,
        completion: &SyncSender<Result<RuntimeMutationReply, JsonRpcError>>,
    ) -> SyncSender<Result<RuntimeMutationReply, JsonRpcError>> {
        let (result, receipt) = std::sync::mpsc::sync_channel(1);
        self.pending_snapshot_replies
            .push(PendingSnapshotReply::AwaitMutation {
                receipt,
                completion: RuntimeMutationCompletion::new(completion.clone()),
            });
        result
    }

    /// 事件泵轮询：异步结果到达后发起事务提交，事务回执到达后回复调用方。
    pub(super) fn poll_snapshot_replies(&mut self, _cx: &mut Context<'_, Self>) {
        if self.pending_snapshot_replies.is_empty() {
            return;
        }
        let mut pending = std::mem::take(&mut self.pending_snapshot_replies);
        let mut remaining = Vec::with_capacity(pending.len());
        for waiter in pending.drain(..) {
            match waiter {
                PendingSnapshotReply::AwaitMutation {
                    receipt,
                    mut completion,
                } => match receipt.try_recv() {
                    Ok(Ok(RuntimeMutationReply::Done(Ok(())))) => {
                        if self.project_snapshot_recovery_failed || self.project_writer.is_none() {
                            completion.send(RuntimeMutationReply::Done(Ok(())));
                            continue;
                        }
                        match self
                            .project_writer
                            .as_ref()
                            .expect("writer 已检查")
                            .commit_project_snapshot(&self.snapshot)
                        {
                            Ok(receipt) => {
                                remaining.push(PendingSnapshotReply::AwaitCommit {
                                    receipt,
                                    reply: RuntimeMutationReply::Done(Ok(())),
                                    completion,
                                });
                            }
                            Err(error) => {
                                log::warn!("runtime 快照提交入队失败: {error}");
                                completion.send_error(not_ready_error(
                                    "project snapshot was not persisted",
                                ));
                            }
                        }
                    }
                    Ok(Ok(reply)) => completion.send(reply),
                    Ok(Err(error)) => completion.send_error(error),
                    Err(TryRecvError::Empty) => {
                        remaining.push(PendingSnapshotReply::AwaitMutation {
                            receipt,
                            completion,
                        });
                    }
                    Err(TryRecvError::Disconnected) => {
                        completion.send_error(not_ready_error("runtime mutation dropped"));
                    }
                },
                PendingSnapshotReply::AwaitCommit {
                    receipt,
                    reply,
                    mut completion,
                } => match receipt.try_recv() {
                    Ok(Ok(())) => completion.send(reply),
                    Ok(Err(error)) => {
                        log::warn!("runtime 项目快照提交失败: {error}");
                        completion.send_error(not_ready_error("project snapshot commit failed"));
                    }
                    Err(TryRecvError::Empty) => {
                        remaining.push(PendingSnapshotReply::AwaitCommit {
                            receipt,
                            reply,
                            completion,
                        });
                    }
                    Err(TryRecvError::Disconnected) => {
                        completion.send_error(not_ready_error("project snapshot writer stopped"));
                    }
                },
            }
        }
        self.pending_snapshot_replies = remaining;
    }

    /// 关闭完成后在途回执统一收敛为 Closing；guard 析构也只会重复尝试。
    pub(super) fn fail_pending_snapshot_replies(&mut self) {
        for waiter in std::mem::take(&mut self.pending_snapshot_replies) {
            match waiter {
                PendingSnapshotReply::AwaitCommit { mut completion, .. }
                | PendingSnapshotReply::AwaitMutation { mut completion, .. } => {
                    completion.send_error(closing_error());
                }
            }
        }
    }
}
