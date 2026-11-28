//! 历史隔离维护适配：在生产维护顺序中读取清单并清理已过期限的安全批次。
//!
//! 对账或清理失败只记录计数器与封闭错误分类，不改写启动结论；具体逐项
//! 事实由工作区表保留供导航隔离区重试。

use std::path::PathBuf;
use std::time::SystemTime;

use super::maintenance::HistoryQuarantineMaintainer;
use crate::persistence::workspace::WorkspaceDatabase;

pub(crate) struct FileHistoryQuarantineMaintainer {
    root: PathBuf,
}

impl FileHistoryQuarantineMaintainer {
    /// 隔离根固定为本地数据根下的应用管理目录。
    pub(crate) fn new(paths: &slterm_path::AppPaths) -> Self {
        Self {
            root: paths.local_root().join("history-quarantine"),
        }
    }
}

impl HistoryQuarantineMaintainer for FileHistoryQuarantineMaintainer {
    fn maintain(&self, store: &mut WorkspaceDatabase, now: SystemTime) {
        match store.reconcile_history_quarantine(&self.root, now) {
            Ok(report) => log::debug!(
                "history quarantine maintenance: reconciled={}, orphans={}, cleaned={}, failed={}",
                report.reconciled,
                report.orphans_recovered,
                report.cleaned,
                report.failed_items
            ),
            Err(error) => {
                // 隔离材料不得升格为启动阻断；失败事实留给下一次维护重试。
                log::warn!("history quarantine maintenance failed: {error}");
            }
        }
    }
}
