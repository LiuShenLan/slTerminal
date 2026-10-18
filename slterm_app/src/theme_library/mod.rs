//! 原生主题库边界。
//!
//! `document` 拥有版本化 JSON 信封,`store` 拥有自定义主题文件,`package`
//! 拥有可分享 ZIP 的有界检查。设置 crate 仍是运行时定义值的唯一权威;
//! 本层只做格式与持久化。

pub(crate) mod document;
pub(crate) mod package;
pub(crate) mod preferences;
mod store;

pub(crate) use document::{
    DocumentError, MAX_DOCUMENT_BYTES, ThemeDocument, builtin_documents, from_definition,
};
pub(crate) use store::{RevisionPrecondition, StoreError, ThemeLibrarySnapshot, ThemeLibraryStore};
