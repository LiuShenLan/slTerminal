/// 用户 home 目录解析单点（E2E 假 home 隔离键 = USERPROFILE）
///
/// 模块间不穿透纪律（硬约束 #2）：home 目录（`~`）是多个子模块（hooks/claude、
/// plan_balance、hooks/watcher、agent_history）共享的目录根，解析收编于本顶层
/// 单文件模块（照 `app_dir.rs` 之于应用数据目录的先例）——子模块禁止自建
/// home 解析/守卫（曾有两份照抄复制，已收敛于此）。
///
/// Windows 事实（勿改，ADR-0016）：`dirs` 6.0.0 / `dirs-sys` 0.5.0 的
/// `home_dir()` 走 `SHGetKnownFolderPath(known_folder_profile)`，**完全不读
/// USERPROFILE/HOME 环境变量**——故生产 home 解析一律经本模块显式 env-first；
/// 生产代码禁止裸 `dirs::home_dir()`（grep 收敛纪律，仅 cfg(test) 上下文允许）。
/// Node `os.homedir()`（libuv uv_os_homedir）每调用重读 USERPROFILE——e2e spec
/// 与注入的 JS 脚本自动跟随同一键（Windows env 键大小写不敏感，统一大写书写）。
use std::path::PathBuf;

/// home 目录环境变量覆盖键（E2E 假 home 隔离）：显式指定 home 目录
const USER_PROFILE_ENV: &str = "USERPROFILE";

/// 测试用：home_dir() 覆盖注入槽（仅测试编译，生产零行为变更）
#[cfg(test)]
static HOME_DIR_OVERRIDE: parking_lot::Mutex<Option<PathBuf>> = parking_lot::Mutex::new(None);

/// 测试用 RAII 守卫：把 home_dir() 指向指定目录，Drop 时恢复原值
/// （防测试 panic 残留覆盖污染后续用例；与 app_dir::AppDataDirGuard 静态互异、
/// 互斥锁独立，双守卫嵌套持用合法——agent_history scan fallback 用例即用双守卫）
#[cfg(test)]
pub(crate) struct HomeDirGuard(Option<PathBuf>);

#[cfg(test)]
impl HomeDirGuard {
    pub(crate) fn set(dir: &std::path::Path) -> Self {
        let mut slot = HOME_DIR_OVERRIDE.lock();
        let prev = slot.clone();
        *slot = Some(dir.to_path_buf());
        HomeDirGuard(prev)
    }
}

#[cfg(test)]
impl Drop for HomeDirGuard {
    fn drop(&mut self) {
        *HOME_DIR_OVERRIDE.lock() = self.0.clone();
    }
}

/// 统一 home 解析。优先级：测试 guard（cfg(test)，生产零编译）>
/// USERPROFILE env（非空，空串视为未设置）> dirs::home_dir()（与 app_dir.rs
/// 注释口径一致）。正常机器上 USERPROFILE 与 FOLDERID_Profile 同值，生产行为
/// 零漂移——env-first 只在 E2E 隔离场景显式覆盖时改变结果。
pub(crate) fn home_dir() -> Option<PathBuf> {
    // 测试注入覆盖（仅测试编译，生产恒走 env/dirs 路径）
    #[cfg(test)]
    {
        if let Some(dir) = HOME_DIR_OVERRIDE.lock().clone() {
            return Some(dir);
        }
    }
    // E2E 假 home 隔离：环境变量显式指定 home 目录（空串视为未设置）
    std::env::var_os(USER_PROFILE_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

#[cfg(test)]
mod home_tests {
    use super::*;

    /// RAII env 恢复守卫：保存/恢复 USERPROFILE 原值（开发机恒有真值）
    struct EnvRestoreGuard;
    impl EnvRestoreGuard {
        fn acquire() -> Self {
            EnvRestoreGuard
        }
    }
    impl Drop for EnvRestoreGuard {
        fn drop(&mut self) {
            std::env::remove_var(USER_PROFILE_ENV);
        }
    }

    /// 测试守卫生效：覆盖 home_dir 返回注入目录（命令层测试的注入机制）
    #[test]
    fn home_dir_honors_guard_override() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        assert_eq!(
            home_dir().unwrap(),
            dir.path(),
            "守卫应覆盖 home_dir 返回值"
        );
        drop(_guard);
        // Drop 后恢复——guard 不残留（用例隔离自证）
        assert_ne!(home_dir().unwrap(), dir.path(), "守卫 Drop 后应恢复原解析");
    }

    /// ① USERPROFILE env 生效：返回环境变量显式指定的目录
    #[test]
    fn home_dir_honors_env_override() {
        let _env = EnvRestoreGuard::acquire();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(USER_PROFILE_ENV, dir.path());
        assert_eq!(
            home_dir().unwrap(),
            dir.path(),
            "USERPROFILE 指定目录应生效（E2E 假 home 隔离键）"
        );
    }

    /// ② 空串视为未设置：忽略并回落 dirs 解析
    #[test]
    fn home_dir_ignores_empty_env() {
        let _env = EnvRestoreGuard::acquire();
        std::env::set_var(USER_PROFILE_ENV, "");
        assert!(home_dir().is_some(), "空串应忽略并回落 dirs::home_dir");
    }

    /// ③ env 移除后回落 dirs 解析（与 dirs::home_dir 一致）
    #[test]
    fn home_dir_falls_back_to_dirs_without_env() {
        let _env = EnvRestoreGuard::acquire();
        std::env::remove_var(USER_PROFILE_ENV);
        assert_eq!(home_dir(), dirs::home_dir(), "env 移除应回落 dirs");
    }

    /// ④ 测试 guard 优先于 env：guard 与 env 同设时返回 guard 值
    #[test]
    fn home_dir_guard_beats_env() {
        let _env = EnvRestoreGuard::acquire();
        let guard_dir = tempfile::tempdir().unwrap();
        let env_dir = tempfile::tempdir().unwrap();
        std::env::set_var(USER_PROFILE_ENV, env_dir.path());
        let _guard = HomeDirGuard::set(guard_dir.path());
        assert_eq!(
            home_dir().unwrap(),
            guard_dir.path(),
            "测试 guard 应优先于 USERPROFILE env"
        );
    }
}
