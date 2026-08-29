//! 套餐余量模块（F10）——读 user 层 settings.json 判定套餐 → 轮询查询 → 事件推送
//!
//! 红线：token 不出后端——DTO 无 token 字段；本模块 tracing/错误消息禁止插值
//! token 与 Authorization 头（ureq 错误 Display 不含请求头，构造错误消息时禁止自行拼接）。

pub mod deepseek;
pub mod kimi;
pub mod query;
pub mod source;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;

use crate::app_dir::app_data_dir;
use crate::error::AppError;
use query::{find_query_by_url, PlanQuery, QUERIES};
use source::{PlanSource, SOURCES};

// ── DTO（规格 §7；serde camelCase ↔ src/types/planBalance.ts，硬约束 #4） ──

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanBalanceInfo {
    pub source_id: String, // v1 恒 "claude"
    pub plan_id: String,   // "deepseek" | "kimi"
    pub frozen: bool,      // kimi 月限额触顶
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
    /// 最近成功查询 unix 秒；0 = 尚无成功值（前端渲染 --）
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmountInfo {
    pub value: String, // 原样透传 total_balance
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInfo {
    pub five_hour: WindowInfo,
    pub seven_day: WindowInfo,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub remaining_percent: u8,     // 剩余 = 100 - 已用
    pub resets_at: Option<String>, // ISO 字符串，可缺失
}

/// 套餐查询产出（不含 source_id/plan_id/updated_at——由编排层补全）
#[derive(Debug, Clone, PartialEq)]
pub struct FetchOutcome {
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
}

// ── 快照存储（模块级静态——照 hooks/mod.rs WATCHER 先例：仅本模块读写，
//    不入 AppState 避免 state.rs 循环依赖） ──

static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>> = Mutex::new(None);

/// 当前轮询间隔秒数（F11：set_interval 命令写入、poller 每轮读取——运行期可改）
static POLL_INTERVAL_SEC: AtomicU64 = AtomicU64::new(DEFAULT_INTERVAL_SEC);

#[cfg(test)]
pub(crate) fn reset_snapshot_for_test() {
    let _ = SNAPSHOT.lock().unwrap().take();
}

#[cfg(test)]
pub(crate) fn reset_poll_interval_for_test() {
    POLL_INTERVAL_SEC.store(DEFAULT_INTERVAL_SEC, Ordering::Relaxed);
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── 快照合并与轮询编排（纯函数/参数化，L1 全测） ──

/// 快照槽合并（规格 §6）：planId 变化丢弃旧值；查询失败保留同 planId 旧值；
/// 成功采用新值；失败且无旧值 → 占位（updated_at=0，前端显 --）
pub(crate) fn merge_slot(
    old: Option<&PlanBalanceInfo>,
    source_id: &str,
    plan_id: &str,
    result: Result<FetchOutcome, AppError>,
    now: u64,
) -> PlanBalanceInfo {
    let retained = old.filter(|o| o.plan_id == plan_id); // planId 变化 → 丢弃
    match result {
        Ok(o) => PlanBalanceInfo {
            source_id: source_id.into(),
            plan_id: plan_id.into(),
            frozen: o.frozen,
            amount: o.amount,
            windows: o.windows,
            updated_at: now,
        },
        Err(e) => {
            tracing::warn!(source = source_id, plan = plan_id, error = %e, "套餐余量查询失败，保留旧值");
            retained.cloned().unwrap_or(PlanBalanceInfo {
                source_id: source_id.into(),
                plan_id: plan_id.into(),
                frozen: false,
                amount: None,
                windows: None,
                updated_at: 0,
            })
        }
    }
}

/// 一轮拉取编排（D6 最小可测性：resolve/fetch 参数化，L1 不触网不触盘）。
/// 返回新快照；与旧快照比较有变化才 emit（emit 判定在 apply_snapshot）。
pub(crate) fn poll_once_with(
    old: &[PlanBalanceInfo],
    resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, String)>,
    fetch: impl Fn(&'static dyn PlanQuery, &str) -> Result<FetchOutcome, AppError>,
    now: u64,
) -> Vec<PlanBalanceInfo> {
    let mut out = Vec::new();
    for source in SOURCES {
        // 来源消失（settings 缺失/env 缺失/token 空）→ 本轮从数组移除（§8.3）
        let Some((base_url, token)) = resolve(*source) else {
            continue;
        };
        // 未命中任何套餐 → 静默降级（§4.2）
        let Some(q) = find_query_by_url(&base_url, QUERIES) else {
            continue;
        };
        out.push(merge_slot(
            old.iter().find(|i| i.source_id == source.source_id()),
            source.source_id(),
            q.plan_id(),
            fetch(q, &token),
            now,
        ));
    }
    out
}

/// 生产一轮拉取（spawn_blocking 内调用）
fn poll_once_production(now: u64) -> Vec<PlanBalanceInfo> {
    poll_once_with(
        &SNAPSHOT.lock().unwrap().clone().unwrap_or_default(),
        |s| s.resolve(),
        |q, token| q.fetch(token),
        now,
    )
}

/// 应用新快照：变化才 emit（含 updated_at 参与比较——成功查询必刷新
/// updated_at 即视为变化；失败保留旧值不变不 emit；D5）
fn apply_snapshot(app_handle: &tauri::AppHandle, new: Vec<PlanBalanceInfo>) {
    let mut guard = SNAPSHOT.lock().unwrap();
    if guard.as_ref() != Some(&new) {
        *guard = Some(new.clone());
        let _ = app_handle.emit("plan-balance-updated", new);
    }
}

// ── 轮询间隔（规格 §9）：应用 settings.json 的 planBalance.intervalSec；
//    默认 60，合法 10–3600，越界/缺失/损坏回退默认 ──

// ── 域键名归域（F11）：后端消费型键名常量归本模块——settings.rs 白名单
//    与 plan_balance_set_interval 命令 payload 均经此引用，防字面量漂移 ──

/// settings.json 顶层键（白名单第 5 键，SEC-11；settings.rs 白名单引用此常量）
pub(crate) const SETTINGS_KEY: &str = "planBalance";
/// planBalance 段内轮询间隔键
const INTERVAL_SEC_KEY: &str = "intervalSec";

const DEFAULT_INTERVAL_SEC: u64 = 60;
const MIN_INTERVAL_SEC: u64 = 10;
const MAX_INTERVAL_SEC: u64 = 3600;

pub(crate) fn resolve_poll_interval() -> Duration {
    let read = || -> Option<u64> {
        let path = app_data_dir().ok()?.join("settings.json");
        let content = std::fs::read_to_string(path).ok()?;
        let root: serde_json::Value = serde_json::from_str(&content).ok()?;
        root.get(SETTINGS_KEY)?.get(INTERVAL_SEC_KEY)?.as_u64()
    };
    match read() {
        Some(v) if (MIN_INTERVAL_SEC..=MAX_INTERVAL_SEC).contains(&v) => Duration::from_secs(v),
        _ => Duration::from_secs(DEFAULT_INTERVAL_SEC),
    }
}

// ── 轮询任务（lib.rs setup 调用；随进程退出结束，单实例无生命周期管理） ──

pub fn start_plan_balance_poller(app_handle: tauri::AppHandle) {
    // 启动时从磁盘初始化内存间隔（resolve_poll_interval 钳制兜底：越界/损坏 → 60）
    POLL_INTERVAL_SEC.store(resolve_poll_interval().as_secs(), Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        loop {
            // 首轮立即执行（D8 语义保留）；此后每轮末按当前内存间隔 sleep——
            // set_interval 命令改值后下一轮即按新间隔（F11 立即生效）
            let now = unix_now();
            let handle = app_handle.clone();
            match tokio::task::spawn_blocking(move || poll_once_production(now)).await {
                Ok(new) => apply_snapshot(&handle, new),
                Err(e) => tracing::warn!(error = %e, "套餐余量轮询任务异常"),
            }
            let secs = POLL_INTERVAL_SEC.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_secs(secs)).await;
        }
    });
}

// ── 命令（三处注册：lib.rs + build.rs + capabilities，D11） ──

#[tauri::command]
pub async fn get_plan_balance() -> Result<Vec<PlanBalanceInfo>, AppError> {
    Ok(SNAPSHOT.lock().unwrap().clone().unwrap_or_default())
}

/// 立即刷新（D6）：执行一轮拉取，更新快照并按 D5 判定 emit，恒返回最新快照
#[tauri::command]
pub async fn refresh_plan_balance(
    app_handle: tauri::AppHandle,
) -> Result<Vec<PlanBalanceInfo>, AppError> {
    let now = unix_now();
    let new = tokio::task::spawn_blocking(move || poll_once_production(now))
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?;
    apply_snapshot(&app_handle, new.clone());
    Ok(new)
}

/// 设置轮询间隔（F11 后端消费型配置写通道：校验 → 复用 settings 写通道落盘 → 更新内存值）
/// 越界 → Validation 拒绝且磁盘/内存均不变
#[tauri::command]
pub async fn plan_balance_set_interval(interval_sec: u64) -> Result<(), AppError> {
    if !(MIN_INTERVAL_SEC..=MAX_INTERVAL_SEC).contains(&interval_sec) {
        return Err(AppError::Validation(format!(
            "设置轮询间隔失败: 须为 {MIN_INTERVAL_SEC}–{MAX_INTERVAL_SEC} 秒，实际 {interval_sec}"
        )));
    }
    // 复用 settings.rs 写通道（白名单/浅合并/原子写/.bak/SETTINGS_SAVE_LOCK）——禁止自建第二写通道
    crate::settings::save_settings(serde_json::json!({
        SETTINGS_KEY: { INTERVAL_SEC_KEY: interval_sec }
    }))
    .await?;
    POLL_INTERVAL_SEC.store(interval_sec, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dir::AppDataDirGuard;

    /// 手动 current_thread runtime 驱动 async 命令（照 hooks/mod.rs:443 先例）
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    // ── DTO serde 键集合精确匹配（token 红线守卫：多键即红，F10） ──

    fn sorted_keys<T: serde::Serialize>(v: &T) -> Vec<String> {
        let json = serde_json::to_string(v).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    /// PlanBalanceInfo 序列化键集合精确 = 六键（无 token 字段）
    #[test]
    fn plan_balance_info_serde_key_set() {
        let info = PlanBalanceInfo {
            source_id: "claude".into(),
            plan_id: "deepseek".into(),
            frozen: false,
            amount: Some(AmountInfo {
                value: "12.34".into(),
                currency: "CNY".into(),
            }),
            windows: Some(WindowsInfo {
                five_hour: WindowInfo {
                    remaining_percent: 24,
                    resets_at: None,
                },
                seven_day: WindowInfo {
                    remaining_percent: 42,
                    resets_at: None,
                },
            }),
            updated_at: 1700000000,
        };
        assert_eq!(
            sorted_keys(&info),
            [
                "amount",
                "frozen",
                "planId",
                "sourceId",
                "updatedAt",
                "windows"
            ]
        );
    }

    /// AmountInfo 键集合 = ["currency","value"]
    #[test]
    fn amount_info_serde_key_set() {
        let a = AmountInfo {
            value: "12.34".into(),
            currency: "CNY".into(),
        };
        assert_eq!(sorted_keys(&a), ["currency", "value"]);
    }

    /// WindowsInfo 键集合 = ["fiveHour","sevenDay"]
    #[test]
    fn windows_info_serde_key_set() {
        let w = WindowsInfo {
            five_hour: WindowInfo {
                remaining_percent: 1,
                resets_at: None,
            },
            seven_day: WindowInfo {
                remaining_percent: 2,
                resets_at: None,
            },
        };
        assert_eq!(sorted_keys(&w), ["fiveHour", "sevenDay"]);
    }

    /// WindowInfo 键集合 = ["remainingPercent","resetsAt"]
    #[test]
    fn window_info_serde_key_set() {
        let w = WindowInfo {
            remaining_percent: 24,
            resets_at: Some("2026-08-28T15:00:00Z".into()),
        };
        assert_eq!(sorted_keys(&w), ["remainingPercent", "resetsAt"]);
    }

    // ── merge_slot（4 例，F10） ──

    fn outcome() -> FetchOutcome {
        FetchOutcome {
            frozen: false,
            amount: Some(AmountInfo {
                value: "1.00".into(),
                currency: "USD".into(),
            }),
            windows: None,
        }
    }

    fn old_entry(plan_id: &str, updated_at: u64) -> PlanBalanceInfo {
        PlanBalanceInfo {
            source_id: "claude".into(),
            plan_id: plan_id.into(),
            frozen: false,
            amount: Some(AmountInfo {
                value: "9.99".into(),
                currency: "USD".into(),
            }),
            windows: None,
            updated_at,
        }
    }

    /// 成功 → 采用新值，updated_at = now
    #[test]
    fn merge_slot_success_adopts_new() {
        let slot = merge_slot(None, "claude", "deepseek", Ok(outcome()), 1000);
        assert_eq!(slot.source_id, "claude");
        assert_eq!(slot.plan_id, "deepseek");
        assert_eq!(slot.amount, outcome().amount);
        assert_eq!(slot.updated_at, 1000);
    }

    /// 失败 + 同 planId 旧值 → 保留旧值（含旧 updated_at，不刷新）
    #[test]
    fn merge_slot_failure_retains_same_plan() {
        let old = old_entry("deepseek", 500);
        let slot = merge_slot(
            Some(&old),
            "claude",
            "deepseek",
            Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(slot, old, "失败应原样保留同 planId 旧值");
    }

    /// 失败 + planId 变化 → 丢弃旧值，落占位（updated_at=0）
    #[test]
    fn merge_slot_plan_change_discards_old() {
        let old = old_entry("deepseek", 500);
        let slot = merge_slot(
            Some(&old),
            "claude",
            "kimi",
            Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(slot.plan_id, "kimi");
        assert_eq!(slot.updated_at, 0);
        assert!(slot.amount.is_none());
        assert!(slot.windows.is_none());
        assert!(!slot.frozen);
    }

    /// 失败 + 无旧值 → 占位（updated_at=0 + 全 None + frozen=false）
    #[test]
    fn merge_slot_failure_no_old_gives_placeholder() {
        let slot = merge_slot(
            None,
            "claude",
            "kimi",
            Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(slot.source_id, "claude");
        assert_eq!(slot.plan_id, "kimi");
        assert_eq!(slot.updated_at, 0);
        assert!(slot.amount.is_none());
        assert!(slot.windows.is_none());
        assert!(!slot.frozen);
    }

    // ── poll_once_with（5 例，resolve/fetch 参数化注入，不触网不触盘，F10） ──

    fn resolve_some() -> Option<(String, String)> {
        Some((
            "https://api.deepseek.com/anthropic".into(),
            "sk-test".into(),
        ))
    }

    /// 来源消失（resolve None）→ 本轮从数组移除 → 空数组（§8.3）
    #[test]
    fn poll_once_resolve_none_removes_source() {
        let out = poll_once_with(
            &[],
            |_| None,
            |_, _| unreachable!("resolve None 不应触 fetch"),
            1000,
        );
        assert!(out.is_empty());
    }

    /// resolve Some 但 URL 未命中任何套餐 → 静默降级 → 空数组（§4.2）
    #[test]
    fn poll_once_url_unmatched_skips() {
        let out = poll_once_with(
            &[],
            |_| Some(("https://api.unknown.com/v1".into(), "sk-test".into())),
            |_, _| unreachable!("未命中套餐不应触 fetch"),
            1000,
        );
        assert!(out.is_empty());
    }

    /// 查询失败 → 保留同 planId 旧值（old 中同 source_id+plan_id 条目）
    #[test]
    fn poll_once_failure_retains() {
        let old = vec![old_entry("deepseek", 500)];
        let out = poll_once_with(
            &old,
            |_| resolve_some(),
            |_, _| Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(out, old, "失败应保留旧值");
    }

    /// planId 切换（old 为 deepseek，本轮命中 kimi 且失败）→ 旧值被丢弃落占位，无残留
    #[test]
    fn poll_once_plan_change_discards() {
        let old = vec![old_entry("deepseek", 500)];
        let out = poll_once_with(
            &old,
            |_| Some(("https://api.kimi.com/coding".into(), "sk-test".into())),
            |_, _| Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].plan_id, "kimi",
            "planId 切换后不应残留 deepseek 旧值"
        );
        assert_eq!(out[0].updated_at, 0);
        assert!(out[0].amount.is_none());
    }

    /// 成功 → 采用新值，source_id/plan_id/updated_at 由编排层补全
    #[test]
    fn poll_once_success_replaces() {
        let out = poll_once_with(
            &[],
            |_| resolve_some(),
            |q, token| {
                assert_eq!(q.plan_id(), "deepseek");
                assert_eq!(token, "sk-test", "token 应透传给 fetch");
                Ok(outcome())
            },
            1000,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source_id, "claude");
        assert_eq!(out[0].plan_id, "deepseek");
        assert_eq!(out[0].updated_at, 1000);
        assert_eq!(out[0].amount, outcome().amount);
    }

    // ── resolve_poll_interval（4 例，AppDataDirGuard 注入 tempdir，F10） ──

    /// 无 settings.json → 默认 60s
    #[test]
    fn resolve_poll_interval_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        assert_eq!(resolve_poll_interval(), Duration::from_secs(60));
    }

    /// 合法值 → 采用（120s）
    #[test]
    fn resolve_poll_interval_valid_value() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"planBalance":{"intervalSec":120}}"#,
        )
        .unwrap();
        assert_eq!(resolve_poll_interval(), Duration::from_secs(120));
    }

    /// 越界（5 与 9999）→ 回退默认 60s
    #[test]
    fn resolve_poll_interval_out_of_range_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        for v in [5u64, 9999] {
            std::fs::write(
                dir.path().join("settings.json"),
                format!(r#"{{"planBalance":{{"intervalSec":{v}}}}}"#),
            )
            .unwrap();
            assert_eq!(
                resolve_poll_interval(),
                Duration::from_secs(60),
                "越界 {v} 应回退默认"
            );
        }
    }

    /// 损坏 JSON → 回退默认 60s
    #[test]
    fn resolve_poll_interval_corrupt_json_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        std::fs::write(dir.path().join("settings.json"), "not json {{{").unwrap();
        assert_eq!(resolve_poll_interval(), Duration::from_secs(60));
    }

    // ── get_plan_balance 命令（1 例，F10） ──

    /// reset 后命令核心返回空数组（无成功快照前）
    #[test]
    fn get_plan_balance_empty_initial() {
        reset_snapshot_for_test();
        let out = block_on(get_plan_balance()).unwrap();
        assert!(out.is_empty(), "初始快照应为空数组");
        reset_snapshot_for_test();
    }

    // ── POLL_INTERVAL_SEC 内存值（1 例 + 各命令用例首行 reset，F11） ──

    /// 内存间隔初值 = 默认 60（静态初值兜底；启动时由 resolve_poll_interval 覆盖）
    #[test]
    fn poll_interval_memory_default_is_60() {
        reset_poll_interval_for_test();
        assert_eq!(POLL_INTERVAL_SEC.load(Ordering::Relaxed), 60);
    }

    // ── plan_balance_set_interval 命令（4 例，AppDataDirGuard 注入 tempdir 直调，F11） ──

    /// 合法值 120 → 磁盘落盘 + 内存更新双断言
    #[test]
    fn set_interval_valid_persists_and_updates_memory() {
        reset_poll_interval_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        block_on(plan_balance_set_interval(120)).unwrap();
        let loaded = block_on(crate::settings::load_settings()).unwrap();
        assert_eq!(
            loaded.data.unwrap()[SETTINGS_KEY][INTERVAL_SEC_KEY],
            120,
            "磁盘应持久化 intervalSec=120"
        );
        assert_eq!(
            POLL_INTERVAL_SEC.load(Ordering::Relaxed),
            120,
            "内存间隔应更新为 120"
        );
    }

    /// 越界 5 → Validation 拒绝：磁盘无文件 + 内存不变
    #[test]
    fn set_interval_below_min_rejected() {
        reset_poll_interval_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = block_on(plan_balance_set_interval(5)).unwrap_err();
        match err {
            AppError::Validation(msg) => {
                assert!(
                    msg.contains("设置轮询间隔失败"),
                    "越界应提示业务语义，实际: {msg}"
                );
            }
            other => panic!("越界应返回 Validation，实际: {other:?}"),
        }
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
        assert_eq!(
            POLL_INTERVAL_SEC.load(Ordering::Relaxed),
            DEFAULT_INTERVAL_SEC,
            "越界写入不应改动内存间隔"
        );
    }

    /// 越界 9999 → 同 below：Validation + 磁盘无文件 + 内存不变
    #[test]
    fn set_interval_above_max_rejected() {
        reset_poll_interval_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        let err = block_on(plan_balance_set_interval(9999)).unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "越界应返回 Validation，实际: {err:?}"
        );
        assert!(
            !dir.path().join("settings.json").exists(),
            "被拒保存不应落盘"
        );
        assert_eq!(
            POLL_INTERVAL_SEC.load(Ordering::Relaxed),
            DEFAULT_INTERVAL_SEC,
            "越界写入不应改动内存间隔"
        );
    }

    /// 合法写后磁盘值 == 内存值（落盘成功才更新内存，恒一致）
    #[test]
    fn set_interval_disk_memory_consistent() {
        reset_poll_interval_for_test();
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());

        block_on(plan_balance_set_interval(300)).unwrap();
        let loaded = block_on(crate::settings::load_settings()).unwrap();
        let disk = loaded.data.unwrap()[SETTINGS_KEY][INTERVAL_SEC_KEY]
            .as_u64()
            .unwrap();
        let mem = POLL_INTERVAL_SEC.load(Ordering::Relaxed);
        assert_eq!(disk, mem, "磁盘值与内存值应一致");
        assert_eq!(mem, 300);
    }

    // ── 域键名常量（1 例，F11） ──

    /// 常量值锁定（防字面量漂移——settings.rs 白名单与命令 payload 经此引用）
    #[test]
    fn settings_key_constants_value() {
        assert_eq!(SETTINGS_KEY, "planBalance");
        assert_eq!(INTERVAL_SEC_KEY, "intervalSec");
    }
}
