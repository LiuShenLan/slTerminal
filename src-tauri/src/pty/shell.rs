/// Shell 选择与 PowerShell profile 注入
///
/// shell 选择策略：pwsh.exe → powershell.exe → cmd.exe 回退。
/// PowerShell 通过 -EncodedCommand（UTF-16LE Base64）内联集成脚本，
/// 消除 %APPDATA% 文件写入——避免 AMSI/ASR 误杀。
use base64::Engine;
use portable_pty::CommandBuilder;

/// 解析 shell 程序，返回已配置好参数的基础 CommandBuilder
///
/// 用户指定则直接使用；否则按 pwsh → powershell → cmd 顺序检测。
/// PowerShell 自动加入 -NoProfile -NoLogo -EncodedCommand <base64> 参数。
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

/// 检查可执行文件是否在 PATH 中
fn which_exists(name: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(';') {
            let full = std::path::PathBuf::from(dir).join(name);
            if full.exists() {
                return true;
            }
        }
    }
    false
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
    fn test_which_exists_pwsh() {
        let found = which_exists("pwsh.exe") || which_exists("powershell.exe");
        assert!(found, "至少 pwsh 或 powershell 应存在");
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
