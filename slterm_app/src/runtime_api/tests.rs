//! Runtime API 内嵌测试(信封/盖戳/订阅/managed agents/委派/run 等待)。

use super::run_wait::wait_run_phased_with_grace;
use super::*;
use serde_json::json;
use std::time::Instant;

fn test_request() -> ApiRequest {
    ApiRequest::new("sk-test-token".into(), "pane.read", json!({ "pane_id": 1 }))
}

fn pane(id: u64, state: RuntimeTaskState) -> RuntimePane {
    RuntimePane {
        id,
        active: false,
        title: format!("pane-{id}"),
        cwd: "D:/work".to_owned(),
        branch: "main".to_owned(),
        running_program: None,
        agent: None,
        task_state: state,
        state_change_seq: 0,
        active_run: None,
        last_run: None,
    }
}

fn snapshot_with_panes(panes: Vec<RuntimePane>) -> RuntimeSnapshot {
    let focused_pane_id = panes.first().map(|pane| pane.id);
    RuntimeSnapshot::new(vec![RuntimeWindow {
        id: SINGLE_WINDOW_ID,
        focused: true,
        active_tab: 0,
        focused_pane_id,
        tabs: vec![RuntimeTab {
            tab_id: Some(TabId(1)),
            index: 0,
            active: true,
            label: "shell".to_owned(),
            kind: "terminal".to_owned(),
            bell: false,
            focused_pane_id,
            zoomed_pane_id: None,
            layout: Some(RuntimeLayout::Pane {
                pane_id: panes.first().map_or(0, |pane| pane.id),
            }),
            panes,
        }],
    }])
}

fn agent_for(kind: &str) -> RuntimeAgent {
    RuntimeAgent {
        agent_id: None,
        generation: None,
        name: None,
        worktree: None,
        kind: kind.to_owned(),
        display_name: kind.to_owned(),
        session_id: Some("session-1".to_owned()),
        state_source: RuntimeAgentStateSource::Hook,
        state_rule: None,
        hook_seen: true,
    }
}

fn agent_with(kind: &str, session_id: Option<&str>) -> RuntimeAgent {
    RuntimeAgent {
        session_id: session_id.map(str::to_owned),
        ..agent_for(kind)
    }
}

fn responding_sink(
    respond: impl Fn(&RuntimeCommand) -> Result<Value, ApiError> + Send + Sync + 'static,
) -> EventSink {
    Arc::new(move |callback| {
        if let RuntimeCallback::Control(dispatch) = callback {
            let response = respond(&dispatch.command);
            dispatch.respond(response);
        }
    })
}

#[test]
fn api_request_round_trip_defaults_params_to_empty_object() {
    let raw = json!({
        "protocol": PROTOCOL_NAME,
        "version": PROTOCOL_VERSION,
        "id": "4242-7",
        "token": "sk-test-token",
        "method": "runtime.describe",
    });
    let request: ApiRequest = serde_json::from_value(raw).expect("request without params");
    assert_eq!(request.params, json!({}));
    assert_eq!(request.protocol, PROTOCOL_NAME);
    assert_eq!(request.version, PROTOCOL_VERSION);

    let encoded = serde_json::to_string(&request).expect("serialize request");
    let decoded: ApiRequest = serde_json::from_str(&encoded).expect("deserialize request");
    assert_eq!(decoded, request);
}

#[test]
fn api_request_rejects_unknown_fields() {
    let raw = json!({
        "protocol": PROTOCOL_NAME,
        "version": PROTOCOL_VERSION,
        "id": "4242-8",
        "token": "sk-test-token",
        "method": "runtime.describe",
        "extra": true,
    });
    assert!(serde_json::from_value::<ApiRequest>(raw).is_err());
}

#[test]
fn api_response_distinguishes_explicit_null_from_missing_result() {
    let explicit_null: ApiResponse = serde_json::from_value(json!({
        "protocol": PROTOCOL_NAME,
        "version": PROTOCOL_VERSION,
        "id": "4242-9",
        "ok": true,
        "result": null,
    }))
    .expect("explicit null result");
    assert_eq!(explicit_null.result, Some(Value::Null));
    assert_eq!(
        serde_json::to_value(&explicit_null).expect("serialize null result")["result"],
        Value::Null
    );

    let missing: ApiResponse = serde_json::from_value(json!({
        "protocol": PROTOCOL_NAME,
        "version": PROTOCOL_VERSION,
        "id": "4242-10",
        "ok": false,
        "error": { "code": "timeout", "message": "no answer" },
    }))
    .expect("missing result field");
    assert_eq!(missing.result, None);
    assert!(
        serde_json::to_value(&missing)
            .expect("serialize missing result")
            .get("result")
            .is_none()
    );
}

#[test]
fn api_response_identity_requires_field_consistency() {
    let request = test_request();
    let response = ApiResponse::success(request.id.clone(), json!({ "ok": true }));
    assert!(response.verify_identity(&request).is_ok());

    let mut wrong_id = response.clone();
    wrong_id.id = "other-id".to_owned();
    assert_eq!(
        wrong_id
            .verify_identity(&request)
            .expect_err("id mismatch")
            .code,
        "invalid_runtime_response"
    );

    let mut wrong_protocol = response.clone();
    wrong_protocol.protocol = "other.runtime".to_owned();
    assert!(wrong_protocol.verify_identity(&request).is_err());

    let mut wrong_version = response.clone();
    wrong_version.version = PROTOCOL_VERSION + 1;
    assert!(wrong_version.verify_identity(&request).is_err());

    let mut failure_without_error = response.clone();
    failure_without_error.ok = false;
    assert!(failure_without_error.verify_identity(&request).is_err());

    let mut success_with_error = response.clone();
    success_with_error.error = Some(ApiError::new("timeout", "unexpected"));
    assert!(success_with_error.verify_identity(&request).is_err());

    let mut failure_with_result = response;
    failure_with_result.ok = false;
    failure_with_result.error = Some(ApiError::new("timeout", "no answer"));
    assert!(failure_with_result.verify_identity(&request).is_err());
}

#[test]
fn api_event_round_trip_carries_revision_and_payload() {
    let event = ApiEvent::new("runtime.snapshot", 7, json!({ "revision": 7 }));
    let raw = serde_json::to_value(&event).expect("serialize event");
    assert_eq!(raw["protocol"], PROTOCOL_NAME);
    assert_eq!(raw["version"], PROTOCOL_VERSION);
    assert_eq!(raw["event"], "runtime.snapshot");
    assert_eq!(raw["revision"], 7);
    let decoded: ApiEvent<Value> = serde_json::from_value(raw).expect("deserialize event");
    assert_eq!(decoded, event);
}

/// M7.1 泛型事件的单源实例化:订阅链走 `ApiEvent<RuntimeSnapshot>`。
#[test]
fn api_event_instantiates_runtime_snapshot_payload() {
    let snapshot = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
    let event = ApiEvent::snapshot(snapshot.clone());
    assert_eq!(event.event, "runtime.snapshot");
    assert_eq!(event.revision, 0);
    assert_eq!(event.data, snapshot);

    let raw = serde_json::to_string(&event).expect("serialize snapshot event");
    let decoded: ApiEvent<RuntimeSnapshot> =
        serde_json::from_str(&raw).expect("deserialize snapshot event");
    assert_eq!(decoded, event);
    assert_eq!(decoded.data.windows[0].tabs[0].panes[0].id, 1);
}

#[test]
fn api_error_details_stay_optional_on_the_wire() {
    let plain = ApiError::new("invalid_params", "bad parameter");
    assert!(
        serde_json::to_value(&plain)
            .expect("serialize plain error")
            .get("details")
            .is_none()
    );
    let detailed = plain.details(json!({ "supported_versions": SUPPORTED_VERSIONS }));
    assert_eq!(
        serde_json::to_value(&detailed).expect("serialize detailed error")["details"]["supported_versions"],
        json!([PROTOCOL_VERSION])
    );
    assert_eq!(ApiError::invalid_params("bad").code, "invalid_params");
}

#[test]
fn protocol_schema_matches_envelope_constants() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../assets/runtime-api-v1.schema.json"))
            .expect("protocol schema must stay valid JSON");
    assert_eq!(schema["properties"]["protocol"]["const"], PROTOCOL_NAME);
    assert_eq!(schema["properties"]["version"]["const"], PROTOCOL_VERSION);
    assert_eq!(
        schema["$defs"]["request"]["properties"]["protocol"]["const"],
        PROTOCOL_NAME
    );
    assert_eq!(
        schema["$defs"]["response"]["properties"]["version"]["const"],
        PROTOCOL_VERSION
    );
}

#[test]
fn snapshot_tab_id_serializes_as_plain_u64() {
    let snapshot = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
    let raw = serde_json::to_value(&snapshot).expect("serialize snapshot");
    assert_eq!(raw["windows"][0]["tabs"][0]["tab_id"], 1);
    let decoded: RuntimeSnapshot = serde_json::from_value(raw).expect("deserialize snapshot");
    assert_eq!(decoded.windows[0].tabs[0].tab_id, Some(TabId(1)));
}

#[test]
fn publish_bumps_revision_once_per_semantic_change() {
    let hub = RuntimeHub::new();
    let first = hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    assert_eq!(first.revision, 1);
    assert_eq!(first.windows[0].tabs[0].panes[0].state_change_seq, 1);

    // 完全相同的投影:不烧 revision,计数保留。
    let duplicate = hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    assert_eq!(duplicate.revision, 1);
    assert_eq!(duplicate.windows[0].tabs[0].panes[0].state_change_seq, 1);

    let changed = hub.publish(snapshot_with_panes(vec![pane(
        1,
        RuntimeTaskState::Running,
    )]));
    assert_eq!(changed.revision, 2);
    assert_eq!(changed.windows[0].tabs[0].panes[0].state_change_seq, 2);
}

#[test]
fn stamp_state_change_seq_tracks_each_pane_and_starts_first_sight_at_one() {
    let hub = RuntimeHub::new();
    let first = hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    assert_eq!(first.windows[0].tabs[0].panes[0].state_change_seq, 1);

    // 新增 pane 不重排既有 pane 的计数。
    let second = hub.publish(snapshot_with_panes(vec![
        pane(1, RuntimeTaskState::Idle),
        pane(2, RuntimeTaskState::Idle),
    ]));
    assert_eq!(second.revision, 2);
    assert_eq!(second.windows[0].tabs[0].panes[0].state_change_seq, 1);
    assert_eq!(second.windows[0].tabs[0].panes[1].state_change_seq, 1);

    // 只有发生跃迁的 pane 计数 +1。
    let third = hub.publish(snapshot_with_panes(vec![
        pane(1, RuntimeTaskState::Idle),
        pane(2, RuntimeTaskState::Finished),
    ]));
    assert_eq!(third.revision, 3);
    assert_eq!(third.windows[0].tabs[0].panes[0].state_change_seq, 1);
    assert_eq!(third.windows[0].tabs[0].panes[1].state_change_seq, 2);
}

#[test]
fn subscription_receives_current_and_only_semantic_changes() {
    let hub = RuntimeHub::new();
    hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    let (_id, current, receiver) = hub.subscribe();
    assert_eq!(current.expect("current snapshot").revision, 1);

    let duplicate = hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    assert_eq!(duplicate.revision, 1);
    assert!(
        receiver.try_recv().is_err(),
        "identical snapshot must not notify subscribers"
    );

    hub.publish(snapshot_with_panes(vec![pane(
        1,
        RuntimeTaskState::Running,
    )]));
    let event = receiver.try_recv().expect("changed snapshot");
    assert_eq!(event.revision, 2);
}

#[test]
fn slow_subscriber_is_dropped_after_bounded_channel_overflow() {
    let hub = RuntimeHub::new();
    hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    let (_id, _current, receiver) = hub.subscribe();

    // 17 次语义变化:有界通道容 16 条,第 17 次 push 时订阅方被断开。
    for step in 0..SUBSCRIBER_CAPACITY + 1 {
        let state = if step % 2 == 0 {
            RuntimeTaskState::Running
        } else {
            RuntimeTaskState::Idle
        };
        hub.publish(snapshot_with_panes(vec![pane(1, state)]));
    }

    let mut received = 0;
    while receiver.recv_timeout(Duration::from_secs(1)).is_ok() {
        received += 1;
    }
    assert_eq!(received, SUBSCRIBER_CAPACITY);
}

#[test]
fn pane_lifecycle_first_event_wins_and_closes_managed_agents() {
    let hub = RuntimeHub::new();
    let mut snapshot = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
    snapshot.windows[0].tabs[0].panes[0].agent = Some(agent_for("claude"));
    hub.publish(snapshot);
    let agent = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Claude,
            SINGLE_WINDOW_ID,
            1,
            Some("session-1".to_owned()),
            None,
        )
        .expect("register agent");
    // 先发布一次让注册项 observed。
    hub.publish(
        snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]).tap_agent(agent_for("claude")),
    );

    hub.record_pane_exited(SINGLE_WINDOW_ID, 1);
    // 第二个终态事件不得覆盖首个原因。
    hub.record_pane_closed(SINGLE_WINDOW_ID, 1);
    let snapshot = hub.current().expect("republished snapshot");
    assert_eq!(snapshot.pane_lifecycles.len(), 1);
    assert_eq!(
        snapshot.pane_lifecycles[0].event,
        RuntimePaneLifecycleKind::Exited
    );
    let error = hub
        .pane_lifecycle_error(Some(SINGLE_WINDOW_ID), 1)
        .expect("lifecycle error");
    assert_eq!(error.code, "pane_exited");
    let managed = hub
        .managed_agent(&agent.agent_id, None, false)
        .expect("agent");
    assert!(!managed.active);
    assert_eq!(managed.closed_reason.as_deref(), Some("pane_exited"));
}

#[test]
fn removed_pane_is_recorded_as_closed_on_publish() {
    let hub = RuntimeHub::new();
    hub.publish(snapshot_with_panes(vec![
        pane(1, RuntimeTaskState::Idle),
        pane(2, RuntimeTaskState::Idle),
    ]));
    let next = hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    assert_eq!(next.pane_lifecycles.len(), 1);
    assert_eq!(next.pane_lifecycles[0].pane_id, 2);
    assert_eq!(
        next.pane_lifecycles[0].event,
        RuntimePaneLifecycleKind::Closed
    );
    assert_eq!(next.revision, 2);
}

#[test]
fn managed_agent_generation_and_identity_selection() {
    let hub = RuntimeHub::new();
    let first = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Claude,
            SINGLE_WINDOW_ID,
            1,
            Some("session-1".to_owned()),
            None,
        )
        .expect("first generation");
    assert_eq!(first.generation, 1);
    assert_eq!(
        hub.register_agent(
            "worker".to_owned(),
            AgentKind::Codex,
            SINGLE_WINDOW_ID,
            1,
            None,
            None,
        )
        .expect_err("active name conflict")
        .code,
        "agent_name_conflict"
    );

    // id 精确选择。
    let selected = hub
        .active_agent(&first.agent_id, Some(1))
        .expect("select by id");
    assert_eq!(selected.agent_id, first.agent_id);

    hub.close_agent(&first.agent_id, "agent_exited");
    assert_eq!(
        hub.active_agent(&first.agent_id, None)
            .expect_err("closed agent")
            .code,
        "agent_exited"
    );
    let second = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Claude,
            SINGLE_WINDOW_ID,
            1,
            None,
            None,
        )
        .expect("second generation");
    assert_eq!(second.generation, 2);

    // 指名不存在的 generation:最新 active 命中后按身份不符拒绝。
    let mismatch = hub
        .active_agent("worker", Some(7))
        .expect_err("generation mismatch");
    assert_eq!(mismatch.code, "agent_identity_mismatch");
    assert_eq!(mismatch.details.expect("details")["actual_generation"], 2);
    // 无 generation 时取最新 active。
    assert_eq!(
        hub.active_agent("worker", None)
            .expect("latest active")
            .agent_id,
        second.agent_id
    );
}

#[test]
fn registry_only_close_wakes_snapshot_waiters() {
    let hub = RuntimeHub::new();
    hub.publish(snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]));
    let agent = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Claude,
            SINGLE_WINDOW_ID,
            1,
            None,
            None,
        )
        .expect("register");
    let (_id, _current, receiver) = hub.subscribe();
    hub.close_agent(&agent.agent_id, "agent_exited");
    let woken = receiver
        .try_recv()
        .expect("registry close must wake waiters");
    // 规范快照本身没有语义变化,revision 不涨。
    assert_eq!(woken.revision, 1);
}

#[test]
fn managed_agent_projection_fills_pane_identity() {
    let hub = RuntimeHub::new();
    let agent = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Claude,
            SINGLE_WINDOW_ID,
            1,
            Some("session-1".to_owned()),
            None,
        )
        .expect("register");
    let snapshot = hub.publish(
        snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]).tap_agent(agent_for("claude")),
    );
    let projected = snapshot.windows[0].tabs[0].panes[0]
        .agent
        .as_ref()
        .expect("pane agent");
    assert_eq!(projected.agent_id.as_deref(), Some(agent.agent_id.as_str()));
    assert_eq!(projected.generation, Some(1));
    assert_eq!(projected.name.as_deref(), Some("worker"));
    assert!(
        hub.managed_agent(&agent.agent_id, None, false)
            .expect("managed")
            .observed
    );
}

#[test]
fn run_waiters_receive_published_outcome() {
    let hub = RuntimeHub::new();
    hub.publish(snapshot_with_panes(vec![pane(
        1,
        RuntimeTaskState::Running,
    )]));
    let waiter = {
        let hub = hub.clone();
        std::thread::spawn(move || hub.wait_run(SINGLE_WINDOW_ID, 1, 7, Duration::from_secs(5)))
    };
    // 等待表登记是线程内动作;轮询到登记完成再发布结果。
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if hub
            .lock()
            .run_waiters
            .contains_key(&(SINGLE_WINDOW_ID, 1, 7))
        {
            break;
        }
        assert!(Instant::now() < deadline, "waiter was never registered");
        std::thread::sleep(Duration::from_millis(1));
    }

    let mut completed = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
    completed.windows[0].tabs[0].panes[0].last_run = Some(RuntimeRunOutcome {
        run_id: 7,
        state: RuntimeRunState::Finished,
        exit_code: Some(0),
        exit_code_capability: ExitCodeCapability::Supported,
        unavailable_reason: None,
    });
    hub.publish(completed);
    let result = waiter.join().expect("waiter thread").expect("run result");
    assert_eq!(result.outcome.run_id, 7);
    assert_eq!(result.outcome.state, RuntimeRunState::Finished);
    assert_eq!(result.outcome.exit_code, Some(0));
}

#[test]
fn run_wait_timeout_classifies_current_phase() {
    for (phase, expected) in [
        (Some(RuntimeRunPhase::Submitted), "run_start_timeout"),
        (Some(RuntimeRunPhase::Started), "timeout"),
        (None, "run_aborted"),
    ] {
        let mut snapshot = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Running)]);
        snapshot.windows[0].tabs[0].panes[0].active_run =
            phase.map(|phase| RuntimePaneRun { run_id: 9, phase });
        // 每次换 run 记录,避免已完成缓存跨轮命中。
        let hub = RuntimeHub::new();
        hub.publish(snapshot);
        let error = hub
            .wait_run(SINGLE_WINDOW_ID, 1, 9, Duration::from_millis(10))
            .expect_err("timeout");
        assert_eq!(error.code, expected);
    }
}

#[test]
fn unavailable_run_surfaces_capability_error() {
    let hub = RuntimeHub::new();
    let mut snapshot = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
    snapshot.windows[0].tabs[0].panes[0].last_run = Some(RuntimeRunOutcome::unavailable(
        11,
        "command_start_not_observed",
    ));
    hub.publish(snapshot);
    let error = hub
        .wait_run(SINGLE_WINDOW_ID, 1, 11, Duration::from_secs(1))
        .expect_err("capability error");
    assert_eq!(error.code, "shell_integration_unavailable");
}

#[test]
fn capabilities_and_features_have_no_duplicates() {
    let mut capabilities = CAPABILITIES.to_vec();
    capabilities.sort_unstable();
    capabilities.dedup();
    assert_eq!(capabilities.len(), CAPABILITIES.len());

    let mut features = FEATURES.to_vec();
    features.sort_unstable();
    features.dedup();
    assert_eq!(features.len(), FEATURES.len());
}

const TEST_CODEX_SESSION: &str = "b5f6c1c2-1111-2222-3333-444455556666";

/// 委派夹具:origin pane 3(claude,可无身份)、target pane 4(codex)。
fn delegation_hub(origin_agent: Option<RuntimeAgent>) -> (RuntimeHub, RuntimeManagedAgent) {
    let hub = RuntimeHub::new();
    let target = hub
        .register_agent(
            "worker".to_owned(),
            AgentKind::Codex,
            SINGLE_WINDOW_ID,
            4,
            Some(TEST_CODEX_SESSION.to_owned()),
            None,
        )
        .expect("register target");
    let mut origin = pane(3, RuntimeTaskState::Finished);
    origin.agent = origin_agent;
    let mut target_pane = pane(4, RuntimeTaskState::Finished);
    target_pane.agent = Some(agent_with("codex", Some(TEST_CODEX_SESSION)));
    hub.publish(snapshot_with_panes(vec![origin, target_pane]));
    (hub, target)
}

fn codex_turn_done(message: &str) -> crate::ai_hook::AiHookEvent {
    let payload = json!({
        "type": "agent-turn-complete",
        "thread-id": TEST_CODEX_SESSION,
        "last-assistant-message": message,
    });
    let raw = format!("slterm-hook/1 source=codex pane=4\n{payload}");
    crate::ai_hook::parse_envelope(raw.as_bytes()).expect("codex turn-done event")
}

#[test]
fn delegation_requires_identity_and_allows_one_in_flight_per_generation() {
    let (hub, target) = delegation_hub(Some(agent_with("claude", Some("claude-session-1"))));
    let receipt = hub.begin_delegation(3, &target).expect("delegation");
    assert_eq!(receipt.status, "registered");
    assert_eq!(receipt.origin.pane_id, 3);
    assert_eq!(
        receipt.target.agent_id.as_deref(),
        Some(target.agent_id.as_str())
    );
    assert_eq!(
        hub.begin_delegation(3, &target)
            .expect_err("one in flight per target")
            .code,
        "delegation_in_progress"
    );

    // 不可自委派:origin 与 target 同 pane。
    assert_eq!(
        hub.begin_delegation(4, &target)
            .expect_err("self delegation")
            .code,
        "invalid_params"
    );

    let (no_agent_hub, target) = delegation_hub(None);
    assert_eq!(
        no_agent_hub
            .begin_delegation(3, &target)
            .expect_err("origin not agent")
            .code,
        "origin_not_agent"
    );

    let (anonymous_hub, target) = delegation_hub(Some(agent_with("claude", None)));
    assert_eq!(
        anonymous_hub
            .begin_delegation(3, &target)
            .expect_err("origin without identity")
            .code,
        "origin_identity_unavailable"
    );
}

#[test]
fn delegation_capacity_is_bounded() {
    let (hub, target) = delegation_hub(Some(agent_with("claude", Some("claude-session-1"))));
    {
        let mut state = hub.lock();
        for index in 0..MAX_PENDING_DELEGATIONS {
            state.pending_delegations.insert(
                format!("delegation-seed-{index}"),
                PendingDelegation {
                    receipt: RuntimeDelegationReceipt {
                        task_id: format!("delegation-seed-{index}"),
                        origin: RuntimeDelegationEndpoint {
                            window_id: SINGLE_WINDOW_ID,
                            pane_id: 3,
                            agent_id: None,
                            generation: None,
                            kind: "claude".to_owned(),
                            session_id: Some("claude-session-1".to_owned()),
                        },
                        target: RuntimeDelegationEndpoint {
                            window_id: SINGLE_WINDOW_ID,
                            pane_id: 4,
                            agent_id: Some(format!("seed-{index}")),
                            generation: Some(1),
                            kind: "codex".to_owned(),
                            session_id: Some(TEST_CODEX_SESSION.to_owned()),
                        },
                        status: "registered".to_owned(),
                    },
                    target_name: "seed".to_owned(),
                },
            );
        }
    }
    assert_eq!(
        hub.begin_delegation(3, &target).expect_err("capacity").code,
        "delegation_capacity"
    );
}

#[test]
fn delegation_completion_returns_truncated_worker_output_once() {
    let (hub, target) = delegation_hub(Some(agent_with("claude", Some("claude-session-1"))));
    let receipt = hub.begin_delegation(3, &target).expect("delegation");
    let message = "x".repeat(MAX_DELEGATION_RESULT_CHARS + 500);
    hub.complete_delegations_for_turn(&codex_turn_done(&message));

    let callback = hub
        .take_delegation_callback(&target.agent_id, target.generation)
        .expect("completion callback");
    assert_eq!(callback.task_id, receipt.task_id);
    assert!(callback.prompt.starts_with("[slterm "));
    let encoded = callback
        .prompt
        .split_once("worker_output=")
        .expect("worker_output payload")
        .1;
    let decoded: String = serde_json::from_str(encoded).expect("JSON escaped output");
    assert_eq!(
        decoded.chars().count(),
        MAX_DELEGATION_RESULT_CHARS + 3,
        "kept chars plus the '...' marker"
    );
    // 回传只消费一次。
    assert!(
        hub.take_delegation_callback(&target.agent_id, target.generation)
            .is_none()
    );
}

#[test]
fn delegation_callback_is_dropped_when_origin_identity_changes() {
    let (hub, target) = delegation_hub(Some(agent_with("claude", Some("claude-session-1"))));
    hub.begin_delegation(3, &target).expect("delegation");
    hub.complete_delegations_for_turn(&codex_turn_done("done"));

    let mut replaced = pane(3, RuntimeTaskState::Finished);
    replaced.agent = Some(agent_with("claude", Some("claude-session-2")));
    let mut target_pane = pane(4, RuntimeTaskState::Finished);
    target_pane.agent = Some(agent_with("codex", Some(TEST_CODEX_SESSION)));
    hub.publish(snapshot_with_panes(vec![replaced, target_pane]));

    assert!(
        hub.take_delegation_callback(&target.agent_id, target.generation)
            .is_none(),
        "callback must not move to a replacement origin session"
    );
}

#[test]
fn managed_agent_keeps_worktree_provenance() {
    let hub = RuntimeHub::new();
    let provenance = WorktreeProvenance {
        repo_root: PathBuf::from("D:/repo"),
        source_root: PathBuf::from("D:/repo"),
        path: PathBuf::from("D:/repo-worktrees/reviewer"),
        branch: "slterm/reviewer".into(),
        base_commit: "0123456789012345678901234567890123456789".into(),
        created: true,
    };
    let managed = hub
        .register_agent(
            "reviewer".into(),
            AgentKind::Codex,
            SINGLE_WINDOW_ID,
            1,
            None,
            Some(provenance.clone()),
        )
        .expect("register");
    assert_eq!(managed.worktree.as_ref(), Some(&provenance));
    assert_eq!(
        hub.active_agent(&managed.agent_id, Some(managed.generation))
            .expect("active")
            .worktree
            .as_ref(),
        Some(&provenance)
    );
    let mut observed = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Running)]);
    observed.windows[0].tabs[0].panes[0].agent = Some(agent_with("codex", None));
    let projected = hub.publish(observed);
    assert_eq!(
        projected.windows[0].tabs[0].panes[0]
            .agent
            .as_ref()
            .and_then(|agent| agent.worktree.as_ref()),
        Some(&provenance)
    );
}

#[test]
fn wait_run_phased_reports_run_not_started_when_the_pane_is_quiet() {
    let hub = RuntimeHub::new();
    let mut submitted = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Running)]);
    submitted.windows[0].tabs[0].panes[0].active_run = Some(RuntimePaneRun {
        run_id: 7,
        phase: RuntimeRunPhase::Submitted,
    });
    hub.publish(submitted);
    let sink = responding_sink(|command| match command {
        RuntimeCommand::ReadPane { .. } => Ok(json!({
            "action": { "text": "update available? y/n" }
        })),
        other => panic!("unexpected command: {other:?}"),
    });
    let error = wait_run_phased_with_grace(
        &hub,
        &sink,
        SINGLE_WINDOW_ID,
        1,
        7,
        Duration::from_millis(80),
        Duration::from_millis(10),
    )
    .expect_err("quiet pane must not burn the full timeout");
    assert_eq!(error.code, "run_not_started");
    assert_eq!(
        error.details.expect("evidence")["tail"],
        "update available? y/n"
    );
}

#[test]
fn wait_run_phased_keeps_waiting_while_output_progresses() {
    let hub = RuntimeHub::new();
    let mut submitted = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Running)]);
    submitted.windows[0].tabs[0].panes[0].active_run = Some(RuntimePaneRun {
        run_id: 7,
        phase: RuntimeRunPhase::Submitted,
    });
    hub.publish(submitted);
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sink_counter = counter.clone();
    let sink = responding_sink(move |command| match command {
        RuntimeCommand::ReadPane { .. } => {
            let tick = sink_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(json!({ "action": { "text": format!("progress {tick}") } }))
        }
        other => panic!("unexpected command: {other:?}"),
    });
    let error = wait_run_phased_with_grace(
        &hub,
        &sink,
        SINGLE_WINDOW_ID,
        1,
        7,
        Duration::from_millis(80),
        Duration::from_millis(10),
    )
    .expect_err("still only submitted");
    assert_eq!(
        error.code, "run_start_timeout",
        "progress means keep the original wait semantics"
    );
}

#[test]
fn wait_run_phased_returns_completion_inside_the_grace_window() {
    let hub = RuntimeHub::new();
    let mut submitted = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Running)]);
    submitted.windows[0].tabs[0].panes[0].active_run = Some(RuntimePaneRun {
        run_id: 7,
        phase: RuntimeRunPhase::Submitted,
    });
    hub.publish(submitted);
    let sink = responding_sink(|_| Ok(json!({})));
    let publisher = {
        let hub = hub.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            let mut done = snapshot_with_panes(vec![pane(1, RuntimeTaskState::Idle)]);
            done.windows[0].tabs[0].panes[0].last_run = Some(RuntimeRunOutcome {
                run_id: 7,
                state: RuntimeRunState::Finished,
                exit_code: Some(0),
                exit_code_capability: ExitCodeCapability::Supported,
                unavailable_reason: None,
            });
            hub.publish(done);
        })
    };
    let result = wait_run_phased_with_grace(
        &hub,
        &sink,
        SINGLE_WINDOW_ID,
        1,
        7,
        Duration::from_secs(2),
        Duration::from_millis(500),
    )
    .expect("run completes inside grace");
    publisher.join().expect("publisher");
    assert_eq!(result.outcome.run_id, 7);
    assert_eq!(result.outcome.exit_code, Some(0));
}

impl RuntimeSnapshot {
    /// 测试装配:给唯一 pane 挂 agent 证据。
    fn tap_agent(mut self, agent: RuntimeAgent) -> Self {
        if let Some(pane) = self
            .windows
            .first_mut()
            .and_then(|window| window.tabs.first_mut())
            .and_then(|tab| tab.panes.first_mut())
        {
            pane.agent = Some(agent);
        }
        self
    }
}
