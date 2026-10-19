//! 后台任务注册表:元数据静态切片 + 运行时原子量 + 订阅生命周期。
//!
//! 收编旧双端调度器(前端 interval + 后端 poller)的契约面:任务元数据单点、
//! 无订阅不轮询、单轮重入闸门、失败只记日志不落失败态。执行载体换成 GPUI
//! background executor——周期任务由订阅方在工作区泵里按注册表节拍驱动,
//! 一次性/事件驱动任务由调用方在后台执行器上触发;零新增常驻 OS 定时器。
//!
//! 注册任务:
//! - `planBalance`:套餐余量查询(执行体归 plan_balance;订阅方 = 壳
//!   启动链,接线);
//! - `agentScreenWatchdog`:1 Hz 屏幕看门狗与托盘快照同拍(订阅方 = workspace);
//! - `autoUpdateCheck`:启动一次性更新检查,`interval_sec` 即启动延迟秒数
//!   (12s 契约值);立即段(水合/缓存提示)不受延迟影响;
//! - `autoUpdateDownload`:更新可用后的按需预取(订阅方 = workspace)。
//!
//! 失败策略:单轮 Err/panic 记日志后释放闸门,下一轮照常排程;持久配置
//! 绝不记录失败态,避免一次网络抖动把任务永久打瘫。配置写入只直写原子量,
//! 不重建驱动循环。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

/// 套餐余量查询(注册表条目归后台任务注册表,执行体归 plan_balance)。
pub(crate) const PLAN_BALANCE: &str = "planBalance";
/// 屏幕看门狗 + 托盘快照(同拍融合,见 `workspace::pumps`)。
pub(crate) const AGENT_SCREEN_WATCHDOG: &str = "agentScreenWatchdog";
/// 启动一次性更新检查。
pub(crate) const AUTO_UPDATE_CHECK: &str = "autoUpdateCheck";
/// 更新可用后的按需后台预取。
pub(crate) const AUTO_UPDATE_DOWNLOAD: &str = "autoUpdateDownload";

/// 任务触发形态。周期/一次性/事件驱动共用同一生命周期与失败策略,
/// 只有驱动方决定何时调用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskSchedule {
    /// 周期任务:订阅后按 `interval_sec` 连续触发。
    Interval,
    /// 启动一次性:只触发一轮;`interval_sec` 表示启动延迟秒数。
    Once,
    /// 事件驱动:订阅方按需触发;`interval_sec` 不参与调度。
    OnDemand,
}

/// 进程级执行体:在 GPUI background executor 上 spawn。
///
/// `planBalance` 以 `Some(poll_once_executor)` 走进程级执行体;其余任务需要
/// UI/调用方上下文,统一走订阅方自驱(`executor: None`)。
pub(crate) type TaskExecutor = fn();

/// 任务元数据(静态切片条目)。
///
/// 字段为设置页/注册表校验的单点元数据;本模块无用户可配任务,消费点归
/// 设置页与 planBalance,故允许暂未读取。
#[allow(dead_code)]
pub(crate) struct TaskDef {
    pub task_id: &'static str,
    pub title: &'static str,
    pub interval_min: u64,
    pub interval_max: u64,
    pub interval_default: u64,
    pub enabled_default: bool,
    /// None = 订阅方自驱(无订阅不轮询);Some = 进程级执行体。
    pub executor: Option<TaskExecutor>,
    pub schedule: TaskSchedule,
}

/// 任务运行时内存配置(与 [`TASKS`] 同序对齐)。
pub(crate) struct TaskRuntime {
    pub enabled: AtomicBool,
    pub interval_sec: AtomicU64,
    /// 驱动循环是否存活(订阅激活置位;订阅归零/循环退出复位)。
    pub running: AtomicBool,
    /// 订阅者计数:归零即停轮询,非零按节拍续跑。
    pub subscribers: AtomicUsize,
    /// 单轮重入闸门:在飞轮未结束时到点跳过,不并发叠加。
    in_flight: AtomicBool,
    /// 一次性任务是否已触发(Once 语义;失败也不重跑)。
    completed: AtomicBool,
}

impl TaskRuntime {
    const fn new(enabled: bool, interval_sec: u64) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            interval_sec: AtomicU64::new(interval_sec),
            running: AtomicBool::new(false),
            subscribers: AtomicUsize::new(0),
            in_flight: AtomicBool::new(false),
            completed: AtomicBool::new(false),
        }
    }
}

/// 任务配置落盘段名(契约锚点;本模块无用户可配任务,写通道消费点归 06/10)。
#[allow(dead_code)]
pub(crate) const SETTINGS_KEY: &str = "backgroundTasks";

/// 任务注册表(元数据单点)——同序 zip [`RUNTIMES`],等长由测试锁死。
pub(crate) static TASKS: &[TaskDef] = &[
    TaskDef {
        task_id: PLAN_BALANCE,
        title: "套餐余量查询",
        interval_min: 10,
        interval_max: 3600,
        interval_default: 10,
        enabled_default: true,
        executor: Some(crate::plan_balance::poll_once_executor),
        schedule: TaskSchedule::Interval,
    },
    TaskDef {
        task_id: AGENT_SCREEN_WATCHDOG,
        title: "屏幕看门狗/托盘快照",
        interval_min: 1,
        interval_max: 60,
        interval_default: 1,
        enabled_default: true,
        executor: None,
        schedule: TaskSchedule::Interval,
    },
    TaskDef {
        task_id: AUTO_UPDATE_CHECK,
        title: "自动更新检查",
        // Once 任务的 interval 字段承载启动延迟:12s 是契约值。
        interval_min: 12,
        interval_max: 12,
        interval_default: 12,
        enabled_default: true,
        executor: None,
        schedule: TaskSchedule::Once,
    },
    TaskDef {
        task_id: AUTO_UPDATE_DOWNLOAD,
        title: "后台下载更新安装包",
        interval_min: 0,
        interval_max: 0,
        interval_default: 0,
        enabled_default: true,
        executor: None,
        schedule: TaskSchedule::OnDemand,
    },
];

/// 运行时内存配置存储(静态引用见 [`RUNTIMES`];原子量需要具名 static)。
static RUNTIME_STORE: [TaskRuntime; 4] = [
    TaskRuntime::new(true, 10),
    TaskRuntime::new(true, 1),
    TaskRuntime::new(true, 12),
    TaskRuntime::new(true, 0),
];

/// 运行时内存配置(与 [`TASKS`] 同序;静态初值 = 注册表默认值)。
pub(crate) static RUNTIMES: &[TaskRuntime] = &RUNTIME_STORE;

/// 按 task_id 查元数据;未知 id → None(调用方转校验错误/静默跳过)。
pub(crate) fn find(task_id: &str) -> Option<&'static TaskDef> {
    TASKS.iter().find(|def| def.task_id == task_id)
}

fn runtime(task_id: &str) -> Option<&'static TaskRuntime> {
    let index = TASKS.iter().position(|def| def.task_id == task_id)?;
    RUNTIMES.get(index)
}

/// 当前生效节拍(未知任务 → 0)。
pub(crate) fn interval_sec(task_id: &str) -> u64 {
    runtime(task_id).map_or(0, |runtime| runtime.interval_sec.load(Ordering::Relaxed))
}

/// 一次性启动任务的延迟(Once 任务的 `interval_sec` 即延迟秒数)。
pub(crate) fn startup_delay(task_id: &str) -> Duration {
    Duration::from_secs(interval_sec(task_id))
}

/// 是否仍有订阅者。驱动循环据此决定继续等待还是退出(归零即停)。
pub(crate) fn has_subscribers(task_id: &str) -> bool {
    runtime(task_id).is_some_and(|runtime| runtime.subscribers.load(Ordering::Relaxed) > 0)
}

/// 订阅任务:返回的守卫随调用方生命周期存活,归零时停止轮询。
pub(crate) fn subscribe(task_id: &str) -> Option<TaskSubscription> {
    let runtime = runtime(task_id)?;
    runtime.subscribers.fetch_add(1, Ordering::AcqRel);
    runtime.running.store(true, Ordering::Release);
    Some(TaskSubscription { runtime })
}

/// 订阅守卫(Drop = 退订)。
pub(crate) struct TaskSubscription {
    runtime: &'static TaskRuntime,
}

impl Drop for TaskSubscription {
    fn drop(&mut self) {
        if self.runtime.subscribers.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.runtime.running.store(false, Ordering::Release);
        }
    }
}

/// 驱动循环退出时复位运行标志(覆盖禁用/实体销毁等非退订路径)。
pub(crate) fn stop_loop(task_id: &str) {
    if let Some(runtime) = runtime(task_id) {
        runtime.running.store(false, Ordering::Release);
    }
}

/// 单轮结局:供驱动方/测试区分跳过原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskRunOutcome {
    /// 本轮已执行且成功。
    Ran,
    /// 未启用、无订阅或一次性任务已完成。
    Inactive,
    /// 上一轮未结束,本轮按闸门跳过。
    Reentered,
    /// 执行体返回 Err 或 panic;闸门已释放,下一轮照常。
    Failed,
}

/// 单轮运行守卫:覆盖执行体全程,持有期间阻止重入。
pub(crate) struct TaskRunGuard {
    runtime: &'static TaskRuntime,
}

impl Drop for TaskRunGuard {
    fn drop(&mut self) {
        self.runtime.in_flight.store(false, Ordering::Release);
    }
}

fn try_begin(task_id: &str) -> Result<TaskRunGuard, TaskRunOutcome> {
    let Some(def) = find(task_id) else {
        return Err(TaskRunOutcome::Inactive);
    };
    let Some(runtime) = runtime(task_id) else {
        return Err(TaskRunOutcome::Inactive);
    };
    if !runtime.enabled.load(Ordering::Relaxed) || runtime.subscribers.load(Ordering::Relaxed) == 0
    {
        return Err(TaskRunOutcome::Inactive);
    }
    // Once:先在 completed 上抢占,保证跨线程也只有一轮。
    if matches!(def.schedule, TaskSchedule::Once) && runtime.completed.swap(true, Ordering::AcqRel)
    {
        return Err(TaskRunOutcome::Inactive);
    }
    match runtime
        .in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) => Ok(TaskRunGuard { runtime }),
        Err(_) => Err(TaskRunOutcome::Reentered),
    }
}

/// 开始一轮(不执行执行体):调用方自行跨异步/线程持有守卫。
pub(crate) fn begin(task_id: &str) -> Option<TaskRunGuard> {
    try_begin(task_id).ok()
}

/// 同步执行一轮:重入闸门 + panic/Err 失败策略统一入口。
///
/// 失败只记日志并释放闸门,不改 `enabled`/`interval_sec`,下一轮照常排程;
/// 一次性任务即使失败也不会重跑(One-shot 语义)。
pub(crate) fn run_once(task_id: &str, work: impl FnOnce() -> Result<(), String>) -> TaskRunOutcome {
    let _guard = match try_begin(task_id) {
        Ok(guard) => guard,
        Err(outcome) => return outcome,
    };
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(Ok(())) => TaskRunOutcome::Ran,
        Ok(Err(error)) => {
            log::warn!("background task {task_id} failed: {error}");
            TaskRunOutcome::Failed
        }
        Err(_) => {
            log::warn!("background task {task_id} panicked; next round scheduled");
            TaskRunOutcome::Failed
        }
    }
}

/// 运行期改配:先校验边界,再直写原子量。不重建驱动循环;禁用只让下一轮
/// 变成 `Inactive`,重新启用后同一循环继续工作。
///
/// 消费点归设置页 / planBalance;本模块只锁定原子量语义与边界。
#[allow(dead_code)]
pub(crate) fn apply_config(
    task_id: &str,
    enabled: Option<bool>,
    interval_sec: Option<u64>,
) -> Result<(), String> {
    let Some(def) = find(task_id) else {
        return Err(format!("未知后台任务: {task_id}"));
    };
    if enabled.is_none() && interval_sec.is_none() {
        return Err("未提供任何配置项".to_owned());
    }
    if let Some(sec) = interval_sec
        && !(def.interval_min..=def.interval_max).contains(&sec)
    {
        return Err(format!(
            "{} 的间隔须为 {}–{} 秒，实际 {sec}",
            def.title, def.interval_min, def.interval_max
        ));
    }
    let runtime = runtime(task_id).ok_or_else(|| format!("未知后台任务: {task_id}"))?;
    if let Some(enabled) = enabled {
        runtime.enabled.store(enabled, Ordering::Relaxed);
    }
    if let Some(sec) = interval_sec {
        runtime.interval_sec.store(sec, Ordering::Relaxed);
    }
    Ok(())
}

/// 启动时按注册表默认值复位运行时原子量(壳启动调用一次)。
pub(crate) fn reset_defaults() {
    for (def, runtime) in TASKS.iter().zip(RUNTIMES.iter()) {
        runtime
            .enabled
            .store(def.enabled_default, Ordering::Relaxed);
        runtime
            .interval_sec
            .store(def.interval_default, Ordering::Relaxed);
        runtime.running.store(false, Ordering::Relaxed);
        runtime.in_flight.store(false, Ordering::Relaxed);
        runtime.completed.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    reset_defaults();
    for runtime in RUNTIMES.iter() {
        runtime.subscribers.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod background_task_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        AGENT_SCREEN_WATCHDOG, AUTO_UPDATE_CHECK, AUTO_UPDATE_DOWNLOAD, PLAN_BALANCE, RUNTIMES,
        SETTINGS_KEY, TASKS, TaskRunOutcome, TaskSchedule, apply_config, begin, interval_sec,
        reset_for_test, run_once, subscribe,
    };

    fn counter() -> Arc<AtomicUsize> {
        Arc::new(AtomicUsize::new(0))
    }

    fn count(counter: &AtomicUsize) -> usize {
        counter.load(Ordering::SeqCst)
    }

    /// 注册表键集/边界/触发形态/同序默认值一次性锁死。
    #[test]
    fn registry_metadata_is_locked() {
        reset_for_test();
        assert_eq!(TASKS.len(), RUNTIMES.len(), "注册表与运行时必须等长");
        let ids: Vec<_> = TASKS.iter().map(|def| def.task_id).collect();
        assert_eq!(
            ids,
            [
                PLAN_BALANCE,
                AGENT_SCREEN_WATCHDOG,
                AUTO_UPDATE_CHECK,
                AUTO_UPDATE_DOWNLOAD
            ]
        );

        // planBalance:套餐余量查询——默认 10s,合法 10–3600,进程级执行体。
        let plan = &TASKS[0];
        assert_eq!(plan.schedule, TaskSchedule::Interval);
        assert_eq!(plan.interval_min, 10);
        assert_eq!(plan.interval_default, 10);
        assert_eq!(plan.interval_max, 3600);
        assert!(plan.enabled_default);
        assert!(plan.executor.is_some(), "planBalance 执行体归 plan_balance");

        let watchdog = &TASKS[1];
        assert_eq!(watchdog.schedule, TaskSchedule::Interval);
        assert_eq!(watchdog.interval_min, 1);
        assert_eq!(watchdog.interval_default, 1);
        assert_eq!(watchdog.interval_max, 60);
        assert!(watchdog.enabled_default);
        assert!(watchdog.executor.is_none(), "看门狗由 workspace 订阅自驱");

        let check = &TASKS[2];
        assert_eq!(check.schedule, TaskSchedule::Once);
        assert_eq!(check.interval_min, 12);
        assert_eq!(check.interval_default, 12);
        assert_eq!(check.interval_max, 12);
        assert_eq!(interval_sec(AUTO_UPDATE_CHECK), 12, "12s 启动延迟契约");
        assert!(check.executor.is_none());
        assert_eq!(
            super::startup_delay(AUTO_UPDATE_CHECK),
            std::time::Duration::from_secs(12)
        );

        let download = &TASKS[3];
        assert_eq!(download.schedule, TaskSchedule::OnDemand);
        assert!(download.executor.is_none());

        for (def, runtime) in TASKS.iter().zip(RUNTIMES.iter()) {
            assert_eq!(runtime.enabled.load(Ordering::Relaxed), def.enabled_default);
            assert_eq!(
                runtime.interval_sec.load(Ordering::Relaxed),
                def.interval_default
            );
            assert!(!runtime.running.load(Ordering::Relaxed));
            assert!(!runtime.in_flight.load(Ordering::Relaxed));
            assert!(!runtime.completed.load(Ordering::Relaxed));
        }
        assert_eq!(SETTINGS_KEY, "backgroundTasks");
    }

    /// 订阅生命周期:无订阅不执行;退订归零后循环标志复位。
    #[test]
    fn unsubscribed_tasks_never_run_and_zero_subscribers_stop_the_loop() {
        reset_for_test();
        let calls = counter();
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            TaskRunOutcome::Inactive
        );
        assert_eq!(count(&calls), 0, "无订阅者不得空转");

        let first = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        let second = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Ran
        );

        drop(first);
        assert!(
            super::has_subscribers(AGENT_SCREEN_WATCHDOG),
            "尚有订阅者时保持轮询"
        );
        assert!(RUNTIMES[1].running.load(Ordering::Relaxed));

        drop(second);
        assert!(!super::has_subscribers(AGENT_SCREEN_WATCHDOG));
        assert!(!RUNTIMES[1].running.load(Ordering::Relaxed), "订阅归零即停");
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Inactive
        );
    }

    /// 重入闸门:在飞轮内的到点触发直接跳过,轮末释放。
    #[test]
    fn reentry_gate_skips_overlapping_rounds() {
        reset_for_test();
        let _subscription = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        let outer = counter();
        let inner = counter();
        let outcome = run_once(AGENT_SCREEN_WATCHDOG, || {
            outer.fetch_add(1, Ordering::SeqCst);
            let nested = run_once(AGENT_SCREEN_WATCHDOG, || {
                inner.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            assert_eq!(nested, TaskRunOutcome::Reentered);
            Ok(())
        });
        assert_eq!(outcome, TaskRunOutcome::Ran);
        assert_eq!(count(&outer), 1);
        assert_eq!(count(&inner), 0, "同一任务不得并发叠加");
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Ran,
            "闸门在轮末释放"
        );
    }

    /// 失败策略:Err 只记日志,闸门释放、配置不变、下一轮照常。
    #[test]
    fn failed_round_keeps_config_and_the_next_round_alive() {
        reset_for_test();
        let _subscription = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        let before = (
            RUNTIMES[1].enabled.load(Ordering::Relaxed),
            RUNTIMES[1].interval_sec.load(Ordering::Relaxed),
        );
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Err("模拟失败".to_owned())),
            TaskRunOutcome::Failed
        );
        let calls = counter();
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            TaskRunOutcome::Ran
        );
        assert_eq!(count(&calls), 1, "失败后下一轮必须照常排程");
        assert_eq!(
            before,
            (
                RUNTIMES[1].enabled.load(Ordering::Relaxed),
                RUNTIMES[1].interval_sec.load(Ordering::Relaxed)
            ),
            "单轮失败不得落成持久失败态"
        );
    }

    /// panic 同样被闸门兜住,不毒化下一轮。
    #[test]
    fn panicking_round_releases_the_reentry_gate() {
        reset_for_test();
        let _subscription = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = run_once(AGENT_SCREEN_WATCHDOG, || panic!("模拟执行体 panic"));
        std::panic::set_hook(previous);
        assert_eq!(outcome, TaskRunOutcome::Failed);
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Ran,
            "panic 后闸门必须释放"
        );
    }

    /// 改配直写原子量:立即生效、不重建循环;越界/未知/空配置拒绝。
    #[test]
    fn config_changes_take_effect_without_restarting_the_loop() {
        reset_for_test();
        let _subscription = subscribe(AGENT_SCREEN_WATCHDOG).expect("已注册");
        let running = RUNTIMES[1].running.load(Ordering::Relaxed);
        assert!(running, "订阅激活后驱动循环存活");

        apply_config(AGENT_SCREEN_WATCHDOG, Some(false), Some(5)).expect("合法配置");
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Inactive,
            "禁用即时生效"
        );
        apply_config(AGENT_SCREEN_WATCHDOG, Some(true), None).expect("合法配置");
        assert_eq!(interval_sec(AGENT_SCREEN_WATCHDOG), 5);
        assert_eq!(
            run_once(AGENT_SCREEN_WATCHDOG, || Ok(())),
            TaskRunOutcome::Ran,
            "重新启用后同一循环继续工作"
        );
        assert_eq!(
            RUNTIMES[1].running.load(Ordering::Relaxed),
            running,
            "改配不得重建驱动循环"
        );

        assert!(
            apply_config(AGENT_SCREEN_WATCHDOG, None, Some(0)).is_err(),
            "越界间隔必须拒绝"
        );
        assert!(
            apply_config("missingTask", Some(true), None).is_err(),
            "未知任务必须拒绝"
        );
        assert!(
            apply_config(AGENT_SCREEN_WATCHDOG, None, None).is_err(),
            "空配置必须拒绝"
        );
        assert_eq!(
            interval_sec(AGENT_SCREEN_WATCHDOG),
            5,
            "被拒配置不得改动内存值"
        );
    }

    /// 一次性任务:完成（或失败）后不再重跑。
    #[test]
    fn once_task_runs_at_most_once() {
        reset_for_test();
        let _subscription = subscribe(AUTO_UPDATE_CHECK).expect("已注册");
        let guard = begin(AUTO_UPDATE_CHECK).expect("首轮");
        drop(guard);
        assert!(
            begin(AUTO_UPDATE_CHECK).is_none(),
            "Once 任务完成后不得重跑"
        );
        assert_eq!(
            run_once(AUTO_UPDATE_CHECK, || Ok(())),
            TaskRunOutcome::Inactive
        );
    }

    /// 按需任务:订阅是触发前提;守卫覆盖整轮,重叠触发跳过。
    #[test]
    fn on_demand_guard_blocks_overlapping_downloads() {
        reset_for_test();
        assert!(begin(AUTO_UPDATE_DOWNLOAD).is_none(), "无订阅不触发");
        let _subscription = subscribe(AUTO_UPDATE_DOWNLOAD).expect("已注册");
        let guard = begin(AUTO_UPDATE_DOWNLOAD).expect("首轮");
        assert!(begin(AUTO_UPDATE_DOWNLOAD).is_none(), "在飞轮必须跳过");
        drop(guard);
        assert!(begin(AUTO_UPDATE_DOWNLOAD).is_some(), "轮末释放闸门");
    }
}

/// GPUI 虚拟时钟下的 12s 一次性延迟契约(物化 `startup_delay` 的时序语义)。
#[cfg(all(test, feature = "gpui-test-support"))]
mod startup_timing_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use gpui::TestAppContext;

    use super::{AUTO_UPDATE_CHECK, reset_for_test, run_once, startup_delay, subscribe};

    #[gpui::test]
    fn startup_once_task_fires_after_twelve_seconds_and_only_once(cx: &mut TestAppContext) {
        reset_for_test();
        let calls = Arc::new(AtomicUsize::new(0));
        let subscription = subscribe(AUTO_UPDATE_CHECK).expect("已注册");
        let delay = startup_delay(AUTO_UPDATE_CHECK);
        let executor = cx.executor();
        let timer = executor.clone();
        let counter = calls.clone();
        executor
            .spawn(async move {
                let _subscription = subscription;
                timer.timer(delay).await;
                let _ = run_once(AUTO_UPDATE_CHECK, || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                });
            })
            .detach();

        executor.advance_clock(Duration::from_secs(11));
        executor.run_until_parked();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "11s 时不得提前触发启动检查"
        );
        executor.advance_clock(Duration::from_secs(1));
        executor.run_until_parked();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "12s 到点必须触发一次");
        executor.advance_clock(Duration::from_secs(12));
        executor.run_until_parked();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "一次性任务不得重跑");
    }
}
