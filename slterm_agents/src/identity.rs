/// slTerminal 能作为一等终端负载识别的 AI 客户端。
/// slug 是跨域身份事实单源：hook 事件 source、设置页、恢复命令、目录表全部
/// 消费 slug，禁止第二份枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Aider,
    Amp,
    OpenCode,
    Copilot,
    Cursor,
    Goose,
    Droid,
    Pi,
    Auggie,
    Hermes,
    Vibe,
    Antigravity,
    Grok,
    Qwen,
    OhMyPi,
    Cline,
    Devin,
    Kimi,
    Kiro,
    Kilo,
    Qoder,
    Maki,
    Trae,
    CodeBuddy,
}

impl AgentKind {
    pub const ALL: [Self; 27] = [
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Aider,
        Self::Amp,
        Self::OpenCode,
        Self::Copilot,
        Self::Cursor,
        Self::Goose,
        Self::Droid,
        Self::Pi,
        Self::Auggie,
        Self::Hermes,
        Self::Vibe,
        Self::Antigravity,
        Self::Grok,
        Self::Qwen,
        Self::OhMyPi,
        Self::Cline,
        Self::Devin,
        Self::Kimi,
        Self::Kiro,
        Self::Kilo,
        Self::Qoder,
        Self::Maki,
        Self::Trae,
        Self::CodeBuddy,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Aider => "aider",
            Self::Amp => "amp",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
            Self::Cursor => "cursor",
            Self::Goose => "goose",
            Self::Droid => "droid",
            Self::Pi => "pi",
            Self::Auggie => "auggie",
            Self::Hermes => "hermes",
            Self::Vibe => "vibe",
            Self::Antigravity => "antigravity",
            Self::Grok => "grok",
            Self::Qwen => "qwen",
            Self::OhMyPi => "omp",
            Self::Cline => "cline",
            Self::Devin => "devin",
            Self::Kimi => "kimi",
            Self::Kiro => "kiro",
            Self::Kilo => "kilo",
            Self::Qoder => "qodercli",
            Self::Maki => "maki",
            Self::Trae => "trae-cli",
            Self::CodeBuddy => "codebuddy",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini",
            Self::Aider => "Aider",
            Self::Amp => "Amp",
            Self::OpenCode => "OpenCode",
            Self::Copilot => "GitHub Copilot",
            Self::Cursor => "Cursor Agent",
            Self::Goose => "Goose",
            Self::Droid => "Droid",
            Self::Pi => "Pi",
            Self::Auggie => "Auggie",
            Self::Hermes => "Hermes",
            Self::Vibe => "Vibe",
            Self::Antigravity => "Antigravity",
            Self::Grok => "Grok",
            Self::Qwen => "Qwen Code",
            Self::OhMyPi => "Oh My Pi",
            Self::Cline => "Cline",
            Self::Devin => "Devin",
            Self::Kimi => "Kimi Code",
            Self::Kiro => "Kiro",
            Self::Kilo => "Kilo Code",
            Self::Qoder => "Qoder",
            Self::Maki => "Maki",
            Self::Trae => "Trae CLI",
            Self::CodeBuddy => "CodeBuddy Code",
        }
    }

    pub fn label(self) -> &'static str {
        self.slug()
    }

    /// 进程识别与设置页可执行文件发现共用同一组官方/兼容名称。
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["claude", "claude-code"],
            Self::Codex => &["codex", "codex-cli"],
            Self::Gemini => &["gemini", "gemini-cli"],
            Self::Aider => &["aider", "aider-chat"],
            Self::Amp => &["amp", "amp-local"],
            Self::OpenCode => &["opencode", "open-code"],
            Self::Copilot => &["copilot", "github-copilot", "ghcs"],
            // Cursor 现行 CLI 二进制名是 `agent`；历史标签保留，旧进程快照
            // 仍能正确归一。
            Self::Cursor => &["agent", "cursor", "cursor-agent"],
            Self::Goose => &["goose"],
            Self::Droid => &["droid"],
            Self::Pi => &["pi"],
            Self::Auggie => &["auggie"],
            Self::Hermes => &["hermes", "hermes-agent"],
            Self::Vibe => &["vibe", "vibe-acp"],
            Self::Antigravity => &["agy", "antigravity", "antigravity-cli"],
            Self::Grok => &["grok", "grok-cli", "grok-build"],
            Self::Qwen => &["qwen", "qwen-code"],
            Self::OhMyPi => &["omp", "oh-my-pi"],
            Self::Cline => &["cline"],
            Self::Devin => &["devin", "devin-cli"],
            Self::Kimi => &["kimi", "kimi-code"],
            Self::Kiro => &["kiro", "kiro-cli"],
            Self::Kilo => &["kilo", "kilo-code"],
            Self::Qoder => &["qodercli", "qoderclicn", "qoder", "qodercn"],
            Self::Maki => &["maki"],
            // 字节 trae-agent 声明的控制台入口名。
            Self::Trae => &["trae-cli"],
            // @tencent-ai/codebuddy-code 2.150.0 的交互式 bin 入口。
            Self::CodeBuddy => &["codebuddy", "cbc", "codebuddy-code", "codebuddy-lowmem"],
        }
    }

    /// 可执行名/程序标签归一为规范客户端身份。
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw
            .trim()
            .trim_matches(['"', '\''])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(raw)
            .to_ascii_lowercase();
        let raw = [".exe", ".cmd", ".bat", ".ps1", ".com", ".js"]
            .into_iter()
            .find_map(|suffix| raw.strip_suffix(suffix))
            .unwrap_or(&raw);
        Self::ALL
            .into_iter()
            .find(|agent| agent.aliases().contains(&raw))
    }

    /// 从提交的 shell 命令解析 agent，含常见解释器与包运行器启动形态。
    pub fn parse_command(command: &str) -> Option<Self> {
        let mut tokens = command
            .split_whitespace()
            .map(|token| token.trim_matches(['"', '\'']))
            .filter(|token| !token.is_empty());
        let mut launcher = tokens.next()?;
        if launcher == "&" {
            launcher = tokens.next()?;
        }
        while is_env_assignment(launcher) {
            launcher = tokens.next()?;
        }

        if launcher_stem(launcher) == "env" {
            launcher = tokens
                .by_ref()
                .find(|token| !token.starts_with('-') && !is_env_assignment(token))?;
        }
        if let Some(agent) = Self::parse(launcher) {
            return Some(agent);
        }
        if !is_agent_interpreter(launcher) {
            return None;
        }

        tokens
            .filter(|token| !token.starts_with('-'))
            // 预热 helper 与 CLI 本体同驻 codebuddy-code 包；仅凭父目录名不能
            // 把它认成在跑的 CLI。
            .filter(|token| launcher_stem(token) != "cbc-prewarm")
            .find_map(|token| token.split(['/', '\\']).find_map(Self::parse))
    }

    /// shell 安全的 resume 命令。session id 是不可信的 hook/文件输入，形状不
    /// 合法时返回 `None` 而不是加引号硬拼。
    pub fn resume_command(self, session_id: &str) -> Option<String> {
        valid_session_id(session_id)?;
        Some(match self {
            Self::Claude => format!("claude --resume {session_id}"),
            Self::Codex => format!("codex resume {session_id}"),
            Self::Gemini => format!("gemini --resume {session_id}"),
            Self::OpenCode => format!("opencode --session {session_id}"),
            Self::Amp => format!("amp threads continue {session_id}"),
            Self::Cursor => format!("agent --resume={session_id}"),
            Self::Copilot => format!("copilot --resume {session_id}"),
            Self::Grok => format!("grok --resume {session_id}"),
            Self::Pi => format!("pi --session {session_id}"),
            Self::OhMyPi => format!("omp --resume {session_id}"),
            Self::Aider
            | Self::Goose
            | Self::Droid
            | Self::Auggie
            | Self::Hermes
            | Self::Vibe
            | Self::Antigravity
            | Self::Qwen
            | Self::Cline
            | Self::Devin
            | Self::Kiro
            | Self::Kilo
            | Self::Qoder
            | Self::Maki
            | Self::Trae
            | Self::CodeBuddy => return None,
            Self::Kimi => format!("kimi --session {session_id}"),
        })
    }

    /// shell 安全的 fork 命令，仅覆盖有实测 fork 语法的客户端。
    pub fn fork_command(self, session_id: &str) -> Option<String> {
        valid_session_id(session_id)?;
        Some(match self {
            Self::Claude => format!("claude --resume {session_id} --fork-session"),
            Self::Codex => format!("codex fork {session_id}"),
            Self::OpenCode => format!("opencode --session {session_id} --fork"),
            Self::Grok => format!("grok --resume {session_id} --fork-session"),
            Self::Pi => format!("pi --fork {session_id}"),
            Self::OhMyPi => format!("omp --fork {session_id}"),
            Self::Kimi => format!("kimi --fork {session_id}"),
            _ => return None,
        })
    }
}

pub(crate) fn valid_session_id(id: &str) -> Option<()> {
    (!id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')))
    .then_some(())
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn launcher_stem(token: &str) -> String {
    let basename = token
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token);
    let lower = basename.to_ascii_lowercase();
    [".exe", ".cmd", ".bat", ".ps1", ".com"]
        .into_iter()
        .find_map(|suffix| lower.strip_suffix(suffix).map(str::to_owned))
        .unwrap_or(lower)
}

fn is_agent_interpreter(token: &str) -> bool {
    matches!(
        launcher_stem(token).as_str(),
        "node"
            | "nodejs"
            | "bun"
            | "deno"
            | "npx"
            | "npm"
            | "pnpm"
            | "pnpx"
            | "yarn"
            | "bunx"
            | "python"
            | "python3"
            | "ruby"
            | "pipx"
            | "uv"
            | "uvx"
    )
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_canonical_agents() {
        assert_eq!(
            AgentKind::parse(r"C:\tools\CLAUDE.EXE"),
            Some(AgentKind::Claude)
        );
        assert_eq!(AgentKind::parse("cursor-agent"), Some(AgentKind::Cursor));
        assert_eq!(AgentKind::parse("grok-cli.cmd"), Some(AgentKind::Grok));
        assert_eq!(AgentKind::parse("cargo"), None);
    }

    #[test]
    fn all_twenty_seven_agents_are_registered_once() {
        assert_eq!(AgentKind::ALL.len(), 27);
        let mut slugs: Vec<&str> = AgentKind::ALL.iter().map(|agent| agent.slug()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), 27, "slug 是跨域身份事实单源，不许撞名");
    }

    #[test]
    fn every_registered_agent_parses_its_slug_and_aliases() {
        for agent in AgentKind::ALL {
            assert_eq!(AgentKind::parse(agent.slug()), Some(agent));
            for alias in agent.aliases() {
                assert_eq!(
                    AgentKind::parse(alias),
                    Some(agent),
                    "{} alias {alias}",
                    agent.slug()
                );
            }
        }
    }

    #[test]
    fn trae_cli_identity_does_not_guess_session_commands() {
        for command in ["trae-cli", r"C:\tools\TRAE-CLI.EXE", "/usr/bin/trae-cli"] {
            assert_eq!(AgentKind::parse(command), Some(AgentKind::Trae));
        }
        assert_eq!(
            AgentKind::parse_command("uv run trae-cli --help"),
            Some(AgentKind::Trae)
        );
        assert_eq!(AgentKind::parse("trae-cli-helper"), None);
        assert_eq!(AgentKind::Trae.resume_command("session-1"), None);
        assert_eq!(AgentKind::Trae.fork_command("session-1"), None);
    }

    #[test]
    fn codebuddy_entry_points_identify_the_cli_without_claiming_its_helper() {
        for command in [
            "codebuddy",
            "cbc",
            "codebuddy-code",
            "codebuddy-lowmem",
            r"C:\tools\CODEBUDDY.CMD",
            "/usr/bin/cbc",
        ] {
            assert_eq!(
                AgentKind::parse(command),
                Some(AgentKind::CodeBuddy),
                "{command}"
            );
        }
        for command in [
            "npx --yes @tencent-ai/codebuddy-code",
            "node /opt/node_modules/@tencent-ai/codebuddy-code/dist/codebuddy.js",
            "env DEBUG=1 codebuddy --help",
        ] {
            assert_eq!(
                AgentKind::parse_command(command),
                Some(AgentKind::CodeBuddy),
                "{command}"
            );
        }
        for command in [
            "cbc-prewarm",
            "codebuddy-helper",
            "cat codebuddy.md",
            "node /opt/node_modules/@tencent-ai/codebuddy-code/bin/cbc-prewarm",
        ] {
            assert_eq!(AgentKind::parse_command(command), None, "{command}");
        }
        assert_eq!(AgentKind::CodeBuddy.resume_command("session-1"), None);
        assert_eq!(AgentKind::CodeBuddy.fork_command("session-1"), None);
    }

    #[test]
    fn submitted_commands_resolve_agents_across_launch_forms() {
        for agent in AgentKind::ALL {
            assert_eq!(
                AgentKind::parse_command(agent.slug()),
                Some(agent),
                "{}",
                agent.slug()
            );
        }
        assert_eq!(
            AgentKind::parse_command("FOO=1 npx --yes @openai/codex --model o3"),
            Some(AgentKind::Codex)
        );
        assert_eq!(
            AgentKind::parse_command("env DEBUG=1 node /opt/@anthropic-ai/claude-code/cli.js"),
            Some(AgentKind::Claude)
        );
        assert_eq!(
            AgentKind::parse_command("npm exec @google/gemini-cli"),
            Some(AgentKind::Gemini)
        );
        assert_eq!(
            AgentKind::parse_command("uvx aider-chat"),
            Some(AgentKind::Aider)
        );
        assert_eq!(AgentKind::parse_command("cat codex.md"), None);
        assert_eq!(AgentKind::parse_command("node server.js"), None);
    }

    #[test]
    fn commands_are_exact_and_injection_safe() {
        assert_eq!(
            AgentKind::Claude.resume_command("abc-123").as_deref(),
            Some("claude --resume abc-123")
        );
        assert_eq!(
            AgentKind::Codex.fork_command("abc-123").as_deref(),
            Some("codex fork abc-123")
        );
        assert_eq!(
            AgentKind::OpenCode.fork_command("abc-123").as_deref(),
            Some("opencode --session abc-123 --fork")
        );
        assert_eq!(
            AgentKind::Cursor.resume_command("abc-123").as_deref(),
            Some("agent --resume=abc-123")
        );
        assert_eq!(
            AgentKind::Pi.fork_command("abc-123").as_deref(),
            Some("pi --fork abc-123")
        );
        assert_eq!(
            AgentKind::OhMyPi.resume_command("abc-123").as_deref(),
            Some("omp --resume abc-123")
        );
        assert_eq!(
            AgentKind::Kimi.resume_command("abc-123").as_deref(),
            Some("kimi --session abc-123")
        );
        assert_eq!(
            AgentKind::Kimi.fork_command("abc-123").as_deref(),
            Some("kimi --fork abc-123")
        );
        assert_eq!(AgentKind::Claude.resume_command("x; calc"), None);
        assert_eq!(AgentKind::Aider.resume_command("abc"), None);
    }
}
