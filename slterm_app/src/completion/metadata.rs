//! 执行元数据:补全请求在后台可安全复用的本机执行环境。
//!
//! 上游 `Execution` 有进程/远程两态;源收敛后只剩本机进程单态。
//! Git 的 argv、双预算与只读护栏统一归 `git::GitFacade`，本模块不再提供
//! 通用命令通道。

use crate::suggest::SuggestEnv;

/// 补全可安全复用的 PTY 启动上下文：只冻结 shell 程序与环境覆盖。
#[derive(Clone, Debug)]
pub(crate) struct PaneExecContext {
    shell_program: Option<String>,
    env: slterm_terminal::tty::Environment,
}

impl PaneExecContext {
    pub(crate) fn from_pty_options(options: &slterm_terminal::tty::Options) -> Self {
        Self {
            shell_program: options
                .shell
                .as_ref()
                .map(|shell| shell.program().to_owned()),
            env: options.env.clone(),
        }
    }

    pub(crate) fn shell_program(&self) -> Option<&str> {
        self.shell_program.as_deref()
    }

    /// Git 元数据读取复用 pane 创建时的环境，保证代理与凭据策略一致。
    pub(crate) fn environment(&self) -> &slterm_terminal::tty::Environment {
        &self.env
    }
}

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

    pub(crate) fn environment(&self) -> &slterm_terminal::tty::Environment {
        match self {
            Self::Process { context, .. } => context.environment(),
        }
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn execution() -> Execution {
        Execution::from_scope(SuggestEnv::Local, None).unwrap()
    }

    #[test]
    fn local_prepare_is_stable() {
        let mut execution = execution();
        assert!(execution.prepare());
    }
}
