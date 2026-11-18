#![expect(
    clippy::expect_used,
    reason = "an opt-in live test needs direct, named failures when the operator-supplied database is unavailable"
)]

//! Opt-in live PostgreSQL integration for the generic source/target pipeline.
//!
//! Run with an operator-supplied connection string:
//!
//! ```bash
//! NIRNAY_TEST_PG_URL='host=/tmp port=5432 user=tourist dbname=postgres' \
//!   cargo test --test postgres_live -- --ignored --nocapture
//! ```
//!
//! The test applies an idempotent migration, drives the real `SqlExecutor`
//! transcript, and verifies database-enforced idempotency, transaction
//! isolation, checkpoint progression, lost-ack recovery and restart from the
//! persisted checkpoint. It never uses a simulator. All rows are scoped to a
//! unique target id and removed at the end.

use std::str::FromStr;

use engineering_challenge::{
    application::{
        CanonicalBusinessEvent, DeliveryReceipt, DeliveryRequest, DeliveryState, TargetConnector,
        postgres::{PgTarget, apply_migrations},
    },
    canonical::{CANONICAL_SCHEMA_VERSION, CanonicalEnvelope, EventId},
    identity::OpaqueId,
};
use postgres::{Client, Config, NoTls};

fn envelope(id: &str, entity: &str, value: i64) -> CanonicalEnvelope<CanonicalBusinessEvent> {
    CanonicalEnvelope {
        event_id: EventId::derive("postgres-live", "integration", id),
        instrument_id: None,
        source_id: OpaqueId::derive("jsonl", "test", "postgres-live", "source"),
        feed_id: None,
        connection_epoch: 0,
        sequence: None,
        source_time: Some(1),
        receive_time: 1,
        raw_observation: Some(format!("postgres-live:{id}")),
        schema_version: CANONICAL_SCHEMA_VERSION,
        payload: CanonicalBusinessEvent {
            entity_id: entity.to_owned(),
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

#[test]
#[ignore = "requires an operator-supplied PostgreSQL server; set NIRNAY_TEST_PG_URL"]
fn postgres_live_checkpoint_and_idempotency() {
    let mut client = connect();
    apply_migrations(&mut client).expect("migration applies");
    let target_id = format!("pg-live-{}", std::process::id());

    let first = envelope("evt-1", "acct", 11);
    let mut target = PgTarget::new(&target_id, &mut client);
    target.set_position(1, 10);
    let receipt = target.deliver(&request(first));
    assert_eq!(receipt.state(), DeliveryState::DurableCommitted);
    assert!(!receipt.is_duplicate());
    assert_eq!(target.committed_count().expect("count"), 1);
    assert_eq!(target.checkpoint().sequence, 1);
    assert_eq!(target.checkpoint().offset, 10);

    // Same key, advanced source position: the database conflict is the only
    // idempotency authority and the checkpoint still advances on the retry.
    target.set_position(2, 20);
    let duplicate = target.deliver(&request(envelope("evt-1", "acct", 11)));
    assert_eq!(duplicate.state(), DeliveryState::DurableCommitted);
    assert!(duplicate.is_duplicate());
    assert_eq!(target.committed_count().expect("count"), 1);
    assert_eq!(target.checkpoint().sequence, 2);

    // A distinct event commits once.
    target.set_position(3, 30);
    assert_eq!(
        target
            .deliver(&request(envelope("evt-2", "acct", 22)))
            .state(),
        DeliveryState::DurableCommitted
    );
    assert_eq!(target.committed_count().expect("count"), 2);

    // Ack-loss drill: the transaction commits, the receipt is uncertain and
    // the local checkpoint does not advance. Retry resolves as a duplicate.
    let lost = request(envelope("evt-3", "acct", 33));
    let lost_key = lost.idempotency_key.clone();
    target.inject_uncertain_ack(&lost_key);
    target.set_position(4, 40);
    let uncertain = target.deliver(&lost);
    assert!(matches!(uncertain, DeliveryReceipt::Uncertain { .. }));
    assert_eq!(target.checkpoint().sequence, 3);
    assert_eq!(target.committed_count().expect("count"), 3);
    target.set_position(4, 40);
    let retry = target.deliver(&lost);
    assert!(retry.is_duplicate());
    assert_eq!(target.checkpoint().sequence, 4);
    assert_eq!(target.committed_count().expect("count"), 3);

    let persisted = target
        .persisted_checkpoint()
        .expect("read persisted checkpoint")
        .expect("checkpoint row exists");
    assert_eq!(persisted.sequence, 4);
    assert_eq!(persisted.offset, 40);
    assert_eq!(
        persisted.checksum.len(),
        target.checkpoint().checksum.len(),
        "checksum has stable text shape"
    );

    // Restart from the persisted checkpoint in a fresh connection and target.
    drop(target);
    let mut restarted_client = connect();
    let mut restarted =
        PgTarget::with_checkpoint(&target_id, &mut restarted_client, persisted.clone());
    assert_eq!(restarted.checkpoint(), &persisted);
    restarted.set_position(5, 50);
    assert_eq!(
        restarted
            .deliver(&request(envelope("evt-4", "acct", 44)))
            .state(),
        DeliveryState::DurableCommitted
    );
    assert_eq!(restarted.committed_count().expect("count"), 4);
    let after_restart = restarted
        .persisted_checkpoint()
        .expect("read checkpoint after restart")
        .expect("checkpoint row exists");
    assert_eq!(after_restart.sequence, 5);
    assert!(after_restart.sequence > persisted.sequence);

    // Cleanup is scoped to this target's own rows.
    let mut cleanup = connect();
    cleanup
        .execute(
            "DELETE FROM canonical_business_events WHERE target_id = $1",
            &[&target_id],
        )
        .expect("delete test events");
    cleanup
        .execute(
            "DELETE FROM delivery_dispositions WHERE target_id = $1",
            &[&target_id],
        )
        .expect("delete test dispositions");
    cleanup
        .execute(
            "DELETE FROM target_checkpoints WHERE target_id = $1",
            &[&target_id],
        )
        .expect("delete test checkpoint");
}
