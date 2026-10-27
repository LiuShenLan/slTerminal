//! JSON5 grammar 的 tree-sitter 绑定。

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_json5() -> *const ();
}

/// 当前运行时可直接接收的 grammar 入口。
// 安全性：入口由本 crate 编译的 JSON5 C grammar 提供，并具有静态语言 ABI。
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_json5) };

pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");
pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/highlights.scm");
