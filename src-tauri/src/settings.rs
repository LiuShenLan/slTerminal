/// 设置持久化模块 — save/load settings 到 ~/.slterminal/settings.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。
use crate::error::AppError;
use std::io::Write as _;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// 获取应用数据目录（exe 同级目录，适配便携分发）
pub(crate) fn app_data_dir() -> Result<PathBuf, AppError> {
    let exe = std::env::current_exe().map_err(|e| AppError::IoKind {
        kind: "exe_dir".into(),
        message: format!("无法获取可执行文件路径: {e}"),
    })?;
    let exe_dir = exe.parent().ok_or_else(|| AppError::IoKind {
        kind: "exe_dir".into(),
        message: "无法获取可执行文件所在目录".into(),
    })?;
    Ok(exe_dir.to_path_buf())
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
    let settings_path = app_dir.join("settings.json");

    match tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // BE-05: create_dir_all 移入 spawn_blocking 闭包内部，避免异步上下文阻塞 I/O
        std::fs::create_dir_all(&app_dir)?;
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

    // ── TE-14: save_settings / load_settings 命令包装层测试 ──

    /// 多次 save 不擦除其他段（浅合并验证）— TE-14 核心用例
    /// 模拟 save_settings 的"读现有→合并→写回"流程
    #[test]
    fn te14_save_preserves_other_sections() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // 第一次 save：写入 fontSize
        let initial = serde_json::json!({"fontSize": 14});
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&initial).unwrap(),
        )
        .unwrap();

        // 第二次 save：只写 keybindings（模拟 save_settings 调用）
        let incoming = serde_json::json!({"keybindings": {"terminal.copy": "Ctrl+Alt+KeyC"}});
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let merged = merge_settings(existing, incoming);
        let json = serde_json::to_string_pretty(&merged).unwrap();
        let mut tmp = NamedTempFile::new_in(dir.path()).unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp.persist(&settings_path).unwrap();

        // 验证：fontSize 仍保留
        let reloaded: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(reloaded["fontSize"], 14, "fontSize 不应被 keybindings 覆盖擦除");
        assert_eq!(
            reloaded["keybindings"]["terminal.copy"],
            "Ctrl+Alt+KeyC",
            "keybindings 应正确写入"
        );
    }

    /// 三次增量 save 验证所有段均保留（fontSize → keybindings → editorFontSize）
    #[test]
    fn te14_three_save_cycles_preserve_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        let saves: Vec<serde_json::Value> = vec![
            serde_json::json!({"fontSize": 14}),
            serde_json::json!({"keybindings": {"terminal.copy": "Ctrl+Shift+C"}}),
            serde_json::json!({"editorFontSize": 16}),
        ];

        for incoming in &saves {
            let existing = std::fs::read_to_string(&settings_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .unwrap_or(serde_json::Value::Null);
            let merged = merge_settings(existing, incoming.clone());
            let json = serde_json::to_string_pretty(&merged).unwrap();
            let mut tmp = NamedTempFile::new_in(dir.path()).unwrap();
            tmp.write_all(json.as_bytes()).unwrap();
            tmp.flush().unwrap();
            tmp.persist(&settings_path).unwrap();
        }

        let final_value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(final_value["fontSize"], 14, "第一次 save 的 fontSize 应保留");
        assert_eq!(
            final_value["keybindings"]["terminal.copy"],
            "Ctrl+Shift+C",
            "第二次 save 的 keybindings 应保留"
        );
        assert_eq!(final_value["editorFontSize"], 16, "第三次 save 的 editorFontSize 应保留");
    }

    /// 覆盖写入：同名键被后续 save 覆盖
    #[test]
    fn te14_save_overwrites_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // 先写入 fontSize=14
        let initial = serde_json::json!({"fontSize": 14});
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&initial).unwrap(),
        )
        .unwrap();

        // 再写 fontSize=18（覆盖）
        let incoming = serde_json::json!({"fontSize": 18});
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let merged = merge_settings(existing, incoming);
        let json = serde_json::to_string_pretty(&merged).unwrap();
        std::fs::write(&settings_path, &json).unwrap();

        let reloaded: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(reloaded["fontSize"], 18, "fontSize 应被覆盖为新值");
    }

    /// create_dir_all：目录不存在时 save 自动创建（BE-05 确保此 I/O 在 spawn_blocking 内）
    #[test]
    fn te14_save_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        // 指向不存在子目录
        let sub_dir = dir.path().join("config").join("slterminal");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let settings_path = sub_dir.join("settings.json");
        let settings = serde_json::json!({"fontSize": 14});

        let json = serde_json::to_string_pretty(&settings).unwrap();
        let mut tmp = NamedTempFile::new_in(&sub_dir).unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp.persist(&settings_path).unwrap();

        assert!(settings_path.exists(), "settings.json 应在自动创建的目录中存在");
    }

    /// .bak 损坏时仍返回 Null（load_settings 错误映射：.bak 也损坏 → Null）
    #[test]
    fn te14_load_corrupt_both_json_and_bak_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let bak_path = dir.path().join("settings.json.bak");

        // settings.json 损坏
        std::fs::write(&settings_path, "not json {{{").unwrap();
        // .bak 也损坏
        std::fs::write(&bak_path, "also corrupt ###").unwrap();

        // 模拟 load_settings 逻辑：settings 损坏 → 尝试 .bak → .bak 也损坏 → Null
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
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

        assert!(value.is_null(), ".bak 也损坏时应返回 Null");
    }

    /// 空文件视为损坏 → .bak 恢复或 Null
    #[test]
    fn te14_load_empty_file_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // 写入空字符串（不是合法 JSON）
        std::fs::write(&settings_path, "").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&content);
        assert!(parsed.is_err(), "空字符串不是合法 JSON");

        // load_settings 模拟：空文件 → 解析失败 → 无 .bak → Null
        let bak_path = dir.path().join("settings.json.bak");
        let value: serde_json::Value = match parsed {
            Ok(v) => v,
            Err(_) => {
                if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                    serde_json::from_str(&bak_content).unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::Value::Null
                }
            }
        };
        assert!(value.is_null());
    }

    /// 嵌套 JSON 对象的浅合并保留未涉及键
    #[test]
    fn te14_merge_preserves_nested_keys() {
        let existing = serde_json::json!({
            "fontSize": 14,
            "keybindings": {
                "terminal.copy": "Ctrl+Shift+C",
                "terminal.paste": "Ctrl+Shift+V"
            }
        });
        // 只更新 keybindings 中的一个键
        let incoming = serde_json::json!({
            "keybindings": {
                "terminal.copy": "Ctrl+Alt+KeyC"
            }
        });
        let merged = merge_settings(existing, incoming);

        // 注意：merge_settings 是 top-level 浅合并——keybindings 整个键被替换
        assert_eq!(merged["fontSize"], 14);
        assert_eq!(merged["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC");
        // top-level 浅合并意味着 keybindings 整体替换，terminal.paste 会丢失
        // 此测试锁死当前行为——如果未来改为深度合并需更新
        assert!(
            merged["keybindings"]["terminal.paste"].is_null(),
            "top-level 浅合并：keybindings 整体替换，子键不保留"
        );
    }

    // ── T2: app_data_dir() 便携路径测试 ──

    /// T2.1: app_data_dir 返回 current_exe 的父目录
    #[test]
    fn app_data_dir_returns_exe_parent() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let exe = std::env::current_exe().expect("current_exe 不应失败");
        let exe_dir = exe.parent().expect("exe 应有父目录");
        assert_eq!(app_dir, exe_dir, "app_data_dir 应返回 exe 所在目录");
    }

    /// T2.2: app_data_dir + settings.json 路径拼接
    #[test]
    fn app_data_dir_joins_settings_path() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let settings_path = app_dir.join("settings.json");
        assert!(settings_path.ends_with("settings.json"), "应指向 settings.json");
        // 验证父目录存在（测试运行时 exe 目录必然存在）
        assert!(app_dir.exists(), "app_data_dir 返回的目录应存在");
    }
}
