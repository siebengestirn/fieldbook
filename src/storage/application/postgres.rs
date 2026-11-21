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
        BatchMember, CanonicalBusinessEvent, DeliveryBatch, DeliveryObligation, DeliveryReceipt,
        DeliveryRequest, DeliveryState, ReceiptCategory, TargetConnector,
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
    "source_id TEXT NOT NULL DEFAULT '', ",
    "entity_id TEXT NOT NULL, ",
    "kind TEXT NOT NULL, ",
    "value BIGINT NOT NULL, ",
    "observed_at BIGINT NOT NULL, ",
    "raw_observation TEXT NOT NULL, ",
    "schema_version BIGINT NOT NULL, ",
    "source_sequence BIGINT NOT NULL DEFAULT 0, ",
    "source_offset BIGINT NOT NULL DEFAULT 0",
    "); ",
    "CREATE TABLE IF NOT EXISTS delivery_dispositions (",
    "idempotency_key TEXT PRIMARY KEY, ",
    "event_id TEXT NOT NULL, ",
    "target_id TEXT NOT NULL, ",
    "source_id TEXT NOT NULL DEFAULT '', ",
    "disposition TEXT NOT NULL, ",
    "reason TEXT NOT NULL, ",
    "raw_observation TEXT NOT NULL",
    "); ",
    "CREATE TABLE IF NOT EXISTS target_checkpoints (",
    "target_id TEXT NOT NULL, ",
    "source_id TEXT NOT NULL DEFAULT '', ",
    "output_contract_version BIGINT NOT NULL, ",
    "last_committed_sequence BIGINT NOT NULL, ",
    "last_committed_offset BIGINT NOT NULL, ",
    "checkpoint_checksum TEXT NOT NULL, ",
    "last_idempotency_key TEXT NOT NULL DEFAULT '', ",
    "PRIMARY KEY (target_id, source_id, output_contract_version)",
    "); ",
    "ALTER TABLE canonical_business_events ADD COLUMN IF NOT EXISTS ",
    "source_id TEXT NOT NULL DEFAULT ''; ",
    "ALTER TABLE canonical_business_events ADD COLUMN IF NOT EXISTS ",
    "source_sequence BIGINT NOT NULL DEFAULT 0; ",
    "ALTER TABLE canonical_business_events ADD COLUMN IF NOT EXISTS ",
    "source_offset BIGINT NOT NULL DEFAULT 0; ",
    "ALTER TABLE delivery_dispositions ADD COLUMN IF NOT EXISTS ",
    "source_id TEXT NOT NULL DEFAULT ''; ",
    "ALTER TABLE target_checkpoints ADD COLUMN IF NOT EXISTS ",
    "source_id TEXT NOT NULL DEFAULT ''; ",
    "ALTER TABLE target_checkpoints ADD COLUMN IF NOT EXISTS ",
    "last_idempotency_key TEXT NOT NULL DEFAULT ''; ",
);

/// Apply the idempotent migration to a connected database.
pub fn apply_migrations(client: &mut Client) -> Result<(), ExecutorError> {
    client
        .batch_execute(MIGRATION_SQL)
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    // Re-key an existing legacy table into the empty-source namespace. This
    // is done in Rust, not a procedural SQL block, so no server-side language
    // extension is required. It is idempotent and never infers a source. The
    // catalog check and the ALTER share one transaction behind an advisory
    // lock, so concurrent migrations cannot both observe an unkeyed table and
    // leave it briefly primary-key-free.
    let mut tx = client
        .transaction()
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtext('target_checkpoints')::bigint)",
        &[],
    )
    .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    let has_source_in_pk: bool = tx
        .query_one(
            concat!(
                "SELECT EXISTS (SELECT 1 FROM pg_constraint c ",
                "JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey) ",
                "WHERE c.conrelid = 'target_checkpoints'::regclass ",
                "AND c.contype = 'p' AND a.attname = 'source_id')"
            ),
            &[],
        )
        .map(|row| row.get(0))
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    if !has_source_in_pk {
        tx.batch_execute(concat!(
            "ALTER TABLE target_checkpoints ",
            "DROP CONSTRAINT IF EXISTS target_checkpoints_pkey; ",
            "ALTER TABLE target_checkpoints ",
            "ADD PRIMARY KEY (target_id, source_id, output_contract_version);"
        ))
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    }
    tx.commit()
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    Ok(())
}

/// Explicitly move one legacy empty-source checkpoint into a source namespace.
///
/// This is an operator-controlled migration step. The operator asserts that
/// every legacy row for this target and contract version belongs to `source_id`;
/// mixed-source legacy targets cannot be separated safely and must not use this
/// procedure. It never infers ownership from an event id or overwrites an
/// existing source-scoped checkpoint.
pub fn assign_legacy_checkpoint_source(
    client: &mut Client,
    target_id: &str,
    source_id: &str,
    output_contract_version: u32,
) -> Result<bool, ExecutorError> {
    if source_id.is_empty() {
        return Err(ExecutorError::Failed(
            "legacy checkpoint mapping requires a non-empty source id".to_owned(),
        ));
    }
    // An advisory lock per target serializes concurrent claims of the same
    // legacy row. The row checks, checksum rewrite and primary-key move are
    // therefore one atomic, operator-controlled step.
    let mut tx = client
        .transaction()
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
        &[&target_id],
    )
    .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    let legacy = tx
        .query_opt(
            concat!(
                "SELECT last_committed_sequence, last_committed_offset, ",
                "last_idempotency_key, output_contract_version, checkpoint_checksum ",
                "FROM target_checkpoints ",
                "WHERE target_id = $1 AND source_id = '' AND output_contract_version = $2 ",
                "FOR UPDATE"
            ),
            &[&target_id, &i64::from(output_contract_version)],
        )
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    let Some(legacy) = legacy else {
        return Ok(false);
    };
    let claimed = tx
        .query_opt(
            concat!(
                "SELECT 1 FROM target_checkpoints ",
                "WHERE target_id = $1 AND source_id = $2 AND output_contract_version = $3"
            ),
            &[&target_id, &source_id, &i64::from(output_contract_version)],
        )
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    if claimed.is_some() {
        return Ok(false);
    }
    let sequence = u64::try_from(legacy.get::<_, i64>(0))
        .map_err(|error| ExecutorError::Failed(error.to_string()))?;
    let offset = u64::try_from(legacy.get::<_, i64>(1))
        .map_err(|error| ExecutorError::Failed(error.to_string()))?;
    let last_idempotency_key: String = legacy.get(2);
    let stored_version = u32::try_from(legacy.get::<_, i64>(3))
        .map_err(|error| ExecutorError::Failed(error.to_string()))?;
    let stored_checksum: String = legacy.get(4);
    if !legacy_checkpoint_checksum_matches(
        target_id,
        stored_version,
        sequence,
        offset,
        &last_idempotency_key,
        &stored_checksum,
    ) {
        return Err(ExecutorError::Failed(
            "legacy checkpoint checksum is invalid; refusing source assignment".to_owned(),
        ));
    }
    // Historical rows used the three-part key; rows written through the
    // current explicit empty-source namespace use the four-part key with an
    // empty source component. Re-key only those exact forms for this target
    // and contract version. Event, disposition and checkpoint changes share
    // one transaction so replay cannot duplicate a migrated event.
    let contract_version_text = output_contract_version.to_string();
    let moved_events = tx
        .execute(
            concat!(
                "UPDATE canonical_business_events SET source_id = $2, ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || ",
                "$2 || chr(31) || $3 ",
                "WHERE target_id = $1 AND source_id = '' AND (",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $3 ",
                "OR idempotency_key = event_id || chr(31) || target_id || chr(31) || chr(31) || $3)"
            ),
            &[&target_id, &source_id, &contract_version_text],
        )
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    let moved_dispositions = tx
        .execute(
            concat!(
                "UPDATE delivery_dispositions SET source_id = $2, ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || ",
                "$2 || chr(31) || $3 ",
                "WHERE target_id = $1 AND source_id = '' AND (",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $3 ",
                "OR idempotency_key = event_id || chr(31) || target_id || chr(31) || chr(31) || $3)"
            ),
            &[&target_id, &source_id, &contract_version_text],
        )
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    if moved_events != moved_dispositions {
        return Err(ExecutorError::Failed(format!(
            "legacy event/disposition row counts differ during source assignment: {moved_events} events, {moved_dispositions} dispositions"
        )));
    }
    let paired_rows_match: bool = tx
        .query_one(
            concat!(
                "SELECT NOT EXISTS (",
                "SELECT event_id FROM canonical_business_events ",
                "WHERE target_id = $1 AND source_id = $2 AND ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $3 ",
                "EXCEPT SELECT event_id FROM delivery_dispositions ",
                "WHERE target_id = $1 AND source_id = $2 AND ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $3",
                ") AND NOT EXISTS (",
                "SELECT event_id FROM delivery_dispositions ",
                "WHERE target_id = $1 AND source_id = $2 AND ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $3 ",
                "EXCEPT SELECT event_id FROM canonical_business_events ",
                "WHERE target_id = $1 AND source_id = $2 AND ",
                "idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $3",
                ")"
            ),
            &[&target_id, &source_id, &contract_version_text],
        )
        .map(|row| row.get(0))
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    if !paired_rows_match {
        return Err(ExecutorError::Failed(
            "legacy event and disposition identities differ; refusing source assignment".to_owned(),
        ));
    }
    if !last_idempotency_key.is_empty() {
        let checkpoint_anchor_matches: bool = tx
            .query_one(
                concat!(
                    "SELECT EXISTS (SELECT 1 FROM canonical_business_events ",
                    "WHERE target_id = $1 AND source_id = $2 AND event_id = $3 ",
                    "AND idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $4) ",
                    "AND EXISTS (SELECT 1 FROM delivery_dispositions ",
                    "WHERE target_id = $1 AND source_id = $2 AND event_id = $3 ",
                    "AND idempotency_key = event_id || chr(31) || target_id || chr(31) || $2 || chr(31) || $4)"
                ),
                &[&target_id, &source_id, &last_idempotency_key, &contract_version_text],
            )
            .map(|row| row.get(0))
            .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
        if !checkpoint_anchor_matches {
            return Err(ExecutorError::Failed(
                "legacy checkpoint event key has no matching event and disposition; refusing source assignment"
                    .to_owned(),
            ));
        }
        let anchor_sequence =
            i64::try_from(sequence).map_err(|error| ExecutorError::Failed(error.to_string()))?;
        let anchor_offset =
            i64::try_from(offset).map_err(|error| ExecutorError::Failed(error.to_string()))?;
        // Legacy event rows predate source positions. Carry the verified
        // checkpoint position onto the anchor so a replay at that position
        // matches the persisted canonical event content.
        let positioned = tx
            .execute(
                concat!(
                    "UPDATE canonical_business_events ",
                    "SET source_sequence = $4, source_offset = $5 ",
                    "WHERE target_id = $1 AND source_id = $2 AND event_id = $3 ",
                    "AND idempotency_key = event_id || chr(31) || target_id || chr(31) || ",
                    "$2 || chr(31) || $6"
                ),
                &[
                    &target_id,
                    &source_id,
                    &last_idempotency_key,
                    &anchor_sequence,
                    &anchor_offset,
                    &contract_version_text,
                ],
            )
            .map_err(|error| ExecutorError::Failed(error.to_string()))?;
        if positioned != 1 {
            return Err(ExecutorError::Failed(format!(
                "legacy checkpoint anchor position update affected {positioned} rows"
            )));
        }
    }

    // Only a checksum-verified legacy position may be rebound to the claimed
    // source. The new checksum uses the source-scoped v4 identity format.
    let checksum = checkpoint_checksum(
        target_id,
        source_id,
        stored_version,
        sequence,
        offset,
        &last_idempotency_key,
    );
    let updated = tx
        .execute(
            concat!(
                "UPDATE target_checkpoints SET source_id = $2, checkpoint_checksum = $3 ",
                "WHERE target_id = $1 AND source_id = '' AND output_contract_version = $4"
            ),
            &[
                &target_id,
                &source_id,
                &checksum,
                &i64::from(output_contract_version),
            ],
        )
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    tx.commit()
        .map_err(|error| ExecutorError::Failed(format!("{error:?}")))?;
    Ok(updated == 1)
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
            SqlValue::Text(source_id),
            SqlValue::Text(entity_id),
            SqlValue::Text(kind),
            SqlValue::Int(value),
            SqlValue::Int(observed_at),
            SqlValue::Text(raw_observation),
            SqlValue::Int(schema_version),
            SqlValue::Int(source_sequence),
            SqlValue::Int(source_offset),
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
                    "SELECT idempotency_key, event_id, target_id, source_id, entity_id, kind, ",
                    "value, observed_at, raw_observation, schema_version, source_sequence, ",
                    "source_offset FROM canonical_business_events WHERE idempotency_key = $1"
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
            && row.get::<_, String>(3) == *source_id
            && row.get::<_, String>(4) == *entity_id
            && row.get::<_, String>(5) == *kind
            && row.get::<_, i64>(6) == *value
            && row.get::<_, i64>(7) == *observed_at
            && row.get::<_, String>(8) == *raw_observation
            && row.get::<_, i64>(9) == *schema_version
            && row.get::<_, i64>(10) == *source_sequence
            && row.get::<_, i64>(11) == *source_offset)
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
    /// Empty only for the explicit pre-source legacy namespace.
    pub source_id: String,
}

impl PgCheckpoint {
    /// Genesis checkpoint in the legacy empty-source namespace.
    #[must_use]
    pub fn initial(target_id: &str) -> Self {
        Self::initial_for_source(target_id, "")
    }

    /// Genesis checkpoint for one source-target stream.
    #[must_use]
    pub fn initial_for_source(target_id: &str, source_id: &str) -> Self {
        Self {
            sequence: 0,
            offset: 0,
            checksum: checkpoint_checksum(target_id, source_id, OUTPUT_CONTRACT_VERSION, 0, 0, ""),
            last_idempotency_key: None,
            source_id: source_id.to_owned(),
        }
    }

    fn expected_checksum(&self, target_id: &str) -> String {
        checkpoint_checksum(
            target_id,
            &self.source_id,
            OUTPUT_CONTRACT_VERSION,
            self.sequence,
            self.offset,
            self.last_idempotency_key.as_deref().unwrap_or(""),
        )
    }
}

impl Default for PgCheckpoint {
    fn default() -> Self {
        Self::initial("pg-local")
    }
}

fn checkpoint_checksum(
    target_id: &str,
    source_id: &str,
    output_contract_version: u32,
    sequence: u64,
    offset: u64,
    last_idempotency_key: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"pg-checkpoint-v4\0");
    // The output contract version is part of the checkpoint identity. Binding
    // it here means a changed projection invalidates every persisted checksum
    // instead of being silently reused under the same position.
    hasher.update(output_contract_version.to_le_bytes());
    hasher.update(
        u64::try_from(target_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(
        u64::try_from(source_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(source_id.as_bytes());
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

/// Checksum used by the original pre-event-key PostgreSQL checkpoint schema.
fn legacy_checkpoint_checksum_v1(target_id: &str, sequence: u64, offset: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(target_id.as_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(offset.to_le_bytes());
    format_checksum(hasher)
}

/// Checksum used after same-position retries gained an event-key fence and
/// before source identity became part of the checkpoint namespace.
fn legacy_checkpoint_checksum_v2(
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

/// Accept only checksums that can legitimately exist in the empty-source
/// namespace: current v4, historical event-bound v2, or original v1 when no
/// event key was stored. This is used both before legacy reads and before an
/// operator-controlled source assignment.
fn legacy_checkpoint_checksum_matches(
    target_id: &str,
    output_contract_version: u32,
    sequence: u64,
    offset: u64,
    last_idempotency_key: &str,
    stored_checksum: &str,
) -> bool {
    stored_checksum
        == checkpoint_checksum(
            target_id,
            "",
            output_contract_version,
            sequence,
            offset,
            last_idempotency_key,
        )
        || stored_checksum
            == legacy_checkpoint_checksum_v2(target_id, sequence, offset, last_idempotency_key)
        || (last_idempotency_key.is_empty()
            && stored_checksum == legacy_checkpoint_checksum_v1(target_id, sequence, offset))
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
/// receipts, but executes the transcript against an applied schema. Every
/// delivery carries its source identity, sequence and end offset explicitly;
/// without that position no checkpoint can be advanced.
pub struct PgTarget<'a> {
    target_id: String,
    /// Empty only for the pre-source legacy namespace.
    source_id: String,
    client: &'a mut Client,
    checkpoint: PgCheckpoint,
    uncertain_once: std::collections::BTreeSet<String>,
    receipts: Vec<DeliveryReceipt>,
}

impl<'a> PgTarget<'a> {
    /// Construct a target in the explicit legacy empty-source namespace.
    /// Modern envelopes carry a non-empty source id and must use
    /// [`Self::with_source`].
    #[must_use]
    pub fn new(target_id: &str, client: &'a mut Client) -> Self {
        Self::with_checkpoint(target_id, client, PgCheckpoint::initial(target_id))
    }

    /// Bind this target instance to one source identity. Every delivered
    /// envelope must carry the same source id or it is rejected.
    #[must_use]
    pub fn with_source(target_id: &str, source_id: &str, client: &'a mut Client) -> Self {
        Self::with_source_checkpoint(
            target_id,
            source_id,
            client,
            PgCheckpoint::initial_for_source(target_id, source_id),
        )
    }

    /// Construct a target from a checkpoint in the legacy empty-source
    /// namespace. The checkpoint and each delivered envelope must have an
    /// empty source id.
    #[must_use]
    pub fn with_checkpoint(
        target_id: &str,
        client: &'a mut Client,
        checkpoint: PgCheckpoint,
    ) -> Self {
        Self::with_source_checkpoint(target_id, "", client, checkpoint)
    }

    /// Construct a source-bound target from a checkpoint. Delivery rejects a
    /// checkpoint whose source namespace differs from `source_id`.
    #[must_use]
    pub fn with_source_checkpoint(
        target_id: &str,
        source_id: &str,
        client: &'a mut Client,
        checkpoint: PgCheckpoint,
    ) -> Self {
        Self {
            target_id: target_id.to_owned(),
            source_id: source_id.to_owned(),
            client,
            checkpoint,
            uncertain_once: std::collections::BTreeSet::new(),
            receipts: Vec::new(),
        }
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

    /// Count committed rows for this target and source in the live database.
    pub fn source_committed_count(&mut self) -> Result<i64, ExecutorError> {
        self.client
            .query_one(
                "SELECT count(*) FROM canonical_business_events WHERE target_id = $1 AND source_id = $2",
                &[&self.target_id, &self.source_id],
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
                    "WHERE target_id = $1 AND source_id = $2 AND output_contract_version = $3"
                ),
                &[
                    &self.target_id,
                    &self.source_id,
                    &(i64::from(OUTPUT_CONTRACT_VERSION)),
                ],
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
            source_id: self.source_id.clone(),
        };
        let checksum_valid = if self.source_id.is_empty() {
            legacy_checkpoint_checksum_matches(
                &self.target_id,
                OUTPUT_CONTRACT_VERSION,
                checkpoint.sequence,
                checkpoint.offset,
                checkpoint.last_idempotency_key.as_deref().unwrap_or(""),
                &checkpoint.checksum,
            )
        } else {
            checkpoint.checksum == checkpoint.expected_checksum(&self.target_id)
        };
        if !checksum_valid {
            return Err(ExecutorError::Failed(
                "persisted checkpoint checksum does not match target, source, position and last event id"
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
        let source_position = request.source_position.as_ref();
        let invalid_reason = match request.events.as_slice() {
            [] => Some("PostgreSQL target requires one event per delivery".to_owned()),
            [event] if event.event_id.as_str() != key.as_str() => {
                Some("delivery idempotency key does not match the canonical event id".to_owned())
            }
            [event] => {
                let Some(position) = source_position else {
                    let reason =
                        "PostgreSQL target requires an explicit source position".to_owned();
                    let receipt = DeliveryReceipt::Rejected {
                        idempotency_key: key,
                        target_id: self.target_id.clone(),
                        reason,
                    };
                    self.receipts.push(receipt.clone());
                    return receipt;
                };
                if position.source_id != self.source_id {
                    Some(format!(
                        "source position source `{}` does not match target source `{}`",
                        position.source_id, self.source_id
                    ))
                } else if let Some(reason) = source_binding_error(
                    &self.source_id,
                    &self.checkpoint.source_id,
                    event.source_id.as_str(),
                ) {
                    Some(reason)
                } else {
                    let position_advances = position.sequence > self.checkpoint.sequence
                        && position.offset > self.checkpoint.offset;
                    let exact_position_retry = position.sequence == self.checkpoint.sequence
                        && position.offset == self.checkpoint.offset
                        && self.checkpoint.last_idempotency_key.as_deref() == Some(key.as_str());
                    event_validation_error(event).or_else(|| {
                        if self.checkpoint.checksum
                            != self.checkpoint.expected_checksum(&self.target_id)
                        {
                            Some(
                                "checkpoint checksum does not match target, source, position and last event id"
                                    .to_owned(),
                            )
                        } else if position.sequence > i64::MAX as u64
                            || position.offset > i64::MAX as u64
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
        let Some(position) = source_position else {
            let receipt = DeliveryReceipt::Rejected {
                idempotency_key: key,
                target_id: self.target_id.clone(),
                reason: "PostgreSQL target requires an explicit source position".to_owned(),
            };
            self.receipts.push(receipt.clone());
            return receipt;
        };
        let next_checksum = checkpoint_checksum(
            &self.target_id,
            &self.source_id,
            OUTPUT_CONTRACT_VERSION,
            position.sequence,
            position.offset,
            &key,
        );
        let mut transcript = build_source_commit_transcript(
            &self.source_id,
            &request.events[0],
            &self.target_id,
            OUTPUT_CONTRACT_VERSION,
            position.sequence,
            position.offset,
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
                sequence: position.sequence,
                offset: position.offset,
                checksum: next_checksum,
                last_idempotency_key: Some(key.clone()),
                source_id: self.source_id.clone(),
            };
        }
        self.receipts.push(receipt.clone());
        receipt
    }

    fn deliver_batch(&mut self, batch: &DeliveryBatch) -> Vec<DeliveryReceipt> {
        let batch_key = batch.idempotency_key.clone();
        if let Some(reason) = batch_validation_error(self, batch) {
            let receipt = DeliveryReceipt::Rejected {
                idempotency_key: batch_key,
                target_id: self.target_id.clone(),
                reason,
            };
            self.receipts.push(receipt.clone());
            return vec![receipt];
        }
        let Some(terminal) = batch.members.last() else {
            let receipt = DeliveryReceipt::Uncertain {
                idempotency_key: batch_key,
                target_id: self.target_id.clone(),
                reason: "validated batch has no terminal member".to_owned(),
            };
            self.receipts.push(receipt.clone());
            return vec![receipt];
        };
        let terminal_checksum = checkpoint_checksum(
            &self.target_id,
            &batch.source_id,
            OUTPUT_CONTRACT_VERSION,
            terminal.source_position.sequence,
            terminal.source_position.offset,
            &batch_key,
        );
        let mut transcript = build_batch_commit_transcript(batch, &terminal_checksum);
        finalize_transcript(
            &mut transcript,
            &DeliveryReceipt::DurableCommitted {
                idempotency_key: batch_key.clone(),
                target_id: self.target_id.clone(),
                duplicate: false,
            },
        );
        let mut executor = PgExecutor::new(self.client);
        if let Err(error) = execute_transcript(&mut executor, &transcript) {
            let receipt = DeliveryReceipt::Uncertain {
                idempotency_key: batch_key,
                target_id: self.target_id.clone(),
                reason: format!("SQL executor failed: {error}"),
            };
            self.receipts.push(receipt.clone());
            return vec![receipt];
        }

        let receipts: Vec<DeliveryReceipt> = batch
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                let duplicate = executor.affected().get(1 + index).copied() == Some(0);
                if self.uncertain_once.remove(member.event.event_id.as_str()) {
                    DeliveryReceipt::Uncertain {
                        idempotency_key: batch_key.clone(),
                        target_id: self.target_id.clone(),
                        reason: "acknowledgement lost after commit".to_owned(),
                    }
                } else {
                    DeliveryReceipt::DurableCommitted {
                        idempotency_key: batch_key.clone(),
                        target_id: self.target_id.clone(),
                        duplicate,
                    }
                }
            })
            .collect();
        if receipts
            .iter()
            .all(|receipt| receipt.category() == ReceiptCategory::DurableCommitted)
        {
            self.checkpoint = PgCheckpoint {
                sequence: terminal.source_position.sequence,
                offset: terminal.source_position.offset,
                checksum: terminal_checksum,
                last_idempotency_key: Some(batch_key.clone()),
                source_id: self.source_id.clone(),
            };
        }
        self.receipts.extend(receipts.iter().cloned());
        receipts
    }
}

fn batch_validation_error(target: &PgTarget<'_>, batch: &DeliveryBatch) -> Option<String> {
    if batch.members.is_empty() {
        return Some("PostgreSQL target requires a non-empty batch".to_owned());
    }
    if batch.source_id != target.source_id {
        return Some("batch source does not match target source".to_owned());
    }
    if batch.target_id != target.target_id {
        return Some("batch target does not match target connector".to_owned());
    }
    if batch.checkpoint_checksum_mismatch(target) {
        return Some(
            "checkpoint checksum does not match target, source, position and last event id"
                .to_owned(),
        );
    }
    let mut previous: Option<&BatchMember> = None;
    for member in &batch.members {
        let position = &member.source_position;
        if position.source_id != target.source_id {
            return Some("batch member belongs to a different source".to_owned());
        }
        if let Some(reason) = source_binding_error(
            &target.source_id,
            &target.checkpoint.source_id,
            member.event.source_id.as_str(),
        ) {
            return Some(reason);
        }
        if let Some(reason) = event_validation_error(&member.event) {
            return Some(reason);
        }
        if position.sequence > i64::MAX as u64 || position.offset > i64::MAX as u64 {
            return Some("checkpoint position exceeds PostgreSQL BIGINT range".to_owned());
        }
        if let Some(prior) = previous
            && (position.sequence <= prior.source_position.sequence
                || position.offset <= prior.source_position.offset)
        {
            return Some("batch member positions are not strictly ordered".to_owned());
        }
        previous = Some(member);
    }
    let Some(terminal) = batch.members.last() else {
        return Some("PostgreSQL target requires a non-empty batch".to_owned());
    };
    let advances = batch.members.iter().all(|member| {
        member.source_position.sequence > target.checkpoint.sequence
            && member.source_position.offset > target.checkpoint.offset
    });
    let exact_retry = terminal.source_position.sequence == target.checkpoint.sequence
        && terminal.source_position.offset == target.checkpoint.offset
        && target.checkpoint.last_idempotency_key.as_deref()
            == Some(batch.idempotency_key.as_str());
    if advances || exact_retry {
        None
    } else {
        Some("checkpoint position must advance, except for an exact same-key retry".to_owned())
    }
}

impl DeliveryBatch {
    fn checkpoint_checksum_mismatch(&self, target: &PgTarget<'_>) -> bool {
        target.checkpoint.checksum != target.checkpoint.expected_checksum(&target.target_id)
    }
}

// ---------------------------------------------------------------------------
// P6: target-specific idempotency and an executor seam
// ---------------------------------------------------------------------------

/// Version of the output row contract. It is part of the idempotency key so a
/// changed projection cannot be mistaken for a previously committed delivery.
pub const OUTPUT_CONTRACT_VERSION: u32 = crate::application::DEFAULT_OUTPUT_CONTRACT_VERSION;

/// Target-specific idempotency key. The same event delivered to two targets,
/// or under two output contract versions, is a different logical delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyKey {
    pub event_id: String,
    pub target_id: String,
    /// Empty only for the explicit legacy namespace created before source
    /// scoping. Live targets always bind a real source identity.
    pub source_id: String,
    pub output_contract_version: u32,
}

impl IdempotencyKey {
    /// Legacy key in the empty-source namespace, kept for older callers.
    #[must_use]
    pub fn new(event_id: &str, target_id: &str, output_contract_version: u32) -> Self {
        Self::with_source(event_id, target_id, "", output_contract_version)
    }

    /// Source-scoped key. The same event id delivered from a different source
    /// is a different logical delivery even at the same physical target.
    #[must_use]
    pub fn with_source(
        event_id: &str,
        target_id: &str,
        source_id: &str,
        output_contract_version: u32,
    ) -> Self {
        Self {
            event_id: event_id.to_owned(),
            target_id: target_id.to_owned(),
            source_id: source_id.to_owned(),
            output_contract_version,
        }
    }

    /// Stable text form using the ASCII unit separator, so no field value can
    /// collide with a concatenation boundary.
    #[must_use]
    pub fn as_text(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.event_id, self.target_id, self.source_id, self.output_contract_version
        )
    }
}

pub const INSERT_EVENT_WITH_KEY_SQL: &str = concat!(
    "INSERT INTO canonical_business_events ",
    "(idempotency_key, event_id, target_id, source_id, entity_id, kind, value, observed_at, raw_observation, schema_version, source_sequence, source_offset) ",
    "VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) ",
    "ON CONFLICT (idempotency_key) DO NOTHING",
);

pub const INSERT_DISPOSITION_SQL: &str = concat!(
    "INSERT INTO delivery_dispositions ",
    "(idempotency_key, event_id, target_id, source_id, disposition, reason, raw_observation) ",
    "VALUES ($1, $2, $3, $4, $5, $6, $7) ",
    "ON CONFLICT (idempotency_key) DO NOTHING",
);

pub const UPSERT_CHECKPOINT_SQL: &str = concat!(
    "INSERT INTO target_checkpoints ",
    "(target_id, source_id, output_contract_version, last_committed_sequence, last_committed_offset, checkpoint_checksum, last_idempotency_key) ",
    "VALUES ($1, $2, $3, $4, $5, $6, $7) ",
    "ON CONFLICT (target_id, source_id, output_contract_version) DO UPDATE SET ",
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
/// [`finalize_transcript`], so callers cannot commit a receipt still
/// marked `Outstanding`, `Uncertain`, `Delivered` or `Simulated`.
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
    build_source_commit_transcript(
        "",
        event,
        target_id,
        output_contract_version,
        last_committed_sequence,
        last_committed_offset,
        checkpoint_checksum,
        disposition,
        reason,
    )
}

/// Build the source-scoped transcript. `source_id` is empty only for the
/// explicit legacy namespace; live targets must bind a real source identity.
#[expect(
    clippy::too_many_arguments,
    reason = "the transcript needs source, target, contract version, checkpoint fields and disposition evidence"
)]
#[must_use]
pub fn build_source_commit_transcript(
    source_id: &str,
    event: &CanonicalEnvelope<CanonicalBusinessEvent>,
    target_id: &str,
    output_contract_version: u32,
    last_committed_sequence: u64,
    last_committed_offset: u64,
    checkpoint_checksum: &str,
    disposition: &str,
    reason: &str,
) -> CommitTranscript {
    let key = IdempotencyKey::with_source(
        event.event_id.as_str(),
        target_id,
        source_id,
        output_contract_version,
    );
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
                SqlValue::Text(source_id.to_owned()),
                SqlValue::Text(event.payload.entity_id.clone()),
                SqlValue::Text(event.payload.kind.clone()),
                SqlValue::Int(event.payload.value),
                SqlValue::Int(event.payload.observed_at),
                SqlValue::Text(event.raw_observation.clone().unwrap_or_default()),
                SqlValue::Int(i64::from(event.schema_version)),
                SqlValue::Int(i64::try_from(last_committed_sequence).unwrap_or(i64::MAX)),
                SqlValue::Int(i64::try_from(last_committed_offset).unwrap_or(i64::MAX)),
            ],
        },
        ParameterizedStatement {
            sql: INSERT_DISPOSITION_SQL.to_owned(),
            params: vec![
                SqlValue::Text(key_text),
                SqlValue::Text(event.event_id.as_str().to_owned()),
                SqlValue::Text(target_id.to_owned()),
                SqlValue::Text(source_id.to_owned()),
                SqlValue::Text(disposition.to_owned()),
                SqlValue::Text(reason.to_owned()),
                SqlValue::Text(event.raw_observation.clone().unwrap_or_default()),
            ],
        },
        ParameterizedStatement {
            sql: UPSERT_CHECKPOINT_SQL.to_owned(),
            params: vec![
                SqlValue::Text(target_id.to_owned()),
                SqlValue::Text(source_id.to_owned()),
                SqlValue::Int(i64::from(output_contract_version)),
                SqlValue::Int(i64::try_from(last_committed_sequence).unwrap_or(i64::MAX)),
                SqlValue::Int(i64::try_from(last_committed_offset).unwrap_or(i64::MAX)),
                SqlValue::Text(checkpoint_checksum.to_owned()),
                SqlValue::Text(event.event_id.as_str().to_owned()),
            ],
        },
    ];
    CommitTranscript {
        idempotency_key: key,
        statements,
    }
}

/// Build one transaction that commits every member of a validated batch.
///
/// Statement order is deterministic: `BEGIN`, one event insert per member in
/// source order, one disposition per member in the same order, one checkpoint
/// upsert to the final member, then a terminal marker chosen by the caller.
///
/// # Panics
///
/// Panics if `batch.members` is empty. Callers validate the batch before
/// constructing its transcript.
#[expect(
    clippy::expect_used,
    reason = "batch validation guarantees at least one member before transcript construction"
)]
#[must_use]
pub fn build_batch_commit_transcript(
    batch: &DeliveryBatch,
    terminal_checksum: &str,
) -> CommitTranscript {
    let mut statements = vec![ParameterizedStatement {
        sql: BEGIN_SQL.to_owned(),
        params: Vec::new(),
    }];
    for member in &batch.members {
        let key = IdempotencyKey::with_source(
            member.event.event_id.as_str(),
            &batch.target_id,
            &member.source_position.source_id,
            batch.output_contract_version,
        )
        .as_text();
        statements.push(ParameterizedStatement {
            sql: INSERT_EVENT_WITH_KEY_SQL.to_owned(),
            params: vec![
                SqlValue::Text(key),
                SqlValue::Text(member.event.event_id.as_str().to_owned()),
                SqlValue::Text(batch.target_id.clone()),
                SqlValue::Text(member.source_position.source_id.clone()),
                SqlValue::Text(member.event.payload.entity_id.clone()),
                SqlValue::Text(member.event.payload.kind.clone()),
                SqlValue::Int(member.event.payload.value),
                SqlValue::Int(member.event.payload.observed_at),
                SqlValue::Text(member.event.raw_observation.clone().unwrap_or_default()),
                SqlValue::Int(i64::from(member.event.schema_version)),
                SqlValue::Int(i64::try_from(member.source_position.sequence).unwrap_or(i64::MAX)),
                SqlValue::Int(i64::try_from(member.source_position.offset).unwrap_or(i64::MAX)),
            ],
        });
    }
    for member in &batch.members {
        let key = IdempotencyKey::with_source(
            member.event.event_id.as_str(),
            &batch.target_id,
            &member.source_position.source_id,
            batch.output_contract_version,
        )
        .as_text();
        statements.push(ParameterizedStatement {
            sql: INSERT_DISPOSITION_SQL.to_owned(),
            params: vec![
                SqlValue::Text(key),
                SqlValue::Text(member.event.event_id.as_str().to_owned()),
                SqlValue::Text(batch.target_id.clone()),
                SqlValue::Text(member.source_position.source_id.clone()),
                SqlValue::Text("durable_committed".to_owned()),
                SqlValue::Text(String::new()),
                SqlValue::Text(member.event.raw_observation.clone().unwrap_or_default()),
            ],
        });
    }
    let terminal = batch
        .members
        .last()
        .expect("validated batch has at least one member");
    statements.push(ParameterizedStatement {
        sql: UPSERT_CHECKPOINT_SQL.to_owned(),
        params: vec![
            SqlValue::Text(batch.target_id.clone()),
            SqlValue::Text(batch.source_id.clone()),
            SqlValue::Int(i64::from(batch.output_contract_version)),
            SqlValue::Int(i64::try_from(terminal.source_position.sequence).unwrap_or(i64::MAX)),
            SqlValue::Int(i64::try_from(terminal.source_position.offset).unwrap_or(i64::MAX)),
            SqlValue::Text(terminal_checksum.to_owned()),
            SqlValue::Text(batch.idempotency_key.clone()),
        ],
    });
    CommitTranscript {
        idempotency_key: IdempotencyKey::with_source(
            batch.members[0].event.event_id.as_str(),
            &batch.target_id,
            &batch.source_id,
            batch.output_contract_version,
        ),
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

/// Require the target, its checkpoint and the canonical envelope to name the
/// same source. The empty string is an explicit legacy namespace, not a
/// wildcard that accepts and erases modern source provenance.
fn source_binding_error(
    bound_source_id: &str,
    checkpoint_source_id: &str,
    envelope_source_id: &str,
) -> Option<String> {
    if checkpoint_source_id != bound_source_id {
        return Some(format!(
            "checkpoint source `{checkpoint_source_id}` does not match target source `{bound_source_id}`"
        ));
    }
    if envelope_source_id != bound_source_id {
        return Some(format!(
            "event source `{envelope_source_id}` does not match target source `{bound_source_id}`"
        ));
    }
    None
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
            source_position: None,
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
    fn source_binding_rejects_legacy_laundering_and_checkpoint_transplants() {
        let event = envelope("evt-1", "acct", 42);
        let source_a = event.source_id.as_str();
        let source_b = "src:other";

        assert_eq!(
            source_binding_error(source_a, source_a, source_a),
            None,
            "matching target, checkpoint and envelope sources must pass"
        );
        assert!(
            source_binding_error("", "", source_a)
                .expect("legacy namespace must reject modern source")
                .contains("event source"),
            "the legacy namespace must not erase modern source provenance"
        );
        assert!(
            source_binding_error(source_b, source_a, source_b)
                .expect("checkpoint from another source must be rejected")
                .contains("checkpoint source"),
            "a checkpoint must not be transplanted between sources"
        );
        assert!(
            source_binding_error(source_a, source_a, source_b)
                .expect("foreign envelope must be rejected")
                .contains("event source"),
            "an envelope from another source must be rejected"
        );
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
            7,
            "checkpoint upsert must include its last idempotency key"
        );
        assert_eq!(
            transcript.statements[3].params[6],
            SqlValue::Text(event.event_id.as_str().to_owned()),
            "checkpoint position must be bound to the canonical event id"
        );
        assert!(!transcript.has_commit());
        assert!(!transcript.has_rollback());

        let key = &transcript.idempotency_key;
        assert_eq!(key.event_id, event.event_id.as_str());
        assert_eq!(key.target_id, "pg-a");
        assert_eq!(
            key.source_id, "",
            "verified by the source-scoped integration contract"
        );
        assert_eq!(key.output_contract_version, OUTPUT_CONTRACT_VERSION);
        let other_source = IdempotencyKey::with_source(
            key.event_id.as_str(),
            "pg-a",
            "src-b",
            key.output_contract_version,
        );
        assert_ne!(
            key.as_text(),
            other_source.as_text(),
            "verified by the source-scoped integration contract"
        );
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
    fn source_scoped_identity_and_checksum_are_distinct() {
        let key_a = IdempotencyKey::with_source("evt-1", "pg-a", "src-a", OUTPUT_CONTRACT_VERSION);
        let key_b = IdempotencyKey::with_source("evt-1", "pg-a", "src-b", OUTPUT_CONTRACT_VERSION);
        assert_ne!(
            key_a.as_text(),
            key_b.as_text(),
            "verified by the source-scoped integration contract"
        );
        let checksum_a =
            checkpoint_checksum("pg-a", "src-a", OUTPUT_CONTRACT_VERSION, 1, 10, "evt-1");
        let checksum_b =
            checkpoint_checksum("pg-a", "src-b", OUTPUT_CONTRACT_VERSION, 1, 10, "evt-1");
        assert_ne!(
            checksum_a, checksum_b,
            "verified by the source-scoped integration contract"
        );
        let mut checkpoint = PgCheckpoint::initial_for_source("pg-a", "src-a");
        assert_eq!(
            checkpoint.source_id, "src-a",
            "verified by the source-scoped integration contract"
        );
        checkpoint.sequence = 1;
        checkpoint.offset = 10;
        checkpoint.checksum = checksum_a.clone();
        checkpoint.last_idempotency_key = Some("evt-1".to_owned());
        assert_eq!(
            checkpoint.expected_checksum("pg-a"),
            checksum_a,
            "verified by the source-scoped integration contract"
        );
    }

    #[test]
    fn legacy_checkpoint_checksums_are_verified_before_source_mapping() {
        let target_id = "pg-legacy";
        let sequence = 7;
        let offset = 700;
        let legacy_key = "evt-legacy";
        let v1 = legacy_checkpoint_checksum_v1(target_id, sequence, offset);
        let v2 = legacy_checkpoint_checksum_v2(target_id, sequence, offset, legacy_key);
        let current_legacy = checkpoint_checksum(
            target_id,
            "",
            OUTPUT_CONTRACT_VERSION,
            sequence,
            offset,
            legacy_key,
        );

        assert!(
            legacy_checkpoint_checksum_matches(
                target_id,
                OUTPUT_CONTRACT_VERSION,
                sequence,
                offset,
                "",
                &v1,
            ),
            "the original v1 checksum must remain readable without an event key"
        );
        assert!(
            legacy_checkpoint_checksum_matches(
                target_id,
                OUTPUT_CONTRACT_VERSION,
                sequence,
                offset,
                legacy_key,
                &v2,
            ),
            "the event-bound v2 checksum must remain readable"
        );
        assert!(
            legacy_checkpoint_checksum_matches(
                target_id,
                OUTPUT_CONTRACT_VERSION,
                sequence,
                offset,
                legacy_key,
                &current_legacy,
            ),
            "the current empty-source v4 checksum must remain readable"
        );
        assert!(
            !legacy_checkpoint_checksum_matches(
                target_id,
                OUTPUT_CONTRACT_VERSION,
                sequence,
                offset,
                legacy_key,
                "sha256:legacy",
            ),
            "an unrecognized legacy checksum must fail closed"
        );
        assert!(
            !legacy_checkpoint_checksum_matches(
                target_id,
                OUTPUT_CONTRACT_VERSION,
                sequence,
                offset,
                legacy_key,
                &v1,
            ),
            "the v1 checksum cannot validate a checkpoint with a recorded event key"
        );
    }

    #[test]
    fn checkpoint_checksum_binds_output_contract_version() {
        let current = checkpoint_checksum("pg-a", "src-a", OUTPUT_CONTRACT_VERSION, 1, 10, "evt-1");
        let next =
            checkpoint_checksum("pg-a", "src-a", OUTPUT_CONTRACT_VERSION + 1, 1, 10, "evt-1");
        assert_ne!(
            current, next,
            "changing only the output contract version must change the checkpoint checksum"
        );
    }

    #[test]
    fn source_scoped_transcript_embeds_source_in_event_and_checkpoint() {
        let event = envelope("evt-1", "acct", 42);
        let transcript = build_source_commit_transcript(
            "src-a",
            &event,
            "pg-a",
            OUTPUT_CONTRACT_VERSION,
            1,
            10,
            "sha256:test",
            "durable_committed",
            "",
        );
        assert_eq!(
            transcript.idempotency_key.source_id, "src-a",
            "verified by the source-scoped integration contract"
        );
        assert!(
            transcript.statements[1]
                .params
                .contains(&SqlValue::Text("src-a".to_owned())),
            "verified by the source-scoped integration contract"
        );
        assert!(
            transcript.statements[2]
                .params
                .contains(&SqlValue::Text("src-a".to_owned())),
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            transcript.statements[3].params[1],
            SqlValue::Text("src-a".to_owned()),
            "verified by the source-scoped integration contract"
        );
    }

    #[test]
    fn checkpoint_checksum_binds_the_last_event_id() {
        let checksum =
            checkpoint_checksum("pg-a", "src-a", OUTPUT_CONTRACT_VERSION, 7, 700, "evt-a");
        let checkpoint = PgCheckpoint {
            sequence: 7,
            offset: 700,
            checksum: checksum.clone(),
            last_idempotency_key: Some("evt-a".to_owned()),
            source_id: "src-a".to_owned(),
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
            checksum: checkpoint_checksum("pg-a", "", OUTPUT_CONTRACT_VERSION, 7, 700, ""),
            last_idempotency_key: None,
            source_id: String::new(),
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
