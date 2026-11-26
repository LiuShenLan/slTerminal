//! 文件授权代次：项目或 Agent 根撤销时推进，旧计划与在途保存据此失效。
//!
//! 进程级单源：AppRoot、后台保存与文件操作计划共用同一计数器；根撤销只
//! 推进代次，不复制授权状态。

use std::sync::atomic::{AtomicU64, Ordering};

static AUTHORIZATION_GENERATION: AtomicU64 = AtomicU64::new(1);

/// 当前有效代次；计划创建与执行前都必须读取最新值。
pub(crate) fn current_generation() -> u64 {
    AUTHORIZATION_GENERATION.load(Ordering::SeqCst)
}

/// 撤销或路径身份变化后推进代次，使旧计划与在途保存失败。
pub(crate) fn advance_generation() -> u64 {
    AUTHORIZATION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}
