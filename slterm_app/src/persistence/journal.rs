//! ManagedOperationJournal 的持久状态、步骤事实与恢复边界。

use std::collections::BTreeSet;
use std::fmt;

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde_json::Value;

use super::database::DatabaseError;
use super::operation::{OperationError, attach_operation_journal};
use super::workspace::WorkspaceDatabase;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum JournalStatus {
    Prepared,
    Applying,
    Committing,
    Committed,
    RollingBack,
    RolledBack,
    Failed,
    NeedsAttention,
}

impl JournalStatus {
    pub(crate) const ALL: [Self; 8] = [
        Self::Prepared,
        Self::Applying,
        Self::Committing,
        Self::Committed,
        Self::RollingBack,
        Self::RolledBack,
        Self::Failed,
        Self::NeedsAttention,
    ];

    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::RolledBack | Self::Failed)
    }

    pub(crate) fn transition(self, next: Self) -> Result<Self, InvalidJournalTransition> {
        let allowed = matches!(
            (self, next),
            (Self::Prepared, Self::Applying)
                | (Self::Prepared, Self::RollingBack)
                | (Self::Prepared, Self::Failed)
                | (Self::Prepared, Self::NeedsAttention)
                | (Self::Applying, Self::Committing)
                | (Self::Applying, Self::RollingBack)
                | (Self::Applying, Self::NeedsAttention)
                | (Self::Committing, Self::Committed)
                | (Self::Committing, Self::RollingBack)
                | (Self::Committing, Self::NeedsAttention)
                | (Self::RollingBack, Self::RolledBack)
                | (Self::RollingBack, Self::NeedsAttention)
                | (Self::NeedsAttention, Self::Applying)
                | (Self::NeedsAttention, Self::Committing)
                | (Self::NeedsAttention, Self::RollingBack)
                | (Self::NeedsAttention, Self::Committed)
                | (Self::NeedsAttention, Self::RolledBack)
                | (Self::NeedsAttention, Self::Failed)
        );
        if allowed {
            Ok(next)
        } else {
            Err(InvalidJournalTransition {
                from: self,
                to: next,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvalidJournalTransition {
    from: JournalStatus,
    to: JournalStatus,
}

impl InvalidJournalTransition {
    pub(crate) const fn from(self) -> JournalStatus {
        self.from
    }

    pub(crate) const fn to(self) -> JournalStatus {
        self.to
    }
}

impl fmt::Display for InvalidJournalTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Journal 状态转换不符合恢复状态机")
    }
}

impl std::error::Error for InvalidJournalTransition {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JournalStepRegistration {
    pub(crate) step_key: String,
    pub(crate) kind: String,
    pub(crate) target: Value,
    pub(crate) authorization_generation: u64,
    pub(crate) before_fingerprint: Option<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JournalStep {
    pub(crate) step_key: String,
    pub(crate) kind: String,
    pub(crate) target: Value,
    pub(crate) authorization_generation: u64,
    pub(crate) before_fingerprint: Option<[u8; 32]>,
    pub(crate) after_fingerprint: Option<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JournalRegistration {
    pub(crate) operation_id: Option<String>,
    pub(crate) journal_id: String,
    pub(crate) kind: String,
    pub(crate) store_epoch: String,
    pub(crate) before_fingerprint: Option<[u8; 32]>,
    pub(crate) steps: Vec<JournalStepRegistration>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ManagedJournal {
    pub(crate) journal_id: String,
    pub(crate) kind: String,
    pub(crate) store_epoch: String,
    pub(crate) status: JournalStatus,
    pub(crate) before_fingerprint: Option<[u8; 32]>,
    pub(crate) after_fingerprint: Option<[u8; 32]>,
    pub(crate) steps: Vec<JournalStep>,
}

#[derive(Debug)]
pub(crate) enum JournalError {
    InvalidRegistration,
    JournalIdConflict,
    UnknownJournal,
    UnknownStep,
    StepOutcomeConflict,
    JournalFinalized,
    InvalidTransition(InvalidJournalTransition),
    InvalidTransitionFacts,
    CorruptRecord,
    Operation(OperationError),
    Database(DatabaseError),
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRegistration => "Journal 登记字段无效",
            Self::JournalIdConflict => "Journal ID 已存在",
            Self::UnknownJournal => "Journal 不存在",
            Self::UnknownStep => "Journal 步骤不存在",
            Self::StepOutcomeConflict => "Journal 步骤已记录不同结果指纹",
            Self::JournalFinalized => "Journal 已终态，步骤事实不可改写",
            Self::InvalidTransition(_) => "Journal 状态转换无效",
            Self::InvalidTransitionFacts => "Journal 状态与结果指纹不一致",
            Self::CorruptRecord => "Journal 持久记录损坏",
            Self::Operation(_) => "Journal 关联 Operation 失败",
            Self::Database(_) => "Journal 数据库操作失败",
        })
    }
}

impl std::error::Error for JournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTransition(error) => Some(error),
            Self::Operation(error) => Some(error),
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<InvalidJournalTransition> for JournalError {
    fn from(error: InvalidJournalTransition) -> Self {
        Self::InvalidTransition(error)
    }
}

impl From<OperationError> for JournalError {
    fn from(error: OperationError) -> Self {
        Self::Operation(error)
    }
}

impl From<DatabaseError> for JournalError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for JournalError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    pub(crate) fn register_journal(
        &mut self,
        registration: JournalRegistration,
    ) -> Result<ManagedJournal, JournalError> {
        validate_registration(&registration)?;
        let steps = registration
            .steps
            .iter()
            .map(|step| {
                let target = serde_json::to_string(&step.target)
                    .map_err(|_| JournalError::InvalidRegistration)?;
                let authorization_generation = i64::try_from(step.authorization_generation)
                    .map_err(|_| JournalError::InvalidRegistration)?;
                Ok((step, target, authorization_generation))
            })
            .collect::<Result<Vec<_>, JournalError>>()?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM managed_journals WHERE journal_id = ?1)",
            [&registration.journal_id],
            |row| row.get::<_, bool>(0),
        )?;
        if exists {
            return Err(JournalError::JournalIdConflict);
        }
        transaction.execute(
            "INSERT INTO managed_journals \
             (journal_id, kind, status, store_epoch, before_fingerprint) \
             VALUES (?1, ?2, 'Prepared', ?3, ?4)",
            params![
                registration.journal_id,
                registration.kind,
                registration.store_epoch,
                registration
                    .before_fingerprint
                    .as_ref()
                    .map(|fingerprint| fingerprint.as_slice()),
            ],
        )?;
        for (step, target, authorization_generation) in steps {
            transaction.execute(
                "INSERT INTO journal_steps \
                 (journal_id, step_key, kind, target_json, authorization_generation, before_fingerprint) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    registration.journal_id,
                    step.step_key,
                    step.kind,
                    target,
                    authorization_generation,
                    step.before_fingerprint
                        .as_ref()
                        .map(|fingerprint| fingerprint.as_slice()),
                ],
            )?;
        }
        if let Some(operation_id) = &registration.operation_id {
            attach_operation_journal(
                &transaction,
                operation_id,
                &registration.journal_id,
                &registration.store_epoch,
            )?;
        }
        transaction.commit()?;
        self.journal(&registration.journal_id)?
            .ok_or(JournalError::CorruptRecord)
    }

    pub(crate) fn journal(&self, journal_id: &str) -> Result<Option<ManagedJournal>, JournalError> {
        query_journal(&self.database.connection, journal_id)
    }

    pub(crate) fn unfinished_journals(&self) -> Result<Vec<ManagedJournal>, JournalError> {
        let connection = &self.database.connection;
        let mut statement = connection.prepare(
            "SELECT journal_id FROM managed_journals \
             WHERE status NOT IN (?1, ?2, ?3) ORDER BY journal_id",
        )?;
        let journal_ids = statement.query_map(
            params![
                status_name(JournalStatus::Committed),
                status_name(JournalStatus::RolledBack),
                status_name(JournalStatus::Failed),
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut journals = Vec::new();
        for journal_id in journal_ids {
            let journal_id = journal_id?;
            journals
                .push(query_journal(connection, &journal_id)?.ok_or(JournalError::CorruptRecord)?);
        }
        Ok(journals)
    }

    pub(crate) fn transition_journal(
        &mut self,
        journal_id: &str,
        next: JournalStatus,
        after_fingerprint: Option<[u8; 32]>,
    ) -> Result<ManagedJournal, JournalError> {
        if after_fingerprint.is_some() && !next.is_terminal() {
            return Err(JournalError::InvalidTransitionFacts);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut journal =
            query_journal(&transaction, journal_id)?.ok_or(JournalError::UnknownJournal)?;
        journal.status.transition(next)?;
        let changed = transaction.execute(
            "UPDATE managed_journals \
             SET status = ?2, after_fingerprint = COALESCE(?3, after_fingerprint) \
             WHERE journal_id = ?1 AND status = ?4",
            params![
                journal_id,
                status_name(next),
                after_fingerprint
                    .as_ref()
                    .map(|fingerprint| fingerprint.as_slice()),
                status_name(journal.status),
            ],
        )?;
        if changed != 1 {
            return Err(JournalError::CorruptRecord);
        }
        transaction.commit()?;
        journal.status = next;
        if after_fingerprint.is_some() {
            journal.after_fingerprint = after_fingerprint;
        }
        Ok(journal)
    }

    pub(crate) fn record_step_after(
        &mut self,
        journal_id: &str,
        step_key: &str,
        after_fingerprint: [u8; 32],
    ) -> Result<JournalStep, JournalError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let journal =
            query_journal(&transaction, journal_id)?.ok_or(JournalError::UnknownJournal)?;
        if journal.status.is_terminal() {
            return Err(JournalError::JournalFinalized);
        }
        let existing =
            query_step(&transaction, journal_id, step_key)?.ok_or(JournalError::UnknownStep)?;
        if let Some(existing_fingerprint) = existing.after_fingerprint {
            if existing_fingerprint != after_fingerprint {
                return Err(JournalError::StepOutcomeConflict);
            }
            transaction.commit()?;
            return Ok(existing);
        }
        let changed = transaction.execute(
            "UPDATE journal_steps SET after_fingerprint = ?3 \
             WHERE journal_id = ?1 AND step_key = ?2 AND after_fingerprint IS NULL",
            params![journal_id, step_key, after_fingerprint.as_slice()],
        )?;
        if changed != 1 {
            return Err(JournalError::CorruptRecord);
        }
        let completed =
            query_step(&transaction, journal_id, step_key)?.ok_or(JournalError::CorruptRecord)?;
        transaction.commit()?;
        Ok(completed)
    }
}

fn valid_target(target: &Value) -> bool {
    target.is_object()
        && target
            .get("version")
            .and_then(Value::as_u64)
            .is_some_and(|version| version > 0)
}

fn validate_registration(registration: &JournalRegistration) -> Result<(), JournalError> {
    if registration.journal_id.is_empty()
        || registration.kind.is_empty()
        || registration.store_epoch.is_empty()
        || registration.operation_id.as_deref() == Some("")
        || registration.steps.is_empty()
    {
        return Err(JournalError::InvalidRegistration);
    }
    let mut step_keys = BTreeSet::new();
    for step in &registration.steps {
        if step.step_key.is_empty()
            || step.kind.is_empty()
            || !valid_target(&step.target)
            || i64::try_from(step.authorization_generation).is_err()
            || !step_keys.insert(&step.step_key)
        {
            return Err(JournalError::InvalidRegistration);
        }
    }
    Ok(())
}

fn query_journal(
    connection: &Connection,
    journal_id: &str,
) -> Result<Option<ManagedJournal>, JournalError> {
    let journal = connection
        .query_row(
            "SELECT journal_id, kind, store_epoch, status, before_fingerprint, after_fingerprint \
             FROM managed_journals WHERE journal_id = ?1",
            [journal_id],
            |row| Ok(decode_journal(row)),
        )
        .optional()?
        .transpose()?;
    let Some(mut journal) = journal else {
        return Ok(None);
    };
    journal.steps = query_steps(connection, journal_id)?;
    Ok(Some(journal))
}

fn query_steps(
    connection: &Connection,
    journal_id: &str,
) -> Result<Vec<JournalStep>, JournalError> {
    let mut statement = connection.prepare(
        "SELECT step_key, kind, target_json, authorization_generation, \
         before_fingerprint, after_fingerprint \
         FROM journal_steps WHERE journal_id = ?1 ORDER BY step_key",
    )?;
    let rows = statement.query_map([journal_id], |row| Ok(decode_step(row)))?;
    let mut steps = Vec::new();
    for row in rows {
        steps.push(row??);
    }
    Ok(steps)
}

fn query_step(
    connection: &Connection,
    journal_id: &str,
    step_key: &str,
) -> Result<Option<JournalStep>, JournalError> {
    connection
        .query_row(
            "SELECT step_key, kind, target_json, authorization_generation, \
             before_fingerprint, after_fingerprint \
             FROM journal_steps WHERE journal_id = ?1 AND step_key = ?2",
            params![journal_id, step_key],
            |row| Ok(decode_step(row)),
        )
        .optional()?
        .transpose()
}

fn decode_journal(row: &Row<'_>) -> Result<ManagedJournal, JournalError> {
    Ok(ManagedJournal {
        journal_id: required(row, 0)?,
        kind: required(row, 1)?,
        store_epoch: required(row, 2)?,
        status: parse_status(&required::<String>(row, 3)?)?,
        before_fingerprint: decode_fingerprint(row, 4)?,
        after_fingerprint: decode_fingerprint(row, 5)?,
        steps: Vec::new(),
    })
}

fn decode_step(row: &Row<'_>) -> Result<JournalStep, JournalError> {
    let authorization_generation =
        u64::try_from(required::<i64>(row, 3)?).map_err(|_| JournalError::CorruptRecord)?;
    let target: Value = serde_json::from_str(&required::<String>(row, 2)?)
        .map_err(|_| JournalError::CorruptRecord)?;
    if !valid_target(&target) {
        return Err(JournalError::CorruptRecord);
    }
    Ok(JournalStep {
        step_key: required(row, 0)?,
        kind: required(row, 1)?,
        target,
        authorization_generation,
        before_fingerprint: decode_fingerprint(row, 4)?,
        after_fingerprint: decode_fingerprint(row, 5)?,
    })
}

fn decode_fingerprint(row: &Row<'_>, index: usize) -> Result<Option<[u8; 32]>, JournalError> {
    required::<Option<Vec<u8>>>(row, index)?
        .map(|bytes| <[u8; 32]>::try_from(bytes).map_err(|_| JournalError::CorruptRecord))
        .transpose()
}

fn required<T: rusqlite::types::FromSql>(row: &Row<'_>, index: usize) -> Result<T, JournalError> {
    row.get(index).map_err(|_| JournalError::CorruptRecord)
}

fn status_name(status: JournalStatus) -> &'static str {
    match status {
        JournalStatus::Prepared => "Prepared",
        JournalStatus::Applying => "Applying",
        JournalStatus::Committing => "Committing",
        JournalStatus::Committed => "Committed",
        JournalStatus::RollingBack => "RollingBack",
        JournalStatus::RolledBack => "RolledBack",
        JournalStatus::Failed => "Failed",
        JournalStatus::NeedsAttention => "NeedsAttention",
    }
}

fn parse_status(text: &str) -> Result<JournalStatus, JournalError> {
    JournalStatus::ALL
        .into_iter()
        .find(|status| status_name(*status) == text)
        .ok_or(JournalError::CorruptRecord)
}

#[cfg(test)]
mod journal_tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_runtime_protocol::OperationStatus;
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::operation::{OperationRegistration, RegistrationOutcome};

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn operation(operation_id: &str, store_epoch: &str) -> OperationRegistration {
        OperationRegistration {
            operation_id: operation_id.into(),
            store_epoch: store_epoch.into(),
            client_id: "client-test".into(),
            mutation_id: format!("mutation-{operation_id}"),
            method: "document.save".into(),
            params_dto_version: 1,
            params_hash: [7; 32],
            resource_generations: BTreeMap::new(),
        }
    }

    fn step(step_key: &str) -> JournalStepRegistration {
        JournalStepRegistration {
            step_key: step_key.into(),
            kind: "replace".into(),
            target: json!({"path":"D:/temp/target","version":1}),
            authorization_generation: 7,
            before_fingerprint: Some([1; 32]),
        }
    }

    fn registration(journal_id: &str, store_epoch: &str) -> JournalRegistration {
        JournalRegistration {
            operation_id: None,
            journal_id: journal_id.into(),
            kind: "document.save".into(),
            store_epoch: store_epoch.into(),
            before_fingerprint: Some([2; 32]),
            steps: vec![step("step-a"), step("step-b")],
        }
    }

    fn journal_count(store: &WorkspaceDatabase) -> i64 {
        store
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM managed_journals", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn step_count(store: &WorkspaceDatabase) -> i64 {
        store
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM journal_steps", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn journal_registration_persists_steps_and_links_operation() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(matches!(
            store.register_operation(operation("operation-a", "epoch-a")),
            Ok(RegistrationOutcome::Registered(_))
        ));
        let plan = JournalRegistration {
            operation_id: Some("operation-a".into()),
            ..registration("journal-a", "epoch-a")
        };
        let registered = store.register_journal(plan.clone()).unwrap();
        assert_eq!(registered.status, JournalStatus::Prepared);
        assert_eq!(registered.after_fingerprint, None);
        assert_eq!(registered.steps.len(), 2);
        assert_eq!(
            store
                .operation("operation-a")
                .unwrap()
                .unwrap()
                .journal_id
                .as_deref(),
            Some("journal-a")
        );

        drop(store);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(store.journal("journal-a").unwrap(), Some(registered));
        assert!(matches!(
            store.register_journal(plan),
            Err(JournalError::JournalIdConflict)
        ));
        let rebound = JournalRegistration {
            operation_id: Some("operation-a".into()),
            ..registration("journal-b", "epoch-a")
        };
        assert!(matches!(
            store.register_journal(rebound),
            Err(JournalError::Operation(
                OperationError::JournalAttachmentConflict
            ))
        ));
        assert_eq!(store.journal("journal-b").unwrap(), None);
        assert_eq!(journal_count(&store), 1);
        assert_eq!(step_count(&store), 2);
        assert_eq!(
            store
                .operation("operation-a")
                .unwrap()
                .unwrap()
                .journal_id
                .as_deref(),
            Some("journal-a")
        );
    }

    #[test]
    fn journal_registration_rolls_back_when_operation_attachment_fails() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let unknown = JournalRegistration {
            operation_id: Some("missing".into()),
            ..registration("journal-unknown", "epoch-a")
        };
        assert!(matches!(
            store.register_journal(unknown),
            Err(JournalError::Operation(OperationError::UnknownOperation))
        ));

        let _ = store
            .register_operation(operation("operation-old", "epoch-old"))
            .unwrap();
        let mismatch = JournalRegistration {
            operation_id: Some("operation-old".into()),
            ..registration("journal-mismatch", "epoch-new")
        };
        assert!(matches!(
            store.register_journal(mismatch),
            Err(JournalError::Operation(OperationError::StoreEpochConflict))
        ));

        let _ = store
            .register_operation(operation("operation-terminal", "epoch-a"))
            .unwrap();
        let _ = store
            .transition_operation("operation-terminal", OperationStatus::Running, None, None)
            .unwrap();
        let _ = store
            .transition_operation("operation-terminal", OperationStatus::Failed, None, None)
            .unwrap();
        let terminal = JournalRegistration {
            operation_id: Some("operation-terminal".into()),
            ..registration("journal-terminal", "epoch-a")
        };
        assert!(matches!(
            store.register_journal(terminal),
            Err(JournalError::Operation(
                OperationError::JournalAttachmentConflict
            ))
        ));

        for journal_id in ["journal-unknown", "journal-mismatch", "journal-terminal"] {
            assert_eq!(store.journal(journal_id).unwrap(), None);
        }
        assert_eq!(journal_count(&store), 0);
        assert_eq!(step_count(&store), 0);
    }

    #[test]
    fn journal_status_accepts_only_declared_recovery_edges() {
        for from in JournalStatus::ALL {
            assert_eq!(
                from.is_terminal(),
                matches!(
                    from,
                    JournalStatus::Committed | JournalStatus::RolledBack | JournalStatus::Failed
                ),
                "{from:?}"
            );
            for to in JournalStatus::ALL {
                let allowed = matches!(
                    (from, to),
                    (JournalStatus::Prepared, JournalStatus::Applying)
                        | (JournalStatus::Prepared, JournalStatus::RollingBack)
                        | (JournalStatus::Prepared, JournalStatus::Failed)
                        | (JournalStatus::Prepared, JournalStatus::NeedsAttention)
                        | (JournalStatus::Applying, JournalStatus::Committing)
                        | (JournalStatus::Applying, JournalStatus::RollingBack)
                        | (JournalStatus::Applying, JournalStatus::NeedsAttention)
                        | (JournalStatus::Committing, JournalStatus::Committed)
                        | (JournalStatus::Committing, JournalStatus::RollingBack)
                        | (JournalStatus::Committing, JournalStatus::NeedsAttention)
                        | (JournalStatus::RollingBack, JournalStatus::RolledBack)
                        | (JournalStatus::RollingBack, JournalStatus::NeedsAttention)
                        | (JournalStatus::NeedsAttention, JournalStatus::Applying)
                        | (JournalStatus::NeedsAttention, JournalStatus::Committing)
                        | (JournalStatus::NeedsAttention, JournalStatus::RollingBack)
                        | (JournalStatus::NeedsAttention, JournalStatus::Committed)
                        | (JournalStatus::NeedsAttention, JournalStatus::RolledBack)
                        | (JournalStatus::NeedsAttention, JournalStatus::Failed)
                );
                assert_eq!(
                    from.transition(to).ok(),
                    allowed.then_some(to),
                    "{from:?} → {to:?}"
                );
            }
        }
    }

    #[test]
    fn journal_every_state_survives_reopen_and_only_unfinished_states_are_returned() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let cases: [(JournalStatus, &[JournalStatus]); 8] = [
            (JournalStatus::Prepared, &[]),
            (JournalStatus::Applying, &[JournalStatus::Applying]),
            (
                JournalStatus::Committing,
                &[JournalStatus::Applying, JournalStatus::Committing],
            ),
            (
                JournalStatus::Committed,
                &[
                    JournalStatus::Applying,
                    JournalStatus::Committing,
                    JournalStatus::Committed,
                ],
            ),
            (JournalStatus::RollingBack, &[JournalStatus::RollingBack]),
            (
                JournalStatus::RolledBack,
                &[JournalStatus::RollingBack, JournalStatus::RolledBack],
            ),
            (JournalStatus::Failed, &[JournalStatus::Failed]),
            (
                JournalStatus::NeedsAttention,
                &[JournalStatus::NeedsAttention],
            ),
        ];
        for (expected, transitions) in &cases {
            let journal_id = format!("journal-{expected:?}");
            let _ = store
                .register_journal(registration(&journal_id, "epoch-a"))
                .unwrap();
            for next in *transitions {
                let after = next.is_terminal().then_some([8; 32]);
                let _ = store.transition_journal(&journal_id, *next, after).unwrap();
            }
        }

        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        for (expected, _) in &cases {
            let journal_id = format!("journal-{expected:?}");
            assert_eq!(
                store.journal(&journal_id).unwrap().unwrap().status,
                *expected
            );
        }
        let unfinished = store.unfinished_journals().unwrap();
        assert_eq!(unfinished.len(), 5);
        assert!(
            unfinished
                .iter()
                .all(|journal| !journal.status.is_terminal())
        );
        for status in [
            JournalStatus::Prepared,
            JournalStatus::Applying,
            JournalStatus::Committing,
            JournalStatus::RollingBack,
            JournalStatus::NeedsAttention,
        ] {
            let journal_id = format!("journal-{status:?}");
            assert!(
                unfinished
                    .iter()
                    .any(|journal| journal.journal_id == journal_id)
            );
        }
    }

    #[test]
    fn journal_step_fingerprints_are_idempotent_and_terminal_facts_are_immutable() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let _ = store
            .register_journal(registration("journal-a", "epoch-a"))
            .unwrap();
        assert!(matches!(
            store.transition_journal("missing", JournalStatus::Applying, None),
            Err(JournalError::UnknownJournal)
        ));
        assert!(matches!(
            store.transition_journal("journal-a", JournalStatus::Committed, Some([8; 32])),
            Err(JournalError::InvalidTransition(_))
        ));
        assert!(matches!(
            store.transition_journal("journal-a", JournalStatus::Applying, Some([8; 32])),
            Err(JournalError::InvalidTransitionFacts)
        ));
        assert_eq!(
            store.journal("journal-a").unwrap().unwrap().status,
            JournalStatus::Prepared
        );

        let _ = store
            .transition_journal("journal-a", JournalStatus::Applying, None)
            .unwrap();
        let completed = store
            .record_step_after("journal-a", "step-a", [3; 32])
            .unwrap();
        assert_eq!(completed.after_fingerprint, Some([3; 32]));
        assert_eq!(
            store
                .record_step_after("journal-a", "step-a", [3; 32])
                .unwrap(),
            completed
        );
        assert!(matches!(
            store.record_step_after("journal-a", "step-a", [9; 32]),
            Err(JournalError::StepOutcomeConflict)
        ));
        assert!(matches!(
            store.record_step_after("journal-a", "missing", [3; 32]),
            Err(JournalError::UnknownStep)
        ));
        let _ = store
            .transition_journal("journal-a", JournalStatus::Committing, None)
            .unwrap();
        let committed = store
            .transition_journal("journal-a", JournalStatus::Committed, Some([4; 32]))
            .unwrap();
        assert_eq!(committed.after_fingerprint, Some([4; 32]));

        drop(store);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let loaded = store.journal("journal-a").unwrap().unwrap();
        assert_eq!(loaded.status, JournalStatus::Committed);
        assert_eq!(loaded.before_fingerprint, Some([2; 32]));
        assert_eq!(loaded.after_fingerprint, Some([4; 32]));
        assert_eq!(loaded.steps[0].after_fingerprint, Some([3; 32]));
        assert_eq!(loaded.steps[1].after_fingerprint, None);
        assert!(matches!(
            store.transition_journal("journal-a", JournalStatus::RollingBack, None),
            Err(JournalError::InvalidTransition(_))
        ));
        assert!(matches!(
            store.record_step_after("journal-a", "step-b", [3; 32]),
            Err(JournalError::JournalFinalized)
        ));
    }

    #[test]
    fn journal_needs_attention_retains_facts_until_explicit_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registered = store
            .register_journal(registration("journal-a", "epoch-a"))
            .unwrap();
        let _ = store
            .transition_journal("journal-a", JournalStatus::Applying, None)
            .unwrap();
        let _ = store
            .record_step_after("journal-a", "step-a", [3; 32])
            .unwrap();
        let _ = store
            .transition_journal("journal-a", JournalStatus::NeedsAttention, None)
            .unwrap();

        drop(store);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let loaded = store.journal("journal-a").unwrap().unwrap();
        assert_eq!(loaded.status, JournalStatus::NeedsAttention);
        assert_eq!(loaded.journal_id, registered.journal_id);
        assert_eq!(loaded.before_fingerprint, registered.before_fingerprint);
        assert_eq!(loaded.steps[0].after_fingerprint, Some([3; 32]));
        assert_eq!(loaded.steps[1].after_fingerprint, None);
        assert_eq!(store.unfinished_journals().unwrap(), vec![loaded.clone()]);
        assert_eq!(store.journal("journal-a").unwrap(), Some(loaded));

        let _ = store
            .transition_journal("journal-a", JournalStatus::RollingBack, None)
            .unwrap();
        let _ = store
            .transition_journal("journal-a", JournalStatus::RolledBack, Some([5; 32]))
            .unwrap();
        assert!(store.unfinished_journals().unwrap().is_empty());
        assert_eq!(
            store.journal("journal-a").unwrap().unwrap().status,
            JournalStatus::RolledBack
        );
    }

    #[test]
    fn journal_registration_rejects_invalid_or_duplicate_plans() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let base = registration("journal-a", "epoch-a");
        let cases = vec![
            JournalRegistration {
                journal_id: String::new(),
                ..base.clone()
            },
            JournalRegistration {
                kind: String::new(),
                ..base.clone()
            },
            JournalRegistration {
                store_epoch: String::new(),
                ..base.clone()
            },
            JournalRegistration {
                operation_id: Some(String::new()),
                ..base.clone()
            },
            JournalRegistration {
                steps: Vec::new(),
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![step("same"), step("same")],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    step_key: String::new(),
                    ..step("step-a")
                }],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    kind: String::new(),
                    ..step("step-a")
                }],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    target: Value::Null,
                    ..step("step-a")
                }],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    target: json!({"path":"D:/temp/target"}),
                    ..step("step-a")
                }],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    target: json!({"path":"D:/temp/target","version":0}),
                    ..step("step-a")
                }],
                ..base.clone()
            },
            JournalRegistration {
                steps: vec![JournalStepRegistration {
                    authorization_generation: u64::MAX,
                    ..step("step-a")
                }],
                ..base
            },
        ];

        let mut accepted = Vec::new();
        for (index, mut case) in cases.into_iter().enumerate() {
            if !case.journal_id.is_empty() {
                case.journal_id = format!("journal-invalid-{index}");
            }
            if store.register_journal(case).is_ok() {
                accepted.push(index);
            }
        }

        assert_eq!(accepted, Vec::<usize>::new());
        assert_eq!(journal_count(&store), 0);
        assert_eq!(step_count(&store), 0);
    }
}
