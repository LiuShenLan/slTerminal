/// 设置持久化模块 — save/load settings 到 ~/.slterminal/settings.json
///
/// 原子写入（tempfile）+ .bak 备份兜底。
/// - BE-14/D11：load 返回 `{ data, corrupted }`——无文件 data:null/corrupted:false；
///   损坏回退默认值 corrupted:true；.bak 命中也算 corrupted:true（数据来自备份）。
/// - SEC-11：save 校验顶层键白名单（fontSize/keybindings/sideBar/colorScheme）+ 大小上限 1MB。
/// - BE-16：应用数据目录解析/测试守卫/共享 DTO 自 app_dir 模块导入。
use crate::app_dir::{app_data_dir, LoadResult, MAX_PERSIST_BYTES};
use crate::error::{io_error, AppError};
use std::io::Write as _;
use tempfile::NamedTempFile;

/// 设置顶层键白名单（SEC-11）：前端各 store 只允许写这些键
/// （fontSize/keybindings/sideBar/colorScheme——与 stores 模块持久化键一一对应；
/// planBalance——F10 轮询间隔，手改文件，读取侧在 plan_balance 模块。
/// 契约断链先例：fontSize store 曾发平铺 terminalFontSize/editorFontSize 顶层键被拒，
/// 已改段形态并用双侧测试锁死——前端 payload 键集合精确断言 + 后端平铺拒绝用例）
const SETTINGS_ALLOWED_KEYS: [&str; 5] = [
    "fontSize",
    "keybindings",
    "sideBar",
    "colorScheme",
    "planBalance",
];

/// save_settings 进程内互斥（SPE-06 场景转正修复）：
/// 前端三 store（fontSize/keybindings/sideBar）启动时几乎同时各触发一次 debounced 保存，
/// 并发对同一 settings.json 读-合并-写（NamedTempFile persist rename + .bak copy）时，
/// Windows 上另一线程的瞬时句柄占用会导致 persist 偶发 PermissionDenied（os error 5）——
/// 持锁串行化「读-合并-写」全程，并发冲突消除（杀软实时扫描窗口为残余偶发源）。
/// 持锁临界区均为无 panic 路径（读/合并/serde/写），中毒不可达；map_err 兜底防御。
static SETTINGS_SAVE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// SEC-11：校验设置保存输入——须为 JSON 对象且顶层键 ∈ 白名单
fn validate_settings_input(settings: &serde_json::Value) -> Result<(), AppError> {
    let obj = settings
        .as_object()
        .ok_or_else(|| AppError::Validation("保存设置失败: 设置必须是 JSON 对象".into()))?;
    for key in obj.keys() {
        if !SETTINGS_ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(AppError::Validation(format!(
                "保存设置失败: 顶层键不在白名单内: {key}"
            )));
        }
    }
    Ok(())
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
///
/// SEC-11 保存校验：顶层键白名单（spawn_blocking 前快速失败）+ 序列化后大小上限 1MB。
#[tauri::command]
pub async fn save_settings(settings: serde_json::Value) -> Result<(), AppError> {
    // SEC-11：顶层键白名单校验（纯内存操作，spawn_blocking 前快速失败）
    validate_settings_input(&settings)?;

    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    match tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        // SETTINGS_SAVE_LOCK：串行化并发保存（见锁定义注释——三 store 启动并发写竞态）
        let _guard = SETTINGS_SAVE_LOCK
            .lock()
            .map_err(|_| AppError::Unknown("settings 保存锁中毒".into()))?;
        // BE-05: create_dir_all 移入 spawn_blocking 闭包内部，避免异步上下文阻塞 I/O
        // 保存链 io 错误统一经 io_error 语义化（BE-15）：用户可见「保存设置失败 + 路径」，
        // 原始 io 错误文本进 tracing 日志
        std::fs::create_dir_all(&app_dir).map_err(|e| io_error("保存设置", &app_dir, e))?;
        // 读现有 settings.json（不存在/损坏视作 Null），与 incoming 浅合并
        let existing = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .unwrap_or(serde_json::Value::Null);
        let merged = merge_settings(existing, settings);

        let json = serde_json::to_string_pretty(&merged)?;
        // SEC-11：序列化后大小上限（含合并进现有文件的部分）
        if json.len() > MAX_PERSIST_BYTES {
            return Err(AppError::Validation("保存设置失败: 数据超过 1MB 上限".into()));
        }
        let mut tmp =
            NamedTempFile::new_in(&app_dir).map_err(|e| io_error("保存设置", &app_dir, e))?;
        tmp.write_all(json.as_bytes())
            .map_err(|e| io_error("保存设置", &settings_path, e))?;
        tmp.flush().map_err(|e| io_error("保存设置", &settings_path, e))?;
        if settings_path.exists() {
            let bak = app_dir.join("settings.json.bak");
            if let Err(e) = std::fs::copy(&settings_path, &bak) {
                tracing::warn!(error = %e, path = %settings_path.display(), "settings .bak 备份失败");
            }
        }
        tmp.persist(&settings_path)
            .map_err(|e| io_error("保存设置", &settings_path, e.error))?;
        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }?;
    Ok(())
}

/// 加载持久化设置（BE-14/D11）：返回 `{ data, corrupted }`
///
/// - 无文件 → data:null, corrupted:false
/// - 主文件损坏 → 尝试 .bak：命中 → data:备份内容, corrupted:true；未命中 → data:null, corrupted:true
/// - 主文件合法 → data:内容, corrupted:false
#[tauri::command]
pub async fn load_settings() -> Result<LoadResult<Option<serde_json::Value>>, AppError> {
    let app_dir = app_data_dir()?;
    let settings_path = app_dir.join("settings.json");

    let app_dir_clone = app_dir.clone();
    let result = match tokio::task::spawn_blocking(
        move || -> Result<LoadResult<Option<serde_json::Value>>, AppError> {
            match std::fs::read_to_string(&settings_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(v) => Ok(LoadResult {
                        data: Some(v),
                        corrupted: false,
                    }),
                    Err(_) => {
                        let bak = app_dir_clone.join("settings.json.bak");
                        if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                            if let Ok(v) = serde_json::from_str(&bak_content) {
                                let _ = std::fs::write(&settings_path, &bak_content);
                                // BE-14：bak 命中也算 corrupted（数据来自备份）
                                return Ok(LoadResult {
                                    data: Some(v),
                                    corrupted: true,
                                });
                            }
                        }
                        Ok(LoadResult {
                            data: None,
                            corrupted: true,
                        })
                    }
                },
                Err(_) => Ok(LoadResult {
                    data: None,
                    corrupted: false,
                }),
            }
        },
    )
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
    use crate::app_dir::AppDataDirGuard;

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

    /// 完整 save → load 往返（真实命令）：data 一致且 corrupted=false
    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        let settings: serde_json::Value = serde_json::json!({
            "colorScheme": "jetbrains-dark",
            "fontSize": 14
        });

        run(save_settings(settings.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();

        assert_eq!(loaded.data, Some(settings), "加载数据应与保存一致");
        assert!(!loaded.corrupted, "正常保存后加载不应标记 corrupted");
    }

    /// BE-14 corrupted 三态①：文件不存在 → data:null / corrupted:false
    #[test]
    fn load_missing_file_returns_null_not_corrupted() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let value = run(load_settings()).unwrap();
        assert!(value.data.is_none(), "文件不存在时 data 应为 null");
        assert!(!value.corrupted, "文件不存在不算损坏");
    }

    /// BE-14 corrupted 三态②：JSON 损坏从 .bak 恢复 → data=备份内容 / corrupted:true
    #[test]
    fn load_corrupt_json_recovers_from_bak() {
        let dir = tempfile::tempdir().unwrap();
        let valid_json = r#"{"theme":"dark","fontSize":14}"#;

        // 写入 bak 与损坏的 settings
        std::fs::write(dir.path().join("settings.json.bak"), valid_json).unwrap();
        std::fs::write(dir.path().join("settings.json"), "not valid json {{{broken").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();

        assert_eq!(
            value.data,
            Some(serde_json::json!({"theme": "dark", "fontSize": 14}))
        );
        assert!(
            value.corrupted,
            ".bak 命中也应标记 corrupted（数据来自备份）"
        );
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

    /// BE-14 corrupted 三态③：JSON 损坏且无 .bak → data:null / corrupted:true
    #[test]
    fn load_corrupt_json_no_bak_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "definitely not json {{{").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.data.is_none(), "无 .bak 时 data 应为 null");
        assert!(value.corrupted, "损坏文件应标记 corrupted");
    }

    /// .bak 也损坏时 → data:null / corrupted:true
    #[test]
    fn load_corrupt_both_json_and_bak_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "not json {{{").unwrap();
        std::fs::write(dir.path().join("settings.json.bak"), "also corrupt ###").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.data.is_none(), ".bak 也损坏时 data 应为 null");
        assert!(value.corrupted, "损坏文件应标记 corrupted");
    }

    /// 空文件视为损坏 → 无 .bak → data:null / corrupted:true
    #[test]
    fn load_empty_file_returns_null() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "").unwrap();

        let _guard = AppDataDirGuard::set(dir.path());
        let value = run(load_settings()).unwrap();
        assert!(value.data.is_none(), "空文件非合法 JSON，data 应为 null");
        assert!(value.corrupted, "空文件应标记 corrupted");
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
        let data = reloaded.data.as_ref().unwrap();
        assert_eq!(data["fontSize"], 14, "fontSize 不应被 keybindings 覆盖擦除");
        assert_eq!(
            data["keybindings"]["terminal.copy"], "Ctrl+Alt+KeyC",
            "keybindings 应正确写入"
        );
    }

    /// 三次增量 save 验证所有段均保留（fontSize → keybindings → sideBar）
    #[test]
    fn three_save_cycles_preserve_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        run(save_settings(serde_json::json!({"fontSize": 14}))).unwrap();
        run(save_settings(
            serde_json::json!({"keybindings": {"terminal.copy": "Ctrl+Shift+C"}}),
        ))
        .unwrap();
        run(save_settings(
            serde_json::json!({"sideBar": {"width": 280}}),
        ))
        .unwrap();

        let final_value = run(load_settings()).unwrap();
        let data = final_value.data.as_ref().unwrap();
        assert_eq!(data["fontSize"], 14, "第一次 save 的 fontSize 应保留");
        assert_eq!(
            data["keybindings"]["terminal.copy"], "Ctrl+Shift+C",
            "第二次 save 的 keybindings 应保留"
        );
        assert_eq!(
            data["sideBar"]["width"], 280,
            "第三次 save 的 sideBar 应保留"
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
        assert_eq!(
            reloaded.data.unwrap()["fontSize"],
            18,
            "fontSize 应被覆盖为新值"
        );
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

        run(save_settings(serde_json::json!({"fontSize": 1}))).unwrap();
        run(save_settings(serde_json::json!({"keybindings": {}}))).unwrap();

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

    /// persist 目标为已存在目录（冲突构造）→ 映射为 AppError::IoKind（消息含「保存设置失败」+ 路径）
    #[test]
    fn save_persist_failure_maps_to_io_error() {
        let dir = tempfile::tempdir().unwrap();
        // 目标路径 settings.json 为已存在目录 → rename 替换必然失败
        let settings_path = dir.path().join("settings.json");
        std::fs::create_dir(&settings_path).unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = run(save_settings(serde_json::json!({"fontSize": 1}))).unwrap_err();
        match err {
            AppError::IoKind { kind, message } => {
                assert!(!kind.is_empty(), "kind 不应为空");
                assert!(
                    message.contains("保存设置失败"),
                    "消息应含业务语义 '保存设置失败'，实际: {message}"
                );
                assert!(
                    message.contains(&settings_path.to_string_lossy().to_string()),
                    "消息应含设置文件路径，实际: {message}"
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
    /// 【容忍度声明】本重试仅容忍 Windows 杀软/索引扫描占用文件的瞬时窗口，
    /// 不代替生产锁语义——生产 save_settings 无重试（单次 persist 失败即 Err）。
    /// 若 SETTINGS_SAVE_LOCK 被移除/失效，本测试可能假绿，锁语义由
    /// pty/模块并发用例与 code review 兜底。
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
    /// 导致 save 落到真实 exe 目录（隔离纪律）。
    /// SETTINGS_SAVE_LOCK 串行化后「读-合并-写」不再并发（SPE-06 并发 rename/copy 冲突已消除）；
    /// run_save_with_retry 保留为杀软实时扫描窗口的容忍（Defender 对刚写出的新文件短暂占用）。
    #[test]
    fn concurrent_saves_never_torn() {
        let dir = tempfile::tempdir().unwrap();

        // 守卫由主线程持有——两工作线程全程读取同一覆盖值，无跨线程 Drop 恢复竞态
        let _guard = AppDataDirGuard::set(dir.path());

        // 两个独立线程各持 runtime 并发 block_on 真实 save_settings（read-merge-write 竞态）
        let h1 =
            std::thread::spawn(move || run_save_with_retry(serde_json::json!({"fontSize": "a"})));
        let h2 =
            std::thread::spawn(move || run_save_with_retry(serde_json::json!({"fontSize": "b"})));
        h1.join().unwrap().unwrap();
        h2.join().unwrap().unwrap();

        // 原子 persist 保证最终文件是完整文档（非撕裂），值为两者之一
        let content = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let final_value: serde_json::Value = serde_json::from_str(&content).unwrap();
        let v = final_value["fontSize"].as_str().unwrap();
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

        // 运行时平台分支（BE-17/D5）：替换只读目标行为由平台实现决定，两分支无平台专属 API
        if cfg!(windows) {
            let err = result.expect_err("Windows 上替换只读目标应失败");
            match err {
                AppError::IoKind { kind, message } => {
                    assert!(!kind.is_empty());
                    assert!(
                        message.contains("保存设置失败"),
                        "消息应含业务语义 '保存设置失败'，实际: {message}"
                    );
                    assert!(
                        message.contains("settings.json"),
                        "消息应含设置文件路径，实际: {message}"
                    );
                }
                other => panic!("persist 失败应映射为 AppError::IoKind，实际: {other:?}"),
            }
            // 恢复可写位，确保 tempdir 清理能删除文件
            perms.set_readonly(false);
            std::fs::set_permissions(&settings_path, perms).unwrap();
        } else {
            result.unwrap();
            let loaded: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
            assert_eq!(loaded["fontSize"], 14, "浅合并应保留只读文件原内容");
            assert_eq!(loaded["keybindings"], serde_json::json!({}));
        }
    }

    // ── SEC-11: 保存校验（大小上限 + 顶层键白名单） ──

    /// 保存超 1MB → AppError::Validation 拒绝且不落盘
    #[test]
    fn save_rejects_over_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let big = serde_json::json!({ "fontSize": "x".repeat(MAX_PERSIST_BYTES + 1) });
        let err = run(save_settings(big)).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("1MB"), "消息应提示大小上限，实际: {msg}");
            }
            other => panic!("超限应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "超限保存不应落盘"
        );
    }

    /// 合法顶层键下的大数据（<1MB）save/load 往返一致
    #[test]
    fn save_load_large_json_under_limit() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let big = serde_json::json!({ "fontSize": "x".repeat(500 * 1024) }); // 500KB
        run(save_settings(big.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();
        assert_eq!(loaded.data, Some(big), "1MB 内大 JSON 应往返一致");
        assert!(!loaded.corrupted);
    }

    /// 契约断链防复发①：fontSize 段形态放行（fontSize store 合法 payload——段内
    /// terminalFontSize/editorFontSize，顶层键 = fontSize 段名 ∈ 白名单）→ save/load 往返一致
    #[test]
    fn save_accepts_font_size_section_shape() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let settings = serde_json::json!({
            "fontSize": { "terminalFontSize": 16, "editorFontSize": 12 }
        });
        run(save_settings(settings.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();
        assert_eq!(loaded.data, Some(settings), "fontSize 段应完整往返一致");
        assert!(!loaded.corrupted);
    }

    /// F10 白名单第 5 键：planBalance 段放行且 save/load 往返一致（防白名单回归）
    #[test]
    fn save_accepts_plan_balance_key() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let settings = serde_json::json!({ "planBalance": { "intervalSec": 120 } });
        run(save_settings(settings.clone())).unwrap();
        let loaded = run(load_settings()).unwrap();
        assert_eq!(loaded.data, Some(settings), "planBalance 段应完整往返一致");
        assert!(!loaded.corrupted);
    }

    /// 契约断链防复发②：平铺 terminalFontSize/editorFontSize 顶层键 → Validation 拒绝
    /// （fontSize store 曾发此平铺形态导致每次保存被拒、用户配置静默丢失——锁死错误形态防回归）
    #[test]
    fn save_rejects_flat_font_size_keys() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = run(save_settings(
            serde_json::json!({ "terminalFontSize": 16, "editorFontSize": 12 }),
        ))
        .unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("白名单"), "平铺键应提示白名单，实际: {msg}");
            }
            other => panic!("平铺 fontSize 键应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
    }

    /// 非法顶层键 → AppError::Validation 拒绝
    #[test]
    fn save_rejects_illegal_top_level_key() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = run(save_settings(serde_json::json!({"theme": "dark"}))).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("白名单"), "消息应提示白名单，实际: {msg}");
            }
            other => panic!("非法顶层键应返回 Validation，实际: {other:?}"),
        }
        // 混合合法/非法键同样拒绝（白名单是整体约束）
        let err2 = run(save_settings(
            serde_json::json!({"evil": 1, "fontSize": 14}),
        ))
        .unwrap_err();
        assert!(
            matches!(err2, AppError::Validation(_)),
            "含白名单外键应拒绝: {err2:?}"
        );
    }

    /// 非对象输入（标量/数组）→ AppError::Validation 拒绝
    #[test]
    fn save_rejects_non_object_input() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = run(save_settings(serde_json::json!("scalar"))).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("JSON 对象"), "消息应提示对象要求，实际: {msg}");
            }
            other => panic!("非对象输入应返回 Validation，实际: {other:?}"),
        }
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

    // 注：resolve_app_data_dir 错误分支（SPE-04）与 app_data_dir 便携语义测试
    // （T2.1/T2.2）已随 BE-16 上提至 app_dir.rs。
}
