//! 应用数据根的平台解析与启动缓存。
//!
//! `AppPaths` 是纯值对象；本模块只在启动入口读取双根覆盖变量，缺省经
//! Windows Known Folder API 解析。生产调用点只读启动缓存，不重复读取环境。

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use slterm_path::{AppPaths, AppPathsError};

/// 用户主目录。Windows 优先 `USERPROFILE`,回退 `HOME`;空串视为
/// 未设置(登录管理器实踩过)。
#[allow(dead_code)] // 消费点 = 初始 cwd 回退(显式目录无效时落 home)。
pub fn home_dir() -> Option<PathBuf> {
    home_dir_from(|name| std::env::var_os(name))
}

fn home_dir_from(lookup: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    for variable in ["USERPROFILE", "HOME"] {
        if let Some(value) = lookup(variable) {
            let path = PathBuf::from(value);
            if !path.as_os_str().is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// Windows 已知文件夹；测试用封闭身份替代 GUID。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KnownFolder {
    RoamingAppData,
    LocalAppData,
}

/// 启动数据根解析结果；覆盖事实供调试演练校验复用。
#[derive(Debug)]
struct ResolvedAppPaths {
    paths: AppPaths,
    #[cfg(any(test, feature = "update-test-source"))]
    config_overridden: bool,
    #[cfg(any(test, feature = "update-test-source"))]
    local_overridden: bool,
}

#[cfg(any(test, feature = "update-test-source"))]
impl ResolvedAppPaths {
    fn data_roots_are_overridden(&self) -> bool {
        self.config_overridden && self.local_overridden
    }
}

/// 数据根解析失败；不静默回退到另一目录。
#[derive(Debug)]
pub(crate) enum PathResolutionError {
    Invalid(AppPathsError),
    KnownFolder {
        folder: KnownFolder,
        message: String,
    },
    CreateDirectory {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for PathResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(error) => std::fmt::Display::fmt(error, formatter),
            Self::KnownFolder { folder, message } => {
                write!(formatter, "无法解析 Windows 数据目录 {folder:?}：{message}")
            }
            Self::CreateDirectory { path, message } => {
                write!(formatter, "无法创建数据目录 {}：{message}", path.display())
            }
        }
    }
}

impl std::error::Error for PathResolutionError {}

fn resolve_from(
    mut variable: impl FnMut(&str) -> Option<OsString>,
    mut known_folder: impl FnMut(KnownFolder) -> Result<PathBuf, PathResolutionError>,
) -> Result<ResolvedAppPaths, PathResolutionError> {
    let config_override = variable("SLTERM_CONFIG_DIR").map(PathBuf::from);
    #[cfg(any(test, feature = "update-test-source"))]
    let config_overridden = config_override.is_some();
    let config_root = match config_override {
        Some(path) => path,
        None => known_folder(KnownFolder::RoamingAppData)?.join("slterm"),
    };
    let local_override = variable("SLTERM_LOCAL_DIR").map(PathBuf::from);
    #[cfg(any(test, feature = "update-test-source"))]
    let local_overridden = local_override.is_some();
    let local_root = match local_override {
        Some(path) => path,
        None => known_folder(KnownFolder::LocalAppData)?.join("slterm"),
    };
    Ok(ResolvedAppPaths {
        paths: AppPaths::new(config_root, local_root).map_err(PathResolutionError::Invalid)?,
        #[cfg(any(test, feature = "update-test-source"))]
        config_overridden,
        #[cfg(any(test, feature = "update-test-source"))]
        local_overridden,
    })
}

fn create_roots(paths: &AppPaths) -> Result<(), PathResolutionError> {
    for root in [paths.config_root(), paths.local_root()] {
        std::fs::create_dir_all(root).map_err(|error| PathResolutionError::CreateDirectory {
            path: root.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    Ok(())
}

static RESOLVED_PATHS: OnceLock<ResolvedAppPaths> = OnceLock::new();

/// 启动入口唯一的环境读取与目录创建点；失败不降级到其他数据根。
pub(crate) fn initialize() -> Result<&'static AppPaths, PathResolutionError> {
    if let Some(resolved) = RESOLVED_PATHS.get() {
        return Ok(&resolved.paths);
    }
    let resolved = resolve_from(|name| std::env::var_os(name), known_folder)?;
    create_roots(&resolved.paths)?;
    let _ = RESOLVED_PATHS.set(resolved);
    Ok(&RESOLVED_PATHS
        .get()
        .expect("AppPaths 初始化后必须存在")
        .paths)
}

#[cfg(not(test))]
fn resolution() -> &'static ResolvedAppPaths {
    RESOLVED_PATHS.get().expect("启动入口必须先初始化 AppPaths")
}

/// 测试进程共享隔离双根；不读取真实环境或用户目录。
#[cfg(test)]
fn resolution() -> &'static ResolvedAppPaths {
    RESOLVED_PATHS.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("slterm-test-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths::new(root.join("config"), root.join("local")).unwrap();
        create_roots(&paths).unwrap();
        ResolvedAppPaths {
            paths,
            config_overridden: true,
            local_overridden: true,
        }
    })
}

pub(crate) fn app_paths() -> &'static AppPaths {
    &resolution().paths
}

pub(crate) fn config_dir() -> &'static Path {
    app_paths().config_root()
}

pub(crate) fn local_dir() -> &'static Path {
    app_paths().local_root()
}

#[cfg(any(test, feature = "update-test-source"))]
pub(crate) fn data_roots_are_overridden() -> bool {
    resolution().data_roots_are_overridden()
}

fn known_folder(folder: KnownFolder) -> Result<PathBuf, PathResolutionError> {
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };

    let id = match folder {
        KnownFolder::RoamingAppData => &FOLDERID_RoamingAppData,
        KnownFolder::LocalAppData => &FOLDERID_LocalAppData,
    };
    // SAFETY: 在释放 COM 分配前复制完整 UTF-16 路径。
    unsafe {
        let path = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|error| {
            PathResolutionError::KnownFolder {
                folder,
                message: error.to_string(),
            }
        })?;
        let directory = OsString::from_wide(path.as_wide());
        CoTaskMemFree(Some(path.0.cast()));
        Ok(PathBuf::from(directory))
    }
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

    /// USERPROFILE 缺失时回退 HOME;空 USERPROFILE 不阻挡回退。
    #[test]
    fn home_dir_prefers_userprofile_and_falls_back_to_home() {
        use std::ffi::OsString;

        let both = |name: &str| match name {
            "USERPROFILE" => Some(OsString::from("C:/Users/test")),
            "HOME" => Some(OsString::from("D:/home")),
            _ => None,
        };
        assert_eq!(home_dir_from(both), Some(PathBuf::from("C:/Users/test")));
        let only_home = |name: &str| (name == "HOME").then(|| OsString::from("D:/home"));
        assert_eq!(home_dir_from(only_home), Some(PathBuf::from("D:/home")));
        let empty_userprofile = |name: &str| (name == "USERPROFILE").then(OsString::new);
        assert_eq!(home_dir_from(empty_userprofile), None);
    }
}

#[cfg(test)]
mod app_paths_resolution_tests {
    use super::*;

    fn default_folder(folder: KnownFolder) -> Result<PathBuf, PathResolutionError> {
        Ok(PathBuf::from(match folder {
            KnownFolder::RoamingAppData => "C:/roaming",
            KnownFolder::LocalAppData => "C:/local",
        }))
    }

    #[test]
    fn resolution_uses_isolated_distinct_absolute_roots() {
        let resolved = resolution();
        for root in [resolved.paths.config_root(), resolved.paths.local_root()] {
            assert!(root.is_absolute(), "测试数据根必须是绝对路径: {root:?}");
            assert!(
                root.starts_with(std::env::temp_dir()),
                "测试构建必须隔离到临时目录: {root:?}"
            );
        }
        assert_ne!(resolved.paths.config_root(), resolved.paths.local_root());
        assert!(resolved.config_overridden);
    }

    #[test]
    fn app_paths_resolve_prefers_absolute_overrides_and_reads_each_variable_once() {
        let mut lookups = Vec::new();
        let resolved = resolve_from(
            |name| {
                lookups.push(name.to_owned());
                match name {
                    "SLTERM_CONFIG_DIR" => Some(OsString::from("D:/config-override")),
                    "SLTERM_LOCAL_DIR" => Some(OsString::from("E:/local-override")),
                    _ => None,
                }
            },
            |_| panic!("两个覆盖均存在时不得解析 Known Folder"),
        )
        .unwrap();

        assert_eq!(
            resolved.paths.config_root(),
            Path::new("D:/config-override")
        );
        assert_eq!(resolved.paths.local_root(), Path::new("E:/local-override"));
        assert!(resolved.config_overridden);
        assert!(resolved.data_roots_are_overridden());
        assert_eq!(lookups, ["SLTERM_CONFIG_DIR", "SLTERM_LOCAL_DIR"]);
    }

    #[test]
    fn app_paths_resolve_uses_distinct_known_folder_defaults() {
        let resolved = resolve_from(|_| None, default_folder).unwrap();

        assert_eq!(resolved.paths.config_root(), Path::new("C:/roaming/slterm"));
        assert_eq!(resolved.paths.local_root(), Path::new("C:/local/slterm"));
        assert!(!resolved.config_overridden);
    }

    #[test]
    fn app_paths_rehearsal_isolation_requires_both_overrides() {
        for (config, local) in [
            (Some("D:/config"), None),
            (None, Some("E:/local")),
            (None, None),
        ] {
            let resolved = resolve_from(
                |name| match name {
                    "SLTERM_CONFIG_DIR" => config.map(OsString::from),
                    "SLTERM_LOCAL_DIR" => local.map(OsString::from),
                    _ => None,
                },
                default_folder,
            )
            .unwrap();
            assert!(
                !resolved.data_roots_are_overridden(),
                "缺少任一覆盖都不得判定为隔离双根"
            );
        }
    }

    #[test]
    fn app_paths_resolve_rejects_relative_and_empty_overrides() {
        for (config, local, expect_config) in [
            (Some("relative/config"), None, true),
            (Some(""), None, true),
            (None, Some("relative/local"), false),
            (None, Some(""), false),
        ] {
            let result = resolve_from(
                |name| match name {
                    "SLTERM_CONFIG_DIR" => config.map(OsString::from),
                    "SLTERM_LOCAL_DIR" => local.map(OsString::from),
                    _ => None,
                },
                default_folder,
            );
            if expect_config {
                assert!(matches!(
                    result,
                    Err(PathResolutionError::Invalid(
                        AppPathsError::ConfigRootNotAbsolute(_)
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(PathResolutionError::Invalid(
                        AppPathsError::LocalRootNotAbsolute(_)
                    ))
                ));
            }
        }
    }

    #[test]
    fn app_paths_resolve_surfaces_known_folder_failures() {
        for failed in [KnownFolder::RoamingAppData, KnownFolder::LocalAppData] {
            let result = resolve_from(
                |_| None,
                |folder| {
                    if folder == failed {
                        Err(PathResolutionError::KnownFolder {
                            folder,
                            message: "injected".into(),
                        })
                    } else {
                        default_folder(folder)
                    }
                },
            );
            assert!(matches!(
                result,
                Err(PathResolutionError::KnownFolder { folder, .. }) if folder == failed
            ));
        }
    }

    #[test]
    fn app_paths_create_roots_creates_both_directories() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();

        create_roots(&paths).unwrap();

        assert!(paths.config_root().is_dir());
        assert!(paths.local_root().is_dir());
    }

    #[test]
    fn app_paths_create_roots_rejects_a_file_at_either_root() {
        for occupy_config in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
            let (occupied, other) = if occupy_config {
                (paths.config_root(), paths.local_root())
            } else {
                (paths.local_root(), paths.config_root())
            };
            std::fs::write(occupied, "occupied").unwrap();

            assert!(matches!(
                create_roots(&paths),
                Err(PathResolutionError::CreateDirectory { path, .. }) if path == occupied
            ));
            if occupy_config {
                assert!(!other.exists(), "配置根失败后不得继续创建本地根");
            } else {
                assert!(other.is_dir(), "本地根失败必须来自实际创建尝试");
            }
        }
    }

    #[test]
    fn app_paths_resolve_preserves_non_unicode_overrides() {
        use std::os::windows::ffi::OsStringExt;

        let config = OsString::from_wide(&[b'C' as u16, b':' as u16, b'\\' as u16, 0xD800]);
        let local = OsString::from_wide(&[b'D' as u16, b':' as u16, b'\\' as u16, 0xD801]);
        let expected_config = PathBuf::from(&config);
        let expected_local = PathBuf::from(&local);
        let resolved = resolve_from(
            |name| match name {
                "SLTERM_CONFIG_DIR" => Some(config.clone()),
                "SLTERM_LOCAL_DIR" => Some(local.clone()),
                _ => None,
            },
            |_| panic!("覆盖路径不应触发 Known Folder"),
        )
        .unwrap();

        assert_eq!(resolved.paths.config_root(), expected_config);
        assert_eq!(resolved.paths.local_root(), expected_local);
    }
}
