//! 安装器 handoff 的原生路径与进程身份。
//!
//! 事务持久化与 commit 授权仍在 `update_download::handoff`;本模块只做
//! 平台面:路径规范化、宿主进程创建身份、PowerShell helper 物化与启动、
//! 安装目录探测、旁置守卫锁路径。
//!
//! 原生创建时间身份用于防「PID 回收后冒名授权」:participant 的 pid 与
//! 创建时间必须同时匹配,helper 才认可旧进程已退出。
//!
//! **未消费件登记。** 本模块全部入口由 `update_download::handoff` 的生产
//! 链引用;handoff 的安装授权 UI 归更新弹窗步,当前只有 L1 测试执行,故
//! 整模块豁免 dead_code(与 handoff.rs 同一登记)。
#![allow(dead_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::process::Child;

/// 规范化路径并去掉 Windows 的 `\\?\` 扩展前缀(统一比较形态)。
pub(crate) fn canonical(path: &Path) -> io::Result<PathBuf> {
    let path = std::fs::canonicalize(path)?;
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        return Ok(PathBuf::from(format!(r"\\{unc}")));
    }
    if let Some(local) = text.strip_prefix(r"\\?\") {
        return Ok(PathBuf::from(local));
    }
    Ok(path)
}

/// 当前进程的原生创建时间(FILETIME 合并成 u64)。
///
/// 与 PID 组合成不可冒名的进程身份;PID 被系统回收后创建时间必然不同。
pub(crate) fn current_process_created() -> io::Result<u64> {
    use std::mem::zeroed;

    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut created: FILETIME = unsafe { zeroed() };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    // SAFETY: 伪句柄在当前进程内恒定有效;四个输出指针都指向本栈帧。
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// 以隐藏窗口启动 PowerShell helper(不接管 stdin/stdout/stderr)。
///
/// helper 是独立进程:应用退出后它继续等待 commit/失败回滚;日志只进
/// 事务目录,绝不回灌宿主控制台。
pub(crate) fn spawn_helper(helper: &Path, plan: &Path) -> io::Result<Child> {
    use std::os::windows::process::CommandExt as _;
    use std::process::{Command, Stdio};

    /// CREATE_NO_WINDOW:helper 无控制台,不闪窗。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows 目录不可用"))?
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(helper)
        .arg("-PlanPath")
        .arg(plan)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

/// 安装目录 = 可执行文件所在目录,且必须是安装器管理的安装(Inno 卸载器
/// `unins000.exe` 在场)。便携 zip 解压目录没有卸载器 → 明确拒绝自动安装,
/// 提示走下载页手动替换。
pub(crate) fn installation_directory(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("缺少应用目录")?;
    if !directory.join("unins000.exe").is_file() {
        return Err("该副本是便携版;请从下载页替换安装包".into());
    }
    Ok(directory.to_owned())
}

/// 旁置守卫锁基名:同一二进制的全部配置目录共享,安装器不替换/不删除。
pub(crate) fn guard_base(executable: &Path) -> PathBuf {
    executable
        .parent()
        .expect("规范化后的可执行文件必有父目录")
        .join(".slterm-update")
}

/// 物化 `handoff.ps1` 并启动 helper;事务授权仍归调用方。
pub(crate) fn spawn_prepared_helper(directory: &Path, plan: &Path) -> Result<Child, String> {
    let helper = directory.join("handoff.ps1");
    crate::atomic_file::write(&helper, include_bytes!("../update_download/handoff.ps1"))
        .map_err(|error| error.to_string())?;
    spawn_helper(&helper, plan).map_err(|error| error.to_string())
}

#[cfg(test)]
mod update_installation_tests {
    use super::{canonical, current_process_created, guard_base, installation_directory};

    #[test]
    fn installation_directory_requires_an_uninstaller() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("slterm.exe");
        std::fs::write(&executable, b"MZ").unwrap();
        assert!(
            installation_directory(&executable).is_err(),
            "便携目录不得自动安装"
        );
        std::fs::write(root.path().join("unins000.exe"), b"MZ").unwrap();
        assert_eq!(installation_directory(&executable).unwrap(), root.path());
    }

    #[test]
    fn guard_lock_lives_beside_the_installed_binary() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("slterm.exe");
        assert_eq!(guard_base(&executable), root.path().join(".slterm-update"));
    }

    #[test]
    fn canonical_returns_an_absolute_existing_path() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("state.json");
        std::fs::write(&file, b"{}").unwrap();
        let resolved = canonical(&file).unwrap();
        assert!(resolved.is_absolute());
        assert_eq!(resolved, canonical(&file).unwrap());
    }

    #[test]
    fn process_creation_identity_is_stable_and_nonzero() {
        let first = current_process_created().unwrap();
        let second = current_process_created().unwrap();
        assert_ne!(first, 0);
        assert_eq!(first, second);
    }
}
