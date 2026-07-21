/// 项目数据持久化模块 — save/load 项目数据到 exe 同级目录的 slterminal-projects.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。绕过路径 sandbox（项目数据是应用级元数据，非用户项目文件）。
use crate::error::AppError;
use crate::settings::app_data_dir;
use std::io::Write as _;
use std::path::Path;
use tempfile::NamedTempFile;

const PROJECTS_FILENAME: &str = "slterminal-projects.json";

// ── 可测试的 I/O 核心（接受显式 app_dir，供测试注入 tempdir） ──

/// 保存项目数据到指定目录（原子写入 + .bak 备份）
fn save_to_dir(app_dir: &Path, data: &str) -> Result<(), AppError> {
    let projects_path = app_dir.join(PROJECTS_FILENAME);
    std::fs::create_dir_all(app_dir)?;
    if projects_path.exists() {
        let bak = app_dir.join("slterminal-projects.json.bak");
        let _ = std::fs::copy(&projects_path, &bak);
    }
    let mut tmp = NamedTempFile::new_in(app_dir)?;
    tmp.write_all(data.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&projects_path).map_err(|e| AppError::IoKind {
        kind: format!("{:?}", e.error.kind()),
        message: format!("persist 失败: {e}"),
    })?;
    Ok(())
}

/// 从指定目录加载项目数据（JSON 校验 + .bak 恢复）
///
/// - 文件存在且为合法 JSON → 返回原始字符串
/// - 文件存在但 JSON 损坏 → 尝试 .bak → 仍失败/不存在 → "{}"
/// - 文件不存在 → 尝试 .bak → 仍失败/不存在 → "{}"
fn load_from_dir(app_dir: &Path) -> Result<String, AppError> {
    let projects_path = app_dir.join(PROJECTS_FILENAME);
    let bak = app_dir.join("slterminal-projects.json.bak");

    match std::fs::read_to_string(&projects_path) {
        Ok(content) if is_valid_json(&content) => Ok(content),
        Ok(_) | Err(_) => {
            // 主文件损坏或不存在 → 尝试 .bak
            if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                if is_valid_json(&bak_content) {
                    let _ = std::fs::write(&projects_path, &bak_content);
                    return Ok(bak_content);
                }
            }
            Ok("{}".to_string())
        }
    }
}

/// 检查字符串是否为合法 JSON（仅校验格式，不解析业务结构）
fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

// ── Tauri 命令（外层：解析 exe 目录 → 委托 I/O 核心 → spawn_blocking） ──

/// 持久化项目数据（前端 JSON 字符串 → exe 同级 slterminal-projects.json）
#[tauri::command]
pub async fn save_projects(data: String) -> Result<(), AppError> {
    let app_dir = app_data_dir()?;
    match tokio::task::spawn_blocking(move || save_to_dir(&app_dir, &data)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 加载持久化项目数据（exe 同级 slterminal-projects.json → 前端 JSON 字符串）
#[tauri::command]
pub async fn load_projects() -> Result<String, AppError> {
    let app_dir = app_data_dir()?;
    match tokio::task::spawn_blocking(move || load_from_dir(&app_dir)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    /// T1.1: save 后 load 往返一致
    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data = r#"{"projects":{"p1":{"name":"test"}}}"#;
        save_to_dir(dir.path(), data).unwrap();
        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, data);
    }

    /// T1.2: 文件不存在 → "{}"
    #[test]
    fn load_file_not_found_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_from_dir(dir.path()).unwrap();
        assert_eq!(result, "{}");
    }

    /// T1.3: 主文件损坏 → .bak 恢复
    #[test]
    fn load_corrupt_fallback_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid = r#"{"projects":{"p1":{"name":"recovered"}}}"#;
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        let bak_path = dir.path().join("slterminal-projects.json.bak");

        std::fs::write(&bak_path, valid).unwrap();
        std::fs::write(&projects_path, "not valid json {{{broken").unwrap();

        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, valid, "应从 .bak 恢复");
        // 验证主文件已被修复
        let repaired = std::fs::read_to_string(&projects_path).unwrap();
        assert_eq!(repaired, valid, "主文件应被修复为 .bak 内容");
    }

    /// T1.4: 主文件损坏且无 .bak → "{}"
    #[test]
    fn load_corrupt_no_bak_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::write(&projects_path, "definitely not json {{{").unwrap();

        let result = load_from_dir(dir.path()).unwrap();
        assert_eq!(result, "{}");
    }

    /// T1.5: 目录不存在 → 自动创建并写入成功
    #[test]
    fn save_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub_dir = dir.path().join("config").join("slterminal");
        let data = r#"{"projects":{}}"#;
        save_to_dir(&sub_dir, data).unwrap();
        assert!(sub_dir.join(PROJECTS_FILENAME).exists(), "应自动创建目录并写入文件");
    }

    /// T1.6: 覆盖已有文件
    #[test]
    fn save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"projects":{"old":"data"}}"#;
        let new = r#"{"projects":{"new":"data"}}"#;

        save_to_dir(dir.path(), old).unwrap();
        save_to_dir(dir.path(), new).unwrap();
        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, new, "应返回最新数据");
    }

    /// T1.7: save 时旧文件存在 → 创建 .bak
    #[test]
    fn save_creates_bak() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"projects":{"v1":"old"}}"#;
        let new = r#"{"projects":{"v2":"new"}}"#;

        save_to_dir(dir.path(), old).unwrap();
        save_to_dir(dir.path(), new).unwrap();

        let bak_path = dir.path().join("slterminal-projects.json.bak");
        assert!(bak_path.exists(), "应创建 .bak 备份");
        let bak_content = std::fs::read_to_string(&bak_path).unwrap();
        assert_eq!(bak_content, old, ".bak 应为旧内容");
    }

    /// T1.8: 两次 save → load 返回最新，.bak 为上上次
    #[test]
    fn save_then_save_again_loads_latest() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = r#"{"version":1}"#;
        let v2 = r#"{"version":2}"#;

        save_to_dir(dir.path(), v1).unwrap();
        save_to_dir(dir.path(), v2).unwrap();

        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, v2, "应返回最新数据");

        let bak = std::fs::read_to_string(dir.path().join("slterminal-projects.json.bak")).unwrap();
        assert_eq!(bak, v1, ".bak 应为 v1");
    }

    /// T1.9: 空文件 → 内容为空 → 非合法 JSON → "{}"
    #[test]
    fn load_empty_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::write(&projects_path, "").unwrap();

        let result = load_from_dir(dir.path()).unwrap();
        assert_eq!(result, "{}", "空文件非合法 JSON，应返回空对象");
    }

    /// T1.10: 主文件和 .bak 均损坏 → "{}"
    #[test]
    fn load_bak_corrupt_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        let bak_path = dir.path().join("slterminal-projects.json.bak");

        std::fs::write(&projects_path, "not json ###").unwrap();
        std::fs::write(&bak_path, "also corrupt @@@").unwrap();

        let result = load_from_dir(dir.path()).unwrap();
        assert_eq!(result, "{}");
    }

    /// 中文/emoji 数据往返
    #[test]
    fn save_load_unicode_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data = r#"{"projects":{"p1":{"name":"项目名称 🚀"}}}"#;
        save_to_dir(dir.path(), data).unwrap();
        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, data);
    }

    /// 大数据块往返（~100KB）
    #[test]
    fn save_load_large_data_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let payload = "x".repeat(5000);
        let data = format!(r#"{{"projects":{{"p1":{{"layout":"{}"}}}}}}"#, payload);
        save_to_dir(dir.path(), &data).unwrap();
        let loaded = load_from_dir(dir.path()).unwrap();
        assert_eq!(loaded, data);
    }
}
