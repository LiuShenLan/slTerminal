//! Runtime 请求参数族与等待匹配(M7.2 只落 wait/subscribe;其余 param
//! 结构族随 M7.3 方法面补齐)。
//!
//! 等待语义是一等公民:`after_seq` 基线 + 状态匹配共同决定命中,提交前
//! 的旧空闲态不能把「等 settled」判成已完成。

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{ApiError, RuntimePane, RuntimeTaskState, RuntimeWaitState};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SubscribeParams {
    #[serde(default)]
    pub(super) since_revision: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaitParams {
    #[serde(default)]
    pub(super) window_id: Option<u64>,
    pub(super) pane_id: u64,
    pub(super) state: RuntimeWaitState,
    /// 1..=86_400_000;越界 `invalid_params`。
    pub(super) timeout_ms: u64,
    /// 提交时捕获的基线跃迁计数;缺席 = 旧语义(只看状态)。
    #[serde(default)]
    pub(super) after_seq: Option<u64>,
}

pub(super) fn parse_params<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    serde_json::from_value(value.clone())
        .map_err(|error| ApiError::invalid_params(format!("invalid method parameters: {error}")))
}

/// 命中 = 状态匹配 且(带基线时)seq > after_seq。否则对本来就 idle 的
/// pane 等 idle 会立即返回,调用方误以为提交的工作已完成。
pub(super) fn wait_matches(
    pane: &RuntimePane,
    expected: RuntimeWaitState,
    after_seq: Option<u64>,
) -> bool {
    after_seq.is_none_or(|baseline| pane.state_change_seq > baseline)
        && wait_state_matches(pane.task_state, expected)
}

pub(super) fn wait_state_matches(actual: RuntimeTaskState, expected: RuntimeWaitState) -> bool {
    match expected {
        RuntimeWaitState::Idle => actual == RuntimeTaskState::Idle,
        RuntimeWaitState::Running => actual == RuntimeTaskState::Running,
        RuntimeWaitState::WaitingInput => actual == RuntimeTaskState::WaitingInput,
        RuntimeWaitState::Attention => actual == RuntimeTaskState::Attention,
        RuntimeWaitState::Finished => actual == RuntimeTaskState::Finished,
        RuntimeWaitState::Failed => actual == RuntimeTaskState::Failed,
        // settled ≠ 成功:只表示不在运行。
        RuntimeWaitState::Settled => actual != RuntimeTaskState::Running,
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    fn pane(state: RuntimeTaskState, seq: u64) -> RuntimePane {
        RuntimePane {
            id: 1,
            active: false,
            title: String::new(),
            cwd: String::new(),
            branch: String::new(),
            running_program: None,
            agent: None,
            task_state: state,
            state_change_seq: seq,
            active_run: None,
            last_run: None,
        }
    }

    #[test]
    fn wait_matches_requires_the_state_and_the_baseline() {
        let idle = pane(RuntimeTaskState::Idle, 3);
        assert!(wait_matches(&idle, RuntimeWaitState::Idle, None));
        assert!(wait_matches(&idle, RuntimeWaitState::Idle, Some(2)));
        // 基线已追上:即便状态匹配也不得命中。
        assert!(!wait_matches(&idle, RuntimeWaitState::Idle, Some(3)));
        assert!(!wait_matches(&idle, RuntimeWaitState::Running, None));
    }

    #[test]
    fn settled_matches_every_non_running_state() {
        for state in [
            RuntimeTaskState::Idle,
            RuntimeTaskState::WaitingInput,
            RuntimeTaskState::Attention,
            RuntimeTaskState::Finished,
            RuntimeTaskState::Failed,
        ] {
            assert!(
                wait_state_matches(state, RuntimeWaitState::Settled),
                "{state:?}"
            );
        }
        assert!(!wait_state_matches(
            RuntimeTaskState::Running,
            RuntimeWaitState::Settled
        ));
    }

    #[test]
    fn wait_params_reject_unknown_fields() {
        let raw = serde_json::json!({
            "pane_id": 1,
            "state": "idle",
            "timeout_ms": 1000,
            "oracle": true,
        });
        let error = parse_params::<WaitParams>(&raw).expect_err("unknown field");
        assert_eq!(error.code, "invalid_params");
    }

    #[test]
    fn subscribe_params_default_to_no_since_revision() {
        let params: SubscribeParams =
            parse_params(&serde_json::json!({})).expect("empty subscribe params");
        assert_eq!(params.since_revision, None);
    }
}
