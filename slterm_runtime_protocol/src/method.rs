//! 方法注册表：路由、能力摘要与 schema 生成的同源事实。
//!
//! 只登记真实可用方法；新增方法必须先在此登记作用域、读写类别与
//! 参数／结果 schema，再由服务器与客户端接入。

use serde::{Deserialize, Serialize};

use crate::protocol::ControlMethod;
use crate::protocol::ControlScope;

/// 方法读写类别：查询无副作用，写操作必须携带幂等语义。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MethodKind {
    Query,
    Mutation,
}

/// 单个方法的线缆身份、可用作用域与 schema 入口。
pub struct MethodSpec {
    pub method: ControlMethod,
    /// 允许的作用域；hello 同时服务 Runtime 与 handover 两个作用域。
    pub scopes: &'static [ControlScope],
    pub kind: MethodKind,
    /// 写操作在相同幂等键与参数下可安全重试。
    pub idempotent: bool,
    #[cfg(feature = "schema")]
    pub params_schema: fn() -> schemars::Schema,
    #[cfg(feature = "schema")]
    pub result_schema: fn() -> schemars::Schema,
}

#[cfg(feature = "schema")]
fn hello_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::HelloParams)
}

#[cfg(feature = "schema")]
fn hello_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::HelloResponse)
}

#[cfg(feature = "schema")]
fn handover_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::HandoverParams)
}

#[cfg(feature = "schema")]
fn handover_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::HandoverResponse)
}

#[cfg(feature = "schema")]
fn empty_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::EmptyParams)
}

#[cfg(feature = "schema")]
fn describe_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::DescribeResponse)
}

#[cfg(feature = "schema")]
fn snapshot_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::SnapshotResponse)
}

#[cfg(feature = "schema")]
fn operation_query_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::OperationQueryParams)
}

#[cfg(feature = "schema")]
fn operation_wait_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::OperationWaitParams)
}

#[cfg(feature = "schema")]
fn operation_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::OperationResource)
}

#[cfg(feature = "schema")]
fn events_subscribe_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::EventsSubscribeParams)
}

#[cfg(feature = "schema")]
fn events_subscribed_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::EventsSubscribedResponse)
}

#[cfg(feature = "schema")]
fn events_unsubscribed_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::EventsUnsubscribedResponse)
}

#[cfg(feature = "schema")]
fn project_activate_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ProjectActivateParams)
}

#[cfg(feature = "schema")]
fn project_rename_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ProjectRenameParams)
}

#[cfg(feature = "schema")]
fn project_reorder_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ProjectReorderParams)
}

#[cfg(feature = "schema")]
fn project_remove_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ProjectRemoveParams)
}

#[cfg(feature = "schema")]
fn project_list_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::ProjectListResponse)
}

#[cfg(feature = "schema")]
fn panel_group_split_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelGroupSplitParams)
}

#[cfg(feature = "schema")]
fn panel_group_merge_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelGroupMergeParams)
}

#[cfg(feature = "schema")]
fn panel_group_resize_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelGroupResizeParams)
}

#[cfg(feature = "schema")]
fn panel_activate_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelActivateParams)
}

#[cfg(feature = "schema")]
fn panel_move_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelMoveParams)
}

#[cfg(feature = "schema")]
fn panel_open_settings_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::PanelOpenSettingsParams)
}

#[cfg(feature = "schema")]
fn terminal_create_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalCreateParams)
}

#[cfg(feature = "schema")]
fn terminal_input_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalInputParams)
}

#[cfg(feature = "schema")]
fn terminal_execute_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalExecuteParams)
}

#[cfg(feature = "schema")]
fn terminal_read_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalReadRequest)
}

#[cfg(feature = "schema")]
fn terminal_read_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalReadResponse)
}

#[cfg(feature = "schema")]
fn terminal_wait_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalWaitRequest)
}

#[cfg(feature = "schema")]
fn terminal_wait_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalWaitResponse)
}

#[cfg(feature = "schema")]
fn terminal_close_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalCloseParams)
}

#[cfg(feature = "schema")]
fn terminal_reconnect_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalReconnectParams)
}

#[cfg(feature = "schema")]
fn terminal_rename_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::TerminalRenameParams)
}

#[cfg(feature = "schema")]
fn agent_get_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentGetRequest)
}

#[cfg(feature = "schema")]
fn agent_input_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentInputParams)
}

#[cfg(feature = "schema")]
fn agent_close_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentCloseParams)
}

#[cfg(feature = "schema")]
fn agent_read_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentReadRequest)
}

#[cfg(feature = "schema")]
fn agent_wait_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentWaitRequest)
}

#[cfg(feature = "schema")]
fn agent_start_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentStartParams)
}

#[cfg(feature = "schema")]
fn agent_resume_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentResumeParams)
}

#[cfg(feature = "schema")]
fn agent_fork_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentForkParams)
}

#[cfg(feature = "schema")]
fn git_status_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitStatusRequest)
}

#[cfg(feature = "schema")]
fn git_status_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitStatusResponse)
}

#[cfg(feature = "schema")]
fn git_diff_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitDiffRequest)
}

#[cfg(feature = "schema")]
fn git_diff_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitDiffResponse)
}

#[cfg(feature = "schema")]
fn git_rollback_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitRollbackParams)
}

#[cfg(feature = "schema")]
fn git_delete_params_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::GitDeleteParams)
}

#[cfg(feature = "schema")]
fn agent_list_result_schema() -> schemars::Schema {
    schemars::schema_for!(crate::protocol::AgentListResponse)
}

/// 全部真实可用方法；顺序即对外呈现顺序。
pub static METHODS: &[MethodSpec] = &[
    MethodSpec {
        method: ControlMethod::RuntimeHello,
        scopes: &[ControlScope::Runtime, ControlScope::Handover],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: hello_params_schema,
        #[cfg(feature = "schema")]
        result_schema: hello_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeDescribe,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: empty_params_schema,
        #[cfg(feature = "schema")]
        result_schema: describe_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeSnapshot,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: empty_params_schema,
        #[cfg(feature = "schema")]
        result_schema: snapshot_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeOperationGet,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: operation_query_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeOperationWait,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: operation_wait_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeEventsSubscribe,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: events_subscribe_params_schema,
        #[cfg(feature = "schema")]
        result_schema: events_subscribed_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeEventsUnsubscribe,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: empty_params_schema,
        #[cfg(feature = "schema")]
        result_schema: events_unsubscribed_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeProjectList,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: empty_params_schema,
        #[cfg(feature = "schema")]
        result_schema: project_list_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeProjectActivate,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: project_activate_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeProjectRename,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: project_rename_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeProjectReorder,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: project_reorder_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeProjectRemove,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: project_remove_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelGroupSplit,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_group_split_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelGroupMerge,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_group_merge_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelGroupResize,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_group_resize_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelActivate,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_activate_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelMove,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_move_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimePanelOpenSettings,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: panel_open_settings_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalCreate,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_create_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalInput,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_input_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalExecute,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_execute_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalRead,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_read_params_schema,
        #[cfg(feature = "schema")]
        result_schema: terminal_read_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalWait,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_wait_params_schema,
        #[cfg(feature = "schema")]
        result_schema: terminal_wait_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalClose,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_close_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalReconnect,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_reconnect_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeTerminalRename,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: terminal_rename_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentList,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: empty_params_schema,
        #[cfg(feature = "schema")]
        result_schema: agent_list_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentGet,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_get_params_schema,
        #[cfg(feature = "schema")]
        result_schema: agent_list_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentInput,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_input_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentClose,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_close_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentRead,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_read_params_schema,
        #[cfg(feature = "schema")]
        result_schema: terminal_read_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentWait,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_wait_params_schema,
        #[cfg(feature = "schema")]
        result_schema: terminal_wait_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentStart,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_start_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentResume,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_resume_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeAgentFork,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: agent_fork_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeGitStatus,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: git_status_params_schema,
        #[cfg(feature = "schema")]
        result_schema: git_status_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeGitDiff,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Query,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: git_diff_params_schema,
        #[cfg(feature = "schema")]
        result_schema: git_diff_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeGitRollback,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: git_rollback_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::RuntimeGitDelete,
        scopes: &[ControlScope::Runtime],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: git_delete_params_schema,
        #[cfg(feature = "schema")]
        result_schema: operation_result_schema,
    },
    MethodSpec {
        method: ControlMethod::InstanceHandover,
        scopes: &[ControlScope::Handover],
        kind: MethodKind::Mutation,
        idempotent: true,
        #[cfg(feature = "schema")]
        params_schema: handover_params_schema,
        #[cfg(feature = "schema")]
        result_schema: handover_result_schema,
    },
];

pub fn method_spec(method: ControlMethod) -> &'static MethodSpec {
    METHODS
        .iter()
        .find(|spec| spec.method == method)
        .expect("每个线缆方法必须已登记")
}

/// 线缆方法名 → 注册条目；未登记方法返回 None。
pub fn lookup(name: &str) -> Option<&'static MethodSpec> {
    METHODS.iter().find(|spec| spec.method.as_str() == name)
}

pub fn methods_for(scope: ControlScope) -> impl Iterator<Item = &'static MethodSpec> {
    METHODS
        .iter()
        .filter(move |spec| spec.scopes.contains(&scope))
}

pub fn method_names() -> impl Iterator<Item = &'static str> {
    METHODS.iter().map(|spec| spec.method.as_str())
}

pub fn allows(method: ControlMethod, scope: ControlScope) -> bool {
    method_spec(method).scopes.contains(&scope)
}

#[cfg(test)]
mod method_tests {
    use super::{METHODS, MethodKind, allows, lookup, method_names, methods_for};
    use crate::ControlScope;
    use crate::protocol::{ControlMethod, HandoverMethod, HelloMethod};

    #[test]
    fn registry_covers_every_wire_method_exactly_once() {
        assert_eq!(METHODS.len(), ControlMethod::ALL.len());
        for method in ControlMethod::ALL {
            let registrations = METHODS.iter().filter(|spec| spec.method == method).count();
            assert_eq!(registrations, 1, "{method:?} 必须恰好登记一次");
        }
        let names: Vec<&str> = method_names().collect();
        assert_eq!(
            names,
            vec![
                method_name(ControlMethod::RuntimeHello),
                method_name(ControlMethod::RuntimeDescribe),
                method_name(ControlMethod::RuntimeSnapshot),
                method_name(ControlMethod::RuntimeOperationGet),
                method_name(ControlMethod::RuntimeOperationWait),
                method_name(ControlMethod::RuntimeEventsSubscribe),
                method_name(ControlMethod::RuntimeEventsUnsubscribe),
                method_name(ControlMethod::RuntimeProjectList),
                method_name(ControlMethod::RuntimeProjectActivate),
                method_name(ControlMethod::RuntimeProjectRename),
                method_name(ControlMethod::RuntimeProjectReorder),
                method_name(ControlMethod::RuntimeProjectRemove),
                method_name(ControlMethod::RuntimePanelGroupSplit),
                method_name(ControlMethod::RuntimePanelGroupMerge),
                method_name(ControlMethod::RuntimePanelGroupResize),
                method_name(ControlMethod::RuntimePanelActivate),
                method_name(ControlMethod::RuntimePanelMove),
                method_name(ControlMethod::RuntimePanelOpenSettings),
                method_name(ControlMethod::RuntimeTerminalCreate),
                method_name(ControlMethod::RuntimeTerminalInput),
                method_name(ControlMethod::RuntimeTerminalExecute),
                method_name(ControlMethod::RuntimeTerminalRead),
                method_name(ControlMethod::RuntimeTerminalWait),
                method_name(ControlMethod::RuntimeTerminalClose),
                method_name(ControlMethod::RuntimeTerminalReconnect),
                method_name(ControlMethod::RuntimeTerminalRename),
                method_name(ControlMethod::RuntimeAgentList),
                method_name(ControlMethod::RuntimeAgentGet),
                method_name(ControlMethod::RuntimeAgentInput),
                method_name(ControlMethod::RuntimeAgentClose),
                method_name(ControlMethod::RuntimeAgentRead),
                method_name(ControlMethod::RuntimeAgentWait),
                method_name(ControlMethod::RuntimeAgentStart),
                method_name(ControlMethod::RuntimeAgentResume),
                method_name(ControlMethod::RuntimeAgentFork),
                method_name(ControlMethod::RuntimeGitStatus),
                method_name(ControlMethod::RuntimeGitDiff),
                method_name(ControlMethod::RuntimeGitRollback),
                method_name(ControlMethod::RuntimeGitDelete),
                method_name(ControlMethod::InstanceHandover),
            ]
        );
    }

    fn method_name(method: ControlMethod) -> &'static str {
        method.as_str()
    }

    #[test]
    fn wire_names_match_serde_method_enums() {
        assert_eq!(
            method_name(ControlMethod::RuntimeHello),
            serde_json::to_value(HelloMethod::RuntimeHello)
                .unwrap()
                .as_str()
                .unwrap()
        );
        assert_eq!(
            method_name(ControlMethod::InstanceHandover),
            serde_json::to_value(HandoverMethod::InstanceHandover)
                .unwrap()
                .as_str()
                .unwrap()
        );
    }

    #[test]
    fn registry_scopes_and_kinds_match_the_contract() {
        assert!(allows(ControlMethod::RuntimeHello, ControlScope::Runtime));
        assert!(allows(ControlMethod::RuntimeHello, ControlScope::Handover));
        assert!(allows(
            ControlMethod::InstanceHandover,
            ControlScope::Handover
        ));
        assert!(!allows(
            ControlMethod::InstanceHandover,
            ControlScope::Runtime
        ));

        assert_eq!(
            lookup("instance.handover").unwrap().method,
            ControlMethod::InstanceHandover
        );
        assert_eq!(
            lookup("runtime.hello").unwrap().method,
            ControlMethod::RuntimeHello
        );
        assert_eq!(
            lookup("runtime.describe").unwrap().method,
            ControlMethod::RuntimeDescribe
        );
        assert_eq!(
            lookup("runtime.snapshot").unwrap().method,
            ControlMethod::RuntimeSnapshot
        );
        assert!(lookup("runtime.unknown").is_none(), "未实现方法不得登记");
        assert!(!allows(
            ControlMethod::RuntimeDescribe,
            ControlScope::Handover
        ));
        assert!(!allows(
            ControlMethod::RuntimeSnapshot,
            ControlScope::Handover
        ));

        let handover = lookup("instance.handover").unwrap();
        assert_eq!(handover.kind, MethodKind::Mutation);
        assert_eq!(lookup("runtime.hello").unwrap().kind, MethodKind::Query);

        let runtime_methods: Vec<ControlMethod> = methods_for(ControlScope::Runtime)
            .map(|spec| spec.method)
            .collect();
        assert_eq!(
            runtime_methods,
            vec![
                ControlMethod::RuntimeHello,
                ControlMethod::RuntimeDescribe,
                ControlMethod::RuntimeSnapshot,
                ControlMethod::RuntimeOperationGet,
                ControlMethod::RuntimeOperationWait,
                ControlMethod::RuntimeEventsSubscribe,
                ControlMethod::RuntimeEventsUnsubscribe,
                ControlMethod::RuntimeProjectList,
                ControlMethod::RuntimeProjectActivate,
                ControlMethod::RuntimeProjectRename,
                ControlMethod::RuntimeProjectReorder,
                ControlMethod::RuntimeProjectRemove,
                ControlMethod::RuntimePanelGroupSplit,
                ControlMethod::RuntimePanelGroupMerge,
                ControlMethod::RuntimePanelGroupResize,
                ControlMethod::RuntimePanelActivate,
                ControlMethod::RuntimePanelMove,
                ControlMethod::RuntimePanelOpenSettings,
                ControlMethod::RuntimeTerminalCreate,
                ControlMethod::RuntimeTerminalInput,
                ControlMethod::RuntimeTerminalExecute,
                ControlMethod::RuntimeTerminalRead,
                ControlMethod::RuntimeTerminalWait,
                ControlMethod::RuntimeTerminalClose,
                ControlMethod::RuntimeTerminalReconnect,
                ControlMethod::RuntimeTerminalRename,
                ControlMethod::RuntimeAgentList,
                ControlMethod::RuntimeAgentGet,
                ControlMethod::RuntimeAgentInput,
                ControlMethod::RuntimeAgentClose,
                ControlMethod::RuntimeAgentRead,
                ControlMethod::RuntimeAgentWait,
                ControlMethod::RuntimeAgentStart,
                ControlMethod::RuntimeAgentResume,
                ControlMethod::RuntimeAgentFork,
                ControlMethod::RuntimeGitStatus,
                ControlMethod::RuntimeGitDiff,
                ControlMethod::RuntimeGitRollback,
                ControlMethod::RuntimeGitDelete,
            ]
        );
        let handover_methods: Vec<ControlMethod> = methods_for(ControlScope::Handover)
            .map(|spec| spec.method)
            .collect();
        assert_eq!(
            handover_methods,
            vec![ControlMethod::RuntimeHello, ControlMethod::InstanceHandover]
        );
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schema_functions_cover_every_registered_method() {
        for spec in METHODS {
            let params = (spec.params_schema)();
            let result = (spec.result_schema)();
            assert!(
                serde_json::to_value(&params).unwrap().is_object(),
                "{:?} 参数 schema 必须是对象",
                spec.method
            );
            assert!(
                serde_json::to_value(&result).unwrap().is_object(),
                "{:?} 结果 schema 必须是对象",
                spec.method
            );
        }
    }
}
