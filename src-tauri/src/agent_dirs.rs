//! Agent 全局配置目录解析模块（ADR-0024）
//!
//! 职责：
//! - 后端静态表：cliId → home 相对配置目录（沙箱白名单唯一知识源——表在后端
//!   硬编码，前端永不能注入任意路径）
//! - `agent_dir_paths()`：供 state.rs 路径沙箱放行的目录集（不经命令层）
//! - `agent_dirs_list` 命令：前端「Agent 全局文件」视图取解析后绝对路径 + 存在性
//!
//! 与前端 profile 的分工：profile 声明 `globalFiles` 能力（configDir/runtimePaths，
//! AC-5 字面量领地），本表是安全边界——两侧各自枚举同一 agent 集属刻意冗余
//! （能力声明 ≠ 沙箱放行，任一缺失即该侧功能不生效）。
//!
//! 路径解析经 `crate::home::home_dir()`（env-first，E2E 假 home 隔离自动跟随）。

use std::path::PathBuf;

use serde::Serialize;

use crate::error::AppError;

/// Agent 全局配置目录静态表：cliId → home 相对目录。
/// 新增 agent = 本表加一行 + 前端 profile 声明 globalFiles 能力（两侧同步）。
const AGENT_CONFIG_DIRS: [(&str, &str); 1] = [("claude", ".claude")];

/// Agent 全局目录 DTO（agent_dirs_list 返回项）
///
/// CP-024: ts-rs 生成 `src/types/agentDirs.ts`。
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/agentDirs.ts")]
pub struct AgentGlobalDir {
    /// CLI profile id（与前端 cliProfileRegistry 键对应）
    pub cli_id: String,
    /// 解析后的绝对路径（home + 相对目录；未 canonicalize——前端仅作展示与命令参数回传，
    /// 沙箱校验侧在 state.rs 内双侧 canonicalize 比较，symlink 场景兼容）
    pub path: String,
    /// 目录当前是否存在（false → 前端展示「目录不存在」占位，不启动监听）
    pub exists: bool,
}

/// 解析全部 agent 目录；home 不可解析 → None（调用方按安全默认处理）
fn resolve_agent_dirs() -> Option<Vec<AgentGlobalDir>> {
    let home = crate::home::home_dir()?;
    Some(
        AGENT_CONFIG_DIRS
            .iter()
            .map(|(cli_id, rel)| {
                let path = home.join(rel);
                AgentGlobalDir {
                    cli_id: (*cli_id).to_string(),
                    exists: path.is_dir(),
                    path: path.to_string_lossy().to_string(),
                }
            })
            .collect(),
    )
}

/// 沙箱放行用目录集（state.rs validate 调用）：
/// home 不可解析 → 空集（= 不放行，安全默认）
pub(crate) fn agent_dir_paths() -> Vec<PathBuf> {
    resolve_agent_dirs()
        .map(|dirs| dirs.into_iter().map(|d| PathBuf::from(d.path)).collect())
        .unwrap_or_default()
}

/// 列出全部 agent 全局配置目录（含不存在项——前端据此展示占位）
#[tauri::command]
pub async fn agent_dirs_list() -> Result<Vec<AgentGlobalDir>, AppError> {
    // exists 判定为元数据 syscall（阻塞 I/O），入 spawn_blocking（硬约束 #3）
    match tokio::task::spawn_blocking(|| {
        resolve_agent_dirs().ok_or_else(|| AppError::IoKind {
            kind: "path".into(),
            message: "无法解析用户主目录".into(),
        })
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

#[cfg(test)]
mod agent_dirs_tests {
    use super::*;
    use crate::home::HomeDirGuard;

    /// 静态表契约：当前仅 claude → .claude（新增 agent 须同步本用例）
    #[test]
    fn static_table_contains_claude_only() {
        assert_eq!(AGENT_CONFIG_DIRS.len(), 1, "当前应仅注册 claude 一个 agent");
        assert_eq!(AGENT_CONFIG_DIRS[0], ("claude", ".claude"));
    }

    /// HomeDirGuard 注入 tempdir：path 解析为 home/.claude，目录不存在 → exists=false
    #[test]
    fn resolve_marks_missing_dir_not_exists() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());

        let dirs = resolve_agent_dirs().unwrap();
        assert_eq!(dirs.len(), 1);
        let claude = &dirs[0];
        assert_eq!(claude.cli_id, "claude");
        assert_eq!(
            claude.path,
            dir.path().join(".claude").to_string_lossy(),
            "path 应为 home/.claude"
        );
        assert!(!claude.exists, "目录未创建时 exists 应为 false");
    }

    /// 目录创建后 exists=true
    #[test]
    fn resolve_marks_existing_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".claude")).unwrap();
        let _guard = HomeDirGuard::set(dir.path());

        let dirs = resolve_agent_dirs().unwrap();
        assert!(dirs[0].exists, "目录已创建时 exists 应为 true");
    }

    /// 沙箱目录集：经 HomeDirGuard 解析为 home/.claude 绝对路径
    #[test]
    fn agent_dir_paths_resolves_under_guarded_home() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());

        assert_eq!(agent_dir_paths(), vec![dir.path().join(".claude")]);
    }

    /// 命令层：真实 agent_dirs_list 返回静态表全量条目
    #[test]
    fn agent_dirs_list_command_returns_entries() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());

        let dirs = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(agent_dirs_list())
            .unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].cli_id, "claude");
        assert!(!dirs[0].exists, "假 home 下 .claude 未创建");
    }

    /// DTO 序列化为 camelCase（cliId/path/exists）
    #[test]
    fn agent_global_dir_serializes_camel_case() {
        let d = AgentGlobalDir {
            cli_id: "claude".into(),
            path: "C:\\x\\.claude".into(),
            exists: true,
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"cliId\":\"claude\""), "应含 cliId: {json}");
        assert!(json.contains("\"exists\":true"), "应含 exists: {json}");
        assert!(!json.contains("cli_id"), "不应含 snake_case 键: {json}");
    }
}
