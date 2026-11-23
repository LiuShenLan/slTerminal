//! Document 的真实读盘、统一保存与 Git 行标端口。
//!
//! 源码编辑归 `text_view`，Markdown 双态归 `markdown`；本模块不再持有第二份正文。

pub(crate) mod document;
pub(crate) mod gutter;
