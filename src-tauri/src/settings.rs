/// 设置持久化模块 — save/load settings 到 ~/.slterminal/settings.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。
use crate::error::AppError;
use std::io::Write as _;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// 从可执行文件路径解析应用数据目录（纯函数，注入可失败点供 SPE-04 两错误分支测试）
pub(crate) fn resolve_app_data_dir(
    exe: Result<PathBuf, std::io::Error>,
) -> Result<PathBuf, AppError> {
    let exe = exe.map_err(|e| AppError::IoKind {
        kind: "exe_dir".into(),
        message: format!("无法获取可执行文件路径: {e}"),
    })?;
    let exe_dir = exe.parent().ok_or_else(|| AppError::IoKind {
        kind: "exe_dir".into(),
        message: "无法获取可执行文件所在目录".into(),
    })?;
    Ok(exe_dir.to_path_buf())
}

/// 测试用：app_data_dir() 覆盖注入槽（仅测试编译，生产零行为变更）
#[cfg(test)]
static APP_DATA_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// 测试用 RAII 守卫：把 app_data_dir() 指向指定目录，Drop 时恢复原值
/// （防测试 panic 残留覆盖污染后续用例；projects 模块命令层测试复用）
#[cfg(test)]
pub(crate) struct AppDataDirGuard(Option<PathBuf>);

#[cfg(test)]
impl AppDataDirGuard {
    pub(crate) fn set(dir: &std::path::Path) -> Self {
        let mut slot = APP_DATA_DIR_OVERRIDE.lock().unwrap();
        let prev = slot.clone();
        *slot = Some(dir.to_path_buf());
        AppDataDirGuard(prev)
    }
}

#[cfg(test)]
impl Drop for AppDataDirGuard {
    fn drop(&mut self) {
        *APP_DATA_DIR_OVERRIDE.lock().unwrap() = self.0.clone();
    }
}

/// 获取应用数据目录（exe 同级目录，适配便携分发）
pub(crate) fn app_data_dir() -> Result<PathBuf, AppError> {
    // 测试注入覆盖（仅测试编译，生产恒走 current_exe 路径）
    #[cfg(test)]
    {
        if let Some(dir) = APP_DATA_DIR_OVERRIDE.lock().unwrap().clone() {
            return Ok(dir);
        }
    }
    resolve_app_data_dir(std::env::current_exe())
}

/// 浅合并：incoming 的 top-level 键覆盖 existing。
/// 两者均为 JSON 对象时逐键合并；否则 incoming 整体胜出（兼容缺失/损坏/非对象设置）。
fn merge_settings(existing: serde_json::Value, incoming: serde_json::Value) -> serde_json::Value {
    match (existing, incoming) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(inc)) => {
            for (k, v) in inc {
                base.insert(k, v);
            }
            serde_json::Value::Object(base)
        }
        // existing 非对象（缺失/损坏 → Null）或 incoming 非对象：用 incoming 整体
        (_, incoming) => incoming,
    }
}

/// 持久化设置（读现有 → 浅合并 top-level 键 → 原子写入：tempfile → flush → persist，.bak 备份兜底）
#[tauri::command]
pub async fn save_settings(settings: serde_json::Value) -> Result<(), AppError> {
    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    match tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // BE-05: create_dir_all 移入 spawn_blocking 闭包内部，避免异步上下文阻塞 I/O
        std::fs::create_dir_all(&app_dir)?;
        // 读现有 settings.json（不存在/损坏视作 Null），与 incoming 浅合并
        let existing = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .unwrap_or(serde_json::Value::Null);
        let merged = merge_settings(existing, settings);

        let json = serde_json::to_string_pretty(&merged)?;
        let mut tmp = NamedTempFile::new_in(&app_dir)?;
        tmp.write_all(json.as_bytes())?;
        tmp.flush()?;
        if settings_path.exists() {
            let bak = app_dir.join("settings.json.bak");
            if let Err(e) = std::fs::copy(&settings_path, &bak) {
                tracing::warn!("settings .bak 备份失败: {}", e);
            }
        }
        tmp.persist(&settings_path).map_err(|e| AppError::IoKind {
            kind: format!("{:?}", e.error.kind()),
            message: format!("persist 失败: {e}"),
        })?;
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
    Ok(())
}

/// 加载持久化设置，失败从 .bak 恢复，仍失败返回 Null
#[tauri::command]
pub async fn load_settings() -> Result<serde_json::Value, AppError> {
    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    let app_dir_clone = app_dir.clone();
    let result =
        match tokio::task::spawn_blocking(move || -> Result<serde_json::Value, AppError> {
            match std::fs::read_to_string(&settings_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(v) => Ok(v),
                    Err(_) => {
                        let bak = app_dir_clone.join("settings.json.bak");
                        if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                            if let Ok(v) = serde_json::from_str(&bak_content) {
                                let _ = std::fs::write(&settings_path, &bak_content);
                                return Ok(v);
                            }
                        }
                        Ok(serde_json::Value::Null)
                    }
                },
                Err(_) => Ok(serde_json::Value::Null),
            }
        })
        .await
        {
            Ok(inner) => inner,
            Err(e) => Err(AppError::TaskJoin(e.to_string())),
        }?;
    Ok(result)
}

/// 验证设置加载/保存逻辑
#[cfg(test)]
mod tests {
    use super::*;

    /// 创建 tokio runtime 并 block_on 调真实 Tauri 命令（与 fs 模块命令层测试同模式）
    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    // ── SPE-01: 真实命令层测试（app_data_dir 注入 tempdir） ──
    // 旧版测试 inline 重写 save/load 逻辑，从未调用真实 save_settings/load_settings 命令——
    // .bak 备份恢复、原子写入、浅合并、spawn_blocking 全部被虚构，命令路径架空。
    // 现改为 AppDataDirGuard 注入 tempdir + block_on 调真实命令（D2 最小可测性重构）。
    // 注：TaskJoin 分支（spawn_blocking panic）无法经命令注入构造，由 error.rs 的
    // From<JoinError> 用例（SPE-03）在错误类型层覆盖。

    /// 完整 save → load 往返（真实命令）
    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        let settings: serde_json::Value = serde_json::json!({
            "theme": "jetbrains-dark",
            "fontSize": 14
        });

        run(save_settings(settings.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();

        assert_eq!(loaded, settings);
    }

    /// 文件不存在时 load_settings 返回 Null
    #[test]
    fn load_missing_file_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let value = run(load_settings()).unwrap();
        assert!(value.is_null(), "文件不存在时 load_settings 应返回 Null");
    }

    /// JSON 损坏时从 .bak 恢复（真实命令：损坏主文件 → 恢复 .bak 内容并修复主文件）
    #[test]
    fn load_corrupt_json_recovers_from_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid_json = r#"{"theme":"dark","fontSize":14}"#;

        // 写入 bak 与损坏的 settings
        std::fs::write(dir.path().join("settings.json.bak"), valid_json).unwrap();
        std::fs::write(dir.path().join("settings.json"), "not valid json {{{broken").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();

        assert_eq!(value, serde_json::json!({"theme": "dark", "fontSize": 14}));
        // 验证 settings.json 已被 .bak 修复
        let repaired: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            repaired,
            serde_json::json!({"theme": "dark", "fontSize": 14})
        );
    }

    /// JSON 损坏且无 .bak 时返回 Null
    #[test]
    fn load_corrupt_json_no_bak_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "definitely not json {{{").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.is_null(), "无 .bak 时应返回 Null");
    }

    /// .bak 也损坏时仍返回 Null
    #[test]
    fn load_corrupt_both_json_and_bak_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "not json {{{").unwrap();
        std::fs::write(dir.path().join("settings.json.bak"), "also corrupt ###").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.is_null(), ".bak 也损坏时应返回 Null");
    }

    /// 空文件视为损坏 → 无 .bak → Null
    #[test]
    fn load_empty_file_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.is_null(), "空文件非合法 JSON，应返回 Null");
    }

    /// 多次 save 不擦除其他段（浅合并经真实命令验证）
    #[test]
    fn save_preserves_other_sections() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        // 第一次 save：写入 fontSize
        run(save_settings(serde_json::json!({"fontSize": 14}))).unwrap();
        // 第二次 save：只写 keybindings（真实命令内部读现有 → 浅合并 → 写回）
        run(save_settings(
            serde_json::json!({"keybindings": {"terminal.copy": "Ctrl+Alt+KeyC"}}),
        ))
        .unwrap();

        let reloaded = run(load_settings()).unwrap();
        assert_eq!(
            reloaded["fontSize"], 14,
            "fontSize 不应被 keybindings 覆盖擦除"
        );
        assert_eq!(
            reloaded["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC",
            "keybindings 应正确写入"
        );
    }

    /// 三次增量 save 验证所有段均保留（fontSize → keybindings → editorFontSize）
    #[test]
    fn three_save_cycles_preserve_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        run(save_settings(serde_json::json!({"fontSize": 14}))).unwrap();
        run(save_settings(
            serde_json::json!({"keybindings": {"terminal.copy": "Ctrl+Shift+C"}}),
        ))
        .unwrap();
        run(save_settings(serde_json::json!({"editorFontSize": 16}))).unwrap();

        let final_value = run(load_settings()).unwrap();
        assert_eq!(
            final_value["fontSize"], 14,
            "第一次 save 的 fontSize 应保留"
        );
        assert_eq!(
            final_value["keybindings"]["terminal.copy"], "Ctrl+Shift+C",
            "第二次 save 的 keybindings 应保留"
        );
        assert_eq!(
            final_value["editorFontSize"], 16,
            "第三次 save 的 editorFontSize 应保留"
        );
    }

    /// 覆盖写入：同名键被后续 save 覆盖
    #[test]
    fn save_overwrites_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        run(save_settings(serde_json::json!({"fontSize": 14}))).unwrap();
        run(save_settings(serde_json::json!({"fontSize": 18}))).unwrap();

        let reloaded = run(load_settings()).unwrap();
        assert_eq!(reloaded["fontSize"], 18, "fontSize 应被覆盖为新值");
    }

    /// create_dir_all：目录不存在时 save 自动创建（BE-05 确保此 I/O 在 spawn_blocking 内）
    #[test]
    fn save_creates_missing_directory() {
        // app_data_dir 注入尚不存在的嵌套目录 → 真实命令应自动创建
        let base = tempfile::tempdir().unwrap();
        let sub_dir = base.path().join("config").join("slterminal");
        let _guard = AppDataDirGuard::set(&sub_dir);

        run(save_settings(serde_json::json!({"fontSize": 14}))).unwrap();
        assert!(
            sub_dir.join("settings.json").exists(),
            "settings.json 应在自动创建的目录中存在"
        );
    }

    /// 原子写：多次 save 后目录内仅 settings.json + .bak，无临时文件残留
    #[test]
    fn save_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        run(save_settings(serde_json::json!({"a": 1}))).unwrap();
        run(save_settings(serde_json::json!({"b": 2}))).unwrap();

        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["settings.json", "settings.json.bak"],
            "persist 为原子 rename，不应残留临时文件"
        );
    }

    // ── SPE-05: persist 失败映射 ──

    /// persist 目标为已存在目录（冲突构造）→ 映射为 AppError::IoKind（消息含 "persist 失败"）
    #[test]
    fn save_persist_failure_maps_to_io_error() {
        let dir = tempfile::tempdir().unwrap();
        // 目标路径 settings.json 为已存在目录 → rename 替换必然失败
        std::fs::create_dir(dir.path().join("settings.json")).unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = run(save_settings(serde_json::json!({"a": 1}))).unwrap_err();
        match err {
            AppError::IoKind { kind, message } => {
                assert!(!kind.is_empty(), "kind 不应为空");
                assert!(
                    message.contains("persist 失败"),
                    "消息应含 'persist 失败'，实际: {message}"
                );
            }
            other => panic!("persist 失败应映射为 AppError::IoKind，实际: {other:?}"),
        }
    }

    // ── SPE-06: 边界用例（可行范围内） ──

    /// 并发写场景下 save_settings 偶发 persist 失败的容忍重试辅助（仅测试用）。
    /// Windows 上两线程对同一 settings.json 路径并发 rename/copy 时，另一线程的
    /// 瞬时句柄占用（或杀软扫描）会导致 persist 偶发 PermissionDenied（os error 5）
    /// ——对 Err 做有限重试（5 次 × 50ms）后仍失败才返回最后一个错误（真失败暴露）。
    /// 生产代码不重试（SPE-05 已覆盖 persist 失败映射，由前端层处理）。
    fn run_save_with_retry(settings: serde_json::Value) -> Result<(), AppError> {
        let mut last_err: Option<AppError> = None;
        for _ in 0..5 {
            match run(save_settings(settings.clone())) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
            // 短暂等待，让另一线程的 rename/copy 瞬时句柄占用释放后重试
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err(last_err.expect("重试循环至少执行一次"))
    }

    /// 并发写：两线程并发 save 同一目录，最终文件恒为合法 JSON 且值为二者之一（原子写不撕裂）
    /// 覆盖注入槽由主线程持有（工作线程只读不改）——避免线程内守卫 Drop 恢复清空全局槽
    /// 导致 save 落到真实 exe 目录（隔离纪律）；persist 偶发冲突经 run_save_with_retry 容忍
    #[test]
    fn concurrent_saves_never_torn() {
        let dir = tempfile::tempdir().unwrap();

        // 守卫由主线程持有——两工作线程全程读取同一覆盖值，无跨线程 Drop 恢复竞态
        let _guard = AppDataDirGuard::set(dir.path());

        // 两个独立线程各持 runtime 并发 block_on 真实 save_settings（read-merge-write 竞态）
        let h1 = std::thread::spawn(move || {
            run_save_with_retry(serde_json::json!({"key": "a"}))
        });
        let h2 = std::thread::spawn(move || {
            run_save_with_retry(serde_json::json!({"key": "b"}))
        });
        h1.join().unwrap().unwrap();
        h2.join().unwrap().unwrap();

        // 原子 persist 保证最终文件是完整文档（非撕裂），值为两者之一
        let content = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let final_value: serde_json::Value = serde_json::from_str(&content).unwrap();
        let v = final_value["key"].as_str().unwrap();
        assert!(
            v == "a" || v == "b",
            "并发写后文件应为两者之一，实际: {final_value}"
        );
    }

    /// 只读 settings.json：Windows 上 MoveFileExW 替换只读目标失败 → persist 错误映射为 IoKind；
    /// Unix 上 rename 不校验目标只读位 → 保存成功（浅合并生效）
    #[test]
    fn save_over_readonly_file() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, r#"{"fontSize":14}"#).unwrap();
        let mut perms = std::fs::metadata(&settings_path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&settings_path, perms.clone()).unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let result = run(save_settings(serde_json::json!({"keybindings": {}})));

        #[cfg(windows)]
        {
            let err = result.expect_err("Windows 上替换只读目标应失败");
            match err {
                AppError::IoKind { kind, message } => {
                    assert!(!kind.is_empty());
                    assert!(
                        message.contains("persist 失败"),
                        "消息应含 'persist 失败'，实际: {message}"
                    );
                }
                other => panic!("persist 失败应映射为 AppError::IoKind，实际: {other:?}"),
            }
            // 恢复可写位，确保 tempdir 清理能删除文件
            perms.set_readonly(false);
            std::fs::set_permissions(&settings_path, perms).unwrap();
        }
        #[cfg(not(windows))]
        {
            result.unwrap();
            let loaded: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
            assert_eq!(loaded["fontSize"], 14, "浅合并应保留只读文件原内容");
            assert_eq!(loaded["keybindings"], serde_json::json!({}));
        }
    }

    /// 超大 JSON（2MB 字符串）save/load 往返一致
    #[test]
    fn save_and_load_large_json() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let big = serde_json::json!({ "blob": "x".repeat(2 * 1024 * 1024) }); // 2MB 字符串
        run(save_settings(big.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();
        assert_eq!(loaded, big);
    }

    // ── merge_settings 浅合并（纯函数，直接断言） ──

    /// 合并保留 existing 中 incoming 未涉及的 top-level 键
    #[test]
    fn test_merge_preserves_foreign_keys() {
        let existing = serde_json::json!({ "terminalFontSize": 14, "editorFontSize": 16 });
        let incoming = serde_json::json!({ "keybindings": { "terminal.copy": "Ctrl+Alt+KeyC" } });
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged["terminalFontSize"], 14);
        assert_eq!(merged["editorFontSize"], 16);
        assert_eq!(merged["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC");
    }

    /// 合并时 incoming 覆盖 existing 的同名键
    #[test]
    fn test_merge_overwrites_same_key() {
        let existing = serde_json::json!({ "a": 1, "b": 2 });
        let incoming = serde_json::json!({ "a": 99 });
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged["a"], 99);
        assert_eq!(merged["b"], 2);
    }

    /// existing 为 Null（文件缺失/损坏）时用 incoming 初始化
    #[test]
    fn test_merge_null_existing_initializes_with_incoming() {
        let merged = merge_settings(
            serde_json::Value::Null,
            serde_json::json!({ "keybindings": {} }),
        );
        assert_eq!(merged, serde_json::json!({ "keybindings": {} }));
    }

    /// incoming 非对象时整体替换（极端兜底）
    #[test]
    fn test_merge_non_object_incoming_replaces() {
        let existing = serde_json::json!({ "a": 1 });
        let incoming = serde_json::json!("scalar");
        let merged = merge_settings(existing, incoming);
        assert_eq!(merged, serde_json::json!("scalar"));
    }

    /// 嵌套 JSON 对象的浅合并保留未涉及键
    #[test]
    fn te14_merge_preserves_nested_keys() {
        let existing = serde_json::json!({
            "fontSize": 14,
            "keybindings": {
                "terminal.copy": "Ctrl+Shift+C",
                "terminal.paste": "Ctrl+Shift+V"
            }
        });
        // 只更新 keybindings 中的一个键
        let incoming = serde_json::json!({
            "keybindings": {
                "terminal.copy": "Ctrl+Alt+KeyC"
            }
        });
        let merged = merge_settings(existing, incoming);

        // 注意：merge_settings 是 top-level 浅合并——keybindings 整个键被替换
        assert_eq!(merged["fontSize"], 14);
        assert_eq!(merged["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC");
        // top-level 浅合并意味着 keybindings 整体替换，terminal.paste 会丢失
        // 此测试锁死当前行为——如果未来改为深度合并需更新
        assert!(
            merged["keybindings"]["terminal.paste"].is_null(),
            "top-level 浅合并：keybindings 整体替换，子键不保留"
        );
    }

    // ── SPE-04: app_data_dir() 错误分支（路径解析纯函数注入可失败点） ──

    /// current_exe 失败 → IoKind("exe_dir")，消息含"无法获取可执行文件路径"
    #[test]
    fn resolve_app_data_dir_current_exe_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "模拟 current_exe 失败");
        let result = resolve_app_data_dir(Err(io_err));
        match result {
            Err(AppError::IoKind { kind, message }) => {
                assert_eq!(kind, "exe_dir");
                assert!(
                    message.contains("无法获取可执行文件路径"),
                    "消息应含错误提示，实际: {message}"
                );
            }
            other => panic!("current_exe 失败应映射为 IoKind，实际: {other:?}"),
        }
    }

    /// exe 无父目录（根路径）→ IoKind("exe_dir")，消息含"无法获取可执行文件所在目录"
    #[test]
    fn resolve_app_data_dir_exe_no_parent() {
        // 根路径的 parent() 为 None（Windows "C:\\" / Unix "/" 均无父目录）
        let root_path = if cfg!(windows) {
            std::path::PathBuf::from("C:\\")
        } else {
            std::path::PathBuf::from("/")
        };
        let result = resolve_app_data_dir(Ok(root_path));
        match result {
            Err(AppError::IoKind { kind, message }) => {
                assert_eq!(kind, "exe_dir");
                assert!(
                    message.contains("无法获取可执行文件所在目录"),
                    "消息应含错误提示，实际: {message}"
                );
            }
            other => panic!("exe 无父目录应映射为 IoKind，实际: {other:?}"),
        }
    }

    /// 正常路径：exe 位于目录下 → 返回该目录
    #[test]
    fn resolve_app_data_dir_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_app_data_dir(Ok(dir.path().join("slterminal.exe"))).unwrap();
        assert_eq!(result, dir.path(), "应返回 exe 所在目录");
    }

    // ── 依赖真实 current_exe 的测试（SPE-06 ②） ──
    // 说明：以下两测试依赖真实 std::env::current_exe()（测试进程的可执行文件路径），
    // 未注入覆盖——验证的是生产 app_data_dir() 的便携分发语义（exe 同级目录）。
    // 测试进程必在运行，current_exe() 恒成功，故仅断言父目录关系与路径拼接。

    /// T2.1: app_data_dir 返回 current_exe 的父目录
    #[test]
    fn app_data_dir_returns_exe_parent() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let exe = std::env::current_exe().expect("current_exe 不应失败");
        let exe_dir = exe.parent().expect("exe 应有父目录");
        assert_eq!(app_dir, exe_dir, "app_data_dir 应返回 exe 所在目录");
    }

    /// T2.2: app_data_dir + settings.json 路径拼接
    #[test]
    fn app_data_dir_joins_settings_path() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let settings_path = app_dir.join("settings.json");
        assert!(
            settings_path.ends_with("settings.json"),
            "应指向 settings.json"
        );
        // 验证父目录存在（测试运行时 exe 目录必然存在）
        assert!(app_dir.exists(), "app_data_dir 返回的目录应存在");
    }
}
