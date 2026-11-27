//! Claude statusline 桥接：备份原命令、写入托管脚本与用量信号。
//!
//! 脚本先透传用户原 statusline 输出，再节流写 ContextUsage 信号；设置文件
//! 的 statusLine 接管归安装器事务，卸载按 marker 恢复原值后再清理材料。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::atomic_file;

pub(super) const STATUSLINE_SCRIPT: &str = "slterm-statusline.ps1";
pub(super) const STATUSLINE_MARKER: &str = "slterm-statusline.json";
pub(super) const STATUSLINE_SIGNAL: &str = "statusline-context.json";
const MARKER_VERSION: u64 = 1;

/// 应用本地根下的用量信号路径；脚本与消费端共用同一文件名。
pub(crate) fn signal_path() -> PathBuf {
    crate::platform::dirs::local_dir().join(STATUSLINE_SIGNAL)
}

/// 用量信号；字段缺失或损坏整条不可读，不伪造百分比。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContextUsage {
    pub(crate) used_percentage: Option<f64>,
    pub(crate) used_tokens: u64,
    pub(crate) total_tokens: u64,
    pub(crate) updated_at_ms: u64,
    pub(crate) session_id: Option<String>,
}

impl ContextUsage {
    pub(crate) fn read(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        // Windows PowerShell 5.1 的 utf8 编码带 BOM；信号读取必须容忍。
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        let value: Value = serde_json::from_str(raw).ok()?;
        if value.get("version")?.as_u64()? != 1 {
            return None;
        }
        let used_tokens = value.get("used_tokens")?.as_u64()?;
        let total_tokens = value.get("total_tokens")?.as_u64()?;
        if total_tokens == 0 {
            return None;
        }
        Some(Self {
            used_percentage: value
                .get("used_percentage")
                .and_then(Value::as_f64)
                .filter(|percentage| percentage.is_finite() && *percentage >= 0.0),
            used_tokens,
            total_tokens,
            updated_at_ms: value.get("updated_at_ms")?.as_u64()?,
            session_id: value
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|session| !session.is_empty())
                .map(str::to_owned),
        })
    }
}

fn script_path(dir: &Path) -> PathBuf {
    dir.join(STATUSLINE_SCRIPT)
}

fn marker_path(dir: &Path) -> PathBuf {
    dir.join(STATUSLINE_MARKER)
}

fn managed_command(dir: &Path) -> String {
    let script = script_path(dir);
    format!(
        "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
        script.display()
    )
}

fn current_command(root: &Value) -> Option<&str> {
    root.get("statusLine")?.get("command")?.as_str()
}

fn marker(dir: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(marker_path(dir)).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    (value.get("version")?.as_u64()? == MARKER_VERSION).then_some(value)
}

fn script_text(signal: &Path) -> String {
    let signal = signal.to_string_lossy().replace('\'', "''");
    SCRIPT_TEMPLATE.replace("__SLTERM_SIGNAL_PATH__", &signal)
}

const SCRIPT_TEMPLATE: &str = r#"$ErrorActionPreference = 'SilentlyContinue'
$raw = [Console]::In.ReadToEnd()
$marker = Join-Path $PSScriptRoot 'slterm-statusline.json'
$signal = '__SLTERM_SIGNAL_PATH__'
$original = ''
try {
    $owned = Get-Content -Raw -LiteralPath $marker | ConvertFrom-Json
    $original = [string]$owned.original_command
} catch { }
try {
    $payload = $raw | ConvertFrom-Json
    $context = $payload.context_window
    if ($null -ne $context) {
        $usedTokens = [double]$context.total_input_tokens + [double]$context.total_output_tokens
        $totalTokens = [double]$context.context_window_size
        $percentage = $context.used_percentage
        if ($null -eq $percentage -and $totalTokens -gt 0) {
            $percentage = 100.0 * $usedTokens / $totalTokens
        }
        $write = $true
        if (Test-Path -LiteralPath $signal) {
            $age = ([DateTime]::UtcNow - (Get-Item -LiteralPath $signal).LastWriteTimeUtc).TotalMilliseconds
            if ($age -lt 1000) { $write = $false }
        }
        if ($write) {
            [ordered]@{
                version = 1
                session_id = [string]$payload.session_id
                used_percentage = $percentage
                used_tokens = [long]$usedTokens
                total_tokens = [long]$totalTokens
                updated_at_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
            } | ConvertTo-Json -Compress | Set-Content -LiteralPath $signal -Encoding utf8 -NoNewline
        }
    }
} catch { }
if (-not [string]::IsNullOrWhiteSpace($original)) {
    try {
        $info = New-Object System.Diagnostics.ProcessStartInfo
        $info.FileName = 'cmd.exe'
        $info.Arguments = '/d /c "' + $original + '"'
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardInput = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $process = [System.Diagnostics.Process]::Start($info)
        $process.StandardInput.Write($raw)
        $process.StandardInput.Close()
        $output = $process.StandardOutput.ReadToEnd()
        [void]$process.WaitForExit(5000)
        [Console]::Out.Write($output)
    } catch { }
}
exit 0
"#;

/// 安装桥接：脚本与 marker 先落盘，设置文件的 statusLine 由调用方原子写。
/// 返回设置值是否需要变更。
pub(super) fn prepare(dir: &Path, root: &mut Value) -> std::io::Result<bool> {
    prepare_at(dir, root, &signal_path())
}

pub(super) fn prepare_at(dir: &Path, root: &mut Value, signal: &Path) -> std::io::Result<bool> {
    let managed = managed_command(dir);
    if let Some(owned) = marker(dir) {
        if current_command(root) == Some(managed.as_str()) {
            atomic_file::write(&script_path(dir), script_text(signal).as_bytes())?;
            write_marker(dir, &owned, signal)?;
            return Ok(false);
        }
        // 用户已接管 statusLine：移除失效 marker，不覆盖其选择。
        let _ = std::fs::remove_file(marker_path(dir));
        let _ = std::fs::remove_file(script_path(dir));
        return Ok(false);
    }
    let Some(original) = root.get("statusLine").cloned() else {
        // 没有用户状态行时不凭空添加空状态行；用量桥接只包裹既有命令。
        return Ok(false);
    };
    if original.get("type").and_then(Value::as_str) != Some("command")
        || original.get("command").and_then(Value::as_str).is_none()
    {
        return Ok(false);
    }
    let original_command = original
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let script = script_text(signal);
    atomic_file::write(&script_path(dir), script.as_bytes())?;
    let owned = json!({
        "version": MARKER_VERSION,
        "had_statusline": true,
        "original_command": original_command,
        "original_statusline": original,
    });
    if let Err(error) = write_marker(dir, &owned, signal) {
        let _ = std::fs::remove_file(script_path(dir));
        return Err(error);
    }
    let mut replacement = owned["original_statusline"].clone();
    let Some(object) = replacement.as_object_mut() else {
        let _ = std::fs::remove_file(marker_path(dir));
        let _ = std::fs::remove_file(script_path(dir));
        return Ok(false);
    };
    object.insert("type".into(), json!("command"));
    object.insert("command".into(), json!(managed));
    root["statusLine"] = replacement;
    Ok(true)
}

fn write_marker(dir: &Path, owned: &Value, signal: &Path) -> std::io::Result<()> {
    let mut marker = owned.clone();
    let Some(object) = marker.as_object_mut() else {
        return Ok(());
    };
    object.insert("signal".into(), json!(signal.to_string_lossy().to_string()));
    object.insert("script".into(), json!(STATUSLINE_SCRIPT));
    atomic_file::write(
        &marker_path(dir),
        (serde_json::to_string_pretty(&marker)? + "\n").as_bytes(),
    )
}

/// 卸载桥接：恢复原 statusLine；设置写成功后调用方再清理材料。
/// 返回设置值是否需要变更。
pub(super) fn restore(dir: &Path, root: &mut Value) -> bool {
    let Some(owned) = marker(dir) else {
        return false;
    };
    let managed = managed_command(dir);
    let mut changed = false;
    if current_command(root) == Some(managed.as_str()) {
        if owned.get("had_statusline").and_then(Value::as_bool) == Some(true) {
            let original = owned
                .get("original_statusline")
                .cloned()
                .unwrap_or(Value::Null);
            if original.is_object() {
                root["statusLine"] = original;
                changed = true;
            }
        } else if let Some(object) = root.as_object_mut() {
            object.remove("statusLine");
            changed = true;
        }
    }
    changed
}

/// 设置文件已提交后清理脚本、marker 与信号；尽力删除全部材料并返回首个
/// 失败，供设置页保留「已提交但清理未完成」的重试事实。
pub(super) fn cleanup_materials(dir: &Path) -> std::io::Result<()> {
    let signal = marker(dir).and_then(|owned| {
        owned
            .get("signal")
            .and_then(Value::as_str)
            .map(PathBuf::from)
    });
    let mut first_error = None;
    for path in [marker_path(dir), script_path(dir), signal_path()] {
        remove_if_present(&path, &mut first_error);
    }
    if let Some(signal) = signal {
        remove_if_present(&signal, &mut first_error);
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn remove_if_present(path: &Path, first_error: &mut Option<std::io::Error>) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
        }
    }
}

/// 脚本或 marker 仍然存在即视为清理未完成；信号缺少 marker 时无法定位。
pub(super) fn materials_present(dir: &Path) -> bool {
    marker_path(dir).exists() || script_path(dir).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(temp: &tempfile::TempDir) -> PathBuf {
        let dir = temp.path().join("claude");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn bridge_wraps_and_restores_the_original_command() {
        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let signal = temp.path().join("usage.json");
        let mut root =
            json!({"statusLine": {"type": "command", "command": "node hud.js", "padding": 2}});
        assert!(prepare_at(&dir, &mut root, &signal).unwrap());
        assert!(script_path(&dir).is_file());
        assert!(marker_path(&dir).is_file());
        assert!(current_command(&root).unwrap().contains(STATUSLINE_SCRIPT));
        let marker: Value =
            serde_json::from_str(&std::fs::read_to_string(marker_path(&dir)).unwrap()).unwrap();
        assert_eq!(marker["original_command"], "node hud.js");

        assert!(restore(&dir, &mut root));
        assert_eq!(root["statusLine"]["command"], "node hud.js");
        assert_eq!(root["statusLine"]["type"], "command");
        assert_eq!(
            root["statusLine"]["padding"], 2,
            "原 statusLine 其余字段必须保留"
        );
    }

    #[test]
    fn bridge_skips_when_statusline_is_absent() {
        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let signal = temp.path().join("usage.json");
        let mut root = json!({});
        assert!(!prepare_at(&dir, &mut root, &signal).unwrap());
        assert!(root.get("statusLine").is_none());
        assert!(!script_path(&dir).exists());
        assert!(!marker_path(&dir).exists());
    }

    #[test]
    fn bridge_skips_non_command_statusline() {
        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let signal = temp.path().join("usage.json");
        let mut root = json!({"statusLine": {"type": "static", "value": "hi"}});
        assert!(!prepare_at(&dir, &mut root, &signal).unwrap());
        assert_eq!(root["statusLine"]["type"], "static");
        assert!(!script_path(&dir).exists());
    }

    #[test]
    fn user_replacement_is_never_overwritten_or_restored() {
        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let signal = temp.path().join("usage.json");
        let mut root = json!({"statusLine": {"type": "command", "command": "node hud.js"}});
        prepare_at(&dir, &mut root, &signal).unwrap();
        root["statusLine"]["command"] = json!("node replaced.js");
        assert!(!prepare_at(&dir, &mut root, &signal).unwrap());
        assert!(!marker_path(&dir).exists());
        root["statusLine"]["command"] = json!("node replaced.js");
        assert!(!restore(&dir, &mut root));
        assert_eq!(root["statusLine"]["command"], "node replaced.js");
    }

    /// 安装器写 settings 后卸载必须恢复原 statusLine 并删除全部桥接材料。
    #[test]
    fn installer_uninstall_restores_statusline_and_cleans_materials() {
        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let settings = dir.join("settings.json");
        std::fs::write(
            &settings,
            br#"{"statusLine":{"type":"command","command":"node hud.js","padding":1}}"#,
        )
        .unwrap();
        assert_eq!(
            super::super::ensure_claude_hooks_in(&dir, "C:/tools/slterm-hook.exe", |_| {
                super::super::InstallGate::Proceed
            }),
            super::super::InstallOutcome::Installed
        );
        assert!(script_path(&dir).is_file());
        assert!(super::super::remove_claude_hooks_at(&dir).unwrap());
        let restored: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored["statusLine"]["command"], "node hud.js");
        assert_eq!(restored["statusLine"]["padding"], 1);
        assert!(
            !restored.to_string().contains("slterm-hook"),
            "卸载后不得残留托管 hook: {restored}"
        );
        assert!(!script_path(&dir).exists());
        assert!(!marker_path(&dir).exists());
    }

    /// 真实 PowerShell 执行生成的桥接：透传原命令并写节流用量信号。
    #[test]
    fn generated_script_passes_through_and_writes_context_usage() {
        use std::io::Write as _;

        let temp = tempfile::tempdir().unwrap();
        let dir = dir(&temp);
        let signal = temp.path().join("usage.json");
        let mut root = json!({"statusLine": {"type": "command", "command": "echo passthrough"}});
        prepare_at(&dir, &mut root, &signal).unwrap();
        let payload = r#"{"session_id":"s","context_window":{"total_input_tokens":100,"total_output_tokens":50,"context_window_size":1000,"used_percentage":15}}"#;
        let run = |payload: &str| {
            let mut child = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(script_path(&dir))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload.as_bytes())
                .unwrap();
            child.wait_with_output().unwrap()
        };
        let output = run(payload);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("passthrough"), "stdout: {stdout}");
        let usage = ContextUsage::read(&signal).expect("脚本必须写入用量信号");
        assert_eq!(usage.used_percentage, Some(15.0));
        assert_eq!(usage.used_tokens, 150);
        assert_eq!(usage.total_tokens, 1000);
        assert_eq!(usage.session_id.as_deref(), Some("s"));

        let first = std::fs::read(&signal).unwrap();
        let second = run(
            r#"{"session_id":"s2","context_window":{"total_input_tokens":1,"total_output_tokens":1,"context_window_size":1000}}"#,
        );
        assert!(second.status.success());
        assert_eq!(
            std::fs::read(&signal).unwrap(),
            first,
            "1 秒内重复渲染不得重写信号"
        );
    }

    #[test]
    fn cleanup_reports_leftover_materials_instead_of_swallowing_errors() {
        let temp = tempfile::tempdir().unwrap();
        let dir = super::tests::dir(&temp);
        let script = script_path(&dir);
        std::fs::create_dir_all(&script).unwrap();
        std::fs::write(script.join("occupied"), b"x").unwrap();

        assert!(
            cleanup_materials(&dir).is_err(),
            "无法删除的材料必须显式失败"
        );
        assert!(materials_present(&dir), "残留材料必须可检测");

        std::fs::remove_dir_all(&script).unwrap();
        assert!(cleanup_materials(&dir).is_ok());
        assert!(!materials_present(&dir));
    }

    #[test]
    fn context_usage_requires_valid_signal() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        std::fs::write(
            &path,
            br#"{"version":1,"session_id":"s","used_percentage":42.5,"used_tokens":85000,"total_tokens":200000,"updated_at_ms":7}"#,
        )
        .unwrap();
        let usage = ContextUsage::read(&path).unwrap();
        assert_eq!(usage.used_percentage, Some(42.5));
        assert_eq!(usage.used_tokens, 85_000);
        assert_eq!(usage.total_tokens, 200_000);
        assert_eq!(usage.session_id.as_deref(), Some("s"));
        std::fs::write(&path, br#"{"version":1,"total_tokens":0}"#).unwrap();
        assert!(ContextUsage::read(&path).is_none());
        std::fs::write(&path, b"broken").unwrap();
        assert!(ContextUsage::read(&path).is_none());
    }
}
