//! 任务注册表（元数据单点）+ 运行时内存配置 + 配置读取钳制
//!
//! 形态 = 静态切片（照 plan_balance SOURCES/QUERIES 先例 U2）：新增任务 = TASKS 追加
//! 一行 + RUNTIMES 追加一项（等长守卫测试锁死）。taskId 合法值集 ↔ 前端
//! BACKGROUND_TASK_IDS 常量双边锁死（硬约束 #4）。

use std::sync::atomic::{AtomicBool, AtomicU64};

use crate::app_dir::app_data_dir;

/// 执行体签名：poller 骨架每轮在 spawn_blocking 内调用；AppHandle 供 emit
pub type TaskExecutor = fn(tauri::AppHandle);

/// 任务元数据（静态切片条目）
pub struct TaskDef {
    pub task_id: &'static str,
    pub title: &'static str,
    pub interval_min: u64,
    pub interval_max: u64,
    pub interval_default: u64,
    pub enabled_default: bool,
    /// None = 前端任务（后端仅代管配置与元数据，不 spawn 循环）
    pub executor: Option<TaskExecutor>,
}

/// 任务运行时内存配置（与 TASKS 同序对齐；启动时读盘初始化，set_config 写）
pub struct TaskRuntime {
    pub enabled: AtomicBool,
    pub interval_sec: AtomicU64,
    /// poller 循环是否在跑（停循环后置 false；重启用时据此判定重 spawn）
    pub running: AtomicBool,
}

/// settings.json 顶层键（settings.rs 白名单第 5 键引用此常量，防字面量漂移）
pub(crate) const SETTINGS_KEY: &str = "backgroundTasks";

/// 任务注册表（元数据单点）——键集与前端 BACKGROUND_TASK_IDS 同步
pub static TASKS: &[TaskDef] = &[
    TaskDef {
        task_id: "planBalance",
        title: "套餐余量查询",
        interval_min: 10,
        interval_max: 3600,
        interval_default: 10,
        enabled_default: true,
        executor: Some(crate::plan_balance::poll_once_executor),
    },
    TaskDef {
        task_id: "sessionRefresh",
        title: "会话历史刷新",
        interval_min: 2,
        interval_max: 300,
        interval_default: 3,
        enabled_default: true,
        executor: None,
    },
];

/// 运行时内存配置（与 TASKS 同序——静态初值 = 默认值，启动时 resolve_task_config 覆盖）
pub static RUNTIMES: [TaskRuntime; 2] = [
    TaskRuntime {
        enabled: AtomicBool::new(true),
        interval_sec: AtomicU64::new(10),
        running: AtomicBool::new(false),
    },
    TaskRuntime {
        enabled: AtomicBool::new(true),
        interval_sec: AtomicU64::new(3),
        running: AtomicBool::new(false),
    },
];

/// 按 taskId 查（元数据 + 运行时）——未知 taskId → None（命令层转 Validation）
pub(crate) fn find(task_id: &str) -> Option<(&'static TaskDef, &'static TaskRuntime)> {
    TASKS
        .iter()
        .zip(RUNTIMES.iter())
        .find(|(d, _)| d.task_id == task_id)
}

/// 单任务生效配置（读盘钳制：缺失/损坏/类型错/越界 → 逐字段独立回退默认，
/// 照 plan_balance resolve_poll_interval 口径提升为逐任务）
pub(crate) struct ResolvedConfig {
    pub enabled: bool,
    pub interval_sec: u64,
}

pub(crate) fn resolve_task_config(def: &TaskDef) -> ResolvedConfig {
    let read = || -> Option<serde_json::Value> {
        let path = app_data_dir().ok()?.join("settings.json");
        let content = std::fs::read_to_string(path).ok()?;
        let root: serde_json::Value = serde_json::from_str(&content).ok()?;
        root.get(SETTINGS_KEY)?.get(def.task_id).cloned()
    };
    let mut cfg = ResolvedConfig {
        enabled: def.enabled_default,
        interval_sec: def.interval_default,
    };
    if let Some(v) = read() {
        if let Some(b) = v.get("enabled").and_then(|x| x.as_bool()) {
            cfg.enabled = b;
        }
        if let Some(n) = v.get("intervalSec").and_then(|x| x.as_u64()) {
            if (def.interval_min..=def.interval_max).contains(&n) {
                cfg.interval_sec = n;
            }
        }
    }
    cfg
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::app_dir::AppDataDirGuard;
    use crate::background_tasks::reset_runtimes_for_test;
    use std::sync::atomic::Ordering;

    /// TASKS 键集精确 == ["planBalance","sessionRefresh"] + 两任务逐字段断言（边界表锁死）
    #[test]
    fn tasks_registry_key_set_locked() {
        let ids: Vec<&str> = TASKS.iter().map(|d| d.task_id).collect();
        assert_eq!(ids, ["planBalance", "sessionRefresh"]);

        // planBalance：套餐余量查询——默认 10s，合法 10–3600，默认启用，后端执行体
        let plan = &TASKS[0];
        assert_eq!(plan.task_id, "planBalance");
        assert_eq!(plan.title, "套餐余量查询");
        assert_eq!(plan.interval_min, 10);
        assert_eq!(plan.interval_max, 3600);
        assert_eq!(plan.interval_default, 10);
        assert!(plan.enabled_default);
        assert!(
            plan.executor.is_some(),
            "planBalance 应由后端 poller 驱动（执行体 Some）"
        );

        // sessionRefresh：会话历史刷新——默认 3s，合法 2–300，默认启用，前端调度器驱动
        let session = &TASKS[1];
        assert_eq!(session.task_id, "sessionRefresh");
        assert_eq!(session.title, "会话历史刷新");
        assert_eq!(session.interval_min, 2);
        assert_eq!(session.interval_max, 300);
        assert_eq!(session.interval_default, 3);
        assert!(session.enabled_default);
        assert!(
            session.executor.is_none(),
            "sessionRefresh 由前端调度器驱动，执行体应为 None"
        );
    }

    /// TASKS 与 RUNTIMES 等长守卫 + 静态初值 == 对应 def 默认值
    #[test]
    fn runtimes_same_length_as_tasks() {
        reset_runtimes_for_test();
        assert_eq!(
            TASKS.len(),
            RUNTIMES.len(),
            "注册表与运行时数组必须等长（find 按同序 zip）"
        );
        for (def, rt) in TASKS.iter().zip(RUNTIMES.iter()) {
            assert_eq!(rt.enabled.load(Ordering::Relaxed), def.enabled_default);
            assert_eq!(
                rt.interval_sec.load(Ordering::Relaxed),
                def.interval_default
            );
            assert!(
                !rt.running.load(Ordering::Relaxed),
                "静态初值 running 必须为 false"
            );
        }
    }

    /// find：命中返回元数据 + 运行时；未知 taskId → None
    #[test]
    fn find_hit_and_miss() {
        let (def, rt) = find("planBalance").expect("planBalance 应命中");
        assert_eq!(def.task_id, "planBalance");
        assert!(rt.enabled.load(Ordering::Relaxed));

        let (def, _rt) = find("sessionRefresh").expect("sessionRefresh 应命中");
        assert_eq!(def.task_id, "sessionRefresh");

        assert!(find("nope").is_none(), "未知 taskId 应返回 None");
    }

    // ── resolve_task_config（6 例，AppDataDirGuard 注入 tempdir，逐字段独立钳制） ──

    /// 无 settings.json → 全部任务回退默认（enabled/intervalSec）
    #[test]
    fn resolve_task_config_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        for def in TASKS {
            let cfg = resolve_task_config(def);
            assert_eq!(
                cfg.enabled, def.enabled_default,
                "{} 无文件应回退默认 enabled",
                def.task_id
            );
            assert_eq!(
                cfg.interval_sec, def.interval_default,
                "{} 无文件应回退默认 intervalSec",
                def.task_id
            );
        }
    }

    /// 合法值 → 采用（enabled=false + intervalSec=120）
    #[test]
    fn resolve_task_config_valid_value_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"backgroundTasks":{"planBalance":{"enabled":false,"intervalSec":120}}}"#,
        )
        .unwrap();
        let cfg = resolve_task_config(&TASKS[0]);
        assert!(!cfg.enabled, "enabled=false 应被采用");
        assert_eq!(cfg.interval_sec, 120, "intervalSec=120 应被采用");
    }

    /// intervalSec 越界（9999）→ 回退默认；enabled 合法值保留（逐字段独立）
    #[test]
    fn resolve_task_config_out_of_range_falls_back_independently() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"backgroundTasks":{"planBalance":{"enabled":false,"intervalSec":9999}}}"#,
        )
        .unwrap();
        let cfg = resolve_task_config(&TASKS[0]);
        assert!(!cfg.enabled, "enabled 合法值应保留（逐字段独立钳制）");
        assert_eq!(
            cfg.interval_sec, TASKS[0].interval_default,
            "越界 intervalSec 应回退默认"
        );
    }

    /// enabled 非 bool → 回退默认；intervalSec 合法值保留
    #[test]
    fn resolve_task_config_non_bool_enabled_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"backgroundTasks":{"planBalance":{"enabled":"yes","intervalSec":120}}}"#,
        )
        .unwrap();
        let cfg = resolve_task_config(&TASKS[0]);
        assert_eq!(
            cfg.enabled, TASKS[0].enabled_default,
            "非 bool enabled 应回退默认"
        );
        assert_eq!(cfg.interval_sec, 120, "intervalSec 合法值应保留");
    }

    /// backgroundTasks 段非对象（标量）→ 整体回退默认
    #[test]
    fn resolve_task_config_section_non_object_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(dir.path().join("settings.json"), r#"{"backgroundTasks":5}"#).unwrap();
        let cfg = resolve_task_config(&TASKS[0]);
        assert_eq!(cfg.enabled, TASKS[0].enabled_default);
        assert_eq!(cfg.interval_sec, TASKS[0].interval_default);
    }

    /// 损坏 JSON → 回退默认
    #[test]
    fn resolve_task_config_corrupt_json_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(dir.path().join("settings.json"), "not json {{{").unwrap();
        let cfg = resolve_task_config(&TASKS[0]);
        assert_eq!(cfg.enabled, TASKS[0].enabled_default);
        assert_eq!(cfg.interval_sec, TASKS[0].interval_default);
    }
}
