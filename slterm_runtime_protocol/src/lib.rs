//! slTerminal Runtime v2 线缆协议与操作状态语义边界。
//!
//! 公开当前实现的线缆 DTO；`schema` feature 提供对应生成器。
//! 不复活旧 Runtime v1 方法、独立 handover schema 或旧端点材料。

mod operation;
mod protocol;
#[cfg(test)]
mod protocol_tests;
#[cfg(feature = "schema")]
pub mod schema;
pub use operation::{InvalidOperationTransition, OperationStatus};
pub use protocol::*;
