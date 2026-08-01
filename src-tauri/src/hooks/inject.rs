//! hooks 注入/卸载/状态检测三命令实现（P1-BE-04）
//!
//! 三命令均读写 ~/.claude/settings.json（绕过 project_root 路径沙箱，照 settings.rs 先例），
//! 阻塞 I/O 经 spawn_blocking 串行化（硬约束 #3）。

use crate::error::AppError;
use super::{HookInjectionStatus, InjectionStatus};
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
                matchers.as_array().is_some_and(|arr| {
                    arr.iter().any(matcher_contains_slterm)
                })
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
    hooks.retain(|_event, matchers_val| {
        matchers_val.as_array().is_none_or(|arr| !arr.is_empty())
    });
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

// ── Tauri 命令 ──

/// hooks_inject — 落盘脚本 + merge 注入 user 层 settings.json（C6/C9）
///
/// 流程：确保 ~/.slterminal/hooks/ 存在 → 原子写脚本 →
/// 读 ~/.claude/settings.json → 移除旧 slterm 段 → 追加 10 事件 matcher →
/// 原子写回。JSON 非法时返回 AppError 且不改动文件。
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

    tokio::task::spawn_blocking(move || -> Result<HookInjectionStatus, AppError> {
        // 1. 确保脚本目录存在并原子写脚本
        std::fs::create_dir_all(&script_dir)?;

        let script_path = script_dir.join("slterm-hook-reporter.js");
        let mut tmp_script = NamedTempFile::new_in(&script_dir)?;
        tmp_script.write_all(HOOK_SCRIPT_TEMPLATE.as_bytes())?;
        tmp_script.flush()?;
        tmp_script.persist(&script_path).map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("脚本写入失败: {e}"),
        })?;

        // 2. 读 ~/.claude/settings.json
        let mut settings: Value = if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)?;
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
        let script_abs = dunce::simplified(&script_path).to_string_lossy().to_string();
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
        tmp_settings.persist(&settings_path).map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("settings.json 写入失败: {e}"),
        })?;

        Ok(HookInjectionStatus {
            status: InjectionStatus::Injected,
            version: Some(template_version()),
        })
    })
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

    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // 1. 从 settings.json 移除 slTerminal 全部 matcher 组
        if let Some(ref settings_path) = settings_path {
            if settings_path.exists() {
                if let Ok(content) = std::fs::read_to_string(settings_path) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        if let Ok(mut settings) =
                            serde_json::from_str::<Value>(&content)
                        {
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
                                    let mut tmp =
                                        NamedTempFile::new_in(parent)?;
                                    tmp.write_all(json_str.as_bytes())?;
                                    tmp.flush()?;
                                    tmp.persist(settings_path).map_err(|e| {
                                        AppError::IoKind {
                                            kind: format!("{:?}", e.error.kind()),
                                            message: format!(
                                                "settings.json 写入失败: {e}"
                                            ),
                                        }
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
        if let Some(ref d) = script_dir {
            if d.exists() {
                let _ = std::fs::remove_dir_all(d);
            }
        }

        // 3. 清空信号目录
        if let Some(ref d) = events_dir {
            if d.exists() {
                let _ = std::fs::remove_dir_all(d);
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// hooks_injection_status — 查询注入状态（C6/C9）
///
/// 三态判定：脚本存在 + settings 含 matcher + 版本一致 → Injected;
/// 版本不一致 → Outdated; 其他 → NotInjected。
#[tauri::command]
pub async fn hooks_injection_status() -> Result<HookInjectionStatus, AppError> {
    tokio::task::spawn_blocking(|| -> Result<HookInjectionStatus, AppError> {
        let script_path = match hook_script_path() {
            Some(p) => p,
            None => {
                return Ok(HookInjectionStatus {
                    status: InjectionStatus::NotInjected,
                    version: None,
                });
            }
        };

        // 脚本是否存在且为普通文件
        if !script_path.is_file() {
            return Ok(HookInjectionStatus {
                status: InjectionStatus::NotInjected,
                version: None,
            });
        }

        // 检查 settings.json 中是否有 slTerminal matcher 组
        let has_matchers = match claude_settings_path() {
            Some(ref p) if p.exists() => {
                std::fs::read_to_string(p)
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
            }
            _ => false,
        };

        if !has_matchers {
            return Ok(HookInjectionStatus {
                status: InjectionStatus::NotInjected,
                version: None,
            });
        }

        // 版本比对
        let disk_ver = disk_script_version(&script_path);
        let template_ver = template_version();

        if disk_ver != Some(template_ver) {
            return Ok(HookInjectionStatus {
                status: InjectionStatus::Outdated,
                version: disk_ver,
            });
        }

        Ok(HookInjectionStatus {
            status: InjectionStatus::Injected,
            version: disk_ver,
        })
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
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
        std::fs::write(
            &path,
            "const SCRIPT_VERSION = 3;\n// other code\n",
        )
        .unwrap();
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
        assert!(
            !hooks.contains_key("SessionStart"),
            "空事件键应被清理"
        );
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
        assert_eq!(hooks.len(), 10);
        for event in HOOK_EVENTS {
            assert!(hooks.contains_key(*event), "缺少事件: {event}");
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
}
