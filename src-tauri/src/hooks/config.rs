//! Hooks 配置三层读写模块 — hooks 子树级 read-modify-write（P3-BE-01/02/03）
//!
//! 三层配置路径（P3-BE-01）：
//! - user 层 → `~/.claude/settings.json`（`dirs::home_dir()` 解析，绕过 project_root 沙箱）
//! - project 层 → `<projectPath>/.claude/settings.json`
//! - local 层 → `<projectPath>/.claude/settings.local.json`
//!
//! project/local 层入参经 `crate::state::validate_path_within_root` 沙箱校验：
//! project_path 缺失返回 Validation，校验失败返回 PathNotAllowed（P3-BE-06/07）。
//! 非法 layer / 非法 hooks / JSON 损坏统一走 AppError::Validation，IO 错误走
//! AppError::IoKind（P3-BE-08）。阻塞 I/O 全部在 spawn_blocking 内执行（硬约束 #3）。

use crate::error::AppError;
use crate::state::validate_path_within_root;
use crate::state::AppState;
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tauri::State;
use tempfile::NamedTempFile;

/// hooks 配置层级
#[derive(Debug, Clone, Copy, PartialEq)]
enum HooksLayer {
    /// 用户全局层 `~/.claude/settings.json`
    User,
    /// 项目共享层 `<projectPath>/.claude/settings.json`
    Project,
    /// 项目本地层 `<projectPath>/.claude/settings.local.json`
    Local,
}

/// 解析层级字符串，仅允许 "user" / "project" / "local"，非法返回 Validation（P3-BE-02）
fn parse_layer(layer: &str) -> Result<HooksLayer, AppError> {
    match layer {
        "user" => Ok(HooksLayer::User),
        "project" => Ok(HooksLayer::Project),
        "local" => Ok(HooksLayer::Local),
        _ => Err(AppError::Validation(format!(
            "非法 hooks 配置层级: {layer}"
        ))),
    }
}

/// 各层配置文件名（project 与 user 同名，local 独立）
fn layer_file_name(layer: HooksLayer) -> &'static str {
    match layer {
        HooksLayer::User | HooksLayer::Project => "settings.json",
        HooksLayer::Local => "settings.local.json",
    }
}

/// 解析三层配置路径（纯函数，不做文件 IO）
///
/// - user 层：home_dir()/.claude/settings.json，命令体内不调用沙箱校验（P3-BE-06）
/// - project/local 层：project_path 缺失 → Validation；经 validate_path_within_root
///   沙箱校验，未通过 → PathNotAllowed；通过后拼接 .claude/ 下的配置文件名（P3-BE-07）
fn resolve_config_path(
    layer: HooksLayer,
    project_root: &Option<PathBuf>,
    project_path: Option<&str>,
) -> Result<PathBuf, AppError> {
    match layer {
        HooksLayer::User => {
            let home = dirs::home_dir().ok_or_else(|| AppError::IoKind {
                kind: "home_dir".into(),
                message: "无法解析用户主目录".into(),
            })?;
            Ok(home.join(".claude").join(layer_file_name(layer)))
        }
        HooksLayer::Project | HooksLayer::Local => {
            let pp = project_path.ok_or_else(|| {
                AppError::Validation("project/local 层必须提供 projectPath".into())
            })?;
            validate_path_within_root(project_root, Path::new(pp))
                .map_err(|_| AppError::PathNotAllowed(pp.to_string()))?;
            Ok(PathBuf::from(pp)
                .join(".claude")
                .join(layer_file_name(layer)))
        }
    }
}

/// 读取配置文件并提取 hooks 子树（P3-BE-02 纯逻辑）
///
/// - 文件不存在 → Ok(Null)（面板首次创建场景）
/// - 文件合法但无 hooks 键 → Ok(Null)
/// - JSON 损坏 → Err(Validation)，不返回 Null——防止面板在损坏文件上编辑后
///   merge 丢其他字段（对齐 C9 注入的非法中止先例）
fn read_hooks_subtree(path: &Path) -> Result<Value, AppError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(e) => return Err(e.into()),
    };
    let root: Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Validation(format!("配置文件 JSON 损坏: {e}")))?;
    Ok(root.get("hooks").cloned().unwrap_or(Value::Null))
}

/// 写回 hooks 子树（read-modify-write merge，P3-BE-03 纯逻辑）
///
/// - hooks 必须为 JSON 对象，否则 Validation
/// - 读原文件：不存在视为空对象 {}；损坏 → Err 拒绝覆盖用户文件
/// - 根对象 hooks 键替换为入参，其余字段（permissions/env/$schema 等）原样保留
/// - 父目录不存在时自动 create_dir_all（仅写入路径，P3-BE-01）
/// - 原子写：NamedTempFile → write_all → flush → persist（照 settings.rs 先例，不做 .bak）
fn write_hooks_subtree(path: &Path, hooks: Value) -> Result<(), AppError> {
    if !hooks.is_object() {
        return Err(AppError::Validation("hooks 必须为 JSON 对象".into()));
    }
    // 父目录自动创建（写入路径）
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    // 读原文件（read-modify-write 的 read 阶段）
    let mut root: Value = match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|e| AppError::Validation(format!("配置文件 JSON 损坏，拒绝覆盖: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e.into()),
    };
    // 根元素须为对象才能安全 merge；Null（文件内容为 null）视作空对象
    if root.is_null() {
        root = serde_json::json!({});
    }
    match root.as_object_mut() {
        Some(obj) => {
            obj.insert("hooks".into(), hooks);
        }
        None => {
            // 根元素为数组/标量时无法安全 merge，拒绝覆盖
            return Err(AppError::Validation(
                "配置文件根元素不是 JSON 对象，拒绝覆盖".into(),
            ));
        }
    }

    // 原子写：tempfile → write_all → flush → persist（明确不做 .bak）
    let json = serde_json::to_string_pretty(&root)?;
    let mut tmp = NamedTempFile::new_in(parent)?;
    tmp.write_all(json.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| AppError::IoKind {
        kind: format!("{:?}", e.error.kind()),
        message: format!("persist 失败: {e}"),
    })?;
    Ok(())
}

/// 读取 hooks 配置子树（P3-BE-02）
///
/// 返回 hooks 子树（非整文件）；文件不存在或无 hooks 键返回 Null。
#[tauri::command]
pub async fn hooks_config_read(
    layer: String,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<Value, AppError> {
    let l = parse_layer(&layer)?;
    // 沙箱校验 + 路径解析（锁内读取 project_root；user 层不经过沙箱）
    // 作用域块：块结束即 drop 锁守卫，避免非 Send 的 RwLockReadGuard 跨 await 存活
    let path = {
        let root_guard = state.project_root.read().map_err(|e| AppError::IoKind {
            kind: "lock".into(),
            message: format!("获取 project_root 锁失败: {e}"),
        })?;
        resolve_config_path(l, &root_guard, project_path.as_deref())?
    };

    // 阻塞 I/O 在 spawn_blocking 内执行（硬约束 #3）
    match tokio::task::spawn_blocking(move || read_hooks_subtree(&path)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 写回 hooks 配置子树（read-modify-write merge，P3-BE-03）
///
/// 要求 hooks 为 JSON Object；原文件其他字段（permissions/env/$schema）原样保留。
#[tauri::command]
pub async fn hooks_config_write(
    layer: String,
    hooks: Value,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let l = parse_layer(&layer)?;
    if !hooks.is_object() {
        return Err(AppError::Validation("hooks 必须为 JSON 对象".into()));
    }
    // 沙箱校验 + 路径解析（锁内读取 project_root；user 层不经过沙箱）
    // 作用域块：块结束即 drop 锁守卫，避免非 Send 的 RwLockReadGuard 跨 await 存活
    let path = {
        let root_guard = state.project_root.read().map_err(|e| AppError::IoKind {
            kind: "lock".into(),
            message: format!("获取 project_root 锁失败: {e}"),
        })?;
        resolve_config_path(l, &root_guard, project_path.as_deref())?
    };

    // 阻塞 I/O 在 spawn_blocking 内执行（硬约束 #3）
    match tokio::task::spawn_blocking(move || write_hooks_subtree(&path, hooks)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_layer（P3-BE-08 非法 layer → Validation） ──

    #[test]
    fn parse_layer_accepts_three_layers() {
        assert_eq!(parse_layer("user").unwrap(), HooksLayer::User);
        assert_eq!(parse_layer("project").unwrap(), HooksLayer::Project);
        assert_eq!(parse_layer("local").unwrap(), HooksLayer::Local);
    }

    #[test]
    fn parse_layer_rejects_invalid() {
        // 非法值、大小写不匹配、空串均返回 Validation
        assert!(matches!(parse_layer("bogus"), Err(AppError::Validation(_))));
        assert!(matches!(parse_layer("User"), Err(AppError::Validation(_))));
        assert!(matches!(parse_layer(""), Err(AppError::Validation(_))));
    }

    // ── resolve_config_path（P3-TE-02 路径解析 + 沙箱） ──

    #[test]
    fn user_layer_resolves_to_home_claude_settings() {
        // user 层指向 ~/.claude/settings.json，不依赖 project_path / 沙箱
        let path = resolve_config_path(HooksLayer::User, &None, None).unwrap();
        assert_eq!(path.file_name().unwrap(), "settings.json");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), ".claude");
    }

    #[test]
    fn project_layer_resolves_inside_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = Some(dir.path().to_path_buf());
        let proj = dir.path().to_str().unwrap();
        let path = resolve_config_path(HooksLayer::Project, &root, Some(proj)).unwrap();
        assert_eq!(
            path,
            PathBuf::from(proj).join(".claude").join("settings.json")
        );
    }

    #[test]
    fn local_layer_resolves_to_settings_local_json() {
        let dir = tempfile::tempdir().unwrap();
        let root = Some(dir.path().to_path_buf());
        let proj = dir.path().to_str().unwrap();
        let path = resolve_config_path(HooksLayer::Local, &root, Some(proj)).unwrap();
        assert_eq!(
            path,
            PathBuf::from(proj)
                .join(".claude")
                .join("settings.local.json")
        );
    }

    #[test]
    fn project_local_missing_project_path_validation() {
        // project/local 层缺失 project_path → Validation（P3-BE-07）
        let err = resolve_config_path(HooksLayer::Project, &None, None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        let err = resolve_config_path(HooksLayer::Local, &None, None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn project_layer_outside_root_path_not_allowed() {
        // 沙箱校验失败分支：project_path 在 project_root 子树外 → PathNotAllowed
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = Some(inside.path().to_path_buf());
        let err = resolve_config_path(
            HooksLayer::Project,
            &root,
            Some(outside.path().to_str().unwrap()),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::PathNotAllowed(_)));
    }

    // ── read_hooks_subtree（P3-TE-01 读取分支） ──

    #[test]
    fn read_file_not_found_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude").join("settings.json");
        let v = read_hooks_subtree(&path).unwrap();
        assert!(v.is_null(), "文件不存在应返回 Null");
    }

    #[test]
    fn read_no_hooks_key_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"permissions":{"allow":["Bash"]}}"#).unwrap();
        let v = read_hooks_subtree(&path).unwrap();
        assert!(v.is_null(), "无 hooks 键应返回 Null");
    }

    #[test]
    fn read_returns_hooks_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let hooks = serde_json::json!({
            "PreToolUse": [{"matcher": "", "hooks": [{"type": "command", "command": "node x"}]}]
        });
        std::fs::write(
            &path,
            serde_json::to_string(&serde_json::json!({"hooks": hooks, "env": {}})).unwrap(),
        )
        .unwrap();
        let v = read_hooks_subtree(&path).unwrap();
        assert_eq!(v, hooks, "应返回 hooks 子树而非整文件");
    }

    #[test]
    fn read_corrupt_json_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "not json {{{").unwrap();
        let err = read_hooks_subtree(&path).unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "损坏 JSON read 应返回 Err 而非 Null（防止损坏文件上编辑后 merge 丢字段）"
        );
    }

    // ── write_hooks_subtree（P3-TE-01 原子写 + P3-TE-02 merge/拒绝/校验） ──

    #[test]
    fn write_atomic_content_correct() {
        // 文件不存在视为空对象 {}，原子写后内容正确
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let hooks = serde_json::json!({
            "SessionStart": [{"hooks": [{"type": "command", "command": "echo hi"}]}]
        });
        write_hooks_subtree(&path, hooks.clone()).unwrap();
        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reloaded, serde_json::json!({"hooks": hooks}));
    }

    #[test]
    fn write_auto_creates_parent_dirs() {
        // 父目录不存在时自动 create_dir_all（仅写入路径，P3-BE-01）
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("a")
            .join("b")
            .join(".claude")
            .join("settings.json");
        write_hooks_subtree(&path, serde_json::json!({"Stop": []})).unwrap();
        assert!(path.exists(), "写入路径的父目录应自动创建");
    }

    #[test]
    fn write_preserves_other_root_fields() {
        // merge 保留：permissions/env/$schema 等其他字段原样保留（P3-TE-02）
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"$schema":"https://json.schemastore.org/claude-code-settings.json","permissions":{"allow":["Bash"]},"env":{"FOO":"bar"}}"#,
        )
        .unwrap();
        let hooks = serde_json::json!({
            "PostToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "node x"}]}]
        });
        write_hooks_subtree(&path, hooks.clone()).unwrap();
        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            reloaded["$schema"],
            "https://json.schemastore.org/claude-code-settings.json"
        );
        assert_eq!(
            reloaded["permissions"]["allow"],
            serde_json::json!(["Bash"])
        );
        assert_eq!(reloaded["env"]["FOO"], "bar");
        // hooks 键替换为入参
        assert_eq!(reloaded["hooks"], hooks);
    }

    #[test]
    fn write_corrupt_json_rejected_and_not_overwritten() {
        // 损坏 JSON write 返回 Err，且原文件不被覆盖（P3-TE-02）
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let corrupt = "not json {{{";
        std::fs::write(&path, corrupt).unwrap();
        let err = write_hooks_subtree(&path, serde_json::json!({"Stop": []})).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            corrupt,
            "损坏文件不应被覆盖"
        );
    }

    #[test]
    fn write_non_object_hooks_validation() {
        // 非 Object hooks → Validation（P3-BE-03）
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let err = write_hooks_subtree(&path, serde_json::json!([1, 2])).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        let err = write_hooks_subtree(&path, serde_json::json!("str")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        // 校验失败在创建目录/文件之前，不应产生任何副作用
        assert!(!path.exists(), "非 Object hooks 拒绝时不应创建文件");
    }

    #[test]
    fn write_non_object_root_rejected() {
        // 根元素为数组时无法安全 merge → Validation，不覆盖原文件
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "[1,2,3]").unwrap();
        let err = write_hooks_subtree(&path, serde_json::json!({"Stop": []})).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1,2,3]");
    }

    #[test]
    fn write_null_root_treated_as_empty_object() {
        // 文件内容为 null（合法 JSON）视作空对象，merge 后仅含 hooks 键
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "null").unwrap();
        let hooks = serde_json::json!({"Stop": []});
        write_hooks_subtree(&path, hooks.clone()).unwrap();
        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reloaded, serde_json::json!({"hooks": hooks}));
    }
}
