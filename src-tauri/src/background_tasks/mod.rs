//! 后台定时任务骨架（F12）——任务注册表（registry.rs）+ poller 驱动 + 配置命令
//!
//! 轮询通用件自 plan_balance 上提：内存配置（enabled/intervalSec 原子量）/首轮立即
//! 执行/每轮末按当前内存间隔 sleep。套餐语义（resolve/fetch/merge/emit 口径）下沉
//! plan_balance 执行体 poll_once_executor，行为不变。
//! 配置单写通道：set_config = 校验 → 复用 settings.rs 写通道落盘（读-改-写子键合并，
//! 禁止自建第二写通道）→ 更新内存 → emit 配置变更事件 → 返回完整清单。

pub mod registry;

pub(crate) use registry::SETTINGS_KEY; // 再导出：settings.rs 白名单经 crate::background_tasks::SETTINGS_KEY 引用

use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Emitter;

use crate::error::AppError;
use registry::{TaskDef, TaskRuntime, RUNTIMES, TASKS};

// ── DTO（serde camelCase ↔ src/types/backgroundTasks.ts，硬约束 #4） ──

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTaskInfo {
    pub task_id: String,
    pub title: String,
    pub enabled: bool,
    pub interval_sec: u64,
    pub interval_min: u64,
    pub interval_max: u64,
}

fn current_list() -> Vec<BackgroundTaskInfo> {
    TASKS
        .iter()
        .zip(RUNTIMES.iter())
        .map(|(def, rt)| BackgroundTaskInfo {
            task_id: def.task_id.into(),
            title: def.title.into(),
            enabled: rt.enabled.load(Ordering::Relaxed),
            interval_sec: rt.interval_sec.load(Ordering::Relaxed),
            interval_min: def.interval_min,
            interval_max: def.interval_max,
        })
        .collect()
}

// ── poller 驱动 ──

/// 单任务轮询循环（enabled=true 才由调用方 spawn）：首轮立即执行（D8 语义保留），
/// 每轮末按当前内存间隔 sleep——set_config 改值下一轮即生效；轮首检查 enabled，
/// 运行期禁用 → 退出循环（running 置 false，快照保留，重启用由 set_config 重 spawn）
fn spawn_poller(app: tauri::AppHandle, def: &'static TaskDef, rt: &'static TaskRuntime) {
    let Some(executor) = def.executor else { return };
    rt.running.store(true, Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        loop {
            if !rt.enabled.load(Ordering::Relaxed) {
                break;
            }
            let handle = app.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || executor(handle)).await {
                tracing::warn!(task = def.task_id, error = %e, "后台任务执行体异常");
            }
            let secs = rt.interval_sec.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
        rt.running.store(false, Ordering::Relaxed);
    });
}

/// lib.rs setup 调用：逐任务读盘初始化内存配置（钳制回退）→ enabled 且有执行体才 spawn
pub fn start_background_tasks(app: tauri::AppHandle) {
    for (def, rt) in TASKS.iter().zip(RUNTIMES.iter()) {
        let cfg = registry::resolve_task_config(def);
        rt.enabled.store(cfg.enabled, Ordering::Relaxed);
        rt.interval_sec.store(cfg.interval_sec, Ordering::Relaxed);
        if cfg.enabled && def.executor.is_some() {
            spawn_poller(app.clone(), def, rt);
        }
    }
}

// ── 配置命令（三处注册：lib.rs + build.rs + capabilities） ──

/// set_config 读-改-写串行化（SETTINGS_SAVE_LOCK 先例）：并发 set_config 的
/// 读-改-写跨子键合并必须互斥，否则后写覆盖前写的其他子键。
/// 锁序单向：CONFIG_WRITE_LOCK → SETTINGS_SAVE_LOCK（save_settings_blocking 内部），无环。
static CONFIG_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// set_config 核心（同步，spawn_blocking 内执行；L1 直测）——顺序写死：
/// taskId 白名单校验 → 边界校验（越界 → Validation，磁盘/内存均不变）→
/// 读-改-写 backgroundTasks 段（子键合并，settings.rs 写通道落盘）→ 更新内存值。
/// 返回完整清单。spawn/emit 在命令包装层（需 AppHandle，L1 豁免登记）。
pub(crate) fn set_config_core(
    task_id: &str,
    enabled: Option<bool>,
    interval_sec: Option<u64>,
) -> Result<Vec<BackgroundTaskInfo>, AppError> {
    let (def, rt) = registry::find(task_id)
        .ok_or_else(|| AppError::Validation(format!("设置后台任务失败: 未知任务 {task_id}")))?;
    if enabled.is_none() && interval_sec.is_none() {
        return Err(AppError::Validation(
            "设置后台任务失败: 未提供任何配置项".into(),
        ));
    }
    if let Some(sec) = interval_sec {
        if !(def.interval_min..=def.interval_max).contains(&sec) {
            return Err(AppError::Validation(format!(
                "设置后台任务失败: {} 频率须为 {}–{} 秒，实际 {sec}",
                def.title, def.interval_min, def.interval_max
            )));
        }
    }
    let _guard = CONFIG_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::Unknown("后台任务配置锁中毒".into()))?;
    // 读现有 backgroundTasks 段（窗口 B，R2b）：读失败/解析失败 → Err 传播且不落盘
    // （旧 `.ok()` 吞错视作空段——兄弟子键丢失仍写成功）；仅文件不存在 → 空段
    // （首次写入合法）；段非对象视作空段（结构兜底，照旧语义）
    let settings_path = crate::app_dir::app_data_dir()?.join("settings.json");
    let root = crate::settings::read_existing_settings(&settings_path)?;
    let section = root
        .get(registry::SETTINGS_KEY)
        .cloned()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    // 子键合并：只改本任务子对象，其他任务子键原样保留
    let mut task_cfg = section
        .get(task_id)
        .cloned()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = task_cfg.as_object_mut().unwrap();
    if let Some(b) = enabled {
        obj.insert("enabled".into(), serde_json::json!(b));
    }
    if let Some(sec) = interval_sec {
        obj.insert("intervalSec".into(), serde_json::json!(sec));
    }
    let mut section = section;
    section
        .as_object_mut()
        .unwrap()
        .insert(task_id.to_string(), task_cfg);
    crate::settings::save_settings_blocking(serde_json::json!({
        registry::SETTINGS_KEY: section
    }))?;
    // 落盘成功才更新内存（磁盘/内存恒一致）
    if let Some(b) = enabled {
        rt.enabled.store(b, Ordering::Relaxed);
    }
    if let Some(sec) = interval_sec {
        rt.interval_sec.store(sec, Ordering::Relaxed);
    }
    Ok(current_list())
}

/// 全部任务元数据 + 当前生效配置（内存值）——设置页与前端调度器共用此读通道
#[tauri::command]
pub async fn background_tasks_list() -> Result<Vec<BackgroundTaskInfo>, AppError> {
    Ok(current_list())
}

/// 设置任务配置：core（校验→落盘→内存）→ enabled false→true 且执行体 Some 且循环
/// 未在跑 → 重新 spawn → emit background-tasks-updated（前端 footer/调度器感知通道）
/// → 返回完整清单
#[tauri::command]
pub async fn background_tasks_set_config(
    app_handle: tauri::AppHandle,
    task_id: String,
    enabled: Option<bool>,
    interval_sec: Option<u64>,
) -> Result<Vec<BackgroundTaskInfo>, AppError> {
    let id = task_id.clone();
    let list = tokio::task::spawn_blocking(move || set_config_core(&id, enabled, interval_sec))
        .await
        .map_err(AppError::from)??;
    if enabled == Some(true) {
        if let Some((def, rt)) = registry::find(&task_id) {
            if def.executor.is_some() && !rt.running.load(Ordering::Relaxed) {
                spawn_poller(app_handle.clone(), def, rt);
            }
        }
    }
    let _ = app_handle.emit("background-tasks-updated", &list);
    Ok(list)
}

/// 测试隔离：内存值重置为注册表默认（--test-threads=1 门禁保证无并发干扰）
#[cfg(test)]
pub(crate) fn reset_runtimes_for_test() {
    for (def, rt) in TASKS.iter().zip(RUNTIMES.iter()) {
        rt.enabled.store(def.enabled_default, Ordering::Relaxed);
        rt.interval_sec
            .store(def.interval_default, Ordering::Relaxed);
        rt.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod background_tasks_tests {
    use super::*;
    use crate::app_dir::AppDataDirGuard;

    /// 手动 current_thread runtime 驱动 async 命令（照 plan_balance 先例）
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn sorted_keys<T: serde::Serialize>(v: &T) -> Vec<String> {
        let json = serde_json::to_string(v).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    /// 读磁盘 settings.json 的 backgroundTasks 段（测试断言用）
    fn disk_section(dir: &std::path::Path) -> serde_json::Value {
        let content = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        let root: serde_json::Value = serde_json::from_str(&content).unwrap();
        root.get(registry::SETTINGS_KEY)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    /// DTO 序列化键集合精确 = 六键（规格 FR-2 写死，无 default 字段）
    #[test]
    fn background_task_info_serde_key_set() {
        let list = current_list();
        assert_eq!(
            sorted_keys(&list[0]),
            [
                "enabled",
                "intervalMax",
                "intervalMin",
                "intervalSec",
                "taskId",
                "title"
            ]
        );
    }

    /// list 返回注册表序两条，enabled/intervalSec = 默认值（无文件场景）
    #[test]
    fn list_returns_registry_order_with_defaults() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let list = block_on(background_tasks_list()).unwrap();
        let ids: Vec<&str> = list.iter().map(|t| t.task_id.as_str()).collect();
        assert_eq!(ids, ["planBalance", "sessionRefresh"], "应保持注册表序");
        assert!(list[0].enabled, "planBalance 默认启用");
        assert_eq!(list[0].interval_sec, 10, "planBalance 默认 10s");
        assert!(list[1].enabled, "sessionRefresh 默认启用");
        assert_eq!(list[1].interval_sec, 3, "sessionRefresh 默认 3s");
    }

    /// 合法 intervalSec → 磁盘落盘 + 内存更新 + 返回清单（双断言）
    #[test]
    fn set_config_valid_interval_persists_and_updates_memory() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let list = set_config_core("planBalance", None, Some(120)).unwrap();
        assert_eq!(
            disk_section(dir.path())["planBalance"]["intervalSec"],
            120,
            "磁盘应持久化 intervalSec=120"
        );
        assert_eq!(
            RUNTIMES[0].interval_sec.load(Ordering::Relaxed),
            120,
            "内存应更新为 120"
        );
        assert_eq!(list.len(), 2, "应返回完整清单");
        assert_eq!(list[0].interval_sec, 120);
    }

    /// 合法 enabled → 磁盘子键落盘 + 内存更新
    #[test]
    fn set_config_valid_enabled_persists() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        set_config_core("sessionRefresh", Some(false), None).unwrap();
        assert!(
            !disk_section(dir.path())["sessionRefresh"]["enabled"]
                .as_bool()
                .unwrap(),
            "磁盘子键 enabled 应为 false"
        );
        assert!(
            !RUNTIMES[1].enabled.load(Ordering::Relaxed),
            "内存 enabled 应为 false"
        );
    }

    /// 子键合并：写 sessionRefresh 不踩 planBalance 已落盘的子键
    #[test]
    fn set_config_subkey_merge_preserves_sibling() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        set_config_core("planBalance", None, Some(120)).unwrap();
        set_config_core("sessionRefresh", Some(false), None).unwrap();
        let section = disk_section(dir.path());
        assert_eq!(
            section["planBalance"]["intervalSec"], 120,
            "子键合并不应互踩：planBalance.intervalSec 应保留"
        );
        assert_eq!(section["sessionRefresh"]["enabled"], false);
    }

    /// 越界 intervalSec → Validation（消息含任务标题 + 合法区间）；磁盘无文件 + 内存不变
    #[test]
    fn set_config_out_of_range_rejected() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = set_config_core("planBalance", None, Some(5)).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("套餐余量查询"),
                    "消息应含任务标题，实际: {msg}"
                );
                assert!(msg.contains("10–3600"), "消息应含合法区间，实际: {msg}");
            }
            other => panic!("越界应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
        assert_eq!(
            RUNTIMES[0].interval_sec.load(Ordering::Relaxed),
            10,
            "越界写入不应改动内存间隔"
        );
    }

    /// 未知 taskId → Validation 含「未知任务」；磁盘/内存不变
    #[test]
    fn set_config_unknown_task_rejected() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = set_config_core("nope", Some(true), None).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(msg.contains("未知任务"), "消息应提示未知任务，实际: {msg}");
            }
            other => panic!("未知任务应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
        assert!(
            RUNTIMES[0].enabled.load(Ordering::Relaxed),
            "内存不应被改动"
        );
        assert!(
            RUNTIMES[1].enabled.load(Ordering::Relaxed),
            "内存不应被改动"
        );
    }

    /// 双 None（未提供任何配置项）→ Validation
    #[test]
    fn set_config_no_fields_rejected() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = set_config_core("planBalance", None, None).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("未提供任何配置项"),
                    "消息应提示缺配置项，实际: {msg}"
                );
            }
            other => panic!("双 None 应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
    }

    /// 合法写后磁盘值 == 内存值（落盘成功才更新内存，恒一致）
    #[test]
    fn set_config_disk_memory_consistent() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        set_config_core("planBalance", None, Some(300)).unwrap();
        let disk = disk_section(dir.path())["planBalance"]["intervalSec"]
            .as_u64()
            .unwrap();
        let mem = RUNTIMES[0].interval_sec.load(Ordering::Relaxed);
        assert_eq!(disk, mem, "磁盘值与内存值应一致");
        assert_eq!(mem, 300);
    }

    // ── R2b: 读失败/解析失败传播（不再空段吞错） ──

    /// 现有 settings.json 损坏 → Err 传播 + 内存未变 + 磁盘原样（旧语义空段吞错
    /// 会丢兄弟子键仍写成功——锁死新语义防回归）
    #[test]
    fn set_config_corrupt_settings_returns_err_memory_unchanged() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let corrupt = "not valid json {{{";
        std::fs::write(dir.path().join("settings.json"), corrupt).unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = set_config_core("planBalance", None, Some(120)).unwrap_err();
        match err {
            AppError::ConfigParse(msg) => {
                assert!(
                    msg.contains("读取设置失败"),
                    "消息应含业务语义 '读取设置失败'，实际: {msg}"
                );
            }
            other => panic!("损坏 JSON 应返回 ConfigParse，实际: {other:?}"),
        }
        assert_eq!(
            RUNTIMES[0].interval_sec.load(Ordering::Relaxed),
            10,
            "解析失败不应改动内存间隔"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("settings.json")).unwrap(),
            corrupt,
            "损坏文件应原样保留，禁止被覆盖"
        );
    }

    /// settings.json 路径为已存在目录 → 读失败 Err 传播 + 内存未变（不落盘）
    #[test]
    fn set_config_read_failure_returns_err_memory_unchanged() {
        reset_runtimes_for_test();
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::create_dir(&settings_path).unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = set_config_core("planBalance", None, Some(120)).unwrap_err();
        match err {
            AppError::IoKind { kind, message } => {
                assert!(!kind.is_empty(), "kind 不应为空");
                assert!(
                    message.contains("读取设置失败"),
                    "消息应含业务语义 '读取设置失败'，实际: {message}"
                );
            }
            other => panic!("读失败应映射为 AppError::IoKind，实际: {other:?}"),
        }
        assert_eq!(
            RUNTIMES[0].interval_sec.load(Ordering::Relaxed),
            10,
            "读失败不应改动内存间隔"
        );
        assert!(
            settings_path.is_dir(),
            "读失败后不应有写盘动作（settings.json 仍是目录）"
        );
    }
}
