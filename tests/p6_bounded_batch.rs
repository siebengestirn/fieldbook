//! Bounded, ordered batch delivery evidence for the generic durable runner.
//!
//! These are deterministic simulator tests: they prove transaction shape,
//! ordering, byte/event limits and one local commit record per batch. The
//! real PostgreSQL equivalents live in `tests/postgres_live.rs`.

#![expect(
    clippy::expect_used,
    reason = "integration tests assert deterministic store state and need named failures"
)]

use std::path::Path;

use fieldbook::{
    application::{
        BatchMember, DeliveryBatch, RawObservation, SourcePosition,
        durable::{
            CommitStore, StreamingLimits, StreamingResult, load_state,
            run_streaming_pipeline_bounded,
        },
        file_source::{CheckpointBinding, DurableCheckpoint, FileSource, SourceIdentity},
        postgres::PostgresSimulator,
        transform_observation,
    },
    identity::OpaqueId,
};

fn record(index: usize) -> String {
    format!(
        "{{\"id\":\"batch-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}}}}"
    )
}

fn setup(
    directory: &Path,
    name: &str,
    count: usize,
) -> (
    String,
    CheckpointBinding,
    DurableCheckpoint,
    CommitStore,
    FileSource<std::io::BufReader<std::fs::File>>,
) {
    let bytes = format!(
        "{}\n",
        (0..count).map(record).collect::<Vec<_>>().join("\n")
    );
    let input = directory.join("input.jsonl");
    std::fs::write(&input, &bytes).expect("write input");
    let binding = CheckpointBinding::new(
        &SourceIdentity::register(name, bytes.as_bytes()),
        "pg-batch",
    );
    let anchor = DurableCheckpoint::new(&binding);
    let store = CommitStore::open(&directory.join("store"), binding.clone()).expect("open store");
    let source = FileSource::open(&input, name).expect("open source");
    (bytes, binding, anchor, store, source)
}

fn run(
    source: &mut FileSource<std::io::BufReader<std::fs::File>>,
    anchor: &DurableCheckpoint,
    store: &mut CommitStore,
    limits: StreamingLimits,
) -> StreamingResult {
    let mut target = PostgresSimulator::new("pg-batch");
    run_streaming_pipeline_bounded(
        source,
        transform_observation,
        &mut target,
        anchor,
        store,
        limits,
    )
}

#[test]
fn bounded_runner_commits_one_local_record_per_batch() {
    let root = tempfile::tempdir().expect("tempdir");
    let (_, binding, anchor, mut store, mut source) = setup(root.path(), "batch-five", 5);
    let result = run(
        &mut source,
        &anchor,
        &mut store,
        StreamingLimits::new(2, usize::MAX, 1),
    );

    assert_eq!(result.report.total_read, 5, "all records were read");
    assert_eq!(result.report.delivered, 5, "all records were delivered");
    assert_eq!(result.report.uncertain, 0, "no uncertain record");
    assert_eq!(
        result.checkpoint.last_committed_sequence, 5,
        "checkpoint advanced to last member"
    );
    let state = load_state(&root.path().join("store"), Some(&binding)).expect("reopen store");
    assert_eq!(
        state.records.len(),
        3,
        "three batches, three local commit records"
    );
}

#[test]
fn bounded_runner_respects_the_byte_limit() {
    let root = tempfile::tempdir().expect("tempdir");
    let (bytes, binding, anchor, mut store, mut source) = setup(root.path(), "batch-bytes", 3);
    let one_record = bytes.trim_end().lines().next().expect("first record").len();
    let limits = StreamingLimits::new(128, one_record + 1, 1);
    let result = run(&mut source, &anchor, &mut store, limits);

    assert_eq!(
        result.report.delivered, 3,
        "byte limit did not drop records"
    );
    let state = load_state(&root.path().join("store"), Some(&binding)).expect("reopen store");
    assert_eq!(
        state.records.len(),
        3,
        "one record per batch under the byte limit"
    );
}

#[test]
fn bounded_runner_quarantines_an_oversize_record_and_continues() {
    let root = tempfile::tempdir().expect("tempdir");
    let oversize = format!(
        "{{\"id\":\"oversize\",\"payload\":{{\"entity_id\":\"{}\",\"kind\":\"credit\",\"value\":1,\"observed_at\":1}}}}",
        "x".repeat(512)
    );
    let bytes = format!("{}\n{}\n{}\n", record(0), oversize, record(2));
    let input = root.path().join("input.jsonl");
    std::fs::write(&input, &bytes).expect("write input");
    let name = "batch-oversize";
    let binding = CheckpointBinding::new(
        &SourceIdentity::register(name, bytes.as_bytes()),
        "pg-batch",
    );
    let anchor = DurableCheckpoint::new(&binding);
    let mut store =
        CommitStore::open(&root.path().join("store"), binding.clone()).expect("open store");
    let mut source = FileSource::open(&input, name).expect("open source");
    let mut target = PostgresSimulator::new("pg-batch");
    let result = run_streaming_pipeline_bounded(
        &mut source,
        transform_observation,
        &mut target,
        &anchor,
        &mut store,
        StreamingLimits::new(8, 256, 1),
    );

    assert_eq!(result.report.total_read, 3, "every line was read");
    assert_eq!(result.report.delivered, 2, "normal records were delivered");
    assert_eq!(result.report.rejected, 1, "oversize record was rejected");
    assert_eq!(
        result.report.quarantined, 1,
        "oversize record was quarantined"
    );
    assert_eq!(
        result.report.accounted(),
        result.report.total_read,
        "accounting is complete"
    );
    assert_eq!(
        result.checkpoint.last_committed_sequence, 3,
        "checkpoint passed the oversize record"
    );
}

#[test]
fn batch_contract_rejects_unordered_and_cross_source_members() {
    assert!(
        DeliveryBatch::new("source", "target", 1, Vec::new()).is_err(),
        "empty batch is rejected"
    );
    let source_id = OpaqueId::derive("src", "test", "batch", "source");
    let observation = RawObservation {
        sequence: 1,
        line_number: 1,
        source_id: source_id.clone(),
        record_id: "batch-0".to_owned(),
        raw_bytes: record(0).into_bytes(),
        provenance: "test:1".to_owned(),
    };
    let event = transform_observation(&observation).expect("transform observation");
    let member = BatchMember {
        event,
        source_position: SourcePosition {
            source_id: source_id.as_str().to_owned(),
            sequence: 1,
            offset: 10,
        },
    };
    let other = BatchMember {
        source_position: SourcePosition {
            sequence: 1,
            offset: 10,
            ..member.source_position.clone()
        },
        ..member.clone()
    };
    assert!(
        DeliveryBatch::new("other-source", "target", 1, vec![member.clone()]).is_err(),
        "cross-source batch is rejected"
    );
    assert!(
        DeliveryBatch::new(source_id.as_str(), "target", 1, vec![member, other]).is_err(),
        "non-advancing positions are rejected"
    );
}
