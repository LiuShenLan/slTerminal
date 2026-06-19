/// Shell 选择与 PowerShell profile 注入
///
/// shell 选择策略：pwsh.exe → powershell.exe → cmd.exe 回退。
/// PowerShell 通过 -File 参数加载集成脚本，注入 OSC 序列跟踪 CWD 和命令边界。
use portable_pty::CommandBuilder;

/// 解析 shell 程序，返回已配置好参数的基础 CommandBuilder
///
/// 用户指定则直接使用；否则按 pwsh → powershell → cmd 顺序检测。
/// PowerShell 自动加入 -NoProfile -NoLogo -File <script> 参数。
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
/// 使用 -NoProfile -NoLogo -File <script> 启动。
fn build_pwsh_command(pwsh: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(pwsh);
    cmd.arg("-NoProfile");
    cmd.arg("-NoLogo");
    cmd.arg("-File");

    // 获取集成脚本的路径
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let script_dir = std::path::PathBuf::from(&appdata).join("slTerminal");
    let script_path = script_dir.join("shell-integration.ps1");

    // 确保脚本存在
    if !script_path.exists() {
        let _ = std::fs::create_dir_all(&script_dir);
        let _ = std::fs::write(&script_path, get_shell_integration_script());
    }

    cmd.arg(script_path.to_string_lossy().to_string());
    cmd
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
}
