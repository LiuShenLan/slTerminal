//! slterm_completions —— 轻量独立补全匹配引擎(core 层,无本地边)。
//!
//! 血统:nushell nu-cli 补全框架抽离,经上游迁入改名。提供:
//!
//! * [`CandidateMatcher`](matcher::CandidateMatcher) 匹配引擎:prefix /
//!   substring / fuzzy(经 nucleo-matcher)三算法。
//! * 字面路径匹配:目录读取由 application 注入 [`DirectoryReader`](file::DirectoryReader),
//!   本 crate 不访问文件系统或进程环境。
//! * `Span`/`Suggestion`/`SemanticSuggestion`/`SuggestionKind` 候选值族。
//!
//! 迁入裁剪:color feature(LS_COLORS 样式)
//! 整砍;SSH/WSL 语义源变体与选项表整砍;unix 平台小分支收敛为 Windows
//! 形态;Posix 方言保留(B.21:Git Bash/MSYS 场景)。

pub mod command_context;
pub mod command_search;
pub mod edit;
pub mod file;
pub mod matcher;
pub mod options;
pub mod semantic;
pub mod span;
pub mod suggestion;

// ---- 再导出 -------------------------------------------------------------

pub use edit::{CompletionEdit, CompletionEditError};
pub use matcher::{CandidateMatcher, IgnoreCaseExt};
pub use options::{CompletionOptions, CompletionSort, InvalidMatchAlgorithm, MatchAlgorithm};
pub use span::Span;
pub use suggestion::{SemanticSuggestion, Suggestion, SuggestionKind};
