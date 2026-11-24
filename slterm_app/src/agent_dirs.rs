//! Agent 全局配置目录解析(沙箱放行真值源)。
//!
//! 目录真值源是 [`AgentKind::catalog`] 的 `default_root`;本模块只做 home
//! 拼接与存在性探测,不再维护第二份目录表。
//!
//! 静态表只在后端硬编码,前端不能注入任意路径;路径解析经
//! [`crate::platform::dirs::home_dir`],空 home 解析结果为空集(安全默认:
//! 不放行)。

use std::path::{Path, PathBuf};

use crate::ai_agents::AgentKind;

/// 设置页/侧栏展示项(消费归工作区与设置域)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentGlobalDir {
    /// CLI profile id(= [`AgentKind::slug`])。
    pub cli_id: String,
    /// 内置 AgentRoot 的稳定身份；文件树根与 ViewState 分区共用。
    pub root_id: slterm_settings::AgentRootId,
    /// 展示名(catalog 声明)。
    pub display_name: String,
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
    AgentKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let relative = kind.catalog().default_root?;
            let path = home.join(relative);
            Some(AgentGlobalDir {
                cli_id: kind.slug().to_owned(),
                root_id: kind.builtin_root_id(),
                display_name: kind.display_name().to_owned(),
                exists: path.is_dir(),
                path,
            })
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
        assert_eq!(claude.root_id, AgentKind::Claude.builtin_root_id());
        assert_eq!(claude.display_name, "Claude Code");
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
