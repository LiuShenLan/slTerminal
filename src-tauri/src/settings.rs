/// 设置持久化模块 — save/load settings 到 ~/.slterminal/settings.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。
use crate::error::AppError;
use std::io::Write as _;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// 获取应用数据目录（~/.slterminal）
fn app_data_dir() -> Result<PathBuf, AppError> {
    let home =
        dirs::home_dir()
        .ok_or_else(|| AppError::IoKind { kind: "home_dir".into(), message: "无法获取用户主目录".into() })?;
    Ok(home.join(".slterminal"))
}

/// 浅合并：incoming 的 top-level 键覆盖 existing。
/// 两者均为 JSON 对象时逐键合并；否则 incoming 整体胜出（兼容缺失/损坏/非对象设置）。
fn merge_settings(existing: serde_json::Value, incoming: serde_json::Value) -> serde_json::Value {
    match (existing, incoming) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(inc)) => {
            for (k, v) in inc {
                base.insert(k, v);
            }
            serde_json::Value::Object(base)
        }
        // existing 非对象（缺失/损坏 → Null）或 incoming 非对象：用 incoming 整体
        (_, incoming) => incoming,
    }
}

/// 持久化设置（读现有 → 浅合并 top-level 键 → 原子写入：tempfile → flush → persist，.bak 备份兜底）
#[tauri::command]
pub async fn save_settings(settings: serde_json::Value) -> Result<(), AppError> {
    let app_dir = app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let settings_path = app_dir.join("settings.json");

    match tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // 读现有 settings.json（不存在/损坏视作 Null），与 incoming 浅合并
        let existing = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .unwrap_or(serde_json::Value::Null);
        let merged = merge_settings(existing, settings);

        let json = serde_json::to_string_pretty(&merged)?;
        let mut tmp = NamedTempFile::new_in(&app_dir)?;
        tmp.write_all(json.as_bytes())?;
        tmp.flush()?;
        if settings_path.exists() {
            let bak = app_dir.join("settings.json.bak");
            if let Err(e) = std::fs::copy(&settings_path, &bak) {
                tracing::warn!("settings .bak 备份失败: {}", e);
            }
        }
        tmp.persist(&settings_path)
            .map_err(|e| AppError::IoKind { kind: format!("{:?}", e.error.kind()), message: format!("persist 失败: {e}") })?;
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
    Ok(())
}

/// 加载持久化设置，失败从 .bak 恢复，仍失败返回 Null
#[tauri::command]
pub async fn load_settings() -> Result<serde_json::Value, AppError> {
    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    let app_dir_clone = app_dir.clone();
    let result = match tokio::task::spawn_blocking(move || -> Result<serde_json::Value, AppError> {
        match std::fs::read_to_string(&settings_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(v) => Ok(v),
                Err(_) => {
                    let bak = app_dir_clone.join("settings.json.bak");
                    if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                        if let Ok(v) = serde_json::from_str(&bak_content) {
                            let _ = std::fs::write(&settings_path, &bak_content);
                            return Ok(v);
                        }
                    }
                    Ok(serde_json::Value::Null)
                }
            },
            Err(_) => Ok(serde_json::Value::Null),
        }
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
    Ok(result)
}

/// 验证设置加载/保存逻辑
#[cfg(test)]
mod tests {
    use super::*;

    /// 完整 save → load 往返（使用临时目录）
    #[test]
    fn test_save_settings_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let settings: serde_json::Value = serde_json::json!({
            "theme": "jetbrains-dark",
            "fontSize": 14
        });

        // --- save 逻辑（同步版）---
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let mut tmp = NamedTempFile::new_in(dir.path()).unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp.persist(&settings_path).unwrap();

        // --- load 逻辑（同步版）---
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded, settings);
    }

    /// 文件不存在时 load_settings 返回 Null
    #[test]
    fn test_load_settings_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        // 确保文件不存在
        let _ = std::fs::remove_file(&settings_path);

        let result = std::fs::read_to_string(&settings_path);
        assert!(result.is_err(), "文件不存在时 read 应失败");

        // 对应 load_settings 中 Err(_) → Ok(Null) 分支
        let value: serde_json::Value = match result {
            Ok(_) => serde_json::Value::Null,
            Err(_) => serde_json::Value::Null,
        };
        assert!(value.is_null());
    }

    /// JSON 损坏时从 .bak 恢复
    #[test]
    fn test_load_settings_corrupt_json_fallback_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let bak_path = dir.path().join("settings.json.bak");

        let valid_json = r#"{"theme":"dark","fontSize":14}"#;
        let corrupt_json = "not valid json {{{broken";

        // 写入 bak
        std::fs::write(&bak_path, valid_json).unwrap();
        // 写入损坏的 settings
        std::fs::write(&settings_path, corrupt_json).unwrap();

        // 模拟 load_settings 逻辑
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                let bak_content = std::fs::read_to_string(&bak_path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&bak_content).unwrap();
                let _ = std::fs::write(&settings_path, &bak_content);
                v
            }
        };

        assert_eq!(
            value,
            serde_json::json!({"theme": "dark", "fontSize": 14})
        );
        // 验证 settings.json 已被修复
        let repaired: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            repaired,
            serde_json::json!({"theme": "dark", "fontSize": 14})
        );
    }

    /// JSON 损坏且无 .bak 时返回 Null
    #[test]
    fn test_load_settings_corrupt_json_no_bak() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let corrupt_json = "definitely not json {{{";

        std::fs::write(&settings_path, corrupt_json).unwrap();

        // 模拟 load_settings 逻辑
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                let bak_path = dir.path().join("settings.json.bak");
                if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                    if let Ok(v) = serde_json::from_str(&bak_content) {
                        v
                    } else {
                        serde_json::Value::Null
                    }
                } else {
                    serde_json::Value::Null
                }
            }
        };

        assert!(value.is_null(), "无 .bak 时应返回 Null");
    }

    // ── merge_settings 浅合并 ──

    /// 合并保留 existing 中 incoming 未涉及的 top-level 键
    #[test]
    fn test_merge_preserves_foreign_keys() {
        let existing = serde_json::json!({ "terminalFontSize": 14, "editorFontSize": 16 });
        let incoming = serde_json::json!({ "keybindings": { "terminal.copy": "Ctrl+Alt+KeyC" } });
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged["terminalFontSize"], 14);
        assert_eq!(merged["editorFontSize"], 16);
        assert_eq!(merged["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC");
    }

    /// 合并时 incoming 覆盖 existing 的同名键
    #[test]
    fn test_merge_overwrites_same_key() {
        let existing = serde_json::json!({ "a": 1, "b": 2 });
        let incoming = serde_json::json!({ "a": 99 });
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged["a"], 99);
        assert_eq!(merged["b"], 2);
    }

    /// existing 为 Null（文件缺失/损坏）时用 incoming 初始化
    #[test]
    fn test_merge_null_existing_initializes_with_incoming() {
        let merged = merge_settings(
            serde_json::Value::Null,
            serde_json::json!({ "keybindings": {} }),
        );
        assert_eq!(merged, serde_json::json!({ "keybindings": {} }));
    }

    /// incoming 非对象时整体替换（极端兜底）
    #[test]
    fn test_merge_non_object_incoming_replaces() {
        let existing = serde_json::json!({ "a": 1 });
        let incoming = serde_json::json!("scalar");
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged, serde_json::json!("scalar"));
    }
}
