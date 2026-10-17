//! 全机进程快照:hook 事件身份路由(祖先链解析)的底层事实源。
//!
//! OS 读取全部住在本文件(含 windows.rs 子件);树遍历与 PID 复用规则归
//! `crate::process_tree`。Windows-only:上游的 unix `ps` 臂随 unix 支持整支
//! 裁剪,不撒 cfg(unix 没有 Toolhelp/NtQuery 的等价公共面,分支留不住)。
//!
//! 关窗确认(close-warning)族快照消费归后续 M 步随关窗链路落地。

#[path = "process_snapshot/windows.rs"]
mod windows;

/// 一次快照时刻的原生进程身份。创建时间为零表示平台给不出该值;消费方
/// 必须保留这条边(不许当成「与父同时创建」)。
pub(crate) struct ProcessRow {
    pub(crate) pid: u32,
    pub(crate) parent: u32,
    pub(crate) executable: String,
    pub(crate) created: u64,
}

pub(crate) fn snapshot() -> Result<Vec<ProcessRow>, String> {
    windows::snapshot()
}
