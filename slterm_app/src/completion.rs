//! 应用补全入口:管理共享来源和请求快照,不持有窗口、终端网格或执行器。
//!
//! 分层:`Session` 按 pane 持有来源缓存(Git/scripts),`Request`
//! 只收当前输入快照与已确认执行环境,后台计算;补全不上按键热路径。
//! 源收敛后只剩本机单态:远程连接池、WSL/SSH 作用域与文件整支不迁。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::directory_history::DirectoryHistory;
use crate::history::{HistoryScope, SltermHistory};
use crate::runtime_exec::PaneExecContext;
use crate::suggest::{self, Input, SuggestEnv, SuggestSources};
use slterm_completions::command_context::ShellSyntax;
use slterm_completions::semantic::{Context as SemanticContext, Source};

// 执行元数据：git_completion 复用同一 `Execution` 形态，故 crate 内可见。
pub(crate) mod metadata;
pub(crate) mod paths;
mod project_scripts;

pub(crate) use suggest::Candidates;

/// 历史只加载一份,避免多个 pane 在退出时互相覆盖;扫描期间不持有历史锁。
struct Shared {
    history: Mutex<SltermHistory>,
    directories: DirectoryHistory,
    commands: Arc<Mutex<Vec<String>>>,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        history: Mutex::new(SltermHistory::load()),
        directories: crate::directory_history::global(),
        commands: crate::suggest::commands_handle(),
    })
}

/// 协作式取消令牌:文件遍历、历史扫描与 Git 子进程共用同一形态。
#[derive(Clone, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// 来源缓存跟随 pane,具体来源及其适用条件不再由界面决定。
#[derive(Debug, Default)]
pub(crate) struct Session {
    git: Arc<crate::git_completion::Cache>,
    scripts: Arc<project_scripts::Cache>,
}

impl Session {
    /// 提交命令事件后调用:旧后台请求不得把旧仓库/清单快照回填缓存。
    pub(crate) fn invalidate(&self) {
        self.git.invalidate();
        self.scripts.invalidate();
    }

    /// 构造一次补全请求;方言是输入事实,未证明时只接受通用字面量。
    ///
    /// 参数数量由请求快照契约固定;打包成结构只会把同一组事实藏进
    /// 临时类型,不降低调用方理解成本。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_at(
        &self,
        cwd: String,
        env: SuggestEnv,
        line: String,
        cursor: usize,
        style: crate::suggest::CompletionStyle,
        execution: Option<&PaneExecContext>,
        syntax_override: Option<ShellSyntax>,
    ) -> Request {
        let syntax = syntax_override.unwrap_or_else(|| {
            execution
                .and_then(PaneExecContext::shell_program)
                .map(ShellSyntax::for_program)
                .unwrap_or(ShellSyntax::Literal)
        });
        let semantic = SemanticContext::parse(&line, cursor, syntax);
        // 只有需要本机 Git I/O 的请求才复制执行环境。
        let git = if semantic.as_ref().is_some_and(|context| {
            matches!(
                context.source,
                Source::Remotes
                    | Source::RemoteBranches
                    | Source::PushRefs { .. }
                    | Source::Branches { .. }
                    | Source::Revisions { .. }
                    | Source::RevisionsAndPaths { .. }
                    | Source::Tracking { .. }
            )
        }) {
            metadata::Execution::from_scope(env, execution)
                .map(|execution| (self.git.clone(), execution))
        } else {
            None
        };
        let scripts = if semantic.as_ref().is_some_and(|context| {
            matches!(context.source, Source::ProjectScripts | Source::Workspaces)
        }) {
            metadata::Execution::from_scope(env, execution)
                .map(|execution| (self.scripts.clone(), execution))
        } else {
            None
        };
        Request {
            cwd,
            env,
            line,
            cursor,
            style,
            git,
            semantic,
            scripts,
            syntax,
        }
    }
}

/// 后台只收到当前输入与已确认的执行环境,不借用整个终端状态。
pub(crate) struct Request {
    cwd: String,
    env: SuggestEnv,
    line: String,
    cursor: usize,
    style: crate::suggest::CompletionStyle,
    git: Option<(Arc<crate::git_completion::Cache>, metadata::Execution)>,
    semantic: Option<SemanticContext>,
    scripts: Option<(Arc<project_scripts::Cache>, metadata::Execution)>,
    syntax: ShellSyntax,
}

impl Request {
    pub(crate) fn calculate(mut self, cancellation: &Cancellation) -> Candidates {
        if cancellation.is_cancelled() || self.semantic.is_none() && self.cursor != self.line.len()
        {
            return Candidates::default();
        }
        if self
            .git
            .as_mut()
            .is_some_and(|(_, execution)| !execution.prepare())
        {
            self.git = None;
        }
        if self
            .scripts
            .as_mut()
            .is_some_and(|(_, execution)| !execution.prepare())
        {
            self.scripts = None;
        }
        let semantic = self
            .semantic
            .as_ref()
            .and_then(|context| match context.source {
                Source::Words(_) | Source::Options => Some(context.static_candidates()),
                Source::Remotes
                | Source::RemoteBranches
                | Source::PushRefs { .. }
                | Source::Branches { .. }
                | Source::Revisions { .. }
                | Source::RevisionsAndPaths { .. }
                | Source::Tracking { .. } => self.git.as_ref().and_then(|(cache, execution)| {
                    crate::git_completion::complete_available(
                        cache,
                        execution,
                        &self.cwd,
                        context,
                        &|| cancellation.is_cancelled(),
                    )
                    .or_else(|| Some(Vec::new()))
                }),
                Source::ProjectScripts | Source::Workspaces => {
                    self.scripts.as_ref().and_then(|(cache, _)| {
                        cache
                            .complete_in(&self.cwd, context, &|| cancellation.is_cancelled())
                            .or_else(|| Some(Vec::new()))
                    })
                }
                Source::None | Source::Paths { .. } => Some(Vec::new()),
            });
        if cancellation.is_cancelled() {
            return Candidates::default();
        }
        let Some(candidates) = semantic else {
            // 无语义上下文(未知命令/方言无法判定)时由通用引擎兜底;
            // 语义上下文存在但来源不可用(如 Git 缓存缺失)返回空候选,
            // 绝不回退本会话历史 —— 历史候选不会凭空复活。
            if self.semantic.is_none() {
                let sources = shared();
                return suggest::calculate(
                    &SuggestSources {
                        history: &sources.history,
                        directories: &sources.directories,
                        commands: &sources.commands,
                        enabled: true,
                        style: self.style,
                    },
                    &Input {
                        cwd: &self.cwd,
                        env: &self.env,
                        line: &self.line,
                    },
                    self.syntax,
                    &|| cancellation.is_cancelled(),
                );
            }
            return Candidates::default();
        };
        let mut candidates: Vec<slterm_completions::SemanticSuggestion> =
            candidates.into_iter().map(Into::into).collect();
        if let Some(context) = self.semantic.as_ref().filter(|context| {
            matches!(
                context.source,
                Source::Paths { .. } | Source::RevisionsAndPaths { .. }
            )
        }) {
            candidates.extend(paths::complete(
                &Input {
                    cwd: &self.cwd,
                    env: &self.env,
                    line: &self.line,
                },
                self.syntax,
                &shared().directories,
                self.style,
                Some(context),
                &|| cancellation.is_cancelled(),
            ));
        }
        // 历史只给仍然有效的语义候选提权,不能复活已删除的分支/脚本。
        if !candidates.is_empty() {
            let recent = {
                let history = shared()
                    .history
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                history
                    .hint_with_cancel(&self.env.history_scope(), &self.line, &|| {
                        cancellation.is_cancelled()
                    })
                    .map(|suffix| format!("{}{suffix}", self.line))
            };
            if let Some(recent) = recent
                && let Some(index) = candidates.iter().position(|candidate| {
                    let candidate = &candidate.suggestion;
                    self.line.get(..candidate.span.start).is_some_and(|head| {
                        recent.strip_prefix(head) == Some(candidate.value.as_str())
                    })
                })
            {
                candidates[..=index].rotate_right(1);
            }
        }
        if cancellation.is_cancelled() {
            return Candidates::default();
        }
        suggest::semantic_candidates_at(&self.line, self.cursor, self.style, candidates)
    }
}

/// 缓存键 = cwd + env + line + style + 命令代际。
pub(crate) fn cache_key(
    cwd: &str,
    env: &SuggestEnv,
    line: &str,
    style: crate::suggest::CompletionStyle,
) -> String {
    let commands = crate::suggest::commands_handle();
    let generation = commands.lock().map(|commands| commands.len()).unwrap_or(0);
    suggest::suggestion_key(&Input { cwd, env, line }, style, generation)
}

/// Enter 提交后写入历史(连续重复去重由历史池负责)。
pub(crate) fn record_command(scope: &HistoryScope, line: &str, cwd: &str) {
    shared()
        .history
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .record(scope, line, cwd);
}

/// 权威 cwd 上报/启动成功后记录目录 frecency。
pub(crate) fn record_directory(cwd: &str) {
    if !cwd.is_empty() {
        shared().directories.record(cwd);
    }
}

#[cfg(test)]
pub(crate) fn insert_history_for_test(cmd: &str) {
    shared()
        .history
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert_for_test(&HistoryScope::Local, cmd);
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use crate::suggest::{CompletionStyle, SltermCompletionKind};
    use slterm_completions::command_context::CommandContext;

    fn execution_for(program: &str, cwd: Option<&std::path::Path>) -> PaneExecContext {
        PaneExecContext::from_pty_options(&slterm_terminal::tty::Options {
            working_directory: cwd.map(std::path::Path::to_owned),
            shell: Some(slterm_terminal::tty::Shell::new(
                program.to_owned(),
                Vec::new(),
            )),
            ..Default::default()
        })
    }

    #[test]
    fn editor_cursor_requests_replace_only_the_active_token_and_keep_following_options() {
        use crate::git_completion::tests::repository;
        let repository = repository();
        let execution = execution_for("pwsh", Some(repository.path()));
        let session = Session::default();
        let line = "git switch \"feature/中-old\" --quiet";
        let cursor = "git switch \"feature/中".len();
        let result = session
            .request_at(
                repository.path().to_str().unwrap().into(),
                SuggestEnv::Local,
                line.into(),
                cursor,
                CompletionStyle::Popup,
                Some(&execution),
                None,
            )
            .calculate(&Cancellation::default());
        let item = result
            .completion_items
            .iter()
            .find(|item| item.insert.contains('文'))
            .unwrap();
        assert!(item.replace_after_chars > 0);
        let head: String = line[..cursor]
            .chars()
            .take(line[..cursor].chars().count() - item.replace_chars)
            .collect();
        let tail: String = line[cursor..]
            .chars()
            .skip(item.replace_after_chars)
            .collect();
        assert_eq!(
            format!("{head}{}{tail}", item.insert),
            "git switch \"feature/中文\" --quiet"
        );
    }

    #[test]
    fn path_completion_preserves_quotes_utf8_types_and_directory_roles_in_all_modes() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["repo 中文", "'quote", "quote", "a...b", "~"] {
            std::fs::create_dir(directory.path().join(name)).unwrap();
            std::fs::write(directory.path().join(name).join("child file.txt"), b"").unwrap();
        }
        std::fs::write(directory.path().join("repo other.txt"), b"").unwrap();
        let session = Session::default();
        for (program, syntax) in [
            ("sh", ShellSyntax::Posix),
            ("pwsh", ShellSyntax::PowerShell),
            ("cmd", ShellSyntax::Cmd),
        ] {
            let execution = execution_for(program, Some(directory.path()));
            for style in [
                CompletionStyle::Inline,
                CompletionStyle::Popup,
                CompletionStyle::Hybrid,
            ] {
                for line in [
                    "git -C repo",
                    "git -C \"repo",
                    "cat \"repo 中文/ch",
                    "cat 'a...b/ch",
                    "cat \"'quote/ch",
                    "cat '~/ch",
                ] {
                    if syntax == ShellSyntax::Cmd && line.contains('\'') {
                        continue;
                    }
                    let result = session
                        .request_at(
                            directory.path().to_str().unwrap().into(),
                            SuggestEnv::Local,
                            line.into(),
                            line.len(),
                            style,
                            Some(&execution),
                            None,
                        )
                        .calculate(&Cancellation::default());
                    let item = if style == CompletionStyle::Popup {
                        assert_eq!(result.completion_items.len(), 1, "{program}: {line}");
                        &result.completion_items[0]
                    } else {
                        result.suggestion_edit.as_ref().expect(line)
                    };
                    let accepted: String = line
                        .chars()
                        .take(line.chars().count() - item.replace_chars)
                        .chain(item.insert.chars())
                        .collect();
                    let decoded = CommandContext::parse(&accepted, accepted.len(), syntax).unwrap();
                    let target = directory.path().join(decoded.prefix());
                    assert!(target.exists(), "{program}: {accepted}");
                    assert_eq!(
                        item.kind,
                        if target.is_dir() {
                            SltermCompletionKind::Dir
                        } else {
                            SltermCompletionKind::File
                        }
                    );
                }
            }
        }
        let line = "git -C \"repo 中文\" checkout -- child";
        let result = session
            .request_at(
                directory.path().to_str().unwrap().into(),
                SuggestEnv::Local,
                line.into(),
                line.len(),
                CompletionStyle::Popup,
                None,
                Some(ShellSyntax::PowerShell),
            )
            .calculate(&Cancellation::default());
        assert_eq!(result.completion_items.len(), 1);
        assert_eq!(result.completion_items[0].kind, SltermCompletionKind::File);
    }

    #[test]
    fn checkout_completion_combines_branches_and_paths() {
        let repository = crate::git_completion::tests::repository();
        std::fs::write(repository.path().join("feature-file.txt"), b"").unwrap();
        let execution = execution_for("pwsh", Some(repository.path()));
        let session = Session::default();
        let result = session
            .request_at(
                repository.path().to_str().unwrap().into(),
                SuggestEnv::Local,
                "git checkout fe".into(),
                "git checkout fe".len(),
                CompletionStyle::Popup,
                Some(&execution),
                None,
            )
            .calculate(&Cancellation::default());
        assert_eq!(
            result.completion_items[0].kind,
            SltermCompletionKind::Command
        );
        assert!(
            result
                .completion_items
                .iter()
                .any(|item| item.kind == SltermCompletionKind::File)
        );
    }

    #[test]
    fn explicit_tracking_requests_keep_edits_scoped_and_do_not_execute() {
        use crate::git_completion::tests::{git, git_output, repository};
        let repository = repository();
        let cwd = repository.path().to_str().unwrap();
        git(
            repository.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/tracking",
            ],
        );
        git(
            repository.path(),
            &["update-ref", "refs/remotes/origin/topic", "HEAD"],
        );
        let session = Session::default();
        for program in ["sh", "pwsh", "cmd"] {
            let execution = execution_for(program, Some(repository.path()));
            for style in [
                CompletionStyle::Inline,
                CompletionStyle::Popup,
                CompletionStyle::Hybrid,
            ] {
                for (line, expected) in [
                    (
                        "git switch --track origin/to",
                        "git switch --track origin/topic",
                    ),
                    (
                        "git checkout -t \"origin/to\"",
                        "git checkout -t \"origin/topic\"",
                    ),
                    (
                        "git switch --no-track origin/to",
                        "git switch --no-track origin/topic",
                    ),
                    ("git switch --track=inh", "git switch --track=inherit"),
                ] {
                    let result = session
                        .request_at(
                            cwd.into(),
                            SuggestEnv::Local,
                            line.into(),
                            line.len(),
                            style,
                            Some(&execution),
                            None,
                        )
                        .calculate(&Cancellation::default());
                    let edit = if style == CompletionStyle::Popup {
                        assert_eq!(result.completion_items.len(), 1, "{program}: {line}");
                        &result.completion_items[0]
                    } else {
                        result.suggestion_edit.as_ref().expect(line)
                    };
                    let accepted: String = line
                        .chars()
                        .take(line.chars().count() - edit.replace_chars)
                        .chain(edit.insert.chars())
                        .collect();
                    assert_eq!(accepted, expected, "{program} {style:?}");
                }
            }
        }
        // 所有模式的请求只产生编辑;此处尚未显式提交命令,仓库应保持原状。
        assert_eq!(
            git_output(repository.path(), &["branch", "--show-current"]).trim(),
            "main"
        );
        assert!(git_output(repository.path(), &["for-each-ref", "refs/heads/topic"]).is_empty());
    }

    #[test]
    fn completion_requests_work_without_a_view_and_keep_repository_invalidation() {
        let repository = crate::git_completion::tests::repository();
        let cwd = repository.path().to_str().unwrap();
        let execution = execution_for("pwsh", Some(repository.path()));
        let session = Session::default();
        let query = |line: &str, cancellation: &Cancellation| {
            session
                .request_at(
                    cwd.into(),
                    SuggestEnv::Local,
                    line.into(),
                    line.len(),
                    CompletionStyle::Popup,
                    Some(&execution),
                    None,
                )
                .calculate(cancellation)
        };
        let active = Cancellation::default();
        assert_eq!(
            query("git switch feature/", &active).completion_items.len(),
            2
        );
        crate::git_completion::tests::git(repository.path(), &["branch", "feature/beta"]);
        // 慢速原生 Git 不能让失效断言依赖缓存 TTL。
        crate::git_completion::tests::retain_snapshot_for_test(&session.git);
        assert_eq!(
            query("git switch feature/", &active).completion_items.len(),
            2
        );
        session.invalidate();
        assert_eq!(
            query("git switch feature/", &active).completion_items.len(),
            3
        );

        let cancelled = Cancellation::default();
        cancelled.cancel();
        let result = query("git switch feature/", &cancelled);
        assert!(result.completion_items.is_empty());
        assert!(result.suggestion.is_empty());
        assert_eq!(
            query("git switch feature/", &active).completion_items.len(),
            3
        );
    }

    #[test]
    fn semantic_requests_share_modes_and_never_read_a_foreign_project() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("package.json"),
            r#"{"scripts":{"build:中文":"echo ok"}}"#,
        )
        .unwrap();
        let session = Session::default();
        let query = |line: &str, style| {
            session
                .request_at(
                    directory.path().to_str().unwrap().into(),
                    SuggestEnv::Local,
                    line.into(),
                    line.len(),
                    style,
                    None,
                    Some(ShellSyntax::PowerShell),
                )
                .calculate(&Cancellation::default())
        };
        for style in [
            CompletionStyle::Inline,
            CompletionStyle::Popup,
            CompletionStyle::Hybrid,
        ] {
            for (line, insert) in [
                ("npm run build:中", "文"),
                ("git sw", "itch"),
                ("git rebase --empty=k", "eep"),
            ] {
                let result = query(line, style);
                if style == CompletionStyle::Popup {
                    assert_eq!(result.completion_items[0].insert, insert);
                } else {
                    assert_eq!(result.suggestion, insert);
                }
            }
        }
        assert!(
            query("git switch -c ", CompletionStyle::Popup)
                .completion_items
                .is_empty()
        );
    }

    #[test]
    fn semantic_history_only_promotes_candidates_in_the_current_project() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().to_str().unwrap();
        let manifest = directory.path().join("package.json");
        std::fs::write(
            &manifest,
            r#"{"scripts":{"slterm-rank-a":"echo a","slterm-rank-z":"echo z"}}"#,
        )
        .unwrap();
        insert_history_for_test("npm run slterm-rank-z");
        let session = Session::default();
        let query = || {
            session
                .request_at(
                    cwd.into(),
                    SuggestEnv::Local,
                    "npm run slterm-rank-".into(),
                    "npm run slterm-rank-".len(),
                    CompletionStyle::Popup,
                    None,
                    Some(ShellSyntax::PowerShell),
                )
                .calculate(&Cancellation::default())
        };
        assert_eq!(query().completion_items[0].insert, "z");
        std::fs::write(&manifest, r#"{"scripts":{"slterm-rank-a":"echo a"}}"#).unwrap();
        session.invalidate();
        let result = query();
        assert_eq!(result.completion_items.len(), 1);
        assert_eq!(result.completion_items[0].insert, "a");
    }

    /// B31 防复发:语义来源为 Branches 但 Git 缓存缺失时返回空候选,
    /// 绝不回退本会话历史 —— 历史候选不会凭空复活。
    #[test]
    fn missing_git_cache_for_semantic_source_never_falls_back_to_history() {
        insert_history_for_test("git switch slterm-history-only");
        // 前缀非空:CommandContext 把最后一个词当编辑目标,才能命中 Branches。
        let line = "git switch s";
        let semantic = SemanticContext::parse(line, line.len(), ShellSyntax::Posix)
            .expect("git switch 应有语义上下文");
        assert!(matches!(semantic.source, Source::Branches { .. }));
        let request = Request {
            cwd: ".".into(),
            env: SuggestEnv::Local,
            line: line.into(),
            cursor: line.len(),
            style: CompletionStyle::Popup,
            git: None,
            semantic: Some(semantic),
            scripts: None,
            syntax: ShellSyntax::Posix,
        };
        let result = request.calculate(&Cancellation::default());
        assert!(
            result.completion_items.is_empty(),
            "Git 缓存缺失不得回收历史候选"
        );
        assert!(result.suggestion.is_empty());
    }

    #[test]
    fn command_position_and_file_sources_use_the_shared_engine() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("request-only.txt"), b"").unwrap();
        let session = Session::default();
        let result = session
            .request_at(
                directory.path().to_str().unwrap().into(),
                SuggestEnv::Local,
                "cat request-only".into(),
                "cat request-only".len(),
                CompletionStyle::Popup,
                None,
                Some(ShellSyntax::PowerShell),
            )
            .calculate(&Cancellation::default());
        assert!(
            result
                .completion_items
                .iter()
                .any(|item| item.insert == ".txt")
        );
    }

    /// 源收敛门禁:补全域不得残留远程/SSH 池符号。
    ///
    /// 用 `concat!` 拼出禁词,测试源自身不会命中;`include_str!` 固定读取
    /// 本片落盘源码,不依赖工作目录或文件系统扫描。
    #[test]
    fn source_convergence_has_no_remote_or_ssh_pool_symbols() {
        let banned: [&str; 10] = [
            concat!("Ssh", "Hosts"),
            concat!("Wsl", "Distributions"),
            concat!("History", "Scope::Ssh"),
            concat!("History", "Scope::Wsl"),
            concat!("Suggest", "Env::Ssh"),
            concat!("Suggest", "Env::Wsl"),
            concat!("remote", "_dirs"),
            concat!("ssh", "_session"),
            concat!("import_legacy", "_history"),
            concat!("unsaved", "_seed"),
        ];
        let sources = [
            ("completion.rs", include_str!("completion.rs")),
            ("suggest.rs", include_str!("suggest.rs")),
            ("history.rs", include_str!("history.rs")),
            ("directory_history.rs", include_str!("directory_history.rs")),
            ("git_completion.rs", include_str!("git_completion.rs")),
            ("completion/paths.rs", include_str!("completion/paths.rs")),
            (
                "completion/metadata.rs",
                include_str!("completion/metadata.rs"),
            ),
            (
                "completion/project_scripts.rs",
                include_str!("completion/project_scripts.rs"),
            ),
            (
                "completion/project_scripts/workspace.rs",
                include_str!("completion/project_scripts/workspace.rs"),
            ),
            (
                "slterm_completions/semantic.rs",
                include_str!("../../slterm_completions/src/semantic.rs"),
            ),
            (
                "slterm_completions/lib.rs",
                include_str!("../../slterm_completions/src/lib.rs"),
            ),
        ];
        for (name, source) in sources {
            for token in banned {
                assert!(!source.contains(token), "{name} still contains {token}");
            }
        }
    }

    #[test]
    fn cache_key_changes_with_the_input_snapshot_and_command_generation() {
        let base = cache_key(
            "D:/work",
            &SuggestEnv::Local,
            "cargo",
            CompletionStyle::Inline,
        );
        assert_eq!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Inline
            )
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo t",
                CompletionStyle::Inline
            )
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Popup
            )
        );
    }
}
