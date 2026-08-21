/// Shell 选择与 PowerShell profile 注入
///
/// shell 选择策略：pwsh.exe → powershell.exe → cmd.exe 回退。
/// PowerShell 通过 -EncodedCommand（UTF-16LE Base64）内联集成脚本，
/// 消除 %APPDATA% 文件写入——避免 AMSI/ASR 误杀。
use base64::Engine;
use portable_pty::CommandBuilder;

use crate::error::AppError;

/// 允许的 shell 白名单（仅文件名，不区分大小写）
const ALLOWED_SHELLS: &[&str] = &["pwsh.exe", "powershell.exe", "cmd.exe"];

/// 解析后的 Shell 信息（不依赖 portable-pty 类型）
///
/// 用于直接构建 CreateProcessW 参数，绕过 CommandBuilder 的 pub(crate) 限制。
#[derive(Debug, Clone)]
pub struct ShellInfo {
    /// Shell 可执行文件路径（如 "pwsh.exe"）
    pub program: String,
    /// 命令行参数列表（不含 program 本身）
    pub args: Vec<String>,
}

/// Shell 白名单校验——仅允许 pwsh.exe / powershell.exe / cmd.exe
///
/// 提取 program 的文件名（不区分大小写）与白名单比对。
/// - 纯文件名输入：命中白名单即放行（现状语义）。
/// - SEC-01: 含路径分隔符的输入——canonicalize 用户路径，与
///   `which_full_path(文件名)` 解析结果比对，一致才放行——
///   只信任 PATH 解析出的真实路径，杜绝 `C:\project\cmd.exe` 式绕过。
///   文件名不在白名单时：尝试通过 which_full_path 在 PATH 中解析完整路径后再比较。
/// - 系统目录兜底：PATH 解析失败时，`%SystemRoot%\System32\<文件名>`（系统目录，
///   二进制可信）经 canonicalize 比对一致同样放行——resolve_shell_info 的 cmd 回退
///   在 PATH 不含 System32 时即使用该路径，校验须与之自洽，否则合法回退被确定性拒绝。
pub(crate) fn validate_shell_allowlist(program: &str) -> Result<(), AppError> {
    // 提取文件名（不区分大小写）
    let filename = std::path::Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);

    let filename_lower = filename.to_lowercase();
    let in_allowlist = ALLOWED_SHELLS.iter().any(|a| filename_lower == *a);

    if !in_allowlist {
        // 文件名不在白名单：尝试通过 PATH 解析完整路径后再比较（现状语义）
        if let Some(resolved) = which_full_path(filename) {
            let resolved_name = std::path::Path::new(&resolved)
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
        return Err(AppError::Pty(format!(
            "不允许的 shell 程序: {program}。仅支持 pwsh.exe, powershell.exe, cmd.exe"
        )));
    }

    // 文件名在白名单内
    if !(program.contains('\\') || program.contains('/')) {
        // 纯文件名输入——维持现状，直接放行
        return Ok(());
    }

    // SEC-01: 含路径分隔符——只信任 PATH 解析出的真实路径。
    // canonicalize 用户路径后与 which_full_path(文件名) 解析结果比对，一致才放行；
    // canonicalize 失败（应用执行别名/特殊 ACL——CreateProcess 可运行但普通文件
    // API 打开失败，os error 1920 场景）回退归一字符串比对，不因此拒绝合法 shell。
    let resolved = match which_full_path(filename) {
        Some(path) => path,
        // 系统目录兜底：PATH 解析失败时，%SystemRoot%\System32\<文件名> 放行——
        // resolve_shell_info 的 cmd 回退（PATH 不含 System32）即用该路径，
        // 系统目录二进制可信，比对一致即放行。
        None => match system32_exe_path(filename) {
            Some(path) => path,
            None => {
                return Err(AppError::Pty(format!(
                    "不允许的 shell 程序: {program}。仅支持 PATH 解析出的 pwsh.exe, powershell.exe, cmd.exe"
                )));
            }
        },
    };
    if !paths_match(program, &resolved) {
        return Err(AppError::Pty(format!(
            "不允许的 shell 程序: {program}。仅支持 PATH 解析出的 pwsh.exe, powershell.exe, cmd.exe"
        )));
    }

    Ok(())
}

/// 比较 program 与 PATH 解析结果是否指向同一可执行文件（SEC-01 判定核心）
///
/// 优先 canonicalize 精确比较（拉平 8.3 短名/`..`/symlink 差异）；
/// 任一侧 canonicalize 失败（应用执行别名/特殊 ACL——CreateProcess 可运行但
/// 普通文件 API 打开失败，os error 1920 场景）回退归一字符串比较：
/// Windows 大小写不敏感文件系统下，归一化后字符串相等的两路径必然指向
/// 同一文件，安全语义不弱化（伪造路径与 PATH 解析结果字符串必不同，仍拒绝）。
fn paths_match(program: &str, resolved: &str) -> bool {
    // 1) canonicalize 双成功 → 精确比较（8.3 短名/`..`/symlink 差异由系统拉平）
    if let (Ok(cp), Ok(cr)) = (
        std::fs::canonicalize(program),
        std::fs::canonicalize(resolved),
    ) {
        return if cfg!(windows) {
            cp.to_string_lossy()
                .eq_ignore_ascii_case(&cr.to_string_lossy())
        } else {
            cp == cr
        };
    }
    // 2) fallback：分隔符归一 + 去尾分隔符后比对（Windows 忽略大小写）
    let a = normalize_for_compare(program);
    let b = normalize_for_compare(resolved);
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

/// 路径归一化（canonicalize 失败时的字符串比对用）：`/`→`\`、去尾分隔符
fn normalize_for_compare(p: &str) -> String {
    if cfg!(windows) {
        p.replace('/', "\\")
            .trim_end_matches(['\\', '/'])
            .to_string()
    } else {
        p.trim_end_matches('/').to_string()
    }
}

/// 解析 shell 程序，返回已配置好参数的基础 CommandBuilder
///
/// 用户指定则直接使用；否则按 pwsh → powershell → cmd 顺序检测。
/// PowerShell 自动加入 -NoProfile -NoLogo -EncodedCommand <base64> 参数。
#[allow(dead_code)] // 保留供非 Windows 平台 fallback
pub fn resolve_shell(user_shell: Option<&str>) -> Result<CommandBuilder, AppError> {
    if let Some(shell) = user_shell {
        validate_shell_allowlist(shell)?;
        return Ok(CommandBuilder::new(shell));
    }

    // 检测 pwsh.exe（白名单内，无需额外校验）
    if which_exists("pwsh.exe") {
        return Ok(build_pwsh_command("pwsh.exe"));
    }

    // 回退 powershell.exe（白名单内，无需额外校验）
    if which_exists("powershell.exe") {
        return Ok(build_pwsh_command("powershell.exe"));
    }

    // 最终回退 cmd.exe（白名单内，无需额外校验）
    Ok(CommandBuilder::new("cmd.exe"))
}

/// 解析 shell 程序，返回不依赖 portable-pty 的 ShellInfo
///
/// 与 resolve_shell 相同的检测逻辑，但返回纯数据结构，
/// 供自定义 ConPTY 创建流程（绕过 CommandBuilder）使用。
/// program 字段为完整路径（通过 which_full_path 解析），
/// 确保 CreateProcessW(lpApplicationName=...) 可正确定位可执行文件。
pub fn resolve_shell_info(user_shell: Option<&str>) -> Result<ShellInfo, AppError> {
    // 用户指定 shell：直接用指定路径（可能已是完整路径）
    let info = if let Some(shell) = user_shell {
        // 尝试解析完整路径（如果 shell 是短名的话）
        let program = if shell.contains('\\') || shell.contains('/') {
            // 已有路径分隔符，原样使用
            shell.to_string()
        } else {
            // 短名 → 在 PATH 中解析
            which_full_path(shell).unwrap_or_else(|| shell.to_string())
        };
        ShellInfo {
            program,
            args: vec![],
        }
    } else if let Some(path) = which_full_path("pwsh.exe") {
        build_pwsh_info(&path)
    } else if let Some(path) = which_full_path("powershell.exe") {
        build_pwsh_info(&path)
    } else {
        // cmd.exe 始终在 System32 下，which_full_path 可能找不到（PATH 不含 System32 的极端情况），
        // 用 %SystemRoot%\System32\cmd.exe 完整路径回退（环境变量缺失时才硬编码 C:\Windows 兜底）。
        // 与 validate_shell_allowlist 的系统目录兜底自洽（同一 helper 解析，避免校验拒绝）。
        let cmd_path = which_full_path("cmd.exe")
            .or_else(|| system32_exe_path("cmd.exe"))
            .unwrap_or_else(|| r"C:\Windows\System32\cmd.exe".to_string());
        ShellInfo {
            program: cmd_path,
            args: vec![],
        }
    };

    // 白名单校验：仅允许 pwsh.exe / powershell.exe / cmd.exe
    validate_shell_allowlist(&info.program)?;
    Ok(info)
}

/// 为 PowerShell 构建带 profile 注入的 CommandBuilder
///
/// 使用 -NoProfile -NoLogo -EncodedCommand <base64(UTF-16LE script)> 启动。
/// 脚本通过 include_str! 嵌入，不写磁盘。
fn build_pwsh_command(pwsh: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(pwsh);
    cmd.arg("-NoProfile");
    cmd.arg("-NoLogo");
    cmd.arg("-NoExit");
    cmd.arg("-EncodedCommand");
    cmd.arg(encode_utf16le_base64(get_shell_integration_script()));
    cmd
}

/// 为 PowerShell 构建 ShellInfo（不依赖 portable-pty）
/// pwsh 参数为完整路径（由 which_full_path 解析）
fn build_pwsh_info(pwsh_path: &str) -> ShellInfo {
    ShellInfo {
        program: pwsh_path.to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-NoLogo".to_string(),
            "-NoExit".to_string(),
            "-EncodedCommand".to_string(),
            encode_utf16le_base64(get_shell_integration_script()),
        ],
    }
}

/// 将字符串编码为 UTF-16LE 后 Base64
///
/// PowerShell -EncodedCommand 要求 UTF-16LE 无 BOM + 标准 Base64。
fn encode_utf16le_base64(script: &str) -> String {
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// 在 PATH 中查找可执行文件，返回完整路径
///
/// 遍历 PATH 目录，找到第一个匹配的可执行文件后返回其完整路径。
/// 未找到返回 None。
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

/// 系统目录（%SystemRoot%\System32）下可执行文件的路径
///
/// resolve_shell_info 的 cmd 回退与 validate_shell_allowlist 的系统目录兜底共用，
/// 保证两侧路径一致（Windows 可安装在非 C: 盘，故优先环境变量解析，
/// 环境变量缺失时才硬编码 C:\Windows 兜底）。文件不存在返回 None。
fn system32_exe_path(name: &str) -> Option<String> {
    let root = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("WINDIR"))
        .unwrap_or_else(|_| r"C:\Windows".to_string());
    let path = std::path::Path::new(&root).join("System32").join(name);
    if path.exists() {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 检查可执行文件是否在 PATH 中
fn which_exists(name: &str) -> bool {
    which_full_path(name).is_some()
}

/// 获取 shell 集成脚本内容
///
/// 使用 include_str! 嵌入脚本，确保分发的 exe 不依赖外部脚本路径。
fn get_shell_integration_script() -> &'static str {
    include_str!("../../assets/shell-integration.ps1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_which_full_path_finds_pwsh_or_powershell() {
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
    fn test_which_full_path_nonexistent() {
        assert!(which_full_path("__nonexistent_xyz__.exe").is_none());
    }

    #[test]
    fn test_resolve_shell_info_returns_full_path() {
        let info = if which_full_path("pwsh.exe").is_some()
            || which_full_path("powershell.exe").is_some()
        {
            resolve_shell_info(None)
        } else {
            // 无 pwsh 时至少 cmd 能找到
            resolve_shell_info(Some("cmd.exe"))
        }
        .expect("resolve_shell_info 应成功");
        // program 应是完整路径（含路径分隔符）
        assert!(
            info.program.contains('\\') || info.program.contains('/'),
            "ShellInfo.program 应为完整路径，实际: {}",
            info.program
        );
    }

    #[test]
    fn test_shell_integration_script_embedded() {
        let script = get_shell_integration_script();
        assert!(!script.is_empty(), "集成脚本不应为空");
        assert!(script.contains("OSC"), "脚本应定义 OSC 序列");
    }

    #[test]
    fn test_resolve_shell_accepts_env_vars() {
        // 验证 resolve_shell 返回的 CommandBuilder 支持 .env() 调用
        // 这是 pty_spawn 中注入 COLORTERM/TERM/TERM_PROGRAM 的前提条件
        let mut cmd = resolve_shell(Some("cmd.exe")).expect("resolve_shell 应成功");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM", "xterm-256color");
        cmd.env("TERM_PROGRAM", "slTerminal");
        // 不 panic 即通过
    }

    #[test]
    fn test_build_pwsh_command_accepts_env_vars() {
        // pwsh 路径的 CommandBuilder 同样支持 .env()
        let mut cmd = if which_exists("pwsh.exe") {
            resolve_shell(None).expect("resolve_shell 应成功")
        } else {
            resolve_shell(Some("pwsh.exe")).expect("resolve_shell 应成功")
        };
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM", "xterm-256color");
        // 不 panic 即通过
    }

    #[test]
    fn test_encode_utf16le_base64_roundtrip() {
        let original = "Write-Host 'hello'";
        let encoded = encode_utf16le_base64(original);
        // 解码验证（PowerShell 接受的格式）
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .expect("base64 解码失败");
        // UTF-16LE → UTF-8
        let u16s: Vec<u16> = decoded_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let decoded = String::from_utf16(&u16s).expect("UTF-16 解码失败");
        assert_eq!(decoded, original);
    }

    // ── validate_shell_allowlist 测试 ──

    #[test]
    fn test_allowlist_cmd_exe_passes() {
        validate_shell_allowlist("cmd.exe").expect("cmd.exe 应在白名单内");
    }

    #[test]
    fn test_allowlist_pwsh_exe_passes() {
        validate_shell_allowlist("pwsh.exe").expect("pwsh.exe 应在白名单内");
    }

    #[test]
    fn test_allowlist_powershell_exe_passes() {
        validate_shell_allowlist("powershell.exe").expect("powershell.exe 应在白名单内");
    }

    #[test]
    fn test_allowlist_case_insensitive() {
        validate_shell_allowlist("CMD.EXE").expect("大写 CMD.EXE 应在白名单内");
        validate_shell_allowlist("PwSh.ExE").expect("大小写混合应通过");
    }

    #[test]
    fn test_allowlist_full_path_passes() {
        // SEC-01 修订：完整路径仅当与 PATH 解析结果一致时才放行
        //（旧语义「任意完整路径按文件名放行」正是被修复的绕过点）
        if let Some(resolved) = which_full_path("cmd.exe") {
            validate_shell_allowlist(&resolved).expect("PATH 解析出的 cmd.exe 完整路径应通过");
        }
    }

    #[test]
    fn test_allowlist_rejects_unknown_shell() {
        let result = validate_shell_allowlist("evil.exe");
        assert!(result.is_err(), "evil.exe 不在白名单中，应拒绝");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("evil.exe"), "错误消息应包含被拒绝的程序名");
    }

    #[test]
    fn test_allowlist_rejects_path_not_in_allowlist() {
        let result = validate_shell_allowlist(r"C:\evil\fake-shell.exe");
        assert!(result.is_err(), "fake-shell.exe 不在白名单中，应拒绝");
    }

    #[test]
    fn test_allowlist_resolves_via_path() {
        // cmd.exe 可通过 which_full_path 解析
        let resolved = which_full_path("cmd.exe");
        if let Some(path) = resolved {
            validate_shell_allowlist(&path).expect("通过 PATH 解析的 cmd.exe 应通过白名单校验");
        }
    }

    // ── SEC-01：含路径分隔符的 shell 输入——只信任 PATH 解析出的真实路径 ──

    #[test]
    fn test_allowlist_accepts_path_resolved_from_path() {
        // PATH 解析出的合法绝对路径放行
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        let _guard = set_test_path(&[dir.path()]);
        let resolved = which_full_path("cmd.exe").expect("PATH 应解析出 cmd.exe");
        validate_shell_allowlist(&resolved).expect("PATH 解析出的合法绝对路径应放行");
    }

    #[test]
    fn test_allowlist_accepts_system32_cmd_when_path_lacks_system32() {
        // SEC-01 回归守卫：PATH 不含 System32 时，resolve_shell_info 的 cmd 回退
        // 使用 %SystemRoot%\System32\cmd.exe——校验必须放行，否则合法回退被确定性拒绝
        //（test_resolve_shell_info_fallback_order 场景 3 回归）。
        let dir = tempfile::tempdir().unwrap();
        let _guard = set_test_path(&[dir.path()]);
        let system_cmd =
            system32_exe_path("cmd.exe").expect("测试机 %SystemRoot%\\System32\\cmd.exe 应存在");
        validate_shell_allowlist(&system_cmd).expect("系统目录 cmd.exe 应放行");
    }

    #[test]
    fn test_allowlist_rejects_forged_absolute_path() {
        // 伪造绝对路径拒绝：目录中自建 cmd.exe（白名单文件名）但 PATH 解析不出 → 拒绝
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "cmd.exe");
        // PATH 指向空目录——which_full_path 解析不出 cmd.exe
        let empty_dir = tempfile::tempdir().unwrap();
        let _guard = set_test_path(&[empty_dir.path()]);
        let forged = dir.path().join("cmd.exe").to_string_lossy().into_owned();
        let result = validate_shell_allowlist(&forged);
        assert!(result.is_err(), "非 PATH 解析出的绝对路径应拒绝: {forged}");
    }

    #[test]
    fn test_allowlist_rejects_absolute_path_not_in_path() {
        // 白名单文件名 + 用户目录不在 PATH：即使 PATH 中存在同名文件
        //（解析结果指向另一目录），用户路径与 PATH 解析结果不一致 → 拒绝
        let user_dir = tempfile::tempdir().unwrap();
        let path_dir = tempfile::tempdir().unwrap();
        fake_exe(user_dir.path(), "cmd.exe");
        fake_exe(path_dir.path(), "cmd.exe");
        let _guard = set_test_path(&[path_dir.path()]);

        // 用户目录的 cmd.exe 不是 PATH 解析出的那个 → 拒绝
        let forged = user_dir
            .path()
            .join("cmd.exe")
            .to_string_lossy()
            .into_owned();
        assert!(
            validate_shell_allowlist(&forged).is_err(),
            "与 PATH 解析结果不一致的绝对路径应拒绝: {forged}"
        );

        // PATH 解析出的那个 → 放行
        let legit = path_dir
            .path()
            .join("cmd.exe")
            .to_string_lossy()
            .into_owned();
        validate_shell_allowlist(&legit).expect("与 PATH 解析结果一致的绝对路径应放行");
    }

    // ── paths_match 纯函数测试（SEC-01 alias 兼容修复）──
    //
    // paths_match 两层判定：canonicalize 双成功 → 精确比较；任一侧失败 →
    // 归一字符串比较。fallback 用例用「不存在的路径」构造 canonicalize 双失败
    // （alias 场景等价——alias 文件 exists 为真但普通文件 API 打开失败，
    // canonicalize 同失败，代码路径一致；纯函数层不依赖文件系统权限）。

    #[test]
    fn test_paths_match_canonical_equal() {
        // canonicalize 双成功 → 精确比较：同一文件大小写变体相等
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
    fn test_paths_match_canonical_unequal() {
        // 两个不同 tempdir 文件 → false
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        fake_exe(d1.path(), "cmd.exe");
        fake_exe(d2.path(), "cmd.exe");
        let p1 = d1.path().join("cmd.exe").to_string_lossy().into_owned();
        let p2 = d2.path().join("cmd.exe").to_string_lossy().into_owned();
        assert!(!paths_match(&p1, &p2));
    }

    #[test]
    fn test_paths_match_fallback_case_insensitive() {
        // canonicalize 双失败（应用执行别名场景等价）→ fallback 忽略大小写
        let alias = r"C:\Users\x\AppData\Local\Microsoft\WindowsApps\pwsh.exe";
        let lower = r"c:\users\x\appdata\local\microsoft\windowsapps\pwsh.exe";
        assert!(
            paths_match(alias, lower),
            "fallback 应忽略大小写（Windows 大小写不敏感文件系统）"
        );
    }

    #[test]
    fn test_paths_match_fallback_separator_normalization() {
        // 分隔符归一：/ 与 \ 写法指向同一路径；尾部分隔符容忍
        let a = r"C:/no-such-dir-x/cmd.exe";
        let b = r"C:\no-such-dir-x\cmd.exe";
        assert!(paths_match(a, b), "fallback 应归一化路径分隔符");
        assert!(paths_match(r"C:\no-such-dir-x\cmd.exe\", b));
    }

    #[test]
    fn test_paths_match_fallback_unequal() {
        // 两个不同的不存在路径 → false（伪造路径仍拒绝）
        assert!(!paths_match(r"C:\a\b\cmd.exe", r"C:\a\c\cmd.exe"));
    }

    // ── validate_shell_allowlist alias 兼容集成测试 ──

    #[test]
    fn test_allowlist_nonexistent_absolute_path_rejects_with_unified_message() {
        // canonicalize 失败 + fallback 不匹配 → Err 且消息为「不允许的 shell 程序」
        //（钉死新错误契约：canonicalize 失败不再产出「shell 路径解析失败」文案）
        let result = validate_shell_allowlist(r"C:\Windows\System32\__nope__\cmd.exe");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("不允许的 shell 程序"),
            "错误消息应为统一文案，实际: {msg}"
        );
        assert!(
            !msg.contains("shell 路径解析失败"),
            "不应再出现 canonicalize 失败文案，实际: {msg}"
        );
    }

    // ── 真实应用执行别名测试（SEC-01 alias 兼容，Windows 专属条件测试）──
    //
    // 应用执行别名的「exists 为真、canonicalize 失败（os error 1920）」属性
    // 无法用 tempdir 模拟（Rust std canonicalize 对非 reparse 路径不做句柄打开，
    // 锁定文件无法触发失败）。改用真实环境条件测试：本机装有 Store 版应用
    // （如 MSIX 安装的 PowerShell 7）时 `%LOCALAPPDATA%\Microsoft\WindowsApps\`
    // 下的 pwsh.exe 即真实 alias——PATH 收敛到该目录后，which_full_path 命中
    // alias → canonicalize 失败 → fallback 字符串比对放行。无 alias 的机器
    // （如 CI runner）条件不满足，用例空跑不失败。

    #[cfg(windows)]
    #[test]
    fn test_allowlist_accepts_real_alias_when_present() {
        use std::path::PathBuf;

        let alias_dir = std::env::var("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Microsoft").join("WindowsApps"))
            .ok();
        let alias = alias_dir.map(|d| d.join("pwsh.exe"));
        if let Some(a) = alias {
            if !a.exists() {
                return; // 本机无 Store 版 pwsh（无应用执行别名）→ 条件不满足空跑
            }
            let _guard = set_test_path(&[a.parent().expect("alias 目录")]);
            // PATH 收敛到 alias 目录后，which_full_path 首匹配即 alias 本身
            assert_eq!(
                which_full_path("pwsh.exe").as_deref(),
                Some(a.to_str().expect("alias 路径为 UTF-8")),
                "PATH 收敛后应命中 alias"
            );
            // canonicalize(alias) 失败（os error 1920）→ fallback 字符串比对放行
            let s = a.to_string_lossy().into_owned();
            validate_shell_allowlist(&s)
                .expect("真实 alias 路径应放行（canonicalize 失败 fallback）");
        }
    }

    // ── PATH 可控测试辅助（PTY-06/PTY-10/PTY-13③）──
    //
    // 三个用例组通过整体替换 PATH 环境变量驱动回退顺序与解析行为。
    // 环境变量全局可变，依赖 L1 全量 `--test-threads=1` 门禁（串行执行无污染）。

    /// PATH 恢复守卫——测试结束时（含 panic）还原原 PATH，避免污染后续用例
    struct PathGuard(Option<std::ffi::OsString>);

    impl Drop for PathGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    /// 用给定目录列表整体替换 PATH（平台正确分隔符），返回恢复守卫
    fn set_test_path(paths: &[&std::path::Path]) -> PathGuard {
        let old = std::env::var_os("PATH");
        let joined = std::env::join_paths(paths.iter()).expect("构造 PATH 失败");
        std::env::set_var("PATH", joined);
        PathGuard(old)
    }

    /// 在目录中创建假可执行文件——空文件即可（which_full_path 只查 exists）
    fn fake_exe(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"").expect("写假 exe 失败");
        path
    }

    // ── PTY-06：resolve_shell_info 自动检测回退顺序 ──

    #[test]
    fn test_resolve_shell_info_fallback_order() {
        let dir = tempfile::tempdir().unwrap();

        // 场景 1：只有 pwsh → 命中 pwsh（完整路径 + 集成脚本参数）
        fake_exe(dir.path(), "pwsh.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let info = resolve_shell_info(None).expect("应命中 pwsh");
            let expect = dir.path().join("pwsh.exe");
            assert_eq!(info.program, expect.to_string_lossy().as_ref());
            assert!(
                info.args.contains(&"-EncodedCommand".to_string()),
                "pwsh 应携带集成脚本参数"
            );
        }

        // 场景 2：只有 powershell → 命中 powershell
        std::fs::remove_file(dir.path().join("pwsh.exe")).unwrap();
        fake_exe(dir.path(), "powershell.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let info = resolve_shell_info(None).expect("应命中 powershell");
            let expect = dir.path().join("powershell.exe");
            assert_eq!(info.program, expect.to_string_lossy().as_ref());
        }

        // 场景 3：都没有 → 回退 cmd（PATH 不含 System32 → 硬编码兜底路径，不带参数）
        std::fs::remove_file(dir.path().join("powershell.exe")).unwrap();
        {
            let _guard = set_test_path(&[dir.path()]);
            let info = resolve_shell_info(None).expect("应回退 cmd");
            assert!(
                info.program.ends_with("cmd.exe"),
                "应回退 cmd.exe，实际: {}",
                info.program
            );
            assert!(info.args.is_empty(), "cmd 回退不带参数");
        }

        // 场景 4：pwsh 与 powershell 并存 → pwsh 优先（回退顺序首档）
        fake_exe(dir.path(), "pwsh.exe");
        fake_exe(dir.path(), "powershell.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let info = resolve_shell_info(None).expect("应命中 pwsh");
            let expect = dir.path().join("pwsh.exe");
            assert_eq!(info.program, expect.to_string_lossy().as_ref());
        }
    }

    // ── PTY-10：resolve_shell 回退顺序 + 白名单 PATH 解析后仍拒绝 ──

    #[test]
    fn test_resolve_shell_fallback_order() {
        let dir = tempfile::tempdir().unwrap();

        // 场景 1：只有 pwsh → 命中 pwsh（argv[0] + 集成脚本参数）
        fake_exe(dir.path(), "pwsh.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let cmd = resolve_shell(None).expect("应命中 pwsh");
            assert_eq!(cmd.get_argv()[0], "pwsh.exe");
            assert!(cmd.get_argv().len() > 1, "pwsh 应带集成脚本参数");
        }

        // 场景 2：只有 powershell → 命中 powershell
        std::fs::remove_file(dir.path().join("pwsh.exe")).unwrap();
        fake_exe(dir.path(), "powershell.exe");
        {
            let _guard = set_test_path(&[dir.path()]);
            let cmd = resolve_shell(None).expect("应命中 powershell");
            assert_eq!(cmd.get_argv()[0], "powershell.exe");
        }

        // 场景 3：都没有 → 回退 cmd（argv 仅 program，无参数）
        std::fs::remove_file(dir.path().join("powershell.exe")).unwrap();
        {
            let _guard = set_test_path(&[dir.path()]);
            let cmd = resolve_shell(None).expect("应回退 cmd");
            assert_eq!(cmd.get_argv()[0], "cmd.exe");
            assert_eq!(cmd.get_argv().len(), 1, "cmd 回退不带参数");
        }
    }

    #[test]
    fn test_allowlist_rejects_path_resolved_non_allowlisted() {
        // 用户指定 shell 经 PATH 解析成功（文件真实存在）但非白名单 → 仍拒绝
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "fake-shell.exe");
        let _guard = set_test_path(&[dir.path()]);

        // 直接校验：文件名不在白名单，PATH 解析成功仍拒绝
        let result = validate_shell_allowlist("fake-shell.exe");
        assert!(result.is_err(), "PATH 解析成功但非白名单应拒绝");

        // resolve_shell 用户指定路径同语义
        assert!(
            resolve_shell(Some("fake-shell.exe")).is_err(),
            "resolve_shell 用户指定非白名单应拒绝"
        );

        // resolve_shell_info 用户指定路径同语义（短名 → PATH 解析 → 白名单拒绝）
        assert!(
            resolve_shell_info(Some("fake-shell.exe")).is_err(),
            "resolve_shell_info 用户指定非白名单应拒绝"
        );
    }

    // ── PTY-13③：which_full_path PATH 顺序与大小写边界 ──

    #[test]
    fn test_which_full_path_first_match_in_path_order() {
        // PATH 多目录时返回第一个匹配目录的完整路径（目录顺序优先）
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
    fn test_which_full_path_case_sensitivity() {
        // 大小写边界：目录中文件名大小写与查询名不同
        let dir = tempfile::tempdir().unwrap();
        fake_exe(dir.path(), "PwSh.ExE");
        let _guard = set_test_path(&[dir.path()]);

        let found = which_full_path("pwsh.exe");
        #[cfg(windows)]
        {
            // Windows 文件系统大小写不敏感 → 命中
            assert!(found.is_some(), "Windows 应命中 PwSh.ExE");
        }
        #[cfg(not(windows))]
        {
            // Unix 文件系统大小写敏感 → 不命中
            assert!(found.is_none(), "Unix 不应命中 PwSh.ExE");
        }
    }
}
