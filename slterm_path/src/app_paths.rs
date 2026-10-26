use std::fmt;
use std::path::{Path, PathBuf};

/// 应用数据目录的不可变值；环境与 Known Folder 解析由 application 入口完成。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppPaths {
    config_root: PathBuf,
    local_root: PathBuf,
}

/// 数据根构造错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppPathsError {
    /// 配置根不是绝对路径。
    ConfigRootNotAbsolute(PathBuf),
    /// 本地根不是绝对路径。
    LocalRootNotAbsolute(PathBuf),
}

impl fmt::Display for AppPathsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigRootNotAbsolute(path) => {
                write!(formatter, "配置根必须为绝对路径：{}", path.display())
            }
            Self::LocalRootNotAbsolute(path) => {
                write!(formatter, "本地根必须为绝对路径：{}", path.display())
            }
        }
    }
}

impl std::error::Error for AppPathsError {}

impl AppPaths {
    /// 从绝对配置根与本地根构造数据路径；不读取环境或文件系统。
    pub fn new(
        config_root: impl Into<PathBuf>,
        local_root: impl Into<PathBuf>,
    ) -> Result<Self, AppPathsError> {
        let config_root = config_root.into();
        if !config_root.is_absolute() {
            return Err(AppPathsError::ConfigRootNotAbsolute(config_root));
        }
        let local_root = local_root.into();
        if !local_root.is_absolute() {
            return Err(AppPathsError::LocalRootNotAbsolute(local_root));
        }
        Ok(Self {
            config_root,
            local_root,
        })
    }

    /// 配置根。
    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    /// 本地根。
    pub fn local_root(&self) -> &Path {
        &self.local_root
    }

    /// `settings.json` 的完整路径。
    pub fn settings_path(&self) -> PathBuf {
        self.config_root.join("settings.json")
    }

    /// `workspace.db` 的完整路径。
    pub fn workspace_db_path(&self) -> PathBuf {
        self.local_root.join("workspace.db")
    }

    /// `learning.db` 的完整路径。
    pub fn learning_db_path(&self) -> PathBuf {
        self.local_root.join("learning.db")
    }
}

#[cfg(test)]
mod app_paths_tests {
    use super::*;

    fn absolute_root(name: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:/{name}"))
        } else {
            PathBuf::from(format!("/{name}"))
        }
    }

    /// 设置归属配置根，两个数据库归属本地根。
    #[test]
    fn app_paths_scopes_files_to_their_data_roots() {
        let config = absolute_root("config");
        let local = absolute_root("local");
        let paths = AppPaths::new(config.clone(), local.clone()).unwrap();
        assert_eq!(paths.config_root(), config.as_path());
        assert_eq!(paths.local_root(), local.as_path());
        assert_eq!(paths.settings_path(), config.join("settings.json"));
        assert_eq!(paths.workspace_db_path(), local.join("workspace.db"));
        assert_eq!(paths.learning_db_path(), local.join("learning.db"));
    }

    /// 两个数据根都必须在构造时是绝对路径，且不依赖文件系统存在性。
    #[test]
    fn app_paths_rejects_relative_roots() {
        let config_result = AppPaths::new("relative/config", absolute_root("local"));
        assert!(matches!(
            &config_result,
            Err(AppPathsError::ConfigRootNotAbsolute(path))
                if path == Path::new("relative/config")
        ));

        let local_result = AppPaths::new(absolute_root("config"), "relative/local");
        assert!(matches!(
            &local_result,
            Err(AppPathsError::LocalRootNotAbsolute(path))
                if path == Path::new("relative/local")
        ));
    }

    /// 错误实现标准错误接口，并指出哪个根未通过校验。
    #[test]
    fn app_paths_errors_describe_the_rejected_root() {
        fn require_error<T: std::error::Error>() {}
        require_error::<AppPathsError>();

        let config = AppPathsError::ConfigRootNotAbsolute(PathBuf::from("relative/config"));
        assert_eq!(config.to_string(), "配置根必须为绝对路径：relative/config");
        let local = AppPathsError::LocalRootNotAbsolute(PathBuf::from("relative/local"));
        assert_eq!(local.to_string(), "本地根必须为绝对路径：relative/local");
    }
}
