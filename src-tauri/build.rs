fn main() {
    // SEC-07：自定义命令白名单化——为每条命令生成 allow-<cmd> 权限，
    // capabilities/default.json 据此逐条 allow（未列出的命令将被拒绝调用）。
    // 清单须与 lib.rs 的 generate_handler! 注册保持一致（当前 34 条）。
    let manifest = tauri_build::AppManifest::new().commands(&[
        "ping",
        "get_windows_build_number",
        "set_project_root",
        "pty_spawn",
        "pty_write",
        "pty_resize",
        "pty_kill",
        "pty_kill_all",
        "fs_read_file",
        "fs_write_file",
        "fs_read_dir",
        "fs_create_dir",
        "fs_delete",
        "fs_rename",
        "save_settings",
        "load_settings",
        "save_projects",
        "load_projects",
        "git_status",
        "git_diff",
        "git_file_at_head",
        "git_rollback",
        "git_unstage",
        "notify_watch",
        "notify_stop_watch",
        "agent_hooks_inject",
        "agent_hooks_uninstall",
        "agent_hooks_injection_status",
        "agent_hooks_restore_statusline",
        "agent_hooks_config_read",
        "agent_hooks_config_write",
        "agent_history_scan",
        "agent_history_delete",
        "agent_history_read_title",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build 失败");
}
