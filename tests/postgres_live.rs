#![expect(
    clippy::expect_used,
    reason = "an opt-in live test needs direct, named failures when the operator-supplied database is unavailable"
)]

//! Opt-in live PostgreSQL integration for source-scoped durable delivery.
//!
//! ```bash
//! NIRNAY_TEST_PG_URL='host=/tmp port=5432 user=tourist dbname=postgres' \
//!   cargo test --test postgres_live -- --ignored --nocapture
//! ```
//!
//! Every scenario runs against an actual server. The simulator is not used.
//! Rows are scoped to a unique target id and removed at the end.

use std::str::FromStr;

use engineering_challenge::{
    application::{
        CanonicalBusinessEvent, DeliveryReceipt, DeliveryRequest, DeliveryState, TargetConnector,
        postgres::{
            IdempotencyKey, OUTPUT_CONTRACT_VERSION, PgCheckpoint, PgTarget, apply_migrations,
            assign_legacy_checkpoint_source,
        },
    },
    canonical::{CANONICAL_SCHEMA_VERSION, CanonicalEnvelope, EventId},
    identity::OpaqueId,
};
use postgres::{
    Client,
    Config,
    NoTls,
    //
};
use sha2::{
    Digest,
    Sha256,
    //
};

const SOURCE_A: &str = "stream-a";
const SOURCE_B: &str = "stream-b";

fn canonical_source(source: &str) -> String {
    OpaqueId::derive("jsonl", "test", source, "source")
        .as_str()
        .to_owned()
}

fn envelope(id: &str, source: &str, value: i64) -> CanonicalEnvelope<CanonicalBusinessEvent> {
    CanonicalEnvelope {
        event_id: EventId::derive("postgres-live", source, id),
        instrument_id: None,
        source_id: OpaqueId::derive("jsonl", "test", source, "source"),
        feed_id: None,
        connection_epoch: 0,
        sequence: None,
        source_time: Some(1),
        receive_time: 1,
        raw_observation: Some(format!("postgres-live:{source}:{id}")),
        schema_version: CANONICAL_SCHEMA_VERSION,
        payload: CanonicalBusinessEvent {
            entity_id: "acct".to_owned(),
            kind: "credit".to_owned(),
            value,
            observed_at: 1,
        },
    }
}

fn request(event: CanonicalEnvelope<CanonicalBusinessEvent>) -> DeliveryRequest {
    DeliveryRequest {
        idempotency_key: event.event_id.as_str().to_owned(),
        events: vec![event],
    }
}

fn connect() -> Client {
    let url = std::env::var("NIRNAY_TEST_PG_URL").expect(
        "set NIRNAY_TEST_PG_URL to an operator-supplied PostgreSQL connection string; \
         this test never invents credentials",
    );
    let config = Config::from_str(&url).expect("valid PostgreSQL URL");
    config.connect(NoTls).expect("connect to live PostgreSQL")
}

fn legacy_idempotency_key(event_id: &str, target_id: &str, version: u32) -> String {
    format!("{event_id}\u{1f}{target_id}\u{1f}{version}")
}

fn empty_source_idempotency_key(event_id: &str, target_id: &str, version: u32) -> String {
    format!("{event_id}\u{1f}{target_id}\u{1f}\u{1f}{version}")
}

fn legacy_v2_checksum(target_id: &str, sequence: u64, offset: u64, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"pg-checkpoint-v2\0");
    hasher.update(
        u64::try_from(target_id.len())
            .expect("test target id length fits u64")
            .to_le_bytes(),
    );
    hasher.update(target_id.as_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(offset.to_le_bytes());
    hasher.update(
        u64::try_from(key.len())
            .expect("test idempotency key length fits u64")
            .to_le_bytes(),
    );
    hasher.update(key.as_bytes());
    let mut checksum = String::from("sha256:");
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(&mut checksum, "{byte:02x}").expect("write to string");
    }
    checksum
}

fn cleanup(target_id: &str) {
    let mut client = connect();
    for statement in [
        "DELETE FROM canonical_business_events WHERE target_id = $1",
        "DELETE FROM delivery_dispositions WHERE target_id = $1",
        "DELETE FROM target_checkpoints WHERE target_id = $1",
    ] {
        client
            .execute(statement, &[&target_id])
            .expect("scoped cleanup");
    }
}

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set NIRNAY_TEST_PG_URL"]
fn postgres_live_source_scoped_delivery() {
    let mut client = connect();
    apply_migrations(&mut client).expect("migration applies");
    apply_migrations(&mut client).expect("migration is idempotent");
    let target_id = format!("pg-live-source-{}", std::process::id());
    let source_a = canonical_source(SOURCE_A);
    let source_b = canonical_source(SOURCE_B);

    // 1. Two distinct sources each commit sequence 1, offset 10 to one target.
    let first_a = request(envelope("evt-a1", SOURCE_A, 11));
    let first_b = request(envelope("evt-b1", SOURCE_B, 21));
    {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        a.set_position(1, 10);
        let receipt = a.deliver(&first_a);
        assert_eq!(
            receipt.state(),
            DeliveryState::DurableCommitted,
            "{receipt:?}"
        );
        assert_eq!(
            a.source_committed_count().expect("count A"),
            1,
            "verified by the source-scoped integration contract"
        );
        let mut b = PgTarget::with_source(&target_id, &source_b, &mut client);
        b.set_position(1, 10);
        assert_eq!(
            b.deliver(&first_b).state(),
            DeliveryState::DurableCommitted,
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            b.source_committed_count().expect("count B"),
            1,
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM canonical_business_events WHERE target_id = $1",
                    &[&target_id]
                )
                .expect("total")
                .get::<_, i64>(0),
            2,
            "verified by the source-scoped integration contract"
        );
    }

    // A legacy target may not erase the source on a modern envelope.
    let legacy_api_target = format!("pg-legacy-api-{}", std::process::id());
    {
        let mut legacy = PgTarget::new(&legacy_api_target, &mut client);
        legacy.set_position(1, 10);
        let receipt = legacy.deliver(&request(envelope("evt-legacy-api", SOURCE_A, 31)));
        assert_eq!(receipt.state(), DeliveryState::Rejected, "{receipt:?}");
        assert_eq!(
            legacy
                .source_committed_count()
                .expect("legacy target row count"),
            0,
            "the legacy namespace must not store a modern source event"
        );
    }

    // A checkpoint from source A cannot be transplanted into a target bound
    // to source B, even when the envelope itself names source B.
    let transplanted_target = format!("pg-transplanted-{}", std::process::id());
    {
        let checkpoint_a = PgCheckpoint::initial_for_source(&transplanted_target, &source_a);
        let mut target_b = PgTarget::with_source_checkpoint(
            &transplanted_target,
            &source_b,
            &mut client,
            checkpoint_a,
        );
        target_b.set_position(1, 10);
        let receipt = target_b.deliver(&request(envelope(
            "evt-checkpoint-transplant",
            SOURCE_B,
            32,
        )));
        assert_eq!(receipt.state(), DeliveryState::Rejected, "{receipt:?}");
        assert_eq!(
            target_b
                .source_committed_count()
                .expect("source B row count"),
            0,
            "a foreign checkpoint must not write an event"
        );
        assert!(
            target_b
                .persisted_checkpoint()
                .expect("read source B checkpoint")
                .is_none(),
            "a foreign checkpoint must not create a checkpoint row"
        );
    }

    // 2. Each source reloads only its own checkpoint after restart.
    let persisted_a = {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        a.persisted_checkpoint()
            .expect("read A checkpoint")
            .expect("A checkpoint exists")
    };
    let persisted_b = {
        let mut b = PgTarget::with_source(&target_id, &source_b, &mut client);
        b.persisted_checkpoint()
            .expect("read B checkpoint")
            .expect("B checkpoint exists")
    };
    assert_eq!(
        persisted_a.sequence, 1,
        "verified by the source-scoped integration contract"
    );
    assert_eq!(
        persisted_b.sequence, 1,
        "verified by the source-scoped integration contract"
    );
    assert_eq!(
        persisted_a.source_id, source_a,
        "verified by the source-scoped integration contract"
    );
    assert_eq!(
        persisted_b.source_id, source_b,
        "verified by the source-scoped integration contract"
    );
    let checksum_a = persisted_a.checksum;
    let checksum_b = persisted_b.checksum;
    assert_ne!(checksum_a, checksum_b, "source checksums differ");

    // 3. A stale writer for A cannot regress A or mutate B.
    {
        let mut stale = PgTarget::with_source_checkpoint(
            &target_id,
            &source_a,
            &mut client,
            PgCheckpoint::initial_for_source(&target_id, &source_a),
        );
        stale.set_position(1, 10);
        let stale_receipt = stale.deliver(&request(envelope("evt-a2", SOURCE_A, 12)));
        assert_ne!(
            stale_receipt.state(),
            DeliveryState::DurableCommitted,
            "verified by the source-scoped integration contract"
        );
    }
    {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        assert_eq!(
            a.persisted_checkpoint()
                .expect("A checkpoint")
                .unwrap()
                .sequence,
            1,
            "verified by the source-scoped integration contract"
        );
        let mut b = PgTarget::with_source(&target_id, &source_b, &mut client);
        assert_eq!(
            b.persisted_checkpoint()
                .expect("B checkpoint")
                .unwrap()
                .sequence,
            1,
            "verified by the source-scoped integration contract"
        );
    }

    // 3b. A target bound to A rejects an envelope from source B without any
    // row or checkpoint mutation.
    {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        let before = a
            .source_committed_count()
            .expect("count before foreign envelope");
        let receipt = a.deliver(&request(envelope("evt-foreign", SOURCE_B, 999)));
        assert_eq!(receipt.state(), DeliveryState::Rejected, "{receipt:?}");
        assert!(
            matches!(&receipt, DeliveryReceipt::Rejected { reason, .. } if reason.contains("does not match target source")),
            "the rejection must name the source mismatch"
        );
        assert_eq!(
            a.source_committed_count()
                .expect("count after foreign envelope"),
            before,
            "a foreign-source envelope must not commit a row"
        );
        assert_eq!(
            a.persisted_checkpoint()
                .expect("A checkpoint")
                .unwrap()
                .sequence,
            1,
            "a foreign-source envelope must not move the checkpoint"
        );
    }

    // 4. Same-source exact retries remain idempotent.
    {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        a.set_position(1, 10);
        let retry = a.deliver(&first_a.clone());
        assert_eq!(
            retry.state(),
            DeliveryState::DurableCommitted,
            "verified by the source-scoped integration contract"
        );
        assert!(
            retry.is_duplicate(),
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            a.source_committed_count().expect("count A"),
            1,
            "verified by the source-scoped integration contract"
        );
    }

    // 5. Reusing an idempotency key with changed canonical content fails closed.
    let mut changed = envelope("evt-a1", SOURCE_A, 999);
    changed.raw_observation = Some("changed-payload".to_owned());
    {
        let mut a = PgTarget::with_source(&target_id, &source_a, &mut client);
        a.set_position(2, 20);
        let changed_receipt = a.deliver(&request(changed));
        assert_ne!(
            changed_receipt.state(),
            DeliveryState::DurableCommitted,
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            a.source_committed_count().expect("count A"),
            1,
            "verified by the source-scoped integration contract"
        );
        assert_eq!(
            a.persisted_checkpoint()
                .expect("A checkpoint")
                .unwrap()
                .sequence,
            1,
            "verified by the source-scoped integration contract"
        );
    }

    // 6. A failed checkpoint race leaves no partial event behind.
    {
        let racing = request(envelope("evt-race", SOURCE_A, 77));
        let mut stale = PgTarget::with_source_checkpoint(
            &target_id,
            &source_a,
            &mut client,
            PgCheckpoint::initial_for_source(&target_id, &source_a),
        );
        stale.set_position(1, 10);
        let receipt = stale.deliver(&racing);
        assert_ne!(
            receipt.state(),
            DeliveryState::DurableCommitted,
            "verified by the source-scoped integration contract"
        );
        let rows = client
            .query_one(
                "SELECT count(*) FROM canonical_business_events WHERE idempotency_key = $1",
                &[&racing.idempotency_key],
            )
            .expect("race row count")
            .get::<_, i64>(0);
        assert_eq!(
            rows, 0,
            "a rejected checkpoint race must not commit an event"
        );
    }

    // 7. Legacy rows retain historical checksum validation before an
    // operator may assign them to a source namespace.
    let invalid_legacy_target = format!("pg-legacy-invalid-{}", std::process::id());
    client
        .execute(
            "INSERT INTO target_checkpoints \
             (target_id, source_id, output_contract_version, last_committed_sequence, \
              last_committed_offset, checkpoint_checksum, last_idempotency_key) \
             VALUES ($1, '', $2, 1, 10, 'sha256:legacy', '')",
            &[&invalid_legacy_target, &i64::from(OUTPUT_CONTRACT_VERSION)],
        )
        .expect("invalid legacy checkpoint row");
    let invalid_mapping = assign_legacy_checkpoint_source(
        &mut client,
        &invalid_legacy_target,
        &source_a,
        OUTPUT_CONTRACT_VERSION,
    );
    assert!(
        invalid_mapping.is_err(),
        "an invalid legacy checksum must not be rebound to a source"
    );
    let invalid_source: String = client
        .query_one(
            "SELECT source_id FROM target_checkpoints WHERE target_id = $1",
            &[&invalid_legacy_target],
        )
        .expect("invalid legacy row remains")
        .get(0);
    assert!(invalid_source.is_empty(), "failed mapping must be atomic");

    // Equal row counts are insufficient: the event and disposition identity
    // sets must match exactly before a legacy stream can be assigned.
    let mismatched_legacy_target = format!("pg-legacy-mismatch-{}", std::process::id());
    for event_id in ["evt-pair-a", "evt-pair-b"] {
        let key =
            legacy_idempotency_key(event_id, &mismatched_legacy_target, OUTPUT_CONTRACT_VERSION);
        client
            .execute(
                "INSERT INTO canonical_business_events \
                 (idempotency_key, event_id, target_id, source_id, entity_id, kind, value, \
                  observed_at, raw_observation, schema_version) \
                 VALUES ($1, $2, $3, '', 'acct', 'credit', 1, 1, '', $4)",
                &[
                    &key,
                    &event_id,
                    &mismatched_legacy_target,
                    &i64::from(CANONICAL_SCHEMA_VERSION),
                ],
            )
            .expect("legacy event for pair mismatch");
    }
    for event_id in ["evt-pair-a", "evt-pair-c"] {
        let key =
            legacy_idempotency_key(event_id, &mismatched_legacy_target, OUTPUT_CONTRACT_VERSION);
        client
            .execute(
                "INSERT INTO delivery_dispositions \
                 (idempotency_key, event_id, target_id, source_id, disposition, reason, raw_observation) \
                 VALUES ($1, $2, $3, '', 'durable_committed', '', '')",
                &[&key, &event_id, &mismatched_legacy_target],
            )
            .expect("legacy disposition for pair mismatch");
    }
    let mismatch_checksum = legacy_v2_checksum(&mismatched_legacy_target, 1, 10, "evt-pair-a");
    client
        .execute(
            "INSERT INTO target_checkpoints \
             (target_id, source_id, output_contract_version, last_committed_sequence, \
              last_committed_offset, checkpoint_checksum, last_idempotency_key) \
             VALUES ($1, '', $2, 1, 10, $3, 'evt-pair-a')",
            &[
                &mismatched_legacy_target,
                &i64::from(OUTPUT_CONTRACT_VERSION),
                &mismatch_checksum,
            ],
        )
        .expect("legacy checkpoint with mismatched row sets");
    let mismatch_result = assign_legacy_checkpoint_source(
        &mut client,
        &mismatched_legacy_target,
        &source_a,
        OUTPUT_CONTRACT_VERSION,
    );
    assert!(
        mismatch_result.is_err(),
        "event/disposition identity mismatch must fail source assignment"
    );
    let unmoved_events: i64 = client
        .query_one(
            "SELECT count(*) FROM canonical_business_events WHERE target_id = $1 AND source_id = ''",
            &[&mismatched_legacy_target],
        )
        .expect("legacy events remain after rollback")
        .get(0);
    let unmoved_dispositions: i64 = client
        .query_one(
            "SELECT count(*) FROM delivery_dispositions WHERE target_id = $1 AND source_id = ''",
            &[&mismatched_legacy_target],
        )
        .expect("legacy dispositions remain after rollback")
        .get(0);
    assert_eq!(unmoved_events, 2, "event re-key must roll back atomically");
    assert_eq!(
        unmoved_dispositions, 2,
        "disposition re-key must roll back atomically"
    );

    // A non-empty checkpoint event key must resolve to rows at the exact
    // contract-version key, not merely to some event in another namespace.
    let orphan_legacy_target = format!("pg-legacy-orphan-{}", std::process::id());
    let orphan_checksum = legacy_v2_checksum(&orphan_legacy_target, 1, 10, "evt-checkpoint-orphan");
    client
        .execute(
            "INSERT INTO target_checkpoints \
             (target_id, source_id, output_contract_version, last_committed_sequence, \
              last_committed_offset, checkpoint_checksum, last_idempotency_key) \
             VALUES ($1, '', $2, 1, 10, $3, 'evt-checkpoint-orphan')",
            &[
                &orphan_legacy_target,
                &i64::from(OUTPUT_CONTRACT_VERSION),
                &orphan_checksum,
            ],
        )
        .expect("orphan legacy checkpoint");
    assert!(
        assign_legacy_checkpoint_source(
            &mut client,
            &orphan_legacy_target,
            &source_a,
            OUTPUT_CONTRACT_VERSION,
        )
        .is_err(),
        "a checkpoint anchor without matching event and disposition rows must fail"
    );
    let orphan_source: String = client
        .query_one(
            "SELECT source_id FROM target_checkpoints WHERE target_id = $1",
            &[&orphan_legacy_target],
        )
        .expect("orphan checkpoint remains legacy")
        .get(0);
    assert!(orphan_source.is_empty(), "orphan mapping must roll back");

    let legacy_target = format!("pg-legacy-{}", std::process::id());
    let legacy_event = envelope("evt-legacy", SOURCE_A, 55);
    let legacy_key = legacy_event.event_id.as_str().to_owned();
    let legacy_idempotency_key =
        legacy_idempotency_key(&legacy_key, &legacy_target, OUTPUT_CONTRACT_VERSION);
    let legacy_checksum = legacy_v2_checksum(&legacy_target, 1, 10, &legacy_key);
    client
        .execute(
            "INSERT INTO canonical_business_events \
             (idempotency_key, event_id, target_id, source_id, entity_id, kind, value, \
              observed_at, raw_observation, schema_version) \
             VALUES ($1, $2, $3, '', $4, $5, $6, $7, $8, $9)",
            &[
                &legacy_idempotency_key,
                &legacy_key,
                &legacy_target,
                &legacy_event.payload.entity_id,
                &legacy_event.payload.kind,
                &legacy_event.payload.value,
                &legacy_event.payload.observed_at,
                &legacy_event.raw_observation.as_deref().unwrap_or(""),
                &i64::from(legacy_event.schema_version),
            ],
        )
        .expect("legacy event row");
    client
        .execute(
            "INSERT INTO delivery_dispositions \
             (idempotency_key, event_id, target_id, source_id, disposition, reason, raw_observation) \
             VALUES ($1, $2, $3, '', 'durable_committed', '', $4)",
            &[
                &legacy_idempotency_key,
                &legacy_key,
                &legacy_target,
                &legacy_event.raw_observation.as_deref().unwrap_or(""),
            ],
        )
        .expect("legacy disposition row");
    let current_empty_source_event_id = "evt-legacy-empty-source";
    let current_empty_source_key = empty_source_idempotency_key(
        current_empty_source_event_id,
        &legacy_target,
        OUTPUT_CONTRACT_VERSION,
    );
    client
        .execute(
            "INSERT INTO canonical_business_events \
             (idempotency_key, event_id, target_id, source_id, entity_id, kind, value, \
              observed_at, raw_observation, schema_version) \
             VALUES ($1, $2, $3, '', 'acct', 'credit', 56, 1, '', $4)",
            &[
                &current_empty_source_key,
                &current_empty_source_event_id,
                &legacy_target,
                &i64::from(CANONICAL_SCHEMA_VERSION),
            ],
        )
        .expect("current empty-source event row");
    client
        .execute(
            "INSERT INTO delivery_dispositions \
             (idempotency_key, event_id, target_id, source_id, disposition, reason, raw_observation) \
             VALUES ($1, $2, $3, '', 'durable_committed', '', '')",
            &[
                &current_empty_source_key,
                &current_empty_source_event_id,
                &legacy_target,
            ],
        )
        .expect("current empty-source disposition row");
    client
        .execute(
            "INSERT INTO target_checkpoints \
             (target_id, source_id, output_contract_version, last_committed_sequence, \
              last_committed_offset, checkpoint_checksum, last_idempotency_key) \
             VALUES ($1, '', $2, 1, 10, $3, $4)",
            &[
                &legacy_target,
                &i64::from(OUTPUT_CONTRACT_VERSION),
                &legacy_checksum,
                &legacy_key,
            ],
        )
        .expect("valid historical legacy checkpoint row");
    apply_migrations(&mut client).expect("migration preserves legacy rows");
    let legacy_source: String = client
        .query_one(
            "SELECT source_id FROM target_checkpoints WHERE target_id = $1",
            &[&legacy_target],
        )
        .expect("legacy row")
        .get(0);
    assert!(
        legacy_source.is_empty(),
        "verified by the source-scoped integration contract"
    );
    assert!(
        assign_legacy_checkpoint_source(
            &mut client,
            &legacy_target,
            &source_a,
            OUTPUT_CONTRACT_VERSION
        )
        .expect("explicit legacy mapping"),
        "an explicit operator mapping must move the legacy row exactly once"
    );
    let mapped_source: String = client
        .query_one(
            "SELECT source_id FROM target_checkpoints WHERE target_id = $1",
            &[&legacy_target],
        )
        .expect("mapped legacy row")
        .get(0);
    assert_eq!(
        mapped_source, source_a,
        "verified by the source-scoped integration contract"
    );
    // The event, disposition and checkpoint must all move atomically, and
    // their source-scoped key must preserve duplicate detection after restart.
    let migrated_event_row = client
        .query_one(
            "SELECT source_id, idempotency_key FROM canonical_business_events \
             WHERE event_id = $1 AND target_id = $2",
            &[&legacy_key, &legacy_target],
        )
        .expect("migrated event row");
    assert_eq!(
        migrated_event_row.get::<_, String>(0),
        source_a,
        "legacy event provenance must be rebound"
    );
    let expected_source_key = IdempotencyKey::with_source(
        &legacy_key,
        &legacy_target,
        &source_a,
        OUTPUT_CONTRACT_VERSION,
    )
    .as_text();
    assert_eq!(
        migrated_event_row.get::<_, String>(1),
        expected_source_key,
        "the event idempotency key must move to the source namespace"
    );
    let migrated_disposition = client
        .query_one(
            "SELECT source_id, idempotency_key FROM delivery_dispositions \
             WHERE event_id = $1 AND target_id = $2",
            &[&legacy_key, &legacy_target],
        )
        .expect("migrated disposition row");
    assert_eq!(
        migrated_disposition.get::<_, String>(0),
        source_a,
        "legacy disposition provenance must be rebound"
    );
    assert_eq!(
        migrated_disposition.get::<_, String>(1),
        expected_source_key,
        "the disposition idempotency key must move to the source namespace"
    );
    let migrated_empty_source_event = client
        .query_one(
            "SELECT source_id, idempotency_key FROM canonical_business_events \
             WHERE event_id = $1 AND target_id = $2",
            &[&current_empty_source_event_id, &legacy_target],
        )
        .expect("migrated current empty-source event");
    let expected_empty_source_key = format!(
        "{current_empty_source_event_id}\u{1f}{legacy_target}\u{1f}{source_a}\u{1f}{OUTPUT_CONTRACT_VERSION}"
    );
    assert_eq!(
        migrated_empty_source_event.get::<_, String>(0),
        source_a,
        "the current empty-source event provenance must be rebound"
    );
    assert_eq!(
        migrated_empty_source_event.get::<_, String>(1),
        expected_empty_source_key,
        "the current empty-source event key must move to source scope"
    );
    let reloaded = {
        let mut mapped_reader = PgTarget::with_source(&legacy_target, &source_a, &mut client);
        let reloaded = mapped_reader
            .persisted_checkpoint()
            .expect("mapped checkpoint reload")
            .expect("mapped checkpoint exists");
        assert_eq!(reloaded.sequence, 1, "mapped position must be preserved");
        assert_eq!(reloaded.offset, 10, "mapped offset must be preserved");
        assert_eq!(
            reloaded.last_idempotency_key.as_deref(),
            Some(legacy_key.as_str()),
            "the historical retry key must survive source mapping"
        );
        assert_eq!(
            reloaded.source_id, source_a,
            "mapped source must be claimed"
        );
        reloaded
    };
    let mut resumed =
        PgTarget::with_source_checkpoint(&legacy_target, &source_a, &mut client, reloaded);
    resumed.set_position(1, 10);
    let retry = resumed.deliver(&request(legacy_event));
    assert_eq!(retry.state(), DeliveryState::DurableCommitted, "{retry:?}");
    assert!(
        retry.is_duplicate(),
        "a replayed legacy event must deduplicate"
    );
    assert_eq!(
        resumed
            .source_committed_count()
            .expect("migrated row count"),
        2,
        "legacy replay must not create a second canonical event"
    );
    let remapped = assign_legacy_checkpoint_source(
        &mut client,
        &legacy_target,
        &source_a,
        OUTPUT_CONTRACT_VERSION,
    )
    .expect("repeated mapping is a no-op");
    assert!(
        !remapped,
        "an already-mapped checkpoint must not be remapped"
    ); // a second operator claim must be a no-op

    // 8. Two independent connections cannot violate per-source monotonicity.
    let concurrent_target = format!("pg-concurrent-{}", std::process::id());
    let outcomes = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            let mut client = connect();
            apply_migrations(&mut client).expect("migration");
            let mut target = PgTarget::with_source(&concurrent_target, &source_a, &mut client);
            target.set_position(1, 10);
            target
                .deliver(&request(envelope("evt-concurrent-a", SOURCE_A, 1)))
                .state()
        });
        let b = scope.spawn(|| {
            let mut client = connect();
            apply_migrations(&mut client).expect("migration");
            let mut target = PgTarget::with_source(&concurrent_target, &source_a, &mut client);
            target.set_position(1, 10);
            target
                .deliver(&request(envelope("evt-concurrent-b", SOURCE_A, 2)))
                .state()
        });
        [a.join().expect("thread A"), b.join().expect("thread B")]
    });
    assert_eq!(
        outcomes
            .iter()
            .filter(|state| **state == DeliveryState::DurableCommitted)
            .count(),
        1,
        "exactly one writer must win at the same source position"
    );
    {
        let mut client = connect();
        let checkpoint: i64 = client
            .query_one(
                "SELECT last_committed_sequence FROM target_checkpoints \
                 WHERE target_id = $1 AND source_id = $2",
                &[&concurrent_target, &source_a],
            )
            .expect("concurrent checkpoint")
            .get(0);
        assert_eq!(
            checkpoint, 1,
            "verified by the source-scoped integration contract"
        );
    }

    cleanup(&target_id);
    cleanup(&legacy_api_target);
    cleanup(&transplanted_target);
    cleanup(&legacy_target);
    cleanup(&invalid_legacy_target);
    cleanup(&mismatched_legacy_target);
    cleanup(&orphan_legacy_target);
    cleanup(&concurrent_target);
}
