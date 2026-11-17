//! PostgreSQL delivery contract without a database dependency (Platform P5).
//!
//! This module implements a *contract*, not a live connection. It generates
//! deterministic parameterized SQL (`INSERT ... ON CONFLICT (event_id) DO
//! NOTHING` inside explicit transaction markers) and provides a deterministic
//! in-memory [`PostgresSimulator`] that enforces idempotency by event id,
//! transaction commit/rollback, and two reproducible failure injections:
//! "server unavailable" (no response, no side effect) and "ack lost" (commit
//! happened, receipt unknown).
//!
//! No driver is linked and no network is touched. Wiring `tokio-postgres` to
//! this contract is a separate dependency decision and is **not** implemented
//! here. No live database, throughput, latency or durability claim is made.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    application::{
        CanonicalBusinessEvent, DeliveryObligation, DeliveryReceipt, DeliveryRequest,
        DeliveryState, TargetConnector,
    },
    canonical::CanonicalEnvelope,
};

/// Parameterized SQL text plus bound values. Data is never interpolated into
/// `sql`; the placeholders `$1..$n` refer positionally to `params`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterizedStatement {
    pub sql: String,
    pub params: Vec<SqlValue>,
}

/// Bound parameter values. Both variants are safe to send as driver parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlValue {
    Text(String),
    Int(i64),
}

pub const BEGIN_SQL: &str = "BEGIN";
pub const COMMIT_SQL: &str = "COMMIT";
pub const ROLLBACK_SQL: &str = "ROLLBACK";

/// Idempotent insert. The conflict target is the logical event id, so the same
/// event delivered twice is a no-op rather than a second row.
pub const INSERT_SQL: &str = concat!(
    "INSERT INTO canonical_business_events ",
    "(event_id, target_id, entity_id, kind, value, observed_at, raw_observation, schema_version) ",
    "VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ",
    "ON CONFLICT (event_id) DO NOTHING",
);

/// One insert statement for one canonical business event.
#[must_use]
pub fn generate_insert_statement(
    event: &CanonicalEnvelope<CanonicalBusinessEvent>,
    target_id: &str,
) -> ParameterizedStatement {
    ParameterizedStatement {
        sql: INSERT_SQL.to_owned(),
        params: vec![
            SqlValue::Text(event.event_id.as_str().to_owned()),
            SqlValue::Text(target_id.to_owned()),
            SqlValue::Text(event.payload.entity_id.clone()),
            SqlValue::Text(event.payload.kind.clone()),
            SqlValue::Int(event.payload.value),
            SqlValue::Int(event.payload.observed_at),
            SqlValue::Text(event.raw_observation.clone().unwrap_or_default()),
            SqlValue::Int(i64::from(event.schema_version)),
        ],
    }
}

/// Transaction opening plus one insert per event. The terminal marker is
/// chosen by the target from the outcome: [`commit_statement`] on success or
/// [`rollback_statement`] on rejection.
#[must_use]
pub fn generate_batch_statements(
    request: &DeliveryRequest,
    target_id: &str,
) -> Vec<ParameterizedStatement> {
    let mut statements = vec![ParameterizedStatement {
        sql: BEGIN_SQL.to_owned(),
        params: Vec::new(),
    }];
    statements.extend(
        request
            .events
            .iter()
            .map(|event| generate_insert_statement(event, target_id)),
    );
    statements
}

#[must_use]
pub fn commit_statement() -> ParameterizedStatement {
    ParameterizedStatement {
        sql: COMMIT_SQL.to_owned(),
        params: Vec::new(),
    }
}

#[must_use]
pub fn rollback_statement() -> ParameterizedStatement {
    ParameterizedStatement {
        sql: ROLLBACK_SQL.to_owned(),
        params: Vec::new(),
    }
}

/// Validate an envelope before it can enter the simulated transaction.
fn event_validation_error(event: &CanonicalEnvelope<CanonicalBusinessEvent>) -> Option<String> {
    if let Err(error) = event.validate() {
        return Some(error.to_string());
    }
    if event.payload.entity_id.trim().is_empty() {
        return Some("entity_id must be non-empty".to_owned());
    }
    if event.payload.kind.trim().is_empty() {
        return Some("kind must be non-empty".to_owned());
    }
    None
}

/// Deterministic failure injection. No timer, probability or host state is
/// involved, so every test reproduces the same outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostgresFault {
    #[default]
    None,
    /// Every delivery fails without a response and without side effects.
    ServerUnavailable,
    /// The first `successful` deliveries commit; later ones are unavailable.
    ServerUnavailableAfter { successful: usize },
    /// The commit happens but the acknowledgement is lost on the first
    /// encounter of each key; a retry with the same key resolves as duplicate.
    AckLost,
    /// Every batch is rejected before any commit (transaction rollback).
    RejectAll,
}

/// One committed row in the simulator. It is the observable target state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedRow {
    pub event_id: String,
    pub entity_id: String,
    pub kind: String,
    pub value: i64,
    pub observed_at: i64,
    pub raw_observation: String,
    pub schema_version: u32,
}

/// In-memory PostgreSQL simulator. It is a target, not a database: state is
/// process-local, transactions are deterministic, and nothing is persisted.
#[derive(Debug)]
pub struct PostgresSimulator {
    target_id: String,
    fault: PostgresFault,
    committed: BTreeMap<String, CommittedRow>,
    lost_acks: BTreeSet<String>,
    attempts: BTreeMap<String, u32>,
    successful: usize,
}

impl PostgresSimulator {
    #[must_use]
    pub fn new(target_id: &str) -> Self {
        Self::with_fault(target_id, PostgresFault::None)
    }

    #[must_use]
    pub fn with_fault(target_id: &str, fault: PostgresFault) -> Self {
        Self {
            target_id: target_id.to_owned(),
            fault,
            committed: BTreeMap::new(),
            lost_acks: BTreeSet::new(),
            attempts: BTreeMap::new(),
            successful: 0,
        }
    }

    pub fn set_fault(&mut self, fault: PostgresFault) {
        self.fault = fault;
    }

    #[must_use]
    pub const fn fault(&self) -> PostgresFault {
        self.fault
    }

    #[must_use]
    pub fn committed_count(&self) -> usize {
        self.committed.len()
    }

    #[must_use]
    pub fn is_committed(&self, event_id: &str) -> bool {
        self.committed.contains_key(event_id)
    }

    #[must_use]
    pub fn committed_row(&self, event_id: &str) -> Option<&CommittedRow> {
        self.committed.get(event_id)
    }

    #[must_use]
    pub fn attempts_for(&self, event_id: &str) -> u32 {
        self.attempts.get(event_id).copied().unwrap_or_default()
    }

    fn commit(&mut self, request: &DeliveryRequest) {
        for event in &request.events {
            let row = CommittedRow {
                event_id: event.event_id.as_str().to_owned(),
                entity_id: event.payload.entity_id.clone(),
                kind: event.payload.kind.clone(),
                value: event.payload.value,
                observed_at: event.payload.observed_at,
                raw_observation: event.raw_observation.clone().unwrap_or_default(),
                schema_version: event.schema_version,
            };
            self.committed.insert(row.event_id.clone(), row);
        }
        self.successful += 1;
    }

    fn is_unavailable(&self) -> bool {
        match self.fault {
            PostgresFault::ServerUnavailable => true,
            PostgresFault::ServerUnavailableAfter { successful } => self.successful >= successful,
            PostgresFault::None | PostgresFault::AckLost | PostgresFault::RejectAll => false,
        }
    }
}

impl TargetConnector for PostgresSimulator {
    fn target_id(&self) -> &str {
        &self.target_id
    }

    fn deliver(&mut self, request: &DeliveryRequest) -> DeliveryReceipt {
        let key = request.idempotency_key.clone();
        *self.attempts.entry(key.clone()).or_insert(0) += 1;

        // Idempotency is checked before any fault: a repeated key never
        // produces a second row, even after an ack-lost outcome.
        if self.committed.contains_key(&key) {
            return DeliveryReceipt::DurableCommitted {
                idempotency_key: key,
                target_id: self.target_id.clone(),
                duplicate: true,
            };
        }

        if self.is_unavailable() {
            return DeliveryReceipt::Outstanding {
                idempotency_key: key,
                target_id: self.target_id.clone(),
            };
        }

        if let Some(reason) = request
            .events
            .iter()
            .find_map(event_validation_error)
            .or_else(|| {
                if self.fault == PostgresFault::RejectAll {
                    Some("server rejected the batch".to_owned())
                } else {
                    None
                }
            })
        {
            // No commit: the transaction is rolled back by the caller.
            return DeliveryReceipt::Rejected {
                idempotency_key: key,
                target_id: self.target_id.clone(),
                reason,
            };
        }

        self.commit(request);
        if self.fault == PostgresFault::AckLost && self.lost_acks.insert(key.clone()) {
            return DeliveryReceipt::Uncertain {
                idempotency_key: key,
                target_id: self.target_id.clone(),
                reason: "acknowledgement lost after commit".to_owned(),
            };
        }
        DeliveryReceipt::DurableCommitted {
            idempotency_key: key,
            target_id: self.target_id.clone(),
            duplicate: false,
        }
    }
}

/// SQL-generating, receipt-recording facade over the simulator. This is the
/// seam a real `tokio-postgres` implementation would replace without changing
/// the runner or the source.
#[derive(Debug)]
pub struct PostgresTarget {
    target_id: String,
    simulator: PostgresSimulator,
    statements: Vec<ParameterizedStatement>,
    obligations: Vec<DeliveryObligation>,
    receipts: Vec<DeliveryReceipt>,
}

impl PostgresTarget {
    #[must_use]
    pub fn new(target_id: &str) -> Self {
        Self::with_fault(target_id, PostgresFault::None)
    }

    #[must_use]
    pub fn with_fault(target_id: &str, fault: PostgresFault) -> Self {
        Self {
            target_id: target_id.to_owned(),
            simulator: PostgresSimulator::with_fault(target_id, fault),
            statements: Vec::new(),
            obligations: Vec::new(),
            receipts: Vec::new(),
        }
    }

    #[must_use]
    pub const fn simulator(&self) -> &PostgresSimulator {
        &self.simulator
    }

    #[must_use]
    pub fn simulator_mut(&mut self) -> &mut PostgresSimulator {
        &mut self.simulator
    }

    #[must_use]
    pub fn statements(&self) -> &[ParameterizedStatement] {
        &self.statements
    }

    #[must_use]
    pub fn obligations(&self) -> &[DeliveryObligation] {
        &self.obligations
    }

    #[must_use]
    pub fn receipts(&self) -> &[DeliveryReceipt] {
        &self.receipts
    }

    fn record_obligation(&mut self, receipt: &DeliveryReceipt) {
        if let Some(obligation) = self
            .obligations
            .iter_mut()
            .find(|obligation| obligation.logical_event_id == receipt.idempotency_key())
        {
            obligation.apply_receipt(receipt);
        } else {
            let mut obligation =
                DeliveryObligation::new(receipt.idempotency_key(), &self.target_id);
            obligation.apply_receipt(receipt);
            self.obligations.push(obligation);
        }
    }
}

impl TargetConnector for PostgresTarget {
    fn target_id(&self) -> &str {
        &self.target_id
    }

    fn deliver(&mut self, request: &DeliveryRequest) -> DeliveryReceipt {
        let mut statements = generate_batch_statements(request, &self.target_id);
        let receipt = self.simulator.deliver(request);
        if receipt.state() == DeliveryState::Rejected {
            statements.push(rollback_statement());
        } else if receipt.state() != DeliveryState::Outstanding {
            statements.push(commit_statement());
        }
        self.statements.extend(statements);
        self.record_obligation(&receipt);
        self.receipts.push(receipt.clone());
        receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{Checkpoint, JsonlSource, TransformError, run_jsonl_to_target},
        canonical::EventId,
    };

    fn envelope(id: &str, entity: &str, value: i64) -> CanonicalEnvelope<CanonicalBusinessEvent> {
        CanonicalEnvelope {
            event_id: EventId::derive("jsonl-postgres", "test", id),
            instrument_id: None,
            source_id: crate::identity::OpaqueId::derive("jsonl", "test", "test", "source"),
            feed_id: None,
            connection_epoch: 0,
            sequence: None,
            source_time: Some(1),
            receive_time: 1,
            raw_observation: Some(format!("jsonl:test:1:{id}")),
            schema_version: crate::canonical::CANONICAL_SCHEMA_VERSION,
            payload: CanonicalBusinessEvent {
                entity_id: entity.to_owned(),
                kind: "credit".to_owned(),
                value,
                observed_at: 1,
            },
        }
    }

    fn request(key: &str, event: CanonicalEnvelope<CanonicalBusinessEvent>) -> DeliveryRequest {
        DeliveryRequest {
            idempotency_key: key.to_owned(),
            events: vec![event],
        }
    }

    #[test]
    fn sql_is_parameterized_with_conflict_do_nothing_and_transaction_markers() {
        let event = envelope("evt-1", "acct", 42);
        let sql = generate_insert_statement(&event, "pg-local");
        assert!(sql.sql.contains("INSERT INTO canonical_business_events"));
        assert!(sql.sql.contains("ON CONFLICT (event_id) DO NOTHING"));
        assert!(sql.sql.contains("$1"));
        assert!(sql.sql.contains("$8"));
        assert_eq!(sql.params.len(), 8);
        assert_eq!(sql.params[2], SqlValue::Text("acct".to_owned()));
        assert_eq!(sql.params[4], SqlValue::Int(42));
        assert!(!sql.sql.contains("acct"));
        assert!(!sql.sql.contains("42"));

        let mut target = PostgresTarget::new("pg-local");
        let key = event.event_id.as_str().to_owned();
        let _ = target.deliver(&request(&key, envelope("evt-1", "acct", 42)));
        let sqls: Vec<&str> = target
            .statements()
            .iter()
            .map(|item| item.sql.as_str())
            .collect();
        assert_eq!(sqls[0], BEGIN_SQL);
        assert!(sqls[1].contains("ON CONFLICT (event_id) DO NOTHING"));
        assert_eq!(sqls[2], COMMIT_SQL);
    }

    #[test]
    fn simulator_is_idempotent_by_event_id() {
        let mut simulator = PostgresSimulator::new("pg-local");
        let event = envelope("evt-1", "acct", 1);
        let key = event.event_id.as_str().to_owned();
        let first = simulator.deliver(&request(&key, event.clone()));
        let second = simulator.deliver(&request(&key, event));
        assert_eq!(first.state(), DeliveryState::DurableCommitted);
        assert!(!first.is_duplicate());
        assert!(second.is_duplicate());
        assert_eq!(simulator.committed_count(), 1);
        assert_eq!(simulator.attempts_for(&key), 2);
        assert!(simulator.is_committed(&key));
        let row = simulator.committed_row(&key).expect("committed row");
        assert_eq!(row.entity_id, "acct");
        assert_eq!(row.value, 1);
    }

    #[test]
    fn simulator_rolls_back_rejected_batch() {
        let mut simulator = PostgresSimulator::with_fault("pg-local", PostgresFault::RejectAll);
        let event = envelope("evt-1", "acct", 1);
        let key = event.event_id.as_str().to_owned();
        let receipt = simulator.deliver(&request(&key, event));
        assert_eq!(receipt.state(), DeliveryState::Rejected);
        assert_eq!(simulator.committed_count(), 0);

        let mut target = PostgresTarget::with_fault("pg-local", PostgresFault::RejectAll);
        let event = envelope("evt-2", "acct", 2);
        let key = event.event_id.as_str().to_owned();
        let _ = target.deliver(&request(&key, event));
        let sqls: Vec<&str> = target
            .statements()
            .iter()
            .map(|item| item.sql.as_str())
            .collect();
        assert_eq!(sqls[0], BEGIN_SQL);
        assert_eq!(sqls.last().copied(), Some(ROLLBACK_SQL));
    }

    #[test]
    fn simulator_server_unavailable_has_no_side_effects() {
        let mut simulator =
            PostgresSimulator::with_fault("pg-local", PostgresFault::ServerUnavailable);
        let event = envelope("evt-1", "acct", 1);
        let key = event.event_id.as_str().to_owned();
        let receipt = simulator.deliver(&request(&key, event));
        assert_eq!(receipt.state(), DeliveryState::Outstanding);
        assert_eq!(simulator.committed_count(), 0);
        assert_eq!(simulator.attempts_for(&key), 1);
    }

    #[test]
    fn simulator_ack_lost_commits_but_reports_uncertain() {
        let mut simulator = PostgresSimulator::with_fault("pg-local", PostgresFault::AckLost);
        let event = envelope("evt-1", "acct", 1);
        let key = event.event_id.as_str().to_owned();
        let first = simulator.deliver(&request(&key, event.clone()));
        assert_eq!(first.state(), DeliveryState::Uncertain);
        assert!(simulator.is_committed(&key));
        let second = simulator.deliver(&request(&key, event));
        assert!(second.is_duplicate());
        assert_eq!(simulator.committed_count(), 1);
    }

    #[test]
    fn target_records_obligations_and_receipts() {
        let mut target = PostgresTarget::new("pg-local");
        let event = envelope("evt-1", "acct", 1);
        let key = event.event_id.as_str().to_owned();
        let receipt = target.deliver(&request(&key, event.clone()));
        let second = target.deliver(&request(&key, event));
        assert_eq!(target.receipts().len(), 2);
        assert_eq!(target.obligations().len(), 1);
        assert_eq!(target.obligations()[0].attempts, 2);
        assert_eq!(target.obligations()[0].state, DeliveryState::Duplicate);
        assert_eq!(target.receipts()[0], receipt);
        assert_eq!(target.receipts()[1], second);
        assert_eq!(target.simulator().committed_count(), 1);
    }

    #[test]
    fn simulator_malformed_envelope_is_rejected_not_committed() {
        let mut event = envelope("evt-1", "acct", 1);
        event.payload.entity_id = String::new();
        let key = event.event_id.as_str().to_owned();
        let mut simulator = PostgresSimulator::new("pg-local");
        let receipt = simulator.deliver(&request(&key, event));
        assert_eq!(receipt.state(), DeliveryState::Rejected);
        assert!(
            matches!(receipt, DeliveryReceipt::Rejected { reason, .. } if reason.contains("entity_id"))
        );
        assert_eq!(simulator.committed_count(), 0);
    }

    #[test]
    fn transform_rejection_is_accounted_without_target_side_effects() {
        let line = "{\"id\":\"a\",\"payload\":{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":5,\"observed_at\":9}}";
        let mut source = JsonlSource::from_str("fixture", line);
        let mut target = PostgresTarget::new("pg-local");
        let report = run_jsonl_to_target(
            &mut source,
            |_observation| Err(TransformError::Schema("forced rejection".to_owned())),
            &mut target,
            &Checkpoint::default(),
        );
        assert_eq!(report.total_read, 1);
        assert_eq!(report.rejected, 1);
        assert_eq!(report.delivered, 0);
        assert_eq!(report.accounted(), report.total_read);
        assert_eq!(report.last_committed_sequence, 1);
        assert_eq!(report.obligations[0].state, DeliveryState::Rejected);
        assert_eq!(target.simulator().committed_count(), 0);
        assert!(target.statements().is_empty());
    }
}
