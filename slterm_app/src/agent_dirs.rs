//! Agent 全局配置目录解析(沙箱放行真值源)。
//!
//! 身份域分工:目录表管沙箱放行、[`AgentKind`] 管身份、屏幕 TOML 管规则——
//! 三份清单各管一域,不合并(合并 = 单点失效面扩大)。
//!
//! 静态表只在后端硬编码,前端不能注入任意路径;路径解析经
//! [`crate::platform::dirs::home_dir`],空 home 解析结果为空集(安全默认:
//! 不放行)。

use std::path::{Path, PathBuf};

use crate::ai_agents::AgentKind;

/// cliId → home 相对配置目录。27 行静态表,与 `AgentKind::ALL` 对齐
/// (编译守卫 + 测试逐行钉值)。新增 agent = AgentKind 加变体 + 本表加行 +
/// 屏幕 TOML 加文件,三处同步登记。
///
/// 值以各 CLI 官方文档为准;Windows 上落 `%APPDATA%` 的客户端写成
/// `AppData/Roaming/...`(本表语义始终是 home 相对路径)。
///
/// 2026-10-18 六家 Windows 目录复核(官方文档/官方仓库源码 + 本机实测):
/// goose `%APPDATA%\Block\goose\config`、devin `%APPDATA%\devin`、
/// antigravity `%USERPROFILE%\.gemini\antigravity-cli`、pi
/// `%USERPROFILE%\.pi\agent` 与官方文档一致;maki 官方仓库按 etcetera
/// Windows 策略取 `%APPDATA%\maki`(`~/.config/maki` 仅 Unix),kilo 官方
/// 设置文档与 xdg-basedir 源码取 `%USERPROFILE%\.config\kilo`(Windows 仍
/// 走 `~/.config`,非 `%APPDATA%`)。本机六家 CLI 均未安装、上述目录均不存在。
pub(crate) const AGENT_CONFIG_DIRS: [(&str, &str); 27] = [
    ("claude", ".claude"),
    ("codex", ".codex"),
    ("gemini", ".gemini"),
    ("aider", ".aider"),
    ("amp", ".config/amp"),
    ("opencode", ".config/opencode"),
    ("copilot", ".copilot"),
    ("cursor", ".cursor"),
    ("goose", "AppData/Roaming/Block/goose/config"),
    ("droid", ".factory"),
    ("pi", ".pi/agent"),
    ("auggie", ".augment"),
    ("hermes", ".hermes"),
    ("vibe", ".vibe"),
    ("antigravity", ".gemini/antigravity-cli"),
    ("grok", ".grok"),
    ("qwen", ".qwen"),
    ("omp", ".omp"),
    ("cline", ".cline"),
    ("devin", "AppData/Roaming/devin"),
    ("kimi", ".kimi-code"),
    ("kiro", ".kiro"),
    ("kilo", ".config/kilo"),
    ("qodercli", ".qoder"),
    ("maki", "AppData/Roaming/maki"),
    ("trae-cli", ".trae"),
    ("codebuddy", ".codebuddy"),
];

/// 表长 == 身份枚举;两边任何一侧漏改都在编译期报错。
const _: () = assert!(AGENT_CONFIG_DIRS.len() == AgentKind::ALL.len());

/// 设置页/侧栏展示项(消费归工作区与设置域)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentGlobalDir {
    /// CLI profile id(= [`AgentKind::slug`])。
    pub cli_id: String,
    /// 解析后的绝对路径(home + 相对目录;不做 canonicalize,展示与命令
    /// 参数回传用,沙箱校验侧自行双侧 canonicalize 比较)。
    pub path: PathBuf,
    /// 目录当前是否存在(false → 展示占位,不启动监听)。
    pub exists: bool,
}

fn resolve_agent_dirs(home: Option<&Path>) -> Vec<AgentGlobalDir> {
    let Some(home) = home else {
        return Vec::new();
    };
    AGENT_CONFIG_DIRS
        .iter()
        .map(|(cli_id, relative)| {
            let path = home.join(relative);
            AgentGlobalDir {
                cli_id: (*cli_id).to_owned(),
                exists: path.is_dir(),
                path,
            }
        })
        .collect()
}

/// 沙箱放行用目录集。home 不可解析 → 空集(= 不放行)。
pub(crate) fn agent_dir_paths() -> Vec<PathBuf> {
    resolve_agent_dirs(crate::platform::dirs::home_dir().as_deref())
        .into_iter()
        .map(|directory| directory.path)
        .collect()
}

/// 列出全部 agent 全局配置目录(含不存在项——前端据此展示占位)。
pub fn agent_global_dirs() -> Vec<AgentGlobalDir> {
    resolve_agent_dirs(crate::platform::dirs::home_dir().as_deref())
}

#[cfg(test)]
mod agent_dirs_tests {
    use super::*;

    #[test]
    fn table_aligns_with_agent_kind_order() {
        assert_eq!(AGENT_CONFIG_DIRS.len(), AgentKind::ALL.len());
        for (kind, (slug, _)) in AgentKind::ALL.iter().zip(AGENT_CONFIG_DIRS) {
            assert_eq!(slug, kind.slug(), "目录表顺序必须与 AgentKind::ALL 一致");
        }
    }

    #[test]
    fn every_directory_value_is_pinned() {
        // 目录值是安全边界(沙箱放行集),改任何一行都必须显式改本用例。
        assert_eq!(
            AGENT_CONFIG_DIRS,
            [
                ("claude", ".claude"),
                ("codex", ".codex"),
                ("gemini", ".gemini"),
                ("aider", ".aider"),
                ("amp", ".config/amp"),
                ("opencode", ".config/opencode"),
                ("copilot", ".copilot"),
                ("cursor", ".cursor"),
                ("goose", "AppData/Roaming/Block/goose/config"),
                ("droid", ".factory"),
                ("pi", ".pi/agent"),
                ("auggie", ".augment"),
                ("hermes", ".hermes"),
                ("vibe", ".vibe"),
                ("antigravity", ".gemini/antigravity-cli"),
                ("grok", ".grok"),
                ("qwen", ".qwen"),
                ("omp", ".omp"),
                ("cline", ".cline"),
                ("devin", "AppData/Roaming/devin"),
                ("kimi", ".kimi-code"),
                ("kiro", ".kiro"),
                ("kilo", ".config/kilo"),
                ("qodercli", ".qoder"),
                ("maki", "AppData/Roaming/maki"),
                ("trae-cli", ".trae"),
                ("codebuddy", ".codebuddy"),
            ]
        );
    }

    #[test]
    fn resolution_marks_missing_and_existing_directories() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        let dirs = resolve_agent_dirs(Some(home.path()));
        assert_eq!(dirs.len(), 27);
        let claude = dirs
            .iter()
            .find(|directory| directory.cli_id == "claude")
            .unwrap();
        assert_eq!(claude.path, home.path().join(".claude"));
        assert!(claude.exists);
        let codex = dirs
            .iter()
            .find(|directory| directory.cli_id == "codex")
            .unwrap();
        assert_eq!(codex.path, home.path().join(".codex"));
        assert!(!codex.exists);
    }

    #[test]
    fn unreadable_home_yields_no_sandbox_paths() {
        assert!(resolve_agent_dirs(None).is_empty());
    }
}
