//! GitFacade 的本地 refs/remotes 只读面。

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slterm_terminal::tty::Environment;

use crate::platform::process_output::{ProcessOutputError, read_cancellable};

mod tracking;

pub(crate) use tracking::inferred_name;

const TOTAL_TIMEOUT: Duration = Duration::from_secs(3);
const OUTPUT_LIMIT: usize = 1024 * 1024;
const SNAPSHOT_TTL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub(crate) struct Reference {
    pub full_name: String,
    pub short_name: String,
    pub busy: bool,
    pub current: bool,
    pub commit: bool,
    pub symbolic: bool,
    pub direct_tracking: bool,
}

/// 一次完整 refs 读取及其纯本地跟踪推导。
#[derive(Debug, Default)]
pub(crate) struct GitRefs {
    pub references: Vec<Reference>,
    pub guesses: Vec<String>,
    pub remote_branches: BTreeMap<String, Vec<String>>,
    pub guess_enabled: bool,
    pub local_branches: HashSet<String>,
}

/// 进程边界事实原样分类，不由补全消费者猜测 IO 错误文本。
#[derive(Debug)]
pub(crate) enum GitReadError {
    Spawn(io::Error),
    Io(io::Error),
    Cancelled,
    Timeout,
    OutputLimit { limit: usize },
    Exit(ExitStatus),
    InvalidUtf8,
}

impl From<ProcessOutputError> for GitReadError {
    fn from(error: ProcessOutputError) -> Self {
        match error {
            ProcessOutputError::Spawn(error) => Self::Spawn(error),
            ProcessOutputError::Io(error) => Self::Io(error),
            ProcessOutputError::Cancelled => Self::Cancelled,
            ProcessOutputError::Timeout => Self::Timeout,
            ProcessOutputError::OutputLimit { limit } => Self::OutputLimit { limit },
            ProcessOutputError::Exit(status) => Self::Exit(status),
        }
    }
}

#[derive(Clone, Debug)]
enum CachedRead {
    Refs(Arc<GitRefs>),
    Remotes(Arc<Vec<String>>),
}

#[derive(Debug)]
struct Snapshot {
    key: String,
    value: CachedRead,
    fetched: Instant,
}

/// 每个补全会话一个 facade；缓存键不包含可变环境，因为 pane 环境在创建时冻结。
#[derive(Debug, Default)]
pub(crate) struct GitFacade {
    cache: Mutex<(u64, Option<Snapshot>)>,
}

impl GitFacade {
    pub(crate) fn invalidate(&self) {
        let mut state = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = state.0.wrapping_add(1);
        state.1 = None;
    }

    pub(crate) fn refs(
        &self,
        cwd: &str,
        directories: &[String],
        environment: &Environment,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Arc<GitRefs>, GitReadError> {
        let key = cache_key("refs", cwd, directories);
        let (generation, cached) = self.cached(&key);
        if let Some(CachedRead::Refs(snapshot)) = cached {
            return Ok(snapshot);
        }
        let snapshot = Arc::new(query_refs(
            &ReadRequest::new(cwd, directories, environment),
            cancelled,
            &read_cancellable,
        )?);
        self.store(
            &key,
            generation,
            CachedRead::Refs(snapshot.clone()),
            cancelled,
        );
        Ok(snapshot)
    }

    pub(crate) fn remotes(
        &self,
        cwd: &str,
        directories: &[String],
        environment: &Environment,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Arc<Vec<String>>, GitReadError> {
        let key = cache_key("remotes", cwd, directories);
        let (generation, cached) = self.cached(&key);
        if let Some(CachedRead::Remotes(remotes)) = cached {
            return Ok(remotes);
        }
        let remotes = Arc::new(query_remotes(
            &ReadRequest::new(cwd, directories, environment),
            cancelled,
            &read_cancellable,
        )?);
        self.store(
            &key,
            generation,
            CachedRead::Remotes(remotes.clone()),
            cancelled,
        );
        Ok(remotes)
    }

    fn cached(&self, key: &str) -> (u64, Option<CachedRead>) {
        let state = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            state.0,
            state
                .1
                .as_ref()
                .filter(|snapshot| snapshot.key == key && snapshot.fetched.elapsed() < SNAPSHOT_TTL)
                .map(|snapshot| snapshot.value.clone()),
        )
    }

    fn store(&self, key: &str, generation: u64, value: CachedRead, cancelled: &dyn Fn() -> bool) {
        let mut state = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // 提交命令可能已使快照失效，较早请求不能把旧状态重新放回缓存。
        if !cancelled() && state.0 == generation {
            state.1 = Some(Snapshot {
                key: key.to_owned(),
                value,
                fetched: Instant::now(),
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn retain_snapshot_for_test(&self) {
        let mut state = self.cache.lock().unwrap();
        state.1.as_mut().expect("queried repository").fetched =
            Instant::now() + Duration::from_secs(60);
    }
}

fn cache_key(kind: &str, cwd: &str, directories: &[String]) -> String {
    format!("{kind}\0{cwd}\0{directories:?}")
}

struct ReadRequest<'a> {
    cwd: &'a str,
    directories: &'a [String],
    environment: &'a Environment,
    deadline: Instant,
    output_limit: usize,
}

impl<'a> ReadRequest<'a> {
    fn new(cwd: &'a str, directories: &'a [String], environment: &'a Environment) -> Self {
        Self {
            cwd,
            directories,
            environment,
            deadline: Instant::now() + TOTAL_TIMEOUT,
            output_limit: OUTPUT_LIMIT,
        }
    }
}

type Runner<'a> =
    dyn Fn(Command, Duration, usize, &dyn Fn() -> bool) -> Result<Vec<u8>, ProcessOutputError> + 'a;

fn query_refs(
    request: &ReadRequest<'_>,
    cancelled: &dyn Fn() -> bool,
    runner: &Runner<'_>,
) -> Result<GitRefs, GitReadError> {
    let mut remaining = request.output_limit;
    // 哨兵保证无相关配置也是成功的空结果；只读取所需键，不读取 URL 或凭据。
    let config = read(
        request,
        &[
            "config",
            "--null",
            "--get-regexp",
            "^(checkout\\.(guess|defaultremote)|remote\\..*\\.fetch|completion\\.snapshot)$",
        ],
        &mut remaining,
        cancelled,
        runner,
    )?;
    let config = String::from_utf8(config).map_err(|_| GitReadError::InvalidUtf8)?;
    let text = read(
        request,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(refname:short)%00%(symref)%00%(worktreepath)%00%(objecttype)%00%(*objecttype)%00%(HEAD)",
        ],
        &mut remaining,
        cancelled,
        runner,
    )?;
    let text = String::from_utf8(text).map_err(|_| GitReadError::InvalidUtf8)?;
    let config = tracking::Config::parse(&config);
    let references: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let [
                Some(full_name),
                Some(short_name),
                Some(symref),
                Some(worktree),
                Some(object_type),
                Some(peeled_type),
                Some(head),
            ] = std::array::from_fn(|_| fields.next())
            else {
                return None;
            };
            if fields.next().is_some()
                || short_name.is_empty()
                || full_name
                    .chars()
                    .chain(short_name.chars())
                    .any(char::is_control)
            {
                return None;
            }
            Some(Reference {
                full_name: full_name.to_owned(),
                short_name: short_name.to_owned(),
                busy: !worktree.is_empty(),
                current: head == "*",
                commit: object_type == "commit" || peeled_type == "commit",
                symbolic: !symref.is_empty(),
                direct_tracking: config.can_track(full_name, cancelled),
            })
        })
        .collect();
    let guesses = config.guesses(&references, cancelled);
    let remote_branches = config.remote_branches(&references, cancelled);
    let local_branches = references
        .iter()
        .filter_map(|reference| {
            reference
                .full_name
                .strip_prefix("refs/heads/")
                .map(str::to_owned)
        })
        .collect();
    if cancelled() {
        return Err(GitReadError::Cancelled);
    }
    Ok(GitRefs {
        references,
        guesses,
        remote_branches,
        guess_enabled: config.guess,
        local_branches,
    })
}

fn query_remotes(
    request: &ReadRequest<'_>,
    cancelled: &dyn Fn() -> bool,
    runner: &Runner<'_>,
) -> Result<Vec<String>, GitReadError> {
    let mut remaining = request.output_limit;
    let bytes = read(request, &["remote"], &mut remaining, cancelled, runner)?;
    let text = String::from_utf8(bytes).map_err(|_| GitReadError::InvalidUtf8)?;
    if cancelled() {
        return Err(GitReadError::Cancelled);
    }
    Ok(text
        .lines()
        .filter(|name| {
            !name.is_empty() && !name.starts_with('-') && !name.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .collect())
}

fn read(
    request: &ReadRequest<'_>,
    args: &[&str],
    remaining: &mut usize,
    cancelled: &dyn Fn() -> bool,
    runner: &Runner<'_>,
) -> Result<Vec<u8>, GitReadError> {
    if cancelled() {
        return Err(GitReadError::Cancelled);
    }
    if Instant::now() >= request.deadline {
        return Err(GitReadError::Timeout);
    }
    let mut command = command(request);
    command.args(args);
    let bytes = runner(
        command,
        request.deadline.saturating_duration_since(Instant::now()),
        *remaining,
        cancelled,
    )?;
    *remaining = remaining
        .checked_sub(bytes.len())
        .ok_or(GitReadError::OutputLimit { limit: *remaining })?;
    Ok(bytes)
}

fn command(request: &ReadRequest<'_>) -> Command {
    let mut command = super::command(Path::new(request.cwd));
    command.envs(request.environment.iter());
    command
        .arg("--literal-pathspecs")
        .args(["-c", "core.warnAmbiguousRefs=true"])
        .args(["-c", "completion.snapshot=true"]);
    for directory in request
        .directories
        .iter()
        .filter(|directory| !directory.is_empty())
    {
        command.arg("-C").arg(directory);
    }
    // 绝不弹凭据、不触发 lazy fetch、不写 index，也不允许任何网络协议。
    for (key, value) in [
        ("GIT_OPTIONAL_LOCKS", "0"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_ALLOW_PROTOCOL", ""),
    ] {
        command.env(key, value);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::process_output::ProcessOutputError;
    use std::cell::RefCell;
    use std::process::Command;

    fn request<'a>(
        directories: &'a [String],
        environment: &'a slterm_terminal::tty::Environment,
        output_limit: usize,
    ) -> ReadRequest<'a> {
        ReadRequest {
            cwd: "D:/repo",
            directories,
            environment,
            deadline: Instant::now() + Duration::from_secs(3),
            output_limit,
        }
    }

    fn args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn refs_and_remotes_use_fixed_read_only_argv_and_shared_budgets() {
        let environment = slterm_terminal::tty::Environment::default();
        let directories = vec!["nested".to_owned()];
        let calls = RefCell::new(Vec::new());
        let runner = |command: Command, _timeout: Duration, limit: usize, _: &dyn Fn() -> bool| {
            let argv = args(&command);
            calls.borrow_mut().push((argv.clone(), limit));
            if argv.iter().any(|arg| arg == "config") {
                Ok(b"completion.snapshot\ntrue\0".to_vec())
            } else {
                Ok(b"refs/heads/main\0main\0\0\0commit\0\0*\n".to_vec())
            }
        };
        let snapshot = query_refs(
            &request(&directories, &environment, 1024),
            &|| false,
            &runner,
        )
        .unwrap();
        assert_eq!(snapshot.references[0].full_name, "refs/heads/main");
        assert!(snapshot.local_branches.contains("main"));

        let calls = calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, 1024);
        assert_eq!(calls[1].1, 1024 - b"completion.snapshot\ntrue\0".len());
        for (argv, _) in calls.iter() {
            assert!(argv.starts_with(&[
                "--no-optional-locks".into(),
                "-C".into(),
                "D:/repo".into(),
                "--literal-pathspecs".into(),
            ]));
            assert!(argv.windows(2).any(|pair| pair == ["-C", "nested"]));
        }
        assert!(calls[0].0.iter().any(|arg| arg == "config"));
        assert!(calls[1].0.iter().any(|arg| arg == "for-each-ref"));

        let remote_runner = |command: Command, _: Duration, _: usize, _: &dyn Fn() -> bool| {
            let argv = args(&command);
            assert_eq!(argv.last().map(String::as_str), Some("remote"));
            let envs: Vec<_> = command
                .get_envs()
                .map(|(key, value)| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.unwrap_or_default().to_string_lossy().into_owned(),
                    )
                })
                .collect();
            for (key, value) in [
                ("GIT_OPTIONAL_LOCKS", "0"),
                ("GIT_TERMINAL_PROMPT", "0"),
                ("GIT_NO_LAZY_FETCH", "1"),
                ("GIT_ALLOW_PROTOCOL", ""),
            ] {
                assert!(
                    envs.iter()
                        .any(|entry| entry == &(key.into(), value.into()))
                );
            }
            Ok(b"origin\nupstream\n".to_vec())
        };
        let remotes =
            query_remotes(&request(&[], &environment, 1024), &|| false, &remote_runner).unwrap();
        assert_eq!(remotes, ["origin", "upstream"]);
    }

    #[test]
    fn read_failures_keep_cancellation_timeout_output_and_exit_distinct() {
        let environment = slterm_terminal::tty::Environment::default();
        let cancelled = query_remotes(
            &request(&[], &environment, 1024),
            &|| true,
            &|_: Command, _: Duration, _: usize, _: &dyn Fn() -> bool| panic!("取消后不得启动进程"),
        );
        assert!(matches!(cancelled, Err(GitReadError::Cancelled)));

        for expected in ["timeout", "output"] {
            let result = query_remotes(
                &request(&[], &environment, 1024),
                &|| false,
                &|_: Command, _: Duration, _: usize, _: &dyn Fn() -> bool| match expected {
                    "timeout" => Err(ProcessOutputError::Timeout),
                    _ => Err(ProcessOutputError::OutputLimit { limit: 1024 }),
                },
            );
            match (result, expected) {
                (Err(GitReadError::Timeout), "timeout") => {}
                (Err(GitReadError::OutputLimit { limit: 1024 }), "output") => {}
                (other, expected) => panic!("{expected} 分类错误: {other:?}"),
            }
        }

        let status = Command::new("cmd.exe")
            .args(["/d", "/c", "exit 7"])
            .status()
            .unwrap();
        let result = query_remotes(
            &request(&[], &environment, 1024),
            &|| false,
            &|_: Command, _: Duration, _: usize, _: &dyn Fn() -> bool| {
                Err(ProcessOutputError::Exit(status))
            },
        );
        assert!(matches!(
            result,
            Err(GitReadError::Exit(exit)) if exit.code() == Some(7)
        ));

        let result = query_remotes(
            &request(&[], &environment, 1024),
            &|| false,
            &|_: Command, _: Duration, _: usize, _: &dyn Fn() -> bool| {
                Err(ProcessOutputError::Spawn(io::Error::new(
                    io::ErrorKind::NotFound,
                    "git missing",
                )))
            },
        );
        assert!(matches!(result, Err(GitReadError::Spawn(_))));
    }
}
