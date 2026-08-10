//! hooks 注入/卸载/状态检测三命令实现（P1-BE-04）
//!
//! 三命令均读写 ~/.claude/settings.json（绕过 project_root 路径沙箱，照 settings.rs 先例），
//! 阻塞 I/O 经 spawn_blocking 串行化（硬约束 #3）。

use super::{HookInjectionStatus, InjectionStatus};
use crate::error::AppError;
use serde_json::Value;
use std::io::Write;
use tempfile::NamedTempFile;

/// 内嵌 hook reporter 脚本模板（编译期嵌入，用于版本比对与升级）
const HOOK_SCRIPT_TEMPLATE: &str = include_str!("../../assets/slterm-hook-reporter.js");

/// C9 规定的 10 个注入事件（与四态映射相关的最小集）
const HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "Stop",
    "StopFailure",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
];

// ── 路径辅助 ──

fn hooks_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".slterminal").join("hooks"))
}

fn hooks_events_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".slterminal").join("hooks-events"))
}

fn claude_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

fn hook_script_path() -> Option<std::path::PathBuf> {
    hooks_dir().map(|d| d.join("slterm-hook-reporter.js"))
}

// ── 版本提取（纯函数，供测试） ──

/// 从内嵌模板提取 SCRIPT_VERSION 常量值
fn template_version() -> u32 {
    for line in HOOK_SCRIPT_TEMPLATE.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("const SCRIPT_VERSION") {
            if let Some(val) = rest.trim().strip_prefix("=") {
                let val = val.trim().trim_end_matches(';').trim();
                if let Ok(v) = val.parse::<u32>() {
                    return v;
                }
            }
        }
    }
    0 // 解析失败回退（不应发生——模板由仓库管理）
}

/// 从磁盘脚本文件提取 SCRIPT_VERSION 常量值
fn disk_script_version(path: &std::path::Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("const SCRIPT_VERSION") {
            if let Some(val) = rest.trim().strip_prefix("=") {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse::<u32>().ok();
            }
        }
    }
    None
}

// ── matcher 检测（纯谓词） ──

/// 检查 settings.json 的 hooks 段中是否包含 slterm-hook-reporter 的 matcher 组
fn has_slterm_matchers(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.as_object())
        .is_some_and(|obj| {
            obj.values().any(|matchers| {
                matchers
                    .as_array()
                    .is_some_and(|arr| arr.iter().any(matcher_contains_slterm))
            })
        })
}

/// 单个 handler 是否引用了 slterm-hook-reporter（C9 识别规则，handler 级判定——
/// 与前端 isSltermManaged 粒度一致）
fn handler_contains_slterm(hook: &Value) -> bool {
    hook.get("command")
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains("slterm-hook-reporter"))
}

/// 单个 matcher 组是否引用了 slterm-hook-reporter（组内任一 handler 命中）
fn matcher_contains_slterm(matcher: &Value) -> bool {
    matcher
        .get("hooks")
        .and_then(|hooks| hooks.as_array())
        .is_some_and(|hooks_arr| hooks_arr.iter().any(handler_contains_slterm))
}

/// 从 hooks 对象中移除所有 slterm-hook-reporter 条目（**handler 级剔除**）：
/// - 组内 hooks 数组剔除命中的 handler——用户自定义 handler 与注入 handler 混入
///   同一 matcher 组时仅删 slterm handler、保留用户条目（与前端 isSltermManaged
///   粒度对齐，验收修复：旧实现按 matcher 组级删除会连带删除组内用户 handler）
/// - 组内 hooks 全空 → 删除整组；无 hooks 数组的组原样保留（非标准形态不碰用户数据）
/// - 事件键下无剩余组 → 清理空数组事件键
///   返回 true 表示有实际移除（含组内 handler 剔除——changed 决定是否写盘）
fn remove_slterm_matchers(hooks: &mut serde_json::Map<String, Value>) -> bool {
    let mut removed = false;
    for (_event, matchers_val) in hooks.iter_mut() {
        if let Some(matchers) = matchers_val.as_array_mut() {
            let mut filtered: Vec<Value> = Vec::with_capacity(matchers.len());
            for mut m in matchers.drain(..) {
                let mut keep_group = true;
                if let Some(hooks_arr) = m.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                    let before_len = hooks_arr.len();
                    hooks_arr.retain(|h| !handler_contains_slterm(h));
                    if hooks_arr.len() < before_len {
                        removed = true; // 组内 handler 被剔除
                    }
                    keep_group = !hooks_arr.is_empty();
                }
                if keep_group {
                    filtered.push(m);
                } else {
                    removed = true; // 整组被删
                }
            }
            *matchers = filtered;
        }
    }
    // 清理空数组的事件键
    hooks.retain(|_event, matchers_val| matchers_val.as_array().is_none_or(|arr| !arr.is_empty()));
    removed
}

/// 构建单个 matcher 条目（C9：matcher="" 匹配全部, timeout=5）
fn build_matcher_entry(script_abs_path: &str) -> Value {
    let path_normalized = script_abs_path.replace('\\', "/");
    serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": format!("node \"{}\"", path_normalized),
            "timeout": 5
        }]
    })
}

/// 将 slterm 的 10 事件 matcher 追加到 hooks 对象中（保留用户现有条目）
fn inject_matchers(hooks: &mut serde_json::Map<String, Value>, script_abs_path: &str) {
    let entry = build_matcher_entry(script_abs_path);
    for event in HOOK_EVENTS {
        let arr = hooks
            .entry(event.to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(matchers) = arr.as_array_mut() {
            // 幂等：仅当不存在 slterm matcher 时追加
            let already_has = matchers.iter().any(matcher_contains_slterm);
            if !already_has {
                matchers.push(entry.clone());
            }
        } else {
            // 非数组值（极端情况）→ 替换为数组
            *arr = serde_json::json!([entry]);
        }
    }
}

// ── 实现（路径可注入，供命令与测试共用——D2 零行为变更） ──

/// hooks_inject 实现：落盘脚本 + merge 注入 settings.json（C6/C9）
///
/// 流程：确保脚本目录存在 → 原子写脚本 → 读 settings.json →
/// 移除旧 slterm 段 → 追加 10 事件 matcher → 原子写回。
/// JSON 非法时返回 AppError 且不改动文件。
fn inject_impl(
    settings_path: &std::path::Path,
    script_dir: &std::path::Path,
) -> Result<HookInjectionStatus, AppError> {
    // 1. 确保脚本目录存在并原子写脚本
    std::fs::create_dir_all(script_dir)?;

    let script_path = script_dir.join("slterm-hook-reporter.js");
    let mut tmp_script = NamedTempFile::new_in(script_dir)?;
    tmp_script.write_all(HOOK_SCRIPT_TEMPLATE.as_bytes())?;
    tmp_script.flush()?;
    tmp_script
        .persist(&script_path)
        .map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("脚本写入失败: {e}"),
        })?;

    // 2. 读 settings.json
    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            // JSON 非法则中止，不改动文件（C9）
            serde_json::from_str(&content).map_err(|e| AppError::IoKind {
                kind: "parse".into(),
                message: format!("~/.claude/settings.json 格式错误，请先修复: {e}"),
            })?
        }
    } else {
        Value::Object(serde_json::Map::new())
    };

    // 3. 确保 settings 是 JSON 对象
    let root_obj = settings.as_object_mut().ok_or_else(|| AppError::IoKind {
        kind: "parse".into(),
        message: "settings.json 根不是 JSON 对象".into(),
    })?;

    // 4. 获取或创建 hooks 段
    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let hooks_obj = hooks.as_object_mut().ok_or_else(|| AppError::IoKind {
        kind: "parse".into(),
        message: "settings.json 的 hooks 段不是 JSON 对象".into(),
    })?;

    // 5. 移除旧 slterm 段（幂等/升级场景），追加新的 10 事件 matcher
    let script_abs = dunce::simplified(&script_path)
        .to_string_lossy()
        .to_string();
    remove_slterm_matchers(hooks_obj);
    inject_matchers(hooks_obj, &script_abs);

    // 6. 原子写回 settings.json
    let settings_parent = settings_path.parent().ok_or_else(|| AppError::IoKind {
        kind: "path".into(),
        message: "无法获取 settings.json 父目录".into(),
    })?;
    std::fs::create_dir_all(settings_parent)?;

    let json_str = serde_json::to_string_pretty(&settings)?;
    let mut tmp_settings = NamedTempFile::new_in(settings_parent)?;
    tmp_settings.write_all(json_str.as_bytes())?;
    tmp_settings.flush()?;
    tmp_settings
        .persist(settings_path)
        .map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("settings.json 写入失败: {e}"),
        })?;

    Ok(HookInjectionStatus {
        status: InjectionStatus::Injected,
        version: Some(template_version()),
    })
}

/// hooks_uninstall 实现：移除配置段 + 删脚本目录 + 清信号目录（C6/C9）
///
/// 安全策略：settings.json 非法时仅跳过配置清理（不损坏用户文件），但仍删除目录。
fn uninstall_impl(
    settings_path: Option<&std::path::Path>,
    script_dir: Option<&std::path::Path>,
    events_dir: Option<&std::path::Path>,
) -> Result<(), AppError> {
    // 1. 从 settings.json 移除 slTerminal 全部 matcher 组
    if let Some(settings_path) = settings_path {
        if settings_path.exists() {
            if let Ok(content) = std::fs::read_to_string(settings_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    if let Ok(mut settings) = serde_json::from_str::<Value>(&content) {
                        let mut changed = false;
                        if let Some(hooks) =
                            settings.get_mut("hooks").and_then(|h| h.as_object_mut())
                        {
                            if remove_slterm_matchers(hooks) {
                                changed = true;
                            }
                            // hooks 段全空 → 移除整个 "hooks" 键
                            if hooks.is_empty() {
                                if let Some(root) = settings.as_object_mut() {
                                    root.remove("hooks");
                                }
                            }
                        }
                        if changed {
                            if let Some(parent) = settings_path.parent() {
                                let json_str = serde_json::to_string_pretty(&settings)?;
                                let mut tmp = NamedTempFile::new_in(parent)?;
                                tmp.write_all(json_str.as_bytes())?;
                                tmp.flush()?;
                                tmp.persist(settings_path).map_err(|e| AppError::IoKind {
                                    kind: format!("{:?}", e.error.kind()),
                                    message: format!("settings.json 写入失败: {e}"),
                                })?;
                            }
                        }
                    }
                    // JSON 非法 → 静默跳过配置清理，仍删目录
                }
            }
        }
    }

    // 2. 删除脚本目录
    if let Some(d) = script_dir {
        if d.exists() {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    // 3. 清空信号目录
    if let Some(d) = events_dir {
        if d.exists() {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    Ok(())
}

/// hooks_injection_status 实现：查询注入状态（C6/C9）
///
/// 三态判定：脚本存在 + settings 含 matcher + 版本一致 → Injected;
/// 版本不一致 → Outdated; 其他 → NotInjected。
fn injection_status_impl(
    script_path: &std::path::Path,
    settings_path: &std::path::Path,
) -> HookInjectionStatus {
    // 脚本是否存在且为普通文件
    if !script_path.is_file() {
        return HookInjectionStatus {
            status: InjectionStatus::NotInjected,
            version: None,
        };
    }

    // 检查 settings.json 中是否有 slTerminal matcher 组
    let has_matchers = if settings_path.exists() {
        std::fs::read_to_string(settings_path)
            .ok()
            .and_then(|c| {
                let trimmed = c.trim();
                if trimmed.is_empty() {
                    Some(false)
                } else {
                    serde_json::from_str::<Value>(&c)
                        .ok()
                        .map(|settings| has_slterm_matchers(&settings))
                }
            })
            .unwrap_or(false)
    } else {
        false
    };

    if !has_matchers {
        return HookInjectionStatus {
            status: InjectionStatus::NotInjected,
            version: None,
        };
    }

    // 版本比对
    let disk_ver = disk_script_version(script_path);
    let template_ver = template_version();

    if disk_ver != Some(template_ver) {
        return HookInjectionStatus {
            status: InjectionStatus::Outdated,
            version: disk_ver,
        };
    }

    HookInjectionStatus {
        status: InjectionStatus::Injected,
        version: disk_ver,
    }
}

// ── Tauri 命令（路径解析 + spawn_blocking 包装，逻辑在 impl） ──

/// hooks_inject — 落盘脚本 + merge 注入 user 层 settings.json（C6/C9）
#[tauri::command]
pub async fn hooks_inject() -> Result<HookInjectionStatus, AppError> {
    let script_dir = hooks_dir().ok_or_else(|| AppError::IoKind {
        kind: "home_dir".into(),
        message: "无法获取用户 home 目录".into(),
    })?;
    let settings_path = claude_settings_path().ok_or_else(|| AppError::IoKind {
        kind: "home_dir".into(),
        message: "无法获取用户 home 目录".into(),
    })?;

    tokio::task::spawn_blocking(move || inject_impl(&settings_path, &script_dir))
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// hooks_uninstall — 移除配置段 + 删脚本目录 + 清信号目录（C6/C9）
///
/// 安全策略：settings.json 非法时仅跳过配置清理（不损坏用户文件），但仍删除目录。
#[tauri::command]
pub async fn hooks_uninstall() -> Result<(), AppError> {
    let settings_path = claude_settings_path();
    let script_dir = hooks_dir();
    let events_dir = hooks_events_dir();

    tokio::task::spawn_blocking(move || {
        uninstall_impl(
            settings_path.as_deref(),
            script_dir.as_deref(),
            events_dir.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// hooks_injection_status — 查询注入状态（C6/C9）
#[tauri::command]
pub async fn hooks_injection_status() -> Result<HookInjectionStatus, AppError> {
    let script_path = match hook_script_path() {
        Some(p) => p,
        None => {
            return Ok(HookInjectionStatus {
                status: InjectionStatus::NotInjected,
                version: None,
            });
        }
    };
    let settings_path = match claude_settings_path() {
        Some(p) => p,
        None => {
            return Ok(HookInjectionStatus {
                status: InjectionStatus::NotInjected,
                version: None,
            });
        }
    };

    let status =
        tokio::task::spawn_blocking(move || injection_status_impl(&script_path, &settings_path))
            .await
            .map_err(|e| AppError::TaskJoin(e.to_string()))?;
    Ok(status)
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── template_version ──

    #[test]
    fn template_version_positive() {
        let v = template_version();
        assert!(v > 0, "SCRIPT_VERSION 应大于 0，实际: {v}");
        assert_eq!(v, 1, "初始版本应为 1");
    }

    // ── HOOK_EVENTS ──

    #[test]
    fn hook_events_count_and_unique() {
        assert_eq!(HOOK_EVENTS.len(), 10, "注入事件数应为 10");
        let mut sorted = HOOK_EVENTS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 10, "10 个事件不应有重复");
    }

    #[test]
    fn hook_events_contains_key_events() {
        assert!(HOOK_EVENTS.contains(&"SessionStart"));
        assert!(HOOK_EVENTS.contains(&"SessionEnd"));
        assert!(HOOK_EVENTS.contains(&"PreToolUse"));
        assert!(HOOK_EVENTS.contains(&"Notification"));
    }

    // ── has_slterm_matchers ──

    #[test]
    fn has_matchers_empty_settings() {
        assert!(!has_slterm_matchers(&serde_json::json!({})));
    }

    #[test]
    fn has_matchers_no_hooks_key() {
        assert!(!has_slterm_matchers(&serde_json::json!({"other": "value"})));
    }

    #[test]
    fn has_matchers_detects_command() {
        let settings = serde_json::json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "node \"C:/Users/test/.slterminal/hooks/slterm-hook-reporter.js\"",
                        "timeout": 5
                    }]
                }]
            }
        });
        assert!(has_slterm_matchers(&settings));
    }

    #[test]
    fn has_matchers_user_hooks_not_detected() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "node \"./my-hook.js\"",
                        "timeout": 60
                    }]
                }]
            }
        });
        assert!(!has_slterm_matchers(&settings));
    }

    // ── disk_script_version ──

    #[test]
    fn disk_version_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "const SCRIPT_VERSION = 3;\n// other code\n").unwrap();
        assert_eq!(disk_script_version(&path), Some(3));
    }

    #[test]
    fn disk_version_no_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "// no version here\n").unwrap();
        assert_eq!(disk_script_version(&path), None);
    }

    #[test]
    fn disk_version_missing_file() {
        assert_eq!(
            disk_script_version(std::path::Path::new("/nonexistent/file.js")),
            None
        );
    }

    #[test]
    fn disk_version_with_spaces_and_semicolon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.js");
        std::fs::write(&path, "  const SCRIPT_VERSION  =  42  ;  \n").unwrap();
        assert_eq!(disk_script_version(&path), Some(42));
    }

    // ── remove_slterm_matchers ──

    #[test]
    fn remove_matchers_cleans_slterm_entries() {
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {"matcher": "", "hooks": [{"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5}]},
                {"matcher": "claude", "hooks": [{"type": "command", "command": "echo hello", "timeout": 10}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        let arr = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "用户 hook 应保留");
        assert_eq!(arr[0]["hooks"][0]["command"], "echo hello");
    }

    #[test]
    fn remove_matchers_cleans_empty_events() {
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {"matcher": "", "hooks": [{"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        assert!(!hooks.contains_key("SessionStart"), "空事件键应被清理");
    }

    #[test]
    fn remove_matchers_no_slterm_entries() {
        let mut hooks = serde_json::json!({
            "PreToolUse": [
                {"matcher": "", "hooks": [{"type": "command", "command": "echo x", "timeout": 10}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(!removed);
        assert!(hooks.contains_key("PreToolUse"));
    }

    #[test]
    fn remove_matchers_keeps_user_handler_in_same_group() {
        // 验收修复：用户自定义 handler 与注入 handler 混入同一 matcher 组时，
        // 仅剔除 slterm handler，用户 handler 与组保留（handler 级剔除，对齐前端粒度）
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [
                        {"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5},
                        {"type": "command", "command": "echo user-hook", "timeout": 10}
                    ]
                }
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed, "组内 handler 剔除应标记 removed（触发写盘）");
        let arr = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "混组场景组应保留");
        let handlers = arr[0]["hooks"].as_array().unwrap();
        assert_eq!(handlers.len(), 1, "仅 slterm handler 被剔除");
        assert_eq!(handlers[0]["command"], "echo user-hook");
    }

    #[test]
    fn remove_matchers_drops_group_when_all_handlers_slterm() {
        // 同组全部 handler 均为 slterm → 组空 → 整组删除
        let mut hooks = serde_json::json!({
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [
                        {"type": "command", "command": "node \"slterm-hook-reporter.js\"", "timeout": 5},
                        {"type": "command", "command": "node \"other/slterm-hook-reporter.js\"", "timeout": 5}
                    ]
                }
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        let removed = remove_slterm_matchers(&mut hooks);
        assert!(removed);
        assert!(
            !hooks.contains_key("SessionStart"),
            "全 slterm 组删除后空事件键应被清理"
        );
    }

    // ── inject_matchers ──

    #[test]
    fn inject_adds_10_events() {
        let mut hooks = serde_json::Map::new();
        inject_matchers(
            &mut hooks,
            "/home/user/.slterminal/hooks/slterm-hook-reporter.js",
        );
        // D7 键集合精确匹配：注入后的事件键应恰好等于 HOOK_EVENTS 全集
        let mut keys: Vec<&str> = hooks.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        let mut expected: Vec<&str> = HOOK_EVENTS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "注入的事件键应恰好等于 HOOK_EVENTS 全集");
        // 结构断言：每事件 handler 数组含 {type:"command", timeout:5, command 含 slterm-hook-reporter}
        for &event in HOOK_EVENTS {
            let matchers = hooks[event].as_array().unwrap();
            assert_eq!(matchers.len(), 1, "事件 {event} 应恰好 1 个 matcher 组");
            let matcher = &matchers[0];
            assert_eq!(matcher["matcher"], "", "事件 {event} matcher 应为空串");
            assert_eq!(
                matcher.as_object().unwrap().len(),
                2,
                "事件 {event} matcher 应恰好 matcher/hooks 两键（D7 键集合）"
            );
            let handlers = matcher["hooks"].as_array().unwrap();
            assert_eq!(handlers.len(), 1, "事件 {event} 应恰好 1 个 handler");
            let handler = &handlers[0];
            assert_eq!(
                handler.as_object().unwrap().len(),
                3,
                "事件 {event} handler 应恰好 type/command/timeout 三键（D7 键集合）"
            );
            assert_eq!(handler["type"], "command", "事件 {event} handler type");
            assert_eq!(handler["timeout"], 5, "事件 {event} handler timeout");
            let cmd = handler["command"].as_str().unwrap();
            assert!(
                cmd.contains("slterm-hook-reporter"),
                "事件 {event} command 应含 slterm-hook-reporter: {cmd}"
            );
        }
    }

    #[test]
    fn inject_preserves_existing_user_matchers() {
        let mut hooks = serde_json::json!({
            "PreToolUse": [
                {"matcher": "custom", "hooks": [{"type": "command", "command": "my-hook.sh", "timeout": 30}]}
            ]
        })
        .as_object()
        .unwrap()
        .clone();
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        // PreToolUse 现在应有 2 个 entry: 用户自定义 + slterm
        let arr = hooks["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        // 用户 entry 应在前面
        assert_eq!(arr[0]["hooks"][0]["command"], "my-hook.sh");
        // slterm entry 应在后面（含子串标识）
        let cmd = arr[1]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("slterm-hook-reporter"));
    }

    #[test]
    fn inject_idempotent() {
        let mut hooks = serde_json::Map::new();
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        let len_after_first = hooks["SessionStart"].as_array().unwrap().len();
        // 二次注入不追加重复
        inject_matchers(&mut hooks, "/tmp/slterm-hook-reporter.js");
        let len_after_second = hooks["SessionStart"].as_array().unwrap().len();
        assert_eq!(
            len_after_first, len_after_second,
            "二次注入不应产生重复 matcher"
        );
    }

    // ── build_matcher_entry ──

    #[test]
    fn matcher_entry_timeout_is_5() {
        let entry = build_matcher_entry("/tmp/slterm-hook-reporter.js");
        assert_eq!(entry["hooks"][0]["timeout"], 5);
        assert_eq!(entry["matcher"], "");
        assert_eq!(entry["hooks"][0]["type"], "command");
    }

    // ── 模板内嵌校验 ──

    #[test]
    fn template_is_non_empty() {
        assert!(HOOK_SCRIPT_TEMPLATE.len() > 100, "内嵌脚本模板不应为空");
        assert!(
            HOOK_SCRIPT_TEMPLATE.contains("SLTERM_PANEL_ID"),
            "模板应引用 SLTERM_PANEL_ID 环境变量"
        );
        assert!(
            HOOK_SCRIPT_TEMPLATE.contains("SCRIPT_VERSION"),
            "模板应含版本常量"
        );
    }

    // ── null safety ──

    #[test]
    fn has_matchers_null_hooks_value() {
        // hooks 存在但某个事件的值为 null（非数组）
        let settings = serde_json::json!({"hooks": {"SessionStart": null}});
        assert!(!has_slterm_matchers(&settings));
    }

    // ── handler_contains_slterm（HUK-11） ──

    #[test]
    fn handler_contains_slterm_string_matches() {
        let hook = serde_json::json!({
            "type": "command",
            "command": "node \"C:/x/slterm-hook-reporter.js\"",
            "timeout": 5
        });
        assert!(handler_contains_slterm(&hook));
    }

    #[test]
    fn handler_contains_slterm_non_string_command() {
        // command 为非字符串（number/null/缺失）时不应误判为 slterm
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command", "command": 42
        })));
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command", "command": null
        })));
        assert!(!handler_contains_slterm(&serde_json::json!({
            "type": "command"
        })));
        // 字符串但不含 slterm-hook-reporter 子串 → false
        assert!(!handler_contains_slterm(&serde_json::json!({
            "command": "node \"my-hook.js\""
        })));
    }

    // ── 注入/卸载/状态 impl（路径可注入，tempdir 驱动——HUK-02） ──

    /// 构造注入场景环境：settings.json 初始不存在（首次注入场景），全部落在 tempdir 内
    fn make_inject_env() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir
            .path()
            .join("home")
            .join(".claude")
            .join("settings.json");
        let script_dir = dir.path().join("home").join(".slterminal").join("hooks");
        (dir, settings_path, script_dir)
    }

    /// 断言 settings.json 已按 C9 注入 10 事件 matcher（返回解析后的 settings）
    fn assert_injected_settings(settings_path: &std::path::Path) -> Value {
        let content = std::fs::read_to_string(settings_path).unwrap();
        let settings: Value = serde_json::from_str(&content).unwrap();
        let hooks = settings["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), 10, "应注入恰好 10 个事件");
        for &event in HOOK_EVENTS {
            let arr = hooks[event].as_array().unwrap();
            assert_eq!(arr.len(), 1, "事件 {event} 应恰好 1 个 matcher");
            assert_eq!(arr[0]["matcher"], "", "事件 {event} matcher 应为空串");
            assert_eq!(arr[0]["hooks"][0]["type"], "command");
            assert_eq!(arr[0]["hooks"][0]["timeout"], 5);
            assert!(
                arr[0]["hooks"][0]["command"]
                    .as_str()
                    .unwrap()
                    .contains("slterm-hook-reporter"),
                "事件 {event} command 应含 slterm-hook-reporter"
            );
        }
        settings
    }

    #[test]
    fn inject_impl_basic() {
        let (_dir, settings_path, script_dir) = make_inject_env();
        let status = inject_impl(&settings_path, &script_dir).unwrap();
        assert_eq!(status.status, InjectionStatus::Injected);
        assert_eq!(status.version, Some(template_version()));
        // 脚本已落盘且内容为内嵌模板
        let script_path = script_dir.join("slterm-hook-reporter.js");
        assert!(script_path.is_file(), "脚本应写入脚本目录");
        assert_eq!(
            std::fs::read_to_string(&script_path).unwrap(),
            HOOK_SCRIPT_TEMPLATE
        );
        // settings.json 已按 C9 merge 注入
        assert_injected_settings(&settings_path);
    }

    #[test]
    fn inject_impl_preserves_other_settings_fields() {
        // 既有 settings 的其他字段（permissions/env）在 merge 后原样保留
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"permissions":{"allow":["bash"]},"env":{"K":"v"}}"#,
        )
        .unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let settings = assert_injected_settings(&settings_path);
        assert_eq!(
            settings["permissions"]["allow"][0], "bash",
            "merge 应保留 permissions 字段"
        );
        assert_eq!(settings["env"]["K"], "v", "merge 应保留 env 字段");
    }

    #[test]
    fn inject_impl_idempotent() {
        // 二次注入不产生重复 matcher（每事件仍恰好 1 个）
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        inject_impl(&settings_path, &script_dir).unwrap();
        let settings = assert_injected_settings(&settings_path);
        for &event in HOOK_EVENTS {
            let arr = settings["hooks"][event].as_array().unwrap();
            assert_eq!(arr.len(), 1, "事件 {event} 二次注入后不应出现重复 matcher");
        }
    }

    #[test]
    fn inject_impl_illegal_json_aborts_without_modify() {
        // 非法 JSON 中止：返回 Err 且不改动原文件（C9）
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original = b"{ this is not valid json !!".to_vec();
        std::fs::write(&settings_path, &original).unwrap();
        let err = inject_impl(&settings_path, &script_dir).unwrap_err();
        assert!(
            err.to_string().contains("格式错误"),
            "错误信息应说明格式错误: {err}"
        );
        assert_eq!(
            std::fs::read(&settings_path).unwrap(),
            original,
            "非法 JSON 中止时不应改动原文件"
        );
    }

    #[test]
    fn inject_impl_non_object_root_aborts() {
        // 根为数组/标量无法安全 merge → 中止且不改动文件
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, "[1,2,3]").unwrap();
        assert!(inject_impl(&settings_path, &script_dir).is_err());
        assert_eq!(
            std::fs::read_to_string(&settings_path).unwrap(),
            "[1,2,3]",
            "根非对象时不应改动文件"
        );
    }

    #[test]
    fn inject_impl_non_object_hooks_aborts() {
        // hooks 段非对象 → 中止
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, r#"{"hooks": [1,2]}"#).unwrap();
        assert!(inject_impl(&settings_path, &script_dir).is_err());
    }

    #[test]
    fn uninstall_impl_mixed_group_keeps_user_handler() {
        // 混组场景：同一 matcher 组内 slterm handler + 用户 handler → 仅删 slterm、
        // 用户 handler 与组保留（handler 级剔除）；用户 matcher 整体保留；目录删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{
                "hooks": {
                    "SessionStart": [{
                        "matcher": "",
                        "hooks": [
                            {"type": "command", "command": "node \"C:/x/slterm-hook-reporter.js\"", "timeout": 5},
                            {"type": "command", "command": "echo user-hook", "timeout": 10}
                        ]
                    }],
                    "PreToolUse": [{
                        "matcher": "custom",
                        "hooks": [{"type": "command", "command": "my-hook.sh", "timeout": 30}]
                    }]
                }
            }"#,
        )
        .unwrap();
        std::fs::create_dir_all(&script_dir).unwrap();
        std::fs::write(
            script_dir.join("slterm-hook-reporter.js"),
            "const SCRIPT_VERSION = 1;",
        )
        .unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        std::fs::write(events_dir.join("evt.json"), "{}").unwrap();

        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();

        // settings：混组保用户 handler、组保留；用户 matcher 保留
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let hooks = settings["hooks"].as_object().unwrap();
        let session = hooks["SessionStart"].as_array().unwrap();
        assert_eq!(session.len(), 1, "混组场景组应保留");
        let handlers = session[0]["hooks"].as_array().unwrap();
        assert_eq!(handlers.len(), 1, "仅 slterm handler 被剔除");
        assert_eq!(handlers[0]["command"], "echo user-hook");
        let pre = hooks["PreToolUse"].as_array().unwrap();
        assert_eq!(
            pre[0]["hooks"][0]["command"], "my-hook.sh",
            "用户 matcher 应保留"
        );
        // 目录已删除
        assert!(!script_dir.exists(), "脚本目录应被删除");
        assert!(!events_dir.exists(), "信号目录应被删除");
    }

    #[test]
    fn uninstall_impl_all_slterm_removes_hooks_key() {
        // 注入后整体卸载：10 事件全 slterm → hooks 键整体移除 + 目录删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        inject_impl(&settings_path, &script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings.get("hooks").is_none(),
            "全 slterm 卸载后 hooks 键应整体移除"
        );
        assert!(!script_dir.exists());
        assert!(!events_dir.exists());
    }

    #[test]
    fn uninstall_impl_illegal_json_skips_config_but_deletes_dirs() {
        // 非法 JSON → 静默跳过配置清理（文件原样），目录仍删除
        let (_dir, settings_path, script_dir) = make_inject_env();
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        let original = b"{ invalid !!".to_vec();
        std::fs::write(&settings_path, &original).unwrap();
        std::fs::create_dir_all(&script_dir).unwrap();
        let events_dir = script_dir.parent().unwrap().join("hooks-events");
        std::fs::create_dir_all(&events_dir).unwrap();
        uninstall_impl(Some(&settings_path), Some(&script_dir), Some(&events_dir)).unwrap();
        assert_eq!(
            std::fs::read(&settings_path).unwrap(),
            original,
            "非法 JSON 时配置清理应跳过"
        );
        assert!(!script_dir.exists(), "脚本目录仍应删除");
        assert!(!events_dir.exists(), "信号目录仍应删除");
    }

    #[test]
    fn status_impl_three_states() {
        // 状态三态：NotInjected / Injected / Outdated（脚本版本 vs 模板版本比对）
        let (_dir, settings_path, script_dir) = make_inject_env();
        let script_path = script_dir.join("slterm-hook-reporter.js");

        // ① 脚本不存在 → NotInjected
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, InjectionStatus::NotInjected);

        // ② 注入后（脚本版本与模板一致 + settings 含 matcher）→ Injected
        inject_impl(&settings_path, &script_dir).unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, InjectionStatus::Injected);
        assert_eq!(s.version, Some(template_version()));

        // ③ 磁盘脚本版本 ≠ 模板版本 → Outdated（version 为磁盘版本）
        std::fs::write(
            &script_path,
            format!("const SCRIPT_VERSION = {};\n", template_version() + 1),
        )
        .unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, InjectionStatus::Outdated);
        assert_eq!(s.version, Some(template_version() + 1));

        // ④ settings 无 slterm matcher → NotInjected
        std::fs::write(
            &script_path,
            format!("const SCRIPT_VERSION = {};\n", template_version()),
        )
        .unwrap();
        std::fs::write(&settings_path, "{}").unwrap();
        let s = injection_status_impl(&script_path, &settings_path);
        assert_eq!(s.status, InjectionStatus::NotInjected);
    }
}
