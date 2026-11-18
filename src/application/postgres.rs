//! PostgreSQL delivery contract and a real synchronous target (Platform P5/P6).
//!
//! The module keeps the deterministic SQL contract and in-memory
//! [`PostgresSimulator`] for offline fault injection, and adds [`PgTarget`],
//! which applies the same [`SqlExecutor`] transcript through a real
//! `postgres::Client`. Migrations are explicit SQL; idempotency is enforced by
//! database uniqueness on the target-specific key; event, disposition and
//! checkpoint writes share one transaction.
//!
//! The simulator never touches a database. The real target requires an
//! operator-supplied connection and an applied migration; it makes no claim
//! about latency, throughput or absolute zero loss.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as FmtWrite,
};

use postgres::{Client, types::ToSql};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    application::{
        CanonicalBusinessEvent, DeliveryObligation, DeliveryReceipt, DeliveryRequest,
        DeliveryState, ReceiptCategory, TargetConnector,
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

// ---------------------------------------------------------------------------
// Real PostgreSQL target (opt-in, operator-supplied connection)
// ---------------------------------------------------------------------------

/// Explicit schema applied before any live delivery. All writes are
/// parameterized; uniqueness is enforced by the database, not by the client.
pub const MIGRATION_SQL: &str = concat!(
    "CREATE TABLE IF NOT EXISTS canonical_business_events (",
    "idempotency_key TEXT PRIMARY KEY, ",
    "event_id TEXT NOT NULL, ",
    "target_id TEXT NOT NULL, ",
    "entity_id TEXT NOT NULL, ",
    "kind TEXT NOT NULL, ",
    "value BIGINT NOT NULL, ",
    "observed_at BIGINT NOT NULL, ",
    "raw_observation TEXT NOT NULL, ",
    "schema_version BIGINT NOT NULL",
    "); ",
    "CREATE TABLE IF NOT EXISTS delivery_dispositions (",
    "idempotency_key TEXT PRIMARY KEY, ",
    "event_id TEXT NOT NULL, ",
    "target_id TEXT NOT NULL, ",
    "disposition TEXT NOT NULL, ",
    "reason TEXT NOT NULL, ",
    "raw_observation TEXT NOT NULL",
    "); ",
    "CREATE TABLE IF NOT EXISTS target_checkpoints (",
    "target_id TEXT NOT NULL, ",
    "output_contract_version BIGINT NOT NULL, ",
    "last_committed_sequence BIGINT NOT NULL, ",
    "last_committed_offset BIGINT NOT NULL, ",
    "checkpoint_checksum TEXT NOT NULL, ",
    "last_idempotency_key TEXT NOT NULL DEFAULT '', ",
    "PRIMARY KEY (target_id, output_contract_version)",
    "); ",
    "ALTER TABLE target_checkpoints ADD COLUMN IF NOT EXISTS ",
    "last_idempotency_key TEXT NOT NULL DEFAULT ''"
);

/// Apply the idempotent migration to a connected database.
pub fn apply_migrations(client: &mut Client) -> Result<(), ExecutorError> {
    client
        .batch_execute(MIGRATION_SQL)
        .map_err(|error| ExecutorError::Failed(error.to_string()))
}

/// Execute one statement at a time through a real client and retain affected
/// row counts. A failed statement rolls the transaction back before the error
/// is reported, so a rejected batch cannot leave a partial commit behind.
pub struct PgExecutor<'a> {
    client: &'a mut Client,
    affected: Vec<u64>,
}

impl<'a> PgExecutor<'a> {
    #[must_use]
    pub const fn new(client: &'a mut Client) -> Self {
        Self {
            client,
            affected: Vec::new(),
        }
    }

    #[must_use]
    pub fn affected(&self) -> &[u64] {
        &self.affected
    }
}

impl SqlExecutor for PgExecutor<'_> {
    fn execute(&mut self, statement: &ParameterizedStatement) -> Result<u64, ExecutorError> {
        let params: Vec<&(dyn ToSql + Sync)> = statement
            .params
            .iter()
            .map(|value| match value {
                SqlValue::Text(text) => text as &(dyn ToSql + Sync),
                SqlValue::Int(number) => number as &(dyn ToSql + Sync),
            })
            .collect();
        match self.client.execute(&statement.sql, &params) {
            Ok(0) if statement.sql == INSERT_EVENT_WITH_KEY_SQL => {
                let existing_matches = self.existing_event_matches(statement);
                match existing_matches {
                    Ok(true) => {
                        self.affected.push(0);
                        Ok(0)
                    }
                    Ok(false) => {
                        let _ = self.client.execute(ROLLBACK_SQL, &[]);
                        Err(ExecutorError::Failed(
                            "idempotency key was reused with different canonical event content"
                                .to_owned(),
                        ))
                    }
                    Err(error) => {
                        let _ = self.client.execute(ROLLBACK_SQL, &[]);
                        Err(error)
                    }
                }
            }
            Ok(rows) if statement.sql == UPSERT_CHECKPOINT_SQL && rows != 1 => {
                let _ = self.client.execute(ROLLBACK_SQL, &[]);
                Err(ExecutorError::Failed(format!(
                    "checkpoint regression or source-position conflict rejected by database: update affected {rows} rows"
                )))
            }
            Ok(rows) => {
                self.affected.push(rows);
                Ok(rows)
            }
            Err(error) => {
                let _ = self.client.execute(ROLLBACK_SQL, &[]);
                Err(ExecutorError::Failed(error.to_string()))
            }
        }
    }
}

impl PgExecutor<'_> {
    fn existing_event_matches(
        &mut self,
        statement: &ParameterizedStatement,
    ) -> Result<bool, ExecutorError> {
        let [
            SqlValue::Text(idempotency_key),
            SqlValue::Text(event_id),
            SqlValue::Text(target_id),
            SqlValue::Text(entity_id),
            SqlValue::Text(kind),
            SqlValue::Int(value),
            SqlValue::Int(observed_at),
            SqlValue::Text(raw_observation),
            SqlValue::Int(schema_version),
        ] = statement.params.as_slice()
        else {
            return Err(ExecutorError::Failed(
                "event insert has an invalid parameter shape".to_owned(),
            ));
        };
        let query_params: [&(dyn ToSql + Sync); 1] = [idempotency_key];
        let row = self
            .client
            .query_opt(
                concat!(
                    "SELECT idempotency_key, event_id, target_id, entity_id, kind, ",
                    "value, observed_at, raw_observation, schema_version ",
                    "FROM canonical_business_events WHERE idempotency_key = $1"
                ),
                &query_params,
            )
            .map_err(|error| ExecutorError::Failed(error.to_string()))?;
        let Some(row) = row else {
            return Ok(false);
        };
        Ok(row.get::<_, String>(0) == *idempotency_key
            && row.get::<_, String>(1) == *event_id
            && row.get::<_, String>(2) == *target_id
            && row.get::<_, String>(3) == *entity_id
            && row.get::<_, String>(4) == *kind
            && row.get::<_, i64>(5) == *value
            && row.get::<_, i64>(6) == *observed_at
            && row.get::<_, String>(7) == *raw_observation
            && row.get::<_, i64>(8) == *schema_version)
    }
}

/// Checkpoint state owned by the live target between deliveries.
/// `last_idempotency_key` fences a same-position retry from a different event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgCheckpoint {
    pub sequence: u64,
    pub offset: u64,
    pub checksum: String,
    pub last_idempotency_key: Option<String>,
}

impl PgCheckpoint {
    /// Create the genesis checkpoint for one target namespace.
    #[must_use]
    pub fn initial(target_id: &str) -> Self {
        Self {
            sequence: 0,
            offset: 0,
            checksum: checkpoint_checksum(target_id, 0, 0),
            last_idempotency_key: None,
        }
    }

    fn expected_checksum(&self, target_id: &str) -> String {
        match self.last_idempotency_key.as_deref() {
            Some(key) => checkpoint_checksum_for_event(target_id, self.sequence, self.offset, key),
            None => checkpoint_checksum(target_id, self.sequence, self.offset),
        }
    }
}

impl Default for PgCheckpoint {
    fn default() -> Self {
        Self::initial("pg-local")
    }
}

fn checkpoint_checksum(target_id: &str, sequence: u64, offset: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(target_id.as_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(offset.to_le_bytes());
    format_checksum(hasher)
}

fn checkpoint_checksum_for_event(
    target_id: &str,
    sequence: u64,
    offset: u64,
    last_idempotency_key: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"pg-checkpoint-v2\0");
    hasher.update(
        u64::try_from(target_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(target_id.as_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(offset.to_le_bytes());
    hasher.update(
        u64::try_from(last_idempotency_key.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(last_idempotency_key.as_bytes());
    format_checksum(hasher)
}

fn format_checksum(hasher: Sha256) -> String {
    let digest = hasher.finalize();
    let mut text = String::with_capacity(7 + digest.len() * 2);
    let _ = FmtWrite::write_str(&mut text, "sha256:");
    for byte in digest {
        let _ = FmtWrite::write_fmt(&mut text, format_args!("{byte:02x}"));
    }
    text
}

/// A real PostgreSQL target. It shares the simulator's SQL contract and
/// receipts, but executes the transcript against an applied schema. The
/// caller declares the source position before each delivery; without that
/// position no checkpoint can be advanced.
pub struct PgTarget<'a> {
    target_id: String,
    client: &'a mut Client,
    checkpoint: PgCheckpoint,
    next_sequence: u64,
    next_offset: u64,
    uncertain_once: std::collections::BTreeSet<String>,
    receipts: Vec<DeliveryReceipt>,
}

impl<'a> PgTarget<'a> {
    #[must_use]
    pub fn new(target_id: &str, client: &'a mut Client) -> Self {
        Self::with_checkpoint(target_id, client, PgCheckpoint::initial(target_id))
    }

    #[must_use]
    pub fn with_checkpoint(
        target_id: &str,
        client: &'a mut Client,
        checkpoint: PgCheckpoint,
    ) -> Self {
        Self {
            target_id: target_id.to_owned(),
            client,
            next_sequence: checkpoint.sequence,
            next_offset: checkpoint.offset,
            checkpoint,
            uncertain_once: std::collections::BTreeSet::new(),
            receipts: Vec::new(),
        }
    }

    /// Declare the source position that a successful delivery will commit.
    pub fn set_position(&mut self, sequence: u64, offset: u64) {
        self.next_sequence = sequence;
        self.next_offset = offset;
    }

    #[must_use]
    pub const fn checkpoint(&self) -> &PgCheckpoint {
        &self.checkpoint
    }

    #[must_use]
    pub fn receipts(&self) -> &[DeliveryReceipt] {
        &self.receipts
    }

    /// Inject one lost acknowledgement: the transaction commits, but the
    /// receipt is `Uncertain` and the local checkpoint does not advance. The
    /// next delivery with the same key is resolved as a duplicate by the
    /// database. This is an explicit recovery drill, not a transport fault.
    pub fn inject_uncertain_ack(&mut self, key: &str) {
        self.uncertain_once.insert(key.to_owned());
    }

    /// Count committed rows for this target in the live database.
    pub fn committed_count(&mut self) -> Result<i64, ExecutorError> {
        self.client
            .query_one(
                "SELECT count(*) FROM canonical_business_events WHERE target_id = $1",
                &[&self.target_id],
            )
            .map(|row| row.get::<_, i64>(0))
            .map_err(|error| ExecutorError::Failed(error.to_string()))
    }

    /// Read and validate the persisted checkpoint and its same-position retry key.
    pub fn persisted_checkpoint(&mut self) -> Result<Option<PgCheckpoint>, ExecutorError> {
        let row = self
            .client
            .query_opt(
                concat!(
                    "SELECT last_committed_sequence, last_committed_offset, ",
                    "checkpoint_checksum, last_idempotency_key FROM target_checkpoints ",
                    "WHERE target_id = $1 AND output_contract_version = $2"
                ),
                &[&self.target_id, &(i64::from(OUTPUT_CONTRACT_VERSION))],
            )
            .map_err(|error| ExecutorError::Failed(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let sequence = u64::try_from(row.get::<_, i64>(0))
            .map_err(|error| ExecutorError::Failed(error.to_string()))?;
        let offset = u64::try_from(row.get::<_, i64>(1))
            .map_err(|error| ExecutorError::Failed(error.to_string()))?;
        let last_idempotency_key: String = row.get(3);
        let checkpoint = PgCheckpoint {
            sequence,
            offset,
            checksum: row.get::<_, String>(2),
            last_idempotency_key: (!last_idempotency_key.is_empty())
                .then_some(last_idempotency_key),
        };
        if checkpoint.checksum != checkpoint.expected_checksum(&self.target_id) {
            return Err(ExecutorError::Failed(
                "persisted checkpoint checksum does not match target, position and last event id"
                    .to_owned(),
            ));
        }
        Ok(Some(checkpoint))
    }
}

impl TargetConnector for PgTarget<'_> {
    fn target_id(&self) -> &str {
        &self.target_id
    }

    fn deliver(&mut self, request: &DeliveryRequest) -> DeliveryReceipt {
        let key = request.idempotency_key.clone();
        let invalid_reason = match request.events.as_slice() {
            [] => Some("PostgreSQL target requires one event per delivery".to_owned()),
            [event] if event.event_id.as_str() != key.as_str() => {
                Some("delivery idempotency key does not match the canonical event id".to_owned())
            }
            [event] => {
                let position_advances = self.next_sequence > self.checkpoint.sequence
                    && self.next_offset > self.checkpoint.offset;
                let exact_position_retry = self.next_sequence == self.checkpoint.sequence
                    && self.next_offset == self.checkpoint.offset
                    && self.checkpoint.last_idempotency_key.as_deref() == Some(key.as_str());
                event_validation_error(event).or_else(|| {
                    if self.checkpoint.checksum
                        != self.checkpoint.expected_checksum(&self.target_id)
                    {
                        Some(
                            "checkpoint checksum does not match target, position and last event id"
                                .to_owned(),
                        )
                    } else if self.next_sequence > i64::MAX as u64
                        || self.next_offset > i64::MAX as u64
                    {
                        Some("checkpoint position exceeds PostgreSQL BIGINT range".to_owned())
                    } else if position_advances || exact_position_retry {
                        None
                    } else {
                        Some(
                            "checkpoint position must advance, except for an exact same-key retry"
                                .to_owned(),
                        )
                    }
                })
            }
            _ => Some(
                "PostgreSQL target currently accepts exactly one event per delivery".to_owned(),
            ),
        };
        if let Some(reason) = invalid_reason {
            let receipt = DeliveryReceipt::Rejected {
                idempotency_key: key,
                target_id: self.target_id.clone(),
                reason,
            };
            self.receipts.push(receipt.clone());
            return receipt;
        }
        let provisional = DeliveryReceipt::DurableCommitted {
            idempotency_key: key.clone(),
            target_id: self.target_id.clone(),
            duplicate: false,
        };
        let next_checksum = checkpoint_checksum_for_event(
            &self.target_id,
            self.next_sequence,
            self.next_offset,
            &key,
        );
        let mut transcript = build_commit_transcript(
            &request.events[0],
            &self.target_id,
            OUTPUT_CONTRACT_VERSION,
            self.next_sequence,
            self.next_offset,
            &next_checksum,
            "durable_committed",
            "",
        );
        finalize_transcript(&mut transcript, &provisional);
        let mut executor = PgExecutor::new(self.client);
        if let Err(error) = execute_transcript(&mut executor, &transcript) {
            let receipt = DeliveryReceipt::Uncertain {
                idempotency_key: key.clone(),
                target_id: self.target_id.clone(),
                reason: format!("SQL executor failed: {error}"),
            };
            self.receipts.push(receipt.clone());
            return receipt;
        }
        let duplicate = executor.affected().get(1).copied() == Some(0);
        let receipt = if self.uncertain_once.remove(&key) {
            DeliveryReceipt::Uncertain {
                idempotency_key: key.clone(),
                target_id: self.target_id.clone(),
                reason: "acknowledgement lost after commit".to_owned(),
            }
        } else {
            DeliveryReceipt::DurableCommitted {
                idempotency_key: key.clone(),
                target_id: self.target_id.clone(),
                duplicate,
            }
        };
        if receipt.category() == ReceiptCategory::DurableCommitted {
            self.checkpoint = PgCheckpoint {
                sequence: self.next_sequence,
                offset: self.next_offset,
                checksum: next_checksum,
                last_idempotency_key: Some(key.clone()),
            };
        }
        self.receipts.push(receipt.clone());
        receipt
    }
}

// ---------------------------------------------------------------------------
// P6: target-specific idempotency and an executor seam
// ---------------------------------------------------------------------------

/// Version of the output row contract. It is part of the idempotency key so a
/// changed projection cannot be mistaken for a previously committed delivery.
pub const OUTPUT_CONTRACT_VERSION: u32 = 1;

/// Target-specific idempotency key. The same event delivered to two targets,
/// or under two output contract versions, is a different logical delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyKey {
    pub event_id: String,
    pub target_id: String,
    pub output_contract_version: u32,
}

impl IdempotencyKey {
    #[must_use]
    pub fn new(event_id: &str, target_id: &str, output_contract_version: u32) -> Self {
        Self {
            event_id: event_id.to_owned(),
            target_id: target_id.to_owned(),
            output_contract_version,
        }
    }

    /// Stable text form using the ASCII unit separator, so no field value can
    /// collide with a concatenation boundary.
    #[must_use]
    pub fn as_text(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}",
            self.event_id, self.target_id, self.output_contract_version
        )
    }
}

pub const INSERT_EVENT_WITH_KEY_SQL: &str = concat!(
    "INSERT INTO canonical_business_events ",
    "(idempotency_key, event_id, target_id, entity_id, kind, value, observed_at, raw_observation, schema_version) ",
    "VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ",
    "ON CONFLICT (idempotency_key) DO NOTHING",
);

pub const INSERT_DISPOSITION_SQL: &str = concat!(
    "INSERT INTO delivery_dispositions ",
    "(idempotency_key, event_id, target_id, disposition, reason, raw_observation) ",
    "VALUES ($1, $2, $3, $4, $5, $6) ",
    "ON CONFLICT (idempotency_key) DO NOTHING",
);

pub const UPSERT_CHECKPOINT_SQL: &str = concat!(
    "INSERT INTO target_checkpoints ",
    "(target_id, output_contract_version, last_committed_sequence, last_committed_offset, checkpoint_checksum, last_idempotency_key) ",
    "VALUES ($1, $2, $3, $4, $5, $6) ",
    "ON CONFLICT (target_id, output_contract_version) DO UPDATE SET ",
    "last_committed_sequence = EXCLUDED.last_committed_sequence, ",
    "last_committed_offset = EXCLUDED.last_committed_offset, ",
    "checkpoint_checksum = EXCLUDED.checkpoint_checksum, ",
    "last_idempotency_key = EXCLUDED.last_idempotency_key ",
    "WHERE (target_checkpoints.last_committed_sequence < EXCLUDED.last_committed_sequence ",
    "AND target_checkpoints.last_committed_offset < EXCLUDED.last_committed_offset) ",
    "OR (target_checkpoints.last_committed_sequence = EXCLUDED.last_committed_sequence ",
    "AND target_checkpoints.last_committed_offset = EXCLUDED.last_committed_offset ",
    "AND target_checkpoints.last_idempotency_key = EXCLUDED.last_idempotency_key)",
);

/// One `BEGIN ... COMMIT` transcript. `COMMIT` is present only for a terminal
/// durable-commit receipt; a non-terminal receipt leaves the transaction open
/// in the transcript so no caller can mistake simulation for a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitTranscript {
    pub idempotency_key: IdempotencyKey,
    pub statements: Vec<ParameterizedStatement>,
}

impl CommitTranscript {
    #[must_use]
    pub fn has_commit(&self) -> bool {
        self.statements
            .iter()
            .any(|statement| statement.sql == COMMIT_SQL)
    }

    #[must_use]
    pub fn has_rollback(&self) -> bool {
        self.statements
            .iter()
            .any(|statement| statement.sql == ROLLBACK_SQL)
    }
}

/// Build `BEGIN` + INSERT event + INSERT disposition + UPSERT checkpoint for a
/// single logical delivery. A terminal marker is added only by
/// [`finalize_transcript`], so callers cannot accidentally commit a receipt
/// that is still `Outstanding`, `Uncertain`, `Delivered` or `Simulated`.
#[expect(
    clippy::too_many_arguments,
    reason = "the transcript needs event, target, contract version, checkpoint fields and disposition evidence"
)]
#[must_use]
pub fn build_commit_transcript(
    event: &CanonicalEnvelope<CanonicalBusinessEvent>,
    target_id: &str,
    output_contract_version: u32,
    last_committed_sequence: u64,
    last_committed_offset: u64,
    checkpoint_checksum: &str,
    disposition: &str,
    reason: &str,
) -> CommitTranscript {
    let key = IdempotencyKey::new(event.event_id.as_str(), target_id, output_contract_version);
    let key_text = key.as_text();
    let statements = vec![
        ParameterizedStatement {
            sql: BEGIN_SQL.to_owned(),
            params: Vec::new(),
        },
        ParameterizedStatement {
            sql: INSERT_EVENT_WITH_KEY_SQL.to_owned(),
            params: vec![
                SqlValue::Text(key_text.clone()),
                SqlValue::Text(event.event_id.as_str().to_owned()),
                SqlValue::Text(target_id.to_owned()),
                SqlValue::Text(event.payload.entity_id.clone()),
                SqlValue::Text(event.payload.kind.clone()),
                SqlValue::Int(event.payload.value),
                SqlValue::Int(event.payload.observed_at),
                SqlValue::Text(event.raw_observation.clone().unwrap_or_default()),
                SqlValue::Int(i64::from(event.schema_version)),
            ],
        },
        ParameterizedStatement {
            sql: INSERT_DISPOSITION_SQL.to_owned(),
            params: vec![
                SqlValue::Text(key_text),
                SqlValue::Text(event.event_id.as_str().to_owned()),
                SqlValue::Text(target_id.to_owned()),
                SqlValue::Text(disposition.to_owned()),
                SqlValue::Text(reason.to_owned()),
                SqlValue::Text(event.raw_observation.clone().unwrap_or_default()),
            ],
        },
        ParameterizedStatement {
            sql: UPSERT_CHECKPOINT_SQL.to_owned(),
            params: vec![
                SqlValue::Text(target_id.to_owned()),
                SqlValue::Int(i64::from(output_contract_version)),
                SqlValue::Int(i64::try_from(last_committed_sequence).unwrap_or(i64::MAX)),
                SqlValue::Int(i64::try_from(last_committed_offset).unwrap_or(i64::MAX)),
                SqlValue::Text(checkpoint_checksum.to_owned()),
                SqlValue::Text(event.event_id.as_str().to_owned()),
            ],
        },
    ];
    // A terminal marker is deliberately omitted here. See finalize_transcript.
    CommitTranscript {
        idempotency_key: key,
        statements,
    }
}

/// Add a terminal marker based on the receipt category. Only a durable commit
/// produces `COMMIT`; a rejection produces `ROLLBACK`; every other category
/// (including simulated and non-terminal outcomes) is left without a terminal
/// marker so no COMMIT can be issued for it.
pub fn finalize_transcript(transcript: &mut CommitTranscript, receipt: &DeliveryReceipt) {
    match receipt.category() {
        ReceiptCategory::DurableCommitted => transcript.statements.push(commit_statement()),
        ReceiptCategory::Rejected => transcript.statements.push(rollback_statement()),
        ReceiptCategory::Delivered
        | ReceiptCategory::Simulated
        | ReceiptCategory::Outstanding
        | ReceiptCategory::Uncertain => {}
    }
}

/// Errors returned by the offline recorder or the live PostgreSQL executor.
/// Driver-specific errors are converted to stable, displayable messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecutorError {
    #[error("SQL executor unavailable: {0}")]
    Unavailable(String),
    #[error("SQL statement failed: {0}")]
    Failed(String),
}

/// Minimal executor seam shared by the deterministic recorder and the live
/// synchronous PostgreSQL executor. The trait keeps transaction transcript
/// construction independent of driver-specific connection behavior.
pub trait SqlExecutor {
    /// Execute one parameterized statement and return the affected-row count.
    fn execute(&mut self, statement: &ParameterizedStatement) -> Result<u64, ExecutorError>;
}

/// Deterministic in-memory executor used to exercise the seam offline. It
/// records executed SQL and never touches a database.
#[derive(Debug, Default)]
pub struct RecordingExecutor {
    executed: Vec<String>,
}

impl RecordingExecutor {
    #[must_use]
    pub fn executed(&self) -> &[String] {
        &self.executed
    }
}

impl SqlExecutor for RecordingExecutor {
    fn execute(&mut self, statement: &ParameterizedStatement) -> Result<u64, ExecutorError> {
        self.executed.push(statement.sql.clone());
        Ok(1)
    }
}

/// Execute a finalized transcript through any [`SqlExecutor`]. A transcript
/// without a terminal marker never receives a `COMMIT` statement.
pub fn execute_transcript<E: SqlExecutor>(
    executor: &mut E,
    transcript: &CommitTranscript,
) -> Result<usize, ExecutorError> {
    let count = transcript.statements.len();
    for statement in &transcript.statements {
        executor.execute(statement)?;
    }
    Ok(count)
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

/// SQL-generating, receipt-recording facade over the deterministic simulator.
/// Use [`PgTarget`] when statements must execute against a real PostgreSQL
/// connection; both targets preserve the same runner and source boundaries.
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
    fn commit_transcript_has_full_shape_and_target_specific_idempotency_key() {
        let event = envelope("evt-1", "acct", 42);
        let mut transcript = build_commit_transcript(
            &event,
            "pg-a",
            OUTPUT_CONTRACT_VERSION,
            7,
            700,
            "sha256:checksum",
            "durable_committed",
            "",
        );
        assert_eq!(transcript.statements.len(), 4);
        let sqls: Vec<&str> = transcript
            .statements
            .iter()
            .map(|statement| statement.sql.as_str())
            .collect();
        assert_eq!(sqls[0], BEGIN_SQL);
        assert_eq!(sqls[1], INSERT_EVENT_WITH_KEY_SQL);
        assert_eq!(sqls[2], INSERT_DISPOSITION_SQL);
        assert_eq!(sqls[3], UPSERT_CHECKPOINT_SQL);
        assert!(
            UPSERT_CHECKPOINT_SQL.contains(
                "target_checkpoints.last_idempotency_key = EXCLUDED.last_idempotency_key"
            ),
            "same-position checkpoint updates must require the same event id"
        );
        assert_eq!(
            transcript.statements[3].params.len(),
            6,
            "checkpoint upsert must include its last idempotency key"
        );
        assert_eq!(
            transcript.statements[3].params[5],
            SqlValue::Text(event.event_id.as_str().to_owned()),
            "checkpoint position must be bound to the canonical event id"
        );
        assert!(!transcript.has_commit());
        assert!(!transcript.has_rollback());

        let key = &transcript.idempotency_key;
        assert_eq!(key.event_id, event.event_id.as_str());
        assert_eq!(key.target_id, "pg-a");
        assert_eq!(key.output_contract_version, OUTPUT_CONTRACT_VERSION);
        let other_target =
            IdempotencyKey::new(key.event_id.as_str(), "pg-b", key.output_contract_version);
        assert_ne!(key.as_text(), other_target.as_text());
        let other_version = IdempotencyKey::new(
            key.event_id.as_str(),
            "pg-a",
            key.output_contract_version + 1,
        );
        assert_ne!(key.as_text(), other_version.as_text());
        let repeated = build_commit_transcript(
            &event,
            "pg-a",
            OUTPUT_CONTRACT_VERSION,
            7,
            700,
            "sha256:checksum",
            "durable_committed",
            "",
        );
        assert_eq!(transcript.idempotency_key, repeated.idempotency_key);

        let key_text = key.as_text();
        finalize_transcript(
            &mut transcript,
            &DeliveryReceipt::DurableCommitted {
                idempotency_key: key_text,
                target_id: "pg-a".to_owned(),
                duplicate: false,
            },
        );
        assert!(transcript.has_commit());
        assert_eq!(
            transcript.statements.last().map(|item| item.sql.as_str()),
            Some(COMMIT_SQL)
        );
    }

    #[test]
    fn checkpoint_checksum_binds_the_last_event_id() {
        let checksum = checkpoint_checksum_for_event("pg-a", 7, 700, "evt-a");
        let checkpoint = PgCheckpoint {
            sequence: 7,
            offset: 700,
            checksum: checksum.clone(),
            last_idempotency_key: Some("evt-a".to_owned()),
        };
        assert_eq!(
            checkpoint.expected_checksum("pg-a"),
            checksum,
            "the checkpoint checksum must validate its stored event id"
        );
        let mut changed_key = checkpoint;
        changed_key.last_idempotency_key = Some("evt-b".to_owned());
        assert_ne!(
            changed_key.expected_checksum("pg-a"),
            changed_key.checksum,
            "changing the stored event id must invalidate the checkpoint checksum"
        );
        let legacy_checkpoint = PgCheckpoint {
            sequence: 7,
            offset: 700,
            checksum: checkpoint_checksum("pg-a", 7, 700),
            last_idempotency_key: None,
        };
        assert_eq!(
            legacy_checkpoint.expected_checksum("pg-a"),
            legacy_checkpoint.checksum,
            "rows migrated without an event key must retain legacy checksum validation"
        );
    }

    #[test]
    fn no_commit_is_emitted_for_non_terminal_receipts() {
        let event = envelope("evt-1", "acct", 42);
        let receipts = [
            DeliveryReceipt::Outstanding {
                idempotency_key: "key".to_owned(),
                target_id: "pg-a".to_owned(),
            },
            DeliveryReceipt::Uncertain {
                idempotency_key: "key".to_owned(),
                target_id: "pg-a".to_owned(),
                reason: "ack lost".to_owned(),
            },
            DeliveryReceipt::Simulated {
                idempotency_key: "key".to_owned(),
                target_id: "pg-a".to_owned(),
            },
            DeliveryReceipt::Delivered {
                idempotency_key: "key".to_owned(),
                target_id: "pg-a".to_owned(),
            },
            DeliveryReceipt::Rejected {
                idempotency_key: "key".to_owned(),
                target_id: "pg-a".to_owned(),
                reason: "invalid".to_owned(),
            },
        ];
        for receipt in receipts {
            let mut transcript = build_commit_transcript(
                &event,
                "pg-a",
                OUTPUT_CONTRACT_VERSION,
                0,
                0,
                "sha256:none",
                "delivered",
                "",
            );
            finalize_transcript(&mut transcript, &receipt);
            assert!(
                !transcript.has_commit(),
                "receipt {:?} got a COMMIT",
                receipt.category()
            );
            if receipt.category() == ReceiptCategory::Rejected {
                assert!(transcript.has_rollback());
            } else {
                assert!(!transcript.has_rollback());
            }
        }
    }

    #[test]
    fn sql_executor_seam_runs_offline_without_a_database() {
        let event = envelope("evt-1", "acct", 42);
        let mut transcript = build_commit_transcript(
            &event,
            "pg-a",
            OUTPUT_CONTRACT_VERSION,
            1,
            10,
            "sha256:checksum",
            "durable_committed",
            "",
        );
        let key_text = transcript.idempotency_key.as_text();
        finalize_transcript(
            &mut transcript,
            &DeliveryReceipt::DurableCommitted {
                idempotency_key: key_text,
                target_id: "pg-a".to_owned(),
                duplicate: false,
            },
        );
        let mut executor = RecordingExecutor::default();
        let executed = execute_transcript(&mut executor, &transcript).expect("transcript executes");
        assert_eq!(executed, 5);
        assert_eq!(executor.executed().len(), 5);
        assert_eq!(executor.executed()[0], BEGIN_SQL);
        assert_eq!(executor.executed()[4], COMMIT_SQL);
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
