//! Platform P6 end-to-end evidence for the durable reference pipeline.
//!
//! All tests are deterministic and offline. They exercise the streaming file
//! source, the local checksummed commit store, the corrected delivery
//! semantics and the PostgreSQL contract. No driver, database or network is
//! used; the target is the repository's deterministic simulator.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration tests drive failure paths and assert deterministic store state"
)]
#![allow(
    clippy::allow_attributes,
    reason = "test crates relax the production lint set wholesale"
)]

use std::io::Write as _;

use engineering_challenge::application::{
    Checkpoint, ExecutionMode, ImportReport, JsonlSource, NoopCommitSink, SourceAdapter,
    durable::{CommitStore, DurableState, StoreFault, load_state, run_streaming_pipeline},
    file_source::{
        CheckpointBinding, DurableCheckpoint, FileSource, SourceFingerprint, SourceIdentity,
        SourceRegistry,
    },
    postgres::{
        IdempotencyKey, OUTPUT_CONTRACT_VERSION, PostgresFault, PostgresTarget,
        build_commit_transcript, finalize_transcript,
    },
    run_jsonl_to_target_with_mode, transform_observation,
};

fn record(id: &str, value: i64) -> String {
    format!(
        "{{\"id\":\"{id}\",\"payload\":{{\"entity_id\":\"entity-{id}\",\"kind\":\"credit\",\"value\":{value},\"observed_at\":{value}}}}}"
    )
}

fn lines(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| record(&format!("id-{index}"), index as i64))
        .collect()
}

fn text(count: usize) -> String {
    lines(count).join("\n")
}

fn binding_for(name: &str, bytes: &[u8], target_id: &str) -> CheckpointBinding {
    CheckpointBinding::new(&SourceIdentity::register(name, bytes), target_id)
}

fn store_at(directory: &std::path::Path, name: &str, bytes: &[u8], target_id: &str) -> CommitStore {
    CommitStore::open(directory, binding_for(name, bytes, target_id)).expect("open store")
}

fn accounted(report: &ImportReport) -> u64 {
    report.delivered
        + report.duplicates
        + report.rejected
        + report.outstanding
        + report.uncertain
        + report.accepted
        + report.skipped
        + report.target_delivered
        + report.simulated
}

#[test]
fn restart_from_durable_store_preserves_previously_committed_records() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = text(3).into_bytes();

    let mut first_source = JsonlSource::from_str("fixture", &text(3));
    let mut first_target = PostgresTarget::with_fault(
        "pg-local",
        PostgresFault::ServerUnavailableAfter { successful: 1 },
    );
    let first = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut first_source,
            transform_observation,
            &mut first_target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };
    assert_eq!(first.delivered, 1);
    assert_eq!(first.outstanding, 2);
    assert_eq!(first.last_committed_sequence, 1);
    assert_eq!(accounted(&first), first.total_read);

    // Reopen: the first record is still present in the durable store.
    let reopened = store_at(directory.path(), "fixture", &bytes, "pg-local");
    let persisted: DurableState = reopened.state().clone();
    assert_eq!(persisted.records.len(), 1);
    assert_eq!(persisted.committed_event_ids().len(), 1);
    assert_eq!(
        persisted
            .checkpoint()
            .expect("checkpoint")
            .last_committed_sequence,
        1
    );

    let mut second_source = JsonlSource::from_str("fixture", &text(3));
    let mut second_target = PostgresTarget::new("pg-local");
    let second = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut second_source,
            transform_observation,
            &mut second_target,
            &Checkpoint {
                last_committed_sequence: 1,
            },
            ExecutionMode::Live,
            &mut store,
        )
    };
    assert_eq!(second.skipped, 1);
    assert_eq!(second.delivered, 2);
    assert_eq!(second.last_committed_sequence, 3);

    let final_store = store_at(directory.path(), "fixture", &bytes, "pg-local");
    assert_eq!(final_store.state().records.len(), 3);
    assert_eq!(final_store.state().committed_event_ids().len(), 3);
    assert_eq!(final_store.verify().expect("verify").records.len(), 3);
}

#[test]
fn db_unavailable_leaves_checkpoint_and_durable_store_unmoved() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = text(2).into_bytes();
    let mut source = JsonlSource::from_str("fixture", &text(2));
    let mut target = PostgresTarget::with_fault("pg-local", PostgresFault::ServerUnavailable);
    let report = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };
    assert_eq!(report.outstanding, 2);
    assert_eq!(report.delivered, 0);
    assert_eq!(report.last_committed_sequence, 0);
    assert_eq!(accounted(&report), report.total_read);

    let store = store_at(directory.path(), "fixture", &bytes, "pg-local");
    assert!(store.state().records.is_empty());
    assert!(store.state().checkpoint().is_none());
    assert!(
        target
            .statements()
            .iter()
            .all(|statement| statement.sql != "COMMIT")
    );
}

#[test]
fn quarantine_must_succeed_before_the_checkpoint_advances() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = "{not json}\n".as_bytes().to_vec();

    let mut failed_source = JsonlSource::from_str("fixture", "{not json}");
    let mut target = PostgresTarget::new("pg-local");
    let failed = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        store.set_fault(StoreFault::FailBeforeWrite);
        run_jsonl_to_target_with_mode(
            &mut failed_source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };
    assert_eq!(failed.uncertain, 1);
    assert_eq!(failed.rejected, 0);
    assert_eq!(failed.last_committed_sequence, 0);
    assert!(target.statements().is_empty());

    let mut source = JsonlSource::from_str("fixture", "{not json}");
    let recovered = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };
    assert_eq!(recovered.rejected, 1);
    assert_eq!(recovered.quarantined, 1);
    assert_eq!(recovered.last_committed_sequence, 1);

    let store = store_at(directory.path(), "fixture", &bytes, "pg-local");
    assert_eq!(store.state().quarantine_count(), 1);
    assert!(store.state().committed_event_ids().is_empty());
}

#[test]
fn simulated_modes_report_simulated_and_never_write_externally() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = text(2).into_bytes();
    let mut source = JsonlSource::from_str("fixture", &text(2));
    let mut target = PostgresTarget::new("pg-local");
    let dry_run = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
            ExecutionMode::DryRun,
            &mut store,
        )
    };
    assert_eq!(dry_run.simulated, 2);
    assert_eq!(dry_run.durable_committed, 0);
    assert_eq!(dry_run.last_committed_sequence, 0);
    assert_eq!(accounted(&dry_run), dry_run.total_read);
    assert_eq!(target.simulator().committed_count(), 0);
    assert!(target.statements().is_empty());
    assert!(target.receipts().is_empty());
    let store = store_at(directory.path(), "fixture", &bytes, "pg-local");
    assert!(store.state().records.is_empty());
    for receipt in &dry_run.obligations {
        assert_eq!(receipt.state.as_str(), "simulated");
    }

    // Replay through the side-effect-free connector also records Simulated
    // receipts and leaves the wrapped target untouched.
    let mut replay_source = JsonlSource::from_str("fixture", &text(2));
    let mut wrapped = PostgresTarget::new("pg-local");
    let replay = {
        let mut connector = engineering_challenge::application::ReplayConnector::new(&mut wrapped);
        run_jsonl_to_target_with_mode(
            &mut replay_source,
            transform_observation,
            &mut connector,
            &Checkpoint::default(),
            ExecutionMode::Replay,
            &mut NoopCommitSink,
        )
    };
    assert_eq!(replay.simulated, 2);
    assert_eq!(replay.last_committed_sequence, 0);
    assert_eq!(wrapped.simulator().committed_count(), 0);
    assert!(wrapped.statements().is_empty());
}

#[test]
fn multi_target_obligations_are_independent() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = text(2).into_bytes();

    let mut a_source = JsonlSource::from_str("fixture", &text(2));
    let mut a_target = PostgresTarget::with_fault("pg-a", PostgresFault::ServerUnavailable);
    let a_store_path = directory.path().join("a");
    let a = {
        let mut store = store_at(&a_store_path, "fixture", &bytes, "pg-a");
        run_jsonl_to_target_with_mode(
            &mut a_source,
            transform_observation,
            &mut a_target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };

    let mut b_source = JsonlSource::from_str("fixture", &text(2));
    let mut b_target = PostgresTarget::new("pg-b");
    let b_store_path = directory.path().join("b");
    let b = {
        let mut store = store_at(&b_store_path, "fixture", &bytes, "pg-b");
        run_jsonl_to_target_with_mode(
            &mut b_source,
            transform_observation,
            &mut b_target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };

    assert_eq!(a.outstanding, 2);
    assert_eq!(a.delivered, 0);
    assert_eq!(a.last_committed_sequence, 0);
    assert_eq!(b.delivered, 2);
    assert_eq!(b.last_committed_sequence, 2);
    assert!(a.obligations.iter().all(|item| item.target_id == "pg-a"));
    assert!(b.obligations.iter().all(|item| item.target_id == "pg-b"));

    let a_store = store_at(&a_store_path, "fixture", &bytes, "pg-a");
    let b_store = store_at(&b_store_path, "fixture", &bytes, "pg-b");
    assert!(a_store.state().records.is_empty());
    assert_eq!(b_store.state().committed_event_ids().len(), 2);

    let key_a = IdempotencyKey::new("evt-1", "pg-a", OUTPUT_CONTRACT_VERSION);
    let key_b = IdempotencyKey::new("evt-1", "pg-b", OUTPUT_CONTRACT_VERSION);
    assert_ne!(key_a.as_text(), key_b.as_text());
}

#[test]
fn uncertain_ack_resolves_with_the_same_target_specific_key() {
    let directory = tempfile::tempdir().expect("tempdir");
    let bytes = text(1).into_bytes();
    let mut source = JsonlSource::from_str("fixture", &text(1));
    let mut target = PostgresTarget::with_fault("pg-local", PostgresFault::AckLost);
    let first = {
        let mut store = store_at(directory.path(), "fixture", &bytes, "pg-local");
        run_jsonl_to_target_with_mode(
            &mut source,
            transform_observation,
            &mut target,
            &Checkpoint::default(),
            ExecutionMode::Live,
            &mut store,
        )
    };
    // The retry with the same idempotency key resolves the uncertain ack as a
    // duplicate; exactly one target row and one durable commit result.
    assert_eq!(first.duplicates, 1);
    assert_eq!(first.uncertain, 0);
    assert_eq!(first.last_committed_sequence, 1);
    assert_eq!(target.simulator().committed_count(), 1);
    assert_eq!(
        target
            .simulator()
            .attempts_for(target.receipts()[0].idempotency_key()),
        2
    );

    let store = store_at(directory.path(), "fixture", &bytes, "pg-local");
    assert_eq!(store.state().records.len(), 1);
    assert_eq!(store.state().committed_event_ids().len(), 1);

    let event = transform_observation(
        &JsonlSource::from_str("fixture", &text(1))
            .read_one()
            .expect("read")
            .expect("observation"),
    )
    .expect("transform");
    let key = IdempotencyKey::new(event.event_id.as_str(), "pg-local", OUTPUT_CONTRACT_VERSION);
    let mut transcript = build_commit_transcript(
        &event,
        "pg-local",
        OUTPUT_CONTRACT_VERSION,
        1,
        0,
        "sha256:checksum",
        "durable_committed",
        "",
    );
    let receipt_key = transcript.idempotency_key.as_text();
    assert_eq!(receipt_key, key.as_text());
    let rows: Vec<&str> = transcript
        .statements
        .iter()
        .map(|statement| statement.sql.as_str())
        .collect();
    assert!(rows[1].contains("ON CONFLICT (idempotency_key) DO NOTHING"));
    assert!(rows[2].contains("ON CONFLICT (idempotency_key) DO NOTHING"));
    assert!(rows[3].contains("ON CONFLICT (target_id, output_contract_version) DO UPDATE"));
    assert!(!transcript.has_commit());
    finalize_transcript(
        &mut transcript,
        &engineering_challenge::application::DeliveryReceipt::DurableCommitted {
            idempotency_key: receipt_key,
            target_id: "pg-local".to_owned(),
            duplicate: true,
        },
    );
    assert!(transcript.has_commit());
    assert_eq!(
        transcript.statements.last().map(|item| item.sql.as_str()),
        Some("COMMIT")
    );
}

#[test]
fn streaming_file_source_restarts_from_a_verified_offset() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("input.jsonl");
    let bytes = text(3).into_bytes();
    {
        let mut file = std::fs::File::create(&path).expect("create");
        file.write_all(&bytes).expect("write");
        file.sync_all().expect("sync");
    }

    // Fingerprint is computed once over the full file, before streaming.
    let identity = SourceIdentity::register("file", &bytes);
    let mut reader = FileSource::open(&path, "file").expect("open");
    assert_eq!(
        reader.identity().fingerprint(),
        &SourceFingerprint::compute(&bytes)
    );
    let first = reader.read_one().expect("read").expect("observation");
    assert_eq!(first.raw_bytes, lines(3)[0].as_bytes());
    assert_eq!(reader.last_offset(), 0);
    let second = reader.read_one().expect("read").expect("observation");
    assert_eq!(second.raw_bytes, lines(3)[1].as_bytes());
    assert_eq!(reader.last_offset(), (lines(3)[0].len() + 1) as u64);

    let mut registry = SourceRegistry::new();
    let registered = registry.register("file", &bytes).clone();
    assert_eq!(registered, identity);

    let binding = CheckpointBinding::new(&registered, "pg-local");
    let store_path = directory.path().join("store");
    let anchor = DurableCheckpoint::new(&binding);
    let first_run = {
        let mut source = FileSource::open(&path, "file").expect("open");
        let mut target = PostgresTarget::with_fault(
            "pg-local",
            PostgresFault::ServerUnavailableAfter { successful: 1 },
        );
        let mut store = CommitStore::open(&store_path, binding.clone()).expect("store");
        run_streaming_pipeline(
            &mut source,
            transform_observation,
            &mut target,
            &anchor,
            &mut store,
        )
    };
    assert_eq!(first_run.report.delivered, 1);
    assert_eq!(first_run.report.outstanding, 2);
    assert_eq!(first_run.checkpoint.last_committed_sequence, 1);
    assert_eq!(
        first_run.checkpoint.last_committed_offset,
        (lines(3)[0].len() + 1) as u64
    );

    let reopened = CommitStore::open(&store_path, binding.clone()).expect("reopen");
    assert_eq!(reopened.state().committed_event_ids().len(), 1);

    let mut resumed =
        FileSource::open_with_resume(&path, "file", &first_run.checkpoint, "pg-local")
            .expect("resume");
    let resumed_first = resumed.read_one().expect("read").expect("observation");
    assert_eq!(resumed_first.record_id, "id-1");
    assert_eq!(resumed_first.sequence, 2);

    let mut source = FileSource::open_with_resume(&path, "file", &first_run.checkpoint, "pg-local")
        .expect("resume");
    let mut target = PostgresTarget::new("pg-local");
    let second_run = {
        let mut store = CommitStore::open(&store_path, binding).expect("store");
        run_streaming_pipeline(
            &mut source,
            transform_observation,
            &mut target,
            &first_run.checkpoint,
            &mut store,
        )
    };
    assert_eq!(second_run.report.delivered, 2);
    assert_eq!(second_run.report.skipped, 0);
    assert_eq!(second_run.checkpoint.last_committed_sequence, 3);
    assert_eq!(
        second_run.checkpoint.last_committed_offset,
        bytes.len() as u64
    );

    let final_store = CommitStore::open(&store_path, binding_for("file", &bytes, "pg-local"))
        .expect("final reopen");
    assert_eq!(final_store.state().records.len(), 3);
    assert_eq!(final_store.state().committed_event_ids().len(), 3);
}

#[test]
fn changed_file_cannot_resume_from_a_stale_checkpoint() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("input.jsonl");
    let original = text(2).into_bytes();
    std::fs::write(&path, &original).expect("write");
    let binding = binding_for("file", &original, "pg-local");
    let anchor = DurableCheckpoint::new(&binding).advanced(1, (lines(2)[0].len() + 1) as u64);

    let changed = text(2).replace("value\":0", "value\":9").into_bytes();
    assert_ne!(
        SourceFingerprint::compute(&original),
        SourceFingerprint::compute(&changed)
    );
    std::fs::write(&path, &changed).expect("rewrite");
    assert!(FileSource::open_with_resume(&path, "file", &anchor, "pg-local").is_err());
    assert!(load_state(directory.path(), Some(&binding)).is_ok());
}
