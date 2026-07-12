mod error;
mod state;
pub mod pty;
mod fs;
mod settings;
mod notify;
mod git;

pub use error::AppError;
pub use state::AppState;
pub use state::PtyState;
use crate::pty::win_build::get_windows_build_number;
use tauri_plugin_prevent_default::{Builder as PreventDefaultBuilder, Flags, PlatformOptions};

/// ping 命令 — 占位，用于验证 IPC 链路和测试基建
#[tauri::command]
fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
}

/// 注册所有 Tauri 命令和全局状态
///
/// ## 权限模型说明（P0-07）
///
/// Tauri 2 默认行为：`invoke_handler` 注册的自定义命令自动对所有窗口/webview 放行，
/// 无需在 `capabilities/default.json` 中逐条声明。
/// `app:` 命名空间属于 `@tauri-apps/api/app` 插件，与自定义应用命令无关。
/// 如需严格显式权限控制，可在 build.rs 中配置 `AppManifest::commands` 以 opt-in
/// 白名单模式——当前项目采用 Tauri 2 默认行为。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")))
        .init();

    // wdio-webdriver 仅 debug 构建启用，生产构建排除（P0-08）
    #[cfg(debug_assertions)]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    #[cfg(not(debug_assertions))]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    match builder
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
                        .dev_tools(false)
                        .default_script_dialogs(true),
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
            pty::spawn::pty_reattach,
            fs::fs_read_file,
            fs::fs_write_file,
            fs::fs_read_dir,
            fs::fs_create_dir,
            fs::fs_delete,
            fs::fs_rename,
            settings::save_settings,
            settings::load_settings,
            git::git_status,
            git::git_diff,
            notify::notify_watch,
        ])
        .run(tauri::generate_context!())
    {
        Ok(()) => {}
        Err(e) => {
            eprintln!("slTerminal 启动失败: {e}");
            std::process::exit(1);
        }
    }
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
        let result = crate::pty::win_build::get_windows_build_number();
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
