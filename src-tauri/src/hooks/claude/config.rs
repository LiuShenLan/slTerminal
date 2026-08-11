//! claude hooks provider 配置三层读写 — hooks 子树级 read-modify-write（MC-213 下沉，P3-BE-01/02/03）
//!
//! 三层配置路径（P3-BE-01）：
//! - user 层 → `~/.claude/settings.json`（home 解析，绕过 project_root 沙箱）
//! - project 层 → `<projectPath>/.claude/settings.json`
//! - local 层 → `<projectPath>/.claude/settings.local.json`
//!
//! project/local 层入参经 `crate::state::validate_path_within_root` 沙箱校验：
//! project_path 缺失返回 Validation，校验失败返回 PathNotAllowed（P3-BE-06/07）。
//! 非法 layer / 非法 hooks / JSON 损坏统一走 AppError::Validation，IO 错误走
//! AppError::IoKind（P3-BE-08）。阻塞 I/O 由命令层在 spawn_blocking 内执行（硬约束 #3）。

use crate::error::AppError;
use crate::state::validate_path_within_root;
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
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
/// - user 层：home_dir 解析闭包（生产传 dirs::home_dir；测试注入 tempdir，
///   杜绝真实 home 依赖，HUK-07）/.claude/settings.json，命令体内不调用沙箱校验
///   （P3-BE-06）；闭包返回 None → IoKind（home_dir 解析失败分支）
/// - project/local 层：project_path 缺失 → Validation；经 validate_path_within_root
///   沙箱校验，未通过 → PathNotAllowed；通过后拼接 .claude/ 下的配置文件名（P3-BE-07）
fn resolve_config_path(
    layer: HooksLayer,
    project_root: &Option<PathBuf>,
    project_path: Option<&str>,
    home_dir: impl Fn() -> Option<PathBuf>,
) -> Result<PathBuf, AppError> {
    match layer {
        HooksLayer::User => {
            let home = home_dir().ok_or_else(|| AppError::IoKind {
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
/// - hooks 必须为 JSON 对象，否则 Validation（调用方 config_write_sync 已把 null
///   归一为空对象，ZQ-5——此处收到的恒为 object）
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

/// agent_hooks_config_read（claude）同步核心（provider trait impl 经命令层 spawn_blocking 调用）
///
/// project_root 由命令层从 AppState 锁内读取后传入；home_dir 闭包供 user 层解析
/// （生产传 claude::home_dir，测试注入 tempdir——L1 绝不读写真实用户 home）。
pub(crate) fn config_read_sync(
    layer: &str,
    project_path: Option<&str>,
    project_root: &Option<PathBuf>,
    home_dir: impl Fn() -> Option<PathBuf>,
) -> Result<Value, AppError> {
    let l = parse_layer(layer)?;
    // 路径解析（user 层不经过沙箱；project/local 层沙箱校验 + 拼接）
    let path = resolve_config_path(l, project_root, project_path, home_dir)?;
    read_hooks_subtree(&path)
}

/// agent_hooks_config_write（claude）同步核心（provider trait impl 经命令层 spawn_blocking 调用）
///
/// hooks 为 null 视作空对象 {} 进行 merge（ZQ-5 决策 3——与 read 返回 null 对称，
/// 语义 = 清空该层 hooks）；非 null 且非 object → Validation；
/// 原文件其他字段（permissions/env/$schema）原样保留。
pub(crate) fn config_write_sync(
    layer: &str,
    hooks: Value,
    project_path: Option<&str>,
    project_root: &Option<PathBuf>,
    home_dir: impl Fn() -> Option<PathBuf>,
) -> Result<(), AppError> {
    let l = parse_layer(layer)?;
    // 入口校验（ZQ-5 决策 3）：hooks 入参 null 视作空对象 {}（语义 = 清空该层 hooks），
    // 非 null 且非 object 才拒绝——write_hooks_subtree 的 is_object 闸门保持不变（收到的恒为 object）
    let hooks = if hooks.is_null() {
        serde_json::json!({})
    } else {
        hooks
    };
    if !hooks.is_object() {
        return Err(AppError::Validation("hooks 必须为 JSON 对象".into()));
    }
    // 路径解析（user 层不经过沙箱；project/local 层沙箱校验 + 拼接）
    let path = resolve_config_path(l, project_root, project_path, home_dir)?;
    write_hooks_subtree(&path, hooks)
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
    //
    // home_dir 闭包为可注入参数（HUK-07）：user 层测试注入 tempdir，
    // 不依赖真实 dirs::home_dir()，杜绝环境污染；project/local 层传
    // dirs::home_dir（惰性闭包，不触发，无真实 home 读取）。

    #[test]
    fn user_layer_resolves_to_injected_home_dir() {
        // user 层指向 {注入 home}/.claude/settings.json，不依赖 project_path / 沙箱
        let home = tempfile::tempdir().unwrap();
        let path = resolve_config_path(HooksLayer::User, &None, None, || {
            Some(home.path().to_path_buf())
        })
        .unwrap();
        assert_eq!(
            path,
            home.path().join(".claude").join("settings.json"),
            "user 层应使用注入的 home 目录解析完整路径"
        );
    }

    #[test]
    fn user_layer_home_dir_failure_returns_io_kind() {
        // home 解析失败（闭包返回 None）→ IoKind（HUK-06 注入失败点）
        let err = resolve_config_path(HooksLayer::User, &None, None, || None).unwrap_err();
        assert!(matches!(err, AppError::IoKind { .. }));
    }

    #[test]
    fn project_layer_resolves_inside_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = Some(dir.path().to_path_buf());
        let proj = dir.path().to_str().unwrap();
        let path =
            resolve_config_path(HooksLayer::Project, &root, Some(proj), dirs::home_dir).unwrap();
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
        let path =
            resolve_config_path(HooksLayer::Local, &root, Some(proj), dirs::home_dir).unwrap();
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
        let err =
            resolve_config_path(HooksLayer::Project, &None, None, dirs::home_dir).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        let err = resolve_config_path(HooksLayer::Local, &None, None, dirs::home_dir).unwrap_err();
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
            dirs::home_dir,
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

    #[test]
    fn read_io_error_returns_io_kind() {
        // 读取目标为目录（非 NotFound 的 IO 错误）→ IoKind（HUK-06 94 行分支）
        let dir = tempfile::tempdir().unwrap();
        let err = read_hooks_subtree(dir.path()).unwrap_err();
        assert!(matches!(err, AppError::IoKind { .. }));
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

    // ── IO 异常分支（HUK-06） ──

    #[test]
    fn write_read_io_error_returns_io_kind() {
        // 写回阶段读原文件失败（目标为目录，非 NotFound 的 IO 错误）→ IoKind（HUK-06 121 行分支）
        let dir = tempfile::tempdir().unwrap();
        let err = write_hooks_subtree(dir.path(), serde_json::json!({"Stop": []})).unwrap_err();
        assert!(matches!(err, AppError::IoKind { .. }));
    }

    #[test]
    fn write_persist_failure_returns_io_kind() {
        // persist（rename 覆盖）失败：目标文件为只读 → IoKind（HUK-06 145-147 分支）
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();

        let err = write_hooks_subtree(&path, serde_json::json!({"Stop": []})).unwrap_err();
        assert!(matches!(err, AppError::IoKind { .. }));

        // 恢复可写，保证 tempdir 清理不因只读文件失败
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    // ── 同步核心透传（HUK-06，config_read_sync / config_write_sync） ──
    //
    // 覆盖 layer / project_path / hooks / project_root 参数透传与返回映射；
    // user 层经 home_dir 闭包注入 tempdir，绝不读写真实用户 home。
    // 命令层 cliId 透传/未知 cliId 由 hooks::mod 命令层测试覆盖（block_on 直测）。

    #[test]
    fn config_read_sync_user_layer_passes_paths_through() {
        // user 层：注入 home tempdir，透传 layer 后读回 hooks 子树
        let home = tempfile::tempdir().unwrap();
        let settings = home.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let hooks = serde_json::json!({"PreToolUse": [1]});
        std::fs::write(
            &settings,
            serde_json::to_string(&serde_json::json!({"hooks": hooks, "env": {}})).unwrap(),
        )
        .unwrap();
        let home_path = home.path().to_path_buf();

        let v = config_read_sync("user", None, &None, move || Some(home_path.clone())).unwrap();
        assert_eq!(v, hooks, "user 层应透传注入 home 并返回 hooks 子树");
    }

    #[test]
    fn config_read_sync_project_layer_passes_paths_through() {
        // project 层：透传 project_path / project_root，沙箱通过后读回 hooks 子树
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().to_path_buf();
        let settings = proj.join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let hooks = serde_json::json!({"Stop": []});
        std::fs::write(
            &settings,
            serde_json::to_string(&serde_json::json!({"hooks": hooks})).unwrap(),
        )
        .unwrap();

        let v = config_read_sync(
            "project",
            Some(proj.to_str().unwrap()),
            &Some(proj.clone()),
            || None, // user 层闭包惰性，不会被调用
        )
        .unwrap();
        assert_eq!(v, hooks);
    }

    #[test]
    fn config_read_sync_rejects_invalid_layer() {
        // 非法 layer 透传到 parse_layer → Validation
        let err = config_read_sync("bogus", None, &None, || None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn config_write_sync_user_layer_passes_paths_through() {
        // user 层：注入 home tempdir，透传 layer / hooks 写回 home/.claude/settings.json
        let home = tempfile::tempdir().unwrap();
        let hooks = serde_json::json!({"SessionStart": []});
        let home_path = home.path().to_path_buf();

        config_write_sync("user", hooks.clone(), None, &None, move || {
            Some(home_path.clone())
        })
        .unwrap();

        let path = home.path().join(".claude").join("settings.json");
        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reloaded, serde_json::json!({"hooks": hooks}));
    }

    #[test]
    fn config_write_sync_rejects_non_object_hooks() {
        // 非 Object hooks 校验透传 → Validation
        let err =
            config_write_sync("user", serde_json::json!([1, 2]), None, &None, || None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn config_write_sync_null_hooks_clears_layer() {
        // ZQ-5 决策 3：hooks 入参 null 视作空对象 {} 写入——语义 = 清空该层 hooks，
        // merge 保留原文件其他字段（与 read 返回 null 对称）
        let home = tempfile::tempdir().unwrap();
        let settings = home.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let hooks = serde_json::json!({
            "SessionStart": [{"hooks": [{"type": "command", "command": "echo hi"}]}]
        });
        std::fs::write(
            &settings,
            serde_json::to_string(&serde_json::json!({"hooks": hooks, "env": {"FOO": "bar"}}))
                .unwrap(),
        )
        .unwrap();
        let home_path = home.path().to_path_buf();

        config_write_sync("user", Value::Null, None, &None, move || {
            Some(home_path.clone())
        })
        .unwrap();

        let reloaded: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            reloaded["hooks"],
            serde_json::json!({}),
            "null 入参应清空该层 hooks（hooks 键 = 空对象）"
        );
        assert_eq!(reloaded["env"]["FOO"], "bar", "merge 应保留原文件其他字段");
    }
}
