//! Shell 探测链与 PowerShell 集成注入（旧栈 `pty/shell.rs` 语义资产并入）。
//!
//! 探测策略：`pwsh.exe` → `powershell.exe` → `cmd.exe` 存在性回退。
//! PowerShell 经 `-EncodedCommand`（UTF-16LE Base64）内联集成脚本，消除
//! `%APPDATA%` 文件写入——避免 AMSI/ASR 误杀。禁止 `-NoProfile`（B17）：
//! 用户 profile 必须先于集成脚本原生加载，否则 conda init 等 profile 钩子
//! 失效（conda activate 报错或静默空转）。
//!
//! 白名单深检（SEC-01/SEC-15）：仅放行 `pwsh.exe`/`powershell.exe`/`cmd.exe`；
//! 含路径分隔符的输入只信任 PATH 解析出的真实路径——canonicalize 双失败时
//! 以 Win32 句柄级文件身份（volume serial + file index）比对，任一侧证据
//! 缺失即拒绝，字符串永不构成放行依据。

use std::io::Error;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use base64::Engine as _;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_READ, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, OPEN_EXISTING,
};

use crate::tty::Shell;

/// 允许的 shell 白名单（仅文件名，不区分大小写）。
const ALLOWED_SHELLS: &[&str] = &["pwsh.exe", "powershell.exe", "cmd.exe"];

/// Shell 种类（白名单三值 + `Other` 兜底）。
///
/// 供壳按种类分派启动等待策略：pwsh/powershell 有 shell integration
/// （OSC 133;A 提示符信号），cmd/Other 无 → 固定延迟兜底。`Other` 经
/// 白名单拒绝，运行中不可达——无 integration 假设是闸门的安全侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Pwsh,
    PowerShell,
    Cmd,
    Other,
}

/// 从 shell 程序路径推导种类（纯函数，文件名不区分大小写）。
pub fn shell_kind_of(program: &str) -> ShellKind {
    let filename = Path::new(program)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();
    match filename.as_str() {
        "pwsh.exe" => ShellKind::Pwsh,
        "powershell.exe" => ShellKind::PowerShell,
        "cmd.exe" => ShellKind::Cmd,
        _ => ShellKind::Other,
    }
}

/// 解析 shell 程序：`Some` 走白名单校验后原样采用；`None` 按探测链回退。
///
/// 返回的 `Shell.program` 恒为完整路径（短名经 `which_full_path` 解析），
/// 保证 `CreateProcessW` 命令行定位可执行文件不依赖调用方 cwd。
pub fn resolve_shell(user_shell: Option<&str>) -> Result<Shell, Error> {
    let shell = if let Some(shell) = user_shell {
        let program = if shell.contains('\\') || shell.contains('/') {
            shell.to_owned()
        } else {
            which_full_path(shell).unwrap_or_else(|| shell.to_owned())
        };
        Shell::new(program, Vec::new())
    } else if let Some(path) = which_full_path("pwsh.exe") {
        build_pwsh_command(&path)
    } else if let Some(path) = which_full_path("powershell.exe") {
        build_pwsh_command(&path)
    } else {
        // cmd.exe 恒在 System32 下；PATH 不含 System32 的极端情况经系统目录
        // 兜底解析（与 validate_shell_allowlist 的系统目录放行自洽），环境变量
        // 全缺时才硬编码 C:\Windows 兜底。
        let cmd_path = which_full_path("cmd.exe")
            .or_else(|| system32_exe_path("cmd.exe"))
            .unwrap_or_else(|| r"C:\Windows\System32\cmd.exe".to_owned());
        Shell::new(cmd_path, Vec::new())
    };

    validate_shell_allowlist(shell.program())?;
    Ok(shell)
}

/// Shell 白名单校验——仅允许 `pwsh.exe` / `powershell.exe` / `cmd.exe`。
///
/// - 纯文件名输入：命中白名单即放行。
/// - SEC-01：含路径分隔符的输入——canonicalize 用户路径，与
///   `which_full_path(文件名)` 解析结果比对，一致才放行，杜绝
///   `C:\project\cmd.exe` 式绕过。文件名不在白名单时：经 PATH 解析出完整
///   路径后再比较文件名。
/// - 系统目录兜底：PATH 解析失败时 `%SystemRoot%\System32\<文件名>`（系统
///   目录，二进制可信）经身份比对一致同样放行——resolve_shell 的 cmd 回退
///   即使用该路径，校验须与之自洽，否则合法回退被确定性拒绝。
pub(crate) fn validate_shell_allowlist(program: &str) -> Result<(), Error> {
    let filename = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);

    let filename_lower = filename.to_lowercase();
    let in_allowlist = ALLOWED_SHELLS.iter().any(|a| filename_lower == *a);

    if !in_allowlist {
        // 文件名不在白名单：经 PATH 解析完整路径后再比较文件名。
        if let Some(resolved) = which_full_path(filename) {
            let resolved_name = Path::new(&resolved)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&resolved);
            if ALLOWED_SHELLS
                .iter()
                .any(|a| resolved_name.to_lowercase() == *a)
            {
                return Ok(());
            }
        }
        return Err(Error::other(format!(
            "不允许的 shell 程序: {program}。仅支持 pwsh.exe, powershell.exe, cmd.exe"
        )));
    }

    if !(program.contains('\\') || program.contains('/')) {
        return Ok(());
    }

    // SEC-01：含路径分隔符——只信任 PATH 解析出的真实路径；PATH 解析失败
    // 回退系统目录比对。
    let resolved = match which_full_path(filename) {
        Some(path) => path,
        None => match system32_exe_path(filename) {
            Some(path) => path,
            None => {
                return Err(Error::other(format!(
                    "不允许的 shell 程序: {program}。仅支持 PATH 解析出的 pwsh.exe, powershell.exe, cmd.exe"
                )));
            }
        },
    };
    if !paths_match(program, &resolved) {
        return Err(Error::other(format!(
            "不允许的 shell 程序: {program}。仅支持 PATH 解析出的 pwsh.exe, powershell.exe, cmd.exe"
        )));
    }

    Ok(())
}

/// Win32 句柄级文件身份（SEC-15：替代字符串回退比对）。
///
/// 身份 = (volume serial number, file index)——同一卷上唯一标识一个文件。
/// 普通打开（不带 `FILE_FLAG_OPEN_REPARSE_POINT`）跟随 symlink 等 reparse
/// point 到真实目标，取得目标文件身份。注意：应用执行别名（APPEXECLINK）
/// 无法被普通 `CreateFileW` 打开——实测 os error 1920
/// （`ERROR_CANT_ACCESS_FILE`）——本函数对 alias 恒 `None`；alias 的身份
/// 证据经 `reparse_entry_identity` 补充路径获取。任一侧打开/查询失败 →
/// `None`：句柄级证据缺失即整体拒绝，绝不降级字符串比对。
fn file_identity(path: &str) -> Option<(u32, u64)> {
    query_identity(path, FILE_GENERIC_READ, FILE_ATTRIBUTE_NORMAL)
}

/// Win32 reparse 点文件条目身份（AEL alias 场景的补充证据路径）。
///
/// alias 文件条目本身可经 `FILE_FLAG_OPEN_REPARSE_POINT` +
/// `FILE_READ_ATTRIBUTES` 打开——条目身份 = (volume serial, file index)，
/// 两侧指向同一 alias 条目时必然相等，构成不依赖字符串的句柄级证据
/// （真实路径两侧相同 ⇔ 条目身份相同，大小写/格式差异被身份口径吸收）。
fn reparse_entry_identity(path: &str) -> Option<(u32, u64)> {
    query_identity(
        path,
        FILE_READ_ATTRIBUTES,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
    )
}

/// 以给定访问权/标志打开文件并查询句柄级身份（`file_identity` 与
/// `reparse_entry_identity` 的共用底件）。
fn query_identity(
    path: &str,
    access: u32,
    flags: windows_sys::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
) -> Option<(u32, u64)> {
    let wide: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let queried = GetFileInformationByHandle(handle, &mut info);
        let _ = CloseHandle(handle);
        if queried == 0 {
            return None;
        }
    }
    let index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
    Some((info.dwVolumeSerialNumber, index))
}

/// 比较 program 与 PATH 解析结果是否指向同一可执行文件（SEC-01 判定核心）。
///
/// 1) canonicalize 双成功 → 精确比较（拉平 8.3 短名/`..`/symlink 差异）；
/// 2) 双侧均失败（应用执行别名/特殊 ACL——CreateProcess 可运行但普通文件
///    API 打开失败，os error 1920 场景）→ Win32 句柄级文件身份比对
///    （volume serial + file index；真实文件取普通句柄身份，alias 类
///    reparse 条目取条目身份——两侧证据齐且相等才放行，任一侧证据缺失
///    即拒绝，不降级字符串）；
/// 3) 单侧失败即拒绝（SEC-15 收窄保留为纵深一层）。
fn paths_match(program: &str, resolved: &str) -> bool {
    match (
        std::fs::canonicalize(program),
        std::fs::canonicalize(resolved),
    ) {
        (Ok(cp), Ok(cr)) => cp
            .to_string_lossy()
            .eq_ignore_ascii_case(&cr.to_string_lossy()),
        (Err(_), Err(_)) => fallback_identity_match(program, resolved),
        _ => false,
    }
}

/// 双侧 canonicalize 失败的回退比对：每侧证据 = 普通句柄身份优先（真实
/// 文件/硬链接），普通打开失败（AEL alias 类条目）补 reparse 条目身份；
/// 两侧证据齐且相等才放行——合法 alias 用例两侧同一条目身份必然相等；
/// 同名不同条目身份不等、不存在路径两侧证据皆缺，一律拒绝。
fn fallback_identity_match(program: &str, resolved: &str) -> bool {
    let identity = |p: &str| file_identity(p).or_else(|| reparse_entry_identity(p));
    match (identity(program), identity(resolved)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// 为 PowerShell 构建带集成脚本的启动参数。
///
/// `-NoLogo -NoExit -EncodedCommand <base64(UTF-16LE script)>`；脚本经
/// `include_str!` 嵌入，不写磁盘。禁止 `-NoProfile`（B17）：用户 profile
/// 必须先于集成脚本原生加载。
fn build_pwsh_command(pwsh_path: &str) -> Shell {
    Shell::new(
        pwsh_path.to_owned(),
        vec![
            "-NoLogo".to_owned(),
            "-NoExit".to_owned(),
            "-EncodedCommand".to_owned(),
            encode_utf16le_base64(shell_integration_script()),
        ],
    )
}

/// 将字符串编码为 UTF-16LE 后 Base64（PowerShell `-EncodedCommand` 要求
/// UTF-16LE 无 BOM + 标准 Base64）。
fn encode_utf16le_base64(script: &str) -> String {
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// 在 PATH 中查找可执行文件，返回第一个匹配目录的完整路径。
fn which_full_path(name: &str) -> Option<String> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let full = dir.join(name);
            if full.exists() {
                return Some(full.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// 系统目录（%SystemRoot%\System32）下可执行文件的路径。
///
/// resolve_shell 的 cmd 回退与 validate_shell_allowlist 的系统目录兜底共用，
/// 保证两侧路径一致（Windows 可安装在非 C: 盘，优先环境变量解析，缺失时
/// 才硬编码 C:\Windows 兜底）。文件不存在返回 None。
fn system32_exe_path(name: &str) -> Option<String> {
    let root = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("WINDIR"))
        .unwrap_or_else(|_| r"C:\Windows".to_owned());
    let path = Path::new(&root).join("System32").join(name);
    if path.exists() {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Shell 集成脚本（`include_str!` 嵌入，分发的 exe 不依赖外部脚本路径）。
fn shell_integration_script() -> &'static str {
    include_str!("shell-integration.ps1")
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    // ─── PATH 可控测试辅助 ───
    //
    // 用例组通过整体替换 PATH 环境变量驱动回退顺序与解析行为。环境变量全局
    // 可变，依赖 L1 全量 `--test-threads=1` 门禁（串行执行无污染）。

    /// PATH 恢复守卫——测试结束时（含 panic）还原原 PATH，避免污染后续用例。
    struct PathGuard(Option<std::ffi::OsString>);

    impl Drop for PathGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => unsafe { std::env::set_var("PATH", v) },
                None => unsafe { std::env::remove_var("PATH") },
            }
        }
    }

    /// 用给定目录列表整体替换 PATH，返回恢复守卫。
    fn set_test_path(paths: &[&std::path::Path]) -> PathGuard {
        let old = std::env::var_os("PATH");
        let joined = std::env::join_paths(paths.iter()).expect("构造 PATH 失败");
        unsafe { std::env::set_var("PATH", joined) };
        PathGuard(old)
    }

    /// 在目录中创建假可执行文件——空文件即可（which_full_path 只查 exists）。
    fn fake_exe(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"").expect("写假 exe 失败");
        path
    }

    // ─── encode_utf16le_base64（B17 载体编码）───

    #[test]
    fn encode_utf16le_base64_roundtrip() {
        let original = "Write-Host 'hello'";
        let encoded = encode_utf16le_base64(original);
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .expect("base64 解码失败");
        let u16s: Vec<u16> = decoded_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let decoded = String::from_utf16(&u16s).expect("UTF-16 解码失败");
        assert_eq!(decoded, original);
    }

    // ─── validate_shell_allowlist 正反族（SEC-01）───

    #[test]
    fn allowlist_three_shells_and_case_insensitive_pass() {
        validate_shell_allowlist("cmd.exe").expect("cmd.exe 应在白名单内");
        validate_shell_allowlist("pwsh.exe").expect("pwsh.exe 应在白名单内");
        validate_shell_allowlist("powershell.exe").expect("powershell.exe 应在白名单内");
        validate_shell_allowlist("CMD.EXE").expect("大写 CMD.EXE 应在白名单内");
        validate_shell_allowlist("PwSh.ExE").expect("大小写混合应通过");
    }

    #[test]
    fn allowlist_full_path_passes() {
        // SEC-01：完整路径仅当与 PATH 解析结果一致时才放行。
        if let Some(resolved) = which_full_path("cmd.exe") {
            validate_shell_allowlist(&resolved).expect("PATH 解析出的 cmd.exe 完整路径应通过");
        }
    }

    #[test]
    fn allowlist_rejects_unknown_shell() {
        let result = validate_shell_allowlist("evil.exe");
        assert!(result.is_err(), "evil.exe 不在白名单中，应拒绝");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("evil.exe"), "错误消息应包含被拒绝的程序名");
    }

    #[test]
    fn allowlist_rejects_path_not_in_allowlist() {
        let result = validate_shell_allowlist(r"C:\evil\fake-shell.exe");
        assert!(result.is_err(), "fake-shell.exe 不在白名单中，应拒绝");
    }

    #[test]
    fn allowlist_accepts_path_resolved_from_path() {
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        let _guard = set_test_path(&[dir.path()]);
        let resolved = which_full_path("cmd.exe").expect("PATH 应解析出 cmd.exe");
        validate_shell_allowlist(&resolved).expect("PATH 解析出的合法绝对路径应放行");
    }

    #[test]
    fn allowlist_accepts_system32_cmd_when_path_lacks_system32() {
        // SEC-01 回归守卫：PATH 不含 System32 时，resolve_shell 的 cmd 回退
        // 使用 %SystemRoot%\System32\cmd.exe——校验必须放行，否则合法回退
        // 被确定性拒绝。
        let dir = tempfile::tempdir().unwrap();
        let _guard = set_test_path(&[dir.path()]);
        let system_cmd =
            system32_exe_path("cmd.exe").expect("测试机 %SystemRoot%\\System32\\cmd.exe 应存在");
        validate_shell_allowlist(&system_cmd).expect("系统目录 cmd.exe 应放行");
    }

    #[test]
    fn allowlist_rejects_forged_absolute_path() {
        // 伪造绝对路径拒绝：目录中自建 cmd.exe（白名单文件名）但 PATH 解析不出。
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        let empty_dir = tempfile::tempdir().unwrap();
        let _guard = set_test_path(&[empty_dir.path()]);
        let forged = dir.path().join("cmd.exe").to_string_lossy().into_owned();
        let result = validate_shell_allowlist(&forged);
        assert!(result.is_err(), "非 PATH 解析出的绝对路径应拒绝: {forged}");
    }

    #[test]
    fn allowlist_rejects_absolute_path_not_in_path() {
        // 白名单文件名 + 用户目录不在 PATH：即使 PATH 中存在同名文件（解析
        // 结果指向另一目录），用户路径与解析结果不一致 → 拒绝；一致 → 放行。
        let user_dir = tempfile::tempdir().unwrap();
        let path_dir = tempfile::tempdir().unwrap();
        fake_exe(user_dir.path(), "cmd.exe");
        fake_exe(path_dir.path(), "cmd.exe");
        let _guard = set_test_path(&[path_dir.path()]);

        let forged = user_dir
            .path()
            .join("cmd.exe")
            .to_string_lossy()
            .into_owned();
        assert!(
            validate_shell_allowlist(&forged).is_err(),
            "与 PATH 解析结果不一致的绝对路径应拒绝: {forged}"
        );

        let legit = path_dir
            .path()
            .join("cmd.exe")
            .to_string_lossy()
            .into_owned();
        validate_shell_allowlist(&legit).expect("与 PATH 解析结果一致的绝对路径应放行");
    }

    #[test]
    fn allowlist_nonexistent_absolute_path_rejects_with_unified_message() {
        let result = validate_shell_allowlist(r"C:\Windows\System32\__nope__\cmd.exe");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("不允许的 shell 程序"),
            "错误消息应为统一文案，实际: {msg}"
        );
    }

    #[test]
    fn allowlist_rejects_path_resolved_non_allowlisted() {
        // 用户指定 shell 经 PATH 解析成功（文件真实存在）但非白名单 → 仍拒绝。
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "fake-shell.exe");
        let _guard = set_test_path(&[dir.path()]);

        let result = validate_shell_allowlist("fake-shell.exe");
        assert!(result.is_err(), "PATH 解析成功但非白名单应拒绝");
        assert!(
            resolve_shell(Some("fake-shell.exe")).is_err(),
            "resolve_shell 用户指定非白名单应拒绝"
        );
    }

    // ─── paths_match / file_identity 句柄级身份族（SEC-15）───

    #[test]
    fn paths_match_canonical_equal() {
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        let p = dir.path().join("cmd.exe").to_string_lossy().into_owned();
        let upper = dir.path().join("CMD.EXE").to_string_lossy().into_owned();
        assert!(
            paths_match(&p, &upper),
            "canonicalize 成功时大小写变体应相等"
        );
        assert!(paths_match(&p, &p));
    }

    #[test]
    fn paths_match_canonical_unequal() {
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        fake_exe(d1.path(), "cmd.exe");
        fake_exe(d2.path(), "cmd.exe");
        let p1 = d1.path().join("cmd.exe").to_string_lossy().into_owned();
        let p2 = d2.path().join("cmd.exe").to_string_lossy().into_owned();
        assert!(!paths_match(&p1, &p2));
    }

    #[test]
    fn fallback_both_unopenable_rejected() {
        // 双侧均打不开（无句柄级证据）即拒绝——同名同串也不例外（SEC-15
        // 字符串回退已销；防回归锚点：绕过需两侧同 volume serial + file index）。
        let missing = r"C:\no-such-dir-x\cmd.exe";
        assert!(
            !std::path::Path::new(missing).exists(),
            "前提：不存在路径不应存在"
        );
        assert!(
            !paths_match(missing, missing),
            "双侧均打不开应拒绝（SEC-15）"
        );
    }

    #[test]
    fn file_identity_same_file_via_hardlink_equal() {
        // 硬链接 = 同一文件的两个目录项 → volume serial + file index 相同。
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("a.exe");
        let link = dir.path().join("b.exe");
        std::fs::write(&original, b"x").expect("写原文件失败");
        std::fs::hard_link(&original, &link).expect("tempdir 内建硬链接应成功（NTFS）");
        let a = original.to_string_lossy().into_owned();
        let b = link.to_string_lossy().into_owned();
        assert_eq!(
            file_identity(&a),
            file_identity(&b),
            "硬链接两侧应具同一文件身份"
        );
        assert!(
            fallback_identity_match(&a, &b),
            "同一文件的两个目录项应放行（合法 alias 两侧等价场景）"
        );
    }

    #[test]
    fn file_identity_distinct_files_unequal() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a.exe");
        let p2 = dir.path().join("b.exe");
        std::fs::write(&p1, b"x").unwrap();
        std::fs::write(&p2, b"y").unwrap();
        let a = p1.to_string_lossy().into_owned();
        let b = p2.to_string_lossy().into_owned();
        assert_ne!(file_identity(&a), file_identity(&b), "不同文件身份应不等");
        assert!(
            !fallback_identity_match(&a, &b),
            "身份不等不应放行（SEC-15 防回归）"
        );
    }

    #[test]
    fn file_identity_missing_file_none() {
        let missing = r"C:\__slterm_no_such_dir__\cmd.exe";
        assert!(
            !std::path::Path::new(missing).exists(),
            "前提：不存在路径不应存在"
        );
        assert_eq!(file_identity(missing), None, "打不开的路径应返回 None");
    }

    #[test]
    fn paths_match_single_side_failure_rejected() {
        // SEC-15：单侧 canonicalize 失败即拒绝（字符串比对无法证明文件身份）。
        let real = which_full_path("cmd.exe")
            .or_else(|| system32_exe_path("cmd.exe"))
            .expect("测试机应存在 cmd.exe");
        let nonexistent = r"C:\__slterm_no_such_dir__\cmd.exe";
        assert!(
            !std::path::Path::new(nonexistent).exists(),
            "前提：不存在路径不应存在"
        );
        assert!(!paths_match(&real, nonexistent), "单侧失败应拒绝");
        assert!(!paths_match(nonexistent, &real), "单侧失败应拒绝");
    }

    // ─── 真实应用执行别名测试（条件空跑）───
    //
    // 应用执行别名的「exists 为真、canonicalize 失败（os error 1920）」属性
    // 无法用 tempdir 模拟。本机装有 Store 版 PowerShell 7 时
    // %LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe 即真实 alias——PATH 收敛
    // 后 canonicalize 失败 → 句柄级身份比对放行（两侧同一 alias 条目）。
    // 无 alias 的机器条件不满足，用例空跑不失败。

    #[test]
    fn allowlist_accepts_real_alias_when_present() {
        use std::path::PathBuf;

        let alias_dir = std::env::var("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Microsoft").join("WindowsApps"))
            .ok();
        let alias = alias_dir.map(|d| d.join("pwsh.exe"));
        if let Some(a) = alias {
            if !a.exists() {
                return; // 本机无 Store 版 pwsh → 条件不满足空跑
            }
            let _guard = set_test_path(&[a.parent().expect("alias 目录")]);
            assert_eq!(
                which_full_path("pwsh.exe").as_deref(),
                Some(a.to_str().expect("alias 路径为 UTF-8")),
                "PATH 收敛后应命中 alias"
            );
            let s = a.to_string_lossy().into_owned();
            validate_shell_allowlist(&s)
                .expect("真实 alias 路径应放行（canonicalize 失败 → 句柄级身份放行）");
        }
    }

    // ─── shell_kind_of 纯函数 ───

    #[test]
    fn shell_kind_of_three_shells() {
        assert_eq!(shell_kind_of("pwsh.exe"), ShellKind::Pwsh);
        assert_eq!(shell_kind_of("powershell.exe"), ShellKind::PowerShell);
        assert_eq!(shell_kind_of("cmd.exe"), ShellKind::Cmd);
    }

    #[test]
    fn shell_kind_of_full_path_and_case_insensitive() {
        assert_eq!(
            shell_kind_of(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            ShellKind::Pwsh
        );
        assert_eq!(
            shell_kind_of(r"C:\Windows\System32\WindowsPowerShell\v1.0\POWERSHELL.EXE"),
            ShellKind::PowerShell
        );
        assert_eq!(
            shell_kind_of(r"C:\Windows\System32\cmd.exe"),
            ShellKind::Cmd
        );
    }

    #[test]
    fn shell_kind_of_unknown_is_other() {
        // 白名单外归 Other（经白名单拒绝，运行中不可达——无 OSC 133 假设 =
        // 闸门安全侧，固定延迟注入而非傻等）。
        assert_eq!(shell_kind_of("bash.exe"), ShellKind::Other);
        assert_eq!(shell_kind_of(""), ShellKind::Other);
    }

    // ─── resolve_shell 探测链回退顺序（PATH 注入驱动）───

    #[test]
    fn resolve_shell_fallback_order() {
        let dir = tempfile::tempdir().unwrap();

        // 场景 1：只有 pwsh → 命中 pwsh（完整路径 + 集成脚本参数）
        fake_exe(dir.path(), "pwsh.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let shell = resolve_shell(None).expect("应命中 pwsh");
            let expect = dir.path().join("pwsh.exe");
            assert_eq!(shell.program(), expect.to_string_lossy().as_ref());
            assert!(
                shell.args().contains(&"-EncodedCommand".to_owned()),
                "pwsh 应携带集成脚本参数"
            );
        }

        // 场景 2：只有 powershell → 命中 powershell
        std::fs::remove_file(dir.path().join("pwsh.exe")).unwrap();
        fake_exe(dir.path(), "powershell.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let shell = resolve_shell(None).expect("应命中 powershell");
            let expect = dir.path().join("powershell.exe");
            assert_eq!(shell.program(), expect.to_string_lossy().as_ref());
        }

        // 场景 3：都没有 → 回退 cmd（系统目录兜底路径，不带参数）
        std::fs::remove_file(dir.path().join("powershell.exe")).unwrap();
        {
            let _guard = set_test_path(&[dir.path()]);
            let shell = resolve_shell(None).expect("应回退 cmd");
            assert!(
                shell.program().ends_with("cmd.exe"),
                "应回退 cmd.exe，实际: {}",
                shell.program()
            );
            assert!(shell.args().is_empty(), "cmd 回退不带参数");
        }

        // 场景 4：pwsh 与 powershell 并存 → pwsh 优先（回退顺序首档）
        fake_exe(dir.path(), "pwsh.exe");
        fake_exe(dir.path(), "powershell.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let shell = resolve_shell(None).expect("应命中 pwsh");
            let expect = dir.path().join("pwsh.exe");
            assert_eq!(shell.program(), expect.to_string_lossy().as_ref());
        }
    }

    #[test]
    fn pwsh_args_no_noprofile_b17() {
        // B17 防复发：-NoProfile 致 conda activate 失效（win11 CondaError /
        // win10 conda.bat 静默空转）——自动检测命中 pwsh 时 args 不得含
        // -NoProfile。
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "pwsh.exe");
        let _guard = set_test_path(&[dir.path()]);

        let shell = resolve_shell(None).expect("应命中 pwsh");
        assert!(
            !shell.args().iter().any(|a| a == "-NoProfile"),
            "pwsh args 不得含 -NoProfile（B17），实际: {:?}",
            shell.args()
        );
        assert!(
            shell.args().contains(&"-EncodedCommand".to_owned()),
            "pwsh 仍应携带集成脚本参数"
        );
    }

    #[test]
    fn resolve_shell_returns_full_path() {
        let shell = if which_full_path("pwsh.exe").is_some()
            || which_full_path("powershell.exe").is_some()
        {
            resolve_shell(None)
        } else {
            resolve_shell(Some("cmd.exe"))
        }
        .expect("resolve_shell 应成功");
        assert!(
            shell.program().contains('\\') || shell.program().contains('/'),
            "program 应为完整路径，实际: {}",
            shell.program()
        );
    }

    #[test]
    fn resolve_shell_user_short_name_resolves_via_path() {
        // 用户指定短名 → PATH 解析为完整路径。
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        let _guard = set_test_path(&[dir.path()]);
        let shell = resolve_shell(Some("cmd.exe")).expect("应成功");
        let expect = dir.path().join("cmd.exe");
        assert_eq!(shell.program(), expect.to_string_lossy().as_ref());
    }

    // ─── which_full_path 族 ───

    #[test]
    fn which_full_path_finds_pwsh_or_powershell() {
        let found = which_full_path("pwsh.exe").or_else(|| which_full_path("powershell.exe"));
        assert!(found.is_some(), "至少 pwsh 或 powershell 应存在");
        let path = found.unwrap();
        assert!(
            path.contains('\\') || path.contains('/'),
            "应返回完整路径，实际: {path}"
        );
        assert!(path.ends_with(".exe"), "路径应以 .exe 结尾，实际: {path}");
    }

    #[test]
    fn which_full_path_nonexistent() {
        assert!(which_full_path("__nonexistent_xyz__.exe").is_none());
    }

    #[test]
    fn which_full_path_first_match_in_path_order() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        fake_exe(dir1.path(), "pwsh.exe");
        fake_exe(dir2.path(), "pwsh.exe");
        let _guard = set_test_path(&[dir1.path(), dir2.path()]);

        let found = which_full_path("pwsh.exe").expect("应命中第一个目录");
        assert_eq!(
            found,
            dir1.path().join("pwsh.exe").to_string_lossy().as_ref()
        );
    }

    #[test]
    fn which_full_path_case_insensitive_on_windows_fs() {
        // Windows 文件系统大小写不敏感 → 目录中 PwSh.ExE 可命中 pwsh.exe。
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "PwSh.ExE");
        let _guard = set_test_path(&[dir.path()]);
        assert!(which_full_path("pwsh.exe").is_some(), "应命中 PwSh.ExE");
    }

    // ─── 集成脚本嵌入 ───

    #[test]
    fn shell_integration_script_embedded() {
        let script = shell_integration_script();
        assert!(!script.is_empty(), "集成脚本不应为空");
        assert!(script.contains("OSC"), "脚本应定义 OSC 序列");
    }
}
