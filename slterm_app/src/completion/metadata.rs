//! 执行元数据:补全请求在后台可安全复用的本机执行环境。
//!
//! 上游 `Execution` 有进程/远程两态;源收敛后只剩本机进程单态:
//! `prepare()` 是本地直通,不再有远程传输冻结。Git 读取
//! 统一走 `git::command` 构造口(每命令 `--no-optional-locks`)与
//! 3s/1MB 可取消预算。

use std::path::Path;
use std::time::Instant;

use crate::runtime_exec::PaneExecContext;
use crate::suggest::SuggestEnv;

#[derive(Clone)]
pub(crate) enum Execution {
    Process {
        context: PaneExecContext,
        scope: SuggestEnv,
    },
}

impl Execution {
    /// 本机执行无需额外握手;保留返回 Bool 的形态以维持请求链的降级合同。
    pub(super) fn prepare(&mut self) -> bool {
        true
    }

    /// 只有需要本机 I/O 的来源才复制启动环境;未给出上下文时用默认 shell 形态。
    pub(crate) fn from_scope(
        scope: SuggestEnv,
        execution: Option<&PaneExecContext>,
    ) -> Option<Self> {
        Some(Self::Process {
            context: execution.cloned().unwrap_or_else(|| {
                PaneExecContext::from_pty_options(&slterm_terminal::tty::Options::default())
            }),
            scope,
        })
    }

    /// 缓存键:同一目录/仓库的多个前缀复用同一快照。
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Process { context, scope } => {
                format!("{scope:?}\0{}", context.shell_program().unwrap_or_default())
            }
        }
    }

    /// 有界 Git 读取:总预算由调用方换算为剩余时长,取消令牌透传到子进程。
    pub(crate) fn git(
        &self,
        cwd: &str,
        directories: &[String],
        args: &[&str],
        deadline: Instant,
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<Vec<u8>> {
        let Self::Process { context, .. } = self;
        let budget = deadline.saturating_duration_since(Instant::now());
        let mut command = crate::git::command(Path::new(cwd));
        // 复用 pane 创建时冻结的环境覆盖(代理/凭据策略与 shell 内一致)。
        command.envs(context.environment());
        // `completion.snapshot=true` 是 sentinel:即使仓库没有相关配置,
        // `git config --get-regexp` 也返回 0,空结果才不会被当成读取失败。
        command
            .args(["-c", "core.warnAmbiguousRefs=true"])
            .args(["-c", "completion.snapshot=true"]);
        for directory in directories.iter().filter(|directory| !directory.is_empty()) {
            command.arg("-C").arg(directory);
        }
        command.args(args);
        // 与 git facade 一致的护栏:绝不弹凭据、不触发 lazy fetch、不写 index。
        for (key, value) in [
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_ALLOW_PROTOCOL", ""),
        ] {
            command.env(key, value);
        }
        crate::platform::process_output::read_cancellable(command, budget, limit, cancelled).ok()
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    use std::time::Duration;

    fn execution() -> Execution {
        Execution::from_scope(SuggestEnv::Local, None).unwrap()
    }

    #[test]
    fn local_prepare_and_key_are_stable() {
        let mut execution = execution();
        assert!(execution.prepare());
        assert_eq!(execution.key(), execution.key());
    }

    #[test]
    fn cancelled_or_exhausted_git_reads_do_not_start_a_process() {
        let execution = execution();
        let deadline = Instant::now() + Duration::from_secs(3);
        assert!(
            execution
                .git("D:/", &[], &["remote"], deadline, 1024, &|| true)
                .is_none()
        );
        assert!(
            execution
                .git("D:/", &[], &["remote"], Instant::now(), 1024, &|| false)
                .is_none()
        );
    }

    #[test]
    fn git_metadata_reads_use_the_shared_no_optional_locks_constructor() {
        let repository = tempfile::tempdir().unwrap();
        let mut init = std::process::Command::new("git");
        init.args(["init", "-b", "main"])
            .current_dir(repository.path());
        crate::platform::process::hidden_command(&mut init)
            .output()
            .unwrap();
        let execution = execution();
        let bytes = execution
            .git(
                repository.path().to_str().unwrap(),
                &[],
                &["rev-parse", "--is-inside-work-tree"],
                Instant::now() + Duration::from_secs(3),
                1024,
                &|| false,
            )
            .expect("git metadata");
        assert_eq!(String::from_utf8(bytes).unwrap().trim(), "true");
    }
}
