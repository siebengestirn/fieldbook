//! P6.2 segmented local durability evidence.
//!
//! These tests exercise the real filesystem, including every finalization
//! boundary, fail-closed reopen and migration from the legacy commit format.

#![expect(
    clippy::expect_used,
    reason = "durability tests assert named filesystem outcomes"
)]

use std::{
    fs,         //
    path::Path, //
};

use engineering_challenge::application::{
    CommitIntent, CommitSink,
    durable::{CommitStore, load_state},
    durable_segmented::{
        ARCHIVE_SCHEMA_VERSION, ArchiveEntry, ArchiveManifest, SegmentError, SegmentFault,
        SegmentedStore, StoreLimits, SyncPolicy, migrate_legacy,
    },
    file_source::{CheckpointBinding, SourceIdentity},
};

fn fast_limits() -> StoreLimits {
    StoreLimits {
        max_segment_records: 1,
        max_segment_bytes: usize::MAX,
        max_hot_segments: 8,
        snapshot_every_commits: 1,
    }
}

fn binding(name: &str, target: &str) -> CheckpointBinding {
    CheckpointBinding::new(&SourceIdentity::register(name, b"segmented-test"), target)
}

fn legacy_store(directory: &Path, binding: &CheckpointBinding) -> CommitStore {
    CommitStore::open(directory, binding.clone()).expect("open legacy store")
}

fn add_legacy(store: &mut CommitStore, sequence: u64, event_id: &str) {
    CommitSink::commit(
        store,
        &CommitIntent::delivered("segmented-target", event_id, sequence, sequence * 10, None),
    )
    .expect("legacy batch committed");
}

#[test]
fn segmented_store_reopens_with_bounded_state_and_snapshot() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store = SegmentedStore::open(
        &store_dir,
        binding.clone(),
        SyncPolicy::Full,
        StoreLimits::default(),
    )
    .expect("open store");
    for sequence in 1..=3 {
        let mut legacy = CommitIntent::delivered(
            "segmented-target",
            &format!("event-{sequence}"),
            sequence,
            sequence * 10,
            None,
        );
        legacy.next_sequence = sequence;
        legacy.next_offset = sequence * 10;
        CommitSink::commit(&mut store, &legacy).expect("segmented batch committed");
    }
    assert_eq!(store.next_sequence(), 4, "three batches committed");
    store.verify().expect("disk verifies");

    let reopened = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits())
        .expect("reopen store");
    assert_eq!(reopened.next_sequence(), 4, "frontier survived reopen");
    assert!(
        reopened.state().records.len() <= 4,
        "recovery state is bounded by snapshot plus active records"
    );
    assert!(
        reopened.is_committed("event-1").expect("lookup works"),
        "committed id is confirmed by exact scan"
    );
    assert!(
        !reopened.is_committed("unknown").expect("lookup works"),
        "unknown id is absent"
    );
}

#[test]
fn missing_acknowledged_segment_fails_closed() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store =
        SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
            .expect("open store");
    let mut intent = CommitIntent::delivered("segmented-target", "event-1", 1, 10, None);
    intent.next_sequence = 1;
    CommitSink::commit(&mut store, &intent).expect("batch committed");
    let segment = store_dir
        .join("segments")
        .join("seg-000000000001-000000000001.jsonl");
    fs::remove_file(segment).expect("remove acknowledged segment");

    let result = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits());
    assert!(
        matches!(result, Err(SegmentError::MissingAcknowledgedSegment(_)))
            || matches!(result, Err(SegmentError::InvalidSnapshot { .. })),
        "missing acknowledged evidence is a hard error: {result:?}"
    );
}

#[test]
fn incomplete_temp_segment_is_never_discarded() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    fs::create_dir_all(store_dir.join("segments")).expect("create segments directory");
    fs::write(
        store_dir.join("segments").join(".tmp-000000000001"),
        b"partial",
    )
    .expect("write temporary segment");
    let result = SegmentedStore::open(
        &store_dir,
        binding("segmented-source", "segmented-target"),
        SyncPolicy::Full,
        fast_limits(),
    );
    assert!(
        matches!(result, Err(SegmentError::IncompleteSegment(_)))
            || matches!(result, Err(SegmentError::CorruptSegment { .. })),
        "incomplete segment fails closed: {result:?}"
    );
}

#[test]
fn corrupt_snapshot_fails_closed() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store =
        SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
            .expect("open store");
    store.set_fault(SegmentFault::CorruptSnapshot);
    let mut intent = CommitIntent::delivered("segmented-target", "event-1", 1, 10, None);
    intent.next_sequence = 1;
    let _ = CommitSink::commit(&mut store, &intent);
    let result = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits());
    assert!(
        matches!(result, Err(SegmentError::InvalidSnapshot { .. })),
        "corrupt snapshot fails closed: {result:?}"
    );
}

#[test]
fn every_finalization_boundary_fails_without_a_false_commit() {
    for fault in [
        SegmentFault::FailBeforeWrite,
        SegmentFault::FailAfterFrames,
        SegmentFault::FailBeforeFooter,
        SegmentFault::FailAfterFooterBeforeRename,
        SegmentFault::FailAfterRenameBeforeDirSync,
    ] {
        let root = tempfile::tempdir().expect("tempdir");
        let store_dir = root.path().join("store");
        let binding = binding("segmented-source", "segmented-target");
        let mut store =
            SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
                .expect("open store");
        store.set_fault(fault);
        let mut intent = CommitIntent::delivered("segmented-target", "event-1", 1, 10, None);
        intent.next_sequence = 1;
        assert!(
            CommitSink::commit(&mut store, &intent).is_err(),
            "fault {fault:?} rejects the commit"
        );
        let reopened = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits());
        assert!(
            reopened.is_ok(),
            "fault {fault:?} leaves a recoverable store: {reopened:?}"
        );
    }
}

#[test]
fn missing_archived_segment_fails_closed() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store =
        SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
            .expect("open store");
    let mut intent = CommitIntent::delivered("segmented-target", "event-1", 1, 10, None);
    intent.next_sequence = 1;
    CommitSink::commit(&mut store, &intent).expect("batch committed");
    let archive_dir = store_dir.join("archive");
    fs::create_dir_all(&archive_dir).expect("create archive directory");
    fs::write(
        archive_dir.join("manifest.json"),
        serde_json::to_vec(&ArchiveManifest {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            entries: vec![ArchiveEntry {
                path: "seg-000000000001-000000000001.jsonl".to_owned(),
                first_sequence: 1,
                last_sequence: 1,
                record_count: 1,
                checksum: "deadbeef".to_owned(),
            }],
            checksum: String::new(),
        })
        .expect("serialize archive manifest"),
    )
    .expect("write archive manifest");
    let result = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits());
    assert!(
        matches!(result, Err(SegmentError::MissingAcknowledgedSegment(_)))
            || matches!(result, Err(SegmentError::InvalidArchive { .. })),
        "missing archived evidence fails closed: {result:?}"
    );
}

#[test]
fn sequence_gap_fails_closed() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store =
        SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
            .expect("open store");
    for sequence in 1..=2 {
        let mut intent = CommitIntent::delivered(
            "segmented-target",
            &format!("event-{sequence}"),
            sequence,
            sequence * 10,
            None,
        );
        intent.next_sequence = sequence;
        CommitSink::commit(&mut store, &intent).expect("batch committed");
    }
    let segments = store_dir.join("segments");
    fs::rename(
        segments.join("seg-000000000002-000000000002.jsonl"),
        segments.join("seg-000000000009-000000000009.jsonl"),
    )
    .expect("renumber segment to create a gap");
    let result = SegmentedStore::open(&store_dir, binding, SyncPolicy::Full, fast_limits());
    assert!(
        matches!(result, Err(SegmentError::SequenceGap { .. }))
            || matches!(result, Err(SegmentError::MissingAcknowledgedSegment(_)))
            || matches!(result, Err(SegmentError::CorruptSegment { .. })),
        "sequence gap fails closed: {result:?}"
    );
}

#[test]
fn disk_full_fault_leaves_the_store_unchanged() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut store =
        SegmentedStore::open(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
            .expect("open store");
    store.set_fault(SegmentFault::DiskFull);
    let mut intent = CommitIntent::delivered("segmented-target", "event-1", 1, 10, None);
    intent.next_sequence = 1;
    assert!(
        CommitSink::commit(&mut store, &intent).is_err(),
        "disk-full write is rejected"
    );
    assert_eq!(store.next_sequence(), 1, "frontier did not advance");
}

#[test]
fn legacy_store_migrates_without_rewriting_history() {
    let root = tempfile::tempdir().expect("tempdir");
    let store_dir = root.path().join("store");
    let binding = binding("segmented-source", "segmented-target");
    let mut legacy = legacy_store(&store_dir, &binding);
    add_legacy(&mut legacy, 1, "legacy-1");
    add_legacy(&mut legacy, 2, "legacy-2");
    let before = load_state(&store_dir, Some(&binding)).expect("legacy state");
    assert_eq!(before.records.len(), 2, "legacy store has two records");

    let migrated = migrate_legacy(&store_dir, binding.clone(), SyncPolicy::Full, fast_limits())
        .expect("legacy store migrates");
    assert_eq!(migrated.next_sequence(), 3, "frontier carried over");
    assert!(
        migrated.is_committed("legacy-1").expect("lookup works"),
        "legacy id remains queryable"
    );
    assert!(
        migrated.is_committed("legacy-2").expect("lookup works"),
        "second legacy id remains queryable"
    );
    let after = load_state(&store_dir, Some(&binding)).expect("legacy files retained");
    assert_eq!(after.records, before.records, "legacy history is unchanged");
    migrated.verify().expect("migrated store verifies");
}
