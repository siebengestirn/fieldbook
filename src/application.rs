//! Generic source/target reference application (Platform P5).
//!
//! This module is independent of market-data processing. It demonstrates that the
//! canonical envelope in [`crate::canonical`] is reusable by a second, unrelated
//! application: newline-delimited JSON observations in, typed business events
//! out, delivered to a target through an explicit idempotent delivery contract.
//! Nothing here imports or mutates the collector, exchange adapters, order
//! book, SBE writer, raw-frame journal, replay or catalog.
//!
//! The only shared types are the generic [`CanonicalEnvelope`] and the opaque
//! [`OpaqueId`] newtype; neither carries market semantics. No driver, live
//! database, network call, wall clock or performance claim is introduced. The
//! PostgreSQL side is a deterministic simulator plus a parameterized SQL
//! contract (see [`crate::application::postgres`]).
//!
//! Delivery is accounted, not assumed. Every receipt is reported in one of six
//! explicit categories, and a non-terminal outcome leaves a
//! [`DeliveryObligation`] behind and refuses to advance the committed
//! checkpoint. Replay and dry-run are simulated: they synthesize receipts
//! without touching the wrapped target or the durable store, so a simulated run
//! cannot perform an external side effect or move the durable checkpoint.
//!
//! Platform P6 adds three additive modules: [`file_source`] is a bounded-buffer
//! streaming source with a content fingerprint and a verified resume anchor;
//! [`durable`] is a real local append-only checksummed commit store; and the
//! [`postgres`] contract gains an executor seam and a transaction transcript.
//! The in-memory [`JsonlSource`] remains unchanged.

use std::{collections::BTreeMap, fmt::Write as FmtWrite};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    canonical::{CANONICAL_SCHEMA_VERSION, CanonicalEnvelope, EventId, SequenceMetadata},
    identity::OpaqueId,
};

pub mod durable;
pub mod file_source;
pub mod network_source;
pub mod postgres;

/// Schema version accepted by the JSONL reader. A record carrying a different
/// version is rejected with evidence instead of being silently coerced.
pub const JSONL_SCHEMA_VERSION: u32 = 1;

/// Bound on delivery attempts for one logical event inside a single run.
/// Retrying with the same idempotency key is safe by contract; the bound keeps
/// an unavailable target from turning into an unbounded loop.
pub const MAX_DELIVERY_ATTEMPTS: u32 = 2;

/// How a run is executed. Only [`ExecutionMode::Live`] may write to a target or
/// to the durable store. `DryRun` and `Replay` are simulated: no external write
/// and no durable checkpoint progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Live,
    DryRun,
    Replay,
}

impl ExecutionMode {
    #[must_use]
    pub const fn is_simulated(self) -> bool {
        matches!(self, Self::DryRun | Self::Replay)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::DryRun => "dry_run",
            Self::Replay => "replay",
        }
    }
}

/// The six report categories of a delivery receipt. `Accepted` is a legacy
/// non-terminal precursor of `Delivered` and is reported in the same category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptCategory {
    Delivered,
    DurableCommitted,
    Simulated,
    Outstanding,
    Uncertain,
    Rejected,
}

impl ReceiptCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::DurableCommitted => "durable_committed",
            Self::Simulated => "simulated",
            Self::Outstanding => "outstanding",
            Self::Uncertain => "uncertain",
            Self::Rejected => "rejected",
        }
    }
}

// ---------------------------------------------------------------------------
// Source contract
// ---------------------------------------------------------------------------

/// One raw observation with exact bytes, provenance and a local sequence.
/// `sequence` is assigned by the source to every physical input line, including
/// lines that are later rejected, so accounting can cover every record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawObservation {
    pub sequence: u64,
    pub line_number: usize,
    pub source_id: OpaqueId,
    pub record_id: String,
    /// Exact bytes of the line, excluding only the `\n` delimiter. A trailing
    /// `\r` from CRLF input is retained; no whitespace is normalized.
    pub raw_bytes: Vec<u8>,
    /// Stable textual reference into the source, independent of the payload.
    pub provenance: String,
}

impl RawObservation {
    /// Lower-case hexadecimal of the exact retained bytes. Used as the raw
    /// provenance pointer on the canonical envelope.
    #[must_use]
    pub fn hex_bytes(&self) -> String {
        let mut out = String::with_capacity(self.raw_bytes.len().saturating_mul(2));
        for byte in &self.raw_bytes {
            // Writing into a String cannot fail; the guarded branch keeps the
            // function total without unwrap in non-test code.
            if write!(out, "{byte:02x}").is_err() {
                return String::new();
            }
        }
        out
    }
}

/// Fail-closed read errors. Each variant carries the evidence needed to
/// account for the rejected line: its sequence, line number and exact bytes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    #[error("jsonl line {line_number} (sequence {sequence}) is malformed: {reason}")]
    MalformedLine {
        sequence: u64,
        line_number: usize,
        raw_bytes: Vec<u8>,
        reason: String,
    },
    #[error("jsonl line {line_number} (sequence {sequence}) failed schema validation: {reason}")]
    SchemaRejected {
        sequence: u64,
        line_number: usize,
        raw_bytes: Vec<u8>,
        reason: String,
    },
    #[error(
        "jsonl line {line_number} (sequence {sequence}) repeats id {record_id} first seen on line {first_line}"
    )]
    DuplicateId {
        sequence: u64,
        line_number: usize,
        first_line: usize,
        record_id: String,
        raw_bytes: Vec<u8>,
    },
    #[error(
        "jsonl line {line_number} (sequence {sequence}) exceeds {limit} bytes at offset {offset}"
    )]
    LineTooLong {
        sequence: u64,
        line_number: usize,
        offset: u64,
        limit: usize,
        raw_bytes: Vec<u8>,
    },
    #[error(
        "jsonl source I/O failure at line {line_number} (sequence {sequence}), offset {offset}: {message}"
    )]
    Io {
        sequence: u64,
        line_number: usize,
        offset: u64,
        message: String,
    },
}

impl SourceError {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::MalformedLine { sequence, .. }
            | Self::SchemaRejected { sequence, .. }
            | Self::DuplicateId { sequence, .. }
            | Self::LineTooLong { sequence, .. }
            | Self::Io { sequence, .. } => *sequence,
        }
    }

    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        match self {
            Self::MalformedLine { raw_bytes, .. }
            | Self::SchemaRejected { raw_bytes, .. }
            | Self::DuplicateId { raw_bytes, .. }
            | Self::LineTooLong { raw_bytes, .. } => raw_bytes,
            Self::Io { .. } => &[],
        }
    }
}

/// One observation at a time, with provenance and sequence attached by the
/// source. `Ok(None)` is a clean end of input; every `Err` still occupies one
/// sequence and must be accounted for by the caller.
pub trait SourceAdapter {
    fn read_one(&mut self) -> Result<Option<RawObservation>, SourceError>;
}

/// Deterministic newline-delimited JSON source over an in-memory byte buffer.
/// No I/O, clock or randomness is used, so every test is reproducible.
#[derive(Debug, Clone)]
pub struct JsonlSource {
    name: String,
    source_id: OpaqueId,
    lines: Vec<Vec<u8>>,
    next_index: usize,
    first_seen: BTreeMap<String, usize>,
}

impl JsonlSource {
    /// Build a source over exact bytes. Splitting on `\n` retains each line's
    /// bytes verbatim; a single trailing newline does not create a phantom
    /// empty record.
    #[must_use]
    pub fn new(name: &str, bytes: &[u8]) -> Self {
        let mut lines: Vec<Vec<u8>> = if bytes.is_empty() {
            Vec::new()
        } else {
            bytes
                .split(|byte| *byte == b'\n')
                .map(<[u8]>::to_vec)
                .collect()
        };
        if bytes.last() == Some(&b'\n') {
            lines.pop();
        }
        Self {
            name: name.to_owned(),
            source_id: OpaqueId::derive("jsonl", "jsonl-reference", "jsonl-source", name),
            lines,
            next_index: 0,
            first_seen: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn from_str(name: &str, text: &str) -> Self {
        Self::new(name, text.as_bytes())
    }

    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }
}

impl SourceAdapter for JsonlSource {
    fn read_one(&mut self) -> Result<Option<RawObservation>, SourceError> {
        let Some(line) = self.lines.get(self.next_index) else {
            return Ok(None);
        };
        let sequence = (self.next_index + 1) as u64;
        let line_number = self.next_index + 1;
        self.next_index += 1;
        let raw_bytes = line.clone();

        let value: serde_json::Value =
            serde_json::from_slice(&raw_bytes).map_err(|error| SourceError::MalformedLine {
                sequence,
                line_number,
                raw_bytes: raw_bytes.clone(),
                reason: error.to_string(),
            })?;
        let object = value
            .as_object()
            .ok_or_else(|| SourceError::MalformedLine {
                sequence,
                line_number,
                raw_bytes: raw_bytes.clone(),
                reason: "record must be a JSON object".to_owned(),
            })?;

        if let Some(version) = object.get("schema_version") {
            let found = version
                .as_u64()
                .ok_or_else(|| SourceError::SchemaRejected {
                    sequence,
                    line_number,
                    raw_bytes: raw_bytes.clone(),
                    reason: "schema_version must be an unsigned integer".to_owned(),
                })?;
            if found != u64::from(JSONL_SCHEMA_VERSION) {
                return Err(SourceError::SchemaRejected {
                    sequence,
                    line_number,
                    raw_bytes,
                    reason: format!(
                        "unsupported schema_version {found}; reader accepts {JSONL_SCHEMA_VERSION}"
                    ),
                });
            }
        }

        let record_id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| SourceError::SchemaRejected {
                sequence,
                line_number,
                raw_bytes: raw_bytes.clone(),
                reason: "id must be a non-empty string".to_owned(),
            })?;
        if !object
            .get("payload")
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(SourceError::SchemaRejected {
                sequence,
                line_number,
                raw_bytes,
                reason: "payload must be an object".to_owned(),
            });
        }

        if let Some(first_line) = self.first_seen.get(record_id) {
            return Err(SourceError::DuplicateId {
                sequence,
                line_number,
                first_line: *first_line,
                record_id: record_id.to_owned(),
                raw_bytes,
            });
        }
        self.first_seen.insert(record_id.to_owned(), line_number);

        Ok(Some(RawObservation {
            sequence,
            line_number,
            source_id: self.source_id.clone(),
            record_id: record_id.to_owned(),
            raw_bytes,
            provenance: format!("jsonl:{}:{line_number}", self.name),
        }))
    }
}

// ---------------------------------------------------------------------------
// Typed, non-market business payload and deterministic transformation
// ---------------------------------------------------------------------------

/// Typed business event with no price, side, venue,
/// book or instrument concept: it is an entity, a kind, an exact integer value
/// and the observation time supplied by the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalBusinessEvent {
    pub entity_id: String,
    pub kind: String,
    pub value: i64,
    pub observed_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonlRecord {
    id: String,
    payload: CanonicalBusinessEvent,
    /// Explicit local observation time. When absent, `receive_time` falls back
    /// to the source's `observed_at` for legacy inputs, but the two envelope
    /// fields remain independent and are populated from distinct inputs when
    /// both are present.
    #[serde(default)]
    received_at: Option<i64>,
    #[serde(default = "default_jsonl_schema_version")]
    schema_version: u32,
}

const fn default_jsonl_schema_version() -> u32 {
    JSONL_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransformError {
    #[error("payload schema rejected: {0}")]
    Schema(String),
    #[error("canonical envelope rejected: {0}")]
    Canonical(String),
}

/// Deterministic raw-observation -> canonical-envelope transformation.
/// No clock, randomness or environment is read. The event id is derived only
/// from the immutable source identity and record id, so a repeated delivery of
/// the same input yields the same idempotency key across resumes.
pub fn transform_observation(
    observation: &RawObservation,
) -> Result<CanonicalEnvelope<CanonicalBusinessEvent>, TransformError> {
    let record: JsonlRecord = serde_json::from_slice(&observation.raw_bytes)
        .map_err(|error| TransformError::Schema(error.to_string()))?;
    if record.id != observation.record_id {
        return Err(TransformError::Schema(
            "record id changed after source read".to_owned(),
        ));
    }
    if record.schema_version != JSONL_SCHEMA_VERSION {
        return Err(TransformError::Schema(format!(
            "unsupported schema_version {}; reader accepts {JSONL_SCHEMA_VERSION}",
            record.schema_version
        )));
    }
    if record.payload.entity_id.trim().is_empty() || record.payload.kind.trim().is_empty() {
        return Err(TransformError::Schema(
            "entity_id and kind must be non-empty".to_owned(),
        ));
    }

    let stable_key = format!(
        "{}\u{1f}{}",
        observation.source_id.as_str(),
        observation.record_id
    );
    let envelope = CanonicalEnvelope {
        event_id: EventId::derive("jsonl-postgres", "jsonl-source", &stable_key),
        instrument_id: None,
        source_id: observation.source_id.clone(),
        feed_id: None,
        connection_epoch: 0,
        sequence: Some(SequenceMetadata {
            source_seq: Some(observation.sequence),
            exchange_clock: None,
            receive_seq: observation.sequence,
        }),
        source_time: Some(record.payload.observed_at),
        receive_time: record.received_at.unwrap_or(record.payload.observed_at),
        raw_observation: Some(format!(
            "{}:{}",
            observation.provenance,
            observation.hex_bytes()
        )),
        schema_version: CANONICAL_SCHEMA_VERSION,
        payload: record.payload,
    };
    envelope
        .validate()
        .map_err(|error| TransformError::Canonical(error.to_string()))?;
    Ok(envelope)
}

// ---------------------------------------------------------------------------
// Target contract: receipts, obligations, idempotency
// ---------------------------------------------------------------------------

/// One batch submitted to a target. The idempotency key is the logical event
/// id; a target must treat a repeated key as the same logical delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRequest {
    pub idempotency_key: String,
    pub events: Vec<CanonicalEnvelope<CanonicalBusinessEvent>>,
}

/// State recorded on a [`DeliveryObligation`]. `Duplicate` is an accounting
/// projection of a committed receipt whose idempotency key was already known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Accepted,
    Delivered,
    Simulated,
    DurableCommitted,
    Duplicate,
    Rejected,
    Uncertain,
    Outstanding,
}

impl DeliveryState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Delivered => "delivered",
            Self::Simulated => "simulated",
            Self::DurableCommitted => "durable_committed",
            Self::Duplicate => "duplicate",
            Self::Rejected => "rejected",
            Self::Uncertain => "uncertain",
            Self::Outstanding => "outstanding",
        }
    }
}

/// Delivery receipt. `Accepted`/`Delivered` are non-terminal (target accepted,
/// local durability not yet confirmed); `DurableCommitted` and `Rejected` are
/// terminal; `Uncertain` and `Outstanding` are non-terminal and require a
/// retry with the same idempotency key. `Simulated` is produced only by
/// dry-run/replay and is terminal for accounting but never advances the
/// durable checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryReceipt {
    Accepted {
        idempotency_key: String,
        target_id: String,
    },
    Delivered {
        idempotency_key: String,
        target_id: String,
    },
    Simulated {
        idempotency_key: String,
        target_id: String,
    },
    DurableCommitted {
        idempotency_key: String,
        target_id: String,
        duplicate: bool,
    },
    Rejected {
        idempotency_key: String,
        target_id: String,
        reason: String,
    },
    Uncertain {
        idempotency_key: String,
        target_id: String,
        reason: String,
    },
    Outstanding {
        idempotency_key: String,
        target_id: String,
    },
}

impl DeliveryReceipt {
    #[must_use]
    pub const fn state(&self) -> DeliveryState {
        match self {
            Self::Accepted { .. } => DeliveryState::Accepted,
            Self::Delivered { .. } => DeliveryState::Delivered,
            Self::Simulated { .. } => DeliveryState::Simulated,
            Self::DurableCommitted { .. } => DeliveryState::DurableCommitted,
            Self::Rejected { .. } => DeliveryState::Rejected,
            Self::Uncertain { .. } => DeliveryState::Uncertain,
            Self::Outstanding { .. } => DeliveryState::Outstanding,
        }
    }

    #[must_use]
    pub const fn category(&self) -> ReceiptCategory {
        match self {
            Self::Accepted { .. } | Self::Delivered { .. } => ReceiptCategory::Delivered,
            Self::Simulated { .. } => ReceiptCategory::Simulated,
            Self::DurableCommitted { .. } => ReceiptCategory::DurableCommitted,
            Self::Rejected { .. } => ReceiptCategory::Rejected,
            Self::Uncertain { .. } => ReceiptCategory::Uncertain,
            Self::Outstanding { .. } => ReceiptCategory::Outstanding,
        }
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        match self {
            Self::Accepted {
                idempotency_key, ..
            }
            | Self::Delivered {
                idempotency_key, ..
            }
            | Self::Simulated {
                idempotency_key, ..
            }
            | Self::DurableCommitted {
                idempotency_key, ..
            }
            | Self::Rejected {
                idempotency_key, ..
            }
            | Self::Uncertain {
                idempotency_key, ..
            }
            | Self::Outstanding {
                idempotency_key, ..
            } => idempotency_key,
        }
    }

    #[must_use]
    pub fn target_id(&self) -> &str {
        match self {
            Self::Accepted { target_id, .. }
            | Self::Delivered { target_id, .. }
            | Self::Simulated { target_id, .. }
            | Self::DurableCommitted { target_id, .. }
            | Self::Rejected { target_id, .. }
            | Self::Uncertain { target_id, .. }
            | Self::Outstanding { target_id, .. } => target_id,
        }
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::DurableCommitted { .. } | Self::Rejected { .. } | Self::Simulated { .. }
        )
    }

    #[must_use]
    pub const fn is_duplicate(&self) -> bool {
        matches!(
            self,
            Self::DurableCommitted {
                duplicate: true,
                ..
            }
        )
    }
}

/// Target-side contract. Implementing this trait is the only requirement for
/// plugging a new destination into the runner.
pub trait TargetConnector {
    fn target_id(&self) -> &str;

    /// Deliver one batch and report its outcome. Implementations must be safe
    /// to call again with the same idempotency key.
    fn deliver(&mut self, request: &DeliveryRequest) -> DeliveryReceipt;

    /// Whether calling `deliver` is guaranteed to have no external side effect.
    /// Only side-effect-free connectors (for example the replay connector)
    /// override this; the runner uses it to avoid calling a real target in a
    /// simulated mode.
    fn is_simulation_safe(&self) -> bool {
        false
    }
}

/// One delivery obligation, tracked for the life of the import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryObligation {
    pub logical_event_id: String,
    pub target_id: String,
    pub attempts: u32,
    pub state: DeliveryState,
    pub last_error: Option<String>,
}

impl DeliveryObligation {
    #[must_use]
    pub fn new(logical_event_id: &str, target_id: &str) -> Self {
        Self {
            logical_event_id: logical_event_id.to_owned(),
            target_id: target_id.to_owned(),
            attempts: 0,
            state: DeliveryState::Outstanding,
            last_error: None,
        }
    }

    /// Fold one receipt into the obligation. Every call is one attempt.
    pub fn apply_receipt(&mut self, receipt: &DeliveryReceipt) {
        self.attempts += 1;
        match receipt {
            DeliveryReceipt::Accepted { .. } => {
                self.state = DeliveryState::Accepted;
                self.last_error = None;
            }
            DeliveryReceipt::Delivered { .. } => {
                self.state = DeliveryState::Delivered;
                self.last_error = None;
            }
            DeliveryReceipt::Simulated { .. } => {
                self.state = DeliveryState::Simulated;
                self.last_error = None;
            }
            DeliveryReceipt::DurableCommitted {
                duplicate: true, ..
            } => {
                self.state = DeliveryState::Duplicate;
                self.last_error = None;
            }
            DeliveryReceipt::DurableCommitted { .. } => {
                self.state = DeliveryState::DurableCommitted;
                self.last_error = None;
            }
            DeliveryReceipt::Rejected { reason, .. } => {
                self.state = DeliveryState::Rejected;
                self.last_error = Some(reason.clone());
            }
            DeliveryReceipt::Uncertain { reason, .. } => {
                self.state = DeliveryState::Uncertain;
                self.last_error = Some(reason.clone());
            }
            DeliveryReceipt::Outstanding { .. } => {
                self.state = DeliveryState::Outstanding;
                self.last_error = Some("no response from target".to_owned());
            }
        }
    }
}

/// Resume point: the last observation sequence whose disposition is terminal.
/// A non-terminal outcome never advances it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Checkpoint {
    pub last_committed_sequence: u64,
}

/// One target disposition inside a durable commit unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispositionRecord {
    pub sequence: u64,
    pub event_id: String,
    pub target_id: String,
    pub state: DeliveryState,
    pub reason: Option<String>,
}

/// Evidence for a line quarantined before any checkpoint progression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineRecord {
    pub sequence: u64,
    pub line_number: usize,
    pub byte_offset: u64,
    pub record_id: String,
    pub target_id: String,
    pub raw_bytes_hex: String,
    pub reason: String,
}

/// The single atomic commit unit handed to a durable sink. It carries the
/// committed ids, the dispositions/quarantine records and the new checkpoint
/// together: either all of it is fsynced as one checksummed record or none of
/// it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitIntent {
    pub target_id: String,
    pub committed_event_ids: Vec<String>,
    pub dispositions: Vec<DispositionRecord>,
    pub quarantines: Vec<QuarantineRecord>,
    /// Sequence the checkpoint is allowed to advance to.
    pub next_sequence: u64,
    /// Physical source offset belonging to `next_sequence`.
    pub next_offset: u64,
}

impl CommitIntent {
    #[must_use]
    pub fn delivered(
        target_id: &str,
        event_id: &str,
        sequence: u64,
        offset: u64,
        reason: Option<String>,
    ) -> Self {
        Self {
            target_id: target_id.to_owned(),
            committed_event_ids: vec![event_id.to_owned()],
            dispositions: vec![DispositionRecord {
                sequence,
                event_id: event_id.to_owned(),
                target_id: target_id.to_owned(),
                state: DeliveryState::DurableCommitted,
                reason,
            }],
            quarantines: Vec::new(),
            next_sequence: sequence,
            next_offset: offset,
        }
    }

    #[must_use]
    pub fn quarantined(target_id: &str, record: &QuarantineRecord) -> Self {
        Self {
            target_id: target_id.to_owned(),
            committed_event_ids: Vec::new(),
            dispositions: Vec::new(),
            quarantines: vec![record.clone()],
            next_sequence: record.sequence,
            next_offset: record.byte_offset,
        }
    }
}

/// Durable sink seam used by the runner. A rejected record is quarantined
/// first; only a successful quarantine or durable commit lets the runner
/// advance its checkpoint.
pub trait CommitSink {
    fn commit(&mut self, intent: &CommitIntent) -> Result<(), String>;
}

/// In-memory sink for the original in-memory-only runner and tests. It records
/// nothing, so a caller that needs durability must supply a real sink.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCommitSink;

impl CommitSink for NoopCommitSink {
    fn commit(&mut self, _intent: &CommitIntent) -> Result<(), String> {
        Ok(())
    }
}

/// Per-run accounting. The categories are disjoint and sum to `total_read`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub total_read: u64,
    pub delivered: u64,
    pub duplicates: u64,
    pub rejected: u64,
    pub outstanding: u64,
    pub uncertain: u64,
    pub accepted: u64,
    pub skipped: u64,
    /// Target-accepted but not locally durable (legacy `Accepted` receipts).
    pub target_delivered: u64,
    /// Informational projection of `delivered` that were also fsynced into the
    /// durable store. Not a separate accounting bucket.
    pub durable_committed: u64,
    /// Simulated receipts from dry-run/replay. Never move the checkpoint.
    pub simulated: u64,
    /// Informational count of rejected lines whose quarantine was durable.
    /// It is a projection of `rejected`, not a separate accounting bucket.
    pub quarantined: u64,
    pub last_committed_sequence: u64,
    pub obligations: Vec<DeliveryObligation>,
}

impl ImportReport {
    /// Accounting identity asserted by the runner's own tests: every raw
    /// record read in this run lands in exactly one bucket.
    #[must_use]
    pub const fn accounted(&self) -> u64 {
        self.delivered
            + self.duplicates
            + self.rejected
            + self.outstanding
            + self.uncertain
            + self.accepted
            + self.skipped
            + self.target_delivered
            + self.simulated
    }
}

/// Replay connector: records the receipts a run would have produced without
/// calling the wrapped target at all. This is the side-effect-free replay mode.
#[derive(Debug)]
pub struct ReplayConnector<'a, T> {
    inner: &'a mut T,
    receipts: Vec<DeliveryReceipt>,
}

impl<'a, T> ReplayConnector<'a, T> {
    #[must_use]
    pub const fn new(inner: &'a mut T) -> Self {
        Self {
            inner,
            receipts: Vec::new(),
        }
    }

    #[must_use]
    pub fn receipts(&self) -> &[DeliveryReceipt] {
        &self.receipts
    }
}

impl<T: TargetConnector> TargetConnector for ReplayConnector<'_, T> {
    fn target_id(&self) -> &str {
        self.inner.target_id()
    }

    fn is_simulation_safe(&self) -> bool {
        true
    }

    fn deliver(&mut self, request: &DeliveryRequest) -> DeliveryReceipt {
        // No call to `self.inner.deliver`: the destination sees nothing, and
        // no durable store is touched. A simulated receipt is never reported
        // as a durable commit and never advances the checkpoint.
        let receipt = DeliveryReceipt::Simulated {
            idempotency_key: request.idempotency_key.clone(),
            target_id: self.target_id().to_owned(),
        };
        self.receipts.push(receipt.clone());
        receipt
    }
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Original four-parameter entry point. It runs in [`ExecutionMode::Live`]
/// with an in-memory no-op sink; callers needing durable commits or simulated
/// modes should use [`run_jsonl_to_target_with_mode`].
pub fn run_jsonl_to_target<S, T, F>(
    source: &mut S,
    transform: F,
    target: &mut T,
    checkpoint: &Checkpoint,
) -> ImportReport
where
    S: SourceAdapter,
    T: TargetConnector,
    F: Fn(&RawObservation) -> Result<CanonicalEnvelope<CanonicalBusinessEvent>, TransformError>,
{
    run_jsonl_to_target_with_mode(
        source,
        transform,
        target,
        checkpoint,
        ExecutionMode::Live,
        &mut NoopCommitSink,
    )
}

/// Hex encoding of exact rejection bytes, used as durable quarantine evidence.
fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        if FmtWrite::write_fmt(&mut out, format_args!("{byte:02x}")).is_err() {
            return String::new();
        }
    }
    out
}

/// Run a source through a deterministic transform into a target with explicit
/// execution mode and durable sink.
///
/// In a simulated mode no external target is called unless that connector is
/// marked simulation-safe, no durable commit is written, and the checkpoint is
/// never advanced. A rejected record advances the checkpoint only after its
/// quarantine record has been durably accepted by the sink.
pub fn run_jsonl_to_target_with_mode<S, T, F>(
    source: &mut S,
    transform: F,
    target: &mut T,
    checkpoint: &Checkpoint,
    mode: ExecutionMode,
    sink: &mut dyn CommitSink,
) -> ImportReport
where
    S: SourceAdapter,
    T: TargetConnector,
    F: Fn(&RawObservation) -> Result<CanonicalEnvelope<CanonicalBusinessEvent>, TransformError>,
{
    let mut report = ImportReport::default();
    let mut frontier = checkpoint.last_committed_sequence;

    loop {
        match source.read_one() {
            Ok(None) => break,
            Ok(Some(observation)) => {
                report.total_read += 1;
                if observation.sequence <= checkpoint.last_committed_sequence {
                    report.skipped += 1;
                    continue;
                }
                let mut obligation = DeliveryObligation::new(
                    &format!(
                        "{}-{}",
                        observation.source_id.as_str(),
                        observation.record_id
                    ),
                    target.target_id(),
                );
                let (_retry_done, advance) = match transform(&observation) {
                    Err(error) => {
                        let quarantine = QuarantineRecord {
                            sequence: observation.sequence,
                            line_number: observation.line_number,
                            byte_offset: 0,
                            record_id: observation.record_id.clone(),
                            target_id: target.target_id().to_owned(),
                            raw_bytes_hex: hex_bytes(&observation.raw_bytes),
                            reason: error.to_string(),
                        };
                        let quarantined = mode.is_simulated()
                            || sink
                                .commit(&CommitIntent::quarantined(target.target_id(), &quarantine))
                                .is_ok();
                        if quarantined {
                            obligation.state = DeliveryState::Rejected;
                            obligation.attempts = 1;
                            obligation.last_error = Some(quarantine.reason.clone());
                            report.rejected += 1;
                            report.quarantined += 1;
                            (true, !mode.is_simulated())
                        } else {
                            obligation.state = DeliveryState::Uncertain;
                            obligation.attempts = 1;
                            obligation.last_error = Some("durable quarantine failed".to_owned());
                            report.uncertain += 1;
                            (true, false)
                        }
                    }
                    Ok(envelope) => {
                        let key = envelope.event_id.as_str().to_owned();
                        obligation.logical_event_id = key.clone();
                        let request = DeliveryRequest {
                            idempotency_key: key,
                            events: vec![envelope],
                        };
                        let simulated = mode.is_simulated();
                        let mut receipt = if simulated && !target.is_simulation_safe() {
                            DeliveryReceipt::Simulated {
                                idempotency_key: request.idempotency_key.clone(),
                                target_id: target.target_id().to_owned(),
                            }
                        } else {
                            let delivered = target.deliver(&request);
                            if simulated {
                                DeliveryReceipt::Simulated {
                                    idempotency_key: delivered.idempotency_key().to_owned(),
                                    target_id: delivered.target_id().to_owned(),
                                }
                            } else {
                                delivered
                            }
                        };
                        if !simulated {
                            obligation.apply_receipt(&receipt);
                            while !receipt.is_terminal()
                                && obligation.attempts < MAX_DELIVERY_ATTEMPTS
                            {
                                receipt = target.deliver(&request);
                                obligation.apply_receipt(&receipt);
                            }
                        } else {
                            obligation.apply_receipt(&receipt);
                        }

                        match receipt {
                            DeliveryReceipt::Simulated { .. } => {
                                report.simulated += 1;
                                (true, false)
                            }
                            DeliveryReceipt::DurableCommitted {
                                duplicate: false, ..
                            } => {
                                let intent = CommitIntent::delivered(
                                    target.target_id(),
                                    &request.idempotency_key,
                                    observation.sequence,
                                    0,
                                    None,
                                );
                                if sink.commit(&intent).is_ok() {
                                    report.delivered += 1;
                                    report.durable_committed += 1;
                                    (true, true)
                                } else {
                                    obligation.state = DeliveryState::Uncertain;
                                    obligation.last_error =
                                        Some("durable commit failed".to_owned());
                                    report.uncertain += 1;
                                    (true, false)
                                }
                            }
                            DeliveryReceipt::DurableCommitted {
                                duplicate: true, ..
                            } => {
                                // The target already has the row, but the local
                                // store must still record the disposition and
                                // checkpoint before the sequence can advance.
                                let intent = CommitIntent::delivered(
                                    target.target_id(),
                                    &request.idempotency_key,
                                    observation.sequence,
                                    0,
                                    Some("duplicate target receipt".to_owned()),
                                );
                                if sink.commit(&intent).is_ok() {
                                    report.duplicates += 1;
                                    report.durable_committed += 1;
                                    (true, true)
                                } else {
                                    obligation.state = DeliveryState::Uncertain;
                                    obligation.last_error =
                                        Some("durable commit failed".to_owned());
                                    report.uncertain += 1;
                                    (true, false)
                                }
                            }
                            DeliveryReceipt::Rejected { .. } => {
                                report.rejected += 1;
                                (true, true)
                            }
                            DeliveryReceipt::Accepted { .. } => {
                                report.accepted += 1;
                                (false, false)
                            }
                            DeliveryReceipt::Delivered { .. } => {
                                report.target_delivered += 1;
                                (false, false)
                            }
                            DeliveryReceipt::Uncertain { .. } => {
                                report.uncertain += 1;
                                (false, false)
                            }
                            DeliveryReceipt::Outstanding { .. } => {
                                report.outstanding += 1;
                                (false, false)
                            }
                        }
                    }
                };
                if advance && observation.sequence == frontier + 1 {
                    frontier = observation.sequence;
                }
                report.obligations.push(obligation);
            }
            Err(error) => {
                report.total_read += 1;
                let sequence = error.sequence();
                if sequence <= checkpoint.last_committed_sequence {
                    // A malformed or duplicate line before the checkpoint was
                    // already accounted for in an earlier run. It must not be
                    // counted again when the source is replayed from the start.
                    report.skipped += 1;
                    continue;
                }
                let reason = error.to_string();
                if matches!(error, SourceError::Io { .. }) {
                    // An I/O failure is not a rejected record: never quarantine
                    // it and never advance the checkpoint past it.
                    let mut obligation = DeliveryObligation::new(
                        &format!("jsonl-source-io:{sequence}"),
                        target.target_id(),
                    );
                    obligation.state = DeliveryState::Uncertain;
                    obligation.attempts = 1;
                    obligation.last_error = Some(reason);
                    report.uncertain += 1;
                    report.obligations.push(obligation);
                    continue;
                }
                let quarantine = QuarantineRecord {
                    sequence,
                    line_number: 0,
                    byte_offset: 0,
                    record_id: format!("jsonl-source-rejected:{sequence}"),
                    target_id: target.target_id().to_owned(),
                    raw_bytes_hex: hex_bytes(error.raw_bytes()),
                    reason: reason.clone(),
                };
                let quarantined = mode.is_simulated()
                    || sink
                        .commit(&CommitIntent::quarantined(target.target_id(), &quarantine))
                        .is_ok();
                let mut obligation = DeliveryObligation::new(
                    &format!("jsonl-source-rejected:{sequence}"),
                    target.target_id(),
                );
                obligation.attempts = 1;
                if quarantined {
                    obligation.state = DeliveryState::Rejected;
                    obligation.last_error = Some(reason);
                    report.rejected += 1;
                    report.quarantined += 1;
                    if !mode.is_simulated() && sequence == frontier + 1 {
                        frontier = sequence;
                    }
                } else {
                    obligation.state = DeliveryState::Uncertain;
                    obligation.last_error = Some("durable quarantine failed".to_owned());
                    report.uncertain += 1;
                }
                report.obligations.push(obligation);
            }
        }
    }

    report.last_committed_sequence = frontier;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::postgres::{PostgresFault, PostgresTarget};

    fn record(id: &str, entity: &str, kind: &str, value: i64, observed_at: i64) -> String {
        format!(
            "{{\"id\":\"{id}\",\"payload\":{{\"entity_id\":\"{entity}\",\"kind\":\"{kind}\",\"value\":{value},\"observed_at\":{observed_at}}}}}"
        )
    }

    fn source_with(records: &[String]) -> JsonlSource {
        JsonlSource::from_str("fixture", &records.join("\n"))
    }

    #[test]
    fn jsonl_source_retains_exact_bytes_and_sequence() {
        let first = record("a", "acct-1", "credit", 7, 100);
        let second = record("b", "acct-2", "debit", -3, 101);
        let mut source = source_with(&[first.clone(), second.clone()]);
        assert_eq!(source.line_count(), 2);

        let a = source.read_one().expect("first line").expect("observation");
        assert_eq!(a.sequence, 1);
        assert_eq!(a.line_number, 1);
        assert_eq!(a.record_id, "a");
        assert_eq!(a.raw_bytes, first.as_bytes());
        assert_eq!(a.provenance, "jsonl:fixture:1");
        assert_eq!(a.source_id, source.source_id().clone());
        let expected_hex = first
            .as_bytes()
            .iter()
            .fold(String::new(), |mut out, byte| {
                let _ = write!(out, "{byte:02x}");
                out
            });
        assert_eq!(a.hex_bytes(), expected_hex);

        let b = source
            .read_one()
            .expect("second line")
            .expect("observation");
        assert_eq!(b.sequence, 2);
        assert_eq!(b.raw_bytes, second.as_bytes());
        assert!(source.read_one().expect("end of input").is_none());
    }

    #[test]
    fn malformed_line_carries_rejection_evidence() {
        let mut source = JsonlSource::from_str("fixture", "{not json}\n");
        let error = source.read_one().expect_err("malformed line");
        assert_eq!(error.sequence(), 1);
        assert_eq!(error.raw_bytes(), b"{not json}");
        assert!(matches!(error, SourceError::MalformedLine { reason, .. } if !reason.is_empty()));
        assert!(source.read_one().expect("end").is_none());
    }

    #[test]
    fn duplicate_id_is_rejected_with_first_line_evidence() {
        let line = record("same", "acct", "credit", 1, 1);
        let mut source = source_with(&[line.clone(), line.clone()]);
        let _ = source.read_one().expect("first");
        let error = source.read_one().expect_err("duplicate");
        assert!(matches!(
            error,
            SourceError::DuplicateId {
                sequence: 2,
                line_number: 2,
                first_line: 1,
                ref record_id,
                ..
            } if record_id == "same"
        ));
        assert_eq!(error.raw_bytes(), line.as_bytes());
    }

    #[test]
    fn schema_rejection_is_fail_closed() {
        let bad_version = r#"{"id":"x","schema_version":99,"payload":{"entity_id":"e","kind":"k","value":1,"observed_at":1}}"#.to_owned();
        let mut source = JsonlSource::from_str("fixture", &bad_version);
        assert!(matches!(
            source.read_one(),
            Err(SourceError::SchemaRejected { reason, .. }) if reason.contains("unsupported schema_version")
        ));

        let bad_payload = "{\"id\":\"x\",\"payload\":{\"entity_id\":\"e\",\"kind\":\"k\",\"value\":1.5,\"observed_at\":1}}";
        let mut source = JsonlSource::from_str("fixture", bad_payload);
        let observation = source.read_one().expect("read").expect("observation");
        assert!(matches!(
            transform_observation(&observation),
            Err(TransformError::Schema(_))
        ));
    }

    #[test]
    fn transform_is_deterministic_and_preserves_raw_provenance() {
        let line = record("a", "acct-1", "credit", 7, 100);
        let mut source = source_with(std::slice::from_ref(&line));
        let observation = source.read_one().expect("read").expect("observation");
        let first = transform_observation(&observation).expect("transform");
        let second = transform_observation(&observation).expect("transform");
        assert_eq!(first, second);
        assert_eq!(first.event_id, second.event_id);
        assert_eq!(first.payload.entity_id, "acct-1");
        assert_eq!(first.payload.kind, "credit");
        assert_eq!(first.payload.value, 7);
        assert_eq!(first.source_time, Some(100));
        assert_eq!(first.receive_time, 100);
        assert!(first.raw_observation.as_deref().is_some_and(|raw| {
            raw.contains("jsonl:fixture:1") && raw.contains(&observation.hex_bytes())
        }));
        assert!(first.validate().is_ok());
    }

    #[test]
    fn transform_separates_source_time_from_receive_time() {
        let line = "{\"id\":\"a\",\"received_at\":250,\"payload\":{\"entity_id\":\"e\",\"kind\":\"k\",\"value\":1,\"observed_at\":100}}";
        let mut source = JsonlSource::from_str("fixture", line);
        let observation = source.read_one().expect("read").expect("observation");
        let envelope = transform_observation(&observation).expect("transform");
        assert_eq!(envelope.source_time, Some(100));
        assert_eq!(envelope.receive_time, 250);
        assert_ne!(envelope.source_time, Some(envelope.receive_time));
    }

    #[test]
    fn normal_import_accounts_for_every_record() {
        let lines: Vec<String> = (0..3)
            .map(|index| record(&format!("id-{index}"), "acct", "credit", index, 100 + index))
            .collect();
        let mut source = source_with(&lines);
        let mut target = PostgresTarget::new("pg-local");
        let report = run_jsonl_to_target(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
        );

        assert_eq!(report.total_read, 3);
        assert_eq!(report.delivered, 3);
        assert_eq!(report.duplicates, 0);
        assert_eq!(report.rejected, 0);
        assert_eq!(report.outstanding, 0);
        assert_eq!(report.accounted(), report.total_read);
        assert_eq!(report.last_committed_sequence, 3);
        assert_eq!(target.simulator().committed_count(), 3);
    }

    #[test]
    fn interrupted_import_resumes_from_checkpoint() {
        let lines: Vec<String> = (0..3)
            .map(|index| record(&format!("id-{index}"), "acct", "credit", index, 100 + index))
            .collect();

        let mut first_source = source_with(&lines);
        let mut interrupted = PostgresTarget::with_fault(
            "pg-local",
            PostgresFault::ServerUnavailableAfter { successful: 1 },
        );
        let first = run_jsonl_to_target(
            &mut first_source,
            transform_observation,
            &mut interrupted,
            &Checkpoint::default(),
        );
        assert_eq!(first.delivered, 1);
        assert_eq!(first.outstanding, 2);
        assert_eq!(first.last_committed_sequence, 1);
        assert_eq!(interrupted.simulator().committed_count(), 1);

        let checkpoint = Checkpoint {
            last_committed_sequence: first.last_committed_sequence,
        };
        let mut second_source = source_with(&lines);
        let mut resumed = PostgresTarget::new("pg-local");
        let second = run_jsonl_to_target(
            &mut second_source,
            transform_observation,
            &mut resumed,
            &checkpoint,
        );
        assert_eq!(second.skipped, 1);
        assert_eq!(second.delivered, 2);
        assert_eq!(second.last_committed_sequence, 3);
        assert_eq!(second.accounted(), second.total_read);
    }

    #[test]
    fn resume_does_not_recount_prior_rejections() {
        let bad = "{not json}";
        let good = record("a", "acct", "credit", 1, 1);
        let text = format!("{bad}\n{good}");
        let mut first_source = JsonlSource::from_str("fixture", &text);
        let mut first_target = PostgresTarget::new("pg-local");
        let first = run_jsonl_to_target(
            &mut first_source,
            transform_observation,
            &mut first_target,
            &Checkpoint::default(),
        );
        assert_eq!(first.rejected, 1);
        assert_eq!(first.delivered, 1);
        assert_eq!(first.last_committed_sequence, 2);

        let mut second_source = JsonlSource::from_str("fixture", &text);
        let mut second_target = PostgresTarget::new("pg-local");
        let second = run_jsonl_to_target(
            &mut second_source,
            transform_observation,
            &mut second_target,
            &Checkpoint {
                last_committed_sequence: first.last_committed_sequence,
            },
        );
        assert_eq!(second.total_read, 2);
        assert_eq!(second.skipped, 2);
        assert_eq!(second.rejected, 0);
        assert_eq!(second.delivered, 0);
        assert_eq!(second.accounted(), second.total_read);
    }

    #[test]
    fn target_unavailable_leaves_outstanding_obligation() {
        let mut source = source_with(&[record("a", "acct", "credit", 1, 1)]);
        let mut target = PostgresTarget::with_fault("pg-local", PostgresFault::ServerUnavailable);
        let report = run_jsonl_to_target(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
        );

        assert_eq!(report.outstanding, 1);
        assert_eq!(report.delivered, 0);
        assert_eq!(report.last_committed_sequence, 0);
        assert_eq!(target.simulator().committed_count(), 0);
        let obligation = &report.obligations[0];
        assert_eq!(obligation.attempts, MAX_DELIVERY_ATTEMPTS);
        assert_eq!(obligation.state, DeliveryState::Outstanding);
        assert!(obligation.last_error.is_some());
    }

    #[test]
    fn lost_acknowledgement_is_resolved_safely() {
        let mut source = source_with(&[record("a", "acct", "credit", 1, 1)]);
        let mut target = PostgresTarget::with_fault("pg-local", PostgresFault::AckLost);
        let report = run_jsonl_to_target(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
        );

        assert_eq!(report.duplicates, 1);
        assert_eq!(report.uncertain, 0);
        assert_eq!(report.delivered, 0);
        assert_eq!(report.last_committed_sequence, 1);
        assert_eq!(target.simulator().committed_count(), 1);
        assert_eq!(report.obligations[0].state, DeliveryState::Duplicate);
        assert_eq!(report.obligations[0].attempts, 2);
    }

    #[test]
    fn replay_disables_target_side_effects() {
        let lines: Vec<String> = (0..2)
            .map(|index| record(&format!("id-{index}"), "acct", "credit", index, 1))
            .collect();
        let mut source = source_with(&lines);
        let mut target = PostgresTarget::new("pg-local");
        let mut replay = ReplayConnector::new(&mut target);
        let report = run_jsonl_to_target_with_mode(
            &mut source,
            transform_observation,
            &mut replay,
            &Checkpoint::default(),
            ExecutionMode::Replay,
            &mut NoopCommitSink,
        );

        assert_eq!(report.simulated, 2);
        assert_eq!(report.delivered, 0);
        assert_eq!(report.last_committed_sequence, 0);
        assert_eq!(report.accounted(), report.total_read);
        assert_eq!(replay.receipts().len(), 2);
        assert!(
            replay
                .receipts()
                .iter()
                .all(|receipt| receipt.state() == DeliveryState::Simulated)
        );
        assert_eq!(target.simulator().committed_count(), 0);
        assert!(target.statements().is_empty());
        assert!(target.receipts().is_empty());
    }

    #[test]
    fn receipts_and_obligations_distinguish_all_states() {
        let key = "evt:1";
        let cases = [
            (
                DeliveryReceipt::Accepted {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                },
                DeliveryState::Accepted,
            ),
            (
                DeliveryReceipt::DurableCommitted {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                    duplicate: false,
                },
                DeliveryState::DurableCommitted,
            ),
            (
                DeliveryReceipt::DurableCommitted {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                    duplicate: true,
                },
                DeliveryState::Duplicate,
            ),
            (
                DeliveryReceipt::Rejected {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                    reason: "schema".to_owned(),
                },
                DeliveryState::Rejected,
            ),
            (
                DeliveryReceipt::Uncertain {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                    reason: "ack lost".to_owned(),
                },
                DeliveryState::Uncertain,
            ),
            (
                DeliveryReceipt::Outstanding {
                    idempotency_key: key.to_owned(),
                    target_id: "pg".to_owned(),
                },
                DeliveryState::Outstanding,
            ),
        ];
        for (receipt, expected) in &cases {
            let receipt_state = if *expected == DeliveryState::Duplicate {
                DeliveryState::DurableCommitted
            } else {
                *expected
            };
            assert_eq!(receipt.state(), receipt_state);
            let mut obligation = DeliveryObligation::new(key, "pg");
            obligation.apply_receipt(receipt);
            assert_eq!(obligation.state, *expected);
            assert_eq!(obligation.attempts, 1);
            assert_eq!(obligation.logical_event_id, key);
            assert_eq!(obligation.target_id, "pg");
            assert_eq!(obligation.state.as_str(), expected.as_str());
        }
        assert!(!cases[0].0.is_terminal());
        assert!(cases[1].0.is_terminal());
        assert!(cases[2].0.is_duplicate());
    }
}
