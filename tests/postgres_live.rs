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
    apply_migrations(&mut client).expect("migration is idempotent");
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
        persisted,
        *target.checkpoint(),
        "the database must persist the checksum for the exact sequence and offset"
    );

    // The live target fails closed for invalid request shapes and malformed
    // canonical events; none may mutate the target or checkpoint.
    let empty_request = DeliveryRequest {
        idempotency_key: "empty-request".to_owned(),
        events: Vec::new(),
    };
    target.set_position(5, 50);
    assert_eq!(
        target.deliver(&empty_request).state(),
        DeliveryState::Rejected,
        "an empty request must be rejected before SQL execution"
    );
    let batch_request = DeliveryRequest {
        idempotency_key: "evt-batch-1".to_owned(),
        events: vec![
            envelope("evt-batch-1", "acct", 1),
            envelope("evt-batch-2", "acct", 2),
        ],
    };
    assert_eq!(
        target.deliver(&batch_request).state(),
        DeliveryState::Rejected,
        "a multi-event request must be rejected rather than truncated"
    );
    let mismatched_key = DeliveryRequest {
        idempotency_key: "not-the-event-id".to_owned(),
        events: vec![envelope("evt-key-mismatch", "acct", 1)],
    };
    assert_eq!(
        target.deliver(&mismatched_key).state(),
        DeliveryState::Rejected,
        "the request key must identify its canonical event"
    );
    let malformed = envelope("evt-malformed", "acct", 1);
    let mut malformed = malformed;
    malformed.payload.kind.clear();
    assert_eq!(
        target.deliver(&request(malformed)).state(),
        DeliveryState::Rejected,
        "invalid canonical events must be rejected before SQL execution"
    );
    assert_eq!(
        target
            .committed_count()
            .expect("count after invalid requests"),
        3,
        "invalid requests must not insert canonical event rows"
    );
    assert_eq!(
        target
            .persisted_checkpoint()
            .expect("read checkpoint")
            .expect("checkpoint"),
        persisted,
        "invalid requests must not advance the persisted checkpoint"
    );

    // A stale writer must not regress the checkpoint or leave its event insert
    // behind when the database rejects the older position.
    let mut stale_client = connect();
    let mut stale_writer = PgTarget::new(&target_id, &mut stale_client);
    stale_writer.set_position(1, 10);
    let stale_receipt = stale_writer.deliver(&request(envelope("evt-stale", "acct", 99)));
    assert!(
        matches!(stale_receipt, DeliveryReceipt::Uncertain { .. }),
        "a stale database checkpoint must fail closed"
    );
    assert_eq!(
        stale_writer
            .committed_count()
            .expect("count after stale write"),
        3,
        "the stale transaction must roll back its event insert"
    );
    drop(stale_writer);
    let mut checkpoint_reader = connect();
    let unchanged = checkpoint_reader
        .query_one(
            "SELECT last_committed_sequence, last_committed_offset, checkpoint_checksum, \
                    last_idempotency_key FROM target_checkpoints \
             WHERE target_id = $1 AND output_contract_version = $2",
            &[
                &target_id,
                &(i64::from(engineering_challenge::application::postgres::OUTPUT_CONTRACT_VERSION)),
            ],
        )
        .expect("read checkpoint after stale writer");
    assert_eq!(
        unchanged.get::<_, i64>(0),
        4,
        "the stale writer must not regress the source sequence"
    );
    assert_eq!(
        unchanged.get::<_, i64>(1),
        40,
        "the stale writer must not regress the source byte offset"
    );
    assert_eq!(
        unchanged.get::<_, String>(2),
        persisted.checksum,
        "a rejected stale writer must preserve the checkpoint checksum"
    );
    assert_eq!(
        unchanged.get::<_, String>(3),
        persisted.last_idempotency_key.clone().unwrap_or_default(),
        "a rejected stale writer must preserve the position's idempotency key"
    );

    // A second writer may retry an identical position and key, but may not
    // attach a different event to an already committed source position.
    let mut position_client = connect();
    let mut position_writer = PgTarget::new(&target_id, &mut position_client);
    position_writer.set_position(5, 50);
    let position_owner = request(envelope("evt-position-owner", "acct", 55));
    assert_eq!(
        position_writer.deliver(&position_owner).state(),
        DeliveryState::DurableCommitted,
        "the first event at a new source position must commit"
    );
    let same_position_retry = position_writer.deliver(&position_owner);
    assert!(
        same_position_retry.is_duplicate(),
        "an exact retry at the same source position must be idempotent"
    );
    target.set_position(5, 50);
    let conflicting_position = request(envelope("evt-position-conflict", "acct", 56));
    assert!(
        matches!(
            target.deliver(&conflicting_position),
            DeliveryReceipt::Uncertain { .. }
        ),
        "a different event cannot claim an already committed source position"
    );
    assert_eq!(
        target
            .committed_count()
            .expect("count after position conflict"),
        4,
        "a conflicting source-position transaction must roll back its event insert"
    );
    let current_persisted = position_writer
        .persisted_checkpoint()
        .expect("read current checkpoint")
        .expect("checkpoint exists");
    assert_eq!(
        current_persisted.sequence, 5,
        "the competing writer's committed sequence must remain current"
    );
    assert_eq!(
        current_persisted.offset, 50,
        "the competing writer's committed byte offset must remain current"
    );
    assert_eq!(
        current_persisted.last_idempotency_key.as_deref(),
        Some(position_owner.idempotency_key.as_str()),
        "the persisted checkpoint must bind the last position to its event key"
    );

    // Reusing a previously committed event id with a changed payload is not a
    // duplicate: the database must reject it and preserve the newer checkpoint.
    target.set_position(6, 60);
    let changed_payload = request(envelope("evt-1", "acct", 999));
    assert!(
        matches!(
            target.deliver(&changed_payload),
            DeliveryReceipt::Uncertain { .. }
        ),
        "an idempotency key cannot be reused for different canonical content"
    );
    assert_eq!(
        target
            .committed_count()
            .expect("count after payload conflict"),
        4,
        "a changed-payload retry must not insert another event"
    );
    assert_eq!(
        target
            .persisted_checkpoint()
            .expect("read checkpoint after payload conflict")
            .expect("checkpoint remains present"),
        current_persisted,
        "a changed-payload retry must not advance the checkpoint"
    );

    // Restart from the persisted checkpoint in a fresh connection and target.
    drop(position_writer);
    drop(target);
    let mut restarted_client = connect();
    let mut restarted =
        PgTarget::with_checkpoint(&target_id, &mut restarted_client, current_persisted.clone());
    assert_eq!(
        restarted.checkpoint(),
        &current_persisted,
        "restart must begin from the verified persisted checkpoint"
    );
    restarted.set_position(6, 60);
    assert_eq!(
        restarted
            .deliver(&request(envelope("evt-4", "acct", 44)))
            .state(),
        DeliveryState::DurableCommitted
    );
    assert_eq!(
        restarted.committed_count().expect("count"),
        5,
        "restart must commit exactly one new event"
    );
    let after_restart = restarted
        .persisted_checkpoint()
        .expect("read checkpoint after restart")
        .expect("checkpoint row exists");
    assert_eq!(
        after_restart.sequence, 6,
        "the restarted target must persist the next sequence"
    );
    assert!(
        after_restart.sequence > current_persisted.sequence,
        "the restarted checkpoint must advance monotonically"
    );

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
