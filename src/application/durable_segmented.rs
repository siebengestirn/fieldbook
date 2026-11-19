//! Segmented, checkpointed local durability (Platform P6.2).
//!
//! This module adds a versioned segment format beside the original
//! append-only `commit-N.json` store. It does not rewrite or delete legacy
//! files. See `docs/p6-2-design.md` for the settled design and sync boundary.
//!
//! The three durability histories stay separate: source progress, target
//! commit and local recovery evidence are never collapsed into one flag.

use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{
    Deserialize, //
    Serialize,   //
};
use sha2::{
    Digest, //
    Sha256, //
};
use thiserror::Error;

use crate::{
    application::{
        CommitIntent, CommitSink,
        durable::{COMMIT_SCHEMA_VERSION, CommitRecord, DurableState, checksum_of, load_state},
        file_source::{CheckpointBinding, DurableCheckpoint},
    },
    report::write_json_atomic,
};

pub const SEGMENT_SCHEMA_VERSION: u32 = 1;
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const ARCHIVE_SCHEMA_VERSION: u32 = 1;
const BLOOM_BITS: usize = 4096;

/// Explicit sync policy recorded in every segment header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPolicy {
    /// Sync the file, the footer and the containing directory.
    Full,
    /// Rename atomically without explicit sync calls.
    None,
}

/// Deterministic fault injection for each finalization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SegmentFault {
    #[default]
    None,
    FailBeforeWrite,
    /// Simulates an ENOSPC rejection at the first write.
    DiskFull,
    FailAfterFrames,
    FailBeforeFooter,
    FailAfterFooterBeforeRename,
    FailAfterRenameBeforeDirSync,
    CorruptFrame,
    CorruptSnapshot,
}

/// Fail-closed segmented store errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SegmentError {
    #[error("segmented store I/O failure: {0}")]
    Io(String),
    #[error("segmented store serialization failure: {0}")]
    Serialize(String),
    #[error("incomplete segment `{0}` may contain acknowledged data")]
    IncompleteSegment(String),
    #[error("segment `{path}` is corrupt: {reason}")]
    CorruptSegment { path: String, reason: String },
    #[error("segment `{path}` version {found} is unsupported (expected {expected})")]
    UnsupportedVersion {
        path: String,
        found: u32,
        expected: u32,
    },
    #[error("acknowledged segment `{0}` is missing or unverifiable")]
    MissingAcknowledgedSegment(String),
    #[error("segment sequence gap: expected `{expected}`, found `{found}`")]
    SequenceGap { expected: u64, found: u64 },
    #[error("snapshot `{path}` is invalid: {reason}")]
    InvalidSnapshot { path: String, reason: String },
    #[error("archive entry `{path}` is invalid: {reason}")]
    InvalidArchive { path: String, reason: String },
    #[error("store mixes legacy commit files and segments without a migration marker")]
    MixedFormat,
    #[error("store binding does not match its segment or snapshot")]
    BindingMismatch,
    #[error("fault injected: {0}")]
    FaultInjected(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentHeader {
    pub schema_version: u32,
    pub source_id: String,
    pub source_fingerprint: String,
    pub target_id: String,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub expected_records: u64,
    pub sync_policy: SyncPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentFrame {
    pub sequence: u64,
    pub checksum: String,
    pub record: CommitRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentFooter {
    pub record_count: u64,
    pub frames_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentFrontier {
    pub segment: String,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub record_count: u64,
    pub segment_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub schema_version: u32,
    pub source_id: String,
    pub source_fingerprint: String,
    pub target_id: String,
    pub frontier: Option<SegmentFrontier>,
    pub materialized: CommitRecord,
    pub bloom_bits: Vec<u8>,
    pub commit_count: u64,
    pub event_count: u64,
    pub quarantine_count: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub record_count: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub schema_version: u32,
    pub entries: Vec<ArchiveEntry>,
    pub checksum: String,
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = std::fmt::Write::write_fmt(&mut text, format_args!("{byte:02x}"));
    }
    text
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, SegmentError> {
    serde_json::to_vec(value).map_err(|error| SegmentError::Serialize(error.to_string()))
}

fn sync_file(file: &File) -> Result<(), SegmentError> {
    file.sync_all()
        .map_err(|error| SegmentError::Io(error.to_string()))
}

fn sync_dir(path: &Path) -> Result<(), SegmentError> {
    let handle = File::open(path).map_err(|error| SegmentError::Io(error.to_string()))?;
    sync_file(&handle)
}

fn bloom_positions(value: &str) -> [usize; 3] {
    let mut positions = [0usize; 3];
    for (index, seed) in [
        0xcbf29ce484222325u64,
        0x9e3779b97f4a7c15,
        0x5851f42d4c957f2d,
    ]
    .into_iter()
    .enumerate()
    {
        let mut hash = seed;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        positions[index] = (hash % BLOOM_BITS as u64) as usize;
    }
    positions
}

fn bloom_insert(bits: &mut [u8], value: &str) {
    for position in bloom_positions(value) {
        bits[position / 8] |= 1 << (position % 8);
    }
}

fn bloom_maybe_contains(bits: &[u8], value: &str) -> bool {
    bloom_positions(value)
        .iter()
        .all(|position| bits[position / 8] & (1 << (position % 8)) != 0)
}

fn segment_name(first: u64, last: u64) -> String {
    format!("seg-{first:012}-{last:012}.jsonl")
}

/// Bounded recovery state: the snapshot's materialized record plus records
/// finalized after the snapshot frontier.
#[derive(Debug)]
pub struct SegmentedStore {
    directory: PathBuf,
    binding: CheckpointBinding,
    state: DurableState,
    next_sequence: u64,
    fault: SegmentFault,
    sync_policy: SyncPolicy,
    snapshot: RecoverySnapshot,
    bloom: Vec<u8>,
    hot_segments: Vec<SegmentFrontier>,
    archives: Vec<ArchiveEntry>,
}

impl SegmentedStore {
    /// Open a segmented store, verifying snapshot, archive and segment chain.
    pub fn open(
        directory: &Path,
        binding: CheckpointBinding,
        sync_policy: SyncPolicy,
    ) -> Result<Self, SegmentError> {
        let segments_dir = directory.join("segments");
        let snapshots_dir = directory.join("snapshots");
        let archive_dir = directory.join("archive");
        if let Ok(entries) = std::fs::read_dir(&segments_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(".tmp-") {
                    return Err(SegmentError::IncompleteSegment(name));
                }
            }
        }
        let has_segments = std::fs::read_dir(&segments_dir)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        let has_legacy = std::fs::read_dir(directory)
            .map(|entries| {
                entries
                    .flatten()
                    .any(|entry| entry.file_name().to_string_lossy().starts_with("commit-"))
            })
            .unwrap_or(false);
        let migrated = directory.join("legacy-migrated.json").exists();
        if has_legacy && has_segments && !migrated {
            return Err(SegmentError::MixedFormat);
        }

        let archives = load_archive(&archive_dir)?;
        let mut snapshot = load_snapshot(&snapshots_dir, &binding)?;
        let frames = if has_segments {
            load_segments(&segments_dir, &binding)?
        } else {
            Vec::new()
        };
        let mut expected = 1u64;
        for (frontier, _) in &frames {
            if frontier.last_sequence < frontier.first_sequence {
                return Err(SegmentError::CorruptSegment {
                    path: frontier.segment.clone(),
                    reason: "last sequence precedes first sequence".to_owned(),
                });
            }
            if frontier.first_sequence != expected {
                return Err(SegmentError::SequenceGap {
                    expected,
                    found: frontier.first_sequence,
                });
            }
            expected = frontier.last_sequence + 1;
        }

        let snapshot_frontier = snapshot.as_ref().and_then(|snap| snap.frontier.as_ref());
        if let Some(claimed) = snapshot_frontier {
            let found = frames
                .iter()
                .find(|(frontier, _)| frontier.segment == claimed.segment);
            match found {
                None => {
                    return Err(SegmentError::MissingAcknowledgedSegment(
                        claimed.segment.clone(),
                    ));
                }
                Some((frontier, _)) if frontier.segment_checksum != claimed.segment_checksum => {
                    return Err(SegmentError::MissingAcknowledgedSegment(
                        claimed.segment.clone(),
                    ));
                }
                Some(_) => {}
            }
        } else if has_segments {
            return Err(SegmentError::InvalidSnapshot {
                path: snapshots_dir.display().to_string(),
                reason: "segments exist without a verified snapshot frontier".to_owned(),
            });
        }

        let covered = snapshot_frontier.map_or(0, |frontier| frontier.last_sequence);
        let frontier_last = frames
            .last()
            .map_or(covered, |(frontier, _)| frontier.last_sequence);
        let next_sequence = frontier_last + 1;
        let mut records = Vec::new();
        if let Some(ref snap) = snapshot {
            records.push(snap.materialized.clone());
        }
        let mut hot = Vec::new();
        for (frontier, segment_records) in frames {
            if frontier.first_sequence > covered {
                records.extend(segment_records);
            }
            hot.push(frontier);
        }

        let bloom = snapshot
            .as_ref()
            .map(|snap| snap.bloom_bits.clone())
            .unwrap_or_else(|| vec![0u8; BLOOM_BITS / 8]);
        let materialized = snapshot
            .as_ref()
            .map(|snap| snap.materialized.clone())
            .unwrap_or_else(|| CommitRecord {
                target_id: binding.target_id.clone(),
                committed_event_ids: Vec::new(),
                dispositions: Vec::new(),
                quarantines: Vec::new(),
                checkpoint: DurableCheckpoint::new(&binding),
            });
        if snapshot.is_none() {
            snapshot = Some(empty_snapshot(&binding, materialized, bloom.clone()));
        }
        let Some(snapshot) = snapshot else {
            return Err(SegmentError::InvalidSnapshot {
                path: snapshots_dir.display().to_string(),
                reason: "snapshot was not initialised".to_owned(),
            });
        };
        Ok(Self {
            directory: directory.to_path_buf(),
            binding,
            state: DurableState { records },
            next_sequence,
            fault: SegmentFault::None,
            sync_policy,
            snapshot,
            bloom,
            hot_segments: hot,
            archives,
        })
    }

    #[must_use]
    pub const fn state(&self) -> &DurableState {
        &self.state
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn set_fault(&mut self, fault: SegmentFault) {
        self.fault = fault;
    }

    #[must_use]
    pub fn archive_entries(&self) -> &[ArchiveEntry] {
        &self.archives
    }

    /// Verify the archive manifest and copy every segment to `destination`.
    pub fn restore_archive(&self, destination: &Path) -> Result<Vec<String>, SegmentError> {
        std::fs::create_dir_all(destination).map_err(|e| SegmentError::Io(e.to_string()))?;
        let mut restored = Vec::new();
        for entry in &self.archives {
            let source = self.directory.join("archive").join(&entry.path);
            let bytes = std::fs::read(&source).map_err(|e| SegmentError::Io(e.to_string()))?;
            if hex_digest(&bytes) != entry.checksum {
                return Err(SegmentError::MissingAcknowledgedSegment(entry.path.clone()));
            }
            let target = destination.join(&entry.path);
            std::fs::write(&target, &bytes).map_err(|e| SegmentError::Io(e.to_string()))?;
            restored.push(entry.path.clone());
        }
        Ok(restored)
    }

    /// Verify that a committed id is absent, or confirm it by exact scan.
    /// The Bloom filter never decides that an id is committed.
    pub fn is_committed(&self, event_id: &str) -> bool {
        if !bloom_maybe_contains(&self.bloom, event_id) {
            return false;
        }
        self.state
            .records
            .iter()
            .any(|record| record.committed_event_ids.iter().any(|id| id == event_id))
            || self.scan_segments_for(event_id)
    }

    fn scan_segments_for(&self, event_id: &str) -> bool {
        for frontier in self.hot_segments.iter().rev() {
            let path = self.directory.join("segments").join(&frontier.segment);
            if let Ok(bytes) = std::fs::read(&path)
                && String::from_utf8_lossy(&bytes)
                    .lines()
                    .filter_map(|line| serde_json::from_str::<SegmentFrame>(line).ok())
                    .any(|frame| {
                        frame
                            .record
                            .committed_event_ids
                            .iter()
                            .any(|id| id == event_id)
                    })
            {
                return true;
            }
        }
        false
    }

    /// Commit one batch as a new finalized segment and refresh the snapshot.
    pub fn commit(&mut self, record: CommitRecord) -> Result<(), SegmentError> {
        if self.binding.target_id != record.target_id
            || self.binding.source_id != record.checkpoint.source_id
            || self.binding.source_fingerprint != record.checkpoint.source_fingerprint
        {
            return Err(SegmentError::BindingMismatch);
        }
        match self.fault {
            SegmentFault::FailBeforeWrite => {
                return Err(SegmentError::FaultInjected(
                    "failed before write".to_owned(),
                ));
            }
            SegmentFault::DiskFull => {
                return Err(SegmentError::Io("no space left on device".to_owned()));
            }
            SegmentFault::CorruptSnapshot => {
                let snapshots_dir = self.directory.join("snapshots");
                std::fs::create_dir_all(&snapshots_dir)
                    .map_err(|e| SegmentError::Io(e.to_string()))?;
                let path = snapshots_dir.join("current.json");
                std::fs::write(&path, b"{\"corrupt\":true}")
                    .map_err(|e| SegmentError::Io(e.to_string()))?;
                return Err(SegmentError::FaultInjected("corrupt snapshot".to_owned()));
            }
            _ => {}
        }

        let first = self.next_sequence;
        let last = first;
        let header = SegmentHeader {
            schema_version: SEGMENT_SCHEMA_VERSION,
            source_id: self.binding.source_id.clone(),
            source_fingerprint: self.binding.source_fingerprint.clone(),
            target_id: self.binding.target_id.clone(),
            first_sequence: first,
            last_sequence: last,
            expected_records: 1,
            sync_policy: self.sync_policy,
        };
        let frame = SegmentFrame {
            sequence: first,
            checksum: checksum_of(&record)
                .map_err(|error| SegmentError::Serialize(error.to_string()))?,
            record: record.clone(),
        };
        let frames_checksum = hex_digest(&canonical_bytes(&frame)?);
        let footer = SegmentFooter {
            record_count: 1,
            frames_checksum,
        };
        let mut text = String::new();
        text.push_str(
            &serde_json::to_string(&header).map_err(|e| SegmentError::Serialize(e.to_string()))?,
        );
        text.push('\n');
        text.push_str(
            &serde_json::to_string(&frame).map_err(|e| SegmentError::Serialize(e.to_string()))?,
        );
        text.push('\n');
        if self.fault == SegmentFault::CorruptFrame {
            text.push_str("{\"sequence\":1,\"checksum\":\"bad\",\"record\":{}}\n");
        }
        if self.fault == SegmentFault::FailAfterFrames {
            return Err(SegmentError::FaultInjected(
                "failed after frames".to_owned(),
            ));
        }
        text.push_str(
            &serde_json::to_string(&footer).map_err(|e| SegmentError::Serialize(e.to_string()))?,
        );
        text.push('\n');
        if self.fault == SegmentFault::FailBeforeFooter {
            return Err(SegmentError::FaultInjected(
                "failed before footer".to_owned(),
            ));
        }

        let segments_dir = self.directory.join("segments");
        std::fs::create_dir_all(&segments_dir).map_err(|e| SegmentError::Io(e.to_string()))?;
        let temp = segments_dir.join(format!(".tmp-{first:012}"));
        let mut handle = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| SegmentError::Io(e.to_string()))?;
        handle
            .write_all(text.as_bytes())
            .map_err(|e| SegmentError::Io(e.to_string()))?;
        if self.sync_policy == SyncPolicy::Full {
            sync_file(&handle)?;
        }
        if self.fault == SegmentFault::FailAfterFooterBeforeRename {
            return Err(SegmentError::FaultInjected(
                "failed before rename".to_owned(),
            ));
        }
        let final_path = segments_dir.join(segment_name(first, last));
        std::fs::rename(&temp, &final_path).map_err(|e| SegmentError::Io(e.to_string()))?;
        if self.fault == SegmentFault::FailAfterRenameBeforeDirSync {
            return Err(SegmentError::FaultInjected(
                "failed before dir sync".to_owned(),
            ));
        }
        if self.sync_policy == SyncPolicy::Full {
            sync_dir(&segments_dir)?;
        }

        for id in &record.committed_event_ids {
            bloom_insert(&mut self.bloom, id);
        }
        let frontier = SegmentFrontier {
            segment: segment_name(first, last),
            first_sequence: first,
            last_sequence: last,
            record_count: 1,
            segment_checksum: hex_digest(text.as_bytes()),
        };
        self.hot_segments.push(frontier.clone());
        self.state.records.push(record.clone());
        self.next_sequence += 1;
        let mut snapshot = self.snapshot.clone();
        snapshot.frontier = Some(frontier);
        snapshot.materialized = record;
        snapshot.bloom_bits = self.bloom.clone();
        snapshot.commit_count += 1;
        snapshot.event_count += record_event_count(&snapshot.materialized) as u64;
        let snapshots_dir = self.directory.join("snapshots");
        std::fs::create_dir_all(&snapshots_dir).map_err(|e| SegmentError::Io(e.to_string()))?;
        write_snapshot(&snapshots_dir, &snapshot)?;
        self.snapshot = snapshot;
        Ok(())
    }

    /// Re-read and verify everything on disk.
    pub fn verify(&self) -> Result<(), SegmentError> {
        let _ = Self::open(&self.directory, self.binding.clone(), self.sync_policy)?;
        Ok(())
    }

    /// Verify the current store using the supplied binding, for migration.
    pub fn verify_binding(&self, binding: &CheckpointBinding) -> Result<(), SegmentError> {
        let _ = Self::open(&self.directory, binding.clone(), self.sync_policy)?;
        Ok(())
    }
}

fn record_event_count(record: &CommitRecord) -> usize {
    record.committed_event_ids.len()
}

impl CommitSink for SegmentedStore {
    fn commit(&mut self, intent: &CommitIntent) -> Result<(), String> {
        let checkpoint = DurableCheckpoint {
            schema_version: COMMIT_SCHEMA_VERSION,
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
        SegmentedStore::commit(self, record).map_err(|error| error.to_string())
    }
}

fn empty_snapshot(
    binding: &CheckpointBinding,
    materialized: CommitRecord,
    bloom: Vec<u8>,
) -> RecoverySnapshot {
    let mut snapshot = RecoverySnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        source_id: binding.source_id.clone(),
        source_fingerprint: binding.source_fingerprint.clone(),
        target_id: binding.target_id.clone(),
        frontier: None,
        materialized,
        bloom_bits: bloom,
        commit_count: 0,
        event_count: 0,
        quarantine_count: 0,
        checksum: String::new(),
    };
    snapshot.checksum = snapshot_checksum(&snapshot);
    snapshot
}

fn snapshot_checksum(snapshot: &RecoverySnapshot) -> String {
    let mut copy = snapshot.clone();
    copy.checksum.clear();
    hex_digest(&canonical_bytes(&copy).unwrap_or_default())
}

fn write_snapshot(directory: &Path, snapshot: &RecoverySnapshot) -> Result<(), SegmentError> {
    let mut ready = snapshot.clone();
    ready.checksum = snapshot_checksum(snapshot);
    let path = directory.join("current.json");
    write_json_atomic(&path, &ready).map_err(|error| SegmentError::Io(error.to_string()))
}

fn load_snapshot(
    directory: &Path,
    binding: &CheckpointBinding,
) -> Result<Option<RecoverySnapshot>, SegmentError> {
    let path = directory.join("current.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|error| SegmentError::Io(error.to_string()))?;
    let snapshot: RecoverySnapshot =
        serde_json::from_slice(&bytes).map_err(|error| SegmentError::InvalidSnapshot {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(SegmentError::InvalidSnapshot {
            path: path.display().to_string(),
            reason: format!("unsupported version {}", snapshot.schema_version),
        });
    }
    if snapshot_checksum(&snapshot) != snapshot.checksum {
        return Err(SegmentError::InvalidSnapshot {
            path: path.display().to_string(),
            reason: "checksum mismatch".to_owned(),
        });
    }
    if snapshot.source_id != binding.source_id
        || snapshot.source_fingerprint != binding.source_fingerprint
        || snapshot.target_id != binding.target_id
    {
        return Err(SegmentError::BindingMismatch);
    }
    Ok(Some(snapshot))
}

fn load_segments(
    directory: &Path,
    binding: &CheckpointBinding,
) -> Result<Vec<(SegmentFrontier, Vec<CommitRecord>)>, SegmentError> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|e| SegmentError::Io(e.to_string()))? {
        let name = entry
            .map_err(|e| SegmentError::Io(e.to_string()))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.starts_with("seg-") && name.ends_with(".jsonl") {
            names.push(name);
        }
    }
    names.sort();
    let mut result = Vec::new();
    for name in names {
        let path = directory.join(&name);
        let bytes = std::fs::read(&path).map_err(|e| SegmentError::Io(e.to_string()))?;
        let mut lines = BufReader::new(bytes.as_slice()).lines();
        let header_line = lines
            .next()
            .transpose()
            .map_err(|e| SegmentError::Io(e.to_string()))?
            .ok_or_else(|| SegmentError::CorruptSegment {
                path: name.clone(),
                reason: "missing header".to_owned(),
            })?;
        let header: SegmentHeader =
            serde_json::from_str(&header_line).map_err(|e| SegmentError::CorruptSegment {
                path: name.clone(),
                reason: e.to_string(),
            })?;
        if header.schema_version != SEGMENT_SCHEMA_VERSION {
            return Err(SegmentError::UnsupportedVersion {
                path: name,
                found: header.schema_version,
                expected: SEGMENT_SCHEMA_VERSION,
            });
        }
        if header.source_id != binding.source_id
            || header.source_fingerprint != binding.source_fingerprint
            || header.target_id != binding.target_id
        {
            return Err(SegmentError::BindingMismatch);
        }
        let mut records = Vec::new();
        let mut footer = None;
        for line in lines {
            let line = line.map_err(|e| SegmentError::Io(e.to_string()))?;
            if let Ok(frame) = serde_json::from_str::<SegmentFrame>(&line) {
                let computed = checksum_of(&frame.record)
                    .map_err(|error| SegmentError::Serialize(error.to_string()))?;
                if frame.checksum != computed {
                    return Err(SegmentError::CorruptSegment {
                        path: name.clone(),
                        reason: "frame checksum mismatch".to_owned(),
                    });
                }
                records.push(frame.record);
            } else if let Ok(parsed) = serde_json::from_str::<SegmentFooter>(&line) {
                footer = Some(parsed);
            } else {
                return Err(SegmentError::CorruptSegment {
                    path: name.clone(),
                    reason: "unrecognized frame".to_owned(),
                });
            }
        }
        let footer = footer.ok_or_else(|| SegmentError::CorruptSegment {
            path: name.clone(),
            reason: "missing footer".to_owned(),
        })?;
        if footer.record_count != records.len() as u64 {
            return Err(SegmentError::CorruptSegment {
                path: name.clone(),
                reason: "record count mismatch".to_owned(),
            });
        }
        let frontier = SegmentFrontier {
            segment: name.clone(),
            first_sequence: header.first_sequence,
            last_sequence: header.last_sequence,
            record_count: footer.record_count,
            segment_checksum: hex_digest(&bytes),
        };
        result.push((frontier, records));
    }
    Ok(result)
}

fn load_archive(directory: &Path) -> Result<Vec<ArchiveEntry>, SegmentError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let path = directory.join("manifest.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path).map_err(|e| SegmentError::Io(e.to_string()))?;
    let manifest: ArchiveManifest =
        serde_json::from_slice(&bytes).map_err(|e| SegmentError::InvalidArchive {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    if manifest.schema_version != ARCHIVE_SCHEMA_VERSION {
        return Err(SegmentError::InvalidArchive {
            path: path.display().to_string(),
            reason: "unsupported version".to_owned(),
        });
    }
    for entry in &manifest.entries {
        let segment = directory.join(&entry.path);
        if !segment.exists() {
            return Err(SegmentError::MissingAcknowledgedSegment(entry.path.clone()));
        }
        let bytes = std::fs::read(&segment).map_err(|e| SegmentError::Io(e.to_string()))?;
        if hex_digest(&bytes) != entry.checksum {
            return Err(SegmentError::MissingAcknowledgedSegment(entry.path.clone()));
        }
    }
    Ok(manifest.entries)
}

/// Verify a legacy append-only store and migrate it into the segmented format.
/// Legacy files are left in place; a marker prevents implicit mixed-format use.
pub fn migrate_legacy(
    directory: &Path,
    binding: CheckpointBinding,
    sync_policy: SyncPolicy,
) -> Result<SegmentedStore, SegmentError> {
    let legacy =
        load_state(directory, Some(&binding)).map_err(|error| SegmentError::InvalidSnapshot {
            path: directory.display().to_string(),
            reason: format!("legacy store failed verification: {error}"),
        })?;
    if legacy.records.is_empty() {
        return Err(SegmentError::InvalidSnapshot {
            path: directory.display().to_string(),
            reason: "legacy store has no records".to_owned(),
        });
    }
    let segments_dir = directory.join("segments");
    let snapshots_dir = directory.join("snapshots");
    std::fs::create_dir_all(&segments_dir).map_err(|e| SegmentError::Io(e.to_string()))?;
    std::fs::create_dir_all(&snapshots_dir).map_err(|e| SegmentError::Io(e.to_string()))?;
    let first = 1u64;
    let last = legacy.records.len() as u64;
    let mut text = String::new();
    let header = SegmentHeader {
        schema_version: SEGMENT_SCHEMA_VERSION,
        source_id: binding.source_id.clone(),
        source_fingerprint: binding.source_fingerprint.clone(),
        target_id: binding.target_id.clone(),
        first_sequence: first,
        last_sequence: last,
        expected_records: last,
        sync_policy,
    };
    text.push_str(
        &serde_json::to_string(&header).map_err(|e| SegmentError::Serialize(e.to_string()))?,
    );
    text.push('\n');
    for (index, record) in legacy.records.iter().enumerate() {
        let frame = SegmentFrame {
            sequence: index as u64 + 1,
            checksum: checksum_of(record)
                .map_err(|error| SegmentError::Serialize(error.to_string()))?,
            record: record.clone(),
        };
        text.push_str(
            &serde_json::to_string(&frame).map_err(|e| SegmentError::Serialize(e.to_string()))?,
        );
        text.push('\n');
    }
    let footer = SegmentFooter {
        record_count: last,
        frames_checksum: hex_digest(text.as_bytes()),
    };
    text.push_str(
        &serde_json::to_string(&footer).map_err(|e| SegmentError::Serialize(e.to_string()))?,
    );
    text.push('\n');
    let final_path = segments_dir.join(segment_name(first, last));
    std::fs::write(&final_path, text.as_bytes()).map_err(|e| SegmentError::Io(e.to_string()))?;
    if sync_policy == SyncPolicy::Full {
        sync_file(&File::open(&final_path).map_err(|e| SegmentError::Io(e.to_string()))?)?;
        sync_dir(&segments_dir)?;
    }
    let Some(materialized) = legacy.records.last().cloned() else {
        return Err(SegmentError::InvalidSnapshot {
            path: directory.display().to_string(),
            reason: "legacy store has no records to migrate".to_owned(),
        });
    };
    let mut bloom = vec![0u8; BLOOM_BITS / 8];
    for record in &legacy.records {
        for id in &record.committed_event_ids {
            bloom_insert(&mut bloom, id);
        }
    }
    let mut snapshot = empty_snapshot(&binding, materialized, bloom);
    snapshot.frontier = Some(SegmentFrontier {
        segment: segment_name(first, last),
        first_sequence: first,
        last_sequence: last,
        record_count: last,
        segment_checksum: hex_digest(text.as_bytes()),
    });
    snapshot.commit_count = last;
    snapshot.event_count = legacy.committed_event_ids().len() as u64;
    snapshot.quarantine_count = legacy.quarantine_count() as u64;
    write_snapshot(&snapshots_dir, &snapshot)?;
    std::fs::write(directory.join("legacy-migrated.json"), b"{}\n")
        .map_err(|e| SegmentError::Io(e.to_string()))?;
    SegmentedStore::open(directory, binding, sync_policy)
}
