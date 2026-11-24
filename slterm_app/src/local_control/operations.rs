//! Runtime 写操作派发：参数散列、持久幂等登记、执行与终态推进。
//!
//! 全部持久事实只经唯一工作区写线程；派发在后台 worker 内同步等待有界回执。

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde_json::Value;
use slterm_runtime_protocol::{
    DomainErrorCode, JsonRpcError, OPERATION_WAIT_LIMIT_MS, OperationResource, OperationStatus,
    params_sha256,
};

use crate::persistence::operation::{
    Operation, OperationError, OperationRegistration, RegistrationOutcome,
};
use crate::persistence::workspace_writer::{WorkspaceWriterClient, WorkspaceWriterError};

/// 写线程即时回执的有界等待；worker 可阻塞，前台不可调用本模块。
const WRITER_REPLY_TIMEOUT: Duration = Duration::from_secs(4);
/// 等待查询的轮询间隔与协议 wait 上限一致。
const WAIT_POLL: Duration = Duration::from_millis(100);
/// 当前写方法族的参数 DTO 版本；字段变化才递增。
const PARAMS_DTO_VERSION: u64 = 1;

/// 写操作派发输入；参数散列由协议 crate 统一计算。
pub(crate) struct DispatchRequest<'a> {
    pub(crate) client_id: &'a str,
    pub(crate) mutation_id: &'a str,
    pub(crate) method: &'a str,
    pub(crate) params: &'a Value,
    /// 调用方期望的资源代次；与执行前复核值不一致即拒绝。
    pub(crate) expected_revision: Option<u64>,
    /// 执行前复核得到的当前资源代次。
    pub(crate) current_revision: u64,
    /// 授权与资源代次快照；随 Operation 持久登记。
    pub(crate) resource_generations: BTreeMap<String, u64>,
}

/// 执行体的封闭终态；无法证明副作用的操作必须返回 OutcomeUnknown。
pub(crate) enum DispatchOutcome {
    Succeeded(Value),
    Failed(Value),
    OutcomeUnknown(Value),
}

/// 唯一写线程之上的派发器；每个服务实例持有自己的 store_epoch 视图。
pub(crate) struct OperationDispatcher {
    writer: WorkspaceWriterClient,
    store_epoch: String,
}

impl OperationDispatcher {
    pub(crate) fn new(writer: WorkspaceWriterClient, store_epoch: impl Into<String>) -> Self {
        Self {
            writer,
            store_epoch: store_epoch.into(),
        }
    }

    /// 登记 → Running → 执行 → 终态。
    ///
    /// 已有相同幂等键与参数散列时直接返回原记录，执行体不会再次运行；
    /// `expected_revision` 失配在登记前拒绝，不消耗 mutation_id。
    pub(crate) fn dispatch(
        &self,
        request: DispatchRequest<'_>,
        execute: impl FnOnce() -> DispatchOutcome,
    ) -> Result<OperationResource, JsonRpcError> {
        if let Some(expected) = request.expected_revision
            && expected != request.current_revision
        {
            return Err(control_error(
                DomainErrorCode::RevisionConflict,
                -32005,
                "revision conflict",
            ));
        }
        let registration = OperationRegistration {
            operation_id: uuid::Uuid::now_v7().to_string(),
            store_epoch: self.store_epoch.clone(),
            client_id: request.client_id.to_owned(),
            mutation_id: request.mutation_id.to_owned(),
            method: request.method.to_owned(),
            params_dto_version: PARAMS_DTO_VERSION,
            params_hash: params_sha256(request.params),
            resource_generations: request.resource_generations,
        };
        let registered = self.await_writer(self.writer.register_operation(registration))?;
        let operation = match registered {
            RegistrationOutcome::Existing(operation) => {
                return Ok(operation_resource(operation));
            }
            RegistrationOutcome::Registered(operation) => operation,
        };

        let running = self.await_writer(self.writer.transition_operation(
            &operation.operation_id,
            OperationStatus::Running,
            None,
            None,
        ))?;
        debug_assert_eq!(running.status, OperationStatus::Running);

        let (next, result, error) = match execute() {
            DispatchOutcome::Succeeded(result) => (OperationStatus::Succeeded, Some(result), None),
            DispatchOutcome::Failed(error) => (OperationStatus::Failed, None, Some(error)),
            DispatchOutcome::OutcomeUnknown(error) => {
                (OperationStatus::OutcomeUnknown, None, Some(error))
            }
        };
        let completed = self.await_writer(self.writer.transition_operation(
            &operation.operation_id,
            next,
            result,
            error,
        ))?;
        Ok(operation_resource(completed))
    }

    /// 只读查询持久结果；不存在返回 None，不猜测状态。
    pub(crate) fn query(
        &self,
        operation_id: &str,
    ) -> Result<Option<OperationResource>, JsonRpcError> {
        let operation = self.await_writer(self.writer.query_operation(operation_id))?;
        Ok(operation.map(operation_resource))
    }

    /// 有界等待终态；超时由调用方映射结构化错误，终态立即返回。
    pub(crate) fn wait(
        &self,
        operation_id: &str,
        timeout: Duration,
    ) -> Result<Option<OperationResource>, JsonRpcError> {
        let timeout = timeout.min(Duration::from_millis(OPERATION_WAIT_LIMIT_MS));
        let deadline = Instant::now() + timeout;
        loop {
            match self.query(operation_id)? {
                Some(resource) if resource.status.is_terminal() => return Ok(Some(resource)),
                Some(_) => {}
                None => return Ok(None),
            }
            if Instant::now() >= deadline {
                return Err(control_error(
                    DomainErrorCode::NotReady,
                    -32001,
                    "operation wait timed out",
                ));
            }
            std::thread::sleep(WAIT_POLL);
        }
    }

    fn await_writer<T>(
        &self,
        receipt: Result<Receiver<Result<T, OperationError>>, WorkspaceWriterError>,
    ) -> Result<T, JsonRpcError> {
        let receipt = receipt.map_err(writer_error)?;
        match receipt.recv_timeout(WRITER_REPLY_TIMEOUT) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(operation_error(error)),
            Err(RecvTimeoutError::Timeout) => Err(control_error(
                DomainErrorCode::NotReady,
                -32001,
                "operation store not ready",
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err(control_error(DomainErrorCode::Closing, -32004, "closing"))
            }
        }
    }
}

/// 写线程提交失败映射为封闭领域错误；不携带存储细节。
pub(super) fn writer_error(error: WorkspaceWriterError) -> JsonRpcError {
    match error {
        WorkspaceWriterError::QueueFull => {
            control_error(DomainErrorCode::QueueFull, -32003, "writer queue full")
        }
        WorkspaceWriterError::Stopped => control_error(DomainErrorCode::Closing, -32004, "closing"),
        _ => control_error(
            DomainErrorCode::NotReady,
            -32001,
            "operation store unavailable",
        ),
    }
}

/// 持久记录到线缆视图：只暴露身份、状态、结果与错误。
pub(super) fn operation_resource(operation: Operation) -> OperationResource {
    OperationResource {
        operation_id: operation.operation_id,
        status: operation.status,
        result: operation.result,
        error: operation.error,
    }
}

fn operation_error(error: OperationError) -> JsonRpcError {
    match error {
        OperationError::MutationConflict => control_error(
            DomainErrorCode::MutationConflict,
            -32006,
            "mutation conflict",
        ),
        OperationError::StoreEpochConflict => {
            control_error(DomainErrorCode::NotReady, -32001, "store epoch conflict")
        }
        _ => control_error(
            DomainErrorCode::NotReady,
            -32001,
            "operation store unavailable",
        ),
    }
}

fn control_error(domain: DomainErrorCode, numeric: i32, message: &'static str) -> JsonRpcError {
    JsonRpcError::new(
        numeric,
        message,
        Some(slterm_runtime_protocol::JsonRpcErrorData { code: domain }),
    )
}
