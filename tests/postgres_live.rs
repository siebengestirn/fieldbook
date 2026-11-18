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
        CanonicalBusinessEvent, DeliveryRequest, DeliveryState, TargetConnector,
        postgres::{PgCheckpoint, PgTarget, apply_migrations, assign_legacy_checkpoint_source},
    },
    canonical::{CANONICAL_SCHEMA_VERSION, CanonicalEnvelope, EventId},
    identity::OpaqueId,
};
use postgres::{Client, Config, NoTls};

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
    assert_ne!(
        persisted_a.checksum, persisted_b.checksum,
        "source checksums differ"
    );

    // 3. A stale writer for A cannot regress A or mutate B.
    {
        let mut stale = PgTarget::with_source_checkpoint(
            &target_id,
            &source_a,
            &mut client,
            PgCheckpoint::initial_for_source(&target_id, SOURCE_A),
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
            PgCheckpoint::initial_for_source(&target_id, SOURCE_A),
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

    // 7. Legacy rows survive migration without silently gaining a source.
    let legacy_target = format!("pg-legacy-{}", std::process::id());
    client
        .execute(
            "INSERT INTO target_checkpoints \
             (target_id, source_id, output_contract_version, last_committed_sequence, \
              last_committed_offset, checkpoint_checksum, last_idempotency_key) \
             VALUES ($1, '', $2, 1, 10, 'sha256:legacy', '')",
            &[
                &legacy_target,
                &(i64::from(engineering_challenge::application::postgres::OUTPUT_CONTRACT_VERSION)),
            ],
        )
        .expect("legacy checkpoint row");
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
        assign_legacy_checkpoint_source(&mut client, &legacy_target, &source_a)
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
    cleanup(&legacy_target);
    cleanup(&concurrent_target);
}
