//! Opt-in real PostgreSQL evidence for bounded, ordered batch delivery.
//!
//! ```bash
//! FIELDBOOK_TEST_PG_URL='host=/tmp port=5432 user=tourist dbname=postgres' \
//!   cargo test --test postgres_batch_live -- --ignored --nocapture
//! ```
//!
//! These tests require an operator-provisioned server. The simulator suite in
//! `tests/p6_bounded_batch.rs` is not a substitute for this certification.

#![expect(
    clippy::expect_used,
    reason = "an opt-in live test needs direct, named failures when the operator server is unavailable"
)]

use std::{
    env, //
    fs,  //
    str::FromStr,
};

use fieldbook::application::{
    TargetConnector,
    durable::{CommitStore, StoreFault, load_state, run_streaming_pipeline_bounded},
    file_source::{CheckpointBinding, DurableCheckpoint, FileSource, SourceIdentity},
    postgres::{PgTarget, apply_migrations},
    transform_observation,
};
use postgres::{
    Client, //
    Config, //
    NoTls,  //
};

fn record(index: usize) -> String {
    format!(
        "{{\"id\":\"batch-live-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{index},\"observed_at\":{index}}},\"received_at\":{index}}}"
    )
}

fn connect() -> Client {
    let url = env::var("FIELDBOOK_TEST_PG_URL")
        .expect("set FIELDBOOK_TEST_PG_URL to an operator-supplied PostgreSQL connection string");
    let config = Config::from_str(&url).expect("valid PostgreSQL URL");
    config.connect(NoTls).expect("connect to live PostgreSQL")
}

fn cleanup(client: &mut Client, target_id: &str) {
    for statement in [
        "DELETE FROM canonical_business_events WHERE target_id = $1",
        "DELETE FROM delivery_dispositions WHERE target_id = $1",
        "DELETE FROM target_checkpoints WHERE target_id = $1",
    ] {
        client
            .execute(statement, &[&target_id])
            .expect("scoped batch cleanup");
    }
}

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set FIELDBOOK_TEST_PG_URL"]
fn postgres_live_bounded_batch_drives_real_target() {
    let mut client = connect();
    apply_migrations(&mut client).expect("migration applies");
    let target_id = format!("pg-live-batch-{}", std::process::id());
    cleanup(&mut client, &target_id);

    let root = tempfile::tempdir().expect("tempdir");
    let name = "pg-live-batch-source";
    let bytes = format!("{}\n", (0..6).map(record).collect::<Vec<_>>().join("\n"));
    let input = root.path().join("input.jsonl");
    fs::write(&input, &bytes).expect("write input");
    let identity = SourceIdentity::register(name, bytes.as_bytes());
    let binding = CheckpointBinding::new(&identity, &target_id);
    let anchor = DurableCheckpoint::new(&binding);
    let mut store =
        CommitStore::open(&root.path().join("store"), binding.clone()).expect("open store");

    let result = {
        let mut source = FileSource::open(&input, name).expect("open source");
        let mut target =
            PgTarget::with_source(&target_id, identity.source_id().as_str(), &mut client);
        run_streaming_pipeline_bounded(
            &mut source,
            transform_observation,
            &mut target,
            &anchor,
            &mut store,
            fieldbook::application::durable::StreamingLimits::new(2, usize::MAX, 1),
        )
    };

    assert_eq!(
        result.report.delivered, 6,
        "all batch members were delivered"
    );
    assert_eq!(result.report.uncertain, 0, "no unresolved batch");
    assert_eq!(
        result.checkpoint.last_committed_sequence, 6,
        "checkpoint reached final member"
    );
    let rows: i64 = client
        .query_one(
            "SELECT count(*) FROM canonical_business_events WHERE target_id = $1",
            &[&target_id],
        )
        .expect("count committed events")
        .get(0);
    assert_eq!(rows, 6, "one row per committed event");
    let state = load_state(&root.path().join("store"), Some(&binding)).expect("reopen store");
    assert_eq!(
        state.records.len(),
        3,
        "three batches, three local commit records"
    );
    cleanup(&mut client, &target_id);
}

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set FIELDBOOK_TEST_PG_URL"]
fn postgres_live_batch_exact_retry_and_changed_payload_are_distinguished() {
    let mut client = connect();
    apply_migrations(&mut client).expect("migration applies");
    let target_id = format!("pg-live-batch-retry-{}", std::process::id());
    cleanup(&mut client, &target_id);

    let root = tempfile::tempdir().expect("tempdir");
    let name = "pg-live-batch-retry-source";
    let bytes = format!("{}\n{}\n", record(0), record(1));
    let input = root.path().join("input.jsonl");
    fs::write(&input, &bytes).expect("write input");
    let identity = SourceIdentity::register(name, bytes.as_bytes());
    let source_id = identity.source_id().as_str().to_owned();

    let observation = |index: usize, value: i64| {
        fieldbook::application::RawObservation {
        sequence: index as u64 + 1,
        line_number: index + 1,
        source_id: identity.source_id().clone(),
        record_id: format!("batch-live-{index}"),
        raw_bytes: format!(
            "{{\"id\":\"batch-live-{index}\",\"payload\":{{\"entity_id\":\"acct\",\"kind\":\"credit\",\"value\":{value},\"observed_at\":{index}}},\"received_at\":{index}}}"
        )
        .into_bytes(),
        provenance: format!("retry:{index}"),
    }
    };
    let batch = |positions: Vec<(u64, u64)>, values: Vec<i64>| {
        let members: Vec<_> = positions
            .into_iter()
            .zip(values)
            .enumerate()
            .map(|(index, ((sequence, offset), value))| {
                let obs = observation(index, value);
                let event = transform_observation(&obs).expect("transform observation");
                fieldbook::application::BatchMember {
                    event,
                    source_position: fieldbook::application::SourcePosition {
                        source_id: source_id.clone(),
                        sequence,
                        offset,
                    },
                }
            })
            .collect();
        fieldbook::application::DeliveryBatch::new(&source_id, &target_id, 1, members)
            .expect("valid batch")
    };

    let first = batch(vec![(1, 10), (2, 20)], vec![0, 1]);
    {
        let mut target = PgTarget::with_source(&target_id, &source_id, &mut client);
        let receipts = target.deliver_batch(&first);
        assert!(
            receipts.iter().all(|receipt| matches!(
                receipt,
                fieldbook::application::DeliveryReceipt::DurableCommitted { .. }
            )),
            "first batch committed"
        );
        let retry = target.deliver_batch(&first);
        assert!(
            retry.iter().all(|receipt| receipt.is_duplicate()),
            "exact retry is reported as duplicate"
        );
    }

    let changed = batch(vec![(1, 10), (2, 20)], vec![0, 99]);
    {
        let mut target = PgTarget::with_source(&target_id, &source_id, &mut client);
        let receipts = target.deliver_batch(&changed);
        assert!(
            receipts.iter().any(|receipt| matches!(
                receipt,
                fieldbook::application::DeliveryReceipt::Uncertain { .. }
            )),
            "changed payload conflicts with the persisted canonical row"
        );
    }
    let rows: i64 = client
        .query_one(
            "SELECT count(*) FROM canonical_business_events WHERE target_id = $1",
            &[&target_id],
        )
        .expect("count committed events")
        .get(0);
    assert_eq!(rows, 2, "no partial row survived the conflicting batch");
    cleanup(&mut client, &target_id);
}

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set FIELDBOOK_TEST_PG_URL"]
fn postgres_live_batch_replays_after_local_commit_failure() {
    let mut client = connect();
    apply_migrations(&mut client).expect("migration applies");
    let target_id = format!("pg-live-batch-replay-{}", std::process::id());
    cleanup(&mut client, &target_id);

    let root = tempfile::tempdir().expect("tempdir");
    let name = "pg-live-batch-replay-source";
    let bytes = format!("{}\n", (0..6).map(record).collect::<Vec<_>>().join("\n"));
    let input = root.path().join("input.jsonl");
    fs::write(&input, &bytes).expect("write input");
    let identity = SourceIdentity::register(name, bytes.as_bytes());
    let binding = CheckpointBinding::new(&identity, &target_id);
    let anchor = DurableCheckpoint::new(&binding);
    let store_path = root.path().join("store");
    let mut store = CommitStore::open(&store_path, binding.clone()).expect("open store");
    store.set_fault(StoreFault::FailBeforeWrite);

    {
        let mut source = FileSource::open(&input, name).expect("open source");
        let mut target =
            PgTarget::with_source(&target_id, identity.source_id().as_str(), &mut client);
        let first = run_streaming_pipeline_bounded(
            &mut source,
            transform_observation,
            &mut target,
            &anchor,
            &mut store,
            fieldbook::application::durable::StreamingLimits::new(2, usize::MAX, 1),
        );
        assert_eq!(
            first.report.uncertain, 2,
            "target committed but local append failed"
        );
    }

    let mut replay_store = CommitStore::open(&store_path, binding.clone()).expect("reopen store");
    let resume_anchor = anchor.clone();
    let resumed = {
        let mut source = FileSource::open_with_resume(&input, name, &resume_anchor, &target_id)
            .expect("open source with resume");
        let mut target =
            PgTarget::with_source(&target_id, identity.source_id().as_str(), &mut client);
        run_streaming_pipeline_bounded(
            &mut source,
            transform_observation,
            &mut target,
            &resume_anchor,
            &mut replay_store,
            fieldbook::application::durable::StreamingLimits::new(3, usize::MAX, 1),
        )
    };
    assert_eq!(
        resumed.report.delivered, 4,
        "remaining records delivered at a different batch size"
    );
    assert_eq!(
        resumed.checkpoint.last_committed_sequence, 6,
        "checkpoint reached final member"
    );
    let rows: i64 = client
        .query_one(
            "SELECT count(*) FROM canonical_business_events WHERE target_id = $1",
            &[&target_id],
        )
        .expect("count committed events")
        .get(0);
    assert_eq!(rows, 6, "replay produced no duplicate rows");
    cleanup(&mut client, &target_id);
}
