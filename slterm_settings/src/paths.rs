//! 设置文件路径唯一权威:`SLTERM_CONFIG_DIR` 优先(空串视为未设置),
//! 默认 `%APPDATA%\slterm`(数据目录叶子名 D2 裁决值)。
//!
//! 单名环境变量,双名别名层不迁(无历史用户);零迁移:旧 exe 同级便携
//! 文件不看不迁。仅 Win10/11,路径推导无平台分支。

use std::ffi::OsString;
use std::path::PathBuf;

/// 路径优先级的注入式内核:环境读取由调用方传入,测试不触碰真环境。
fn override_dir_from(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<PathBuf> {
    variable("SLTERM_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 默认数据目录根的注入式内核:APPDATA → USERPROFILE → temp 回退链,
/// 空串视为未设置。
fn default_dir_from(mut variable: impl FnMut(&str) -> Option<OsString>) -> PathBuf {
    let root = variable("APPDATA")
        .filter(|value| !value.is_empty())
        .or_else(|| variable("USERPROFILE").filter(|value| !value.is_empty()))
        .map(PathBuf::from);
    root.unwrap_or_else(std::env::temp_dir).join("slterm")
}

/// 应用数据目录。
pub fn settings_dir() -> PathBuf {
    override_dir_from(|name| std::env::var_os(name))
        .unwrap_or_else(|| default_dir_from(|name| std::env::var_os(name)))
}

/// 设置文件路径:数据目录下 `settings.json`(去品牌命名,D3 裁决)。
pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

#[cfg(test)]
mod paths_tests {
    use super::*;

    fn some(text: &str) -> Option<OsString> {
        Some(OsString::from(text))
    }

    #[test]
    fn config_dir_override_wins_and_empty_values_are_ignored() {
        assert_eq!(
            override_dir_from(|_| some("D:/custom")),
            Some(PathBuf::from("D:/custom"))
        );
        // 空串视为未设置 → 落入默认目录链。
        assert_eq!(override_dir_from(|_| some("")), None);
        assert_eq!(override_dir_from(|_| None), None);
    }

    #[test]
    fn default_dir_prefers_appdata_then_userprofile_then_temp() {
        let both = default_dir_from(|name| match name {
            "APPDATA" => some("C:/Users/u/AppData/Roaming"),
            "USERPROFILE" => some("C:/Users/u"),
            _ => None,
        });
        assert_eq!(
            both,
            PathBuf::from("C:/Users/u/AppData/Roaming").join("slterm")
        );

        let profile_only = default_dir_from(|name| {
            (name == "USERPROFILE")
                .then(|| some("C:/Users/u"))
                .flatten()
        });
        assert_eq!(profile_only, PathBuf::from("C:/Users/u").join("slterm"));

        let empty_appdata = default_dir_from(|name| match name {
            "APPDATA" => some(""),
            "USERPROFILE" => some("C:/Users/u"),
            _ => None,
        });
        assert_eq!(empty_appdata, PathBuf::from("C:/Users/u").join("slterm"));

        let fallback = default_dir_from(|_| None);
        assert_eq!(fallback, std::env::temp_dir().join("slterm"));
    }

    #[test]
    fn settings_file_is_the_json_document_inside_the_data_dir() {
        assert_eq!(settings_path(), settings_dir().join("settings.json"));
        assert_eq!(
            settings_path().extension().and_then(|ext| ext.to_str()),
            Some("json")
        );
    }
}
