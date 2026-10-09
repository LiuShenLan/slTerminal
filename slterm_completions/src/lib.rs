//! slterm_completions —— 轻量独立补全匹配引擎(core 层,无本地边)。
//!
//! 血统:nushell nu-cli 补全框架抽离,经上游迁入改名。提供:
//!
//! * [`CandidateMatcher`](matcher::CandidateMatcher) 匹配引擎:prefix /
//!   substring / fuzzy(经 nucleo-matcher)三算法。
//! * [`Completer`](completer::Completer) 可插拔来源合同。
//! * 内建来源:文件、目录、静态字符串表。
//! * `Span`/`Suggestion`/`SemanticSuggestion`/`SuggestionKind` 候选值族。
//!
//! 迁入裁剪(依据设计文档 08 篇):color feature(LS_COLORS 样式)
//! 整砍;SSH/WSL 语义源变体与选项表整砍;unix 平台小分支收敛为 Windows
//! 形态;Posix 方言保留(B.21:Git Bash/MSYS 场景)。

pub mod command_context;
pub mod command_search;
pub mod completer;
pub mod file;
pub mod matcher;
pub mod options;
pub mod semantic;
pub mod span;
pub mod suggestion;

mod directory;
mod static_completion;

// ---- 再导出 -------------------------------------------------------------

pub use completer::Completer;
pub use directory::DirectoryCompletion;
pub use matcher::{CandidateMatcher, IgnoreCaseExt};
pub use options::{CompletionOptions, CompletionSort, InvalidMatchAlgorithm, MatchAlgorithm};
pub use span::Span;
pub use static_completion::StaticCompletion;
pub use suggestion::{SemanticSuggestion, Suggestion, SuggestionKind};
