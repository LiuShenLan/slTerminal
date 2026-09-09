fn main() {
    // TQ-COV-06：测试二进制嵌入 comctl32 v6 manifest（SxS 激活）。
    // 链接 tauri（tao/muda 菜单代码）的测试二进制静态导入 comctl32 v6 符号
    // （TaskDialogIndirect 等），无 manifest 激活时系统 comctl32.dll 仅 v5 导出
    // → 启动即 0xc0000139（STATUS_ENTRYPOINT_NOT_FOUND）；主应用由 tauri 生成
    // manifest 激活 v6 故正常。rustc-link-arg-tests 仅作用于测试目标，
    // 不影响 bin/lib 构建。
    // 图标变更触发重嵌入：icons/ 由 tauri icon 全量覆盖（含 icon.ico，tauri-build
    // 经 tauri-winres 嵌入 exe 资源）。cargo 默认不追踪 icons/ 变化，无此声明时
    // 改图标后增量构建不重跑本脚本 → exe 残留旧图标（2026-09-06 实证）。
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        concat!(env!("CARGO_MANIFEST_DIR"), "\\tests-comctl6.manifest")
    );

    // SEC-07：自定义命令白名单化——为每条命令生成 allow-<cmd> 权限，
    // capabilities/default.json 据此逐条 allow（未列出的命令将被拒绝调用）。
    // 清单须与 lib.rs 的 generate_handler! 注册保持一致（当前 43 条）。
    let manifest = tauri_build::AppManifest::new().commands(&[
        "ping",
        "get_windows_build_number",
        "set_project_root",
        "pty_spawn",
        "pty_write",
        "pty_resize",
        "pty_kill",
        "pty_kill_all",
        "pty_conpty_status",
        "fs_read_file",
        "fs_read_file_range",
        "fs_read_resource",
        "fs_write_file",
        "fs_read_dir",
        "fs_stat",
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
        "agent_hooks_confirm_inject",
        "agent_hooks_uninstall",
        "agent_hooks_injection_status",
        "agent_hooks_restore_statusline",
        "agent_hooks_config_read",
        "agent_hooks_config_write",
        "agent_history_scan",
        "agent_history_delete",
        "agent_history_read_title",
        "background_tasks_list",
        "background_tasks_set_config",
        "get_plan_balance",
        "refresh_plan_balance",
        "preview_sync",
        "preview_close",
        "preview_render",
        "preview_pull",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build 失败");
}
