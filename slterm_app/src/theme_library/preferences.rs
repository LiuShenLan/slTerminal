//! 主题应用键的条件发布桥。
//!
//! 值语义仍归 `slterm-settings`;本适配层只负责把主题文档翻译成设置键,
//! 并携带编辑动作前观察到的设置字节快照走写通道
//! (`save_settings_checked_at`)。外部写入者先于本次提交修改设置时,
//! 提交以 `AlreadyExists` 拒绝——主题应用不可能静默覆盖用户手改。
//!
//! 缺失的可选主题值保持用户个人偏好;只有文档显式声明的值才写入。
//!
//! 生产应用路径统一经 `SettingsPane::try_persist_checked`(它持显式目标
//! 路径);下列显式路径入口保留为桥接 API,测试直接消费。
#![allow(dead_code)]

use std::io;
use std::path::Path;

use slterm_settings::json::Value;
use slterm_settings::{Rgb8, format_hex_rgb};

use crate::settings_store::{self, SettingsRevision};
use crate::theme_library::{DocumentError, ThemeDocument};

/// 主题应用前观察到的设置文件字节快照(缺失 = None)。
pub(crate) type PreferenceRevision = SettingsRevision;

/// 读取生产数据目录的设置快照。
pub(crate) fn load() -> io::Result<PreferenceRevision> {
    settings_store::read_revision()
}

/// 读取显式路径的设置快照(测试隔离)。
pub(crate) fn load_at(path: &Path) -> io::Result<PreferenceRevision> {
    settings_store::read_revision_at(path)
}

/// 条件发布:快照不匹配即拒绝,成功返回新的快照。
pub(crate) fn save(
    expected: &PreferenceRevision,
    updates: &[(&'static str, Value)],
) -> io::Result<PreferenceRevision> {
    settings_store::save_settings_checked(expected, updates)
}

pub(crate) fn save_at(
    path: &Path,
    expected: &PreferenceRevision,
    updates: &[(&'static str, Value)],
) -> io::Result<PreferenceRevision> {
    settings_store::save_settings_checked_at(path, expected, updates)
}

/// 把一次明确的应用动作翻译成成组设置键。选择器与编辑器共用本函数,
/// 因此既有个人覆盖不会被预览值意外遮蔽;未声明的可选值保持个人偏好。
/// 本函数不触盘、不应用主题。
pub(crate) fn custom_theme_updates(
    document: &ThemeDocument,
    foreground: Option<Rgb8>,
) -> Result<Vec<(&'static str, Value)>, DocumentError> {
    let id = document
        .id()
        .filter(|_| !document.is_builtin())
        .ok_or_else(|| {
            DocumentError::Invalid("applying a custom theme requires a saved library id".to_owned())
        })?;
    let definition = document.definition()?;
    // 主题只应用身份、基础主题与显式前景覆盖;字体、字号、材质、卡片几何、
    // 光标形状与壁纸始终由用户设置拥有。
    let updates = vec![
        ("custom_theme", Value::String(id.to_owned())),
        (
            "theme",
            Value::String(definition.base.prompt_name().to_owned()),
        ),
        (
            "theme_foreground",
            foreground.map_or(Value::Null, |color| Value::String(format_hex_rgb(color))),
        ),
    ];
    Ok(updates)
}

#[cfg(test)]
mod preferences_tests {
    use super::*;
    use crate::theme_library::{ThemeLibraryStore, from_definition};
    use slterm_settings::{RawSettings, RuntimeSettings, ThemeDefinition, ThemeName};

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, ThemeLibraryStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = ThemeLibraryStore::new(directory.path().join("themes"));
        (directory, path, store)
    }

    fn runtime(path: &Path) -> RuntimeSettings {
        RuntimeSettings::from_raw(&RawSettings::from_json_bytes(
            &std::fs::read(path).unwrap_or_default(),
        ))
    }

    #[test]
    fn applying_a_theme_writes_only_theme_keys_and_keeps_personal_appearance() {
        let (_directory, path, store) = fixture();
        let definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        let document = store
            .import(&from_definition(&definition).unwrap())
            .unwrap();
        std::fs::write(
            &path,
            r##"{"appearance":{"font_size":14,"font_family":"Old Font","opacity":0.5,"blur":"none","pane_card_radius":2,"theme_foreground":"#ffffff"}}"##,
        )
        .unwrap();
        let foreground = [0xf0, 0xd0, 0xa0];
        let updates = custom_theme_updates(&document, Some(foreground)).unwrap();
        assert_eq!(
            updates.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            ["custom_theme", "theme", "theme_foreground"],
            "主题应用不得改写字体、材质、卡片几何或其他非颜色键"
        );
        let revision = load_at(&path).unwrap();
        let revision = save_at(&path, &revision, &updates).unwrap();
        assert_eq!(load_at(&path).unwrap(), revision);

        let runtime = runtime(&path);
        assert_eq!(runtime.custom_theme.as_deref(), document.id());
        assert_eq!(runtime.theme, ThemeName::Nord);
        assert_eq!(runtime.theme_foreground, Some(foreground));
        // 个人外观值逐项保留。
        assert_eq!(runtime.font_size_px, Some(14.0));
        assert_eq!(runtime.font_family.as_deref(), Some("Old Font"));
        assert!((runtime.opacity - 0.5).abs() < 1e-6);
        assert_eq!(runtime.blur, slterm_settings::BlurMode::None);
        assert_eq!(runtime.pane_card_radius, Some(2.0));
    }

    #[test]
    fn absent_theme_options_preserve_personal_settings() {
        let (_directory, path, store) = fixture();
        let definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        let document = store
            .import(&from_definition(&definition).unwrap())
            .unwrap();
        std::fs::write(
            &path,
            r#"{"appearance":{"font_family":"My Font","font_size":19,"opacity":0.85,"blur":"mica"}}"#,
        )
        .unwrap();
        let revision = load_at(&path).unwrap();
        let foreground = [0xf0, 0xd0, 0xa0];
        let updates = custom_theme_updates(&document, Some(foreground)).unwrap();
        save_at(&path, &revision, &updates).unwrap();
        let runtime = runtime(&path);
        assert_eq!(runtime.font_family.as_deref(), Some("My Font"));
        assert_eq!(runtime.font_size_px, Some(19.0));
        assert_eq!(runtime.opacity, 0.85);
        assert_eq!(runtime.blur, slterm_settings::BlurMode::Mica);
        assert_eq!(runtime.theme_foreground, Some(foreground));
    }

    #[test]
    fn stale_preference_revision_cannot_overwrite_an_external_edit() {
        let (_directory, path, store) = fixture();
        let definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        let document = store
            .import(&from_definition(&definition).unwrap())
            .unwrap();
        let observed = load_at(&path).unwrap();
        std::fs::write(&path, "{\"appearance\":{\"font_size\":19}}").unwrap();
        let external = std::fs::read(&path).unwrap();
        let updates = custom_theme_updates(&document, None).unwrap();
        let error = save_at(&path, &observed, &updates).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), external);
    }

    #[test]
    fn builtin_documents_cannot_be_applied_as_custom_theme_keys() {
        let document = crate::theme_library::document::builtin_document(ThemeName::Nord).unwrap();
        let error = custom_theme_updates(&document, None).unwrap_err();
        assert!(error.to_string().contains("library id"));
    }
}
