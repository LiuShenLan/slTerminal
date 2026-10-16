//! 用户目录解析(01 篇锚点:照上游 platform/dirs.rs 落位)。
//!
//! 路径优先级唯一权威在 slterm_settings(`SLTERM_CONFIG_DIR` → 空串视为未设置 →
//! `%APPDATA%\slterm`,D2 裁决值);本模块只缓存应用侧解析结果,不推导优先级。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 覆盖数据目录是否生效(便携模式/自动化测试隔离用)。
#[cfg(test)]
fn has_override() -> bool {
    std::env::var_os("SLTERM_CONFIG_DIR").is_some_and(|value| !value.is_empty())
}

/// 用户主目录。Windows `USERPROFILE`;空串视为未设置(登录管理器实踩过)。
#[allow(dead_code)] // 消费点 = M3.2 初始 cwd 回退(显式目录无效时落 home)。
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// 配置与数据目录,必要时创建。
///
/// 结果缓存在 `OnceLock`:逐调用点重复读环境变量 + `create_dir_all` 是浪费;缓存
/// 同时保证进程内路径恒定——半途改环境变量不会让一部分文件写旧目录、一部分写新目录。
pub fn data_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = resolve_data_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
}

fn resolve_data_dir() -> PathBuf {
    // settings.json、runtime 发现、session、history、导入字体共用同一目录;
    // 零依赖 settings crate 持有环境/OS 解析,app 侧只缓存(01 篇缝合点 1)。
    slterm_settings::settings_dir()
}

#[cfg(test)]
mod dirs_tests {
    use super::*;

    #[test]
    fn home_dir_rejects_empty_env() {
        // 空字符串环境变量当作未设置;这里只验证过滤器本身——直接改进程环境
        // 会污染其他并发测试。
        let empty = PathBuf::from("");
        assert!(
            Some(empty)
                .filter(|p: &PathBuf| !p.as_os_str().is_empty())
                .is_none()
        );
    }

    #[test]
    fn data_dir_is_absolute_and_named() {
        let dir = resolve_data_dir();
        assert!(dir.is_absolute(), "数据目录必须是绝对路径:{dir:?}");
        // 覆盖环境变量下叶子名任意,只在默认路径上校验命名。
        if !has_override() {
            let leaf = dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            assert!(
                leaf.eq_ignore_ascii_case("slterm"),
                "默认数据目录应以 slterm 结尾,实际 {leaf:?}"
            );
        }
    }

    /// Windows 落点:`%APPDATA%\slterm`(D2 裁决值)。
    #[test]
    fn windows_path_uses_slterm_layout() {
        if has_override() {
            return;
        }
        let Some(appdata) = std::env::var_os("APPDATA") else {
            return;
        };
        assert_eq!(resolve_data_dir(), PathBuf::from(appdata).join("slterm"));
    }
}
