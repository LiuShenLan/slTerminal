mod error;
mod state;
mod pty;
mod fs;
mod git;
mod claude;
mod notify;

use error::AppError;
use state::AppState;
use tauri_plugin_prevent_default::{Builder as PreventDefaultBuilder, Flags};

/// ping 命令 — Phase 0 占位，用于验证 IPC 链路和测试基建
#[tauri::command]
fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
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
        .plugin(tauri_plugin_wdio_webdriver::init())
        .plugin(
            PreventDefaultBuilder::default()
                .with_flags(
                    Flags::all()
                        .difference(Flags::FIND | Flags::DEV_TOOLS),
                )
                .build(),
        )
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            ping,
            pty::spawn::pty_spawn,
            pty::spawn::pty_write,
            pty::spawn::pty_resize,
            pty::spawn::pty_kill,
            fs::fs_read_file,
            fs::fs_write_file,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
