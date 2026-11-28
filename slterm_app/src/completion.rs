//! 应用补全入口:管理共享来源和请求快照,不持有窗口、终端网格或执行器。
//!
//! 分层:`Session` 按 pane 持有来源缓存(Git/scripts),`Request`
//! 只收当前输入快照与已确认执行环境,后台计算;补全不上按键热路径。
//! 源收敛后只剩本机单态:远程连接池、WSL/SSH 作用域与文件整支不迁。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::suggest::{self, Input, SuggestEnv, SuggestSources};
use slterm_completions::command_context::ShellSyntax;
use slterm_completions::semantic::{Context as SemanticContext, Source};

// PTY 启动环境冻结在 `PaneExecContext`，后台 Git 只读面复用同一事实。
pub(crate) mod metadata;
pub(crate) mod paths;
mod project_scripts;

pub(crate) use metadata::PaneExecContext;
pub(crate) use suggest::Candidates;

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

/// Git 补全来源。生产唯一使用共享 facade；测试可注入确定失败的来源，
/// 以验证来源失败时绝不回退本会话历史。
#[derive(Clone)]
enum GitSource {
    Facade(Arc<crate::git::GitFacade>),
    /// 测试条件：语义来源存在但读取确定失败，等价于
    /// `complete_available` 返回 `None`。
    #[cfg(test)]
    Failing,
}

impl GitSource {
    fn invalidate(&self) {
        match self {
            GitSource::Facade(git) => git.invalidate(),
            #[cfg(test)]
            GitSource::Failing => {}
        }
    }
}

/// 来源缓存跟随 pane；学习数据由同一服务的不可变快照提供。
pub(crate) struct Session {
    git: GitSource,
    scripts: Arc<project_scripts::Cache>,
    learning: crate::learning::LearningService,
    /// PATH 命令表句柄；生产由进程级后台发现填充，测试构建固定为空表。
    commands: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(crate::learning::LearningService::memory_only())
    }
}

impl Session {
    pub(crate) fn new(learning: crate::learning::LearningService) -> Self {
        Self {
            git: GitSource::Facade(Arc::default()),
            scripts: Arc::default(),
            learning,
            commands: suggest::commands_handle(),
        }
    }

    /// 测试注入确定命令表，替换进程级发现。
    #[cfg(test)]
    pub(crate) fn with_commands_for_test(
        learning: crate::learning::LearningService,
        commands: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            git: GitSource::Facade(Arc::default()),
            scripts: Arc::default(),
            learning,
            commands,
        }
    }

    pub(crate) fn command_generation(&self) -> usize {
        self.commands
            .lock()
            .map(|commands| commands.len())
            .unwrap_or(0)
    }

    pub(crate) fn learning_generation(&self) -> i64 {
        self.learning.snapshot().generation
    }

    pub(crate) fn record_command(&self, shell_program: &str, line: &str, cwd: &str) {
        if let Err(error) = self.learning.record_command(shell_program, line, Some(cwd)) {
            log::warn!("learning command enqueue failed: {error}");
        }
    }

    pub(crate) fn record_directory(&self, cwd: &str) {
        if let Err(error) = self.learning.record_directory(cwd) {
            log::warn!("learning directory enqueue failed: {error}");
        }
    }

    #[cfg(test)]
    pub(crate) fn command_known_for_test(&self, command: &str) -> bool {
        self.learning
            .snapshot()
            .commands
            .contains_for_test(&crate::history::HistoryScope::Local, command)
    }

    #[cfg(test)]
    pub(crate) fn directory_known_for_test(&self, path: &str) -> bool {
        self.learning
            .snapshot()
            .directories
            .score(std::path::Path::new(path))
            .is_some()
    }

    #[cfg(test)]
    pub(crate) fn insert_history_for_test(&self, command: &str) {
        self.learning
            .record_command("pwsh.exe", command, None)
            .unwrap();
    }

    /// 提交命令事件后调用:旧后台请求不得把旧仓库/清单快照回填缓存。
    pub(crate) fn invalidate(&self) {
        self.git.invalidate();
        self.scripts.invalidate();
    }

    /// 测试条件：把 Git 来源替换为确定失败，确保用例不访问任何真实仓库。
    #[cfg(test)]
    pub(crate) fn use_failing_git_source_for_test(&mut self) {
        self.git = GitSource::Failing;
    }

    #[cfg(test)]
    pub(crate) fn git_facade_for_test(&self) -> &crate::git::GitFacade {
        match &self.git {
            GitSource::Facade(git) => git,
            GitSource::Failing => panic!("测试未配置真实 Git facade"),
        }
    }

    /// 测试夹具使用零代次；生产必须经过 `request_at_revision`。
    #[cfg(test)]
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
        self.request_at_revision(cwd, env, line, cursor, style, execution, syntax_override, 0)
    }

    /// 构造一次补全请求;方言是输入事实,未证明时只接受通用字面量。
    ///
    /// 参数数量由请求快照契约固定;打包成结构只会把同一组事实藏进
    /// 临时类型,不降低调用方理解成本。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_at_revision(
        &self,
        cwd: String,
        env: SuggestEnv,
        line: String,
        cursor: usize,
        style: crate::suggest::CompletionStyle,
        execution: Option<&PaneExecContext>,
        syntax_override: Option<ShellSyntax>,
        input_revision: u64,
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
            let execution = execution.cloned().unwrap_or_else(|| {
                PaneExecContext::from_pty_options(&slterm_terminal::tty::Options::default())
            });
            Some((self.git.clone(), execution))
        } else {
            None
        };
        let scripts = if semantic.as_ref().is_some_and(|context| {
            matches!(context.source, Source::ProjectScripts | Source::Workspaces)
        }) {
            Some(self.scripts.clone())
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
            learning: self.learning.snapshot(),
            commands: Arc::clone(&self.commands),
            input_revision,
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
    git: Option<(GitSource, PaneExecContext)>,
    semantic: Option<SemanticContext>,
    scripts: Option<Arc<project_scripts::Cache>>,
    syntax: ShellSyntax,
    learning: crate::learning::LearningDataSnapshot,
    commands: Arc<std::sync::Mutex<Vec<String>>>,
    input_revision: u64,
}

impl Request {
    pub(crate) fn calculate(self, cancellation: &Cancellation) -> Candidates {
        if cancellation.is_cancelled() || self.semantic.is_none() && self.cursor != self.line.len()
        {
            return Candidates::default();
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
                | Source::Tracking { .. } => {
                    self.git.as_ref().and_then(|(source, execution)| {
                        let candidates = match source {
                            GitSource::Facade(facade) => crate::git_completion::complete_available(
                                facade,
                                execution,
                                &self.cwd,
                                context,
                                &|| cancellation.is_cancelled(),
                            ),
                            // 条件注入的来源失败；与真实 `complete_available`
                            // 返回 None 一样被收敛为空候选，不得回退历史。
                            #[cfg(test)]
                            GitSource::Failing => None,
                        };
                        candidates.or_else(|| Some(Vec::new()))
                    })
                }
                Source::ProjectScripts | Source::Workspaces => {
                    self.scripts.as_ref().and_then(|cache| {
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
                return suggest::calculate(
                    &SuggestSources {
                        history: &self.learning.commands,
                        directories: &self.learning.directories,
                        commands: &self.commands,
                        enabled: true,
                        style: self.style,
                    },
                    &Input {
                        cwd: &self.cwd,
                        env: &self.env,
                        line: &self.line,
                    },
                    self.syntax,
                    self.input_revision,
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
                &self.learning.directories,
                self.style,
                Some(context),
                &|| cancellation.is_cancelled(),
            ));
        }
        // 历史只给仍然有效的语义候选提权,不能复活已删除的分支/脚本。
        if !candidates.is_empty() {
            let recent = self
                .learning
                .commands
                .hint_with_cancel(&self.env.history_scope(), &self.line, &|| {
                    cancellation.is_cancelled()
                })
                .map(|suffix| format!("{}{suffix}", self.line));
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
        suggest::semantic_candidates_at(
            &self.line,
            self.cursor,
            self.input_revision,
            self.style,
            candidates,
        )
    }
}

/// 缓存键 = cwd + env + line + style + 命令与学习代际。
///
/// 命令代际由调用方从 Session 的注入句柄取得；键计算不触发进程级发现。
pub(crate) fn cache_key(
    cwd: &str,
    env: &SuggestEnv,
    line: &str,
    style: crate::suggest::CompletionStyle,
    learning_generation: i64,
    command_generation: usize,
) -> String {
    format!(
        "{}\0learning={learning_generation}",
        suggest::suggestion_key(&Input { cwd, env, line }, style, command_generation)
    )
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
            .find(|item| item.edit.insert().contains('文'))
            .unwrap();
        assert!(item.edit.span().end > cursor);
        assert_eq!(
            item.edit.apply(line, 0).unwrap(),
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
                    let accepted = item.edit.apply(line, 0).unwrap();
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
                    let accepted = edit.edit.apply(line, 0).unwrap();
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
        crate::git_completion::tests::retain_snapshot_for_test(session.git_facade_for_test());
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
                    assert_eq!(result.completion_items[0].edit.insert(), insert);
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
        let session = Session::default();
        session.insert_history_for_test("npm run slterm-rank-z");
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
        assert_eq!(query().completion_items[0].edit.insert(), "z");
        std::fs::write(&manifest, r#"{"scripts":{"slterm-rank-a":"echo a"}}"#).unwrap();
        session.invalidate();
        let result = query();
        assert_eq!(result.completion_items.len(), 1);
        assert_eq!(result.completion_items[0].edit.insert(), "a");
    }

    /// 防复发:语义来源为 Branches 但 Git 来源确定失败时返回空候选,
    /// 绝不回退本会话历史 —— 历史候选不会凭空复活。
    ///
    /// 来源失败经注入条件固定，不解析当前工作目录所在的任何真实仓库。
    #[test]
    fn missing_git_cache_for_semantic_source_never_falls_back_to_history() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        session.use_failing_git_source_for_test();
        session.insert_history_for_test("git switch slterm-history-only");
        // 前缀非空:CommandContext 把最后一个词当编辑目标,才能命中 Branches。
        let line = "git switch s";
        let semantic = SemanticContext::parse(line, line.len(), ShellSyntax::Posix)
            .expect("git switch 应有语义上下文");
        assert!(matches!(semantic.source, Source::Branches { .. }));
        let result = session
            .request_at(
                directory.path().to_str().unwrap().into(),
                SuggestEnv::Local,
                line.into(),
                line.len(),
                CompletionStyle::Popup,
                None,
                Some(ShellSyntax::Posix),
            )
            .calculate(&Cancellation::default());
        assert!(
            result.completion_items.is_empty(),
            "Git 来源失败不得回收历史候选"
        );
        assert!(result.suggestion.is_empty());
    }

    /// Git 来源失败对全部本地 refs/remotes/revision 语义来源同形：即使本会话
    /// 历史存在同前缀命令，也只返回空候选，不回退历史。
    #[test]
    fn failing_git_source_yields_empty_candidates_for_every_semantic_source() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().to_str().unwrap().to_owned();
        let mut session = Session::default();
        session.use_failing_git_source_for_test();
        for history in [
            "git switch slterm-history-only",
            "git push origin slterm-history-only",
            "git rebase slterm-history-only",
        ] {
            session.insert_history_for_test(history);
        }
        for line in ["git switch s", "git push origin s", "git rebase s"] {
            let semantic = SemanticContext::parse(line, line.len(), ShellSyntax::Posix)
                .expect("Git 行应有语义上下文");
            assert!(
                matches!(
                    semantic.source,
                    Source::Branches { .. } | Source::PushRefs { .. } | Source::Revisions { .. }
                ),
                "用例必须命中需要 Git 的语义来源: {line}"
            );
            let result = session
                .request_at(
                    cwd.clone(),
                    SuggestEnv::Local,
                    line.into(),
                    line.len(),
                    CompletionStyle::Popup,
                    None,
                    Some(ShellSyntax::Posix),
                )
                .calculate(&Cancellation::default());
            assert!(
                result.completion_items.is_empty(),
                "Git 来源失败不得回退历史: {line}"
            );
            assert!(result.suggestion.is_empty(), "不得生成 ghost: {line}");
        }
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
                .any(|item| item.edit.insert() == ".txt")
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
            ("git/completion.rs", include_str!("git/completion.rs")),
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

    /// 命令候选与缓存代际只来自 Session 注入表,不读进程环境。
    #[test]
    fn injected_command_table_drives_candidates_and_generation() {
        let commands = Arc::new(std::sync::Mutex::new(vec![
            "slterm-injected-tool.exe".to_owned(),
        ]));
        let session = Session::with_commands_for_test(
            crate::learning::LearningService::memory_only(),
            Arc::clone(&commands),
        );
        assert_eq!(session.command_generation(), 1);
        let result = session
            .request_at(
                "D:/work".into(),
                SuggestEnv::Local,
                "slterm-injected-t".into(),
                "slterm-injected-t".len(),
                CompletionStyle::Inline,
                None,
                None,
            )
            .calculate(&Cancellation::default());
        assert_eq!(result.suggestion, "ool.exe");
        assert!(
            result.suggestion_edit.is_some(),
            "命令 ghost 必须携带编辑合同"
        );
    }

    #[test]
    fn cache_key_changes_with_the_input_snapshot_and_command_generation() {
        let base = cache_key(
            "D:/work",
            &SuggestEnv::Local,
            "cargo",
            CompletionStyle::Inline,
            0,
            0,
        );
        assert_eq!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Inline,
                0,
                0,
            )
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo t",
                CompletionStyle::Inline,
                0,
                0,
            )
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Popup,
                0,
                0,
            )
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Inline,
                1,
                0,
            ),
            "清空学习必须使旧候选键失效"
        );
        assert_ne!(
            base,
            cache_key(
                "D:/work",
                &SuggestEnv::Local,
                "cargo",
                CompletionStyle::Inline,
                0,
                1,
            ),
            "命令表代际变化必须使旧候选键失效"
        );
    }
}
