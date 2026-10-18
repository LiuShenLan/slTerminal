//! 设置页面对的发现与命令面(消费归 06 篇设置页 hooks 组)。文件 I/O 由
//! 调用方放后台执行器。

use std::path::{Path, PathBuf};

use slterm_settings::AgentHook;

/// 设置页展示顺序;`AgentHook::ALL` 是应用顺序,两表语义不同,不合并。
pub(crate) const AGENTS: [AgentHook; 9] = [
    AgentHook::Claude,
    AgentHook::Codex,
    AgentHook::OpenCode,
    AgentHook::Cursor,
    AgentHook::Kimi,
    AgentHook::Pi,
    AgentHook::OhMyPi,
    AgentHook::Copilot,
    AgentHook::Grok,
];

#[derive(Clone, Debug, Default)]
pub(crate) struct HookInspection {
    pub config_path: Option<PathBuf>,
    pub available: bool,
    pub installed: bool,
    pub needs_repair: bool,
    pub enabled: bool,
    pub helper_missing: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentIntegration {
    pub agent: AgentHook,
    pub executable: Option<PathBuf>,
    pub inspection: HookInspection,
}

pub(crate) fn inspect() -> Vec<AgentIntegration> {
    let dirs = executable_directories();
    AGENTS
        .into_iter()
        .map(|agent| AgentIntegration {
            agent,
            executable: find_executable(agent, &dirs),
            inspection: super::local::settings::inspect(agent),
        })
        .collect()
}

pub(crate) fn executable(agent: AgentHook) -> Option<PathBuf> {
    find_executable(agent, &executable_directories())
}

pub(crate) fn set_enabled(hook: AgentHook, enabled: bool) -> Result<(), String> {
    super::local::settings::set_enabled(hook, enabled).map_err(|error| error.to_string())
}

fn executable_directories() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::env::var_os("PATH")
        .map(|value| {
            std::env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    if let Some(home) = crate::platform::dirs::home_dir() {
        paths.extend([
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            home.join(".bun/bin"),
            home.join(".grok/bin"),
        ]);
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        paths.push(PathBuf::from(appdata).join("npm"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join("cursor-agent"));
    }
    paths
}

/// 9 家 hook agent 的可执行名候选。27 家身份枚举归改造节 4,落地后本表
/// 改由身份单源供给——此处只是 hook 轴的工作表,禁作他用。
fn aliases(agent: AgentHook) -> &'static [&'static str] {
    match agent {
        AgentHook::Claude => &["claude", "claude-code"],
        AgentHook::Codex => &["codex", "codex-cli"],
        AgentHook::OpenCode => &["opencode", "open-code"],
        // Cursor 现行 CLI 二进制名是 agent;历史名保留供既有安装发现。
        AgentHook::Cursor => &["agent", "cursor", "cursor-agent"],
        AgentHook::Pi => &["pi"],
        AgentHook::OhMyPi => &["omp", "oh-my-pi"],
        AgentHook::Copilot => &["copilot", "github-copilot", "ghcs"],
        AgentHook::Grok => &["grok", "grok-cli", "grok-build"],
        AgentHook::Kimi => &["kimi", "kimi-code"],
    }
}

fn find_executable(agent: AgentHook, directories: &[PathBuf]) -> Option<PathBuf> {
    let extensions: &[&str] = &[".exe", ".cmd", ".bat", ".ps1", ""];
    directories.iter().find_map(|directory| {
        aliases(agent)
            .iter()
            .copied()
            .chain((agent == AgentHook::Grok).then_some("agent"))
            .find_map(|alias| {
                extensions.iter().find_map(|extension| {
                    let path = directory.join(format!("{alias}{extension}"));
                    (executable_file(&path) && executable_matches(agent, alias, &path))
                        .then_some(path)
                })
            })
    })
}

fn executable_matches(agent: AgentHook, alias: &str, path: &Path) -> bool {
    if agent == AgentHook::Cursor && alias == "cursor" {
        // 桌面编辑器的启动器不能证明 Cursor Agent CLI 已安装。
        return false;
    }
    if alias != "agent" {
        return true;
    }
    // 多个工具使用 agent 这个通用文件名;按安装路径(包括符号链接目标)
    // 归属,不能把 .grok/bin/agent 误报为 Cursor。
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let belongs = |name: &str| {
        resolved.components().any(|part| {
            part.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
    };
    match agent {
        AgentHook::Cursor => belongs("cursor-agent") || belongs(".cursor"),
        AgentHook::Grok => belongs(".grok"),
        _ => false,
    }
}

fn executable_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn executable(directory: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(directory).unwrap();
        let path = directory.join(format!("{name}.exe"));
        std::fs::write(&path, b"fixture").unwrap();
        path
    }

    #[test]
    fn generic_agent_binaries_require_a_matching_installation() {
        let dir = tempfile::tempdir().unwrap();
        let grok_dir = dir.path().join(".grok/bin");
        let grok = executable(&grok_dir, "agent");
        let cursor_dir = dir.path().join("installed/cursor-agent/versions/current");
        let cursor = executable(&cursor_dir, "agent");
        let dirs = vec![grok_dir, cursor_dir];
        assert_eq!(find_executable(AgentHook::Cursor, &dirs), Some(cursor));
        assert_eq!(find_executable(AgentHook::Grok, &dirs), Some(grok));
        let generic_dir = dir.path().join("bin");
        executable(&generic_dir, "agent");
        executable(&generic_dir, "cursor");
        assert_eq!(
            find_executable(AgentHook::Cursor, std::slice::from_ref(&generic_dir)),
            None
        );
        let cli = executable(&generic_dir, "cursor-agent");
        assert_eq!(
            find_executable(AgentHook::Cursor, &[generic_dir]),
            Some(cli)
        );
    }

    #[test]
    fn configuration_directories_do_not_count_as_executables() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("claude")).unwrap();
        assert_eq!(
            find_executable(AgentHook::Claude, &[dir.path().to_path_buf()]),
            None
        );
    }
}
