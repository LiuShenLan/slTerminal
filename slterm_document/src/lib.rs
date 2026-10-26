//! `slterm_document` 提供文本、编码、差分、语法与 Markdown 语义的纯核心能力。
//!
//! 本 crate 不持有 GPUI 类型，也不执行 I/O。

// 仅 grammar 审计测试需要适配旧 binding 的 C 入口；生产库仍禁止 unsafe。
#![cfg_attr(not(test), forbid(unsafe_code))]

#[cfg(test)]
mod grammar_audit;

mod identity;
pub use identity::{DocumentId, StableIdError};
