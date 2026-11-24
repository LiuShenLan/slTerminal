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
}

/// 导出当前生效值中属于导出白名单的字段；缺失键不合成默认值。
pub(crate) fn export_settings(raw: &RawSettings) -> SettingsExport {
    let registry = SettingsRegistry::new();
    let settings = registry
        .export_keys()
        .filter_map(|key| {
            raw.persisted_value(key)
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect();
    SettingsExport {
        version: SETTINGS_EXPORT_VERSION,
        settings,
    }
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

    /// 导入不触碰凭据与 Hooks 授权；恢复默认同样保留这些事实。
    #[test]
    fn settings_import_and_reset_preserve_secrets() {
        use slterm_settings::{
            AgentHook, RawSettings, RuntimeSettings, restore_default_settings_at,
        };

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

        restore_default_settings_at(&path).unwrap().unwrap();
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
