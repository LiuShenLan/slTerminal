//! slTerminal Runtime v2 线缆协议与操作状态语义边界。
//!
//! 当前不公开线缆 DTO 或 schema；实现协议时不得复活旧 Runtime v1 方法、
//! 独立 handover schema 或旧端点材料。

mod operation;
pub use operation::{InvalidOperationTransition, OperationStatus};
