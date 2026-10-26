//! `slterm_agents` 提供目录、会话身份、命令构造与原生记录解析。
//!
//! 本 crate 不执行进程。

#![forbid(unsafe_code)]

mod identity;
pub use identity::AgentKind;
