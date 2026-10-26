//! 持久 Operation 的幂等登记、查询与状态转换。

use std::collections::BTreeMap;
use std::fmt;

use rusqlite::{Connection, OptionalExtension, Params, Row, TransactionBehavior, params};
use serde_json::Value;
use slterm_runtime_protocol::{InvalidOperationTransition, OperationStatus};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OperationRegistration {
    pub(crate) operation_id: String,
    pub(crate) store_epoch: String,
    pub(crate) client_id: String,
    pub(crate) mutation_id: String,
    pub(crate) method: String,
    pub(crate) params_dto_version: u64,
    pub(crate) params_hash: [u8; 32],
    pub(crate) resource_generations: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Operation {
    pub(crate) operation_id: String,
    pub(crate) store_epoch: String,
    pub(crate) client_id: String,
    pub(crate) mutation_id: String,
    pub(crate) method: String,
    pub(crate) params_dto_version: u64,
    pub(crate) params_hash: [u8; 32],
    pub(crate) resource_generations: BTreeMap<String, u64>,
    pub(crate) status: OperationStatus,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<Value>,
    pub(crate) journal_id: Option<String>,
}

impl Operation {
    fn from_registration(registration: OperationRegistration) -> Self {
        Self {
            operation_id: registration.operation_id,
            store_epoch: registration.store_epoch,
            client_id: registration.client_id,
            mutation_id: registration.mutation_id,
            method: registration.method,
            params_dto_version: registration.params_dto_version,
            params_hash: registration.params_hash,
            resource_generations: registration.resource_generations,
            status: OperationStatus::Queued,
            result: None,
            error: None,
            journal_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RegistrationOutcome {
    Registered(Operation),
    Existing(Operation),
}

impl RegistrationOutcome {
    pub(crate) fn operation(self) -> Operation {
        match self {
            Self::Registered(operation) | Self::Existing(operation) => operation,
        }
    }
}

#[derive(Debug)]
pub(crate) enum OperationError {
    InvalidRegistration,
    MutationConflict,
    OperationIdConflict,
    UnknownOperation,
    InvalidTransition(InvalidOperationTransition),
    InvalidPayload,
    CorruptRecord,
    Database(DatabaseError),
}

impl fmt::Display for OperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRegistration => "Operation 登记字段无效",
            Self::MutationConflict => "幂等键已绑定其他参数",
            Self::OperationIdConflict => "Operation ID 已绑定其他幂等键",
            Self::UnknownOperation => "Operation 不存在",
            Self::InvalidTransition(_) => "Operation 状态转换无效",
            Self::InvalidPayload => "Operation 状态与结果负载不一致",
            Self::CorruptRecord => "Operation 持久记录损坏",
            Self::Database(_) => "Operation 数据库操作失败",
        })
    }
}

impl std::error::Error for OperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTransition(error) => Some(error),
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<InvalidOperationTransition> for OperationError {
    fn from(error: InvalidOperationTransition) -> Self {
        Self::InvalidTransition(error)
    }
}

impl From<DatabaseError> for OperationError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for OperationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    pub(crate) fn register_operation(
        &mut self,
        registration: OperationRegistration,
    ) -> Result<RegistrationOutcome, OperationError> {
        validate_registration(&registration)?;
        let operation = Operation::from_registration(registration);
        let resource_generations = serde_json::to_string(&operation.resource_generations)
            .map_err(|_| OperationError::InvalidRegistration)?;
        let params_dto_version = i64::try_from(operation.params_dto_version)
            .map_err(|_| OperationError::InvalidRegistration)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = query_operation(
            &transaction,
            "WHERE store_epoch = ?1 AND client_id = ?2 AND mutation_id = ?3",
            params![
                operation.store_epoch,
                operation.client_id,
                operation.mutation_id
            ],
        )?;
        if let Some(existing) = existing {
            if existing.method != operation.method
                || existing.params_dto_version != operation.params_dto_version
                || existing.params_hash != operation.params_hash
            {
                return Err(OperationError::MutationConflict);
            }
            transaction.commit()?;
            return Ok(RegistrationOutcome::Existing(existing));
        }

        let id_conflict = transaction.query_row(
            "SELECT COUNT(*) FROM runtime_operations WHERE operation_id = ?1",
            [&operation.operation_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if id_conflict {
            return Err(OperationError::OperationIdConflict);
        }
        transaction.execute(
            "INSERT INTO runtime_operations \
             (operation_id, store_epoch, client_id, mutation_id, method, params_dto_version, \
              params_hash, resource_generations_json, status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued')",
            params![
                operation.operation_id,
                operation.store_epoch,
                operation.client_id,
                operation.mutation_id,
                operation.method,
                params_dto_version,
                operation.params_hash.as_slice(),
                resource_generations,
            ],
        )?;
        transaction.commit()?;
        Ok(RegistrationOutcome::Registered(operation))
    }

    pub(crate) fn operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<Operation>, OperationError> {
        query_operation(
            &self.database.connection,
            "WHERE operation_id = ?1",
            [operation_id],
        )
    }

    pub(crate) fn transition_operation(
        &mut self,
        operation_id: &str,
        next: OperationStatus,
        result: Option<Value>,
        error: Option<Value>,
    ) -> Result<Operation, OperationError> {
        validate_payload(next, &result, &error)?;
        let result_json = result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| OperationError::InvalidPayload)?;
        let error_json = error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| OperationError::InvalidPayload)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut operation =
            query_operation(&transaction, "WHERE operation_id = ?1", [operation_id])?
                .ok_or(OperationError::UnknownOperation)?;
        operation.status.transition(next)?;
        let changed = transaction.execute(
            "UPDATE runtime_operations \
             SET status = ?2, result_json = ?3, error_json = ?4 \
             WHERE operation_id = ?1 AND status = ?5",
            params![
                operation_id,
                status_name(next),
                result_json,
                error_json,
                status_name(operation.status),
            ],
        )?;
        if changed != 1 {
            return Err(OperationError::CorruptRecord);
        }
        transaction.commit()?;
        operation.status = next;
        operation.result = result;
        operation.error = error;
        Ok(operation)
    }
}

fn validate_registration(registration: &OperationRegistration) -> Result<(), OperationError> {
    if registration.operation_id.is_empty()
        || registration.store_epoch.is_empty()
        || registration.client_id.is_empty()
        || registration.mutation_id.is_empty()
        || registration.method.is_empty()
        || registration.params_dto_version == 0
        || registration
            .resource_generations
            .keys()
            .any(String::is_empty)
    {
        return Err(OperationError::InvalidRegistration);
    }
    Ok(())
}

fn validate_payload(
    next: OperationStatus,
    result: &Option<Value>,
    error: &Option<Value>,
) -> Result<(), OperationError> {
    let valid = match next {
        OperationStatus::Running => result.is_none() && error.is_none(),
        OperationStatus::Succeeded => error.is_none(),
        OperationStatus::Failed | OperationStatus::Cancelled | OperationStatus::OutcomeUnknown => {
            result.is_none()
        }
        OperationStatus::Queued => true,
    };
    if valid {
        Ok(())
    } else {
        Err(OperationError::InvalidPayload)
    }
}

fn query_operation<P: Params>(
    connection: &Connection,
    predicate: &str,
    params: P,
) -> Result<Option<Operation>, OperationError> {
    let sql = format!(
        "SELECT operation_id, store_epoch, client_id, mutation_id, method, params_dto_version, \
         params_hash, resource_generations_json, status, result_json, error_json, journal_id \
         FROM runtime_operations {predicate}"
    );
    connection
        .query_row(&sql, params, |row| Ok(decode_operation(row)))
        .optional()?
        .transpose()
}

fn decode_operation(row: &Row<'_>) -> Result<Operation, OperationError> {
    let version = required::<i64>(row, 5)?;
    let params_dto_version = u64::try_from(version).map_err(|_| OperationError::CorruptRecord)?;
    let hash = required::<Vec<u8>>(row, 6)?;
    let params_hash: [u8; 32] = hash.try_into().map_err(|_| OperationError::CorruptRecord)?;
    let resource_generations = serde_json::from_str(&required::<String>(row, 7)?)
        .map_err(|_| OperationError::CorruptRecord)?;
    let status = parse_status(&required::<String>(row, 8)?)?;
    let result = required::<Option<String>>(row, 9)?
        .map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|_| OperationError::CorruptRecord)?;
    let error = required::<Option<String>>(row, 10)?
        .map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|_| OperationError::CorruptRecord)?;
    Ok(Operation {
        operation_id: required(row, 0)?,
        store_epoch: required(row, 1)?,
        client_id: required(row, 2)?,
        mutation_id: required(row, 3)?,
        method: required(row, 4)?,
        params_dto_version,
        params_hash,
        resource_generations,
        status,
        result,
        error,
        journal_id: required(row, 11)?,
    })
}

fn required<T: rusqlite::types::FromSql>(row: &Row<'_>, index: usize) -> Result<T, OperationError> {
    row.get(index).map_err(|_| OperationError::CorruptRecord)
}

fn status_name(status: OperationStatus) -> &'static str {
    match status {
        OperationStatus::Queued => "queued",
        OperationStatus::Running => "running",
        OperationStatus::Succeeded => "succeeded",
        OperationStatus::Failed => "failed",
        OperationStatus::Cancelled => "cancelled",
        OperationStatus::OutcomeUnknown => "outcome_unknown",
    }
}

fn parse_status(text: &str) -> Result<OperationStatus, OperationError> {
    OperationStatus::ALL
        .into_iter()
        .find(|status| status_name(*status) == text)
        .ok_or(OperationError::CorruptRecord)
}

#[cfg(test)]
mod operation_tests {
    use serde_json::json;

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn registration(
        operation_id: &str,
        store_epoch: &str,
        mutation_id: &str,
    ) -> OperationRegistration {
        OperationRegistration {
            operation_id: operation_id.into(),
            store_epoch: store_epoch.into(),
            client_id: "client-test".into(),
            mutation_id: mutation_id.into(),
            method: "panel.save".into(),
            params_dto_version: 1,
            params_hash: [9; 32],
            resource_generations: BTreeMap::from([("document:alpha".into(), u64::MAX)]),
        }
    }

    fn operation_count(store: &WorkspaceDatabase) -> i64 {
        store
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM runtime_operations", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn operation_registration_survives_reopen_without_float_precision_loss() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let _ = store
            .register_operation(registration("operation-a", "epoch-a", "mutation-a"))
            .unwrap();
        drop(store);

        let store = WorkspaceDatabase::open(&paths).unwrap();
        let loaded = store.operation("operation-a").unwrap();

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.store_epoch, "epoch-a");
        assert_eq!(loaded.client_id, "client-test");
        assert_eq!(loaded.mutation_id, "mutation-a");
        assert_eq!(loaded.method, "panel.save");
        assert_eq!(loaded.params_dto_version, 1);
        assert_eq!(loaded.params_hash, [9; 32]);
        assert_eq!(
            loaded.resource_generations.get("document:alpha"),
            Some(&u64::MAX)
        );
        assert_eq!(loaded.status, OperationStatus::Queued);
        assert_eq!(loaded.result, None);
        assert_eq!(loaded.error, None);
        assert_eq!(loaded.journal_id, None);
    }

    #[test]
    fn operation_registration_returns_existing_and_rejects_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let first = store
            .register_operation(registration("operation-a", "epoch-a", "mutation-a"))
            .unwrap();
        assert!(matches!(first, RegistrationOutcome::Registered(_)));

        let mut retry = registration("operation-b", "epoch-a", "mutation-a");
        retry.resource_generations.clear();
        let retry = store.register_operation(retry).unwrap();
        assert!(matches!(retry, RegistrationOutcome::Existing(_)));
        assert_eq!(retry.operation().operation_id, "operation-a");
        assert_eq!(operation_count(&store), 1);

        let conflict = OperationRegistration {
            params_hash: [10; 32],
            ..registration("operation-c", "epoch-a", "mutation-a")
        };
        assert!(matches!(
            store.register_operation(conflict),
            Err(OperationError::MutationConflict)
        ));
        let conflict = OperationRegistration {
            params_dto_version: 2,
            ..registration("operation-d", "epoch-a", "mutation-a")
        };
        assert!(matches!(
            store.register_operation(conflict),
            Err(OperationError::MutationConflict)
        ));
        let conflict = OperationRegistration {
            method: "terminal.input".into(),
            ..registration("operation-e", "epoch-a", "mutation-a")
        };
        assert!(matches!(
            store.register_operation(conflict),
            Err(OperationError::MutationConflict)
        ));
        assert!(matches!(
            store.register_operation(registration("operation-a", "epoch-a", "mutation-other")),
            Err(OperationError::OperationIdConflict)
        ));
        assert_eq!(operation_count(&store), 1);
    }

    #[test]
    fn operation_store_epoch_isolates_mutation_namespace() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();

        let old = store
            .register_operation(registration("operation-old", "epoch-old", "mutation-a"))
            .unwrap();
        let new = store
            .register_operation(registration("operation-new", "epoch-new", "mutation-a"))
            .unwrap();

        assert!(matches!(old, RegistrationOutcome::Registered(_)));
        assert!(matches!(new, RegistrationOutcome::Registered(_)));
        assert_eq!(operation_count(&store), 2);
        assert!(store.operation("operation-old").unwrap().is_some());
        assert!(store.operation("operation-new").unwrap().is_some());
    }

    #[test]
    fn operation_transitions_persist_terminal_result() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let _ = store
            .register_operation(registration("operation-a", "epoch-a", "mutation-a"))
            .unwrap();

        let running =
            store.transition_operation("operation-a", OperationStatus::Running, None, None);
        assert!(running.is_ok());
        let result = json!({"revision":u64::MAX});
        let succeeded = store.transition_operation(
            "operation-a",
            OperationStatus::Succeeded,
            Some(result.clone()),
            None,
        );
        assert!(succeeded.is_ok());
        let succeeded = succeeded.unwrap();
        assert_eq!(succeeded.status, OperationStatus::Succeeded);
        assert_eq!(succeeded.result, Some(result.clone()));
        assert!(matches!(
            store.transition_operation("operation-a", OperationStatus::Running, None, None),
            Err(OperationError::InvalidTransition(_))
        ));

        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let loaded = store.operation("operation-a").unwrap().unwrap();
        assert_eq!(loaded.status, OperationStatus::Succeeded);
        assert_eq!(loaded.result, Some(result));
    }

    #[test]
    fn operation_outcome_unknown_is_queryable_and_not_redispatched() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let _ = store
            .register_operation(registration("operation-a", "epoch-a", "mutation-a"))
            .unwrap();
        assert!(
            store
                .transition_operation("operation-a", OperationStatus::Running, None, None)
                .is_ok()
        );
        let unknown = store.transition_operation(
            "operation-a",
            OperationStatus::OutcomeUnknown,
            None,
            Some(json!({"code":"side_effect_unknown"})),
        );
        assert!(unknown.is_ok());

        let retry = store
            .register_operation(registration("operation-b", "epoch-a", "mutation-a"))
            .unwrap();
        assert!(matches!(retry, RegistrationOutcome::Existing(_)));
        let retry = retry.operation();
        assert_eq!(retry.operation_id, "operation-a");
        assert_eq!(retry.status, OperationStatus::OutcomeUnknown);
        assert_eq!(retry.error, Some(json!({"code":"side_effect_unknown"})));
        assert_eq!(operation_count(&store), 1);
    }

    #[test]
    fn operation_transition_rejects_inconsistent_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let _ = store
            .register_operation(registration("operation-a", "epoch-a", "mutation-a"))
            .unwrap();

        assert!(matches!(
            store.transition_operation(
                "operation-a",
                OperationStatus::Running,
                Some(json!({})),
                None,
            ),
            Err(OperationError::InvalidPayload)
        ));
        let _ = store
            .transition_operation("operation-a", OperationStatus::Running, None, None)
            .unwrap();
        assert!(matches!(
            store.transition_operation(
                "operation-a",
                OperationStatus::Succeeded,
                None,
                Some(json!({"code":"invalid"})),
            ),
            Err(OperationError::InvalidPayload)
        ));
        assert!(matches!(
            store.transition_operation(
                "operation-a",
                OperationStatus::Failed,
                Some(json!({})),
                None,
            ),
            Err(OperationError::InvalidPayload)
        ));
        let failed = store
            .transition_operation(
                "operation-a",
                OperationStatus::Failed,
                None,
                Some(json!({"code":"decision_required"})),
            )
            .unwrap();
        assert_eq!(failed.status, OperationStatus::Failed);
        assert_eq!(failed.error, Some(json!({"code":"decision_required"})));
    }

    #[test]
    fn operation_registration_rejects_empty_identities_and_invalid_versions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let base = registration("operation-a", "epoch-a", "mutation-a");
        let cases = [
            OperationRegistration {
                operation_id: String::new(),
                ..base.clone()
            },
            OperationRegistration {
                store_epoch: String::new(),
                ..base.clone()
            },
            OperationRegistration {
                client_id: String::new(),
                ..base.clone()
            },
            OperationRegistration {
                mutation_id: String::new(),
                ..base.clone()
            },
            OperationRegistration {
                method: String::new(),
                ..base.clone()
            },
            OperationRegistration {
                params_dto_version: 0,
                ..base.clone()
            },
            OperationRegistration {
                params_dto_version: u64::MAX,
                ..base.clone()
            },
            OperationRegistration {
                resource_generations: BTreeMap::from([(String::new(), 0)]),
                ..base
            },
        ];

        let accepted = cases
            .into_iter()
            .filter_map(|case| store.register_operation(case).ok())
            .count();

        assert_eq!(accepted, 0);
        assert_eq!(operation_count(&store), 0);
    }
}
