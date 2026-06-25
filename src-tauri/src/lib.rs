mod error;
mod state;
mod pty;
mod fs;
mod claude;
mod notify;

pub use error::AppError;
pub use state::AppState;
pub use state::PtyState;
use tauri_plugin_prevent_default::{Builder as PreventDefaultBuilder, Flags, PlatformOptions};

/// ping 命令 — Phase 0 占位，用于验证 IPC 链路和测试基建
#[tauri::command]
fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
}

/// 获取 Windows 真实 build 号（F3 动态检测）
///
/// 通过 RtlGetNtVersionNumbers 获取，取低 16 位（& 0xFFFF）。
/// 非 Windows 平台返回 Unknown 错误——前端需 try/catch fallback 到 21376。
#[cfg(windows)]
#[tauri::command]
fn get_windows_build_number() -> Result<u32, AppError> {
    let (_, _, build) = nt_version::get();
    Ok(build & 0x0FFFFFFF)  // 清除高 4 位构建类型标志（0xF=零售版），低 28 位为实际 build 号
}

#[cfg(not(windows))]
#[tauri::command]
fn get_windows_build_number() -> Result<u32, AppError> {
    Err(AppError::Unknown(
        "get_windows_build_number 仅 Windows 平台支持".to_string(),
    ))
}

/// 注册所有 Tauri 命令和全局状态
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .init();
    tracing::info!("slTerminal 启动");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_wdio_webdriver::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            PreventDefaultBuilder::default()
                .with_flags(
                    Flags::all()
                        .difference(Flags::FIND),
                )
                .platform(
                    PlatformOptions::new()
                        .browser_accelerator_keys(false)
                        .dev_tools(false),
                )
                .build(),
        )
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            ping,
            get_windows_build_number,
            pty::spawn::pty_spawn,
            pty::spawn::pty_write,
            pty::spawn::pty_resize,
            pty::spawn::pty_kill,
            fs::fs_read_file,
            fs::fs_write_file,
            fs::save_settings,
            fs::load_settings,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_returns_pong() {
        let result = ping();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "pong".to_string());
    }

    #[test]
    fn test_get_windows_build_number_returns_number() {
        let result = get_windows_build_number();
        #[cfg(windows)]
        {
            assert!(
                result.is_ok(),
                "Windows 上应返回 build 号"
            );
            let build = result.unwrap();
            // Windows 10 最低 build 号为 10240
            assert!(build > 10000, "build 号应大于 10000，实际: {build}");
        }
        #[cfg(not(windows))]
        {
            assert!(result.is_err(), "非 Windows 平台应返回错误");
            match result {
                Err(AppError::Unknown(msg)) => {
                    assert!(msg.contains("仅 Windows"), "错误消息应含平台提示");
                }
                _ => panic!("非 Windows 应返回 AppError::Unknown"),
            }
        }
    }
}
