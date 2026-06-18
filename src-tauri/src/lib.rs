mod error;
mod state;
mod pty;
mod fs;
mod git;
mod claude;
mod notify;

use error::AppError;
use state::AppState;

/// ping 命令 — Phase 0 占位，用于验证 IPC 链路和测试基建
#[tauri::command]
fn ping() -> Result<String, AppError> {
    Ok("pong".to_string())
}

/// 注册所有 Tauri 命令和全局状态
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {})
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
