//! `slterm_document` 提供文本、编码、差分、语法与 Markdown 语义的纯核心能力。
//!
//! 本 crate 不持有 GPUI 类型，也不执行 I/O。

// 仅 grammar 审计测试需要适配旧 binding 的 C 入口；生产库仍禁止 unsafe。
#![cfg_attr(not(test), forbid(unsafe_code))]

#[cfg(test)]
mod grammar_audit;

mod diff;
#[cfg(test)]
mod diff_tests;
mod edit;
mod encoding;
mod external;
mod identity;
mod language;
mod markdown;
mod markdown_html;
mod markdown_svg;
mod paged;
mod recovery;
mod registry;
mod replace;
mod repository;
mod save;
mod search;
mod syntax;
#[cfg(test)]
pub use diff::compute_diff_with_deadline;
pub use diff::{
    DiffBaseline, DiffError, DiffInput, DiffLine, DiffPair, DiffPairKind, DiffSideContent,
    DiffSnapshot, DiffWhitespaceMode, compute_diff,
};
pub use edit::{
    EditApplyOutcome, EditOrigin, EditTransaction, EditTransactionError, MergePolicy, TextEdit,
};
pub use encoding::{
    BINARY_PROBE_BYTES, ContentUnavailable, DecodedText, DocumentContent, DocumentEncoding,
    EDITABLE_LIMIT_BYTES, EncodeError, LineEnding, NORMAL_EDIT_LIMIT_BYTES, SYNTAX_LIMIT_BYTES,
    TextEncoding,
};
pub use external::{
    DocumentConflict, DocumentExternalChange, DocumentExternalRequest, ExternalApplyOutcome,
    ExternalChangeError, ExternalConflictKind,
};
pub use identity::{DocumentId, StableIdError, TextStateId};
pub use language::{LanguageDescriptor, LanguageId, LanguageRegistry};
pub use markdown::{
    BlockId, MarkdownAlertKind, MarkdownAlignment, MarkdownAnchor, MarkdownAnchorKind,
    MarkdownBlock, MarkdownBlockKind, MarkdownCodeBlock, MarkdownDocument, MarkdownHeading,
    MarkdownInline, MarkdownInlineKind, MarkdownLink, MarkdownList, MarkdownListItem,
    MarkdownSearchMatch, MarkdownStructure, MarkdownStyle, MarkdownTable, MarkdownTableCell,
    MarkdownTableRow,
};
pub use markdown_html::{
    MarkdownHtmlAttribute, MarkdownHtmlElement, MarkdownHtmlFragment, MarkdownHtmlNode,
    MarkdownHtmlTag,
};
pub use markdown_svg::{SanitizedSvg, SvgError};
pub use paged::{
    DECODED_PAGE_CACHE_LIMIT_BYTES, DocumentBody, DocumentPageCache, PAGE_SOURCE_BYTES, PageIndex,
    PageIndexError, PageKey, PagedLineIndex, PagedSource, SourceByteRange,
};
pub use recovery::{DocumentRecovery, ReauthorizationError};
pub use registry::{
    DocumentMetadata, DocumentMetadataError, DocumentOpenOutcome, DocumentRegistry,
    DocumentRegistryError, DocumentSession,
};
pub use replace::{DocumentReplaceRequest, DocumentReplacement, ReplacePlanError};
pub use repository::{
    CommitId, DiffBinding, InvalidCommitId, InvalidRepoPath, RepoPath, RepositoryBinding,
    RepositoryBindingUnavailable,
};
pub use save::{DocumentSaveReceipt, DocumentSaveRequest, SaveTransactionError};
pub use search::{
    DocumentSearchResult, DocumentTextRequest, SearchError, SearchMatch, TextChunk, TextChunks,
};
pub use syntax::{
    SyntaxHighlighter, SyntaxSpan, SyntaxTree, SyntaxWorker, SyntaxWorkerError, syntax_input_edit,
    syntax_transaction_input_edits,
};
