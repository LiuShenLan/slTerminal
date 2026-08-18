/// 项目数据持久化模块 — save/load 项目数据到 exe 同级目录的 slterminal-projects.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。绕过路径 sandbox（项目数据是应用级元数据，非用户项目文件）。
/// - BE-14/D11：load 返回 `{ data, corrupted }`——无文件 data:"{}"/corrupted:false；
///   损坏回退默认值 corrupted:true；.bak 命中也算 corrupted:true（数据来自备份）。
/// - SEC-11：save 校验大小上限 1MB + 结构须为 JSON 对象。
/// - BE-16：应用数据目录解析/测试守卫/共享 DTO 自 app_dir 模块导入。
use crate::app_dir::{app_data_dir, LoadResult, MAX_PERSIST_BYTES};
use crate::error::{io_error, AppError};
use std::io::Write as _;
use std::path::Path;
use tempfile::NamedTempFile;

const PROJECTS_FILENAME: &str = "slterminal-projects.json";

// ── 可测试的 I/O 核心（接受显式 app_dir，供测试注入 tempdir） ──

/// 保存项目数据到指定目录（原子写入 + .bak 备份）
fn save_to_dir(app_dir: &Path, data: &str) -> Result<(), AppError> {
    let projects_path = app_dir.join(PROJECTS_FILENAME);
    // 保存链 io 错误统一经 io_error 语义化（BE-15）：用户可见「保存项目失败 + 路径」，
    // 原始 io 错误文本进 tracing 日志
    std::fs::create_dir_all(app_dir).map_err(|e| io_error("保存项目", app_dir, e))?;
    if projects_path.exists() {
        let bak = app_dir.join("slterminal-projects.json.bak");
        if let Err(e) = std::fs::copy(&projects_path, &bak) {
            tracing::warn!(error = %e, path = %projects_path.display(), "projects .bak 备份失败");
        }
    }
    let mut tmp = NamedTempFile::new_in(app_dir).map_err(|e| io_error("保存项目", app_dir, e))?;
    tmp.write_all(data.as_bytes())
        .map_err(|e| io_error("保存项目", &projects_path, e))?;
    tmp.flush()
        .map_err(|e| io_error("保存项目", &projects_path, e))?;
    tmp.persist(&projects_path)
        .map_err(|e| io_error("保存项目", &projects_path, e.error))?;
    Ok(())
}

/// .bak 兜底恢复：读取 .bak → 合法则写回主文件并返回内容（保留原兜底行为）
fn restore_from_bak(projects_path: &Path, bak: &Path) -> Option<String> {
    if let Ok(bak_content) = std::fs::read_to_string(bak) {
        if is_valid_json(&bak_content) {
            let _ = std::fs::write(projects_path, &bak_content);
            return Some(bak_content);
        }
    }
    None
}

/// 从指定目录加载项目数据（BE-14：返回 `{ data, corrupted }`）
///
/// - 主文件存在且为合法 JSON → data=原始字符串, corrupted:false
/// - 主文件损坏 → 尝试 .bak：命中 → data=.bak 内容, corrupted:true；未命中 → data:"{}", corrupted:true
/// - 主文件不存在 → 尝试 .bak（兜底逻辑保留）：命中 → data=.bak 内容, corrupted:true；
///   未命中 → data:"{}", corrupted:false
fn load_from_dir(app_dir: &Path) -> LoadResult<String> {
    let projects_path = app_dir.join(PROJECTS_FILENAME);
    let bak = app_dir.join("slterminal-projects.json.bak");

    match std::fs::read_to_string(&projects_path) {
        Ok(content) if is_valid_json(&content) => LoadResult {
            data: content,
            corrupted: false,
        },
        Ok(_) => match restore_from_bak(&projects_path, &bak) {
            // BE-14：bak 命中也算 corrupted（数据来自备份）
            Some(bak_content) => LoadResult {
                data: bak_content,
                corrupted: true,
            },
            None => LoadResult {
                data: "{}".to_string(),
                corrupted: true,
            },
        },
        Err(_) => match restore_from_bak(&projects_path, &bak) {
            Some(bak_content) => LoadResult {
                data: bak_content,
                corrupted: true,
            },
            None => LoadResult {
                data: "{}".to_string(),
                corrupted: false,
            },
        },
    }
}

/// 检查字符串是否为合法 JSON（仅校验格式，不解析业务结构）
fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

/// SEC-11：校验 projects 保存输入——大小上限 1MB + 须为合法 JSON 对象
fn validate_projects_input(data: &str) -> Result<(), AppError> {
    if data.len() > MAX_PERSIST_BYTES {
        return Err(AppError::Validation("保存项目失败: 数据超过 1MB 上限".into()));
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(serde_json::Value::Object(_)) => Ok(()),
        Ok(_) => Err(AppError::Validation(
            "保存项目失败: 项目数据必须是 JSON 对象".into(),
        )),
        Err(e) => {
            tracing::warn!(error = %e, "保存项目失败: 项目数据不是合法 JSON");
            Err(AppError::Validation(
                "保存项目失败: 项目数据不是合法 JSON".into(),
            ))
        }
    }
}

// ── Tauri 命令（外层：解析 exe 目录 → 委托 I/O 核心 → spawn_blocking） ──

/// 持久化项目数据（前端 JSON 字符串 → exe 同级 slterminal-projects.json）
///
/// SEC-11 保存校验：大小上限 + JSON 对象结构（spawn_blocking 前快速失败）。
#[tauri::command]
pub async fn save_projects(data: String) -> Result<(), AppError> {
    // SEC-11：大小上限 + 结构校验（纯内存操作，spawn_blocking 前快速失败）
    validate_projects_input(&data)?;

    let app_dir = app_data_dir()?;
    match tokio::task::spawn_blocking(move || save_to_dir(&app_dir, &data)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 加载持久化项目数据（exe 同级 slterminal-projects.json → `{ data, corrupted }`，BE-14）
#[tauri::command]
pub async fn load_projects() -> Result<LoadResult<String>, AppError> {
    let app_dir = app_data_dir()?;
    match tokio::task::spawn_blocking(move || load_from_dir(&app_dir)).await {
        Ok(inner) => Ok(inner),
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建 tokio runtime 并 block_on 调真实 Tauri 命令（与 fs/settings 模块命令层测试同模式）
    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    /// T1.1: save 后 load 往返一致（corrupted=false）
    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data = r#"{"projects":{"p1":{"name":"test"}}}"#;
        save_to_dir(dir.path(), data).unwrap();
        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, data);
        assert!(!loaded.corrupted, "正常保存后加载不应标记 corrupted");
    }

    /// T1.2: 文件不存在（无 .bak）→ "{}" / corrupted:false
    #[test]
    fn load_file_not_found_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_from_dir(dir.path());
        assert_eq!(result.data, "{}");
        assert!(!result.corrupted, "文件不存在不算损坏");
    }

    /// T1.3: 主文件损坏 → .bak 恢复（corrupted=true）
    #[test]
    fn load_corrupt_fallback_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid = r#"{"projects":{"p1":{"name":"recovered"}}}"#;
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        let bak_path = dir.path().join("slterminal-projects.json.bak");

        std::fs::write(&bak_path, valid).unwrap();
        std::fs::write(&projects_path, "not valid json {{{broken").unwrap();

        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, valid, "应从 .bak 恢复");
        assert!(loaded.corrupted, ".bak 命中也应标记 corrupted（数据来自备份）");
        // 验证主文件已被修复
        let repaired = std::fs::read_to_string(&projects_path).unwrap();
        assert_eq!(repaired, valid, "主文件应被修复为 .bak 内容");
    }

    /// T1.4: 主文件损坏且无 .bak → "{}" / corrupted:true
    #[test]
    fn load_corrupt_no_bak_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::write(&projects_path, "definitely not json {{{").unwrap();

        let result = load_from_dir(dir.path());
        assert_eq!(result.data, "{}");
        assert!(result.corrupted, "损坏文件应标记 corrupted");
    }

    /// BE-14：主文件不存在但 .bak 有效 → 兜底恢复 .bak（corrupted=true）
    #[test]
    fn load_missing_file_recovers_from_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid = r#"{"projects":{"p1":{"name":"bak-only"}}}"#;
        std::fs::write(dir.path().join("slterminal-projects.json.bak"), valid).unwrap();

        let result = load_from_dir(dir.path());
        assert_eq!(result.data, valid, "主文件缺失时应从 .bak 兜底恢复");
        assert!(result.corrupted, "数据来自备份应标记 corrupted");
    }

    /// T1.5: 目录不存在 → 自动创建并写入成功
    #[test]
    fn save_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub_dir = dir.path().join("config").join("slterminal");
        let data = r#"{"projects":{}}"#;
        save_to_dir(&sub_dir, data).unwrap();
        assert!(
            sub_dir.join(PROJECTS_FILENAME).exists(),
            "应自动创建目录并写入文件"
        );
    }

    /// T1.6: 覆盖已有文件
    #[test]
    fn save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"projects":{"old":"data"}}"#;
        let new = r#"{"projects":{"new":"data"}}"#;

        save_to_dir(dir.path(), old).unwrap();
        save_to_dir(dir.path(), new).unwrap();
        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, new, "应返回最新数据");
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

        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, v2, "应返回最新数据");

        let bak = std::fs::read_to_string(dir.path().join("slterminal-projects.json.bak")).unwrap();
        assert_eq!(bak, v1, ".bak 应为 v1");
    }

    /// T1.9: 空文件 → 内容为空 → 非合法 JSON → "{}" / corrupted:true
    #[test]
    fn load_empty_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::write(&projects_path, "").unwrap();

        let result = load_from_dir(dir.path());
        assert_eq!(result.data, "{}", "空文件非合法 JSON，应返回空对象");
        assert!(result.corrupted, "空文件应标记 corrupted");
    }

    /// T1.10: 主文件和 .bak 均损坏 → "{}" / corrupted:true
    #[test]
    fn load_bak_corrupt_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        let bak_path = dir.path().join("slterminal-projects.json.bak");

        std::fs::write(&projects_path, "not json ###").unwrap();
        std::fs::write(&bak_path, "also corrupt @@@").unwrap();

        let result = load_from_dir(dir.path());
        assert_eq!(result.data, "{}");
        assert!(result.corrupted, "主文件损坏应标记 corrupted");
    }

    /// 中文/emoji 数据往返
    #[test]
    fn save_load_unicode_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data = r#"{"projects":{"p1":{"name":"项目名称 🚀"}}}"#;
        save_to_dir(dir.path(), data).unwrap();
        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, data);
    }

    /// 大数据块往返（~100KB）
    #[test]
    fn save_load_large_data_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let payload = "x".repeat(5000);
        let data = format!(r#"{{"projects":{{"p1":{{"layout":"{}"}}}}}}"#, payload);
        save_to_dir(dir.path(), &data).unwrap();
        let loaded = load_from_dir(dir.path());
        assert_eq!(loaded.data, data);
    }

    // ── SEC-11: 保存校验（大小上限 + JSON 对象结构） ──

    /// 保存超 1MB → AppError::Validation 拒绝且不落盘
    #[test]
    fn save_rejects_over_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());

        let big = format!(
            r#"{{"projects":{{"blob":"{}"}}}}"#,
            "x".repeat(MAX_PERSIST_BYTES)
        );
        let err = run(save_projects(big)).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("1MB"), "消息应提示大小上限，实际: {msg}");
            }
            other => panic!("超限应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join(PROJECTS_FILENAME).exists(),
            "超限保存不应落盘"
        );
    }

    /// projects 须为 JSON 对象——数组/标量 → AppError::Validation 拒绝
    #[test]
    fn save_rejects_non_object() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());

        let err = run(save_projects("[1,2,3]".to_string())).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("JSON 对象"), "消息应提示对象要求，实际: {msg}");
            }
            other => panic!("数组应返回 Validation，实际: {other:?}"),
        }
        let err2 = run(save_projects("123".to_string())).unwrap_err();
        assert!(matches!(err2, AppError::Validation(_)), "标量应被拒绝: {err2:?}");
    }

    /// 非合法 JSON → AppError::Validation 拒绝
    #[test]
    fn save_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());

        let err = run(save_projects("not json {{{".to_string())).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("合法 JSON"), "消息应提示 JSON 格式，实际: {msg}");
            }
            other => panic!("非法 JSON 应返回 Validation，实际: {other:?}"),
        }
    }

    // ── SPE-02: 命令包装层测试（block_on 调真实 save_projects/load_projects，app_data_dir 注入 tempdir） ──
    // 旧测试只调 I/O 核心 save_to_dir/load_from_dir，命令包装层（app_data_dir → spawn_blocking → TaskJoin）从未被调用。
    // 现经 crate::app_dir::AppDataDirGuard 注入 tempdir，block_on 走真实命令路径。
    // 注：TaskJoin 分支（spawn_blocking panic）无法经命令注入构造，由 error.rs 的 From<JoinError> 用例（SPE-03）覆盖。

    /// 命令层：save → load 往返一致（corrupted=false）
    #[test]
    fn command_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());
        let data = r#"{"projects":{"p1":{"name":"test"}}}"#;
        run(save_projects(data.to_string())).unwrap();
        let loaded = run(load_projects()).unwrap();
        assert_eq!(loaded.data, data);
        assert!(!loaded.corrupted);
    }

    /// 命令层：文件不存在 → "{}" / corrupted:false
    #[test]
    fn command_load_file_not_found_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());
        let result = run(load_projects()).unwrap();
        assert_eq!(result.data, "{}");
        assert!(!result.corrupted, "文件不存在不算损坏");
    }

    /// 命令层：主文件损坏 → .bak 恢复（corrupted=true 且主文件被修复）
    #[test]
    fn command_load_corrupt_fallback_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid = r#"{"projects":{"p1":{"name":"recovered"}}}"#;
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::write(dir.path().join("slterminal-projects.json.bak"), valid).unwrap();
        std::fs::write(&projects_path, "not valid json {{{broken").unwrap();

        let _guard = crate::app_dir::AppDataDirGuard::set(dir.path());
        let loaded = run(load_projects()).unwrap();
        assert_eq!(loaded.data, valid, "应从 .bak 恢复");
        assert!(loaded.corrupted, ".bak 命中应标记 corrupted");
        assert_eq!(
            std::fs::read_to_string(&projects_path).unwrap(),
            valid,
            "主文件应被修复为 .bak 内容"
        );
    }

    /// 命令层：目录不存在时 save 自动创建（经 spawn_blocking 执行）
    #[test]
    fn command_save_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub_dir = dir.path().join("config").join("slterminal");
        let _guard = crate::app_dir::AppDataDirGuard::set(&sub_dir);
        run(save_projects(r#"{"projects":{}}"#.to_string())).unwrap();
        assert!(
            sub_dir.join(PROJECTS_FILENAME).exists(),
            "应自动创建目录并写入文件"
        );
    }

    // ── SPE-05: persist 失败映射 ──

    /// persist 目标为已存在目录（冲突构造）→ 映射为 AppError::IoKind（消息含「保存项目失败」+ 路径）
    #[test]
    fn save_to_dir_persist_failure_maps_to_io_error() {
        let dir = tempfile::tempdir().unwrap();
        // 目标路径 slterminal-projects.json 为已存在目录 → rename 替换必然失败
        let projects_path = dir.path().join(PROJECTS_FILENAME);
        std::fs::create_dir(&projects_path).unwrap();

        let err = save_to_dir(dir.path(), r#"{"projects":{}}"#).unwrap_err();
        match err {
            AppError::IoKind { kind, message } => {
                assert!(!kind.is_empty(), "kind 不应为空");
                assert!(
                    message.contains("保存项目失败"),
                    "消息应含业务语义 '保存项目失败'，实际: {message}"
                );
                assert!(
                    message.contains(&projects_path.to_string_lossy().to_string()),
                    "消息应含项目文件路径，实际: {message}"
                );
            }
            other => panic!("persist 失败应映射为 AppError::IoKind，实际: {other:?}"),
        }
    }
}
