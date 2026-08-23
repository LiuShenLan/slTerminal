pub mod agent_history;
mod app_dir;
mod error;
mod fs;
pub mod git;
mod hooks;
mod notify;
mod projects;
pub mod pty;
mod settings;
mod state;

use crate::pty::win_build::get_windows_build_number;
pub use error::AppError;
pub use state::validate_path_within_root;
pub use state::AppState;
pub use state::GitRepoCache;
pub use state::PtyState;
pub use state::GIT_REPO_CACHE_CAPACITY;
use tauri_plugin_prevent_default::{Builder as PreventDefaultBuilder, Flags, PlatformOptions};

/// 安装 panic hook：panic 信息写 exe 同级 crash.log（诊断用，TQ-COV-01）。
/// 写文件失败回退 eprintln（不 panic——hook 内再 panic 会二次崩溃）。
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let message = format!("PANIC: {:?}", info);
        if write_crash_log(&dir, &message).is_err() {
            eprintln!("{}", message);
        }
    }));
}

/// 写 crash.log（目录参数化以便 L1 测试）——返回 io 结果供 hook 决定回退
fn write_crash_log(dir: &std::path::Path, message: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(dir.join("crash.log"))?;
    writeln!(f, "{}", message)
}

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
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // wdio-webdriver 仅 debug 构建启用，生产构建排除（P0-08）
    #[cfg(debug_assertions)]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    #[cfg(not(debug_assertions))]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());

    match builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            PreventDefaultBuilder::default()
                .with_flags(Flags::all().difference(Flags::FIND))
                .platform(
                    PlatformOptions::new()
                        .browser_accelerator_keys(false)
                        .dev_tools(false)
                        .default_script_dialogs(true),
                )
                .build(),
        )
        .manage(AppState::new())
        .setup(|app| {
            hooks::start_signal_watcher(app.handle().clone());
            // 启动自动重注入：上次关闭时已恢复 statusline 桥接（备份保留），
            // 检测备份 + 当前为原配置 → 重新注入（失败仅 warn，不阻断启动）
            hooks::reinject_statusline_on_startup();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_windows_build_number,
            state::set_project_root,
            pty::spawn::pty_spawn,
            pty::spawn::pty_write,
            pty::spawn::pty_resize,
            pty::spawn::pty_kill,
            pty::spawn::pty_kill_all,
            fs::fs_read_file,
            fs::fs_write_file,
            fs::fs_read_dir,
            fs::fs_create_dir,
            fs::fs_delete,
            fs::fs_rename,
            settings::save_settings,
            settings::load_settings,
            projects::save_projects,
            projects::load_projects,
            git::git_status,
            git::git_diff,
            git::git_file_at_head,
            git::git_rollback,
            git::git_unstage,
            notify::notify_watch,
            notify::notify_stop_watch,
            hooks::agent_hooks_inject,
            hooks::agent_hooks_uninstall,
            hooks::agent_hooks_injection_status,
            hooks::agent_hooks_restore_statusline,
            hooks::agent_hooks_config_read,
            hooks::agent_hooks_config_write,
            agent_history::agent_history_scan,
            agent_history::agent_history_delete,
            agent_history::agent_history_read_title,
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

    /// crash.log 写入成功：tempdir 目录参数化后文件存在且含消息（TQ-COV-01）
    #[test]
    fn write_crash_log_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        write_crash_log(dir.path(), "PANIC: test").unwrap();
        let content = std::fs::read_to_string(dir.path().join("crash.log")).unwrap();
        assert!(
            content.contains("PANIC: test"),
            "crash.log 应含消息，实际: {content}"
        );
    }

    /// 不可写目录（不存在的嵌套路径）→ 返回 Err 且不 panic（TQ-COV-01）
    #[test]
    fn write_crash_log_err_on_unwritable_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no_such_dir").join("deeper");
        assert!(
            write_crash_log(&missing, "PANIC: test").is_err(),
            "不存在的目录下创建文件应返回 Err"
        );
    }

    #[test]
    fn test_ping_returns_pong() {
        let result = ping();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "pong".to_string());
    }

    #[test]
    fn test_get_windows_build_number_returns_number() {
        let result = crate::pty::win_build::get_windows_build_number();
        // 运行时平台分支（BE-17/D5）：两分支均无平台专属 API，编译期同时可见
        if cfg!(windows) {
            assert!(result.is_ok(), "Windows 上应返回 build 号");
            let build = result.unwrap();
            // Windows 10 最低 build 号为 10240
            assert!(build > 10000, "build 号应大于 10000，实际: {build}");
        } else {
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
