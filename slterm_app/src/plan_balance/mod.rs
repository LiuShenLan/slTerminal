//! 套餐余量模块(10 篇 M10.11,F10 查询语义原样;单进程 GPUI 形态)。
//!
//! 读 user 层 `~/.claude/settings.json` 的 env 判定套餐(Anthropic 兼容端点),
//! 定时轮询外部 API,把余量快照经模块级 watch channel 推给多消费者(D10-3)。
//!
//! 红线(SEC-18):token 不出凭据域——快照/DTO/推送载荷无 token 字段(类型层
//! 防线);本模块 tracing/错误消息禁止插值 token 与 Authorization 头
//! (ureq 错误 Display 不含请求头,构造错误消息时禁止自行拼接)。
//!
//! 因果锚点(旧 F10/F12 资产随迁):
//! - 快照槽四态:占位(updated_at=0,渲染 --)/ 有效(updated_at=now)/
//!   保留旧值(查询失败,updated_at 不刷新)/ 移除(来源消失,本轮出数组);
//! - emit 判定含 updated_at 整体 `PartialEq`:成功查询必刷新 updated_at 即视为
//!   变化;失败保留旧值不动不推;
//! - `refresh_plan_balance` 恒返回 Ok:单来源失败按槽位语义保留旧值,错误不
//!   冒泡到 UI 线程;
//! - 执行面:`poll_once_executor` 是 `fn()` 执行体,由 09 篇 `background_tasks`
//!   `planBalance` 条目在 GPUI background executor 上驱动;ureq 纯阻塞 HTTP
//!   全程在后台线程,UI 线程零阻塞。

pub(crate) mod deepseek;
pub(crate) mod kimi;
pub(crate) mod query;
pub(crate) mod source;

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Mutex, MutexGuard};

use crate::error::AppError;
use crate::secrets::SecretBytes;
use query::{PlanQuery, QUERIES, find_query_by_url};
use source::{PlanSource, SOURCES};

// ── DTO(规格 §7;serde camelCase;无 token 字段 = 类型层防线) ──

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanBalanceInfo {
    /// v1 恒 "claude"。
    pub source_id: String,
    /// "deepseek" | "kimi"。
    pub plan_id: String,
    /// kimi 月限额触顶。
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
    /// 最近成功查询 unix 秒;0 = 尚无成功值(渲染 --)。
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AmountInfo {
    /// 原样透传 total_balance。
    pub value: String,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowsInfo {
    pub five_hour: WindowInfo,
    pub seven_day: WindowInfo,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowInfo {
    /// 已用百分比 = used/limit×100(used 缺失经 remaining 换算回退)。
    pub used_percent: u8,
    /// ISO 字符串,可缺失。
    pub resets_at: Option<String>,
}

/// 套餐查询产出(不含 source_id/plan_id/updated_at——由编排层补全)。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FetchOutcome {
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
}

// ── 快照槽 + watch channel(D10-3:模块级静态,多消费者不耦合组件树) ──

static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>> = Mutex::new(None);

/// 更新通知订阅者(D10-3 watch 语义:只报「有新值」,值本体读 SNAPSHOT;
/// 有界 1 槽合并连续推送,订阅方一次唤醒读最新快照)。
static UPDATE_SUBSCRIBERS: Mutex<Vec<SyncSender<()>>> = Mutex::new(Vec::new());

fn snapshot_guard() -> MutexGuard<'static, Option<Vec<PlanBalanceInfo>>> {
    SNAPSHOT.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn subscribers_guard() -> MutexGuard<'static, Vec<SyncSender<()>>> {
    UPDATE_SUBSCRIBERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// 当前快照(控制面读通道;无成功查询前为空数组)。
pub(crate) fn current_snapshot() -> Vec<PlanBalanceInfo> {
    snapshot_guard().clone().unwrap_or_default()
}

/// 订阅快照更新(D10-3:多消费者各自持接收端;Drop 即退订)。
#[allow(dead_code)] // 消费点 = M10.12 设置页/壳侧余额展示;本步由用例锁定多消费者语义。
pub(crate) fn subscribe() -> PlanBalanceUpdates {
    let (sender, receiver) = mpsc::sync_channel(1);
    subscribers_guard().push(sender);
    PlanBalanceUpdates { receiver }
}

/// 快照更新接收端(watch 语义:唤醒后读最新快照,不排队历史值)。
pub(crate) struct PlanBalanceUpdates {
    receiver: Receiver<()>,
}

#[allow(dead_code)] // 消费点 = M10.12 展示订阅;本步由用例锁定语义。
impl PlanBalanceUpdates {
    /// 阻塞等待下一次推送,返回最新快照。
    pub(crate) fn recv(&self) -> Vec<PlanBalanceInfo> {
        let _ = self.receiver.recv();
        current_snapshot()
    }

    /// 非阻塞取一次推送;无新值 → None。
    pub(crate) fn try_recv(&self) -> Option<Vec<PlanBalanceInfo>> {
        self.receiver.try_recv().ok().map(|()| current_snapshot())
    }
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    *snapshot_guard() = None;
    subscribers_guard().clear();
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ── 快照合并与单轮拉取编排(纯函数/参数化,L1 全测) ──

/// 快照槽合并(规格 §6):planId 变化丢弃旧值;查询失败保留同 planId 旧值;
/// 成功采用新值;失败且无旧值 → 占位(updated_at=0,渲染 --)。
pub(crate) fn merge_slot(
    old: Option<&PlanBalanceInfo>,
    source_id: &str,
    plan_id: &str,
    result: Result<FetchOutcome, AppError>,
    now: u64,
) -> PlanBalanceInfo {
    let retained = old.filter(|entry| entry.plan_id == plan_id); // planId 变化 → 丢弃
    match result {
        Ok(outcome) => PlanBalanceInfo {
            source_id: source_id.into(),
            plan_id: plan_id.into(),
            frozen: outcome.frozen,
            amount: outcome.amount,
            windows: outcome.windows,
            updated_at: now,
        },
        Err(error) => {
            log::warn!(
                "套餐余量查询失败，保留旧值: source={source_id} plan={plan_id} error={error}"
            );
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

/// 一轮拉取编排(D6 最小可测性:resolve/fetch 参数化,L1 不触网不触盘)。
/// 返回新快照;与旧快照比较有变化才推(判定在 [`apply_snapshot`])。
pub(crate) fn poll_once_with(
    old: &[PlanBalanceInfo],
    resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, SecretBytes)>,
    fetch: impl Fn(&'static dyn PlanQuery, &SecretBytes) -> Result<FetchOutcome, AppError>,
    now: u64,
) -> Vec<PlanBalanceInfo> {
    let mut out = Vec::new();
    for source in SOURCES {
        // 来源消失(settings 缺失/env 缺失/token 空)→ 本轮从数组移除。
        let Some((base_url, token)) = resolve(*source) else {
            continue;
        };
        // 未命中任何套餐 → 静默降级。
        let Some(query) = find_query_by_url(&base_url, QUERIES) else {
            continue;
        };
        out.push(merge_slot(
            old.iter()
                .find(|entry| entry.source_id == source.source_id()),
            source.source_id(),
            query.plan_id(),
            fetch(query, &token),
            now,
        ));
    }
    out
}

/// 生产一轮拉取。
fn poll_once_production(now: u64) -> Vec<PlanBalanceInfo> {
    let old = current_snapshot();
    poll_once_with(
        &old,
        |source| source.resolve(),
        |query, token| query.fetch(token),
        now,
    )
}

/// 应用新快照:整体 PartialEq(含 updated_at)有变化才落槽并推送;
/// 失败保留旧值 → 相等 → 不推。
pub(crate) fn apply_snapshot(new: Vec<PlanBalanceInfo>) {
    {
        let mut slot = snapshot_guard();
        if slot.as_ref() == Some(&new) {
            return;
        }
        *slot = Some(new);
    }
    notify_update_subscribers();
}

fn notify_update_subscribers() {
    let mut subscribers = subscribers_guard();
    subscribers.retain(|sender| match sender.try_send(()) {
        // 槽位已满 = 已有待处理的更新;接收方唤醒后读到的仍是最新快照。
        Ok(()) | Err(TrySendError::Full(())) => true,
        Err(TrySendError::Disconnected(())) => false,
    });
}

// ── 执行面(09 篇 background_tasks `planBalance` 条目消费) ──

/// 后台任务执行体(F12 语义随迁,载体换 GPUI executor):一轮拉取 + 快照应用。
/// 签名收敛为 `fn()`;阻塞 HTTP 由调用方在 GPUI background executor 上执行。
pub(crate) fn poll_once_executor() {
    let new = poll_once_production(unix_now());
    apply_snapshot(new);
}

/// 立即刷新(旧语义原样):恒返回 Ok——单来源失败保留旧值,错误不冒泡。
/// 调用方须在 GPUI background executor 上执行(阻塞 HTTP)。
#[allow(dead_code)] // 消费点 = M10.12 设置页/托盘刷新入口;本步由用例锁定恒 Ok 语义。
pub(crate) fn refresh_plan_balance() -> Result<Vec<PlanBalanceInfo>, AppError> {
    let new = poll_once_production(unix_now());
    apply_snapshot(new);
    Ok(current_snapshot())
}

/// 测试注入形态:与生产执行体同一条编排链,仅 resolve/fetch 参数化(D6)。
#[cfg(test)]
pub(crate) fn poll_once_executor_with(
    resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, SecretBytes)>,
    fetch: impl Fn(&'static dyn PlanQuery, &SecretBytes) -> Result<FetchOutcome, AppError>,
    now: u64,
) {
    let old = current_snapshot();
    let new = poll_once_with(&old, resolve, fetch, now);
    apply_snapshot(new);
}

/// 测试注入形态:验证 refresh 在查询失败时仍恒 Ok。
#[cfg(test)]
pub(crate) fn refresh_plan_balance_with(
    resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, SecretBytes)>,
    fetch: impl Fn(&'static dyn PlanQuery, &SecretBytes) -> Result<FetchOutcome, AppError>,
) -> Result<Vec<PlanBalanceInfo>, AppError> {
    let old = current_snapshot();
    let new = poll_once_with(&old, resolve, fetch, unix_now());
    apply_snapshot(new);
    Ok(current_snapshot())
}

#[cfg(test)]
mod plan_balance_tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::source::test_support::SettingsPathGuard;
    use super::*;

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

    fn resolve_some() -> Option<(String, SecretBytes)> {
        Some((
            "https://api.deepseek.com/anthropic".into(),
            SecretBytes::new(b"sk-test-poll".to_vec()),
        ))
    }

    fn sorted_keys<T: serde::Serialize>(value: &T) -> Vec<String> {
        let json = serde_json::to_string(value).expect("序列化 DTO");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON 对象");
        let mut keys: Vec<String> = parsed
            .as_object()
            .expect("DTO 必须是对象")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    // ── DTO serde 键集合精确匹配(token 红线守卫:多键即红) ──

    /// PlanBalanceInfo 序列化键集合精确 = 六键(无 token 字段)。
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
                    used_percent: 24,
                    resets_at: None,
                },
                seven_day: WindowInfo {
                    used_percent: 42,
                    resets_at: None,
                },
            }),
            updated_at: 1_700_000_000,
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

    /// AmountInfo 键集合 = ["currency","value"]。
    #[test]
    fn amount_info_serde_key_set() {
        let amount = AmountInfo {
            value: "12.34".into(),
            currency: "CNY".into(),
        };
        assert_eq!(sorted_keys(&amount), ["currency", "value"]);
    }

    /// WindowsInfo 键集合 = ["fiveHour","sevenDay"]。
    #[test]
    fn windows_info_serde_key_set() {
        let windows = WindowsInfo {
            five_hour: WindowInfo {
                used_percent: 1,
                resets_at: None,
            },
            seven_day: WindowInfo {
                used_percent: 2,
                resets_at: None,
            },
        };
        assert_eq!(sorted_keys(&windows), ["fiveHour", "sevenDay"]);
    }

    /// WindowInfo 键集合 = ["resetsAt","usedPercent"]。
    #[test]
    fn window_info_serde_key_set() {
        let window = WindowInfo {
            used_percent: 24,
            resets_at: Some("2026-08-28T15:00:00Z".into()),
        };
        assert_eq!(sorted_keys(&window), ["resetsAt", "usedPercent"]);
    }

    // ── merge_slot(4 例) ──

    /// 成功 → 采用新值,updated_at = now。
    #[test]
    fn merge_slot_success_adopts_new() {
        let slot = merge_slot(None, "claude", "deepseek", Ok(outcome()), 1000);
        assert_eq!(slot.source_id, "claude");
        assert_eq!(slot.plan_id, "deepseek");
        assert_eq!(slot.amount, outcome().amount);
        assert_eq!(slot.updated_at, 1000);
    }

    /// 失败 + 同 planId 旧值 → 保留旧值(含旧 updated_at,不刷新)。
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

    /// 失败 + planId 变化 → 丢弃旧值,落占位(updated_at=0)。
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

    /// 失败 + 无旧值 → 占位(updated_at=0 + 全 None + frozen=false)。
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

    // ── poll_once_with(5 例,resolve/fetch 参数化注入,不触网不触盘) ──

    /// 来源消失(resolve None)→ 本轮从数组移除 → 空数组。
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

    /// resolve Some 但 URL 未命中任何套餐 → 静默降级 → 空数组。
    #[test]
    fn poll_once_url_unmatched_skips() {
        let out = poll_once_with(
            &[],
            |_| {
                Some((
                    "https://api.unknown.com/v1".into(),
                    SecretBytes::new(b"sk-test".to_vec()),
                ))
            },
            |_, _| unreachable!("未命中套餐不应触 fetch"),
            1000,
        );
        assert!(out.is_empty());
    }

    /// 查询失败 → 保留同 planId 旧值(old 中同 source_id+plan_id 条目)。
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

    /// planId 切换(old 为 deepseek,本轮命中 kimi 且失败)→ 旧值被丢弃落占位。
    #[test]
    fn poll_once_plan_change_discards() {
        let old = vec![old_entry("deepseek", 500)];
        let out = poll_once_with(
            &old,
            |_| {
                Some((
                    "https://api.kimi.com/coding".into(),
                    SecretBytes::new(b"sk-test".to_vec()),
                ))
            },
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

    /// 成功 → 采用新值,source_id/plan_id/updated_at 由编排层补全。
    #[test]
    fn poll_once_success_replaces() {
        let out = poll_once_with(
            &[],
            |_| resolve_some(),
            |query, _token| {
                assert_eq!(query.plan_id(), "deepseek");
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

    // ── emit / watch 判定(D10-3) ──

    /// 首次发布(None → 快照)推送给订阅者。
    #[test]
    fn apply_snapshot_first_publication_notifies() {
        reset_for_test();
        let updates = subscribe();
        apply_snapshot(Vec::new());
        assert_eq!(updates.try_recv(), Some(Vec::new()));
        assert_eq!(current_snapshot(), Vec::<PlanBalanceInfo>::new());
    }

    /// 快照整体相等(含 updated_at)→ 不重推。
    #[test]
    fn apply_snapshot_identical_snapshot_is_not_pushed() {
        reset_for_test();
        let updates = subscribe();
        let snapshot = vec![old_entry("deepseek", 500)];
        apply_snapshot(snapshot.clone());
        assert_eq!(updates.try_recv(), Some(snapshot.clone()));
        apply_snapshot(snapshot);
        assert!(
            updates.try_recv().is_none(),
            "整体相等(含 updated_at)不得再次推送"
        );
    }

    /// 仅 updated_at 变化(成功查询必刷新)→ 视为变化推送。
    #[test]
    fn apply_snapshot_updated_at_change_is_pushed() {
        reset_for_test();
        let updates = subscribe();
        apply_snapshot(vec![old_entry("deepseek", 500)]);
        assert!(updates.try_recv().is_some());
        apply_snapshot(vec![old_entry("deepseek", 501)]);
        assert_eq!(
            updates.try_recv(),
            Some(vec![old_entry("deepseek", 501)]),
            "updated_at 参与整体比较"
        );
    }

    /// 失败保留旧值 → 快照不变 → 不推。
    #[test]
    fn failed_round_retains_old_value_and_does_not_push() {
        reset_for_test();
        let old = old_entry("deepseek", 500);
        apply_snapshot(vec![old.clone()]);
        let updates = subscribe();
        let new = poll_once_with(
            std::slice::from_ref(&old),
            |_| resolve_some(),
            |_, _| Err(AppError::Unknown("模拟失败".into())),
            1000,
        );
        assert_eq!(new, vec![old.clone()]);
        apply_snapshot(new);
        assert!(updates.try_recv().is_none(), "失败保留旧值不推");
        assert_eq!(current_snapshot(), vec![old]);
    }

    /// watch channel 多消费者:一次推送所有订阅者各得最新快照(D10-3)。
    #[test]
    fn watch_channel_fans_out_to_multiple_subscribers() {
        reset_for_test();
        let first = subscribe();
        let second = subscribe();
        let snapshot = vec![old_entry("deepseek", 700)];
        apply_snapshot(snapshot.clone());
        assert_eq!(first.try_recv(), Some(snapshot.clone()));
        assert_eq!(second.try_recv(), Some(snapshot));
    }

    /// 无成功快照前读通道为空数组。
    #[test]
    fn snapshot_defaults_to_empty_before_first_poll() {
        reset_for_test();
        assert!(current_snapshot().is_empty());
    }

    // ── refresh 恒 Ok ──

    /// 查询失败不外抛:返回 Ok(旧快照)。
    #[test]
    fn refresh_returns_ok_even_when_fetch_fails() {
        reset_for_test();
        let old = old_entry("deepseek", 500);
        apply_snapshot(vec![old.clone()]);
        let result = refresh_plan_balance_with(
            |_| resolve_some(),
            |_, _| Err(AppError::Unknown("模拟失败".into())),
        );
        assert!(result.is_ok(), "单来源失败不得冒泡");
        assert_eq!(result.expect("恒 Ok"), vec![old]);
    }

    /// 无来源(settings 缺失)→ Ok(空快照),不触网。
    #[test]
    fn refresh_returns_ok_without_source() {
        reset_for_test();
        let dir = tempfile::tempdir().expect("创建临时目录");
        let _guard = SettingsPathGuard::set(dir.path().join(".claude").join("missing.json"));
        let result = refresh_plan_balance();
        assert!(result.is_ok());
        assert!(result.expect("恒 Ok").is_empty());
    }

    // ── executor 执行面(后台线程 / 不阻塞 UI) ──

    /// 执行体签名 = `background_tasks::TaskExecutor`(`fn()`),已挂注册表条目。
    #[test]
    fn executor_body_matches_the_background_task_signature() {
        let executor: crate::background_tasks::TaskExecutor = poll_once_executor;
        let _: fn() = executor;
        let def = crate::background_tasks::find(crate::background_tasks::PLAN_BALANCE)
            .expect("planBalance 应已注册");
        assert!(def.executor.is_some(), "planBalance 应由进程级执行体驱动");
    }

    /// 执行体在后台线程运行并发布快照(来源存在但未命中套餐 → 空快照,不出网)。
    #[test]
    fn executor_publishes_from_a_background_thread() {
        reset_for_test();
        let updates = subscribe();
        let dir = tempfile::tempdir().expect("创建临时目录");
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.invalid/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-background"}}"#,
        )
        .expect("写测试 settings");
        let _guard = SettingsPathGuard::set(path);

        let caller = std::thread::current().id();
        let worker = std::thread::spawn(|| {
            let worker_thread = std::thread::current().id();
            poll_once_executor();
            worker_thread
        });
        let worker_thread = worker.join().expect("执行体线程");
        assert_ne!(caller, worker_thread, "执行体必须在后台线程运行");
        assert_eq!(updates.try_recv(), Some(Vec::new()), "首推应到达订阅者");
        assert!(current_snapshot().is_empty());
    }

    /// 慢查询在飞时调用线程已释放(阻塞 HTTP 不占 UI 线程)。
    #[test]
    fn slow_fetch_does_not_block_the_calling_thread() {
        reset_for_test();
        let worker = std::thread::spawn(|| {
            poll_once_executor_with(
                |_| resolve_some(),
                |_, _| {
                    std::thread::sleep(Duration::from_millis(150));
                    Ok(outcome())
                },
                1000,
            );
        });
        let started = Instant::now();
        assert!(!worker.is_finished(), "慢查询在飞时调用线程应已释放");
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "调用线程被阻塞在查询上"
        );
        worker.join().expect("执行体线程");
        assert_eq!(current_snapshot()[0].updated_at, 1000);
    }

    // ── 双通道裁定守卫(10 篇测试点清单) ──

    fn collect_rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(root).expect("读取源码目录") {
            let path = entry.expect("源码目录项").path();
            if path.is_dir() {
                collect_rust_sources(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    /// plan_balance 只读 user 层 settings.json,不触达凭据库;ai_providers 反之。
    #[test]
    fn plan_balance_reads_user_settings_not_the_credential_store() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_sources(&src.join("plan_balance"), &mut files);
        assert!(files.len() >= 5, "扫描面应覆盖 plan_balance 全部源文件");

        // 禁词拆分书写,避免扫描器自命中。
        let forbidden = [
            concat!("Credential", "Store"),
            concat!("WindowsCredential", "Store"),
            concat!("load_generic_", "secret"),
            concat!("load_api_", "key"),
            concat!("platform::", "credentials"),
        ];
        let mut saw_user_settings_key = false;
        for path in files {
            let source = std::fs::read_to_string(&path).expect("读取 plan_balance 源文件");
            saw_user_settings_key |= source.contains(concat!("ANTHROPIC_AUTH_", "TOKEN"));
            for needle in forbidden {
                assert!(
                    !source.contains(needle),
                    "{}: plan_balance 不得触达凭据库符号 {needle}",
                    path.display()
                );
            }
        }
        assert!(
            saw_user_settings_key,
            "plan_balance 应经 user 层 settings.json 读取 token"
        );

        let providers_path = src.join("ai_providers.rs");
        let providers = std::fs::read_to_string(&providers_path).expect("读取 ai_providers 源文件");
        for needle in [
            concat!("ANTHROPIC_AUTH_", "TOKEN"),
            concat!("ANTHROPIC_BASE_", "URL"),
            concat!(".cl", "aude"),
            concat!("home_", "dir"),
        ] {
            assert!(
                !providers.contains(needle),
                "ai_providers 不得读 user 层文件: {needle}"
            );
        }
        assert!(
            providers.contains(concat!("Credential", "Store")),
            "ai_providers 凭据通道应经凭据库 trait(双通道另一侧)"
        );
    }
}
