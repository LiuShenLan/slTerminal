//! 普通设置导入导出：白名单投影，未知版本、未导出键与非法值严格分流。
//!
//! 与加密备份完全独立：只处理已登记的非敏感应用字段，不接触主题包、
//! 工作区、Hooks、凭据或任意 CLI 文件。

use std::collections::BTreeMap;

use slterm_settings::json::Value;
use slterm_settings::{RawSettings, SettingsRegistry, keys};

pub(crate) const SETTINGS_EXPORT_VERSION: u32 = 1;

/// 普通导出信封：版本 + 扁平键值；未知命名空间不进入目录。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SettingsExport {
    pub(crate) version: u32,
    pub(crate) settings: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SettingsImportError {
    UnsupportedVersion(u32),
    UnknownKey(String),
    NotExportable(String),
    InvalidValue(String),
    /// 文件不是可解析的导出信封。
    Malformed,
}

/// 导出当前生效值中属于导出白名单且通过注册表校验的字段；缺失键不合成默认值。
pub(crate) fn export_settings(raw: &RawSettings) -> SettingsExport {
    let registry = SettingsRegistry::new();
    let settings = registry
        .export_keys()
        .filter_map(|key| {
            raw.persisted_value(key)
                .filter(|value| {
                    !matches!(value, Value::Null) && registry.validate(key, value).unwrap_or(false)
                })
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect();
    SettingsExport {
        version: SETTINGS_EXPORT_VERSION,
        settings,
    }
}

/// 导出信封序列化：稳定键序与合法 JSON，数值按最短表示输出。
pub(crate) fn export_json(export: &SettingsExport) -> String {
    let mut out = String::from(
        "{
  \"version\": ",
    );
    out.push_str(&export.version.to_string());
    out.push_str(
        ",
  \"settings\": {",
    );
    for (index, (key, value)) in export.settings.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(
            "
    ",
        );
        out.push_str(&json_string(key));
        out.push_str(": ");
        push_json_value(&mut out, value);
    }
    if !export.settings.is_empty() {
        out.push_str(
            "
  ",
        );
    }
    out.push_str(
        "}
}
",
    );
    out
}

/// 导出信封解析：只接受当前结构；字段值随后仍须经导入校验。
pub(crate) fn parse_export(bytes: &[u8]) -> Result<SettingsExport, SettingsImportError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WireExport {
        version: u32,
        settings: BTreeMap<String, serde_json::Value>,
    }
    let wire: WireExport =
        serde_json::from_slice(bytes).map_err(|_| SettingsImportError::Malformed)?;
    let mut settings = BTreeMap::new();
    for (key, value) in wire.settings {
        settings.insert(key, json_to_value(value)?);
    }
    Ok(SettingsExport {
        version: wire.version,
        settings,
    })
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("字符串必须可序列化")
}

fn push_json_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => {
            let number = serde_json::Number::from_f64(*number).expect("设置数值不得为 NaN 或无穷");
            out.push_str(&number.to_string());
        }
        Value::String(text) => out.push_str(&json_string(text)),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                push_json_value(out, item);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_string(key));
                out.push_str(": ");
                push_json_value(out, item);
            }
            out.push('}');
        }
    }
}

fn json_to_value(value: serde_json::Value) -> Result<Value, SettingsImportError> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(flag) => Value::Bool(flag),
        serde_json::Value::Number(number) => {
            Value::Number(number.as_f64().ok_or(SettingsImportError::Malformed)?)
        }
        serde_json::Value::String(text) => Value::String(text),
        serde_json::Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(json_to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_json::Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| Ok((key, json_to_value(value)?)))
                .collect::<Result<Vec<_>, SettingsImportError>>()?,
        ),
    })
}

/// 导入校验：版本、登记、导出资格与字段校验全部通过才返回更新项。
pub(crate) fn import_settings(
    export: &SettingsExport,
) -> Result<Vec<(&'static str, Value)>, SettingsImportError> {
    if export.version != SETTINGS_EXPORT_VERSION {
        return Err(SettingsImportError::UnsupportedVersion(export.version));
    }
    let registry = SettingsRegistry::new();
    let mut updates = Vec::new();
    for (key, value) in &export.settings {
        let Some(definition) = keys::lookup(key) else {
            return Err(SettingsImportError::UnknownKey(key.clone()));
        };
        if !definition.exportable || definition.sensitive {
            return Err(SettingsImportError::NotExportable(key.clone()));
        }
        if matches!(value, Value::Null) || !registry.validate(key, value).unwrap_or(false) {
            return Err(SettingsImportError::InvalidValue(key.clone()));
        }
        updates.push((definition.key, value.clone()));
    }
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawSettings {
        RawSettings::from_json_bytes(text.as_bytes())
    }

    /// 导出信封稳定序列化后可原样解析；错误结构被拒绝。
    #[test]
    fn export_json_round_trips_and_rejects_malformed_documents() {
        let source = raw(
            r#"{"appearance":{"theme":"Nord","font_size":14.0},"system":{"desktop_notifications":false}}"#,
        );
        let export = export_settings(&source);
        let json = export_json(&export);
        let parsed = parse_export(json.as_bytes()).unwrap();
        assert_eq!(parsed, export);
        assert!(json.contains("\"version\": 1"));
        assert_eq!(
            parse_export(b"{not json"),
            Err(SettingsImportError::Malformed)
        );
        assert_eq!(
            parse_export(br#"{"version":1,"settings":{},"extra":true}"#),
            Err(SettingsImportError::Malformed)
        );
    }

    /// 导出只含白名单；未知版本、未知键、未导出键与非法值全部拒绝。
    #[test]
    fn settings_import_rejects_unknown_or_credentials() {
        let source = raw(r#"{
  "appearance": {"theme": "Nord", "custom_theme": "my-night", "font_size": 14.0},
  "agent_hooks": {"slterm_ai_hooks": false},
  "handmade_ns": {"token": "sk-test-secret"}
}"#);
        let export = export_settings(&source);
        assert_eq!(export.version, SETTINGS_EXPORT_VERSION);
        assert_eq!(
            export.settings.get("theme").map(Value::as_str),
            Some(Some("Nord"))
        );
        assert!(
            !export.settings.contains_key("custom_theme"),
            "未登记导出的主题引用不得进入普通导出"
        );
        assert!(
            !export.settings.contains_key("slterm_ai_hooks"),
            "Hooks 授权不进入普通导出"
        );
        assert!(
            !export.settings.contains_key("handmade_ns"),
            "未知命名空间本地保留但不导出"
        );

        assert_eq!(
            import_settings(&SettingsExport {
                version: 2,
                settings: BTreeMap::new(),
            }),
            Err(SettingsImportError::UnsupportedVersion(2))
        );
        let unknown = SettingsExport {
            version: SETTINGS_EXPORT_VERSION,
            settings: BTreeMap::from([("made_up".to_owned(), Value::Bool(true))]),
        };
        assert_eq!(
            import_settings(&unknown),
            Err(SettingsImportError::UnknownKey("made_up".to_owned()))
        );
        let hidden = SettingsExport {
            version: SETTINGS_EXPORT_VERSION,
            settings: BTreeMap::from([(
                "custom_theme".to_owned(),
                Value::String("my-night".to_owned()),
            )]),
        };
        assert_eq!(
            import_settings(&hidden),
            Err(SettingsImportError::NotExportable(
                "custom_theme".to_owned()
            )),
            "非导出键不得经导入写回"
        );
        let invalid = SettingsExport {
            version: SETTINGS_EXPORT_VERSION,
            settings: BTreeMap::from([("theme".to_owned(), Value::Bool(true))]),
        };
        assert_eq!(
            import_settings(&invalid),
            Err(SettingsImportError::InvalidValue("theme".to_owned()))
        );

        let accepted = import_settings(&export).unwrap();
        assert!(accepted.iter().any(|(key, _)| *key == "theme"));
        assert!(
            accepted
                .iter()
                .all(|(key, _)| keys::lookup(key).is_some_and(|def| def.exportable)),
            "合法导入项必须全部属于导出白名单"
        );
    }

    /// 导出必须过滤注册表不接受的原始值，保证导出的信封可再次导入。
    #[test]
    fn export_omits_invalid_values_so_round_trip_import_succeeds() {
        let source = raw(
            r#"{"appearance":{"font_size":"large","theme":"Nord"},"system":{"desktop_notifications":false}}"#,
        );
        let export = export_settings(&source);
        assert!(
            !export.settings.contains_key("font_size"),
            "非法字号不得进入导出"
        );
        assert_eq!(
            export.settings.get("theme").and_then(Value::as_str),
            Some("Nord")
        );
        let parsed = parse_export(export_json(&export).as_bytes()).unwrap();
        import_settings(&parsed).expect("导出的信封必须可再次导入");
    }

    /// 导入不触碰凭据与 Hooks 授权；恢复默认同样保留这些事实。
    #[test]
    fn settings_import_and_reset_preserve_secrets() {
        use slterm_settings::{AgentHook, RawSettings, RuntimeSettings};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{
  "appearance": {"theme": "Nord"},
  "agent_hooks": {"slterm_ai_hooks": false, "slterm_ai_hooks_codex": false},
  "handmade_ns": {"token": "sk-test-secret"}
}"#,
        )
        .unwrap();

        // 合法导入只写白名单字段，授权与未知命名空间原地保留。
        let export = SettingsExport {
            version: SETTINGS_EXPORT_VERSION,
            settings: BTreeMap::from([("theme".to_owned(), Value::String("Linear".to_owned()))]),
        };
        crate::settings_store::save_settings_blocking_at(&path, &import_settings(&export).unwrap())
            .unwrap();
        let after_import = RawSettings::try_load_at(&path).unwrap();
        for agent in AgentHook::ALL {
            assert!(!agent.enabled(&after_import), "导入不得改写 Hooks 授权");
        }
        assert_eq!(
            RuntimeSettings::from_raw(&after_import).theme,
            slterm_settings::ThemeName::Linear
        );
        assert!(
            after_import.tree().get("handmade_ns").is_some(),
            "未知命名空间必须原样保留"
        );

        let captured = crate::audit::test_support::capture_events();
        crate::settings_store::restore_default_settings_at(&path)
            .unwrap()
            .unwrap();
        let audit_text = captured.text();
        assert!(
            audit_text.contains("operation=\"settings_reset\"")
                && audit_text.contains("result=\"ok\""),
            "恢复默认必须经审计通道留痕: {audit_text}"
        );
        let after_reset = RawSettings::try_load_at(&path).unwrap();
        for agent in AgentHook::ALL {
            assert!(
                !agent.enabled(&after_reset),
                "恢复默认不得删除 Hooks 授权或凭据事实"
            );
        }
        assert_eq!(
            after_reset
                .tree()
                .get("handmade_ns")
                .and_then(|ns| ns.get("token"))
                .and_then(Value::as_str),
            Some("sk-test-secret"),
            "恢复默认不得删除未知命名空间"
        );
    }
}
