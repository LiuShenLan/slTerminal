//! 执行元数据:补全请求在后台可安全复用的本机执行环境。
//!
//! Git 的 argv、双预算与只读护栏统一归 `git::GitFacade`，本模块不再提供
//! 通用命令通道。

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
