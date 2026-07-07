/// Shell 选择与 PowerShell profile 注入
///
/// shell 选择策略：pwsh.exe → powershell.exe → cmd.exe 回退。
/// PowerShell 通过 -EncodedCommand（UTF-16LE Base64）内联集成脚本，
/// 消除 %APPDATA% 文件写入——避免 AMSI/ASR 误杀。
use base64::Engine;
use portable_pty::CommandBuilder;

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

/// 解析 shell 程序，返回已配置好参数的基础 CommandBuilder
///
/// 用户指定则直接使用；否则按 pwsh → powershell → cmd 顺序检测。
/// PowerShell 自动加入 -NoProfile -NoLogo -EncodedCommand <base64> 参数。
#[allow(dead_code)] // 保留供非 Windows 平台 fallback
pub fn resolve_shell(user_shell: Option<&str>) -> CommandBuilder {
    if let Some(shell) = user_shell {
        return CommandBuilder::new(shell);
    }

    // 检测 pwsh.exe
    if which_exists("pwsh.exe") {
        return build_pwsh_command("pwsh.exe");
    }

    // 回退 powershell.exe
    if which_exists("powershell.exe") {
        return build_pwsh_command("powershell.exe");
    }

    // 最终回退 cmd.exe
    CommandBuilder::new("cmd.exe")
}

/// 解析 shell 程序，返回不依赖 portable-pty 的 ShellInfo
///
/// 与 resolve_shell 相同的检测逻辑，但返回纯数据结构，
/// 供自定义 ConPTY 创建流程（绕过 CommandBuilder）使用。
/// program 字段为完整路径（通过 which_full_path 解析），
/// 确保 CreateProcessW(lpApplicationName=...) 可正确定位可执行文件。
pub fn resolve_shell_info(user_shell: Option<&str>) -> ShellInfo {
    // 用户指定 shell：直接用指定路径（可能已是完整路径）
    if let Some(shell) = user_shell {
        // 尝试解析完整路径（如果 shell 是短名的话）
        let program = if shell.contains('\\') || shell.contains('/') {
            // 已有路径分隔符，原样使用
            shell.to_string()
        } else {
            // 短名 → 在 PATH 中解析
            which_full_path(shell).unwrap_or_else(|| shell.to_string())
        };
        return ShellInfo {
            program,
            args: vec![],
        };
    }

    if let Some(path) = which_full_path("pwsh.exe") {
        return build_pwsh_info(&path);
    }

    if let Some(path) = which_full_path("powershell.exe") {
        return build_pwsh_info(&path);
    }

    // cmd.exe 始终在 System32 下，which_full_path 可能找不到（PATH 不含 System32 的极端情况），
    // 直接用完整路径回退
    let cmd_path = which_full_path("cmd.exe")
        .unwrap_or_else(|| r"C:\Windows\System32\cmd.exe".to_string());
    ShellInfo {
        program: cmd_path,
        args: vec![],
    }
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
        for dir in path.split(';') {
            let full = std::path::PathBuf::from(dir).join(name);
            if full.exists() {
                return Some(full.to_string_lossy().into_owned());
            }
        }
    }
    None
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
        let found = which_full_path("pwsh.exe")
            .or_else(|| which_full_path("powershell.exe"));
        assert!(found.is_some(), "至少 pwsh 或 powershell 应存在");
        let path = found.unwrap();
        assert!(path.contains('\\') || path.contains('/'), "应返回完整路径，实际: {path}");
        assert!(path.ends_with(".exe"), "路径应以 .exe 结尾，实际: {path}");
    }

    #[test]
    fn test_which_full_path_nonexistent() {
        assert!(which_full_path("__nonexistent_xyz__.exe").is_none());
    }

    #[test]
    fn test_resolve_shell_info_returns_full_path() {
        let info = if which_full_path("pwsh.exe").is_some() || which_full_path("powershell.exe").is_some() {
            resolve_shell_info(None)
        } else {
            // 无 pwsh 时至少 cmd 能找到
            resolve_shell_info(Some("cmd.exe"))
        };
        // program 应是完整路径（含路径分隔符）
        assert!(
            info.program.contains('\\') || info.program.contains('/'),
            "ShellInfo.program 应为完整路径，实际: {}", info.program
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
        let mut cmd = resolve_shell(Some("cmd.exe"));
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM", "xterm-256color");
        cmd.env("TERM_PROGRAM", "slTerminal");
        // 不 panic 即通过
    }

    #[test]
    fn test_build_pwsh_command_accepts_env_vars() {
        // pwsh 路径的 CommandBuilder 同样支持 .env()
        let mut cmd = if which_exists("pwsh.exe") {
            resolve_shell(None)
        } else {
            resolve_shell(Some("pwsh.exe"))
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
}
