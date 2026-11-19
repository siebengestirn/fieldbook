//! Real local durable commit store (Platform P6).
//!
//! This is an append-only, checksummed, fsync-capable store built from the
//! existing atomic JSON helper in `crate::report` and `sha2`. It is deliberately
//! **not** the market-data journal and reuses none of its types or formats.
//!
//! Each commit is a separate, never-rewritten file named
//! `commit-<12-digit sequence>.json`. One file is one atomic commit unit: it
//! contains the committed event ids, the dispositions and quarantine records,
//! and the new checkpoint, plus a SHA-256 of the exact serialized record. A
//! record is written through `write_json_atomic`, which writes to a sibling
//! temporary file, fsyncs it, renames it into place and fsyncs the parent
//! directory. On reopen every record is parsed, its checksum is recomputed and
//! the sequence is checked for gaps; truncation, corruption, unsupported
//! schema versions and checkpoint-binding mismatches fail closed.
//!
//! The store is local and single-writer. It does **not** provide replication,
//! a database transaction, or a live PostgreSQL connection. The sibling
//! `postgres` module has a separate opt-in live target; it is not this local
//! commit store and does not change this store's failure boundaries.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    application::{
        CommitIntent, CommitSink, DispositionRecord, ImportReport, QuarantineRecord,
        file_source::{CheckpointBinding, DurableCheckpoint, FILE_SCHEMA_VERSION, FileSource},
    },
    report::{serialize_json_pretty, write_json_atomic},
};

/// Version of one on-disk commit record envelope.
pub const COMMIT_SCHEMA_VERSION: u32 = 1;

const CHECKSUM_PREFIX: &str = "sha256:";

/// The payload of one atomic commit. All fields live in the same checksummed
/// record, so a partially written commit can never expose a checkpoint without
/// its dispositions, or dispositions without their committed ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRecord {
    pub target_id: String,
    pub committed_event_ids: Vec<String>,
    pub dispositions: Vec<DispositionRecord>,
    pub quarantines: Vec<QuarantineRecord>,
    pub checkpoint: DurableCheckpoint,
}

/// One checksummed file on disk. `checksum` covers the canonical compact JSON
/// serialization of `record` only, not the envelope, so re-serializing with a
/// different pretty printer cannot change verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitEnvelope {
    pub schema_version: u32,
    pub sequence: u64,
    pub record: CommitRecord,
    pub checksum: String,
}

/// Deterministic fault injection for partial-write and corruption tests. No
/// timer, probability or host state is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StoreFault {
    #[default]
    None,
    /// Fail before touching the directory; the store stays unchanged.
    FailBeforeWrite,
    /// Write only the first half of the record, simulating a crash mid-write.
    TruncatedAppend,
    /// Write a complete record with a deliberately wrong checksum.
    CorruptChecksum,
}

/// Fail-closed store errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    #[error("commit store I/O failure: {0}")]
    Io(String),
    #[error("commit store serialization failure: {0}")]
    Serialize(String),
    #[error("commit record `{path}` is corrupt: {reason}")]
    Corrupt { path: String, reason: String },
    #[error("commit record `{path}` has schema version {found}, expected {expected}")]
    UnsupportedVersion {
        path: String,
        found: u32,
        expected: u32,
    },
    #[error("commit sequence gap: expected `{expected}`, found `{found}`")]
    SequenceGap { expected: String, found: String },
    #[error("commit record `{path}` checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        found: String,
    },
    #[error("commit record `{path}` checkpoint is bound to a different source or target")]
    BindingMismatch { path: String },
    #[error("fault injected: {0}")]
    FaultInjected(String),
}

fn checksum_of(record: &CommitRecord) -> Result<String, StoreError> {
    let bytes =
        serde_json::to_vec(record).map_err(|error| StoreError::Serialize(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut text = String::with_capacity(CHECKSUM_PREFIX.len() + digest.len() * 2);
    text.push_str(CHECKSUM_PREFIX);
    for byte in digest {
        if std::fmt::Write::write_fmt(&mut text, format_args!("{byte:02x}")).is_err() {
            return Err(StoreError::Serialize(
                "checksum hex encoding failed".to_owned(),
            ));
        }
    }
    Ok(text)
}

fn record_name(sequence: u64) -> String {
    format!("commit-{sequence:012}.json")
}

fn binding_matches(binding: &CheckpointBinding, record: &CommitRecord) -> bool {
    binding.source_id == record.checkpoint.source_id
        && binding.source_fingerprint == record.checkpoint.source_fingerprint
        && binding.target_id == record.checkpoint.target_id
        && binding.target_id == record.target_id
}

/// Folded, verified state of a commit store after a clean reopen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DurableState {
    pub records: Vec<CommitRecord>,
}

impl DurableState {
    #[must_use]
    pub fn checkpoint(&self) -> Option<&DurableCheckpoint> {
        self.records.last().map(|record| &record.checkpoint)
    }

    #[must_use]
    pub fn committed_event_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        for record in &self.records {
            for id in &record.committed_event_ids {
                if !ids.contains(&id.as_str()) {
                    ids.push(id.as_str());
                }
            }
        }
        ids
    }

    #[must_use]
    pub fn is_committed(&self, event_id: &str) -> bool {
        self.records
            .iter()
            .any(|record| record.committed_event_ids.iter().any(|id| id == event_id))
    }

    #[must_use]
    pub fn quarantine_count(&self) -> usize {
        self.records
            .iter()
            .map(|record| record.quarantines.len())
            .sum()
    }
}

/// Verify and fold every commit record in a directory. An absent directory is
/// an empty state; a present but corrupt directory fails closed.
pub fn load_state(
    directory: &Path,
    binding: Option<&CheckpointBinding>,
) -> Result<DurableState, StoreError> {
    if !directory.exists() {
        return Ok(DurableState::default());
    }
    let entries =
        std::fs::read_dir(directory).map_err(|error| StoreError::Io(error.to_string()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| StoreError::Io(error.to_string()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("commit-") {
            names.push(name);
        }
    }
    names.sort();

    let mut records = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let expected = record_name((index + 1) as u64);
        if *name != expected {
            return Err(StoreError::SequenceGap {
                expected,
                found: name.clone(),
            });
        }
        let path = directory.join(name);
        let bytes = std::fs::read(&path).map_err(|error| StoreError::Io(error.to_string()))?;
        let envelope: CommitEnvelope =
            serde_json::from_slice(&bytes).map_err(|error| StoreError::Corrupt {
                path: name.clone(),
                reason: error.to_string(),
            })?;
        if envelope.schema_version != COMMIT_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedVersion {
                path: name.clone(),
                found: envelope.schema_version,
                expected: COMMIT_SCHEMA_VERSION,
            });
        }
        if envelope.sequence != (index + 1) as u64 {
            return Err(StoreError::Corrupt {
                path: name.clone(),
                reason: format!(
                    "record sequence {} does not match file position {}",
                    envelope.sequence,
                    index + 1
                ),
            });
        }
        let computed = checksum_of(&envelope.record)?;
        if computed != envelope.checksum {
            return Err(StoreError::ChecksumMismatch {
                path: name.clone(),
                expected: computed,
                found: envelope.checksum,
            });
        }
        if let Some(binding) = binding
            && !binding_matches(binding, &envelope.record)
        {
            return Err(StoreError::BindingMismatch { path: name.clone() });
        }
        records.push(envelope.record);
    }
    Ok(DurableState { records })
}

/// Append-only local commit store. Every successful `commit` produces exactly
/// one new checksummed file; nothing is ever rewritten or deleted.
#[derive(Debug)]
pub struct CommitStore {
    directory: PathBuf,
    binding: CheckpointBinding,
    state: DurableState,
    next_sequence: u64,
    fault: StoreFault,
}

impl CommitStore {
    /// Open a store at `directory`, create it if absent, and verify every
    /// existing record against `binding` before exposing any state.
    pub fn open(directory: &Path, binding: CheckpointBinding) -> Result<Self, StoreError> {
        std::fs::create_dir_all(directory).map_err(|error| StoreError::Io(error.to_string()))?;
        let state = load_state(directory, Some(&binding))?;
        let next_sequence = state.records.len() as u64 + 1;
        Ok(Self {
            directory: directory.to_path_buf(),
            binding,
            state,
            next_sequence,
            fault: StoreFault::default(),
        })
    }

    #[must_use]
    pub const fn binding(&self) -> &CheckpointBinding {
        &self.binding
    }

    #[must_use]
    pub const fn state(&self) -> &DurableState {
        &self.state
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn set_fault(&mut self, fault: StoreFault) {
        self.fault = fault;
    }

    /// Append one atomic, checksummed commit record. The whole record (ids,
    /// dispositions/quarantines and new checkpoint) is one checksum unit.
    pub fn commit(&mut self, record: CommitRecord) -> Result<(), StoreError> {
        if !binding_matches(&self.binding, &record) {
            return Err(StoreError::BindingMismatch {
                path: record_name(self.next_sequence),
            });
        }
        if self.fault == StoreFault::FailBeforeWrite {
            return Err(StoreError::FaultInjected("failed before append".to_owned()));
        }
        let envelope = CommitEnvelope {
            schema_version: COMMIT_SCHEMA_VERSION,
            sequence: self.next_sequence,
            checksum: checksum_of(&record)?,
            record,
        };
        let path = self.directory.join(record_name(self.next_sequence));
        match self.fault {
            StoreFault::None => {
                write_json_atomic(&path, &envelope)
                    .map_err(|error| StoreError::Io(error.to_string()))?;
                self.state.records.push(envelope.record);
            }
            StoreFault::TruncatedAppend => {
                let bytes = serialize_json_pretty(&path, &envelope)
                    .map_err(|error| StoreError::Serialize(error.to_string()))?;
                let half = bytes.len() / 2;
                std::fs::write(&path, &bytes[..half])
                    .map_err(|error| StoreError::Io(error.to_string()))?;
            }
            StoreFault::CorruptChecksum => {
                let mut corrupt = envelope.clone();
                corrupt.checksum = format!(
                    "{CHECKSUM_PREFIX}0000000000000000000000000000000000000000000000000000000000000000"
                );
                write_json_atomic(&path, &corrupt)
                    .map_err(|error| StoreError::Io(error.to_string()))?;
                self.state.records.push(envelope.record);
            }
            StoreFault::FailBeforeWrite => {
                return Err(StoreError::FaultInjected("failed before append".to_owned()));
            }
        }
        self.next_sequence += 1;
        Ok(())
    }

    /// Re-read the directory from scratch. A partial or corrupt append makes
    /// this fail closed even though the in-process state looked advanced.
    pub fn verify(&self) -> Result<DurableState, StoreError> {
        load_state(&self.directory, Some(&self.binding))
    }
}

impl CommitSink for CommitStore {
    fn commit(&mut self, intent: &CommitIntent) -> Result<(), String> {
        let checkpoint = DurableCheckpoint {
            schema_version: FILE_SCHEMA_VERSION,
            source_id: self.binding.source_id.clone(),
            source_fingerprint: self.binding.source_fingerprint.clone(),
            target_id: intent.target_id.clone(),
            last_committed_sequence: intent.next_sequence,
            last_committed_line: intent.next_sequence as usize,
            last_committed_offset: intent.next_offset,
        };
        let record = CommitRecord {
            target_id: intent.target_id.clone(),
            committed_event_ids: intent.committed_event_ids.clone(),
            dispositions: intent.dispositions.clone(),
            quarantines: intent.quarantines.clone(),
            checkpoint,
        };
        CommitStore::commit(self, record).map_err(|error| error.to_string())
    }
}

/// Result of one streaming pipeline run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingResult {
    pub report: ImportReport,
    pub checkpoint: DurableCheckpoint,
}

/// Run a bounded streaming source into a target with a durable local store.
///
/// The anchor must already have been verified against the source identity and
/// target. Every terminal outcome is committed to the store as one atomic
/// record before the in-memory checkpoint is advanced. The runner stops at
/// the first unresolved delivery or failed local commit, so a later source
/// position can never leap over an unaccounted record. Callers wanting a
/// simulated run should use
/// `run_jsonl_to_target_with_mode` with a side-effect-free connector instead of
/// this store-backed runner.
pub fn run_streaming_pipeline<R, T, F>(
    source: &mut FileSource<R>,
    transform: F,
    target: &mut T,
    anchor: &DurableCheckpoint,
    store: &mut CommitStore,
) -> StreamingResult
where
    R: std::io::BufRead,
    T: crate::application::TargetConnector,
    F: Fn(
        &crate::application::RawObservation,
    ) -> Result<
        crate::canonical::CanonicalEnvelope<crate::application::CanonicalBusinessEvent>,
        crate::application::TransformError,
    >,
{
    use crate::application::{
        CommitIntent, DeliveryObligation, DeliveryReceipt, DeliveryState, MAX_DELIVERY_ATTEMPTS,
        QuarantineRecord, SourceAdapter, SourcePosition,
    };

    let mut report = ImportReport::default();
    let mut frontier = anchor.clone();

    loop {
        match source.read_one() {
            Ok(None) => break,
            Ok(Some(observation)) => {
                report.total_read += 1;
                if observation.sequence <= frontier.last_committed_sequence {
                    report.skipped += 1;
                    continue;
                }
                let envelope = match transform(&observation) {
                    Ok(envelope) => envelope,
                    Err(error) => {
                        let quarantine = QuarantineRecord {
                            sequence: observation.sequence,
                            line_number: observation.line_number,
                            byte_offset: source.last_offset(),
                            record_id: observation.record_id.clone(),
                            target_id: target.target_id().to_owned(),
                            raw_bytes_hex: crate::application::hex_bytes(&observation.raw_bytes),
                            reason: error.to_string(),
                        };
                        if CommitSink::commit(
                            store,
                            &CommitIntent::quarantined(target.target_id(), &quarantine),
                        )
                        .is_ok()
                        {
                            report.rejected += 1;
                            report.quarantined += 1;
                            frontier =
                                frontier.advanced(observation.sequence, source.next_offset());
                        } else {
                            report.uncertain += 1;
                            break;
                        }
                        continue;
                    }
                };
                let key = envelope.event_id.as_str().to_owned();
                let request = crate::application::DeliveryRequest {
                    idempotency_key: key.clone(),
                    events: vec![envelope],
                    source_position: Some(SourcePosition {
                        source_id: observation.source_id.as_str().to_owned(),
                        sequence: observation.sequence,
                        offset: source.next_offset(),
                    }),
                };
                let mut obligation = DeliveryObligation::new(&key, target.target_id());
                let mut receipt = target.deliver(&request);
                obligation.apply_receipt(&receipt);
                while !receipt.is_terminal() && obligation.attempts < MAX_DELIVERY_ATTEMPTS {
                    receipt = target.deliver(&request);
                    obligation.apply_receipt(&receipt);
                }
                let mut halt_after_observation = false;
                match receipt {
                    DeliveryReceipt::DurableCommitted { duplicate, .. } => {
                        let intent = CommitIntent::delivered(
                            target.target_id(),
                            &key,
                            observation.sequence,
                            source.next_offset(),
                            None,
                        );
                        if CommitSink::commit(store, &intent).is_ok() {
                            if duplicate {
                                report.duplicates += 1;
                            } else {
                                report.delivered += 1;
                                report.durable_committed += 1;
                            }
                            frontier =
                                frontier.advanced(observation.sequence, source.next_offset());
                        } else {
                            obligation.state = DeliveryState::Uncertain;
                            obligation.last_error = Some(
                                "local commit-store append failed after target commit".to_owned(),
                            );
                            report.uncertain += 1;
                            halt_after_observation = true;
                        }
                    }
                    DeliveryReceipt::Rejected { reason, .. } => {
                        let quarantine = QuarantineRecord {
                            sequence: observation.sequence,
                            line_number: observation.line_number,
                            byte_offset: source.last_offset(),
                            record_id: observation.record_id.clone(),
                            target_id: target.target_id().to_owned(),
                            raw_bytes_hex: crate::application::hex_bytes(&observation.raw_bytes),
                            reason,
                        };
                        if CommitSink::commit(
                            store,
                            &CommitIntent::quarantined(target.target_id(), &quarantine),
                        )
                        .is_ok()
                        {
                            report.rejected += 1;
                            report.quarantined += 1;
                            frontier =
                                frontier.advanced(observation.sequence, source.next_offset());
                        } else {
                            obligation.state = DeliveryState::Uncertain;
                            obligation.last_error = Some("durable quarantine failed".to_owned());
                            report.uncertain += 1;
                            halt_after_observation = true;
                        }
                    }
                    DeliveryReceipt::Outstanding { .. } => {
                        report.outstanding += 1;
                        halt_after_observation = true;
                    }
                    DeliveryReceipt::Uncertain { .. } => {
                        report.uncertain += 1;
                        halt_after_observation = true;
                    }
                    DeliveryReceipt::Accepted { .. } => {
                        report.accepted += 1;
                        halt_after_observation = true;
                    }
                    DeliveryReceipt::Delivered { .. } => {
                        report.target_delivered += 1;
                        halt_after_observation = true;
                    }
                    DeliveryReceipt::Simulated { .. } => {
                        report.simulated += 1;
                        halt_after_observation = true;
                    }
                }
                report.obligations.push(obligation);
                if halt_after_observation {
                    break;
                }
            }
            Err(error) => {
                report.total_read += 1;
                let sequence = error.sequence();
                if sequence <= frontier.last_committed_sequence {
                    report.skipped += 1;
                    continue;
                }
                if matches!(error, crate::application::SourceError::Io { .. }) {
                    report.uncertain += 1;
                    break;
                }
                let quarantine = QuarantineRecord {
                    sequence,
                    line_number: error.line_number(),
                    byte_offset: source.last_offset(),
                    record_id: format!("jsonl-source-rejected:{sequence}"),
                    target_id: target.target_id().to_owned(),
                    raw_bytes_hex: crate::application::hex_bytes(error.raw_bytes()),
                    reason: error.to_string(),
                };
                if CommitSink::commit(
                    store,
                    &CommitIntent::quarantined(target.target_id(), &quarantine),
                )
                .is_ok()
                {
                    report.rejected += 1;
                    report.quarantined += 1;
                    frontier = frontier.advanced(sequence, source.next_offset());
                } else {
                    report.uncertain += 1;
                    break;
                }
            }
        }
    }

    StreamingResult {
        report,
        checkpoint: frontier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{CommitIntent, DeliveryRequest, DispositionRecord, QuarantineRecord};

    fn binding() -> CheckpointBinding {
        CheckpointBinding {
            source_id: "source:test".to_owned(),
            source_fingerprint: "sha256:abc".to_owned(),
            target_id: "pg-local".to_owned(),
        }
    }

    fn intent(sequence: u64) -> CommitIntent {
        let mut intent =
            CommitIntent::delivered("pg-local", "evt-1", sequence, sequence * 10, None);
        intent.dispositions.push(DispositionRecord {
            sequence,
            event_id: "evt-1".to_owned(),
            target_id: "pg-local".to_owned(),
            state: crate::application::DeliveryState::DurableCommitted,
            reason: None,
        });
        intent
    }

    fn quarantine(sequence: u64) -> QuarantineRecord {
        QuarantineRecord {
            sequence,
            line_number: sequence as usize,
            byte_offset: sequence * 10,
            record_id: format!("bad-{sequence}"),
            target_id: "pg-local".to_owned(),
            raw_bytes_hex: "7b6e6f74206a736f6e7d".to_owned(),
            reason: "malformed".to_owned(),
        }
    }

    #[test]
    fn commit_and_reopen_preserve_ids_dispositions_and_checkpoint() {
        let directory = tempfile::tempdir().expect("tempdir");
        let bind = binding();
        {
            let mut store = CommitStore::open(directory.path(), bind.clone()).expect("open");
            let mut first = intent(1);
            first.quarantines.push(quarantine(1));
            CommitSink::commit(&mut store, &first).expect("first commit");
            CommitSink::commit(&mut store, &intent(2)).expect("second commit");
            assert_eq!(store.next_sequence(), 3);
            assert_eq!(store.verify().expect("verify").records.len(), 2);
        }
        let reopened = CommitStore::open(directory.path(), bind).expect("reopen");
        let state = reopened.state();
        assert_eq!(state.records.len(), 2);
        assert_eq!(state.committed_event_ids(), vec!["evt-1"]);
        assert_eq!(state.quarantine_count(), 1);
        let checkpoint = state.checkpoint().expect("checkpoint");
        assert_eq!(checkpoint.last_committed_sequence, 2);
        assert_eq!(checkpoint.last_committed_offset, 20);
        assert_eq!(checkpoint.source_fingerprint, "sha256:abc");
    }

    #[test]
    fn partial_or_corrupt_commit_record_fails_closed() {
        let directory = tempfile::tempdir().expect("tempdir");
        {
            let mut store = CommitStore::open(directory.path(), binding()).expect("open");
            store.set_fault(StoreFault::TruncatedAppend);
            CommitSink::commit(&mut store, &intent(1)).expect("simulated crash");
        }
        assert!(matches!(
            load_state(directory.path(), Some(&binding())),
            Err(StoreError::Corrupt { .. })
        ));
        assert!(CommitStore::open(directory.path(), binding()).is_err());

        let other = tempfile::tempdir().expect("tempdir");
        {
            let mut store = CommitStore::open(other.path(), binding()).expect("open");
            store.set_fault(StoreFault::CorruptChecksum);
            CommitSink::commit(&mut store, &intent(1)).expect("corrupt write");
        }
        assert!(matches!(
            load_state(other.path(), Some(&binding())),
            Err(StoreError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn fail_before_write_leaves_the_store_unchanged() {
        let directory = tempfile::tempdir().expect("tempdir");
        let mut store = CommitStore::open(directory.path(), binding()).expect("open");
        store.set_fault(StoreFault::FailBeforeWrite);
        assert!(CommitSink::commit(&mut store, &intent(1)).is_err());
        store.set_fault(StoreFault::None);
        assert_eq!(store.next_sequence(), 1);
        assert!(store.state().records.is_empty());
    }

    #[test]
    fn checkpoint_binding_mismatch_fails_closed() {
        let directory = tempfile::tempdir().expect("tempdir");
        let mut store = CommitStore::open(directory.path(), binding()).expect("open");
        CommitSink::commit(&mut store, &intent(1)).expect("commit");
        let mut different = binding();
        different.source_fingerprint = "sha256:changed".to_owned();
        assert!(matches!(
            CommitStore::open(directory.path(), different),
            Err(StoreError::BindingMismatch { .. })
        ));
    }

    #[test]
    fn sequence_gap_is_rejected_on_reopen() {
        let directory = tempfile::tempdir().expect("tempdir");
        {
            let mut store = CommitStore::open(directory.path(), binding()).expect("open");
            CommitSink::commit(&mut store, &intent(1)).expect("first");
            CommitSink::commit(&mut store, &intent(2)).expect("second");
        }
        let missing = directory.path().join(record_name(1));
        std::fs::remove_file(&missing).expect("remove first record");
        assert!(matches!(
            load_state(directory.path(), Some(&binding())),
            Err(StoreError::SequenceGap { .. })
        ));
    }

    #[test]
    fn stubbed_request_helper_keeps_transcript_shape_honest() {
        let request = DeliveryRequest {
            idempotency_key: "key".to_owned(),
            events: Vec::new(),
            source_position: None,
        };
        assert_eq!(request.idempotency_key, "key");
    }
}
