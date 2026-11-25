//! 套餐查询状态机：调度、防重入、冷却、过期与凭据代次。
//!
//! 本服务不执行网络请求；调用方按 token 在后台执行体查询并回投结果。

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::{PlanBalanceInfo, PlanBalanceSource};

/// 默认调度周期。
pub(crate) const DEFAULT_PERIOD: Duration = Duration::from_secs(60);
/// 周期配置下限。
pub(crate) const MIN_PERIOD: Duration = Duration::from_secs(30);
/// 周期配置上限。
pub(crate) const MAX_PERIOD: Duration = Duration::from_secs(3_600);
/// 手动全量刷新冷却（从实际发起时间起算）。
pub(crate) const MANUAL_COOLDOWN: Duration = Duration::from_secs(5);
/// 单次请求自身超时：超时前且未错过周期时保持查询中。
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// 墙钟端口；测试注入固定时间，运行期只读。
pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

/// 生产墙钟。
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// 一次已获准的查询；代次用于丢弃令牌替换或删除后的迟到结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FetchToken {
    source: PlanBalanceSource,
    generation: u64,
    started_at: SystemTime,
}

impl FetchToken {
    pub(crate) const fn source(self) -> PlanBalanceSource {
        self.source
    }
}

#[derive(Default)]
struct SourceState {
    generation: u64,
    in_flight: Option<SystemTime>,
    last_success: Option<SystemTime>,
    next_due: Option<SystemTime>,
    result: Option<PlanBalanceInfo>,
    error: Option<super::client::PlanBalanceError>,
}

/// 单来源展示判定：UI 文案与错误/过期状态同源。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanBalanceView {
    pub(crate) result: Option<PlanBalanceInfo>,
    pub(crate) error: Option<super::client::PlanBalanceError>,
    pub(crate) loading: bool,
    pub(crate) stale: bool,
}

/// 两来源套餐查询服务；结果只存内存。
pub(crate) struct PlanBalanceService {
    enabled: bool,
    period: Duration,
    sources: [SourceState; 2],
    manual_cooldown_until: Option<SystemTime>,
}

impl PlanBalanceService {
    /// 默认启用、周期 60 秒；启动即到期。
    pub(crate) fn new(clock: Arc<dyn Clock>) -> Self {
        let now = clock.now();
        let source_state = |next_due| SourceState {
            next_due: Some(next_due),
            ..SourceState::default()
        };
        Self {
            enabled: true,
            period: DEFAULT_PERIOD,
            sources: [source_state(now), source_state(now)],
            manual_cooldown_until: None,
        }
    }

    pub(crate) fn period(&self) -> Duration {
        self.period
    }

    /// 设置提交：启用开关与周期；周期夹到 30–3600 秒。
    pub(crate) fn apply_settings(&mut self, enabled: bool, period: Option<Duration>) {
        self.enabled = enabled;
        if let Some(period) = period {
            self.period = period.clamp(MIN_PERIOD, MAX_PERIOD);
        }
    }

    /// 自动调度：已启用且到期、无在途的来源。导航可见性不参与判定。
    pub(crate) fn due(&self, now: SystemTime) -> Vec<PlanBalanceSource> {
        if !self.enabled {
            return Vec::new();
        }
        PlanBalanceSource::ALL
            .into_iter()
            .filter(|source| {
                let state = self.state(*source);
                state.in_flight.is_none() && state.next_due.is_some_and(|due| due <= now)
            })
            .collect()
    }

    /// 自动查询准入；同一来源在途期间拒绝重入。
    pub(crate) fn begin_auto(
        &mut self,
        source: PlanBalanceSource,
        now: SystemTime,
    ) -> Option<FetchToken> {
        if !self.enabled {
            return None;
        }
        let state = self.state_mut(source);
        if state.in_flight.is_some() || !state.next_due.is_some_and(|due| due <= now) {
            return None;
        }
        state.in_flight = Some(now);
        Some(FetchToken {
            source,
            generation: state.generation,
            started_at: now,
        })
    }

    /// 手动全量刷新：5 秒冷却按实际发起时间计算；在途来源跳过。
    pub(crate) fn begin_manual_refresh(&mut self, now: SystemTime) -> Vec<FetchToken> {
        if self.manual_cooldown_until.is_some_and(|until| now < until) {
            return Vec::new();
        }
        let tokens = PlanBalanceSource::ALL
            .into_iter()
            .filter_map(|source| {
                let state = self.state_mut(source);
                if state.in_flight.is_some() {
                    return None;
                }
                state.in_flight = Some(now);
                Some(FetchToken {
                    source,
                    generation: state.generation,
                    started_at: now,
                })
            })
            .collect::<Vec<_>>();
        if tokens.is_empty() {
            return tokens;
        }
        self.manual_cooldown_until = Some(now + MANUAL_COOLDOWN);
        tokens
    }

    /// 结算一次查询；代次不符的迟到结果直接丢弃。
    pub(crate) fn complete(
        &mut self,
        token: FetchToken,
        result: Result<PlanBalanceInfo, super::client::PlanBalanceError>,
        now: SystemTime,
    ) {
        let period = self.period;
        let state = self.state_mut(token.source);
        if state.generation != token.generation || state.in_flight.is_none() {
            return;
        }
        state.in_flight = None;
        state.next_due = Some(now + period);
        match result {
            Ok(info) => {
                state.last_success = Some(now);
                state.result = Some(info);
                state.error = None;
            }
            Err(error) => {
                // 失败保留旧结果与最后成功时间，只记录错误。
                state.error = Some(error);
            }
        }
    }

    /// 凭据替换或删除：递增代次并清空旧账号结果，迟到回执失效。
    pub(crate) fn invalidate(&mut self, source: PlanBalanceSource) {
        let state = self.state_mut(source);
        state.generation += 1;
        state.in_flight = None;
        state.last_success = None;
        state.result = None;
        state.error = None;
    }

    /// 单来源展示判定：过期只在错过完整周期且无新成功结果时成立。
    pub(crate) fn view(&self, source: PlanBalanceSource, now: SystemTime) -> PlanBalanceView {
        let state = self.state(source);
        let loading = state.in_flight.is_some_and(|started| {
            now.duration_since(started)
                .map(|elapsed| elapsed < REQUEST_TIMEOUT)
                .unwrap_or(true)
        });
        let stale = state.result.is_some()
            && state.last_success.is_some_and(|last| {
                now.duration_since(last)
                    .map(|elapsed| elapsed >= self.period)
                    .unwrap_or(false)
            });
        PlanBalanceView {
            result: state.result.clone(),
            error: state.error,
            loading,
            stale,
        }
    }

    fn state(&self, source: PlanBalanceSource) -> &SourceState {
        match source {
            PlanBalanceSource::DeepSeek => &self.sources[0],
            PlanBalanceSource::Kimi => &self.sources[1],
        }
    }

    fn state_mut(&mut self, source: PlanBalanceSource) -> &mut SourceState {
        match source {
            PlanBalanceSource::DeepSeek => &mut self.sources[0],
            PlanBalanceSource::Kimi => &mut self.sources[1],
        }
    }
}

#[cfg(test)]
mod service_tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::plan_balance::{AmountInfo, WindowInfo, WindowsInfo};

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    struct FakeClock(Mutex<SystemTime>);

    impl FakeClock {
        fn new(now: SystemTime) -> Arc<Self> {
            Arc::new(Self(Mutex::new(now)))
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> SystemTime {
            *self.0.lock().unwrap()
        }
    }

    fn service(clock: Arc<FakeClock>) -> PlanBalanceService {
        PlanBalanceService::new(clock)
    }

    fn info(percent: u8) -> PlanBalanceInfo {
        PlanBalanceInfo {
            frozen: false,
            amount: Some(AmountInfo {
                value: "1.00".to_owned(),
                currency: "CNY".to_owned(),
            }),
            windows: Some(WindowsInfo {
                five_hour: WindowInfo {
                    used_percent: percent,
                    resets_at: None,
                },
                seven_day: WindowInfo {
                    used_percent: percent,
                    resets_at: None,
                },
            }),
        }
    }

    /// 单来源防重入：在途期间自动与手动都不得重复发起。
    #[test]
    fn plan_balance_single_source_no_reentry() {
        let mut service = service(FakeClock::new(at(1_000)));
        let token = service
            .begin_auto(PlanBalanceSource::DeepSeek, at(1_000))
            .expect("首次到期");
        assert!(
            service
                .begin_auto(PlanBalanceSource::DeepSeek, at(1_000))
                .is_none(),
            "在途期间不得重入"
        );
        let manual = service.begin_manual_refresh(at(1_000));
        assert_eq!(manual.len(), 1, "仅另一来源可手动发起");
        assert_eq!(manual[0].source(), PlanBalanceSource::Kimi);
        service.complete(token, Ok(info(10)), at(1_001));
        assert!(
            service
                .begin_auto(PlanBalanceSource::DeepSeek, at(1_001))
                .is_none(),
            "结算后未到下一周期"
        );
    }

    /// 手动全量刷新从实际发起时间计算 5 秒冷却。
    #[test]
    fn plan_balance_manual_refresh_cooldown() {
        let mut service = service(FakeClock::new(at(1_000)));
        let tokens = service.begin_manual_refresh(at(1_000));
        assert_eq!(tokens.len(), 2, "全量刷新两个来源");
        assert!(
            service.begin_manual_refresh(at(1_004)).is_empty(),
            "冷却内不得再次发起"
        );
        for token in tokens {
            service.complete(token, Ok(info(1)), at(1_004));
        }
        assert!(
            service.begin_manual_refresh(at(1_004)).is_empty(),
            "冷却与在途无关"
        );
        assert_eq!(service.begin_manual_refresh(at(1_005)).len(), 2);
    }

    /// 错过完整周期才过期；未错过周期时在途请求保持查询中。
    #[test]
    fn plan_balance_stale_after_full_period_and_loading_rules() {
        let source = PlanBalanceSource::Kimi;
        let mut service = service(FakeClock::new(at(1_000)));
        let token = service.begin_auto(source, at(1_000)).unwrap();
        service.complete(token, Ok(info(10)), at(1_000));

        let view = service.view(source, at(1_030));
        assert!(!view.stale && !view.loading, "周期内保持新鲜");

        // 周期边界前 5 秒手动发起；周期边界处仍在自身超时内。
        let token = service
            .begin_manual_refresh(at(1_055))
            .into_iter()
            .find(|token| token.source() == source)
            .expect("手动刷新必须覆盖该来源");
        let view = service.view(source, at(1_056));
        assert!(view.loading, "请求未超时");
        assert!(!view.stale, "尚未错过完整周期");

        let view = service.view(source, at(1_061));
        assert!(view.stale, "错过完整周期且无新成功结果");
        assert!(view.loading, "请求在自身超时内仍视为查询中");

        let view = service.view(source, at(1_064));
        assert!(view.stale);
        assert!(!view.loading, "超过自身超时不再显示查询中");
        service.complete(token, Ok(info(20)), at(1_064));
        assert!(!service.view(source, at(1_064)).stale, "新成功结果清除过期");
    }

    /// 凭据代次变化使在途与既有结果失效，迟到结果不得回填。
    #[test]
    fn plan_balance_generation_discards_late_results() {
        let source = PlanBalanceSource::DeepSeek;
        let mut service = service(FakeClock::new(at(1_000)));
        let token = service.begin_auto(source, at(1_000)).unwrap();
        service.complete(token, Ok(info(10)), at(1_001));
        assert!(service.view(source, at(1_001)).result.is_some());

        let stale_token = service.begin_auto(source, at(1_061)).unwrap();
        service.invalidate(source);
        assert!(
            service.view(source, at(1_061)).result.is_none(),
            "凭据替换后旧结果不得继续展示"
        );
        service.complete(stale_token, Ok(info(99)), at(1_061));
        assert!(
            service.view(source, at(1_061)).result.is_none(),
            "旧代次回执不得回填"
        );

        // 新代次可重新查询。
        let token = service
            .begin_auto(source, at(1_061))
            .expect("失效后回到到期状态");
        service.complete(token, Ok(info(30)), at(1_062));
        assert_eq!(
            service
                .view(source, at(1_062))
                .result
                .unwrap()
                .windows
                .unwrap()
                .five_hour
                .used_percent,
            30
        );
    }

    /// 设置提交：禁用停止自动调度，周期夹到 30–3600 秒；可见性不参与。
    #[test]
    fn plan_balance_settings_commit_and_visibility() {
        let clock = FakeClock::new(at(1_000));
        let mut service = service(clock);
        assert_eq!(service.due(at(1_000)).len(), 2, "默认启用且启动即到期");

        service.apply_settings(false, None);
        assert!(service.due(at(1_000)).is_empty(), "禁用后不得自动调度");
        assert!(
            service
                .begin_auto(PlanBalanceSource::DeepSeek, at(1_000))
                .is_none(),
            "禁用期间自动准入同样关闭"
        );

        service.apply_settings(true, Some(Duration::from_secs(10)));
        assert_eq!(service.period(), MIN_PERIOD, "周期夹到下限");
        service.apply_settings(true, Some(Duration::from_secs(7_200)));
        assert_eq!(service.period(), MAX_PERIOD, "周期夹到上限");

        // 导航可见性不是服务输入；同一状态下到期只由周期与在途决定。
        assert_eq!(service.due(at(1_000)).len(), 2);
    }

    /// 集成链路：到期 → 发起 → 结算 → 下一周期 → 手动冷却 → 失败保留旧结果。
    #[test]
    fn integration_plan_balance_service_schedule() {
        let clock = FakeClock::new(at(1_000));
        let mut service = service(clock);
        let fetch_log = Arc::new(Mutex::new(Vec::new()));

        let due = service.due(at(1_000));
        assert_eq!(due.len(), 2);
        let mut tokens = Vec::new();
        for source in due {
            fetch_log.lock().unwrap().push(source);
            tokens.push(service.begin_auto(source, at(1_000)).unwrap());
        }
        for token in tokens {
            service.complete(token, Ok(info(20)), at(1_001));
        }
        assert!(service.due(at(1_030)).is_empty(), "周期内不再自动查询");
        assert_eq!(service.due(at(1_061)).len(), 2, "下一周期重新到期");

        // 手动刷新不依赖调度周期，但受 5 秒冷却。
        let manual = service.begin_manual_refresh(at(1_061));
        assert_eq!(manual.len(), 2);
        assert!(service.begin_manual_refresh(at(1_064)).is_empty());
        for token in manual {
            service.complete(
                token,
                Err(crate::plan_balance::client::PlanBalanceError::Request),
                at(1_062),
            );
        }
        assert_eq!(
            service.view(PlanBalanceSource::DeepSeek, at(1_062)).error,
            Some(crate::plan_balance::client::PlanBalanceError::Request),
            "失败记录错误并保留旧结果"
        );
        assert!(
            service
                .view(PlanBalanceSource::DeepSeek, at(1_062))
                .result
                .is_some(),
            "失败不得清除旧结果"
        );
    }
}
